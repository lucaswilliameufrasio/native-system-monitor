use anyhow::{Context, Result, anyhow, bail};
use native_monitor_agent_lifecycle::{self as agent_lifecycle, StartupMode};
use native_monitor_ipc::client;
use native_monitor_ipc::wire::{Operation, ProcessMetrics};
use slint::{ModelRc, SharedString, VecModel};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};
use tracing_subscriber::EnvFilter;

slint::include_modules!();

const MAX_PROCESS_ROWS: usize = 1_000;
const MAX_PROCESS_FILTER_CHARS: usize = 256;
const MAX_CHART_POINTS: usize = 72;
const PROCESS_QUERY_PROTOCOL_MINOR: u32 = 2;
const VOLUME_QUERY_PROTOCOL_MINOR: u32 = 3;
const VOLUME_IO_QUERY_PROTOCOL_MINOR: u32 = 5;
const NETWORK_QUERY_PROTOCOL_MINOR: u32 = 3;
const BLOCK_DEVICE_IO_PROTOCOL_MINOR: u32 = 4;
const GPU_METRICS_PROTOCOL_MINOR: u32 = 7;
const BATTERY_METRICS_PROTOCOL_MINOR: u32 = 8;
const ONE_MINUTE_MS: u64 = 60_000;
const FIVE_MINUTES_MS: u64 = 300_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Locale {
    PtBr,
    EnUs,
}

impl Locale {
    fn from_system() -> Self {
        Self::from_language_tag(sys_locale::get_locale().as_deref())
    }

    fn from_language_tag(tag: Option<&str>) -> Self {
        let language = tag
            .and_then(|value| value.split(['-', '_']).next())
            .unwrap_or_default();
        if language.eq_ignore_ascii_case("en") {
            Self::EnUs
        } else {
            Self::PtBr
        }
    }

    fn from_selection(value: &str) -> Option<Self> {
        match value {
            "pt-BR" => Some(Self::PtBr),
            "en-US" => Some(Self::EnUs),
            _ => None,
        }
    }

    fn from_code(value: u8) -> Self {
        match value {
            1 => Self::EnUs,
            _ => Self::PtBr,
        }
    }

    fn code(self) -> u8 {
        match self {
            Self::PtBr => 0,
            Self::EnUs => 1,
        }
    }

    fn ui_language(self) -> UiLanguage {
        match self {
            Self::PtBr => UiLanguage::PtBr,
            Self::EnUs => UiLanguage::EnUs,
        }
    }

    fn unavailable(self) -> &'static str {
        if self == Self::EnUs {
            "Unavailable"
        } else {
            "Indisponível"
        }
    }
    fn unavailable_sample(self) -> &'static str {
        if self == Self::EnUs {
            "Unavailable in this sample"
        } else {
            "Indisponível nesta amostra"
        }
    }
    fn core_name(self) -> &'static str {
        if self == Self::EnUs {
            "Core {}"
        } else {
            "Núcleo {}"
        }
    }
    fn unknown_state(self) -> &'static str {
        if self == Self::EnUs {
            "unknown state"
        } else {
            "estado desconhecido"
        }
    }
    fn power_unavailable(self) -> &'static str {
        if self == Self::EnUs {
            "power unavailable"
        } else {
            "potência indisponível"
        }
    }
    fn no_recent_sample(self) -> &'static str {
        if self == Self::EnUs {
            "No recent sample"
        } else {
            "Sem amostra recente"
        }
    }
    fn agent_disconnected(self) -> &'static str {
        if self == Self::EnUs {
            "Agent disconnected"
        } else {
            "Sem conexão com o agente"
        }
    }
    fn waiting_for_agent(self) -> &'static str {
        if self == Self::EnUs {
            "Waiting for agent connection…"
        } else {
            "Aguardando conexão com o agente…"
        }
    }
    fn gpu_unavailable(self) -> &'static str {
        if self == Self::EnUs {
            "GPU unavailable"
        } else {
            "GPU indisponível"
        }
    }
    fn battery_unavailable(self) -> &'static str {
        if self == Self::EnUs {
            "Battery unavailable"
        } else {
            "Bateria indisponível"
        }
    }
    fn relative_age(self, amount: u64, english_unit: &str, portuguese_unit: &str) -> String {
        if self == Self::EnUs {
            let unit = match (english_unit, amount) {
                ("sec", 1) => "sec",
                ("sec", _) => "secs",
                ("min", _) => "min",
                ("hr", 1) => "hr",
                ("hr", _) => "hrs",
                ("day", 1) => "day",
                ("day", _) => "days",
                _ => english_unit,
            };
            format!("{amount} {unit} ago")
        } else {
            format!("há {amount} {portuguese_unit}")
        }
    }
    fn unsupported(self, what: &str) -> String {
        let what = match (self, what) {
            (Self::EnUs, "GPU metrics") => "GPU metrics",
            (Self::EnUs, "battery metrics") => "battery metrics",
            (Self::EnUs, "mounted volumes") => "mounted volumes",
            (Self::EnUs, "per-volume I/O rates") => "per-volume I/O rates",
            (Self::EnUs, "network interfaces") => "network interfaces",
            (Self::EnUs, "block device I/O") => "block device I/O",
            (_, "GPU metrics") => "métricas de GPU",
            (_, "battery metrics") => "métricas de bateria",
            (_, "mounted volumes") => "volumes montados",
            (_, "per-volume I/O rates") => "taxas de I/O por volume",
            (_, "network interfaces") => "interfaces de rede",
            (_, "block device I/O") => "I/O de dispositivos",
            _ => what,
        };
        if self == Self::EnUs {
            format!("Agent does not support {what} in this version")
        } else {
            format!("Agente sem suporte a {what} nesta versão")
        }
    }
    fn none_available(self, what: &str) -> String {
        match (self, what) {
            (Self::EnUs, "mounted volumes") => "No mounted volumes available.".to_owned(),
            (Self::EnUs, _) => "No network interfaces available.".to_owned(),
            (Self::PtBr, "mounted volumes") => "Nenhum volume montado disponível.".to_owned(),
            (Self::PtBr, _) => "Nenhuma interface de rede disponível.".to_owned(),
        }
    }
}

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
    selected_process_key: Option<String>,
}

fn main() -> Result<()> {
    initialize_logging();

    let window = MainWindow::new().context("create the desktop window")?;
    let initial_locale = Locale::from_system();
    window.set_language(initial_locale.ui_language());
    let locale_state = Arc::new(AtomicU8::new(initial_locale.code()));
    let locale_for_callback = Arc::clone(&locale_state);
    let weak_window_for_locale = window.as_weak();
    window.on_language_selected(move |value| {
        let Some(locale) = Locale::from_selection(value.as_str()) else {
            warn!(locale = %value, "ignored unsupported UI locale");
            return;
        };
        locale_for_callback.store(locale.code(), Ordering::Release);
        if let Some(window) = weak_window_for_locale.upgrade() {
            window.set_language(locale.ui_language());
        }
    });
    let preferences = Arc::new(Mutex::new(UiPreferences::default()));
    let filter_preferences = Arc::clone(&preferences);
    window.on_filter_edited(move |text| match filter_preferences.lock() {
        Ok(mut preferences) => {
            preferences.filter = text.chars().take(MAX_PROCESS_FILTER_CHARS).collect();
        }
        Err(error) => warn!(%error, "process filter state is unavailable"),
    });

    let selection_preferences = Arc::clone(&preferences);
    window.on_process_selected(move |key| match selection_preferences.lock() {
        Ok(mut preferences) => preferences.selected_process_key = Some(key.to_string()),
        Err(error) => warn!(%error, "selected process state is unavailable"),
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
    let worker_locale = Arc::clone(&locale_state);
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
            let mut cached_tree_key = None::<String>;
            let mut cached_tree_rows = Vec::new();
            let mut cached_tree_status = String::new();
            let mut last_tree_refresh = None::<Instant>;

            while !worker_stop.load(Ordering::Acquire) {
                let locale = Locale::from_code(worker_locale.load(Ordering::Acquire));
                let (filter, sort, selected_process_key) = match worker_preferences.lock() {
                    Ok(preferences) => (
                        preferences.filter.clone(),
                        preferences.sort,
                        preferences.selected_process_key.clone(),
                    ),
                    Err(error) => {
                        warn!(%error, "could not read process table preferences");
                        (String::new(), ProcessSort::default(), None)
                    }
                };
                let response = runtime.block_on(client::request_with_process_query(
                    Operation::Snapshot,
                    MAX_PROCESS_ROWS as u32,
                    sort.key.label(),
                    Some(sort.descending),
                    &filter,
                ));
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
                        let supports_server_process_query =
                            response.protocol_minor >= PROCESS_QUERY_PROTOCOL_MINOR;
                        if let Some(snapshot) = response.snapshot {
                            let total_memory_bytes = snapshot
                                .memory
                                .as_ref()
                                .and_then(|memory| memory.total_bytes);
                            let now_unix_ms = unix_time_ms();
                            let normalized_filter = filter.to_lowercase();
                            let mut processes = snapshot.processes;
                            if !supports_server_process_query && !normalized_filter.is_empty() {
                                processes.retain(|process| {
                                    process.name.to_lowercase().contains(&normalized_filter)
                                        || process.pid.to_string().contains(&normalized_filter)
                                });
                            }
                            if !supports_server_process_query {
                                sort_processes(&mut processes, sort);
                            }
                            processes.truncate(MAX_PROCESS_ROWS);

                            match selected_process_key.as_deref() {
                                Some(key) => {
                                    let selection_changed = cached_tree_key.as_deref() != Some(key);
                                    let refresh_due = last_tree_refresh
                                        .is_none_or(|last| last.elapsed() >= Duration::from_secs(5));
                                    if selection_changed {
                                        cached_tree_key = Some(key.to_owned());
                                        cached_tree_rows.clear();
                                        cached_tree_status = if locale == Locale::EnUs {
                                            "Loading process tree…".to_owned()
                                        } else {
                                            "Carregando árvore de processos…".to_owned()
                                        };
                                        last_tree_refresh = None;
                                    }
                                    if (selection_changed || refresh_due)
                                        && let Some(process) = find_selected_process(&processes, key)
                                    {
                                        let tree_response = runtime.block_on(
                                            client::request_process_tree(
                                                process.pid,
                                                process.start_time_unix_seconds,
                                            ),
                                        );
                                        last_tree_refresh = Some(Instant::now());
                                        match tree_response {
                                            Ok(tree_response) => {
                                                if let Some(tree_snapshot) = tree_response.snapshot {
                                                    cached_tree_rows =
                                                        process_tree_rows(&tree_snapshot.processes);
                                                    cached_tree_status = if tree_snapshot.process_tree_truncated {
                                                        if locale == Locale::EnUs {
                                                            "Tree truncated at the response limit.".to_owned()
                                                        } else {
                                                            "Árvore truncada no limite da resposta.".to_owned()
                                                        }
                                                    } else if cached_tree_rows.is_empty() {
                                                        if locale == Locale::EnUs {
                                                            "No process tree data available.".to_owned()
                                                        } else {
                                                            "Nenhum dado de árvore disponível.".to_owned()
                                                        }
                                                    } else {
                                                        String::new()
                                                    };
                                                }
                                            }
                                            Err(error) => {
                                                cached_tree_rows.clear();
                                                cached_tree_status = if format!("{error:#}").contains("process_not_found") {
                                                    if locale == Locale::EnUs {
                                                        "The selected process no longer exists.".to_owned()
                                                    } else {
                                                        "O processo selecionado não existe mais.".to_owned()
                                                    }
                                                } else if locale == Locale::EnUs {
                                                    "Could not load the process tree.".to_owned()
                                                } else {
                                                    "Não foi possível carregar a árvore de processos.".to_owned()
                                                };
                                            }
                                        }
                                    }
                                }
                                None => {
                                    cached_tree_key = None;
                                    cached_tree_rows.clear();
                                    cached_tree_status.clear();
                                    last_tree_refresh = None;
                                }
                            }
                            let process_tree = cached_tree_rows.clone();
                            let process_tree_status = cached_tree_status.clone();

                            let selected_process = selected_process_key
                                .as_deref()
                                .and_then(|key| find_selected_process(&processes, key))
                                .map(|process| {
                                    process_row(process, total_memory_bytes, now_unix_ms, locale)
                                });
                            if selected_process_key.is_some()
                                && selected_process.is_none()
                                && let Ok(mut preferences) = worker_preferences.lock()
                                && preferences.selected_process_key == selected_process_key
                            {
                                preferences.selected_process_key = None;
                            }

                            let cpu_value = snapshot
                                .cpu
                                .as_ref()
                                .and_then(|cpu| cpu.total_usage_percent)
                                .map(|value| format!("{value:.1}%"))
                                .unwrap_or_else(|| "—".to_owned());
                            let cpu_status = snapshot
                                .cpu
                                .as_ref()
                                .map(|cpu| metric_status_label(cpu.status.as_str(), locale))
                                .unwrap_or_else(|| locale.unavailable().to_owned());
                            let (memory_value, memory_usage) = memory_summary(
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
                                .map(|memory| metric_status_label(memory.status.as_str(), locale))
                                .unwrap_or_else(|| locale.unavailable().to_owned());
                            let memory_status = if memory_value == "—" {
                                if snapshot
                                    .memory
                                    .as_ref()
                                    .is_some_and(|memory| memory.status == "ok")
                                {
                                    localize_summary(memory_usage, locale)
                                } else {
                                    memory_status
                                }
                            } else {
                                format!("{memory_usage} · {memory_status}")
                            };
                            let (swap_value, swap_status) = snapshot
                                .memory
                                .as_ref()
                                .map(|memory| {
                                    swap_summary(memory.swap_used_bytes, memory.swap_total_bytes)
                                })
                                .unwrap_or_else(|| {
                                    ("—".to_owned(), locale.unavailable_sample().to_owned())
                                });
                            let swap_status = localize_summary(swap_status, locale);
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
                            let cpu_start_label = localize_history_label(cpu_start_label, locale);
                            let memory_start_label =
                                localize_history_label(memory_start_label, locale);
                            let cpu_cores = snapshot
                                .cpu
                                .as_ref()
                                .map(|cpu| {
                                    cpu.per_core_usage_percent
                                        .iter()
                                        .enumerate()
                                        .map(|(index, usage)| CpuCoreRow {
                                            name: SharedString::from(format!(
                                                "{} {}",
                                                locale.core_name(),
                                                index + 1
                                            )),
                                            usage: SharedString::from(format!("{usage:.1}%")),
                                            fraction: usage.clamp(0.0, 100.0) / 100.0,
                                        })
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default();
                            let gpus = snapshot
                                .gpus
                                .iter()
                                .map(|gpu| GpuRow {
                                    name: SharedString::from(gpu.name.clone()),
                                    utilization: SharedString::from(
                                        gpu.utilization_percent
                                            .map(|value| format!("{value:.0}%"))
                                            .unwrap_or_else(|| "—".to_owned()),
                                    ),
                                    memory: SharedString::from(format!(
                                        "{} / {}",
                                        format_bytes(gpu.memory_used_bytes),
                                        format_bytes(gpu.memory_total_bytes),
                                    )),
                                    temperature: SharedString::from(
                                        gpu.temperature_celsius
                                            .map(|value| format!("{value} °C"))
                                            .unwrap_or_else(|| "—".to_owned()),
                                    ),
                                    clocks: SharedString::from(format!(
                                        "{} / {} MHz",
                                        gpu.graphics_clock_mhz
                                            .map(|value| value.to_string())
                                            .unwrap_or_else(|| "—".to_owned()),
                                        gpu.memory_clock_mhz
                                            .map(|value| value.to_string())
                                            .unwrap_or_else(|| "—".to_owned()),
                                    )),
                                    power: SharedString::from(
                                        gpu.power_watts
                                            .map(|value| format!("{value:.1} W"))
                                            .unwrap_or_else(|| "—".to_owned()),
                                    ),
                                })
                                .collect::<Vec<_>>();
                            let gpu_status = if response.protocol_minor < GPU_METRICS_PROTOCOL_MINOR
                            {
                                locale.unsupported("GPU metrics").to_owned()
                            } else if gpus.is_empty() {
                                metric_status_label(snapshot.gpu_status.as_str(), locale)
                            } else {
                                String::new()
                            };
                            let battery_value = snapshot
                                .batteries
                                .first()
                                .and_then(|battery| battery.capacity_percent)
                                .map(|capacity| format!("{capacity}%"))
                                .unwrap_or_else(|| "—".to_owned());
                            let battery_status =
                                if response.protocol_minor < BATTERY_METRICS_PROTOCOL_MINOR {
                                    locale.unsupported("battery metrics").to_owned()
                                } else if let Some(battery) = snapshot.batteries.first() {
                                    let state =
                                        battery.status.as_deref().unwrap_or(locale.unknown_state());
                                    let power = battery
                                        .power_watts
                                        .map(|value| format!("{value:.1} W"))
                                        .unwrap_or_else(|| locale.power_unavailable().to_owned());
                                    format!("{} · {state} · {power}", battery.id)
                                } else {
                                    metric_status_label(snapshot.battery_status.as_str(), locale)
                                };
                            let volumes = snapshot
                                .volumes
                                .iter()
                                .map(|volume| VolumeRow {
                                    mount_point: SharedString::from(volume.mount_point.clone()),
                                    file_system: SharedString::from(volume.file_system.clone()),
                                    total: SharedString::from(format_bytes(volume.total_bytes)),
                                    available: SharedString::from(format_bytes(
                                        volume.available_bytes,
                                    )),
                                    read_rate: SharedString::from(format_bytes_per_second(
                                        volume.read_bytes_per_second,
                                    )),
                                    write_rate: SharedString::from(format_bytes_per_second(
                                        volume.write_bytes_per_second,
                                    )),
                                })
                                .collect::<Vec<_>>();
                            let volume_status =
                                if response.protocol_minor < VOLUME_QUERY_PROTOCOL_MINOR {
                                    locale.unsupported("mounted volumes").to_owned()
                                } else if response.protocol_minor < VOLUME_IO_QUERY_PROTOCOL_MINOR {
                                    locale.unsupported("per-volume I/O rates").to_owned()
                                } else if volumes.is_empty() {
                                    locale.none_available("mounted volumes").to_owned()
                                } else {
                                    String::new()
                                };
                            let network_interfaces = snapshot
                                .network_interfaces
                                .iter()
                                .map(|interface| NetworkInterfaceRow {
                                    name: SharedString::from(interface.name.clone()),
                                    received: SharedString::from(format_bytes_per_second(
                                        interface.received_bytes_per_second,
                                    )),
                                    transmitted: SharedString::from(format_bytes_per_second(
                                        interface.transmitted_bytes_per_second,
                                    )),
                                })
                                .collect::<Vec<_>>();
                            let network_status =
                                if response.protocol_minor < NETWORK_QUERY_PROTOCOL_MINOR {
                                    locale.unsupported("network interfaces").to_owned()
                                } else if network_interfaces.is_empty() {
                                    locale.none_available("network interfaces").to_owned()
                                } else {
                                    String::new()
                                };
                            let block_devices = snapshot
                                .block_devices
                                .iter()
                                .map(|device| BlockDeviceRow {
                                    name: SharedString::from(device.name.clone()),
                                    read_rate: SharedString::from(format_bytes_per_second(
                                        device.read_bytes_per_second,
                                    )),
                                    write_rate: SharedString::from(format_bytes_per_second(
                                        device.write_bytes_per_second,
                                    )),
                                    read_iops: SharedString::from(format_operations_per_second(
                                        device.read_operations_per_second,
                                    )),
                                    write_iops: SharedString::from(format_operations_per_second(
                                        device.write_operations_per_second,
                                    )),
                                    read_latency: SharedString::from(format_latency_ms(
                                        device.read_latency_ms,
                                    )),
                                    write_latency: SharedString::from(format_latency_ms(
                                        device.write_latency_ms,
                                    )),
                                    busy: SharedString::from(
                                        device
                                            .busy_percent
                                            .map(|value| format!("{value:.1}%"))
                                            .unwrap_or_else(|| "—".to_owned()),
                                    ),
                                })
                                .collect::<Vec<_>>();
                            let block_device_io_status =
                                if response.protocol_minor < BLOCK_DEVICE_IO_PROTOCOL_MINOR {
                                    locale.unsupported("block device I/O").to_owned()
                                } else if block_devices.is_empty() {
                                    metric_status_label(
                                        snapshot.block_device_io_status.as_str(),
                                        locale,
                                    )
                                } else {
                                    String::new()
                                };
                            let captured_at = snapshot.captured_at_unix_ms;
                            let selected_process_key = selected_process
                                .as_ref()
                                .map(|process| process.key.clone())
                                .unwrap_or_default();
                            let rows = processes
                                .iter()
                                .map(|process| {
                                    process_row(process, total_memory_bytes, now_unix_ms, locale)
                                })
                                .collect::<Vec<_>>();
                            let agent_status = lifecycle_status(
                                status.as_str(),
                                startup_mode,
                                startup_error.as_deref(),
                            );
                            let startup_configured =
                                startup_mode.is_some() && startup_error.is_none();
                            let sample_age = sample_age_label(captured_at, unix_time_ms());
                            let sample_age = localized_sample_age(&sample_age, locale);
                            let agent_status = localize_agent_status(agent_status, locale);
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
                                window.set_swap_value(SharedString::from(swap_value));
                                window.set_swap_status(SharedString::from(swap_status));
                                window
                                    .set_process_count(SharedString::from(rows.len().to_string()));
                                window.set_process_count_description(SharedString::from(
                                    process_count_description(rows.len(), locale),
                                ));
                                window.set_selected_process_key(selected_process_key);
                                window.set_process_tree(ModelRc::new(VecModel::from(process_tree)));
                                window.set_process_tree_status(SharedString::from(process_tree_status));
                                if let Some(process) = selected_process {
                                    window.set_has_selected_process(true);
                                    window.set_selected_process_name(process.name);
                                    window.set_selected_process_pid(process.pid);
                                    window.set_selected_process_parent_pid(process.parent_pid);
                                    window.set_selected_process_started(process.started);
                                    window.set_selected_process_cpu(process.cpu);
                                    window.set_selected_process_memory(process.memory);
                                    window.set_selected_process_memory_percent(
                                        process.memory_percent,
                                    );
                                } else {
                                    clear_selected_process(&window);
                                }
                                window.set_processes(ModelRc::new(VecModel::from(rows)));
                                window.set_volumes(ModelRc::new(VecModel::from(volumes)));
                                window.set_volume_status(SharedString::from(volume_status));
                                window.set_network_interfaces(ModelRc::new(VecModel::from(
                                    network_interfaces,
                                )));
                                window.set_network_status(SharedString::from(network_status));
                                window
                                    .set_block_devices(ModelRc::new(VecModel::from(block_devices)));
                                window.set_block_device_io_status(SharedString::from(
                                    block_device_io_status,
                                ));
                                window.set_cpu_cores(ModelRc::new(VecModel::from(cpu_cores)));
                                window.set_gpus(ModelRc::new(VecModel::from(gpus)));
                                window.set_gpu_status(SharedString::from(gpu_status));
                                window.set_battery_value(SharedString::from(battery_value));
                                window.set_battery_status(SharedString::from(battery_status));
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
                        let agent_status = localize_agent_status(
                            unavailable_status(&ipc_error, startup_mode, startup_error.as_deref()),
                            locale,
                        );
                        let startup_configured = startup_mode.is_some() && startup_error.is_none();
                        let weak_for_status = weak_window.clone();
                        if let Err(invoke_error) = slint::invoke_from_event_loop(move || {
                            if let Some(window) = weak_for_status.upgrade() {
                                window.set_agent_connected(false);
                                window.set_agent_startup_configured(startup_configured);
                                window.set_agent_status(SharedString::from(agent_status));
                                window
                                    .set_sample_age(SharedString::from(locale.no_recent_sample()));
                                window.set_cpu_value(SharedString::from("—"));
                                window.set_cpu_status(SharedString::from(
                                    locale.agent_disconnected(),
                                ));
                                window.set_memory_value(SharedString::from("—"));
                                window.set_swap_value(SharedString::from("—"));
                                window.set_memory_status(SharedString::from(
                                    locale.agent_disconnected(),
                                ));
                                window.set_swap_status(SharedString::from(
                                    locale.agent_disconnected(),
                                ));
                                window.set_process_count(SharedString::from("—"));
                                window.set_process_count_description(SharedString::from("—"));
                                window.set_selected_process_key(SharedString::from(""));
                                clear_selected_process(&window);
                                window.set_processes(ModelRc::new(VecModel::from(Vec::new())));
                                window.set_volumes(ModelRc::new(VecModel::from(Vec::new())));
                                window.set_volume_status(SharedString::from(
                                    locale.waiting_for_agent(),
                                ));
                                window.set_network_interfaces(ModelRc::new(VecModel::from(
                                    Vec::new(),
                                )));
                                window.set_network_status(SharedString::from(
                                    locale.waiting_for_agent(),
                                ));
                                window.set_block_devices(ModelRc::new(VecModel::from(Vec::new())));
                                window.set_block_device_io_status(SharedString::from(
                                    locale.waiting_for_agent(),
                                ));
                                window.set_cpu_cores(ModelRc::new(VecModel::from(Vec::new())));
                                window.set_gpus(ModelRc::new(VecModel::from(Vec::new())));
                                window.set_gpu_status(SharedString::from(locale.gpu_unavailable()));
                                window.set_battery_value(SharedString::from("—"));
                                window.set_battery_status(SharedString::from(
                                    locale.battery_unavailable(),
                                ));
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

fn process_identity_key(process: &ProcessMetrics) -> String {
    format!("{}:{}", process.pid, process.start_time_unix_seconds)
}

fn find_selected_process<'a>(
    processes: &'a [ProcessMetrics],
    selected_key: &str,
) -> Option<&'a ProcessMetrics> {
    processes
        .iter()
        .find(|process| process_identity_key(process) == selected_key)
}

fn process_row(
    process: &ProcessMetrics,
    total_memory_bytes: Option<u64>,
    now_unix_ms: u64,
    locale: Locale,
) -> ProcessRow {
    ProcessRow {
        key: SharedString::from(process_identity_key(process)),
        name: SharedString::from(process.name.clone()),
        pid: SharedString::from(process.pid.to_string()),
        parent_pid: SharedString::from(
            process
                .parent_pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "—".to_owned()),
        ),
        started: SharedString::from(process_start_age_label(
            process.start_time_unix_seconds,
            now_unix_ms / 1_000,
            locale,
        )),
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
        memory_percent: SharedString::from(
            memory_percentage(process.memory_bytes, total_memory_bytes)
                .map(|percentage| format!("{percentage:.1}%"))
                .unwrap_or_else(|| "—".to_owned()),
        ),
    }
}

fn process_tree_rows(processes: &[ProcessMetrics]) -> Vec<ProcessTreeRow> {
    let mut depths = std::collections::HashMap::<u32, i32>::with_capacity(processes.len());
    let mut rows = Vec::with_capacity(processes.len());
    for process in processes {
        let depth = process
            .parent_pid
            .and_then(|parent_pid| depths.get(&parent_pid).copied())
            .map(|parent_depth| parent_depth.saturating_add(1))
            .unwrap_or(0);
        depths.insert(process.pid, depth);
        rows.push(ProcessTreeRow {
            label: SharedString::from(format!("{} (PID {})", process.name, process.pid)),
            depth,
        });
    }
    rows
}

fn memory_percentage(used_bytes: u64, total_bytes: Option<u64>) -> Option<f64> {
    let total_bytes = total_bytes.filter(|total| *total > 0)?;
    Some(used_bytes as f64 / total_bytes as f64 * 100.0)
}

fn format_bytes(bytes: Option<u64>) -> String {
    let Some(bytes) = bytes else {
        return "—".to_owned();
    };

    const KIB: f64 = 1_024.0;
    const MIB: f64 = KIB * 1_024.0;
    const GIB: f64 = MIB * 1_024.0;

    if bytes >= GIB as u64 {
        format!("{:.1} GiB", bytes as f64 / GIB)
    } else if bytes >= MIB as u64 {
        format!("{:.1} MiB", bytes as f64 / MIB)
    } else if bytes >= KIB as u64 {
        format!("{:.1} KiB", bytes as f64 / KIB)
    } else {
        format!("{bytes} B")
    }
}

fn format_bytes_per_second(bytes_per_second: Option<f64>) -> String {
    let Some(bytes_per_second) =
        bytes_per_second.filter(|value| value.is_finite() && *value >= 0.0)
    else {
        return "—".to_owned();
    };

    const KIB: f64 = 1_024.0;
    const MIB: f64 = KIB * 1_024.0;
    const GIB: f64 = MIB * 1_024.0;

    if bytes_per_second >= GIB {
        format!("{:.1} GiB/s", bytes_per_second / GIB)
    } else if bytes_per_second >= MIB {
        format!("{:.1} MiB/s", bytes_per_second / MIB)
    } else if bytes_per_second >= KIB {
        format!("{:.1} KiB/s", bytes_per_second / KIB)
    } else {
        format!("{bytes_per_second:.0} B/s")
    }
}

fn format_operations_per_second(operations_per_second: Option<f64>) -> String {
    operations_per_second
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| format!("{value:.1} op/s"))
        .unwrap_or_else(|| "—".to_owned())
}

fn format_latency_ms(latency_ms: Option<f64>) -> String {
    latency_ms
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "—".to_owned())
}

fn process_start_age_label(
    start_time_unix_seconds: u64,
    now_unix_seconds: u64,
    locale: Locale,
) -> String {
    if start_time_unix_seconds == 0 {
        return locale.unavailable().to_owned();
    }

    let age_seconds = now_unix_seconds.saturating_sub(start_time_unix_seconds);
    if age_seconds < 60 {
        locale.relative_age(age_seconds, "sec", "s")
    } else if age_seconds < 3_600 {
        locale.relative_age(age_seconds / 60, "min", "min")
    } else if age_seconds < 86_400 {
        locale.relative_age(age_seconds / 3_600, "hr", "h")
    } else {
        locale.relative_age(age_seconds / 86_400, "day", "d")
    }
}

fn clear_selected_process(window: &MainWindow) {
    window.set_has_selected_process(false);
    window.set_selected_process_name(SharedString::from(""));
    window.set_selected_process_pid(SharedString::from("—"));
    window.set_selected_process_parent_pid(SharedString::from("—"));
    window.set_selected_process_started(SharedString::from("—"));
    window.set_selected_process_cpu(SharedString::from("—"));
    window.set_selected_process_memory(SharedString::from("—"));
    window.set_selected_process_memory_percent(SharedString::from("—"));
    window.set_process_tree(ModelRc::new(VecModel::from(Vec::new())));
    window.set_process_tree_status(SharedString::from(""));
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
        ordering
            .then_with(|| left.pid.cmp(&right.pid))
            .then_with(|| {
                left.start_time_unix_seconds
                    .cmp(&right.start_time_unix_seconds)
            })
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

fn memory_summary(used_bytes: Option<u64>, total_bytes: Option<u64>) -> (String, String) {
    match (used_bytes, total_bytes.filter(|total| *total > 0)) {
        (Some(used), Some(total)) => {
            let used_gib = used as f64 / (1024.0 * 1024.0 * 1024.0);
            let total_gib = total as f64 / (1024.0 * 1024.0 * 1024.0);
            let percentage = used as f64 / total as f64 * 100.0;
            (
                format!("{used_gib:.1} / {total_gib:.1} GiB"),
                format!("{percentage:.1}% utilizado"),
            )
        }
        _ => ("—".to_owned(), "Indisponível nesta amostra".to_owned()),
    }
}

fn swap_summary(used_bytes: Option<u64>, total_bytes: Option<u64>) -> (String, String) {
    match (used_bytes, total_bytes) {
        (Some(0), Some(0)) => ("Sem swap".to_owned(), "Nenhuma área configurada".to_owned()),
        (Some(used), Some(total)) if total > 0 => {
            let (value, percentage) = memory_summary(Some(used), Some(total));
            (value, format!("{percentage} · disponível"))
        }
        _ => ("—".to_owned(), "Indisponível nesta amostra".to_owned()),
    }
}

fn metric_status_label(status: &str, locale: Locale) -> String {
    match status {
        "ok" => if locale == Locale::EnUs {
            "Available"
        } else {
            "Disponível"
        }
        .to_owned(),
        "warming_up" => if locale == Locale::EnUs {
            "Warming up"
        } else {
            "Aquecendo"
        }
        .to_owned(),
        "unsupported" => if locale == Locale::EnUs {
            "Not supported"
        } else {
            "Não suportado"
        }
        .to_owned(),
        "temporarily_unavailable" => if locale == Locale::EnUs {
            "Temporarily unavailable"
        } else {
            "Temporariamente indisponível"
        }
        .to_owned(),
        "error" => if locale == Locale::EnUs {
            "Error"
        } else {
            "Erro"
        }
        .to_owned(),
        other => other.to_owned(),
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

fn localized_sample_age(value: &str, locale: Locale) -> String {
    if locale == Locale::PtBr {
        return value.to_owned();
    }
    match value {
        "Aguardando primeira amostra" => "Waiting for the first sample".to_owned(),
        "Amostra há menos de 1 s" => "Sample less than 1 s ago".to_owned(),
        value if value.starts_with("Amostra há ") => value
            .strip_prefix("Amostra há ")
            .map(|age| format!("Sample {age} ago"))
            .unwrap_or_else(|| value.to_owned()),
        _ => value.to_owned(),
    }
}

fn localize_agent_status(value: String, locale: Locale) -> String {
    if locale == Locale::PtBr {
        return value;
    }
    let replacements = [
        ("Agente ativo", "Agent active"),
        ("Agente conectado", "Agent connected"),
        ("Agente indisponível", "Agent unavailable"),
        ("coleta:", "collection:"),
        ("início no login:", "start at login:"),
        (
            "início automático não configurado:",
            "automatic startup not configured:",
        ),
    ];
    replacements
        .into_iter()
        .fold(value, |text, (from, to)| text.replace(from, to))
}

fn localize_summary(value: String, locale: Locale) -> String {
    if locale == Locale::PtBr {
        return value;
    }
    value
        .replace("utilizado", "used")
        .replace("disponível", "available")
        .replace("Indisponível nesta amostra", "Unavailable in this sample")
        .replace("Sem swap", "No swap")
        .replace("Nenhuma área configurada", "No swap area configured")
}

fn localize_history_label(value: String, locale: Locale) -> String {
    if locale == Locale::PtBr {
        return value;
    }
    match value.as_str() {
        "sem dados" => "no data".to_owned(),
        "histórico" => "history".to_owned(),
        "início" => "start".to_owned(),
        value if value.starts_with('−') => format!("{} min ago", &value[1..]),
        _ => value,
    }
}

fn process_count_description(count: usize, locale: Locale) -> String {
    match (count, locale) {
        (1, Locale::EnUs) => "1 process visible".to_owned(),
        (_, Locale::EnUs) => format!("{count} processes visible"),
        (1, Locale::PtBr) => "1 processo visível".to_owned(),
        (_, Locale::PtBr) => format!("{count} processos visíveis"),
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
        FIVE_MINUTES_MS, Locale, MainPage, MainWindow, ONE_MINUTE_MS, ProcessRow, ProcessSort,
        SortKey, UiLanguage, compare_optional_cpu, downsample_max, find_selected_process,
        format_bytes, format_bytes_per_second, lifecycle_status, localize_history_label,
        memory_percentage, memory_summary, metric_status_label, process_count_description,
        process_identity_key, process_start_age_label, process_tree_rows, sample_age_label,
        select_history, sort_processes, swap_summary, unavailable_status,
    };
    use native_monitor_agent_lifecycle::StartupMode;
    use native_monitor_ipc::wire::ProcessMetrics;
    use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
    use std::sync::Mutex;

    static UI_TEST_LOCK: Mutex<()> = Mutex::new(());

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
        assert_eq!(localize_history_label(label, Locale::EnUs), "start");
    }

    #[test]
    fn empty_history_label_is_localized_for_english() {
        let (history, label) = select_history(&[], 1_000, FIVE_MINUTES_MS, 72);

        assert!(history.is_empty());
        assert_eq!(localize_history_label(label, Locale::EnUs), "no data");
    }

    #[test]
    fn process_count_description_uses_localized_singular_and_plural_forms() {
        assert_eq!(
            process_count_description(1, Locale::EnUs),
            "1 process visible"
        );
        assert_eq!(
            process_count_description(2, Locale::EnUs),
            "2 processes visible"
        );
        assert_eq!(
            process_count_description(1, Locale::PtBr),
            "1 processo visível"
        );
        assert_eq!(
            process_count_description(2, Locale::PtBr),
            "2 processos visíveis"
        );
        assert_eq!(
            process_count_description(0, Locale::PtBr),
            "0 processos visíveis"
        );
    }

    #[test]
    fn memory_summary_does_not_invent_missing_values() {
        assert_eq!(
            memory_summary(None, Some(8 * 1024 * 1024 * 1024)),
            ("—".to_owned(), "Indisponível nesta amostra".to_owned())
        );
        assert_eq!(
            memory_summary(Some(4 * 1024 * 1024 * 1024), Some(8 * 1024 * 1024 * 1024)),
            ("4.0 / 8.0 GiB".to_owned(), "50.0% utilizado".to_owned())
        );
    }

    #[test]
    fn memory_percentage_preserves_missing_and_zero_total_states() {
        assert_eq!(memory_percentage(1_024, None), None);
        assert_eq!(memory_percentage(1_024, Some(0)), None);
        assert_eq!(memory_percentage(1_024, Some(4_096)), Some(25.0));
    }

    #[test]
    fn byte_formatting_uses_iec_units_and_preserves_missing_values() {
        assert_eq!(format_bytes(None), "—");
        assert_eq!(format_bytes(Some(0)), "0 B");
        assert_eq!(format_bytes(Some(1_536)), "1.5 KiB");
        assert_eq!(format_bytes(Some(1_572_864)), "1.5 MiB");
        assert_eq!(format_bytes(Some(1_610_612_736)), "1.5 GiB");
        assert_eq!(format_bytes_per_second(None), "—");
        assert_eq!(format_bytes_per_second(Some(1_536.0)), "1.5 KiB/s");
        assert_eq!(format_bytes_per_second(Some(f64::NAN)), "—");
    }

    #[test]
    fn swap_summary_distinguishes_missing_from_unconfigured_swap() {
        assert_eq!(
            swap_summary(Some(0), Some(0)),
            ("Sem swap".to_owned(), "Nenhuma área configurada".to_owned())
        );
        assert_eq!(
            swap_summary(None, None),
            ("—".to_owned(), "Indisponível nesta amostra".to_owned())
        );
        assert_eq!(
            swap_summary(Some(1_073_741_824), Some(2_147_483_648)),
            (
                "1.0 / 2.0 GiB".to_owned(),
                "50.0% utilizado · disponível".to_owned()
            )
        );
    }

    #[test]
    fn metric_status_is_localized_for_overview_cards() {
        assert_eq!(metric_status_label("ok", Locale::PtBr), "Disponível");
        assert_eq!(metric_status_label("ok", Locale::EnUs), "Available");
        assert_eq!(metric_status_label("warming_up", Locale::PtBr), "Aquecendo");
        assert_eq!(
            metric_status_label("unexpected", Locale::EnUs),
            "unexpected"
        );
    }

    #[test]
    fn process_selection_identity_distinguishes_reused_pids() {
        let original = process(42, "worker", Some(0.0), 100, None);
        let mut replacement = original.clone();
        replacement.start_time_unix_seconds = 200;

        assert_ne!(
            process_identity_key(&original),
            process_identity_key(&replacement)
        );
        assert_eq!(process_identity_key(&original), "42:1");
    }

    #[test]
    fn process_tree_rows_preserve_breadth_first_order_and_depth() {
        let rows = process_tree_rows(&[
            process(10, "root", Some(1.0), 10, None),
            process(11, "child", Some(2.0), 20, Some(10)),
            process(12, "grandchild", Some(3.0), 30, Some(11)),
            process(13, "sibling", Some(4.0), 40, Some(10)),
        ]);

        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].label.as_str(), "root (PID 10)");
        assert_eq!(rows[0].depth, 0);
        assert_eq!(rows[1].depth, 1);
        assert_eq!(rows[2].depth, 2);
        assert_eq!(rows[3].depth, 1);
    }

    #[test]
    fn selected_process_lookup_does_not_select_a_reused_pid() {
        let original = process(42, "worker", Some(0.0), 100, None);
        let mut replacement = original.clone();
        replacement.start_time_unix_seconds = 200;

        assert!(find_selected_process(std::slice::from_ref(&original), "42:1").is_some());
        assert!(find_selected_process(std::slice::from_ref(&replacement), "42:1").is_none());
    }

    #[test]
    fn clicking_a_process_row_emits_its_pid_and_start_time_identity() {
        use i_slint_backend_testing::ElementHandle;
        use slint::platform::PointerEventButton;
        use std::cell::RefCell;
        use std::rc::Rc;

        let _guard = UI_TEST_LOCK.lock().expect("lock UI integration test");
        i_slint_backend_testing::init_integration_test_with_system_time();

        slint::spawn_local(async move {
            let window = MainWindow::new().expect("create the test window");
            window.set_active_page(MainPage::Processes);
            window.set_processes(ModelRc::new(VecModel::from(vec![ProcessRow {
                key: SharedString::from("42:1700000000"),
                name: SharedString::from("fixture-worker"),
                pid: SharedString::from("42"),
                parent_pid: SharedString::from("1"),
                started: SharedString::from("há 1 min"),
                cpu: SharedString::from("1.0%"),
                memory: SharedString::from("4.0"),
                memory_percent: SharedString::from("50.0%"),
            }])));

            let selected_key = Rc::new(RefCell::new(None::<String>));
            window.on_process_selected({
                let selected_key = Rc::clone(&selected_key);
                move |key| {
                    *selected_key.borrow_mut() = Some(key.to_string());
                }
            });

            let weak_window = window.as_weak();
            window.on_language_selected(move |value| {
                if let Some(locale) = Locale::from_selection(value.as_str())
                    && let Some(window) = weak_window.upgrade()
                {
                    window.set_language(locale.ui_language());
                }
            });

            let mut targets =
                ElementHandle::find_by_element_id(&window, "MainWindow::process_row_hit_target");
            let target = targets.next().expect("find the process row hit target");
            assert!(targets.next().is_none());

            target.single_click(PointerEventButton::Left).await;

            assert_eq!(selected_key.borrow().as_deref(), Some("42:1700000000"));
            assert_eq!(window.get_selected_process_key().as_str(), "42:1700000000");
            assert!(window.get_has_selected_process());
            assert_eq!(
                window.get_selected_process_name().as_str(),
                "fixture-worker"
            );
            assert_eq!(window.get_selected_process_pid().as_str(), "42");
            assert_eq!(window.get_selected_process_parent_pid().as_str(), "1");
            assert_eq!(window.get_selected_process_started().as_str(), "há 1 min");
            assert_eq!(window.get_selected_process_cpu().as_str(), "1.0%");
            assert_eq!(window.get_selected_process_memory().as_str(), "4.0");
            assert_eq!(
                window.get_selected_process_memory_percent().as_str(),
                "50.0%"
            );

            let dispatch_shortcut = |digit: &str| {
                use slint::platform::{Key, WindowEvent};

                window.window().dispatch_event(WindowEvent::KeyPressed {
                    text: Key::Control.into(),
                });
                window
                    .window()
                    .dispatch_event(WindowEvent::KeyPressed { text: digit.into() });
                window
                    .window()
                    .dispatch_event(WindowEvent::KeyReleased { text: digit.into() });
                window.window().dispatch_event(WindowEvent::KeyReleased {
                    text: Key::Control.into(),
                });
            };

            dispatch_shortcut("5");
            assert_eq!(window.get_active_page(), MainPage::Network);
            dispatch_shortcut("2");
            assert_eq!(window.get_active_page(), MainPage::Processes);

            let mut language_selectors =
                ElementHandle::find_by_element_id(&window, "MainWindow::language_en_us");
            language_selectors
                .next()
                .expect("find English selector")
                .single_click(PointerEventButton::Left)
                .await;
            assert_eq!(window.get_language(), UiLanguage::EnUs);
            slint::quit_event_loop().expect("stop the testing event loop");
        })
        .expect("schedule the UI interaction");

        slint::run_event_loop().expect("run the testing event loop");
    }

    #[test]
    fn process_start_age_shows_relative_time_and_missing_values() {
        assert_eq!(
            process_start_age_label(0, 10_000, Locale::PtBr),
            "Indisponível"
        );
        assert_eq!(
            process_start_age_label(9_970, 10_000, Locale::PtBr),
            "há 30 s"
        );
        assert_eq!(
            process_start_age_label(9_000, 10_000, Locale::PtBr),
            "há 16 min"
        );
        assert_eq!(process_start_age_label(1_000, 100, Locale::PtBr), "há 0 s");
        assert_eq!(
            process_start_age_label(9_970, 10_000, Locale::EnUs),
            "30 secs ago"
        );
    }

    #[test]
    fn system_locale_maps_supported_languages_and_defaults_to_portuguese() {
        assert_eq!(Locale::from_language_tag(Some("pt_BR")), Locale::PtBr);
        assert_eq!(Locale::from_language_tag(Some("en-GB")), Locale::EnUs);
        assert_eq!(Locale::from_language_tag(Some("fr-FR")), Locale::PtBr);
        assert_eq!(Locale::from_language_tag(None), Locale::PtBr);
    }

    #[test]
    fn manual_locale_selection_accepts_only_supported_locales() {
        assert_eq!(Locale::from_selection("pt-BR"), Some(Locale::PtBr));
        assert_eq!(Locale::from_selection("en-US"), Some(Locale::EnUs));
        assert_eq!(Locale::from_selection("fr-FR"), None);
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
