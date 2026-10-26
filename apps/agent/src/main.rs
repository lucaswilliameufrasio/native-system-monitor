use anyhow::{Context, Result, bail};
use native_monitor_collector_api::SystemCollector;
use native_monitor_collectors_common::SysinfoCollector;
use native_monitor_domain::{MetricHistory, SystemSnapshot};
use native_monitor_exporters::{bind_loopback, serve as serve_metrics};
use native_monitor_ipc::server;
use native_monitor_settings::AgentSettings;
use std::env;
use std::fs;
use std::future::pending;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    initialize_logging();

    let config_path = parse_command_line()?;
    let settings = read_settings(config_path.as_deref())?;
    let (snapshot_sender, snapshot_receiver) = watch::channel(Arc::new(SystemSnapshot::starting()));
    let metrics_listener = if settings.export.prometheus.enabled {
        Some(
            bind_loopback(settings.export.prometheus.listen)
                .await
                .with_context(|| {
                    format!(
                        "bind Prometheus listener at {}",
                        settings.export.prometheus.listen
                    )
                })?,
        )
    } else {
        None
    };
    let (shutdown_sender, shutdown_receiver) = watch::channel(false);
    let stop_worker = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop_worker);
    let collection_interval = Duration::from_millis(settings.collection_interval_ms);
    let history_capacity = settings.history_capacity;
    let mut collector = SysinfoCollector::new();
    let capabilities = Arc::new(collector.capabilities());
    let metrics_snapshot_receiver = snapshot_receiver.clone();
    let metrics_shutdown_receiver = shutdown_receiver.clone();
    let metrics_families = settings.export.prometheus.families.clone();

    let worker = thread::Builder::new()
        .name("native-monitor-collector".to_owned())
        .spawn(move || {
            let mut cpu_history = MetricHistory::new(history_capacity);
            let mut memory_history = MetricHistory::new(history_capacity);

            while !worker_stop.load(Ordering::Acquire) {
                let sample_started = Instant::now();
                let mut snapshot = match collector.collect() {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        error!(%error, "system collection failed");
                        SystemSnapshot::unavailable(error.to_string())
                    }
                };

                if let Some(cpu_usage) = snapshot.cpu.total_usage_percent {
                    cpu_history.push(cpu_usage.clamp(0.0, 100.0));
                }

                if let (Some(used), Some(total)) = (
                    snapshot.memory.used_bytes,
                    snapshot.memory.total_bytes.filter(|total| *total > 0),
                ) {
                    let memory_percent = (used as f64 / total as f64 * 100.0) as f32;
                    memory_history.push(memory_percent.clamp(0.0, 100.0));
                }

                snapshot.add_history(&cpu_history, &memory_history);
                let _previous_snapshot = snapshot_sender.send_replace(Arc::new(snapshot));
                let wait = collection_interval.saturating_sub(sample_started.elapsed());
                thread::park_timeout(wait);
            }
        })
        .context("start the system collection worker")?;

    info!(
        interval_ms = settings.collection_interval_ms,
        "native system monitor agent started"
    );

    let server_result = tokio::select! {
        result = server::serve(
            snapshot_receiver,
            shutdown_receiver,
            settings.max_processes_per_response,
            settings.collection_interval_ms,
            capabilities,
        ) => result,
        result = async {
            if let Some(listener) = metrics_listener {
                serve_metrics(
                    listener,
                    metrics_snapshot_receiver,
                    metrics_shutdown_receiver,
                    metrics_families,
                )
                .await
                .map_err(anyhow::Error::from)
            } else {
                pending().await
            }
        } => result,
        // The IPC server owns a separate process lifetime; this signal only stops this agent.
        signal = tokio::signal::ctrl_c() => {
            signal.context("wait for shutdown signal")?;
            info!("shutdown signal received");
            Ok(())
        }
    };

    let _previous_shutdown_state = shutdown_sender.send_replace(true);
    stop_worker.store(true, Ordering::Release);
    worker.thread().unpark();
    worker
        .join()
        .map_err(|_| anyhow::anyhow!("system collection worker panicked"))?;

    server_result.context("agent server stopped with an error")?;
    info!("native system monitor agent stopped");
    Ok(())
}

fn initialize_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn parse_command_line() -> Result<Option<String>> {
    let mut arguments = env::args().skip(1);
    match arguments.next().as_deref() {
        Some("run") => {}
        Some("--help" | "-h") | None => {
            println!("Usage: monitor-agent run [settings.toml]");
            std::process::exit(0);
        }
        Some(command) => bail!("unknown command {command}; use `monitor-agent run`"),
    }

    let config_path = arguments.next();
    if arguments.next().is_some() {
        bail!("expected at most one optional settings TOML path");
    }

    Ok(config_path)
}

fn read_settings(config_path: Option<&str>) -> Result<AgentSettings> {
    let Some(path) = config_path else {
        return Ok(AgentSettings::default());
    };

    let content = fs::read_to_string(path).with_context(|| format!("read settings file {path}"))?;
    let settings = AgentSettings::parse_toml(&content)
        .with_context(|| format!("validate settings file {path}"))?;

    Ok(settings)
}
