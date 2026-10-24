use anyhow::{Context, Result, anyhow, bail};
use native_monitor_agent_lifecycle::{self as agent_lifecycle, StartupMode};
use native_monitor_ipc::client;
use native_monitor_ipc::wire::{Operation, ProcessMetrics};
use slint::{ModelRc, SharedString, VecModel};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};
use tracing_subscriber::EnvFilter;

slint::include_modules!();

const MAX_PROCESS_ROWS: usize = 1_000;
const MAX_CHART_POINTS: usize = 72;
const ONE_MINUTE_MS: u64 = 60_000;
const FIVE_MINUTES_MS: u64 = 300_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SortKey {
    Name,
    Pid,
    ParentPid,
    Cpu,
    Memory,
}

impl SortKey {
    fn from_label(label: &str) -> Option<Self> {
        match label {
            "name" => Some(Self::Name),
            "pid" => Some(Self::Pid),
            "parent" => Some(Self::ParentPid),
            "cpu" => Some(Self::Cpu),
            "memory" => Some(Self::Memory),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Pid => "pid",
            Self::ParentPid => "parent",
            Self::Cpu => "cpu",
            Self::Memory => "memory",
        }
    }

    fn default_descending(self) -> bool {
        matches!(self, Self::Cpu | Self::Memory)
    }
}

#[derive(Clone, Copy, Debug)]
struct ProcessSort {
    key: SortKey,
    descending: bool,
}

impl Default for ProcessSort {
    fn default() -> Self {
        Self {
            key: SortKey::Cpu,
            descending: true,
        }
    }
}

#[derive(Debug, Default)]
struct UiPreferences {
    filter: String,
    sort: ProcessSort,
}

fn main() -> Result<()> {
    initialize_logging();

    let window = MainWindow::new().context("create the desktop window")?;
    let preferences = Arc::new(Mutex::new(UiPreferences::default()));
    let filter_preferences = Arc::clone(&preferences);
    window.on_filter_edited(move |text| match filter_preferences.lock() {
        Ok(mut preferences) => preferences.filter = text.to_lowercase(),
        Err(error) => warn!(%error, "process filter state is unavailable"),
    });

    let sort_preferences = Arc::clone(&preferences);
    let weak_window_for_sort = window.as_weak();
    window.on_sort_requested(move |label| {
        let sort = match (sort_preferences.lock(), SortKey::from_label(label.as_str())) {
            (Ok(mut preferences), Some(key)) => {
                if preferences.sort.key == key {
                    preferences.sort.descending = !preferences.sort.descending;
                } else {
                    preferences.sort = ProcessSort {
                        key,
                        descending: key.default_descending(),
                    };
                }
                Some(preferences.sort)
            }
            (Err(error), _) => {
                warn!(%error, "process sort state is unavailable");
                None
            }
            (_, None) => {
                warn!(sort_key = %label, "ignored unknown process sort key");
                None
            }
        };

        if let Some(sort) = sort {
            let weak_window = weak_window_for_sort.clone();
            if let Err(error) = slint::invoke_from_event_loop(move || {
                if let Some(window) = weak_window.upgrade() {
                    window.set_sort_key(SharedString::from(sort.key.label()));
                    window.set_sort_descending(sort.descending);
                }
            }) {
                debug!(%error, "could not update process sort indicator");
            }
        }
    });

    let retry_requested = Arc::new(AtomicBool::new(false));
    let retry_for_callback = Arc::clone(&retry_requested);
    window.on_retry_agent(move || {
        retry_for_callback.store(true, Ordering::Release);
    });

    let agent_connected = Arc::new(AtomicBool::new(false));
    let connected_for_worker = Arc::clone(&agent_connected);
    let retry_for_worker = Arc::clone(&retry_requested);
    let history_window_ms = Arc::new(AtomicU64::new(FIVE_MINUTES_MS));
    let history_window_for_callback = Arc::clone(&history_window_ms);
    window.on_history_range_selected(move |range| {
        let window_ms = match range.as_str() {
            "one-minute" => ONE_MINUTE_MS,
            "five-minutes" => FIVE_MINUTES_MS,
            _ => {
                warn!(history_range = %range, "ignored unknown chart history range");
                return;
            }
        };
        history_window_for_callback.store(window_ms, Ordering::Release);
    });

    let stop_worker = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop_worker);
    let worker_preferences = Arc::clone(&preferences);
    let worker_history_window = Arc::clone(&history_window_ms);
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

            let agent_path = discover_agent_executable();
            if let Err(error) = &agent_path {
                warn!(%error, "could not find the agent executable beside the desktop app");
            }

            let mut startup_mode = None;
            let mut startup_error = None;
            let mut startup_configured = false;

            while !worker_stop.load(Ordering::Acquire) {
                let response = runtime.block_on(client::request(Operation::Snapshot, 1_000, "cpu"));
                let connected = response.is_ok();
                let retry = retry_for_worker.swap(false, Ordering::AcqRel);

                if !startup_configured || retry {
                    let lifecycle_result = match (&agent_path, connected) {
                        (Ok(agent_path), true) => {
                            agent_lifecycle::register_for_user_login(agent_path)
                        }
                        (Ok(agent_path), false) => agent_lifecycle::ensure_started(agent_path),
                        (Err(error), _) => Err(anyhow!("{error:#}")),
                    };

                    match lifecycle_result {
                        Ok(mode) => {
                            startup_mode = Some(mode);
                            startup_error = None;
                        }
                        Err(error) => {
                            startup_mode = None;
                            startup_error = Some(format!("{error:#}"));
                            warn!(%error, "could not configure the per-user agent lifecycle");
                        }
                    }
                    startup_configured = true;
                }

                connected_for_worker.store(connected, Ordering::Release);

                match response {
                    Ok(response) => {
                        let status = response.status;
                        if let Some(snapshot) = response.snapshot {
                            let preferences = match worker_preferences.lock() {
                                Ok(preferences) => (preferences.filter.clone(), preferences.sort),
                                Err(error) => {
                                    warn!(%error, "could not read process table preferences");
                                    (String::new(), ProcessSort::default())
                                }
                            };

                            let mut processes = snapshot
                                .processes
                                .into_iter()
                                .filter(|process| {
                                    preferences.0.is_empty()
                                        || process.name.to_lowercase().contains(&preferences.0)
                                        || process.pid.to_string().contains(&preferences.0)
                                })
                                .collect::<Vec<_>>();
                            sort_processes(&mut processes, preferences.1);
                            processes.truncate(MAX_PROCESS_ROWS);

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
                            let selected_window = worker_history_window.load(Ordering::Acquire);
                            let (cpu_history, cpu_start_label) = select_history(
                                &snapshot.cpu_history_percent,
                                snapshot.history_interval_ms,
                                selected_window,
                                MAX_CHART_POINTS,
                            );
                            let (memory_history, memory_start_label) = select_history(
                                &snapshot.memory_history_percent,
                                snapshot.history_interval_ms,
                                selected_window,
                                MAX_CHART_POINTS,
                            );
                            let cpu_cores = snapshot
                                .cpu
                                .as_ref()
                                .map(|cpu| {
                                    cpu.per_core_usage_percent
                                        .iter()
                                        .enumerate()
                                        .map(|(index, usage)| CpuCoreRow {
                                            name: SharedString::from(format!(
                                                "Núcleo {}",
                                                index + 1
                                            )),
                                            usage: SharedString::from(format!("{usage:.1}%")),
                                            fraction: usage.clamp(0.0, 100.0) / 100.0,
                                        })
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default();
                            let captured_at = snapshot.captured_at_unix_ms;
                            let rows = processes.into_iter().map(process_row).collect::<Vec<_>>();
                            let agent_status = lifecycle_status(
                                status.as_str(),
                                startup_mode,
                                startup_error.as_deref(),
                            );
                            let startup_configured =
                                startup_mode.is_some() && startup_error.is_none();
                            let sample_age = sample_age_label(captured_at, unix_time_ms());
                            let weak_for_update = weak_window.clone();

                            if let Err(error) = slint::invoke_from_event_loop(move || {
                                let Some(window) = weak_for_update.upgrade() else {
                                    return;
                                };

                                window.set_agent_connected(true);
                                window.set_agent_startup_configured(startup_configured);
                                window.set_agent_status(SharedString::from(agent_status));
                                window.set_sample_age(SharedString::from(sample_age));
                                window.set_cpu_value(SharedString::from(cpu_value));
                                window.set_cpu_status(SharedString::from(cpu_status));
                                window.set_memory_value(SharedString::from(memory_value));
                                window.set_memory_status(SharedString::from(memory_status));
                                window
                                    .set_process_count(SharedString::from(rows.len().to_string()));
                                window.set_processes(ModelRc::new(VecModel::from(rows)));
                                window.set_cpu_cores(ModelRc::new(VecModel::from(cpu_cores)));
                                window.set_cpu_history(ModelRc::new(VecModel::from(cpu_history)));
                                window.set_memory_history(ModelRc::new(VecModel::from(
                                    memory_history,
                                )));
                                window.set_cpu_history_start_label(SharedString::from(
                                    cpu_start_label,
                                ));
                                window.set_memory_history_start_label(SharedString::from(
                                    memory_start_label,
                                ));
                            }) {
                                debug!(%error, "could not update desktop models");
                            }
                        }
                    }
                    Err(error) => {
                        let ipc_error = format!("{error:#}");
                        let agent_status =
                            unavailable_status(&ipc_error, startup_mode, startup_error.as_deref());
                        let startup_configured = startup_mode.is_some() && startup_error.is_none();
                        let weak_for_status = weak_window.clone();
                        if let Err(invoke_error) = slint::invoke_from_event_loop(move || {
                            if let Some(window) = weak_for_status.upgrade() {
                                window.set_agent_connected(false);
                                window.set_agent_startup_configured(startup_configured);
                                window.set_agent_status(SharedString::from(agent_status));
                                window.set_sample_age(SharedString::from("Sem amostra recente"));
                                window.set_cpu_value(SharedString::from("—"));
                                window
                                    .set_cpu_status(SharedString::from("Sem conexão com o agente"));
                                window.set_memory_value(SharedString::from("—"));
                                window.set_memory_status(SharedString::from(
                                    "Sem conexão com o agente",
                                ));
                                window.set_process_count(SharedString::from("—"));
                                window.set_processes(ModelRc::new(VecModel::from(Vec::new())));
                                window.set_cpu_cores(ModelRc::new(VecModel::from(Vec::new())));
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
        .map_err(|_| anyhow!("desktop IPC worker panicked"))?;
    run_result.context("run the desktop event loop")?;

    Ok(())
}

fn discover_agent_executable() -> Result<PathBuf> {
    let desktop_path = std::env::current_exe().context("read the desktop executable path")?;
    let desktop_directory = desktop_path
        .parent()
        .context("desktop executable has no parent directory")?;
    let agent_name = if cfg!(windows) {
        "native-monitor-agent.exe"
    } else {
        "native-monitor-agent"
    };
    let agent_path = desktop_directory.join(agent_name);
    if !agent_path.is_file() {
        bail!(
            "agent executable was not found beside the desktop app at {}; build the workspace or install both binaries together",
            agent_path.display()
        );
    }

    agent_path
        .canonicalize()
        .with_context(|| format!("resolve agent executable path {}", agent_path.display()))
}

fn process_row(process: ProcessMetrics) -> ProcessRow {
    ProcessRow {
        name: SharedString::from(process.name),
        pid: SharedString::from(process.pid.to_string()),
        parent_pid: SharedString::from(
            process
                .parent_pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "—".to_owned()),
        ),
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
    }
}

fn sort_processes(processes: &mut [ProcessMetrics], sort: ProcessSort) {
    processes.sort_by(|left, right| {
        let ordering = match sort.key {
            SortKey::Name => compare_order(
                left.name.to_lowercase().cmp(&right.name.to_lowercase()),
                sort.descending,
            ),
            SortKey::Pid => compare_order(left.pid.cmp(&right.pid), sort.descending),
            SortKey::ParentPid => {
                compare_optional_order(left.parent_pid, right.parent_pid, sort.descending)
            }
            SortKey::Cpu => compare_optional_cpu(
                left.cpu_usage_percent,
                right.cpu_usage_percent,
                sort.descending,
            ),
            SortKey::Memory => {
                compare_order(left.memory_bytes.cmp(&right.memory_bytes), sort.descending)
            }
        };
        ordering.then_with(|| left.pid.cmp(&right.pid))
    });
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

fn select_history(
    values: &[f32],
    interval_ms: u64,
    window_ms: u64,
    maximum_points: usize,
) -> (Vec<f32>, String) {
    if values.is_empty() {
        return (Vec::new(), "sem dados".to_owned());
    }
    if interval_ms == 0 || window_ms == 0 {
        return (
            downsample_max(values, maximum_points),
            "histórico".to_owned(),
        );
    }

    let sample_count_u64 = window_ms.div_ceil(interval_ms).max(1);
    let sample_count = usize::try_from(sample_count_u64).unwrap_or(usize::MAX);
    let first_sample = values.len().saturating_sub(sample_count);
    let selected = &values[first_sample..];
    let range_label = if values.len() < sample_count {
        "início".to_owned()
    } else {
        format!("−{} min", window_ms.div_ceil(ONE_MINUTE_MS))
    };

    (downsample_max(selected, maximum_points), range_label)
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

fn sample_age_label(captured_at_unix_ms: u64, now_unix_ms: u64) -> String {
    if captured_at_unix_ms == 0 {
        return "Aguardando primeira amostra".to_owned();
    }

    let age_ms = now_unix_ms.saturating_sub(captured_at_unix_ms);
    if age_ms < 1_000 {
        "Amostra há menos de 1 s".to_owned()
    } else if age_ms < 60_000 {
        format!("Amostra há {} s", age_ms / 1_000)
    } else {
        format!("Amostra há {} min", age_ms / ONE_MINUTE_MS)
    }
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

fn lifecycle_status(
    collection_status: &str,
    startup_mode: Option<StartupMode>,
    startup_error: Option<&str>,
) -> String {
    match (startup_mode, startup_error) {
        (Some(mode), None) => format!(
            "Agente ativo · coleta: {collection_status} · início no login: {}",
            mode.label()
        ),
        (_, Some(error)) => {
            format!("Agente conectado · início automático não configurado: {error}")
        }
        (None, None) => format!("Agente conectado · coleta: {collection_status}"),
    }
}

fn unavailable_status(
    ipc_error: &str,
    startup_mode: Option<StartupMode>,
    startup_error: Option<&str>,
) -> String {
    match (startup_mode, startup_error) {
        (_, Some(error)) => format!("Agente indisponível · {error} · IPC: {ipc_error}"),
        (Some(mode), None) => format!(
            "Agente indisponível · IPC: {ipc_error} · início no login: {}",
            mode.label()
        ),
        (None, None) => format!("Agente indisponível · IPC: {ipc_error}"),
    }
}

fn initialize_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

#[cfg(test)]
mod tests {
    use super::{
        FIVE_MINUTES_MS, ONE_MINUTE_MS, ProcessSort, SortKey, compare_optional_cpu, downsample_max,
        lifecycle_status, memory_summary, sample_age_label, select_history, sort_processes,
        unavailable_status,
    };
    use native_monitor_agent_lifecycle::StartupMode;
    use native_monitor_ipc::wire::ProcessMetrics;

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
    fn history_selection_uses_the_requested_time_window() {
        let values = (0..360).map(|value| value as f32).collect::<Vec<_>>();

        let (one_minute, one_minute_label) = select_history(&values, 1_000, ONE_MINUTE_MS, 100);
        let (five_minutes, five_minutes_label) =
            select_history(&values, 1_000, FIVE_MINUTES_MS, 100);

        assert_eq!(one_minute.len(), 60);
        assert_eq!(one_minute[0], 300.0);
        assert_eq!(one_minute_label, "−1 min");
        assert_eq!(five_minutes.len(), 100);
        assert_eq!(five_minutes_label, "−5 min");
    }

    #[test]
    fn short_history_shows_its_start_instead_of_claiming_a_full_window() {
        let (history, label) = select_history(&[12.0, 20.0], 1_000, FIVE_MINUTES_MS, 72);

        assert_eq!(history, vec![12.0, 20.0]);
        assert_eq!(label, "início");
    }

    #[test]
    fn memory_summary_does_not_invent_missing_values() {
        assert_eq!(memory_summary(None, Some(8_000)), "—");
        assert_eq!(
            memory_summary(Some(4_000), Some(8_000)),
            "0.0 / 0.0 GiB · 50.0%"
        );
    }

    #[test]
    fn sample_age_uses_the_snapshot_time_and_handles_uninitialized_data() {
        assert_eq!(sample_age_label(0, 10_000), "Aguardando primeira amostra");
        assert_eq!(sample_age_label(9_500, 10_000), "Amostra há menos de 1 s");
        assert_eq!(sample_age_label(5_000, 10_000), "Amostra há 5 s");
        assert_eq!(sample_age_label(11_000, 10_000), "Amostra há menos de 1 s");
    }

    #[test]
    fn process_sort_keeps_missing_cpu_values_after_known_values() {
        let mut processes = vec![
            process(1, "low", Some(10.0), 10, None),
            process(2, "unknown", None, 100, None),
            process(3, "high", Some(80.0), 1, None),
        ];

        sort_processes(&mut processes, ProcessSort::default());

        assert_eq!(
            processes
                .iter()
                .map(|process| process.pid)
                .collect::<Vec<_>>(),
            [3, 1, 2]
        );
    }

    #[test]
    fn process_sort_can_order_names_ascending() {
        let mut processes = vec![
            process(1, "Zulu", Some(10.0), 10, None),
            process(2, "alpha", Some(10.0), 10, None),
        ];

        sort_processes(
            &mut processes,
            ProcessSort {
                key: SortKey::Name,
                descending: false,
            },
        );

        assert_eq!(processes[0].name, "alpha");
        assert_eq!(processes[1].name, "Zulu");
    }

    #[test]
    fn optional_cpu_sort_places_missing_data_last_for_both_directions() {
        use std::cmp::Ordering;

        assert_eq!(compare_optional_cpu(Some(5.0), None, true), Ordering::Less);
        assert_eq!(compare_optional_cpu(Some(5.0), None, false), Ordering::Less);
    }

    #[test]
    fn lifecycle_status_reports_the_session_manager_and_registration_failure() {
        assert!(
            lifecycle_status("ok", Some(StartupMode::SystemdUser), None).contains("systemd --user")
        );
        assert!(
            lifecycle_status("ok", None, Some("permission denied")).contains("permission denied")
        );
    }

    #[test]
    fn unavailable_status_never_reports_a_disconnected_agent_as_active() {
        let status = unavailable_status("socket not found", Some(StartupMode::SystemdUser), None);

        assert!(status.contains("Agente indisponível"));
        assert!(status.contains("systemd --user"));
        assert!(!status.contains("Agente ativo"));
    }

    fn process(
        pid: u32,
        name: &str,
        cpu_usage_percent: Option<f32>,
        memory_bytes: u64,
        parent_pid: Option<u32>,
    ) -> ProcessMetrics {
        ProcessMetrics {
            pid,
            parent_pid,
            name: name.to_owned(),
            cpu_usage_percent,
            memory_bytes,
            start_time_unix_seconds: 1,
        }
    }
}
