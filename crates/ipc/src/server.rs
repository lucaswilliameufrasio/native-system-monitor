use crate::{
    MAX_ACTIVE_CLIENTS, PROTOCOL_MAJOR, PROTOCOL_MINOR,
    frame::{read_message, write_message},
    wire::{
        Capability as WireCapability, CpuMetrics as WireCpuMetrics,
        MemoryMetrics as WireMemoryMetrics, Operation, ProcessMetrics as WireProcessMetrics,
        Request, Response, Snapshot as WireSnapshot,
    },
};
use anyhow::{Context, Result};
use interprocess::local_socket::{
    tokio::Listener as LocalSocketListener, traits::StreamCommon as _, traits::tokio::Listener as _,
};
use native_monitor_collector_api::Capability;
use native_monitor_domain::SystemSnapshot;
use std::io;
use std::time::Duration;
use tokio::sync::{Semaphore, watch};
use tokio::time::timeout;
use tracing::{debug, warn};

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_PROCESSES_PER_RESPONSE: usize = 1_000;

pub async fn serve(
    snapshots: watch::Receiver<std::sync::Arc<SystemSnapshot>>,
    mut shutdown: watch::Receiver<bool>,
    max_processes_per_response: usize,
    capabilities: std::sync::Arc<Vec<Capability>>,
) -> Result<()> {
    let listener = create_listener().context("bind the per-user local IPC endpoint")?;
    let active_clients = std::sync::Arc::new(Semaphore::new(MAX_ACTIVE_CLIENTS));

    tracing::info!("local IPC endpoint is accepting requests");

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let mut stream = match accepted {
                    Ok(stream) => stream,
                    Err(error) => {
                        warn!(%error, "local IPC accept failed");
                        continue;
                    }
                };

                if !peer_is_current_user(&stream) {
                    warn!("rejected a local IPC client with a different or unavailable identity");
                    continue;
                }

                let permit = match active_clients.clone().try_acquire_owned() {
                    Ok(permit) => permit,
                    Err(_) => {
                        debug!("rejected IPC request because all client slots are occupied");
                        continue;
                    }
                };

                let snapshot = std::sync::Arc::clone(&snapshots.borrow());
                let capabilities = std::sync::Arc::clone(&capabilities);
                tokio::spawn(async move {
                    let _permit = permit;
                    match timeout(
                        CONNECTION_TIMEOUT,
                        handle_connection(
                            &mut stream,
                            &snapshot,
                            max_processes_per_response,
                            &capabilities,
                        ),
                    )
                    .await
                    {
                        Err(error) => {
                            debug!(%error, "IPC client exceeded the connection time limit");
                        }
                        Ok(Err(error)) => {
                            debug!(%error, "IPC request failed");
                        }
                        Ok(Ok(())) => {}
                    }
                });
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
        }
    }

    Ok(())
}

async fn handle_connection(
    stream: &mut interprocess::local_socket::tokio::Stream,
    snapshot: &SystemSnapshot,
    max_processes_per_response: usize,
    capabilities: &[Capability],
) -> io::Result<()> {
    let request: Request = read_message(stream).await?;
    let response = handle_request(request, snapshot, max_processes_per_response, capabilities);
    write_message(stream, &response).await
}

fn handle_request(
    request: Request,
    snapshot: &SystemSnapshot,
    max_processes_per_response: usize,
    capabilities: &[Capability],
) -> Response {
    if request.protocol_major != PROTOCOL_MAJOR {
        return error_response(
            "protocol_version_mismatch",
            format!(
                "client protocol major {} is incompatible with agent protocol major {}",
                request.protocol_major, PROTOCOL_MAJOR
            ),
        );
    }

    let operation = match Operation::try_from(request.operation) {
        Ok(operation) => operation,
        Err(_) => return error_response("invalid_operation", "unknown operation".to_owned()),
    };

    match operation {
        Operation::Status => Response {
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR,
            status: "running".to_owned(),
            ..Response::default()
        },
        Operation::Capabilities => Response {
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR,
            status: "running".to_owned(),
            capabilities: capabilities
                .iter()
                .map(|capability| WireCapability {
                    name: capability.name.to_owned(),
                    status: capability.status.to_owned(),
                    source: capability.source.to_owned(),
                })
                .collect(),
            ..Response::default()
        },
        Operation::Snapshot | Operation::Processes => {
            let mut processes = snapshot.processes.clone();
            match request.sort_by.as_str() {
                "memory" => processes.sort_by(|left, right| {
                    right
                        .memory_bytes
                        .cmp(&left.memory_bytes)
                        .then_with(|| left.identity.pid.cmp(&right.identity.pid))
                }),
                "cpu" | "" => processes.sort_by(|left, right| {
                    compare_cpu_usage(left.cpu_usage_percent, right.cpu_usage_percent)
                        .then_with(|| left.identity.pid.cmp(&right.identity.pid))
                }),
                _ => {
                    return error_response(
                        "invalid_sort",
                        "sort_by must be either cpu or memory".to_owned(),
                    );
                }
            }

            let limit = if request.process_limit == 0 {
                100
            } else {
                request.process_limit as usize
            }
            .min(MAX_PROCESSES_PER_RESPONSE);
            let limit = limit.min(max_processes_per_response.max(1));

            let processes = processes
                .into_iter()
                .take(limit)
                .map(|process| WireProcessMetrics {
                    pid: process.identity.pid,
                    parent_pid: process.parent_pid,
                    name: process.name,
                    cpu_usage_percent: process.cpu_usage_percent,
                    memory_bytes: process.memory_bytes,
                    start_time_unix_seconds: process.identity.start_time_unix_seconds,
                })
                .collect();

            Response {
                protocol_major: PROTOCOL_MAJOR,
                protocol_minor: PROTOCOL_MINOR,
                status: snapshot.collection_status.clone(),
                snapshot: Some(WireSnapshot {
                    captured_at_unix_ms: snapshot.captured_at_unix_ms,
                    cpu: Some(WireCpuMetrics {
                        total_usage_percent: snapshot.cpu.total_usage_percent,
                        per_core_usage_percent: snapshot.cpu.per_core_usage_percent.clone(),
                        status: snapshot.cpu.status.as_str().to_owned(),
                    }),
                    memory: Some(WireMemoryMetrics {
                        total_bytes: snapshot.memory.total_bytes,
                        used_bytes: snapshot.memory.used_bytes,
                        swap_total_bytes: snapshot.memory.swap_total_bytes,
                        swap_used_bytes: snapshot.memory.swap_used_bytes,
                        status: snapshot.memory.status.as_str().to_owned(),
                    }),
                    processes,
                    cpu_history_percent: snapshot.cpu_history_percent.clone(),
                    memory_history_percent: snapshot.memory_history_percent.clone(),
                    collection_status: snapshot.collection_status.clone(),
                }),
                ..Response::default()
            }
        }
        Operation::Unspecified => {
            error_response("invalid_operation", "operation is required".to_owned())
        }
    }
}

fn compare_cpu_usage(left: Option<f32>, right: Option<f32>) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => right.total_cmp(&left),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn error_response(error_code: &str, error_message: String) -> Response {
    Response {
        protocol_major: PROTOCOL_MAJOR,
        protocol_minor: PROTOCOL_MINOR,
        error_code: error_code.to_owned(),
        error_message,
        ..Response::default()
    }
}

#[cfg(unix)]
fn create_listener() -> io::Result<LocalSocketListener> {
    crate::unix_endpoint::prepare_runtime_directory()?;
    create_listener_at(&crate::unix_endpoint::socket_path())
}

#[cfg(unix)]
fn create_listener_at(path: &std::path::Path) -> io::Result<LocalSocketListener> {
    use interprocess::local_socket::{GenericFilePath, ListenerOptions, ToFsName};

    let name = path.to_fs_name::<GenericFilePath>()?;
    ListenerOptions::new().name(name).create_tokio()
}

#[cfg(windows)]
fn create_listener() -> io::Result<LocalSocketListener> {
    use interprocess::local_socket::{GenericNamespaced, ListenerOptions, ToNsName};
    use interprocess::os::windows::local_socket::ListenerOptionsExt as _;
    use interprocess::os::windows::security_descriptor::SecurityDescriptor;
    use widestring::U16CString;

    let name = pipe_name();
    let socket_name = name.to_ns_name::<GenericNamespaced>()?;
    let sddl = U16CString::from_str("D:(A;;GA;;;OW)")
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let security_descriptor = SecurityDescriptor::deserialize(sddl.as_ucstr())?;

    ListenerOptions::new()
        .name(socket_name)
        .security_descriptor(security_descriptor)
        .create_tokio()
}

#[cfg(windows)]
fn pipe_name() -> String {
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_owned());
    let safe_user = user
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .take(48)
        .collect::<String>();
    let safe_user = if safe_user.is_empty() {
        "unknown".to_owned()
    } else {
        safe_user
    };

    format!("native-system-monitor-{safe_user}")
}

#[cfg(unix)]
fn peer_is_current_user(stream: &interprocess::local_socket::tokio::Stream) -> bool {
    let expected_user_id = crate::unix_identity::effective_user_id();
    stream
        .peer_creds()
        .map(|credentials| credentials.euid() == Some(expected_user_id))
        .unwrap_or(false)
}

#[cfg(windows)]
fn peer_is_current_user(stream: &interprocess::local_socket::tokio::Stream) -> bool {
    stream
        .peer_creds()
        .map(|credentials| credentials.pid().is_some())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{MAX_PROCESSES_PER_RESPONSE, handle_request};
    use crate::{
        PROTOCOL_MAJOR,
        wire::{Operation, Request},
    };
    use native_monitor_domain::SystemSnapshot;

    #[cfg(unix)]
    #[tokio::test]
    async fn binds_local_listener_without_requiring_socket_file_chmod_support() {
        use super::create_listener_at;
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_TEST_PATH: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_TEST_PATH.fetch_add(1, Ordering::Relaxed);
        let runtime_directory = std::env::temp_dir().join(format!(
            "native-system-monitor-listener-test-{}-{id}",
            std::process::id()
        ));
        crate::unix_endpoint::ensure_private_directory(&runtime_directory)
            .expect("create private test runtime directory");

        let socket_path = runtime_directory.join("agent.sock");
        let listener = create_listener_at(&socket_path).expect("bind local IPC listener");
        drop(listener);

        std::fs::remove_dir(runtime_directory).expect("remove test runtime directory");
    }

    #[test]
    fn rejects_incompatible_protocol_major() {
        let response = handle_request(
            Request {
                protocol_major: PROTOCOL_MAJOR + 1,
                operation: Operation::Status as i32,
                ..Request::default()
            },
            &SystemSnapshot::starting(),
            MAX_PROCESSES_PER_RESPONSE,
            &[],
        );

        assert_eq!(response.error_code, "protocol_version_mismatch");
    }

    #[test]
    fn rejects_unbounded_process_limits_by_applying_the_cap() {
        use native_monitor_domain::{ProcessIdentity, ProcessMetrics};

        let mut source = SystemSnapshot::starting();
        for pid in 1..=1_005 {
            source.processes.push(ProcessMetrics {
                identity: ProcessIdentity {
                    pid,
                    start_time_unix_seconds: 1,
                },
                parent_pid: None,
                name: format!("process-{pid}"),
                cpu_usage_percent: Some(0.0),
                memory_bytes: 0,
            });
        }

        let response = handle_request(
            Request {
                protocol_major: PROTOCOL_MAJOR,
                operation: Operation::Processes as i32,
                process_limit: u32::MAX,
                sort_by: "cpu".to_owned(),
                ..Request::default()
            },
            &source,
            MAX_PROCESSES_PER_RESPONSE,
            &[],
        );

        let snapshot = response.snapshot.expect("snapshot response");
        assert_eq!(snapshot.processes.len(), 1_000);
    }

    #[test]
    fn preserves_cpu_topology_changes_in_snapshot_responses() {
        let hotplug_fixtures = [
            ("before-core-offline", vec![12.0, 18.0, 25.0, 31.0]),
            ("after-core-offline", vec![14.0, 22.0, 28.0]),
        ];

        for (_fixture_name, per_core_usage_percent) in hotplug_fixtures {
            let mut source = SystemSnapshot::starting();
            source.cpu.total_usage_percent = Some(20.0);
            source.cpu.per_core_usage_percent = per_core_usage_percent.clone();

            let response = handle_request(
                Request {
                    protocol_major: PROTOCOL_MAJOR,
                    operation: Operation::Snapshot as i32,
                    process_limit: 1,
                    sort_by: "cpu".to_owned(),
                    ..Request::default()
                },
                &source,
                MAX_PROCESSES_PER_RESPONSE,
                &[],
            );

            let cpu = response
                .snapshot
                .expect("snapshot response")
                .cpu
                .expect("CPU metrics");
            assert_eq!(cpu.per_core_usage_percent, per_core_usage_percent);
        }
    }

    #[test]
    fn large_process_fixture_stays_within_the_coarse_latency_guard() {
        use native_monitor_domain::{ProcessIdentity, ProcessMetrics};
        use std::time::{Duration, Instant};

        let mut source = SystemSnapshot::starting();
        for pid in 1..=50_000 {
            source.processes.push(ProcessMetrics {
                identity: ProcessIdentity {
                    pid,
                    start_time_unix_seconds: 1,
                },
                parent_pid: None,
                name: format!("process-{pid}"),
                cpu_usage_percent: Some((pid % 101) as f32),
                memory_bytes: u64::from(pid) * 1024,
            });
        }

        let started = Instant::now();
        let response = handle_request(
            Request {
                protocol_major: PROTOCOL_MAJOR,
                operation: Operation::Processes as i32,
                process_limit: 1_000,
                sort_by: "cpu".to_owned(),
                ..Request::default()
            },
            &source,
            MAX_PROCESSES_PER_RESPONSE,
            &[],
        );
        let elapsed = started.elapsed();

        assert_eq!(
            response
                .snapshot
                .expect("snapshot response")
                .processes
                .len(),
            1_000
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "50,000-process fixture exceeded the 5-second smoke guard: {elapsed:?}"
        );
    }
}
