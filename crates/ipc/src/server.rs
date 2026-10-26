use crate::{
    MAX_ACTIVE_CLIENTS, PROTOCOL_MAJOR, PROTOCOL_MINOR,
    frame::{read_message, write_message},
    wire::{
        BlockDeviceMetrics as WireBlockDeviceMetrics, Capability as WireCapability,
        CpuMetrics as WireCpuMetrics, MemoryMetrics as WireMemoryMetrics,
        NetworkInterfaceMetrics as WireNetworkInterfaceMetrics, Operation,
        ProcessMetrics as WireProcessMetrics, Request, Response, Snapshot as WireSnapshot,
        VolumeMetrics as WireVolumeMetrics,
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
const MAX_PROCESS_FILTER_CHARS: usize = 256;

pub async fn serve(
    snapshots: watch::Receiver<std::sync::Arc<SystemSnapshot>>,
    mut shutdown: watch::Receiver<bool>,
    max_processes_per_response: usize,
    history_interval_ms: u64,
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
                            history_interval_ms,
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
    history_interval_ms: u64,
    capabilities: &[Capability],
) -> io::Result<()> {
    let request: Request = read_message(stream).await?;
    let response = handle_request(
        request,
        snapshot,
        max_processes_per_response,
        history_interval_ms,
        capabilities,
    );
    write_message(stream, &response).await
}

fn handle_request(
    request: Request,
    snapshot: &SystemSnapshot,
    max_processes_per_response: usize,
    history_interval_ms: u64,
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
            let sort_by = if request.process_sort_key.is_empty() {
                if request.sort_by.is_empty() {
                    "cpu"
                } else {
                    request.sort_by.as_str()
                }
            } else {
                request.process_sort_key.as_str()
            };
            if !matches!(
                sort_by,
                "cpu" | "memory" | "name" | "pid" | "parent" | "start"
            ) {
                return error_response(
                    "invalid_sort",
                    "sort_by must be cpu, memory, name, pid, parent, or start".to_owned(),
                );
            }
            if request.process_filter.chars().count() > MAX_PROCESS_FILTER_CHARS {
                return error_response(
                    "invalid_filter",
                    format!("process_filter must be at most {MAX_PROCESS_FILTER_CHARS} characters"),
                );
            }
            let sort_descending = request
                .sort_descending
                .unwrap_or(matches!(sort_by, "cpu" | "memory"));
            let process_filter = request.process_filter.to_lowercase();
            let mut processes = Vec::with_capacity(snapshot.processes.len());
            for process in &snapshot.processes {
                let normalized_name = if !process_filter.is_empty() || sort_by == "name" {
                    Some(process.name.to_lowercase())
                } else {
                    None
                };
                let matches_filter = process_filter.is_empty()
                    || normalized_name
                        .as_ref()
                        .is_some_and(|name| name.contains(&process_filter))
                    || process.identity.pid.to_string().contains(&process_filter);
                if matches_filter {
                    processes.push((process, normalized_name));
                }
            }
            processes.sort_by(|(left, left_name), (right, right_name)| {
                compare_processes(
                    left,
                    right,
                    left_name.as_deref(),
                    right_name.as_deref(),
                    sort_by,
                    sort_descending,
                )
            });

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
                .map(|(process, _normalized_name)| WireProcessMetrics {
                    pid: process.identity.pid,
                    parent_pid: process.parent_pid,
                    name: process.name.clone(),
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
                    history_interval_ms,
                    volumes: snapshot
                        .volumes
                        .iter()
                        .map(|volume| WireVolumeMetrics {
                            mount_point: volume.mount_point.clone(),
                            file_system: volume.file_system.clone(),
                            total_bytes: volume.total_bytes,
                            available_bytes: volume.available_bytes,
                            read_bytes_per_second: volume.read_bytes_per_second,
                            write_bytes_per_second: volume.write_bytes_per_second,
                        })
                        .collect(),
                    network_interfaces: snapshot
                        .network_interfaces
                        .iter()
                        .map(|interface| WireNetworkInterfaceMetrics {
                            name: interface.name.clone(),
                            received_bytes_per_second: interface.received_bytes_per_second,
                            transmitted_bytes_per_second: interface.transmitted_bytes_per_second,
                        })
                        .collect(),
                    block_devices: snapshot
                        .block_devices
                        .iter()
                        .map(|device| WireBlockDeviceMetrics {
                            id: device.id.clone(),
                            name: device.name.clone(),
                            read_bytes_per_second: device.read_bytes_per_second,
                            write_bytes_per_second: device.write_bytes_per_second,
                            read_operations_per_second: device.read_operations_per_second,
                            write_operations_per_second: device.write_operations_per_second,
                            busy_percent: device.busy_percent,
                            read_latency_ms: device.read_latency_ms,
                            write_latency_ms: device.write_latency_ms,
                        })
                        .collect(),
                    block_device_io_status: snapshot.block_device_io_status.as_str().to_owned(),
                    gpus: snapshot
                        .gpus
                        .iter()
                        .map(|gpu| crate::wire::GpuMetrics {
                            id: gpu.id.clone(),
                            name: gpu.name.clone(),
                            utilization_percent: gpu.utilization_percent,
                            memory_used_bytes: gpu.memory_used_bytes,
                            memory_total_bytes: gpu.memory_total_bytes,
                            temperature_celsius: gpu.temperature_celsius,
                            graphics_clock_mhz: gpu.graphics_clock_mhz,
                            memory_clock_mhz: gpu.memory_clock_mhz,
                            power_watts: gpu.power_watts,
                        })
                        .collect(),
                    gpu_status: snapshot.gpu_status.as_str().to_owned(),
                }),
                ..Response::default()
            }
        }
        Operation::Unspecified => {
            error_response("invalid_operation", "operation is required".to_owned())
        }
    }
}

fn compare_processes(
    left: &native_monitor_domain::ProcessMetrics,
    right: &native_monitor_domain::ProcessMetrics,
    left_name: Option<&str>,
    right_name: Option<&str>,
    sort_by: &str,
    descending: bool,
) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let ordering = match sort_by {
        "cpu" => compare_optional_cpu(left.cpu_usage_percent, right.cpu_usage_percent, descending),
        "memory" => compare_order(left.memory_bytes.cmp(&right.memory_bytes), descending),
        "name" => {
            let left_name = left_name.unwrap_or_default();
            let right_name = right_name.unwrap_or_default();
            compare_order(left_name.cmp(right_name), descending)
        }
        "pid" => compare_order(left.identity.pid.cmp(&right.identity.pid), descending),
        "parent" => compare_optional_order(left.parent_pid, right.parent_pid, descending),
        "start" => compare_order(
            left.identity
                .start_time_unix_seconds
                .cmp(&right.identity.start_time_unix_seconds),
            descending,
        ),
        _ => Ordering::Equal,
    };

    ordering
        .then_with(|| left.identity.pid.cmp(&right.identity.pid))
        .then_with(|| {
            left.identity
                .start_time_unix_seconds
                .cmp(&right.identity.start_time_unix_seconds)
        })
}

fn compare_order(ordering: std::cmp::Ordering, descending: bool) -> std::cmp::Ordering {
    if descending {
        ordering.reverse()
    } else {
        ordering
    }
}

fn compare_optional_order<T: Ord>(
    left: Option<T>,
    right: Option<T>,
    descending: bool,
) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => compare_order(left.cmp(&right), descending),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn compare_optional_cpu(
    left: Option<f32>,
    right: Option<f32>,
    descending: bool,
) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => {
            if descending {
                right.total_cmp(&left)
            } else {
                left.total_cmp(&right)
            }
        }
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
    match create_listener_at_unchecked(path) {
        Ok(listener) => Ok(listener),
        Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
            remove_stale_socket(path)?;
            create_listener_at_unchecked(path)
        }
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn create_listener_at_unchecked(path: &std::path::Path) -> io::Result<LocalSocketListener> {
    use interprocess::local_socket::{GenericFilePath, ListenerOptions, ToFsName};

    let name = path.to_fs_name::<GenericFilePath>()?;
    ListenerOptions::new().name(name).create_tokio()
}

#[cfg(unix)]
fn remove_stale_socket(path: &std::path::Path) -> io::Result<()> {
    use std::os::unix::fs::FileTypeExt as _;

    match std::os::unix::net::UnixStream::connect(path) {
        Ok(_stream) => {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "local monitor agent is already listening",
            ));
        }
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
            ) => {}
        Err(error) => return Err(error),
    }

    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_socket() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "refusing to remove a non-socket IPC endpoint",
        ));
    }

    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
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
    use super::{MAX_PROCESS_FILTER_CHARS, MAX_PROCESSES_PER_RESPONSE, handle_request};
    use crate::{
        PROTOCOL_MAJOR,
        wire::{Operation, Request},
    };
    use native_monitor_domain::SystemSnapshot;

    #[cfg(unix)]
    fn socket_test_directory(label: &str, id: u64) -> std::path::PathBuf {
        std::path::PathBuf::from("/tmp").join(format!("nsm-{label}-{}-{id}", std::process::id()))
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn binds_local_listener_without_requiring_socket_file_chmod_support() {
        use super::create_listener_at;
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_TEST_PATH: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_TEST_PATH.fetch_add(1, Ordering::Relaxed);
        let runtime_directory = socket_test_directory("listener", id);
        crate::unix_endpoint::ensure_private_directory(&runtime_directory)
            .expect("create private test runtime directory");

        let socket_path = runtime_directory.join("agent.sock");
        let listener = create_listener_at(&socket_path).expect("bind local IPC listener");
        drop(listener);

        std::fs::remove_dir(runtime_directory).expect("remove test runtime directory");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn reclaims_a_stale_socket_after_an_agent_crash() {
        use super::create_listener_at;
        use std::os::unix::net::UnixListener;
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_TEST_PATH: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_TEST_PATH.fetch_add(1, Ordering::Relaxed);
        let runtime_directory = socket_test_directory("stale", id);
        crate::unix_endpoint::ensure_private_directory(&runtime_directory)
            .expect("create private test runtime directory");
        let socket_path = runtime_directory.join("agent.sock");

        let stale_listener = UnixListener::bind(&socket_path).expect("create stale socket");
        drop(stale_listener);
        let listener = create_listener_at(&socket_path).expect("replace stale socket");
        drop(listener);

        std::fs::remove_dir(runtime_directory).expect("remove test runtime directory");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn does_not_remove_an_active_agent_socket() {
        use super::create_listener_at;
        use std::os::unix::net::UnixListener;
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_TEST_PATH: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_TEST_PATH.fetch_add(1, Ordering::Relaxed);
        let runtime_directory = socket_test_directory("active", id);
        crate::unix_endpoint::ensure_private_directory(&runtime_directory)
            .expect("create private test runtime directory");
        let socket_path = runtime_directory.join("agent.sock");
        let active_listener = UnixListener::bind(&socket_path).expect("bind active socket");

        let error = create_listener_at(&socket_path).expect_err("preserve active socket");

        assert_eq!(error.kind(), std::io::ErrorKind::AddrInUse);
        assert!(socket_path.exists());
        drop(active_listener);
        std::fs::remove_file(socket_path).expect("remove active socket file");
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
            1_000,
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
            1_000,
            &[],
        );

        let snapshot = response.snapshot.expect("snapshot response");
        assert_eq!(snapshot.processes.len(), 1_000);
    }

    #[test]
    fn process_filter_is_applied_to_the_full_snapshot_before_the_response_limit() {
        use native_monitor_domain::{ProcessIdentity, ProcessMetrics};

        let mut source = SystemSnapshot::starting();
        for pid in 1..=1_500 {
            source.processes.push(ProcessMetrics {
                identity: ProcessIdentity {
                    pid,
                    start_time_unix_seconds: u64::from(pid),
                },
                parent_pid: None,
                name: if pid == 1 {
                    "Needle Process".to_owned()
                } else {
                    format!("process-{pid}")
                },
                cpu_usage_percent: Some(pid as f32),
                memory_bytes: 0,
            });
        }

        let response = handle_request(
            Request {
                protocol_major: PROTOCOL_MAJOR,
                operation: Operation::Processes as i32,
                process_limit: 1,
                sort_by: "cpu".to_owned(),
                process_filter: "NEEDLE".to_owned(),
                ..Request::default()
            },
            &source,
            MAX_PROCESSES_PER_RESPONSE,
            1_000,
            &[],
        );

        let processes = response.snapshot.expect("snapshot response").processes;
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].pid, 1);
    }

    #[test]
    fn process_filter_matches_pid_text() {
        use native_monitor_domain::{ProcessIdentity, ProcessMetrics};

        let mut source = SystemSnapshot::starting();
        source.processes = vec![
            ProcessMetrics {
                identity: ProcessIdentity {
                    pid: 12,
                    start_time_unix_seconds: 1,
                },
                parent_pid: None,
                name: "worker".to_owned(),
                cpu_usage_percent: Some(0.0),
                memory_bytes: 0,
            },
            ProcessMetrics {
                identity: ProcessIdentity {
                    pid: 21,
                    start_time_unix_seconds: 1,
                },
                parent_pid: None,
                name: "worker".to_owned(),
                cpu_usage_percent: Some(0.0),
                memory_bytes: 0,
            },
        ];

        let response = handle_request(
            Request {
                protocol_major: PROTOCOL_MAJOR,
                operation: Operation::Processes as i32,
                process_limit: 10,
                sort_by: "cpu".to_owned(),
                process_filter: "21".to_owned(),
                ..Request::default()
            },
            &source,
            MAX_PROCESSES_PER_RESPONSE,
            1_000,
            &[],
        );

        let processes = response.snapshot.expect("snapshot response").processes;
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].pid, 21);
    }

    #[test]
    fn rejects_process_filters_above_the_character_limit() {
        let response = handle_request(
            Request {
                protocol_major: PROTOCOL_MAJOR,
                operation: Operation::Processes as i32,
                process_filter: "x".repeat(MAX_PROCESS_FILTER_CHARS + 1),
                ..Request::default()
            },
            &SystemSnapshot::starting(),
            MAX_PROCESSES_PER_RESPONSE,
            1_000,
            &[],
        );

        assert_eq!(response.error_code, "invalid_filter");
    }

    #[test]
    #[ignore = "manual performance acceptance check; run in release mode"]
    fn filter_sort_10k_processes_p95() {
        use native_monitor_domain::{ProcessIdentity, ProcessMetrics};
        use prost::Message as _;
        use std::time::{Duration, Instant};

        const PROCESS_COUNT: u32 = 10_000;
        const WARMUP_SAMPLES: usize = 10;
        const MEASURED_SAMPLES: usize = 100;
        const ACCEPTANCE_LIMIT: Duration = Duration::from_millis(150);

        let mut source = SystemSnapshot::starting();
        source.processes.reserve(PROCESS_COUNT as usize);
        for pid in 1..=PROCESS_COUNT {
            source.processes.push(ProcessMetrics {
                identity: ProcessIdentity {
                    pid,
                    start_time_unix_seconds: u64::from(pid),
                },
                parent_pid: Some(pid / 2),
                name: format!("process-{pid}"),
                cpu_usage_percent: Some((pid % 10_000) as f32 / 100.0),
                memory_bytes: u64::from(pid) * 4_096,
            });
        }
        let request = Request {
            protocol_major: PROTOCOL_MAJOR,
            operation: Operation::Processes as i32,
            process_limit: MAX_PROCESSES_PER_RESPONSE as u32,
            sort_by: "cpu".to_owned(),
            process_sort_key: "memory".to_owned(),
            sort_descending: Some(true),
            process_filter: "process-9".to_owned(),
            ..Request::default()
        };

        for _ in 0..WARMUP_SAMPLES {
            let response = handle_request(
                request.clone(),
                &source,
                MAX_PROCESSES_PER_RESPONSE,
                1_000,
                &[],
            );
            assert_eq!(
                response
                    .snapshot
                    .as_ref()
                    .expect("snapshot response")
                    .processes
                    .len(),
                MAX_PROCESSES_PER_RESPONSE
            );
            let _payload = response.encode_to_vec();
        }

        let mut samples = Vec::with_capacity(MEASURED_SAMPLES);
        for _ in 0..MEASURED_SAMPLES {
            let started = Instant::now();
            let response = handle_request(
                request.clone(),
                &source,
                MAX_PROCESSES_PER_RESPONSE,
                1_000,
                &[],
            );
            assert_eq!(
                response
                    .snapshot
                    .as_ref()
                    .expect("snapshot response")
                    .processes
                    .len(),
                MAX_PROCESSES_PER_RESPONSE
            );
            let _payload = response.encode_to_vec();
            samples.push(started.elapsed());
        }
        samples.sort_unstable();

        let p95_index = (MEASURED_SAMPLES * 95).div_ceil(100) - 1;
        let p95 = samples[p95_index];
        println!(
            "server query + protobuf encode: processes={PROCESS_COUNT}, filter=process-9, sort=memory desc, samples={MEASURED_SAMPLES}, p95={p95:?}, max={:?}",
            samples[MEASURED_SAMPLES - 1]
        );
        assert!(
            p95 <= ACCEPTANCE_LIMIT,
            "server query p95 {p95:?} exceeded {ACCEPTANCE_LIMIT:?}"
        );
    }

    #[test]
    fn process_sort_is_applied_to_the_full_snapshot_before_the_response_limit() {
        use native_monitor_domain::{ProcessIdentity, ProcessMetrics};

        let mut source = SystemSnapshot::starting();
        source.processes = vec![
            ProcessMetrics {
                identity: ProcessIdentity {
                    pid: 1,
                    start_time_unix_seconds: 1,
                },
                parent_pid: None,
                name: "Zulu".to_owned(),
                cpu_usage_percent: Some(1.0),
                memory_bytes: 100,
            },
            ProcessMetrics {
                identity: ProcessIdentity {
                    pid: 2,
                    start_time_unix_seconds: 2,
                },
                parent_pid: None,
                name: "alpha".to_owned(),
                cpu_usage_percent: Some(1.0),
                memory_bytes: 100,
            },
        ];

        let response = handle_request(
            Request {
                protocol_major: PROTOCOL_MAJOR,
                operation: Operation::Processes as i32,
                process_limit: 1,
                sort_by: "cpu".to_owned(),
                sort_descending: Some(false),
                process_sort_key: "name".to_owned(),
                ..Request::default()
            },
            &source,
            MAX_PROCESSES_PER_RESPONSE,
            1_000,
            &[],
        );

        let processes = response.snapshot.expect("snapshot response").processes;
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].pid, 2);
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
                1_000,
                &[],
            );

            let cpu = response
                .snapshot
                .as_ref()
                .expect("snapshot response")
                .cpu
                .as_ref()
                .expect("CPU metrics");
            assert_eq!(cpu.per_core_usage_percent, per_core_usage_percent);
            assert_eq!(
                response
                    .snapshot
                    .expect("snapshot response")
                    .history_interval_ms,
                1_000
            );
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
            1_000,
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
