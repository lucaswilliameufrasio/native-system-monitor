use anyhow::{Context, Result, anyhow, bail};
use native_monitor_agent_lifecycle::{self as agent_lifecycle, StartupMode};
use native_monitor_domain::NetworkInterfaceKind;
use native_monitor_ipc::client;
use native_monitor_ipc::wire::{
    NetworkInterfaceKind as WireNetworkInterfaceKind, Operation, ProcessMetrics,
};
use slint::{ModelRc, SharedString, VecModel};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};
use tracing_subscriber::EnvFilter;

slint::include_modules!();

mod localization;
mod ui_settings;

use localization::{CountKind, UiCatalogs};
use ui_settings::{ByteUnitSystem, UiSettings};

const MAX_PROCESS_ROWS: usize = 1_000;
const MAX_PROCESS_FILTER_CHARS: usize = 256;
const MAX_CHART_POINTS: usize = 72;
const PROCESS_QUERY_PROTOCOL_MINOR: u32 = 2;
const VOLUME_QUERY_PROTOCOL_MINOR: u32 = 3;
const VOLUME_IO_QUERY_PROTOCOL_MINOR: u32 = 5;
const NETWORK_QUERY_PROTOCOL_MINOR: u32 = 3;
const NETWORK_INTERFACE_KIND_PROTOCOL_MINOR: u32 = 10;
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
        let formatted_amount = self.format_integer(u128::from(amount));
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
            format!("{formatted_amount} {unit} ago")
        } else {
            format!("há {formatted_amount} {portuguese_unit}")
        }
    }
    fn format_integer(self, value: u128) -> String {
        self.group_integer(&value.to_string())
    }
    fn format_decimal(self, value: f64, precision: usize) -> String {
        let formatted = format!("{value:.precision$}");
        let Some((integer, fraction)) = formatted.split_once('.') else {
            return self.group_integer(&formatted);
        };
        let integer = self.group_integer(integer);
        let decimal_separator = match self {
            Self::PtBr => ',',
            Self::EnUs => '.',
        };
        format!("{integer}{decimal_separator}{fraction}")
    }
    fn group_integer(self, value: &str) -> String {
        let (sign, digits) = if let Some(digits) = value.strip_prefix('-') {
            ("-", digits)
        } else if let Some(digits) = value.strip_prefix('+') {
            ("+", digits)
        } else {
            ("", value)
        };
        if digits.is_empty() || !digits.bytes().all(|digit| digit.is_ascii_digit()) {
            return value.to_owned();
        }

        let grouping_separator = match self {
            Self::PtBr => '.',
            Self::EnUs => ',',
        };
        let mut grouped = String::with_capacity(value.len() + digits.len() / 3);
        grouped.push_str(sign);
        for (index, digit) in digits.bytes().enumerate() {
            if index > 0 && (digits.len() - index) % 3 == 0 {
                grouped.push(grouping_separator);
            }
            grouped.push(char::from(digit));
        }
        grouped
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

    let ui_catalogs = Arc::new(UiCatalogs::load()?);
    let window = MainWindow::new().context("create the desktop window")?;
    let initial_locale = Locale::from_system();
    window.set_language(initial_locale.ui_language());
    apply_header_texts(&window, &ui_catalogs, initial_locale);
    apply_navigation_texts(&window, &ui_catalogs, initial_locale);
    apply_density_texts(&window, &ui_catalogs, initial_locale);
    apply_overview_texts(&window, &ui_catalogs, initial_locale);
    apply_performance_texts(&window, &ui_catalogs, initial_locale);
    apply_process_texts(&window, &ui_catalogs, initial_locale);
    window.set_volume_count(SharedString::from(ui_catalogs.format_count(
        CountKind::Volumes,
        0,
        initial_locale,
    )));
    window.set_network_interface_count(SharedString::from(ui_catalogs.format_count(
        CountKind::NetworkInterfaces,
        0,
        initial_locale,
    )));
    let locale_state = Arc::new(AtomicU8::new(initial_locale.code()));
    let settings_path = ui_settings::default_settings_path();
    let initial_ui_settings = match settings_path.as_deref() {
        Some(path) => match UiSettings::load(path) {
            Ok(settings) => settings,
            Err(error) => {
                warn!(%error, "could not load desktop preferences; using defaults");
                UiSettings::default()
            }
        },
        None => UiSettings::default(),
    };
    let initial_byte_unit_system = initial_ui_settings.byte_unit_system;
    window.set_byte_unit_system(initial_byte_unit_system.ui_value());
    let byte_unit_system_state = Arc::new(AtomicU8::new(initial_byte_unit_system.code()));
    let unit_state_for_callback = Arc::clone(&byte_unit_system_state);
    window.on_byte_unit_system_selected(move |value| {
        match apply_byte_unit_system_selection(
            value.as_str(),
            &unit_state_for_callback,
            settings_path.as_deref(),
        ) {
            Ok(Some(_)) if settings_path.is_none() => {
                warn!("no per-user configuration directory; byte unit choice is session-only");
            }
            Ok(Some(_)) => {}
            Ok(None) => warn!(unit_system = %value, "ignored unsupported byte unit system"),
            Err(error) => warn!(%error, "could not persist byte unit preference"),
        }
    });
    let locale_for_callback = Arc::clone(&locale_state);
    let catalogs_for_locale_callback = Arc::clone(&ui_catalogs);
    let weak_window_for_locale = window.as_weak();
    window.on_language_selected(move |value| {
        let Some(locale) = Locale::from_selection(value.as_str()) else {
            warn!(locale = %value, "ignored unsupported UI locale");
            return;
        };
        locale_for_callback.store(locale.code(), Ordering::Release);
        if let Some(window) = weak_window_for_locale.upgrade() {
            window.set_language(locale.ui_language());
            apply_header_texts(&window, &catalogs_for_locale_callback, locale);
            apply_navigation_texts(&window, &catalogs_for_locale_callback, locale);
            apply_density_texts(&window, &catalogs_for_locale_callback, locale);
            apply_overview_texts(&window, &catalogs_for_locale_callback, locale);
            apply_performance_texts(&window, &catalogs_for_locale_callback, locale);
            apply_process_texts(&window, &catalogs_for_locale_callback, locale);
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
    let worker_catalogs = Arc::clone(&ui_catalogs);
    let worker_byte_unit_system = Arc::clone(&byte_unit_system_state);
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
                let byte_unit_system =
                    ByteUnitSystem::from_code(worker_byte_unit_system.load(Ordering::Acquire));
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
                                    process_row(
                                        process,
                                        total_memory_bytes,
                                        now_unix_ms,
                                        locale,
                                        byte_unit_system,
                                    )
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
                                .map(|value| {
                                    format!("{}%", locale.format_decimal(f64::from(value), 1))
                                })
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
                                locale,
                                byte_unit_system,
                            );
                            let memory_status = snapshot
                                .memory
                                .as_ref()
                                .map(|memory| metric_status_label(memory.status.as_str(), locale))
                                .unwrap_or_else(|| locale.unavailable().to_owned());
                            let memory_sample_is_ok = snapshot
                                .memory
                                .as_ref()
                                .is_some_and(|memory| memory.status == "ok");
                            let memory_status = memory_status_description(
                                &memory_value,
                                &memory_usage,
                                &memory_status,
                                memory_sample_is_ok,
                                locale,
                            );
                            let (swap_value, swap_status) = snapshot
                                .memory
                                .as_ref()
                                .map(|memory| {
                                    swap_summary(
                                        memory.swap_used_bytes,
                                        memory.swap_total_bytes,
                                        locale,
                                        byte_unit_system,
                                    )
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
                            let cpu_history_accessible_summary =
                                chart_accessible_summary("CPU", &cpu_history, locale);
                            let memory_history_accessible_summary =
                                chart_accessible_summary("memory", &memory_history, locale);
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
                                            usage: SharedString::from(format!(
                                                "{}%",
                                                locale.format_decimal(f64::from(*usage), 1)
                                            )),
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
                                            .map(|value| {
                                                format!(
                                                    "{}%",
                                                    locale.format_decimal(f64::from(value), 0)
                                                )
                                            })
                                            .unwrap_or_else(|| "—".to_owned()),
                                    ),
                                    memory: SharedString::from(format!(
                                        "{} / {}",
                                        format_bytes(
                                            gpu.memory_used_bytes,
                                            locale,
                                            byte_unit_system,
                                        ),
                                        format_bytes(
                                            gpu.memory_total_bytes,
                                            locale,
                                            byte_unit_system,
                                        ),
                                    )),
                                    temperature: SharedString::from(
                                        gpu.temperature_celsius
                                            .map(|value| {
                                                format!(
                                                    "{} °C",
                                                    locale.format_integer(u128::from(value))
                                                )
                                            })
                                            .unwrap_or_else(|| "—".to_owned()),
                                    ),
                                    clocks: SharedString::from(format_gpu_clocks(
                                        gpu.graphics_clock_mhz,
                                        gpu.memory_clock_mhz,
                                        locale,
                                    )),
                                    power: SharedString::from(
                                        gpu.power_watts
                                            .map(|value| {
                                                format!("{} W", locale.format_decimal(value, 1))
                                            })
                                            .unwrap_or_else(|| "—".to_owned()),
                                    ),
                                })
                                .collect::<Vec<_>>();
                            let gpu_status = gpu_status_label(
                                response.protocol_minor,
                                gpus.len(),
                                snapshot.gpu_status.as_str(),
                                locale,
                            );
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
                                        .map(|value| {
                                            format!("{} W", locale.format_decimal(value, 1))
                                        })
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
                                    total: SharedString::from(format_bytes(
                                        volume.total_bytes,
                                        locale,
                                        byte_unit_system,
                                    )),
                                    available: SharedString::from(format_bytes(
                                        volume.available_bytes,
                                        locale,
                                        byte_unit_system,
                                    )),
                                    read_rate: SharedString::from(format_bytes_per_second(
                                        volume.read_bytes_per_second,
                                        locale,
                                        byte_unit_system,
                                    )),
                                    write_rate: SharedString::from(format_bytes_per_second(
                                        volume.write_bytes_per_second,
                                        locale,
                                        byte_unit_system,
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
                                    kind: SharedString::from(network_interface_kind_label(
                                        network_interface_kind_from_wire(
                                            interface.kind,
                                            response.protocol_minor,
                                        ),
                                        locale,
                                    )),
                                    received: SharedString::from(format_bytes_per_second(
                                        interface.received_bytes_per_second,
                                        locale,
                                        byte_unit_system,
                                    )),
                                    transmitted: SharedString::from(format_bytes_per_second(
                                        interface.transmitted_bytes_per_second,
                                        locale,
                                        byte_unit_system,
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
                                        locale,
                                        byte_unit_system,
                                    )),
                                    write_rate: SharedString::from(format_bytes_per_second(
                                        device.write_bytes_per_second,
                                        locale,
                                        byte_unit_system,
                                    )),
                                    read_iops: SharedString::from(format_operations_per_second(
                                        device.read_operations_per_second,
                                        locale,
                                    )),
                                    write_iops: SharedString::from(format_operations_per_second(
                                        device.write_operations_per_second,
                                        locale,
                                    )),
                                    read_latency: SharedString::from(format_latency_ms(
                                        device.read_latency_ms,
                                        locale,
                                    )),
                                    write_latency: SharedString::from(format_latency_ms(
                                        device.write_latency_ms,
                                        locale,
                                    )),
                                    busy: SharedString::from(
                                        device
                                            .busy_percent
                                            .map(|value| {
                                                format!(
                                                    "{}%",
                                                    locale.format_decimal(value, 1)
                                                )
                                            })
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
                                    process_row(
                                        process,
                                        total_memory_bytes,
                                        now_unix_ms,
                                        locale,
                                        byte_unit_system,
                                    )
                                })
                                .collect::<Vec<_>>();
                            let process_count = rows.len();
                            let process_count_description = worker_catalogs.format_count(
                                CountKind::Processes,
                                process_count,
                                locale,
                            );
                            let process_count_detail =
                                worker_catalogs.process_detail(process_count, locale);
                            let volume_count = worker_catalogs.format_count(
                                CountKind::Volumes,
                                volumes.len(),
                                locale,
                            );
                            let network_interface_count = worker_catalogs.format_count(
                                CountKind::NetworkInterfaces,
                                network_interfaces.len(),
                                locale,
                            );
                            let agent_status = lifecycle_status(
                                status.as_str(),
                                startup_mode,
                                startup_error.as_deref(),
                            );
                            let startup_configured =
                                startup_mode.is_some() && startup_error.is_none();
                            let sample_age =
                                sample_age_label(captured_at, unix_time_ms(), locale);
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
                                window.set_process_count(SharedString::from(
                                    locale.format_integer(rows.len() as u128),
                                ));
                                window.set_process_count_description(SharedString::from(
                                    process_count_description,
                                ));
                                window.set_process_count_detail(SharedString::from(
                                    process_count_detail,
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
                                window.set_volume_count(SharedString::from(volume_count));
                                window.set_volume_status(SharedString::from(volume_status));
                                window.set_network_interfaces(ModelRc::new(VecModel::from(
                                    network_interfaces,
                                )));
                                window.set_network_interface_count(SharedString::from(
                                    network_interface_count,
                                ));
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
                                window.set_cpu_history_accessible_summary(SharedString::from(
                                    cpu_history_accessible_summary,
                                ));
                                window.set_memory_history_accessible_summary(SharedString::from(
                                    memory_history_accessible_summary,
                                ));
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
                        let empty_volume_count =
                            worker_catalogs.format_count(CountKind::Volumes, 0, locale);
                        let empty_network_interface_count = worker_catalogs.format_count(
                            CountKind::NetworkInterfaces,
                            0,
                            locale,
                        );
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
                                window.set_process_count_detail(SharedString::from(""));
                                window.set_selected_process_key(SharedString::from(""));
                                clear_selected_process(&window);
                                window.set_processes(ModelRc::new(VecModel::from(Vec::new())));
                                window.set_volumes(ModelRc::new(VecModel::from(Vec::new())));
                                window.set_volume_count(SharedString::from(empty_volume_count));
                                window.set_volume_status(SharedString::from(
                                    locale.waiting_for_agent(),
                                ));
                                window.set_network_interfaces(ModelRc::new(VecModel::from(
                                    Vec::new(),
                                )));
                                window.set_network_interface_count(SharedString::from(
                                    empty_network_interface_count,
                                ));
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

fn apply_byte_unit_system_selection(
    selection: &str,
    state: &AtomicU8,
    settings_path: Option<&Path>,
) -> Result<Option<ByteUnitSystem>> {
    let Some(byte_unit_system) = ByteUnitSystem::from_selection(selection) else {
        return Ok(None);
    };
    state.store(byte_unit_system.code(), Ordering::Release);

    if let Some(path) = settings_path {
        UiSettings { byte_unit_system }.save(path)?;
    }

    Ok(Some(byte_unit_system))
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
    byte_unit_system: ByteUnitSystem,
) -> ProcessRow {
    let (memory_value, memory_unit) =
        format_process_memory(process.memory_bytes, locale, byte_unit_system);
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
                .map(|value| locale.format_decimal(f64::from(value), 1))
                .unwrap_or_else(|| "—".to_owned()),
        ),
        memory: SharedString::from(memory_value),
        memory_unit: SharedString::from(memory_unit),
        memory_percent: SharedString::from(
            memory_percentage(process.memory_bytes, total_memory_bytes)
                .map(|percentage| format!("{}%", locale.format_decimal(percentage, 1)))
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

fn format_bytes(bytes: Option<u64>, locale: Locale, byte_unit_system: ByteUnitSystem) -> String {
    let Some(bytes) = bytes else {
        return "—".to_owned();
    };

    let base = byte_unit_system.base();
    let kilo = base;
    let mega = kilo * base;
    let giga = mega * base;
    let [kilo_label, mega_label, giga_label] = byte_unit_system.unit_labels();

    if bytes as f64 >= giga {
        format!(
            "{} {giga_label}",
            locale.format_decimal(bytes as f64 / giga, 1)
        )
    } else if bytes as f64 >= mega {
        format!(
            "{} {mega_label}",
            locale.format_decimal(bytes as f64 / mega, 1)
        )
    } else if bytes as f64 >= kilo {
        format!(
            "{} {kilo_label}",
            locale.format_decimal(bytes as f64 / kilo, 1)
        )
    } else {
        format!("{bytes} B")
    }
}

fn format_process_memory(
    bytes: u64,
    locale: Locale,
    byte_unit_system: ByteUnitSystem,
) -> (String, String) {
    let base = byte_unit_system.base();
    let value = locale.format_decimal(bytes as f64 / base.powi(2), 1);
    (value, byte_unit_system.process_memory_unit().to_owned())
}

fn format_gpu_clocks(
    graphics_clock_mhz: Option<u32>,
    memory_clock_mhz: Option<u32>,
    locale: Locale,
) -> String {
    let graphics_clock = graphics_clock_mhz
        .map(|value| locale.format_integer(u128::from(value)))
        .unwrap_or_else(|| "—".to_owned());
    let memory_clock = memory_clock_mhz
        .map(|value| locale.format_integer(u128::from(value)))
        .unwrap_or_else(|| "—".to_owned());
    format!("{graphics_clock} / {memory_clock} MHz")
}

fn format_bytes_per_second(
    bytes_per_second: Option<f64>,
    locale: Locale,
    byte_unit_system: ByteUnitSystem,
) -> String {
    let Some(bytes_per_second) =
        bytes_per_second.filter(|value| value.is_finite() && *value >= 0.0)
    else {
        return "—".to_owned();
    };

    let base = byte_unit_system.base();
    let kilo = base;
    let mega = kilo * base;
    let giga = mega * base;
    let [kilo_label, mega_label, giga_label] = byte_unit_system.unit_labels();

    if bytes_per_second >= giga {
        format!(
            "{} {giga_label}/s",
            locale.format_decimal(bytes_per_second / giga, 1)
        )
    } else if bytes_per_second >= mega {
        format!(
            "{} {mega_label}/s",
            locale.format_decimal(bytes_per_second / mega, 1)
        )
    } else if bytes_per_second >= kilo {
        format!(
            "{} {kilo_label}/s",
            locale.format_decimal(bytes_per_second / kilo, 1)
        )
    } else {
        format!("{} B/s", locale.format_decimal(bytes_per_second, 0))
    }
}

fn format_operations_per_second(operations_per_second: Option<f64>, locale: Locale) -> String {
    operations_per_second
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| format!("{} op/s", locale.format_decimal(value, 1)))
        .unwrap_or_else(|| "—".to_owned())
}

fn format_latency_ms(latency_ms: Option<f64>, locale: Locale) -> String {
    latency_ms
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| locale.format_decimal(value, 2))
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
    let memory_unit = match window.get_byte_unit_system() {
        UiByteUnitSystem::Si => "MB",
        UiByteUnitSystem::Iec => "MiB",
    };
    window.set_selected_process_memory_unit(SharedString::from(memory_unit));
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

fn memory_summary(
    used_bytes: Option<u64>,
    total_bytes: Option<u64>,
    locale: Locale,
    byte_unit_system: ByteUnitSystem,
) -> (String, String) {
    match (used_bytes, total_bytes.filter(|total| *total > 0)) {
        (Some(used), Some(total)) => {
            let base = byte_unit_system.base();
            let giga = base * base * base;
            let unit = byte_unit_system.unit_labels()[2];
            let used_gib = used as f64 / giga;
            let total_gib = total as f64 / giga;
            let percentage = used as f64 / total as f64 * 100.0;
            (
                format!(
                    "{} / {} {unit}",
                    locale.format_decimal(used_gib, 1),
                    locale.format_decimal(total_gib, 1)
                ),
                format!("{}% utilizado", locale.format_decimal(percentage, 1)),
            )
        }
        _ => ("—".to_owned(), "Indisponível nesta amostra".to_owned()),
    }
}

fn swap_summary(
    used_bytes: Option<u64>,
    total_bytes: Option<u64>,
    locale: Locale,
    byte_unit_system: ByteUnitSystem,
) -> (String, String) {
    match (used_bytes, total_bytes) {
        (Some(0), Some(0)) => ("Sem swap".to_owned(), "Nenhuma área configurada".to_owned()),
        (Some(used), Some(total)) if total > 0 => {
            let (value, percentage) =
                memory_summary(Some(used), Some(total), locale, byte_unit_system);
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

fn network_interface_kind_from_wire(kind: i32, protocol_minor: u32) -> NetworkInterfaceKind {
    if protocol_minor < NETWORK_INTERFACE_KIND_PROTOCOL_MINOR {
        return NetworkInterfaceKind::Unknown;
    }

    match WireNetworkInterfaceKind::try_from(kind) {
        Ok(WireNetworkInterfaceKind::Loopback) => NetworkInterfaceKind::Loopback,
        Ok(WireNetworkInterfaceKind::Unknown) | Err(_) => NetworkInterfaceKind::Unknown,
    }
}

fn network_interface_kind_label(kind: NetworkInterfaceKind, locale: Locale) -> &'static str {
    match kind {
        NetworkInterfaceKind::Loopback => "Loopback",
        NetworkInterfaceKind::Unknown if locale == Locale::EnUs => "Unknown",
        NetworkInterfaceKind::Unknown => "Desconhecido",
    }
}

fn gpu_status_label(protocol_minor: u32, gpu_count: usize, status: &str, locale: Locale) -> String {
    if protocol_minor < GPU_METRICS_PROTOCOL_MINOR {
        return locale.unsupported("GPU metrics").to_owned();
    }
    if gpu_count == 0 || status != "ok" {
        return metric_status_label(status, locale);
    }

    String::new()
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

fn sample_age_label(captured_at_unix_ms: u64, now_unix_ms: u64, locale: Locale) -> String {
    if captured_at_unix_ms == 0 {
        return "Aguardando primeira amostra".to_owned();
    }

    let age_ms = now_unix_ms.saturating_sub(captured_at_unix_ms);
    if age_ms < 1_000 {
        "Amostra há menos de 1 s".to_owned()
    } else if age_ms < 60_000 {
        format!(
            "Amostra há {} s",
            locale.format_integer(u128::from(age_ms / 1_000))
        )
    } else {
        format!(
            "Amostra há {} min",
            locale.format_integer(u128::from(age_ms / ONE_MINUTE_MS))
        )
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
        .replace("Indisponível nesta amostra", "Unavailable in this sample")
        .replace("disponível", "available")
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
        value if value.starts_with('−') => value
            .strip_prefix('−')
            .map(|age| format!("{age} ago"))
            .unwrap_or_else(|| value.to_owned()),
        _ => value,
    }
}

fn memory_status_description(
    memory_value: &str,
    memory_usage: &str,
    memory_status: &str,
    memory_sample_is_ok: bool,
    locale: Locale,
) -> String {
    let localized_memory_usage = localize_summary(memory_usage.to_owned(), locale);
    if memory_value != "—" {
        return format!("{localized_memory_usage} · {memory_status}");
    }

    if memory_sample_is_ok {
        localized_memory_usage
    } else {
        memory_status.to_owned()
    }
}

fn chart_accessible_summary(title: &str, values: &[f32], locale: Locale) -> String {
    let mut finite_values = values.iter().copied().filter(|value| value.is_finite());
    let Some(first_value) = finite_values.next() else {
        return if locale == Locale::EnUs {
            format!("{title} chart, waiting for samples")
        } else {
            format!("Gráfico de {title}, aguardando amostras")
        };
    };

    let mut minimum = first_value;
    let mut maximum = first_value;
    let mut latest = first_value;
    let mut sample_count = 1usize;
    for value in finite_values {
        minimum = minimum.min(value);
        maximum = maximum.max(value);
        latest = value;
        sample_count += 1;
    }

    if locale == Locale::EnUs {
        format!(
            "{title} chart, {} samples, minimum {}%, maximum {}%, latest {}%",
            locale.format_integer(sample_count as u128),
            locale.format_decimal(f64::from(minimum), 1),
            locale.format_decimal(f64::from(maximum), 1),
            locale.format_decimal(f64::from(latest), 1)
        )
    } else {
        let localized_title = if title == "CPU" { "CPU" } else { "memória" };
        format!(
            "Gráfico de {localized_title}, {} amostras, mínimo {}%, máximo {}%, atual {}%",
            locale.format_integer(sample_count as u128),
            locale.format_decimal(f64::from(minimum), 1),
            locale.format_decimal(f64::from(maximum), 1),
            locale.format_decimal(f64::from(latest), 1)
        )
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

fn apply_header_texts(window: &MainWindow, catalogs: &UiCatalogs, locale: Locale) {
    let header_texts = catalogs.header_texts(locale);
    window.set_header_title(SharedString::from(header_texts.title));
    window.set_header_tagline(SharedString::from(header_texts.tagline));
    window.set_agent_active_label(SharedString::from(header_texts.agent_active));
    window.set_agent_unavailable_label(SharedString::from(header_texts.agent_unavailable));
    window.set_setup_startup_label(SharedString::from(header_texts.setup_startup));
    window.set_retry_connection_label(SharedString::from(header_texts.retry_connection));
}

fn apply_navigation_texts(window: &MainWindow, catalogs: &UiCatalogs, locale: Locale) {
    let navigation_texts = catalogs.navigation_texts(locale);
    window.set_nav_section_label(SharedString::from(navigation_texts.section));
    window.set_nav_accessible_label(SharedString::from(navigation_texts.accessible_label));
    window.set_nav_overview_label(SharedString::from(navigation_texts.overview));
    window.set_nav_processes_label(SharedString::from(navigation_texts.processes));
    window.set_nav_performance_label(SharedString::from(navigation_texts.performance));
    window.set_nav_storage_label(SharedString::from(navigation_texts.storage));
    window.set_nav_network_label(SharedString::from(navigation_texts.network));
    window.set_nav_collection_continues_label(SharedString::from(
        navigation_texts.collection_continues,
    ));
    window.set_nav_keyboard_shortcut_label(SharedString::from(navigation_texts.keyboard_shortcut));
    window.set_nav_keyboard_shortcut_accessible_label(SharedString::from(
        navigation_texts.keyboard_shortcut_accessible,
    ));
}

fn apply_density_texts(window: &MainWindow, catalogs: &UiCatalogs, locale: Locale) {
    let density_texts = catalogs.density_texts(locale);
    window.set_density_comfortable_label(SharedString::from(density_texts.comfortable));
    window.set_density_comfortable_accessible_label(SharedString::from(
        density_texts.comfortable_accessible,
    ));
    window.set_density_compact_label(SharedString::from(density_texts.compact));
    window
        .set_density_compact_accessible_label(SharedString::from(density_texts.compact_accessible));
}

fn apply_overview_texts(window: &MainWindow, catalogs: &UiCatalogs, locale: Locale) {
    let overview_texts = catalogs.overview_texts(locale);
    window.set_overview_live_updates_label(SharedString::from(overview_texts.live_updates));
    window.set_overview_total_cpu_label(SharedString::from(overview_texts.total_cpu));
    window.set_overview_memory_label(SharedString::from(overview_texts.memory));
    window.set_overview_swap_label(SharedString::from(overview_texts.swap));
    window.set_overview_processes_label(SharedString::from(overview_texts.processes));
    window.set_overview_battery_label(SharedString::from(overview_texts.battery));
    window.set_overview_cpu_usage_label(SharedString::from(overview_texts.cpu_usage));
    window.set_overview_memory_usage_label(SharedString::from(overview_texts.memory_usage));
}

fn apply_performance_texts(window: &MainWindow, catalogs: &UiCatalogs, locale: Locale) {
    let performance_texts = catalogs.performance_texts(locale);
    window.set_performance_one_minute_label(SharedString::from(performance_texts.one_minute));
    window.set_performance_five_minutes_label(SharedString::from(performance_texts.five_minutes));
    window.set_performance_cpu_total_label(SharedString::from(performance_texts.total_cpu));
    window.set_performance_memory_used_label(SharedString::from(performance_texts.memory_used));
    window.set_performance_cpu_per_core_label(SharedString::from(performance_texts.cpu_per_core));
    window.set_performance_cpu_unavailable_label(SharedString::from(
        performance_texts.cpu_unavailable,
    ));
    window.set_performance_gpu_vram_label(SharedString::from(performance_texts.gpu_vram));
}

fn apply_process_texts(window: &MainWindow, catalogs: &UiCatalogs, locale: Locale) {
    let process_texts = catalogs.process_texts(locale);
    window.set_process_list_title_label(SharedString::from(process_texts.list_title));
    window.set_process_filter_placeholder(SharedString::from(process_texts.filter_placeholder));
    window.set_process_filter_accessible_label(SharedString::from(
        process_texts.filter_accessible_label,
    ));
    window.set_process_column_label(SharedString::from(process_texts.column_process));
    window.set_process_empty_filtered_label(SharedString::from(process_texts.empty_filtered));
    window
        .set_process_waiting_connection_label(SharedString::from(process_texts.waiting_connection));
}

#[cfg(test)]
mod tests {
    use super::{
        ByteUnitSystem, CountKind, FIVE_MINUTES_MS, GpuRow, Locale, MAX_CHART_POINTS, MainPage,
        MainWindow, NetworkInterfaceRow, ONE_MINUTE_MS, ProcessRow, ProcessSort, SortKey,
        UiByteUnitSystem, UiCatalogs, UiDensity, UiLanguage, UiSettings,
        apply_byte_unit_system_selection, apply_density_texts, apply_header_texts,
        apply_navigation_texts, apply_overview_texts, apply_performance_texts, apply_process_texts,
        chart_accessible_summary, compare_optional_cpu, downsample_max, find_selected_process,
        format_bytes, format_bytes_per_second, format_gpu_clocks, format_latency_ms,
        format_operations_per_second, format_process_memory, gpu_status_label, lifecycle_status,
        localize_history_label, localized_sample_age, memory_percentage, memory_status_description,
        memory_summary, metric_status_label, network_interface_kind_from_wire,
        network_interface_kind_label, process_identity_key, process_row, process_start_age_label,
        process_tree_rows, sample_age_label, select_history, sort_processes, swap_summary,
        unavailable_status,
    };
    use native_monitor_agent_lifecycle::StartupMode;
    use native_monitor_domain::NetworkInterfaceKind;
    use native_monitor_ipc::wire::{
        NetworkInterfaceKind as WireNetworkInterfaceKind, ProcessMetrics,
    };
    use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static UI_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn downsamples_long_series_and_keeps_bucket_peaks() {
        let values = vec![1.0, 2.0, 90.0, 4.0, 5.0, 6.0, 7.0, 8.0];

        let result = downsample_max(&values, 4);

        assert_eq!(result.len(), 4);
        assert_eq!(result[1], 90.0);
    }

    #[test]
    fn five_minute_chart_history_handles_1200_samples_with_bounded_peak_preserving_output() {
        let mut values = (0..1_200)
            .map(|index| (index % 17) as f32)
            .collect::<Vec<_>>();
        values[18] = 1_000.0;
        values[500] = 2_000.0;
        values[1_199] = 3_000.0;

        let (history, range_label) =
            select_history(&values, 250, FIVE_MINUTES_MS, MAX_CHART_POINTS);

        assert_eq!(range_label, "−5 min");
        assert!(history.len() <= MAX_CHART_POINTS);
        assert_eq!(history[1], 1_000.0);
        assert_eq!(history[29], 2_000.0);
        assert_eq!(history[70], 3_000.0);
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
    fn negative_history_labels_are_localized_without_panicking() {
        assert_eq!(
            localize_history_label("−1 min".to_owned(), Locale::EnUs),
            "1 min ago"
        );
        assert_eq!(
            localize_history_label("−5 min".to_owned(), Locale::EnUs),
            "5 min ago"
        );
    }

    #[test]
    fn empty_history_label_is_localized_for_english() {
        let (history, label) = select_history(&[], 1_000, FIVE_MINUTES_MS, 72);

        assert!(history.is_empty());
        assert_eq!(localize_history_label(label, Locale::EnUs), "no data");
    }

    #[test]
    fn network_interface_kind_falls_back_and_localizes_labels() {
        assert_eq!(
            network_interface_kind_from_wire(WireNetworkInterfaceKind::Loopback as i32, 10),
            NetworkInterfaceKind::Loopback
        );
        assert_eq!(
            network_interface_kind_from_wire(WireNetworkInterfaceKind::Unknown as i32, 10),
            NetworkInterfaceKind::Unknown
        );
        assert_eq!(
            network_interface_kind_from_wire(WireNetworkInterfaceKind::Loopback as i32, 9),
            NetworkInterfaceKind::Unknown
        );
        assert_eq!(
            network_interface_kind_from_wire(99, 10),
            NetworkInterfaceKind::Unknown
        );
        assert_eq!(
            network_interface_kind_label(NetworkInterfaceKind::Loopback, Locale::PtBr),
            "Loopback"
        );
        assert_eq!(
            network_interface_kind_label(NetworkInterfaceKind::Unknown, Locale::PtBr),
            "Desconhecido"
        );
        assert_eq!(
            network_interface_kind_label(NetworkInterfaceKind::Unknown, Locale::EnUs),
            "Unknown"
        );
    }

    #[test]
    fn gpu_status_remains_visible_when_only_some_device_metrics_are_available() {
        assert_eq!(
            gpu_status_label(7, 1, "temporarily_unavailable", Locale::EnUs),
            "Temporarily unavailable"
        );
        assert_eq!(gpu_status_label(7, 1, "ok", Locale::EnUs), "");
        assert_eq!(
            gpu_status_label(7, 0, "unsupported", Locale::PtBr),
            "Não suportado"
        );
        assert_eq!(
            gpu_status_label(6, 1, "ok", Locale::EnUs),
            "Agent does not support GPU metrics in this version"
        );
    }

    #[test]
    fn locale_grouping_preserves_decimal_precision_rounding_and_signs() {
        assert_eq!(
            Locale::PtBr.format_integer(1_234_567_890_123_456_789),
            "1.234.567.890.123.456.789"
        );
        assert_eq!(Locale::PtBr.format_integer(0), "0");
        assert_eq!(Locale::EnUs.format_integer(1_234_567), "1,234,567");
        assert_eq!(Locale::PtBr.format_decimal(1_234.567, 2), "1.234,57");
        assert_eq!(Locale::EnUs.format_decimal(1_234.567, 2), "1,234.57");
        assert_eq!(Locale::PtBr.format_decimal(-12_345.678, 2), "-12.345,68");
        assert_eq!(Locale::EnUs.format_decimal(999.999, 2), "1,000.00");
        assert_eq!(Locale::PtBr.format_decimal(999.994, 2), "999,99");
    }

    #[test]
    fn gpu_clock_quantities_are_grouped_without_filling_missing_values() {
        assert_eq!(
            format_gpu_clocks(Some(2_450), Some(1_000), Locale::PtBr),
            "2.450 / 1.000 MHz"
        );
        assert_eq!(
            format_gpu_clocks(Some(2_450), Some(1_000), Locale::EnUs),
            "2,450 / 1,000 MHz"
        );
        assert_eq!(
            format_gpu_clocks(Some(2_450), None, Locale::PtBr),
            "2.450 / — MHz"
        );
    }

    #[test]
    fn chart_accessible_summary_reports_range_latest_and_ignores_non_finite_samples() {
        let samples = [10.0, f32::NAN, 35.5, 20.0, f32::INFINITY];

        assert_eq!(
            chart_accessible_summary("CPU", &samples, Locale::EnUs),
            "CPU chart, 3 samples, minimum 10.0%, maximum 35.5%, latest 20.0%"
        );
        assert_eq!(
            chart_accessible_summary("CPU", &[], Locale::PtBr),
            "Gráfico de CPU, aguardando amostras"
        );
        assert_eq!(
            chart_accessible_summary("memory", &[20.0, 40.0], Locale::PtBr),
            "Gráfico de memória, 2 amostras, mínimo 20,0%, máximo 40,0%, atual 40,0%"
        );
        let many_samples = vec![25.0; 1_234];
        assert!(
            chart_accessible_summary("CPU", &many_samples, Locale::PtBr).contains("1.234 amostras")
        );
        assert!(
            chart_accessible_summary("CPU", &many_samples, Locale::EnUs).contains("1,234 samples")
        );
    }

    #[test]
    fn memory_summary_does_not_invent_missing_values() {
        assert_eq!(
            memory_summary(
                None,
                Some(8 * 1024 * 1024 * 1024),
                Locale::PtBr,
                ByteUnitSystem::Iec,
            ),
            ("—".to_owned(), "Indisponível nesta amostra".to_owned())
        );
        assert_eq!(
            memory_summary(
                Some(4 * 1024 * 1024 * 1024),
                Some(8 * 1024 * 1024 * 1024),
                Locale::PtBr,
                ByteUnitSystem::Iec,
            ),
            ("4,0 / 8,0 GiB".to_owned(), "50,0% utilizado".to_owned())
        );
        assert_eq!(
            memory_summary(
                Some(4 * 1024 * 1024 * 1024),
                Some(8 * 1024 * 1024 * 1024),
                Locale::EnUs,
                ByteUnitSystem::Iec,
            )
            .0,
            "4.0 / 8.0 GiB"
        );
        assert_eq!(
            memory_summary(
                Some(4 * 1024 * 1024 * 1024),
                Some(8 * 1024 * 1024 * 1024),
                Locale::PtBr,
                ByteUnitSystem::Si,
            ),
            ("4,3 / 8,6 GB".to_owned(), "50,0% utilizado".to_owned())
        );
    }

    #[test]
    fn memory_status_description_localizes_summaries_and_preserves_missing_states() {
        assert_eq!(
            memory_status_description(
                "4.0 / 8.0 GiB",
                "50.0% utilizado",
                "Available",
                true,
                Locale::EnUs,
            ),
            "50.0% used · Available"
        );
        assert_eq!(
            memory_status_description(
                "4,0 / 8,0 GiB",
                "50,0% utilizado",
                "Disponível",
                true,
                Locale::PtBr,
            ),
            "50,0% utilizado · Disponível"
        );
        assert_eq!(
            memory_status_description(
                "—",
                "Indisponível nesta amostra",
                "Unavailable",
                false,
                Locale::EnUs,
            ),
            "Unavailable"
        );
        assert_eq!(
            memory_status_description(
                "—",
                "Indisponível nesta amostra",
                "Available",
                true,
                Locale::EnUs,
            ),
            "Unavailable in this sample"
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
        assert_eq!(format_bytes(None, Locale::PtBr, ByteUnitSystem::Iec), "—");
        assert_eq!(
            format_bytes(Some(0), Locale::PtBr, ByteUnitSystem::Iec),
            "0 B"
        );
        assert_eq!(
            format_bytes(Some(1_536), Locale::PtBr, ByteUnitSystem::Iec),
            "1,5 KiB"
        );
        assert_eq!(
            format_bytes(Some(1_572_864), Locale::PtBr, ByteUnitSystem::Iec),
            "1,5 MiB"
        );
        assert_eq!(
            format_bytes(Some(1_610_612_736), Locale::PtBr, ByteUnitSystem::Iec),
            "1,5 GiB"
        );
        assert_eq!(
            format_bytes(Some(1_536), Locale::EnUs, ByteUnitSystem::Iec),
            "1.5 KiB"
        );
        assert_eq!(
            format_bytes(Some(1_500), Locale::PtBr, ByteUnitSystem::Si),
            "1,5 kB"
        );
        assert_eq!(
            format_bytes(Some(1_500_000), Locale::PtBr, ByteUnitSystem::Si),
            "1,5 MB"
        );
        assert_eq!(
            format_bytes(Some(1_500_000_000), Locale::PtBr, ByteUnitSystem::Si),
            "1,5 GB"
        );
        assert_eq!(
            format_bytes(Some(1_500_000), Locale::EnUs, ByteUnitSystem::Si),
            "1.5 MB"
        );
        assert_eq!(
            format_bytes_per_second(None, Locale::PtBr, ByteUnitSystem::Iec),
            "—"
        );
        assert_eq!(
            format_bytes_per_second(Some(1_536.0), Locale::PtBr, ByteUnitSystem::Iec),
            "1,5 KiB/s"
        );
        assert_eq!(
            format_bytes_per_second(Some(1_536.0), Locale::EnUs, ByteUnitSystem::Iec),
            "1.5 KiB/s"
        );
        assert_eq!(
            format_bytes_per_second(Some(1_500.0), Locale::PtBr, ByteUnitSystem::Si),
            "1,5 kB/s"
        );
        assert_eq!(
            format_bytes_per_second(Some(f64::NAN), Locale::PtBr, ByteUnitSystem::Iec),
            "—"
        );
        assert_eq!(
            format_operations_per_second(Some(2.75), Locale::PtBr),
            "2,8 op/s"
        );
        assert_eq!(
            format_operations_per_second(Some(2.75), Locale::EnUs),
            "2.8 op/s"
        );
        assert_eq!(format_latency_ms(Some(1.236), Locale::PtBr), "1,24");
        assert_eq!(format_latency_ms(Some(1.236), Locale::EnUs), "1.24");
        assert_eq!(
            format_process_memory(4 * 1024 * 1024, Locale::PtBr, ByteUnitSystem::Iec),
            ("4,0".to_owned(), "MiB".to_owned())
        );
        assert_eq!(
            format_process_memory(4 * 1024 * 1024, Locale::PtBr, ByteUnitSystem::Si),
            ("4,2".to_owned(), "MB".to_owned())
        );
    }

    #[test]
    fn swap_summary_distinguishes_missing_from_unconfigured_swap() {
        assert_eq!(
            swap_summary(Some(0), Some(0), Locale::PtBr, ByteUnitSystem::Iec),
            ("Sem swap".to_owned(), "Nenhuma área configurada".to_owned())
        );
        assert_eq!(
            swap_summary(None, None, Locale::PtBr, ByteUnitSystem::Iec),
            ("—".to_owned(), "Indisponível nesta amostra".to_owned())
        );
        assert_eq!(
            swap_summary(
                Some(1_073_741_824),
                Some(2_147_483_648),
                Locale::PtBr,
                ByteUnitSystem::Iec,
            ),
            (
                "1,0 / 2,0 GiB".to_owned(),
                "50,0% utilizado · disponível".to_owned()
            )
        );
        assert_eq!(
            swap_summary(
                Some(1_500_000_000),
                Some(3_000_000_000),
                Locale::EnUs,
                ByteUnitSystem::Si,
            ),
            (
                "1.5 / 3.0 GB".to_owned(),
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
    fn selecting_byte_units_updates_the_worker_state_and_persists_the_choice() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let test_directory = std::env::temp_dir().join(format!(
            "native-monitor-byte-unit-selection-{}-{unique}",
            std::process::id()
        ));
        let settings_path = test_directory.join("ui-settings.toml");
        let state = AtomicU8::new(ByteUnitSystem::Iec.code());

        assert_eq!(
            apply_byte_unit_system_selection("si", &state, Some(&settings_path))
                .expect("persist SI unit choice"),
            Some(ByteUnitSystem::Si)
        );
        assert_eq!(
            ByteUnitSystem::from_code(state.load(Ordering::Acquire)),
            ByteUnitSystem::Si
        );
        assert_eq!(
            UiSettings::load(&settings_path)
                .expect("reload persisted unit choice")
                .byte_unit_system,
            ByteUnitSystem::Si
        );
        assert_eq!(
            apply_byte_unit_system_selection("unknown", &state, Some(&settings_path))
                .expect("ignore unknown unit choice"),
            None
        );
        assert_eq!(
            ByteUnitSystem::from_code(state.load(Ordering::Acquire)),
            ByteUnitSystem::Si
        );

        std::fs::remove_dir_all(&test_directory).expect("remove test preferences directory");
    }

    #[test]
    fn process_row_formats_cpu_and_memory_decimals_for_the_selected_locale() {
        let sample = process(
            1_234_567,
            "worker",
            Some(12.34),
            4 * 1024 * 1024,
            Some(9_876),
        );
        let total_memory = Some(8 * 1024 * 1024);

        let portuguese = process_row(
            &sample,
            total_memory,
            10_000,
            Locale::PtBr,
            ByteUnitSystem::Iec,
        );
        assert_eq!(portuguese.cpu.as_str(), "12,3");
        assert_eq!(portuguese.memory.as_str(), "4,0");
        assert_eq!(portuguese.memory_percent.as_str(), "50,0%");
        assert_eq!(portuguese.pid.as_str(), "1234567");
        assert_eq!(portuguese.parent_pid.as_str(), "9876");
        assert_eq!(portuguese.key.as_str(), "1234567:1");

        let english = process_row(
            &sample,
            total_memory,
            10_000,
            Locale::EnUs,
            ByteUnitSystem::Iec,
        );
        assert_eq!(english.cpu.as_str(), "12.3");
        assert_eq!(english.memory.as_str(), "4.0");
        assert_eq!(english.memory_percent.as_str(), "50.0%");
        assert_eq!(english.pid.as_str(), "1234567");
        assert_eq!(english.parent_pid.as_str(), "9876");

        let si = process_row(
            &sample,
            total_memory,
            10_000,
            Locale::PtBr,
            ByteUnitSystem::Si,
        );
        assert_eq!(si.memory.as_str(), "4,2");
        assert_eq!(si.memory_unit.as_str(), "MB");
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
            let ui_catalogs = Arc::new(UiCatalogs::load().expect("load UI catalogs"));
            apply_header_texts(&window, &ui_catalogs, Locale::PtBr);
            apply_navigation_texts(&window, &ui_catalogs, Locale::PtBr);
            apply_density_texts(&window, &ui_catalogs, Locale::PtBr);
            apply_overview_texts(&window, &ui_catalogs, Locale::PtBr);
            apply_performance_texts(&window, &ui_catalogs, Locale::PtBr);
            apply_process_texts(&window, &ui_catalogs, Locale::PtBr);
            let weak_window = window.as_weak();
            let catalogs_for_locale = Arc::clone(&ui_catalogs);
            window.on_language_selected(move |value| {
                if let Some(locale) = Locale::from_selection(value.as_str())
                    && let Some(window) = weak_window.upgrade()
                {
                    window.set_language(locale.ui_language());
                    apply_header_texts(&window, &catalogs_for_locale, locale);
                    apply_navigation_texts(&window, &catalogs_for_locale, locale);
                    apply_density_texts(&window, &catalogs_for_locale, locale);
                    apply_overview_texts(&window, &catalogs_for_locale, locale);
                    apply_performance_texts(&window, &catalogs_for_locale, locale);
                    apply_process_texts(&window, &catalogs_for_locale, locale);
                }
            });
            let expected_chart_summary =
                "Gráfico de CPU, 2 amostras, mínimo 12,0%, máximo 28,0%, atual 28,0%";
            window.set_cpu_history_accessible_summary(SharedString::from(expected_chart_summary));
            let mut ptbr_swap_panels =
                ElementHandle::find_by_accessible_label(&window, "Memória swap");
            assert!(ptbr_swap_panels.next().is_some());
            let mut charts = ElementHandle::find_by_element_id(&window, "MainWindow::cpu_chart");
            let chart = charts.next().expect("find the CPU history chart");
            assert!(charts.next().is_none());
            assert_eq!(
                chart.accessible_role(),
                Some(i_slint_backend_testing::AccessibleRole::Image)
            );
            assert_eq!(
                chart
                    .accessible_label()
                    .expect("the chart exposes an accessible summary")
                    .as_str(),
                expected_chart_summary
            );

            let mut overview_tabs = ElementHandle::find_by_accessible_label(&window, "Visão geral");
            let overview_tab = overview_tabs.next().expect("find the overview tab");
            assert_eq!(
                overview_tab.accessible_role(),
                Some(i_slint_backend_testing::AccessibleRole::Tab)
            );
            assert_eq!(overview_tab.accessible_item_selected(), Some(true));
            let mut portuguese_navigation =
                ElementHandle::find_by_accessible_label(&window, "Navegação principal");
            assert_eq!(
                portuguese_navigation
                    .next()
                    .expect("find the Portuguese navigation landmark")
                    .accessible_role(),
                Some(i_slint_backend_testing::AccessibleRole::TabList)
            );
            for label in [
                "Visão geral",
                "Processos",
                "Desempenho",
                "Armazenamento",
                "Rede",
            ] {
                let mut navigation_items = ElementHandle::find_by_accessible_label(&window, label);
                assert!(
                    navigation_items.next().is_some(),
                    "missing tab label {label}"
                );
            }
            assert!(
                ElementHandle::find_by_accessible_label(&window, "Visão geral").count() >= 2,
                "the Portuguese overview title must appear in both navigation and page content"
            );
            let mut portuguese_comfortable_density =
                ElementHandle::find_by_element_id(&window, "MainWindow::density_comfortable");
            assert_eq!(
                portuguese_comfortable_density
                    .next()
                    .expect("find comfortable density selector")
                    .accessible_label()
                    .expect("density selector has an accessible label")
                    .as_str(),
                "Densidade confortável"
            );
            let mut portuguese_compact_density =
                ElementHandle::find_by_element_id(&window, "MainWindow::density_compact");
            assert_eq!(
                portuguese_compact_density
                    .next()
                    .expect("find compact density selector")
                    .accessible_label()
                    .expect("density selector has an accessible label")
                    .as_str(),
                "Densidade compacta"
            );
            let mut portuguese_shortcuts = ElementHandle::find_by_accessible_label(
                &window,
                "Atalho: Control mais 1 a 5 troca de página",
            );
            assert!(portuguese_shortcuts.next().is_some());
            for label in [
                "Atualização contínua",
                "CPU total",
                "Memória",
                "Memória swap",
                "Processos",
                "Bateria",
            ] {
                let mut overview_labels = ElementHandle::find_by_accessible_label(&window, label);
                assert!(
                    overview_labels.next().is_some(),
                    "missing Portuguese overview label {label}"
                );
            }

            let mut english_selectors =
                ElementHandle::find_by_element_id(&window, "MainWindow::language_en_us");
            english_selectors
                .next()
                .expect("find English selector")
                .single_click(PointerEventButton::Left)
                .await;
            let mut english_header_titles =
                ElementHandle::find_by_accessible_label(&window, "System Monitor");
            assert!(english_header_titles.next().is_some());
            let mut english_header_actions =
                ElementHandle::find_by_accessible_label(&window, "Retry connection");
            assert!(english_header_actions.next().is_some());
            for label in [
                "Live updates",
                "Total CPU",
                "Memory",
                "Swap",
                "Processes",
                "Battery",
            ] {
                let mut overview_labels = ElementHandle::find_by_accessible_label(&window, label);
                assert!(
                    overview_labels.next().is_some(),
                    "missing English overview label {label}"
                );
            }
            window.set_active_page(MainPage::Performance);
            for label in [
                "1 minute",
                "5 minutes",
                "CPU per core",
                "Per-core data unavailable in this sample.",
                "GPU and VRAM",
            ] {
                let mut performance_labels =
                    ElementHandle::find_by_accessible_label(&window, label);
                assert!(
                    performance_labels.next().is_some(),
                    "missing English performance label {label}"
                );
            }
            window.set_active_page(MainPage::Processes);
            for label in ["Process list", "Filter processes by name or PID", "Process"] {
                let mut process_page_labels =
                    ElementHandle::find_by_accessible_label(&window, label);
                assert!(
                    process_page_labels.next().is_some(),
                    "missing English process-page label {label}"
                );
            }
            let mut english_filter_fields =
                ElementHandle::find_by_accessible_label(&window, "Filter processes by name or PID");
            assert_eq!(
                english_filter_fields
                    .next()
                    .expect("find the English process filter")
                    .accessible_placeholder_text()
                    .expect("process filter has a placeholder")
                    .as_str(),
                "Filter by name or PID"
            );
            window.set_processes(ModelRc::new(VecModel::from(Vec::new())));
            window.set_agent_connected(false);
            let mut english_waiting_messages =
                ElementHandle::find_by_accessible_label(&window, "Waiting for agent connection…");
            assert!(english_waiting_messages.next().is_some());
            window.set_agent_connected(true);
            let mut english_empty_messages =
                ElementHandle::find_by_accessible_label(&window, "No processes match the filter.");
            assert!(english_empty_messages.next().is_some());
            window.set_agent_connected(false);
            let mut english_comfortable_density =
                ElementHandle::find_by_element_id(&window, "MainWindow::density_comfortable");
            assert_eq!(
                english_comfortable_density
                    .next()
                    .expect("find comfortable density selector")
                    .accessible_label()
                    .expect("density selector has an accessible label")
                    .as_str(),
                "Comfortable density"
            );
            let mut english_compact_density =
                ElementHandle::find_by_element_id(&window, "MainWindow::density_compact");
            assert_eq!(
                english_compact_density
                    .next()
                    .expect("find compact density selector")
                    .accessible_label()
                    .expect("density selector has an accessible label")
                    .as_str(),
                "Compact density"
            );
            let mut english_navigation =
                ElementHandle::find_by_accessible_label(&window, "Main navigation");
            assert_eq!(
                english_navigation
                    .next()
                    .expect("find the English navigation landmark")
                    .accessible_role(),
                Some(i_slint_backend_testing::AccessibleRole::TabList)
            );
            for label in ["Overview", "Processes", "Performance", "Storage", "Network"] {
                let mut navigation_items = ElementHandle::find_by_accessible_label(&window, label);
                assert!(
                    navigation_items.next().is_some(),
                    "missing tab label {label}"
                );
            }
            for (page, label) in [
                (MainPage::Overview, "Overview"),
                (MainPage::Processes, "Processes"),
                (MainPage::Performance, "Performance"),
                (MainPage::Storage, "Storage"),
                (MainPage::Network, "Network"),
            ] {
                window.set_active_page(page);
                assert!(
                    ElementHandle::find_by_accessible_label(&window, label).count() >= 2,
                    "the English {label} title must appear in navigation and page content"
                );
            }
            window.set_active_page(MainPage::Overview);
            let mut english_shortcuts = ElementHandle::find_by_accessible_label(
                &window,
                "Keyboard shortcut: Control plus 1 through 5 switches pages",
            );
            assert!(english_shortcuts.next().is_some());
            let mut english_swap_panels = ElementHandle::find_by_accessible_label(&window, "Swap");
            assert!(english_swap_panels.next().is_some());

            let mut portuguese_selectors =
                ElementHandle::find_by_element_id(&window, "MainWindow::language_pt_br");
            portuguese_selectors
                .next()
                .expect("find Portuguese selector")
                .single_click(PointerEventButton::Left)
                .await;
            let mut portuguese_header_titles =
                ElementHandle::find_by_accessible_label(&window, "Monitor do Sistema");
            assert!(portuguese_header_titles.next().is_some());
            let mut portuguese_header_actions =
                ElementHandle::find_by_accessible_label(&window, "Tentar conexão");
            assert!(portuguese_header_actions.next().is_some());
            window.set_active_page(MainPage::Performance);
            for label in [
                "1 minuto",
                "5 minutos",
                "CPU por núcleo",
                "Dados por núcleo indisponíveis nesta amostra.",
                "GPU e VRAM",
            ] {
                let mut performance_labels =
                    ElementHandle::find_by_accessible_label(&window, label);
                assert!(
                    performance_labels.next().is_some(),
                    "missing Portuguese performance label {label}"
                );
            }
            window.set_active_page(MainPage::Processes);
            for label in [
                "Lista de processos",
                "Filtrar processos por nome ou PID",
                "Processo",
            ] {
                let mut process_page_labels =
                    ElementHandle::find_by_accessible_label(&window, label);
                assert!(
                    process_page_labels.next().is_some(),
                    "missing Portuguese process-page label {label}"
                );
            }
            let mut portuguese_filter_fields = ElementHandle::find_by_accessible_label(
                &window,
                "Filtrar processos por nome ou PID",
            );
            assert_eq!(
                portuguese_filter_fields
                    .next()
                    .expect("find the Portuguese process filter")
                    .accessible_placeholder_text()
                    .expect("process filter has a placeholder")
                    .as_str(),
                "Filtrar por nome ou PID"
            );
            window.set_processes(ModelRc::new(VecModel::from(Vec::new())));
            window.set_agent_connected(false);
            let mut portuguese_waiting_messages = ElementHandle::find_by_accessible_label(
                &window,
                "Aguardando conexão com o agente…",
            );
            assert!(portuguese_waiting_messages.next().is_some());
            window.set_agent_connected(true);
            let mut portuguese_empty_messages = ElementHandle::find_by_accessible_label(
                &window,
                "Nenhum processo corresponde ao filtro.",
            );
            assert!(portuguese_empty_messages.next().is_some());
            window.set_agent_connected(false);
            let mut restored_portuguese_comfortable_density =
                ElementHandle::find_by_element_id(&window, "MainWindow::density_comfortable");
            assert_eq!(
                restored_portuguese_comfortable_density
                    .next()
                    .expect("find comfortable density selector after locale change")
                    .accessible_label()
                    .expect("density selector has an accessible label")
                    .as_str(),
                "Densidade confortável"
            );
            for (page, label) in [
                (MainPage::Overview, "Visão geral"),
                (MainPage::Processes, "Processos"),
                (MainPage::Performance, "Desempenho"),
                (MainPage::Storage, "Armazenamento"),
                (MainPage::Network, "Rede"),
            ] {
                window.set_active_page(page);
                assert!(
                    ElementHandle::find_by_accessible_label(&window, label).count() >= 2,
                    "the Portuguese {label} title must appear in navigation and page content"
                );
            }
            window.set_active_page(MainPage::Overview);
            let mut localized_swap_panels =
                ElementHandle::find_by_accessible_label(&window, "Memória swap");
            assert!(localized_swap_panels.next().is_some());
            window.window().set_size(slint::PhysicalSize::new(950, 720));
            let mut narrow_swap_panels =
                ElementHandle::find_by_accessible_label(&window, "Memória swap");
            assert!(narrow_swap_panels.next().is_some());
            window
                .window()
                .set_size(slint::PhysicalSize::new(1_120, 720));
            let mut wide_swap_panels =
                ElementHandle::find_by_accessible_label(&window, "Memória swap");
            assert!(wide_swap_panels.next().is_some());

            window.set_active_page(MainPage::Storage);
            window.set_volume_count(SharedString::from(ui_catalogs.format_count(
                CountKind::Volumes,
                1,
                Locale::PtBr,
            )));
            let mut volume_counts = ElementHandle::find_by_accessible_label(&window, "1 volume");
            assert!(volume_counts.next().is_some());
            assert_eq!(window.get_byte_unit_system(), UiByteUnitSystem::Iec);
            let mut si_choices =
                ElementHandle::find_by_element_id(&window, "MainWindow::byte_units_si");
            let si_choice = si_choices.next().expect("find the SI unit selector");
            assert_eq!(
                si_choice.accessible_role(),
                Some(i_slint_backend_testing::AccessibleRole::RadioButton)
            );
            assert_eq!(
                si_choice
                    .accessible_label()
                    .expect("SI selector has a localized accessible label")
                    .as_str(),
                "Unidades decimais SI"
            );
            assert_eq!(si_choice.accessible_checked(), Some(false));
            let selected_byte_units = Rc::new(RefCell::new(Vec::<String>::new()));
            let unit_state = Arc::new(AtomicU8::new(ByteUnitSystem::Iec.code()));
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is after the Unix epoch")
                .as_nanos();
            let settings_directory = std::env::temp_dir().join(format!(
                "native-monitor-unit-ui-test-{}-{unique}",
                std::process::id()
            ));
            let settings_path = settings_directory.join("ui-settings.toml");
            window.on_byte_unit_system_selected({
                let selected_byte_units = Rc::clone(&selected_byte_units);
                let unit_state = Arc::clone(&unit_state);
                let settings_path = settings_path.clone();
                move |selection| {
                    let selected = apply_byte_unit_system_selection(
                        selection.as_str(),
                        &unit_state,
                        Some(&settings_path),
                    )
                    .expect("apply and persist UI byte unit selection");
                    assert!(selected.is_some());
                    selected_byte_units.borrow_mut().push(selection.to_string());
                }
            });
            si_choice.single_click(PointerEventButton::Left).await;
            assert_eq!(window.get_byte_unit_system(), UiByteUnitSystem::Si);
            assert_eq!(si_choice.accessible_checked(), Some(true));
            assert_eq!(selected_byte_units.borrow().as_slice(), ["si"]);
            assert_eq!(
                UiSettings::load(&settings_path)
                    .expect("reload SI preference selected in the UI")
                    .byte_unit_system,
                ByteUnitSystem::Si
            );

            window.set_active_page(MainPage::Processes);
            window.set_process_count_description(SharedString::from(ui_catalogs.format_count(
                CountKind::Processes,
                1,
                Locale::PtBr,
            )));
            window.set_process_count_detail(SharedString::from(
                ui_catalogs.process_detail(1, Locale::PtBr),
            ));
            let mut process_counts =
                ElementHandle::find_by_accessible_label(&window, "1 processo visível");
            assert!(process_counts.next().is_some());
            window.set_processes(ModelRc::new(VecModel::from(vec![ProcessRow {
                key: SharedString::from("42:1700000000"),
                name: SharedString::from("fixture-worker"),
                pid: SharedString::from("42"),
                parent_pid: SharedString::from("1"),
                started: SharedString::from("há 1 min"),
                cpu: SharedString::from("1,0%"),
                memory: SharedString::from("4,2"),
                memory_unit: SharedString::from("MB"),
                memory_percent: SharedString::from("50,0%"),
            }])));
            let mut si_process_rows = ElementHandle::find_by_accessible_label(
                &window,
                "fixture-worker, PID 42, CPU 1,0%, memória 4,2 MB",
            );
            assert!(si_process_rows.next().is_some());

            window.set_active_page(MainPage::Storage);

            let mut iec_choices =
                ElementHandle::find_by_element_id(&window, "MainWindow::byte_units_iec");
            let iec_choice = iec_choices.next().expect("find the IEC unit selector");
            assert_eq!(
                iec_choice
                    .accessible_label()
                    .expect("IEC selector has a localized accessible label")
                    .as_str(),
                "Unidades binárias IEC"
            );
            assert_eq!(iec_choice.accessible_checked(), Some(false));
            iec_choice.single_click(PointerEventButton::Left).await;
            assert_eq!(window.get_byte_unit_system(), UiByteUnitSystem::Iec);
            assert_eq!(iec_choice.accessible_checked(), Some(true));
            let mut refreshed_si_choices =
                ElementHandle::find_by_element_id(&window, "MainWindow::byte_units_si");
            assert_eq!(
                refreshed_si_choices
                    .next()
                    .expect("find SI selector after switching back")
                    .accessible_checked(),
                Some(false)
            );
            assert_eq!(selected_byte_units.borrow().as_slice(), ["si", "iec"]);
            assert_eq!(
                UiSettings::load(&settings_path)
                    .expect("reload IEC preference selected in the UI")
                    .byte_unit_system,
                ByteUnitSystem::Iec
            );

            window.set_active_page(MainPage::Network);
            window.set_network_interface_count(SharedString::from(ui_catalogs.format_count(
                CountKind::NetworkInterfaces,
                1,
                Locale::PtBr,
            )));
            let mut interface_counts =
                ElementHandle::find_by_accessible_label(&window, "1 interface");
            assert!(interface_counts.next().is_some());

            window.set_active_page(MainPage::Processes);
            let mut process_tabs = ElementHandle::find_by_accessible_label(&window, "Processos");
            let process_tab = process_tabs.next().expect("find the process tab");
            assert_eq!(process_tab.accessible_item_selected(), Some(true));
            assert_eq!(overview_tab.accessible_item_selected(), Some(false));

            let mut filter_fields = ElementHandle::find_by_accessible_label(
                &window,
                "Filtrar processos por nome ou PID",
            );
            let filter_field = filter_fields.next().expect("find the process filter");
            assert_eq!(
                filter_field.accessible_role(),
                Some(i_slint_backend_testing::AccessibleRole::TextInput)
            );

            let mut process_headers = ElementHandle::find_by_accessible_label(&window, "Processo");
            let process_header = process_headers
                .next()
                .expect("find the process sort header");
            assert_eq!(
                process_header.accessible_role(),
                Some(i_slint_backend_testing::AccessibleRole::Button)
            );
            assert_eq!(
                process_header
                    .accessible_description()
                    .expect("the process sort header describes its sort state")
                    .as_str(),
                "Não ordenado por esta coluna"
            );

            window.set_processes(ModelRc::new(VecModel::from(vec![ProcessRow {
                key: SharedString::from("42:1700000000"),
                name: SharedString::from("fixture-worker"),
                pid: SharedString::from("42"),
                parent_pid: SharedString::from("1"),
                started: SharedString::from("há 1 min"),
                cpu: SharedString::from("1,0%"),
                memory: SharedString::from("4,0"),
                memory_unit: SharedString::from("MiB"),
                memory_percent: SharedString::from("50,0%"),
            }])));

            let mut process_rows = ElementHandle::find_by_accessible_label(
                &window,
                "fixture-worker, PID 42, CPU 1,0%, memória 4,0 MiB",
            );
            let process_row = process_rows
                .next()
                .expect("find the accessible process row");
            assert_eq!(
                process_row.accessible_role(),
                Some(i_slint_backend_testing::AccessibleRole::ListItem)
            );
            assert_eq!(process_row.accessible_item_selectable(), Some(true));
            assert_eq!(process_row.accessible_item_selected(), Some(false));

            let selected_key = Rc::new(RefCell::new(None::<String>));
            window.on_process_selected({
                let selected_key = Rc::clone(&selected_key);
                move |key| {
                    *selected_key.borrow_mut() = Some(key.to_string());
                }
            });

            let mut targets =
                ElementHandle::find_by_element_id(&window, "MainWindow::process_row_hit_target");
            let target = targets.next().expect("find the process row hit target");
            assert!(targets.next().is_none());

            assert_eq!(window.get_density(), UiDensity::Comfortable);
            let mut compact_choices =
                ElementHandle::find_by_element_id(&window, "MainWindow::density_compact");
            let compact_choice = compact_choices
                .next()
                .expect("find the compact density choice");
            compact_choice.single_click(PointerEventButton::Left).await;
            assert_eq!(window.get_density(), UiDensity::Compact);

            let mut comfortable_choices =
                ElementHandle::find_by_element_id(&window, "MainWindow::density_comfortable");
            let comfortable_choice = comfortable_choices
                .next()
                .expect("find the comfortable density choice");
            comfortable_choice
                .single_click(PointerEventButton::Left)
                .await;
            assert_eq!(window.get_density(), UiDensity::Comfortable);

            target.single_click(PointerEventButton::Left).await;

            assert_eq!(process_row.accessible_item_selected(), Some(true));
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
            assert_eq!(window.get_selected_process_cpu().as_str(), "1,0%");
            assert_eq!(window.get_selected_process_memory().as_str(), "4,0");
            assert_eq!(
                window.get_selected_process_memory_percent().as_str(),
                "50,0%"
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
            window.set_network_interfaces(ModelRc::new(VecModel::from(vec![
                NetworkInterfaceRow {
                    name: SharedString::from("lo"),
                    kind: SharedString::from(network_interface_kind_label(
                        NetworkInterfaceKind::Loopback,
                        Locale::PtBr,
                    )),
                    received: SharedString::from("0 B/s"),
                    transmitted: SharedString::from("0 B/s"),
                },
                NetworkInterfaceRow {
                    name: SharedString::from("enp-test"),
                    kind: SharedString::from(network_interface_kind_label(
                        NetworkInterfaceKind::Unknown,
                        Locale::PtBr,
                    )),
                    received: SharedString::from("—"),
                    transmitted: SharedString::from("—"),
                },
            ])));

            let mut loopback_rows =
                ElementHandle::find_by_accessible_label(&window, "lo, Loopback");
            let loopback_row = loopback_rows
                .next()
                .expect("find the accessible loopback interface row");
            assert_eq!(
                loopback_row.accessible_role(),
                Some(i_slint_backend_testing::AccessibleRole::ListItem)
            );

            let mut unknown_rows =
                ElementHandle::find_by_accessible_label(&window, "enp-test, Desconhecido");
            assert!(unknown_rows.next().is_some());

            dispatch_shortcut("3");
            assert_eq!(window.get_active_page(), MainPage::Performance);
            window.set_gpu_status(SharedString::from(gpu_status_label(
                7,
                1,
                "temporarily_unavailable",
                Locale::EnUs,
            )));
            window.set_gpus(ModelRc::new(VecModel::from(vec![GpuRow {
                name: SharedString::from("Synthetic GPU"),
                utilization: SharedString::from("25%"),
                memory: SharedString::from("— / —"),
                temperature: SharedString::from("—"),
                clocks: SharedString::from("— / — MHz"),
                power: SharedString::from("—"),
            }])));
            let mut gpu_status_labels =
                ElementHandle::find_by_accessible_label(&window, "Temporarily unavailable");
            assert!(gpu_status_labels.next().is_some());

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

            window.set_processes(ModelRc::new(VecModel::from(vec![ProcessRow {
                key: SharedString::from("42:1700000000"),
                name: SharedString::from("fixture-worker"),
                pid: SharedString::from("42"),
                parent_pid: SharedString::from("1"),
                started: SharedString::from("1 min ago"),
                cpu: SharedString::from("1.0%"),
                memory: SharedString::from("4.0"),
                memory_unit: SharedString::from("MiB"),
                memory_percent: SharedString::from("50.0%"),
            }])));

            let mut english_process_tabs =
                ElementHandle::find_by_accessible_label(&window, "Processes");
            let english_process_tab = english_process_tabs
                .next()
                .expect("find the localized process tab");
            assert_eq!(english_process_tab.accessible_item_selected(), Some(true));

            let mut english_process_rows = ElementHandle::find_by_accessible_label(
                &window,
                "fixture-worker, PID 42, CPU 1.0%, memory 4.0 MiB",
            );
            let english_process_row = english_process_rows
                .next()
                .expect("find the localized accessible process row");
            assert_eq!(
                english_process_row.accessible_role(),
                Some(i_slint_backend_testing::AccessibleRole::ListItem)
            );
            assert_eq!(english_process_row.accessible_item_selected(), Some(true));
            std::fs::remove_dir_all(&settings_directory)
                .expect("remove UI preference integration-test directory");
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
        assert_eq!(
            process_start_age_label(1, 1 + 1_234 * 86_400, Locale::PtBr),
            "há 1.234 d"
        );
        assert_eq!(
            process_start_age_label(1, 1 + 1_234 * 86_400, Locale::EnUs),
            "1,234 days ago"
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
        assert_eq!(
            sample_age_label(0, 10_000, Locale::PtBr),
            "Aguardando primeira amostra"
        );
        assert_eq!(
            sample_age_label(9_500, 10_000, Locale::PtBr),
            "Amostra há menos de 1 s"
        );
        assert_eq!(
            sample_age_label(5_000, 10_000, Locale::PtBr),
            "Amostra há 5 s"
        );
        assert_eq!(
            sample_age_label(11_000, 10_000, Locale::PtBr),
            "Amostra há menos de 1 s"
        );
        assert_eq!(
            sample_age_label(0, 10_000, Locale::EnUs),
            "Aguardando primeira amostra"
        );
        assert_eq!(
            sample_age_label(10_000, 74_050_000, Locale::PtBr),
            "Amostra há 1.234 min"
        );
        assert_eq!(
            localized_sample_age(
                &sample_age_label(10_000, 74_050_000, Locale::EnUs),
                Locale::EnUs,
            ),
            "Sample 1,234 min ago"
        );
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
