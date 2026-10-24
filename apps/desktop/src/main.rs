use anyhow::{Context, Result};
use native_monitor_ipc::client;
use native_monitor_ipc::wire::Operation;
use slint::{ModelRc, SharedString, VecModel};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tracing::{debug, warn};
use tracing_subscriber::EnvFilter;

slint::include_modules!();

fn main() -> Result<()> {
    initialize_logging();

    let window = MainWindow::new().context("create the desktop window")?;
    let filter_text = Arc::new(Mutex::new(String::new()));
    let filter_for_callback = Arc::clone(&filter_text);
    window.on_filter_edited(move |text| match filter_for_callback.lock() {
        Ok(mut filter) => *filter = text.to_string(),
        Err(error) => warn!(%error, "process filter state is unavailable"),
    });

    let stop_worker = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop_worker);
    let weak_window = window.as_weak();
    let worker = thread::Builder::new()
        .name("native-monitor-ui-ipc".to_owned())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    warn!(%error, "could not start UI IPC runtime");
                    return;
                }
            };

            while !worker_stop.load(Ordering::Acquire) {
                let response = runtime.block_on(client::request(Operation::Snapshot, 1_000, "cpu"));
                match response {
                    Ok(response) => {
                        let status = response.status;
                        if let Some(snapshot) = response.snapshot {
                            let filter = match filter_text.lock() {
                                Ok(filter) => filter.to_lowercase(),
                                Err(error) => {
                                    warn!(%error, "could not read process filter");
                                    String::new()
                                }
                            };
                            let processes = snapshot
                                .processes
                                .into_iter()
                                .filter(|process| {
                                    filter.is_empty()
                                        || process.name.to_lowercase().contains(&filter)
                                        || process.pid.to_string().contains(&filter)
                                })
                                .take(1_000)
                                .collect::<Vec<_>>();
                            let cpu_value = snapshot
                                .cpu
                                .as_ref()
                                .and_then(|cpu| cpu.total_usage_percent)
                                .map(|value| format!("{value:.1}%"))
                                .unwrap_or_else(|| "—".to_owned());
                            let cpu_status = snapshot
                                .cpu
                                .as_ref()
                                .map(|cpu| cpu.status.clone())
                                .unwrap_or_else(|| "indisponível".to_owned());
                            let memory_value = memory_summary(
                                snapshot
                                    .memory
                                    .as_ref()
                                    .and_then(|memory| memory.used_bytes),
                                snapshot
                                    .memory
                                    .as_ref()
                                    .and_then(|memory| memory.total_bytes),
                            );
                            let memory_status = snapshot
                                .memory
                                .as_ref()
                                .map(|memory| memory.status.clone())
                                .unwrap_or_else(|| "indisponível".to_owned());
                            let cpu_history = downsample_max(&snapshot.cpu_history_percent, 72);
                            let memory_history =
                                downsample_max(&snapshot.memory_history_percent, 72);
                            let weak_for_update = weak_window.clone();

                            if let Err(error) = slint::invoke_from_event_loop(move || {
                                let Some(window) = weak_for_update.upgrade() else {
                                    return;
                                };

                                let rows = processes
                                    .into_iter()
                                    .map(|process| ProcessRow {
                                        name: SharedString::from(process.name),
                                        pid: SharedString::from(process.pid.to_string()),
                                        cpu: SharedString::from(
                                            process
                                                .cpu_usage_percent
                                                .map(|value| format!("{value:.1}"))
                                                .unwrap_or_else(|| "—".to_owned()),
                                        ),
                                        memory: SharedString::from(format!(
                                            "{:.1}",
                                            process.memory_bytes as f64 / (1024.0 * 1024.0)
                                        )),
                                    })
                                    .collect::<Vec<_>>();

                                window.set_agent_status(SharedString::from(format!(
                                    "Agente ativo · {status}"
                                )));
                                window.set_cpu_value(SharedString::from(cpu_value));
                                window.set_cpu_status(SharedString::from(cpu_status));
                                window.set_memory_value(SharedString::from(memory_value));
                                window.set_memory_status(SharedString::from(memory_status));
                                window
                                    .set_process_count(SharedString::from(rows.len().to_string()));
                                window.set_processes(ModelRc::new(VecModel::from(rows)));
                                window.set_cpu_history(ModelRc::new(VecModel::from(cpu_history)));
                                window.set_memory_history(ModelRc::new(VecModel::from(
                                    memory_history,
                                )));
                            }) {
                                debug!(%error, "could not update desktop models");
                            }
                        }
                    }
                    Err(error) => {
                        let status = format!("Agente indisponível · {error}");
                        let weak_for_status = weak_window.clone();
                        if let Err(invoke_error) = slint::invoke_from_event_loop(move || {
                            if let Some(window) = weak_for_status.upgrade() {
                                window.set_agent_status(SharedString::from(status));
                                window.set_cpu_value(SharedString::from("—"));
                                window
                                    .set_cpu_status(SharedString::from("Sem conexão com o agente"));
                                window.set_memory_value(SharedString::from("—"));
                                window.set_memory_status(SharedString::from(
                                    "Sem conexão com o agente",
                                ));
                                window.set_process_count(SharedString::from("0"));
                                window.set_processes(ModelRc::new(VecModel::from(Vec::new())));
                                window.set_cpu_history(ModelRc::new(VecModel::from(Vec::new())));
                                window.set_memory_history(ModelRc::new(VecModel::from(Vec::new())));
                            }
                        }) {
                            debug!(%invoke_error, "could not update agent status");
                        }
                    }
                }

                thread::park_timeout(Duration::from_secs(1));
            }
        })
        .context("start the desktop IPC worker")?;

    let run_result = window.run();
    stop_worker.store(true, Ordering::Release);
    worker.thread().unpark();
    worker
        .join()
        .map_err(|_| anyhow::anyhow!("desktop IPC worker panicked"))?;
    run_result.context("run the desktop event loop")?;

    Ok(())
}

fn memory_summary(used_bytes: Option<u64>, total_bytes: Option<u64>) -> String {
    match (used_bytes, total_bytes.filter(|total| *total > 0)) {
        (Some(used), Some(total)) => {
            let used_gib = used as f64 / (1024.0 * 1024.0 * 1024.0);
            let total_gib = total as f64 / (1024.0 * 1024.0 * 1024.0);
            let percentage = used as f64 / total as f64 * 100.0;
            format!("{used_gib:.1} / {total_gib:.1} GiB · {percentage:.1}%")
        }
        _ => "—".to_owned(),
    }
}

fn downsample_max(values: &[f32], maximum_points: usize) -> Vec<f32> {
    if maximum_points == 0 || values.is_empty() {
        return Vec::new();
    }

    if values.len() <= maximum_points {
        return values.to_vec();
    }

    let bucket_size = values.len().div_ceil(maximum_points);
    let mut output = Vec::with_capacity(maximum_points);
    for bucket in values.chunks(bucket_size) {
        let mut maximum = f32::NEG_INFINITY;
        for value in bucket {
            maximum = maximum.max(*value);
        }
        output.push(maximum);
    }

    output
}

fn initialize_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

#[cfg(test)]
mod tests {
    use super::{downsample_max, memory_summary};

    #[test]
    fn downsamples_long_series_and_keeps_bucket_peaks() {
        let values = vec![1.0, 2.0, 90.0, 4.0, 5.0, 6.0, 7.0, 8.0];

        let result = downsample_max(&values, 4);

        assert_eq!(result.len(), 4);
        assert_eq!(result[1], 90.0);
    }

    #[test]
    fn empty_and_zero_width_series_are_empty() {
        assert!(downsample_max(&[1.0, 2.0], 0).is_empty());
        assert!(downsample_max(&[], 10).is_empty());
    }

    #[test]
    fn memory_summary_does_not_invent_missing_values() {
        assert_eq!(memory_summary(None, Some(8_000)), "—");
        assert_eq!(
            memory_summary(Some(4_000), Some(8_000)),
            "0.0 / 0.0 GiB · 50.0%"
        );
    }
}
