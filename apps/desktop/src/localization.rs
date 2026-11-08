use crate::Locale;
use anyhow::{Context, Result, bail};
use native_monitor_agent_lifecycle::StartupMode;
use serde::Deserialize;

#[derive(Clone, Copy, Debug)]
pub enum CountKind {
    Processes,
    Volumes,
    NetworkInterfaces,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CountForms {
    singular: String,
    plural: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HeaderCatalog {
    title: String,
    tagline: String,
    #[serde(rename = "language-pt-br-accessible")]
    language_pt_br_accessible: String,
    #[serde(rename = "language-en-us-accessible")]
    language_en_us_accessible: String,
    #[serde(rename = "language-pt-br-short")]
    language_pt_br_short: String,
    #[serde(rename = "language-en-us-short")]
    language_en_us_short: String,
    #[serde(rename = "agent-active")]
    agent_active: String,
    #[serde(rename = "agent-unavailable")]
    agent_unavailable: String,
    #[serde(rename = "setup-startup")]
    setup_startup: String,
    #[serde(rename = "retry-connection")]
    retry_connection: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectionStatusCatalog {
    #[serde(rename = "agent-disconnected")]
    agent_disconnected: String,
    #[serde(rename = "waiting-for-agent")]
    waiting_for_agent: String,
    #[serde(rename = "gpu-unavailable")]
    gpu_unavailable: String,
    #[serde(rename = "battery-unavailable")]
    battery_unavailable: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentStatusCatalog {
    connecting: String,
    #[serde(rename = "active-with-startup")]
    active_with_startup: String,
    #[serde(rename = "connected-startup-error")]
    connected_startup_error: String,
    #[serde(rename = "connected-collection")]
    connected_collection: String,
    #[serde(rename = "unavailable-startup-error")]
    unavailable_startup_error: String,
    #[serde(rename = "unavailable-startup")]
    unavailable_startup: String,
    #[serde(rename = "unavailable-ipc")]
    unavailable_ipc: String,
    #[serde(rename = "startup-systemd-user")]
    startup_systemd_user: String,
    #[serde(rename = "startup-xdg-autostart")]
    startup_xdg_autostart: String,
    #[serde(rename = "startup-launch-agent")]
    startup_launch_agent: String,
    #[serde(rename = "startup-windows-logon-task")]
    startup_windows_logon_task: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NavigationCatalog {
    section: String,
    #[serde(rename = "accessible-label")]
    accessible_label: String,
    overview: String,
    processes: String,
    performance: String,
    storage: String,
    network: String,
    #[serde(rename = "collection-continues")]
    collection_continues: String,
    #[serde(rename = "keyboard-shortcut")]
    keyboard_shortcut: String,
    #[serde(rename = "keyboard-shortcut-accessible")]
    keyboard_shortcut_accessible: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DensityCatalog {
    comfortable: String,
    #[serde(rename = "comfortable-accessible")]
    comfortable_accessible: String,
    compact: String,
    #[serde(rename = "compact-accessible")]
    compact_accessible: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OverviewCatalog {
    #[serde(rename = "live-updates")]
    live_updates: String,
    #[serde(rename = "total-cpu")]
    total_cpu: String,
    memory: String,
    swap: String,
    processes: String,
    battery: String,
    #[serde(rename = "cpu-usage")]
    cpu_usage: String,
    #[serde(rename = "memory-usage")]
    memory_usage: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PerformanceCatalog {
    #[serde(rename = "one-minute")]
    one_minute: String,
    #[serde(rename = "five-minutes")]
    five_minutes: String,
    #[serde(rename = "total-cpu")]
    total_cpu: String,
    #[serde(rename = "memory-used")]
    memory_used: String,
    #[serde(rename = "cpu-per-core")]
    cpu_per_core: String,
    #[serde(rename = "cpu-unavailable")]
    cpu_unavailable: String,
    #[serde(rename = "gpu-vram")]
    gpu_vram: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetricStatusCatalog {
    available: String,
    #[serde(rename = "warming-up")]
    warming_up: String,
    unsupported: String,
    #[serde(rename = "temporarily-unavailable")]
    temporarily_unavailable: String,
    error: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TelemetryStatusCatalog {
    unavailable: String,
    #[serde(rename = "unavailable-sample")]
    unavailable_sample: String,
    #[serde(rename = "core-name")]
    core_name: String,
    #[serde(rename = "unknown-state")]
    unknown_state: String,
    #[serde(rename = "power-unavailable")]
    power_unavailable: String,
    #[serde(rename = "memory-used-percent")]
    memory_used_percent: String,
    #[serde(rename = "swap-none")]
    swap_none: String,
    #[serde(rename = "swap-unconfigured")]
    swap_unconfigured: String,
    #[serde(rename = "swap-available")]
    swap_available: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapabilityStatusCatalog {
    unsupported: String,
    #[serde(rename = "gpu-metrics")]
    gpu_metrics: String,
    #[serde(rename = "battery-metrics")]
    battery_metrics: String,
    #[serde(rename = "mounted-volumes")]
    mounted_volumes: String,
    #[serde(rename = "per-volume-io-rates")]
    per_volume_io_rates: String,
    #[serde(rename = "network-interfaces")]
    network_interfaces: String,
    #[serde(rename = "block-device-io")]
    block_device_io: String,
    #[serde(rename = "no-mounted-volumes")]
    no_mounted_volumes: String,
    #[serde(rename = "no-network-interfaces")]
    no_network_interfaces: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SampleAgeCatalog {
    #[serde(rename = "waiting-first-sample")]
    waiting_first_sample: String,
    #[serde(rename = "less-than-second")]
    less_than_second: String,
    #[serde(rename = "seconds-ago")]
    seconds_ago: String,
    #[serde(rename = "minutes-ago")]
    minutes_ago: String,
    #[serde(rename = "no-recent-sample")]
    no_recent_sample: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcessAgeCatalog {
    #[serde(rename = "seconds-singular")]
    seconds_singular: String,
    #[serde(rename = "seconds-plural")]
    seconds_plural: String,
    minutes: String,
    #[serde(rename = "hours-singular")]
    hours_singular: String,
    #[serde(rename = "hours-plural")]
    hours_plural: String,
    #[serde(rename = "days-singular")]
    days_singular: String,
    #[serde(rename = "days-plural")]
    days_plural: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChartsCatalog {
    now: String,
    #[serde(rename = "waiting-for-samples")]
    waiting_for_samples: String,
    history: String,
    start: String,
    #[serde(rename = "no-data")]
    no_data: String,
    #[serde(rename = "minutes-ago")]
    minutes_ago: String,
    #[serde(rename = "cpu-title")]
    cpu_title: String,
    #[serde(rename = "memory-title")]
    memory_title: String,
    #[serde(rename = "accessible-waiting")]
    accessible_waiting: String,
    #[serde(rename = "accessible-summary")]
    accessible_summary: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StorageCatalog {
    units: String,
    #[serde(rename = "si-accessible")]
    si_accessible: String,
    #[serde(rename = "iec-accessible")]
    iec_accessible: String,
    #[serde(rename = "mount-point")]
    mount_point: String,
    read: String,
    write: String,
    #[serde(rename = "file-system")]
    file_system: String,
    available: String,
    total: String,
    #[serde(rename = "block-activity")]
    block_activity: String,
    device: String,
    #[serde(rename = "read-iops")]
    read_iops: String,
    #[serde(rename = "write-iops")]
    write_iops: String,
    busy: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkCatalog {
    interface: String,
    kind: String,
    #[serde(rename = "kind-loopback")]
    kind_loopback: String,
    #[serde(rename = "kind-unknown")]
    kind_unknown: String,
    received: String,
    sent: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcessCountForms {
    singular: String,
    plural: String,
    #[serde(rename = "detail-singular")]
    detail_singular: String,
    #[serde(rename = "detail-plural")]
    detail_plural: String,
    #[serde(rename = "list-title")]
    list_title: String,
    #[serde(rename = "filter-placeholder")]
    filter_placeholder: String,
    #[serde(rename = "filter-accessible-label")]
    filter_accessible_label: String,
    #[serde(rename = "column-process")]
    column_process: String,
    #[serde(rename = "pid-label")]
    pid_label: String,
    #[serde(rename = "parent-pid-label")]
    parent_pid_label: String,
    #[serde(rename = "cpu-label")]
    cpu_label: String,
    #[serde(rename = "sort-unsorted-description")]
    sort_unsorted_description: String,
    #[serde(rename = "sort-ascending-description")]
    sort_ascending_description: String,
    #[serde(rename = "sort-descending-description")]
    sort_descending_description: String,
    #[serde(rename = "empty-filtered")]
    empty_filtered: String,
    #[serde(rename = "started-label")]
    started_label: String,
    #[serde(rename = "memory-label")]
    memory_label: String,
    #[serde(rename = "memory-accessible-label")]
    memory_accessible_label: String,
    #[serde(rename = "select-prompt")]
    select_prompt: String,
    #[serde(rename = "tree-title")]
    tree_title: String,
    #[serde(rename = "tree-loading")]
    tree_loading: String,
    #[serde(rename = "tree-truncated")]
    tree_truncated: String,
    #[serde(rename = "tree-empty")]
    tree_empty: String,
    #[serde(rename = "tree-process-missing")]
    tree_process_missing: String,
    #[serde(rename = "tree-error")]
    tree_error: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UiCatalog {
    header: HeaderCatalog,
    #[serde(rename = "connection-status")]
    connection_status: ConnectionStatusCatalog,
    #[serde(rename = "agent-status")]
    agent_status: AgentStatusCatalog,
    navigation: NavigationCatalog,
    density: DensityCatalog,
    overview: OverviewCatalog,
    performance: PerformanceCatalog,
    #[serde(rename = "metric-status")]
    metric_status: MetricStatusCatalog,
    #[serde(rename = "telemetry-status")]
    telemetry_status: TelemetryStatusCatalog,
    #[serde(rename = "capability-status")]
    capability_status: CapabilityStatusCatalog,
    #[serde(rename = "sample-age")]
    sample_age: SampleAgeCatalog,
    #[serde(rename = "process-age")]
    process_age: ProcessAgeCatalog,
    charts: ChartsCatalog,
    storage: StorageCatalog,
    network: NetworkCatalog,
    processes: ProcessCountForms,
    volumes: CountForms,
    #[serde(rename = "network-interfaces")]
    network_interfaces: CountForms,
}

impl UiCatalog {
    fn validate(&self, locale_name: &str) -> Result<()> {
        let header_labels = [
            ("header.title", self.header.title.as_str()),
            ("header.tagline", self.header.tagline.as_str()),
            (
                "header.language-pt-br-accessible",
                self.header.language_pt_br_accessible.as_str(),
            ),
            (
                "header.language-en-us-accessible",
                self.header.language_en_us_accessible.as_str(),
            ),
            (
                "header.language-pt-br-short",
                self.header.language_pt_br_short.as_str(),
            ),
            (
                "header.language-en-us-short",
                self.header.language_en_us_short.as_str(),
            ),
            ("header.agent-active", self.header.agent_active.as_str()),
            (
                "header.agent-unavailable",
                self.header.agent_unavailable.as_str(),
            ),
            ("header.setup-startup", self.header.setup_startup.as_str()),
            (
                "header.retry-connection",
                self.header.retry_connection.as_str(),
            ),
            (
                "connection-status.agent-disconnected",
                self.connection_status.agent_disconnected.as_str(),
            ),
            (
                "connection-status.waiting-for-agent",
                self.connection_status.waiting_for_agent.as_str(),
            ),
            (
                "connection-status.gpu-unavailable",
                self.connection_status.gpu_unavailable.as_str(),
            ),
            (
                "connection-status.battery-unavailable",
                self.connection_status.battery_unavailable.as_str(),
            ),
            (
                "agent-status.connecting",
                self.agent_status.connecting.as_str(),
            ),
            (
                "agent-status.startup-systemd-user",
                self.agent_status.startup_systemd_user.as_str(),
            ),
            (
                "agent-status.startup-xdg-autostart",
                self.agent_status.startup_xdg_autostart.as_str(),
            ),
            (
                "agent-status.startup-launch-agent",
                self.agent_status.startup_launch_agent.as_str(),
            ),
            (
                "agent-status.startup-windows-logon-task",
                self.agent_status.startup_windows_logon_task.as_str(),
            ),
            (
                "agent-status.active-with-startup",
                self.agent_status.active_with_startup.as_str(),
            ),
            (
                "agent-status.connected-startup-error",
                self.agent_status.connected_startup_error.as_str(),
            ),
            (
                "agent-status.connected-collection",
                self.agent_status.connected_collection.as_str(),
            ),
            (
                "agent-status.unavailable-startup-error",
                self.agent_status.unavailable_startup_error.as_str(),
            ),
            (
                "agent-status.unavailable-startup",
                self.agent_status.unavailable_startup.as_str(),
            ),
            (
                "agent-status.unavailable-ipc",
                self.agent_status.unavailable_ipc.as_str(),
            ),
            ("navigation.section", self.navigation.section.as_str()),
            (
                "navigation.accessible-label",
                self.navigation.accessible_label.as_str(),
            ),
            ("navigation.overview", self.navigation.overview.as_str()),
            ("navigation.processes", self.navigation.processes.as_str()),
            (
                "navigation.performance",
                self.navigation.performance.as_str(),
            ),
            ("navigation.storage", self.navigation.storage.as_str()),
            ("navigation.network", self.navigation.network.as_str()),
            (
                "navigation.collection-continues",
                self.navigation.collection_continues.as_str(),
            ),
            (
                "navigation.keyboard-shortcut",
                self.navigation.keyboard_shortcut.as_str(),
            ),
            (
                "navigation.keyboard-shortcut-accessible",
                self.navigation.keyboard_shortcut_accessible.as_str(),
            ),
            ("density.comfortable", self.density.comfortable.as_str()),
            (
                "density.comfortable-accessible",
                self.density.comfortable_accessible.as_str(),
            ),
            ("density.compact", self.density.compact.as_str()),
            (
                "density.compact-accessible",
                self.density.compact_accessible.as_str(),
            ),
            ("overview.live-updates", self.overview.live_updates.as_str()),
            ("overview.total-cpu", self.overview.total_cpu.as_str()),
            ("overview.memory", self.overview.memory.as_str()),
            ("overview.swap", self.overview.swap.as_str()),
            ("overview.processes", self.overview.processes.as_str()),
            ("overview.battery", self.overview.battery.as_str()),
            ("overview.cpu-usage", self.overview.cpu_usage.as_str()),
            ("overview.memory-usage", self.overview.memory_usage.as_str()),
            (
                "performance.one-minute",
                self.performance.one_minute.as_str(),
            ),
            (
                "performance.five-minutes",
                self.performance.five_minutes.as_str(),
            ),
            ("performance.total-cpu", self.performance.total_cpu.as_str()),
            (
                "performance.memory-used",
                self.performance.memory_used.as_str(),
            ),
            (
                "performance.cpu-per-core",
                self.performance.cpu_per_core.as_str(),
            ),
            (
                "performance.cpu-unavailable",
                self.performance.cpu_unavailable.as_str(),
            ),
            ("performance.gpu-vram", self.performance.gpu_vram.as_str()),
            (
                "metric-status.available",
                self.metric_status.available.as_str(),
            ),
            (
                "metric-status.warming-up",
                self.metric_status.warming_up.as_str(),
            ),
            (
                "metric-status.unsupported",
                self.metric_status.unsupported.as_str(),
            ),
            (
                "metric-status.temporarily-unavailable",
                self.metric_status.temporarily_unavailable.as_str(),
            ),
            ("metric-status.error", self.metric_status.error.as_str()),
            (
                "telemetry-status.unavailable",
                self.telemetry_status.unavailable.as_str(),
            ),
            (
                "telemetry-status.unavailable-sample",
                self.telemetry_status.unavailable_sample.as_str(),
            ),
            (
                "telemetry-status.core-name",
                self.telemetry_status.core_name.as_str(),
            ),
            (
                "telemetry-status.unknown-state",
                self.telemetry_status.unknown_state.as_str(),
            ),
            (
                "telemetry-status.power-unavailable",
                self.telemetry_status.power_unavailable.as_str(),
            ),
            (
                "telemetry-status.memory-used-percent",
                self.telemetry_status.memory_used_percent.as_str(),
            ),
            (
                "telemetry-status.swap-none",
                self.telemetry_status.swap_none.as_str(),
            ),
            (
                "telemetry-status.swap-unconfigured",
                self.telemetry_status.swap_unconfigured.as_str(),
            ),
            (
                "telemetry-status.swap-available",
                self.telemetry_status.swap_available.as_str(),
            ),
            (
                "capability-status.unsupported",
                self.capability_status.unsupported.as_str(),
            ),
            (
                "capability-status.gpu-metrics",
                self.capability_status.gpu_metrics.as_str(),
            ),
            (
                "capability-status.battery-metrics",
                self.capability_status.battery_metrics.as_str(),
            ),
            (
                "capability-status.mounted-volumes",
                self.capability_status.mounted_volumes.as_str(),
            ),
            (
                "capability-status.per-volume-io-rates",
                self.capability_status.per_volume_io_rates.as_str(),
            ),
            (
                "capability-status.network-interfaces",
                self.capability_status.network_interfaces.as_str(),
            ),
            (
                "capability-status.block-device-io",
                self.capability_status.block_device_io.as_str(),
            ),
            (
                "capability-status.no-mounted-volumes",
                self.capability_status.no_mounted_volumes.as_str(),
            ),
            (
                "capability-status.no-network-interfaces",
                self.capability_status.no_network_interfaces.as_str(),
            ),
            (
                "sample-age.waiting-first-sample",
                self.sample_age.waiting_first_sample.as_str(),
            ),
            (
                "sample-age.less-than-second",
                self.sample_age.less_than_second.as_str(),
            ),
            (
                "sample-age.seconds-ago",
                self.sample_age.seconds_ago.as_str(),
            ),
            (
                "sample-age.minutes-ago",
                self.sample_age.minutes_ago.as_str(),
            ),
            (
                "sample-age.no-recent-sample",
                self.sample_age.no_recent_sample.as_str(),
            ),
            ("charts.now", self.charts.now.as_str()),
            (
                "charts.waiting-for-samples",
                self.charts.waiting_for_samples.as_str(),
            ),
            ("charts.history", self.charts.history.as_str()),
            ("charts.start", self.charts.start.as_str()),
            ("charts.no-data", self.charts.no_data.as_str()),
            ("charts.minutes-ago", self.charts.minutes_ago.as_str()),
            ("charts.cpu-title", self.charts.cpu_title.as_str()),
            ("charts.memory-title", self.charts.memory_title.as_str()),
            (
                "charts.accessible-waiting",
                self.charts.accessible_waiting.as_str(),
            ),
            (
                "charts.accessible-summary",
                self.charts.accessible_summary.as_str(),
            ),
            ("storage.units", self.storage.units.as_str()),
            ("storage.si-accessible", self.storage.si_accessible.as_str()),
            (
                "storage.iec-accessible",
                self.storage.iec_accessible.as_str(),
            ),
            ("storage.mount-point", self.storage.mount_point.as_str()),
            ("storage.read", self.storage.read.as_str()),
            ("storage.write", self.storage.write.as_str()),
            ("storage.file-system", self.storage.file_system.as_str()),
            ("storage.available", self.storage.available.as_str()),
            ("storage.total", self.storage.total.as_str()),
            (
                "storage.block-activity",
                self.storage.block_activity.as_str(),
            ),
            ("storage.device", self.storage.device.as_str()),
            ("storage.read-iops", self.storage.read_iops.as_str()),
            ("storage.write-iops", self.storage.write_iops.as_str()),
            ("storage.busy", self.storage.busy.as_str()),
            ("network.interface", self.network.interface.as_str()),
            ("network.kind", self.network.kind.as_str()),
            ("network.kind-loopback", self.network.kind_loopback.as_str()),
            ("network.kind-unknown", self.network.kind_unknown.as_str()),
            ("network.received", self.network.received.as_str()),
            ("network.sent", self.network.sent.as_str()),
            ("processes.list-title", self.processes.list_title.as_str()),
            (
                "processes.filter-placeholder",
                self.processes.filter_placeholder.as_str(),
            ),
            (
                "processes.filter-accessible-label",
                self.processes.filter_accessible_label.as_str(),
            ),
            (
                "processes.column-process",
                self.processes.column_process.as_str(),
            ),
            ("processes.pid-label", self.processes.pid_label.as_str()),
            (
                "processes.parent-pid-label",
                self.processes.parent_pid_label.as_str(),
            ),
            ("processes.cpu-label", self.processes.cpu_label.as_str()),
            (
                "processes.sort-unsorted-description",
                self.processes.sort_unsorted_description.as_str(),
            ),
            (
                "processes.sort-ascending-description",
                self.processes.sort_ascending_description.as_str(),
            ),
            (
                "processes.sort-descending-description",
                self.processes.sort_descending_description.as_str(),
            ),
            (
                "processes.empty-filtered",
                self.processes.empty_filtered.as_str(),
            ),
            (
                "processes.started-label",
                self.processes.started_label.as_str(),
            ),
            (
                "processes.memory-label",
                self.processes.memory_label.as_str(),
            ),
            (
                "processes.memory-accessible-label",
                self.processes.memory_accessible_label.as_str(),
            ),
            (
                "processes.select-prompt",
                self.processes.select_prompt.as_str(),
            ),
            ("processes.tree-title", self.processes.tree_title.as_str()),
            (
                "processes.tree-loading",
                self.processes.tree_loading.as_str(),
            ),
            (
                "processes.tree-truncated",
                self.processes.tree_truncated.as_str(),
            ),
            ("processes.tree-empty", self.processes.tree_empty.as_str()),
            (
                "processes.tree-process-missing",
                self.processes.tree_process_missing.as_str(),
            ),
            ("processes.tree-error", self.processes.tree_error.as_str()),
        ];

        for (key, label) in header_labels {
            if label.trim().is_empty() {
                bail!("{locale_name} catalog key {key} must not be empty");
            }
        }

        let templates = [
            ("processes.singular", self.processes.singular.as_str()),
            ("processes.plural", self.processes.plural.as_str()),
            ("volumes.singular", self.volumes.singular.as_str()),
            ("volumes.plural", self.volumes.plural.as_str()),
            (
                "network-interfaces.singular",
                self.network_interfaces.singular.as_str(),
            ),
            (
                "network-interfaces.plural",
                self.network_interfaces.plural.as_str(),
            ),
        ];

        for (key, template) in templates {
            if template.matches("{count}").count() != 1 {
                bail!("{locale_name} catalog key {key} must contain exactly one {{count}} token");
            }
        }

        for (key, template, token) in [
            (
                "agent-status.active-with-startup",
                self.agent_status.active_with_startup.as_str(),
                "{collection}",
            ),
            (
                "agent-status.active-with-startup",
                self.agent_status.active_with_startup.as_str(),
                "{startup}",
            ),
            (
                "agent-status.connected-startup-error",
                self.agent_status.connected_startup_error.as_str(),
                "{error}",
            ),
            (
                "agent-status.connected-collection",
                self.agent_status.connected_collection.as_str(),
                "{collection}",
            ),
            (
                "agent-status.unavailable-startup-error",
                self.agent_status.unavailable_startup_error.as_str(),
                "{startup_error}",
            ),
            (
                "agent-status.unavailable-startup-error",
                self.agent_status.unavailable_startup_error.as_str(),
                "{ipc_error}",
            ),
            (
                "agent-status.unavailable-startup",
                self.agent_status.unavailable_startup.as_str(),
                "{ipc_error}",
            ),
            (
                "agent-status.unavailable-startup",
                self.agent_status.unavailable_startup.as_str(),
                "{startup}",
            ),
            (
                "agent-status.unavailable-ipc",
                self.agent_status.unavailable_ipc.as_str(),
                "{ipc_error}",
            ),
        ] {
            if template.matches(token).count() != 1 {
                bail!("{locale_name} catalog key {key} must contain exactly one {token} token");
            }
        }

        if self.charts.minutes_ago.matches("{minutes}").count() != 1 {
            bail!(
                "{locale_name} catalog key charts.minutes-ago must contain exactly one {{minutes}} token"
            );
        }
        for (key, template, token) in [
            (
                "charts.accessible-waiting",
                self.charts.accessible_waiting.as_str(),
                "{title}",
            ),
            (
                "charts.accessible-summary",
                self.charts.accessible_summary.as_str(),
                "{title}",
            ),
            (
                "charts.accessible-summary",
                self.charts.accessible_summary.as_str(),
                "{count}",
            ),
            (
                "charts.accessible-summary",
                self.charts.accessible_summary.as_str(),
                "{minimum}",
            ),
            (
                "charts.accessible-summary",
                self.charts.accessible_summary.as_str(),
                "{maximum}",
            ),
            (
                "charts.accessible-summary",
                self.charts.accessible_summary.as_str(),
                "{latest}",
            ),
        ] {
            if template.matches(token).count() != 1 {
                bail!("{locale_name} catalog key {key} must contain exactly one {token} token");
            }
        }
        if self
            .capability_status
            .unsupported
            .matches("{feature}")
            .count()
            != 1
        {
            bail!(
                "{locale_name} catalog key capability-status.unsupported must contain exactly one {{feature}} token"
            );
        }
        for (key, template) in [
            (
                "sample-age.seconds-ago",
                self.sample_age.seconds_ago.as_str(),
            ),
            (
                "sample-age.minutes-ago",
                self.sample_age.minutes_ago.as_str(),
            ),
        ] {
            if template.matches("{amount}").count() != 1 {
                bail!("{locale_name} catalog key {key} must contain exactly one {{amount}} token");
            }
        }
        for (key, template) in [
            (
                "process-age.seconds-singular",
                self.process_age.seconds_singular.as_str(),
            ),
            (
                "process-age.seconds-plural",
                self.process_age.seconds_plural.as_str(),
            ),
            ("process-age.minutes", self.process_age.minutes.as_str()),
            (
                "process-age.hours-singular",
                self.process_age.hours_singular.as_str(),
            ),
            (
                "process-age.hours-plural",
                self.process_age.hours_plural.as_str(),
            ),
            (
                "process-age.days-singular",
                self.process_age.days_singular.as_str(),
            ),
            (
                "process-age.days-plural",
                self.process_age.days_plural.as_str(),
            ),
        ] {
            if template.matches("{amount}").count() != 1 {
                bail!("{locale_name} catalog key {key} must contain exactly one {{amount}} token");
            }
        }
        if self.telemetry_status.core_name.matches("{number}").count() != 1 {
            bail!(
                "{locale_name} catalog key telemetry-status.core-name must contain exactly one {{number}} token"
            );
        }
        if self
            .telemetry_status
            .memory_used_percent
            .matches("{percentage}")
            .count()
            != 1
        {
            bail!(
                "{locale_name} catalog key telemetry-status.memory-used-percent must contain exactly one {{percentage}} token"
            );
        }

        let details = [
            (
                "processes.detail-singular",
                self.processes.detail_singular.as_str(),
            ),
            (
                "processes.detail-plural",
                self.processes.detail_plural.as_str(),
            ),
        ];

        for (key, detail) in details {
            if detail.contains("{count}") {
                bail!("{locale_name} catalog key {key} must not contain a {{count}} token");
            }
        }

        Ok(())
    }

    fn count_template(&self, kind: CountKind, count: usize) -> &str {
        match kind {
            CountKind::Processes if count == 1 => &self.processes.singular,
            CountKind::Processes => &self.processes.plural,
            CountKind::Volumes if count == 1 => &self.volumes.singular,
            CountKind::Volumes => &self.volumes.plural,
            CountKind::NetworkInterfaces if count == 1 => &self.network_interfaces.singular,
            CountKind::NetworkInterfaces => &self.network_interfaces.plural,
        }
    }
}

#[derive(Debug)]
pub struct UiCatalogs {
    pt_br: UiCatalog,
    en_us: UiCatalog,
}

pub struct HeaderTexts<'a> {
    pub title: &'a str,
    pub tagline: &'a str,
    pub language_pt_br_accessible: &'a str,
    pub language_en_us_accessible: &'a str,
    pub language_pt_br_short: &'a str,
    pub language_en_us_short: &'a str,
    pub agent_active: &'a str,
    pub agent_unavailable: &'a str,
    pub setup_startup: &'a str,
    pub retry_connection: &'a str,
}

pub struct AgentStatusTexts<'a> {
    connecting: &'a str,
    active_with_startup: &'a str,
    connected_startup_error: &'a str,
    connected_collection: &'a str,
    unavailable_startup_error: &'a str,
    unavailable_startup: &'a str,
    unavailable_ipc: &'a str,
    startup_systemd_user: &'a str,
    startup_xdg_autostart: &'a str,
    startup_launch_agent: &'a str,
    startup_windows_logon_task: &'a str,
}

impl AgentStatusTexts<'_> {
    pub fn connecting(&self) -> &str {
        self.connecting
    }

    fn startup_mode_label(&self, mode: StartupMode) -> &str {
        match mode {
            StartupMode::SystemdUser => self.startup_systemd_user,
            StartupMode::XdgAutostart => self.startup_xdg_autostart,
            StartupMode::LaunchAgent => self.startup_launch_agent,
            StartupMode::WindowsLogonTask => self.startup_windows_logon_task,
        }
    }

    pub fn lifecycle(
        &self,
        collection_status: &str,
        startup_mode: Option<StartupMode>,
        startup_error: Option<&str>,
    ) -> String {
        match (startup_mode, startup_error) {
            (Some(mode), None) => self
                .active_with_startup
                .replace("{collection}", collection_status)
                .replace("{startup}", self.startup_mode_label(mode)),
            (_, Some(error)) => self.connected_startup_error.replace("{error}", error),
            (None, None) => self
                .connected_collection
                .replace("{collection}", collection_status),
        }
    }

    pub fn unavailable(
        &self,
        ipc_error: &str,
        startup_mode: Option<StartupMode>,
        startup_error: Option<&str>,
    ) -> String {
        match (startup_mode, startup_error) {
            (_, Some(error)) => self
                .unavailable_startup_error
                .replace("{startup_error}", error)
                .replace("{ipc_error}", ipc_error),
            (Some(mode), None) => self
                .unavailable_startup
                .replace("{ipc_error}", ipc_error)
                .replace("{startup}", self.startup_mode_label(mode)),
            (None, None) => self.unavailable_ipc.replace("{ipc_error}", ipc_error),
        }
    }
}

pub struct NavigationTexts<'a> {
    pub section: &'a str,
    pub accessible_label: &'a str,
    pub overview: &'a str,
    pub processes: &'a str,
    pub performance: &'a str,
    pub storage: &'a str,
    pub network: &'a str,
    pub collection_continues: &'a str,
    pub keyboard_shortcut: &'a str,
    pub keyboard_shortcut_accessible: &'a str,
}

pub struct DensityTexts<'a> {
    pub comfortable: &'a str,
    pub comfortable_accessible: &'a str,
    pub compact: &'a str,
    pub compact_accessible: &'a str,
}

pub struct OverviewTexts<'a> {
    pub live_updates: &'a str,
    pub total_cpu: &'a str,
    pub memory: &'a str,
    pub swap: &'a str,
    pub processes: &'a str,
    pub battery: &'a str,
    pub cpu_usage: &'a str,
    pub memory_usage: &'a str,
}

pub struct PerformanceTexts<'a> {
    pub one_minute: &'a str,
    pub five_minutes: &'a str,
    pub total_cpu: &'a str,
    pub memory_used: &'a str,
    pub cpu_per_core: &'a str,
    pub cpu_unavailable: &'a str,
    pub gpu_vram: &'a str,
}

pub struct MetricStatusTexts<'a> {
    available: &'a str,
    warming_up: &'a str,
    unsupported: &'a str,
    temporarily_unavailable: &'a str,
    error: &'a str,
}

pub struct TelemetryStatusTexts<'a> {
    unavailable: &'a str,
    unavailable_sample: &'a str,
    core_name: &'a str,
    unknown_state: &'a str,
    power_unavailable: &'a str,
    memory_used_percent: &'a str,
    swap_none: &'a str,
    swap_unconfigured: &'a str,
    swap_available: &'a str,
}

impl TelemetryStatusTexts<'_> {
    pub fn unavailable(&self) -> &str {
        self.unavailable
    }

    pub fn unavailable_sample(&self) -> &str {
        self.unavailable_sample
    }

    pub fn core_name(&self, number: usize, locale: Locale) -> String {
        self.core_name
            .replace("{number}", &locale.format_integer(number as u128))
    }

    pub fn unknown_state(&self) -> &str {
        self.unknown_state
    }

    pub fn power_unavailable(&self) -> &str {
        self.power_unavailable
    }

    pub fn memory_used_percent(&self, percentage: f64, locale: Locale) -> String {
        self.memory_used_percent
            .replace("{percentage}", &locale.format_decimal(percentage, 1))
    }

    pub fn swap_none(&self) -> &str {
        self.swap_none
    }

    pub fn swap_unconfigured(&self) -> &str {
        self.swap_unconfigured
    }

    pub fn swap_available(&self) -> &str {
        self.swap_available
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedCapability {
    GpuMetrics,
    BatteryMetrics,
    MountedVolumes,
    PerVolumeIoRates,
    NetworkInterfaces,
    BlockDeviceIo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmptyDeviceCollection {
    MountedVolumes,
    NetworkInterfaces,
}

pub struct CapabilityStatusTexts<'a> {
    unsupported_template: &'a str,
    gpu_metrics: &'a str,
    battery_metrics: &'a str,
    mounted_volumes: &'a str,
    per_volume_io_rates: &'a str,
    network_interfaces: &'a str,
    block_device_io: &'a str,
    no_mounted_volumes: &'a str,
    no_network_interfaces: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SampleAgeLabel {
    WaitingForFirstSample,
    LessThanSecond,
    SecondsAgo(u64),
    MinutesAgo(u64),
    NoRecentSample,
}

pub struct SampleAgeTexts<'a> {
    waiting_first_sample: &'a str,
    less_than_second: &'a str,
    seconds_ago: &'a str,
    minutes_ago: &'a str,
    no_recent_sample: &'a str,
}

impl SampleAgeTexts<'_> {
    pub fn label(&self, age: SampleAgeLabel, locale: Locale) -> String {
        match age {
            SampleAgeLabel::WaitingForFirstSample => self.waiting_first_sample.to_owned(),
            SampleAgeLabel::LessThanSecond => self.less_than_second.to_owned(),
            SampleAgeLabel::SecondsAgo(amount) => self
                .seconds_ago
                .replace("{amount}", &locale.format_integer(u128::from(amount))),
            SampleAgeLabel::MinutesAgo(amount) => self
                .minutes_ago
                .replace("{amount}", &locale.format_integer(u128::from(amount))),
            SampleAgeLabel::NoRecentSample => self.no_recent_sample.to_owned(),
        }
    }
}

pub struct ProcessAgeTexts<'a> {
    seconds_singular: &'a str,
    seconds_plural: &'a str,
    minutes: &'a str,
    hours_singular: &'a str,
    hours_plural: &'a str,
    days_singular: &'a str,
    days_plural: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessAgeUnit {
    Seconds,
    Minutes,
    Hours,
    Days,
}

impl ProcessAgeTexts<'_> {
    pub fn label(&self, unit: ProcessAgeUnit, amount: u64, locale: Locale) -> String {
        let template = match unit {
            ProcessAgeUnit::Seconds if amount == 1 => self.seconds_singular,
            ProcessAgeUnit::Seconds => self.seconds_plural,
            ProcessAgeUnit::Minutes => self.minutes,
            ProcessAgeUnit::Hours if amount == 1 => self.hours_singular,
            ProcessAgeUnit::Hours => self.hours_plural,
            ProcessAgeUnit::Days if amount == 1 => self.days_singular,
            ProcessAgeUnit::Days => self.days_plural,
        };
        template.replace("{amount}", &locale.format_integer(u128::from(amount)))
    }
}

impl CapabilityStatusTexts<'_> {
    pub fn unsupported(&self, capability: UnsupportedCapability) -> String {
        let feature = match capability {
            UnsupportedCapability::GpuMetrics => self.gpu_metrics,
            UnsupportedCapability::BatteryMetrics => self.battery_metrics,
            UnsupportedCapability::MountedVolumes => self.mounted_volumes,
            UnsupportedCapability::PerVolumeIoRates => self.per_volume_io_rates,
            UnsupportedCapability::NetworkInterfaces => self.network_interfaces,
            UnsupportedCapability::BlockDeviceIo => self.block_device_io,
        };
        self.unsupported_template.replace("{feature}", feature)
    }

    pub fn none_available(&self, collection: EmptyDeviceCollection) -> &str {
        match collection {
            EmptyDeviceCollection::MountedVolumes => self.no_mounted_volumes,
            EmptyDeviceCollection::NetworkInterfaces => self.no_network_interfaces,
        }
    }
}

impl MetricStatusTexts<'_> {
    pub fn label(&self, status: &str) -> String {
        match status {
            "ok" => self.available.to_owned(),
            "warming_up" => self.warming_up.to_owned(),
            "unsupported" => self.unsupported.to_owned(),
            "temporarily_unavailable" => self.temporarily_unavailable.to_owned(),
            "error" => self.error.to_owned(),
            other => other.to_owned(),
        }
    }
}

pub struct ChartsTexts<'a> {
    pub now: &'a str,
    pub waiting_for_samples: &'a str,
    pub history: &'a str,
    pub start: &'a str,
    pub no_data: &'a str,
    pub minutes_ago: &'a str,
    cpu_title: &'a str,
    memory_title: &'a str,
    accessible_waiting: &'a str,
    accessible_summary: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChartMetric {
    Cpu,
    Memory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChartHistoryRange {
    NoData,
    History,
    Start,
    MinutesAgo(u64),
}

impl ChartsTexts<'_> {
    pub fn history_range_label(&self, range: ChartHistoryRange, locale: Locale) -> String {
        match range {
            ChartHistoryRange::NoData => self.no_data.to_owned(),
            ChartHistoryRange::History => self.history.to_owned(),
            ChartHistoryRange::Start => self.start.to_owned(),
            ChartHistoryRange::MinutesAgo(minutes) => self
                .minutes_ago
                .replace("{minutes}", &locale.format_integer(u128::from(minutes))),
        }
    }

    pub fn accessible_summary(
        &self,
        metric: ChartMetric,
        values: &[f32],
        locale: Locale,
    ) -> String {
        let title = match metric {
            ChartMetric::Cpu => self.cpu_title,
            ChartMetric::Memory => self.memory_title,
        };
        let mut finite_values = values.iter().copied().filter(|value| value.is_finite());
        let Some(first_value) = finite_values.next() else {
            return self.accessible_waiting.replace("{title}", title);
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

        self.accessible_summary
            .replace("{title}", title)
            .replace("{count}", &locale.format_integer(sample_count as u128))
            .replace("{minimum}", &locale.format_decimal(f64::from(minimum), 1))
            .replace("{maximum}", &locale.format_decimal(f64::from(maximum), 1))
            .replace("{latest}", &locale.format_decimal(f64::from(latest), 1))
    }
}

pub struct StorageTexts<'a> {
    pub units: &'a str,
    pub si_accessible: &'a str,
    pub iec_accessible: &'a str,
    pub mount_point: &'a str,
    pub read: &'a str,
    pub write: &'a str,
    pub file_system: &'a str,
    pub available: &'a str,
    pub total: &'a str,
    pub block_activity: &'a str,
    pub device: &'a str,
    pub read_iops: &'a str,
    pub write_iops: &'a str,
    pub busy: &'a str,
}

pub struct NetworkTexts<'a> {
    pub interface: &'a str,
    pub kind: &'a str,
    pub kind_loopback: &'a str,
    pub kind_unknown: &'a str,
    pub received: &'a str,
    pub sent: &'a str,
}

pub struct ProcessTexts<'a> {
    pub list_title: &'a str,
    pub filter_placeholder: &'a str,
    pub filter_accessible_label: &'a str,
    pub column_process: &'a str,
    pub pid_label: &'a str,
    pub parent_pid_label: &'a str,
    pub cpu_label: &'a str,
    pub sort_unsorted_description: &'a str,
    pub sort_ascending_description: &'a str,
    pub sort_descending_description: &'a str,
    pub empty_filtered: &'a str,
    pub waiting_connection: &'a str,
    pub started_label: &'a str,
    pub memory_label: &'a str,
    pub memory_accessible_label: &'a str,
    pub select_prompt: &'a str,
    pub tree_title: &'a str,
    pub tree_loading: &'a str,
    pub tree_truncated: &'a str,
    pub tree_empty: &'a str,
    pub tree_process_missing: &'a str,
    pub tree_error: &'a str,
}

pub struct ConnectionStatusTexts<'a> {
    pub agent_disconnected: &'a str,
    pub waiting_for_agent: &'a str,
    pub gpu_unavailable: &'a str,
    pub battery_unavailable: &'a str,
}

impl ProcessTexts<'_> {
    pub fn tree_status(&self, status: ProcessTreeStatus) -> &str {
        match status {
            ProcessTreeStatus::None => "",
            ProcessTreeStatus::Loading => self.tree_loading,
            ProcessTreeStatus::Truncated => self.tree_truncated,
            ProcessTreeStatus::Empty => self.tree_empty,
            ProcessTreeStatus::ProcessMissing => self.tree_process_missing,
            ProcessTreeStatus::Error => self.tree_error,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProcessTreeStatus {
    None = 0,
    Loading = 1,
    Truncated = 2,
    Empty = 3,
    ProcessMissing = 4,
    Error = 5,
}

impl ProcessTreeStatus {
    pub fn code(self) -> u8 {
        self as u8
    }

    pub fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Loading,
            2 => Self::Truncated,
            3 => Self::Empty,
            4 => Self::ProcessMissing,
            5 => Self::Error,
            _ => Self::None,
        }
    }
}

impl UiCatalogs {
    pub fn load() -> Result<Self> {
        let pt_br = toml::from_str(include_str!("../resources/i18n/pt-BR.toml"))
            .context("parse embedded pt-BR UI catalog")?;
        let en_us = toml::from_str(include_str!("../resources/i18n/en-US.toml"))
            .context("parse embedded en-US UI catalog")?;
        let catalogs = Self { pt_br, en_us };
        catalogs.pt_br.validate("pt-BR")?;
        catalogs.en_us.validate("en-US")?;
        Ok(catalogs)
    }

    pub fn format_count(&self, kind: CountKind, count: usize, locale: Locale) -> String {
        let catalog = self.catalog(locale);
        let template = catalog.count_template(kind, count);
        let formatted_count = locale.format_integer(count as u128);
        template.replace("{count}", &formatted_count)
    }

    pub fn process_detail(&self, count: usize, locale: Locale) -> String {
        let catalog = self.catalog(locale);
        let detail = if count == 1 {
            &catalog.processes.detail_singular
        } else {
            &catalog.processes.detail_plural
        };
        detail.clone()
    }

    pub fn header_texts(&self, locale: Locale) -> HeaderTexts<'_> {
        let header = &self.catalog(locale).header;
        HeaderTexts {
            title: &header.title,
            tagline: &header.tagline,
            language_pt_br_accessible: &header.language_pt_br_accessible,
            language_en_us_accessible: &header.language_en_us_accessible,
            language_pt_br_short: &header.language_pt_br_short,
            language_en_us_short: &header.language_en_us_short,
            agent_active: &header.agent_active,
            agent_unavailable: &header.agent_unavailable,
            setup_startup: &header.setup_startup,
            retry_connection: &header.retry_connection,
        }
    }

    pub fn agent_status_texts(&self, locale: Locale) -> AgentStatusTexts<'_> {
        let status = &self.catalog(locale).agent_status;
        AgentStatusTexts {
            connecting: &status.connecting,
            active_with_startup: &status.active_with_startup,
            connected_startup_error: &status.connected_startup_error,
            connected_collection: &status.connected_collection,
            unavailable_startup_error: &status.unavailable_startup_error,
            unavailable_startup: &status.unavailable_startup,
            unavailable_ipc: &status.unavailable_ipc,
            startup_systemd_user: &status.startup_systemd_user,
            startup_xdg_autostart: &status.startup_xdg_autostart,
            startup_launch_agent: &status.startup_launch_agent,
            startup_windows_logon_task: &status.startup_windows_logon_task,
        }
    }

    pub fn navigation_texts(&self, locale: Locale) -> NavigationTexts<'_> {
        let navigation = &self.catalog(locale).navigation;
        NavigationTexts {
            section: &navigation.section,
            accessible_label: &navigation.accessible_label,
            overview: &navigation.overview,
            processes: &navigation.processes,
            performance: &navigation.performance,
            storage: &navigation.storage,
            network: &navigation.network,
            collection_continues: &navigation.collection_continues,
            keyboard_shortcut: &navigation.keyboard_shortcut,
            keyboard_shortcut_accessible: &navigation.keyboard_shortcut_accessible,
        }
    }

    pub fn density_texts(&self, locale: Locale) -> DensityTexts<'_> {
        let density = &self.catalog(locale).density;
        DensityTexts {
            comfortable: &density.comfortable,
            comfortable_accessible: &density.comfortable_accessible,
            compact: &density.compact,
            compact_accessible: &density.compact_accessible,
        }
    }

    pub fn overview_texts(&self, locale: Locale) -> OverviewTexts<'_> {
        let overview = &self.catalog(locale).overview;
        OverviewTexts {
            live_updates: &overview.live_updates,
            total_cpu: &overview.total_cpu,
            memory: &overview.memory,
            swap: &overview.swap,
            processes: &overview.processes,
            battery: &overview.battery,
            cpu_usage: &overview.cpu_usage,
            memory_usage: &overview.memory_usage,
        }
    }

    pub fn performance_texts(&self, locale: Locale) -> PerformanceTexts<'_> {
        let performance = &self.catalog(locale).performance;
        PerformanceTexts {
            one_minute: &performance.one_minute,
            five_minutes: &performance.five_minutes,
            total_cpu: &performance.total_cpu,
            memory_used: &performance.memory_used,
            cpu_per_core: &performance.cpu_per_core,
            cpu_unavailable: &performance.cpu_unavailable,
            gpu_vram: &performance.gpu_vram,
        }
    }

    pub fn metric_status_texts(&self, locale: Locale) -> MetricStatusTexts<'_> {
        let statuses = &self.catalog(locale).metric_status;
        MetricStatusTexts {
            available: &statuses.available,
            warming_up: &statuses.warming_up,
            unsupported: &statuses.unsupported,
            temporarily_unavailable: &statuses.temporarily_unavailable,
            error: &statuses.error,
        }
    }

    pub fn telemetry_status_texts(&self, locale: Locale) -> TelemetryStatusTexts<'_> {
        let statuses = &self.catalog(locale).telemetry_status;
        TelemetryStatusTexts {
            unavailable: &statuses.unavailable,
            unavailable_sample: &statuses.unavailable_sample,
            core_name: &statuses.core_name,
            unknown_state: &statuses.unknown_state,
            power_unavailable: &statuses.power_unavailable,
            memory_used_percent: &statuses.memory_used_percent,
            swap_none: &statuses.swap_none,
            swap_unconfigured: &statuses.swap_unconfigured,
            swap_available: &statuses.swap_available,
        }
    }

    pub fn capability_status_texts(&self, locale: Locale) -> CapabilityStatusTexts<'_> {
        let statuses = &self.catalog(locale).capability_status;
        CapabilityStatusTexts {
            unsupported_template: &statuses.unsupported,
            gpu_metrics: &statuses.gpu_metrics,
            battery_metrics: &statuses.battery_metrics,
            mounted_volumes: &statuses.mounted_volumes,
            per_volume_io_rates: &statuses.per_volume_io_rates,
            network_interfaces: &statuses.network_interfaces,
            block_device_io: &statuses.block_device_io,
            no_mounted_volumes: &statuses.no_mounted_volumes,
            no_network_interfaces: &statuses.no_network_interfaces,
        }
    }

    pub fn sample_age_texts(&self, locale: Locale) -> SampleAgeTexts<'_> {
        let age = &self.catalog(locale).sample_age;
        SampleAgeTexts {
            waiting_first_sample: &age.waiting_first_sample,
            less_than_second: &age.less_than_second,
            seconds_ago: &age.seconds_ago,
            minutes_ago: &age.minutes_ago,
            no_recent_sample: &age.no_recent_sample,
        }
    }

    pub fn process_age_texts(&self, locale: Locale) -> ProcessAgeTexts<'_> {
        let age = &self.catalog(locale).process_age;
        ProcessAgeTexts {
            seconds_singular: &age.seconds_singular,
            seconds_plural: &age.seconds_plural,
            minutes: &age.minutes,
            hours_singular: &age.hours_singular,
            hours_plural: &age.hours_plural,
            days_singular: &age.days_singular,
            days_plural: &age.days_plural,
        }
    }

    pub fn charts_texts(&self, locale: Locale) -> ChartsTexts<'_> {
        let charts = &self.catalog(locale).charts;
        ChartsTexts {
            now: &charts.now,
            waiting_for_samples: &charts.waiting_for_samples,
            history: &charts.history,
            start: &charts.start,
            no_data: &charts.no_data,
            minutes_ago: &charts.minutes_ago,
            cpu_title: &charts.cpu_title,
            memory_title: &charts.memory_title,
            accessible_waiting: &charts.accessible_waiting,
            accessible_summary: &charts.accessible_summary,
        }
    }

    pub fn storage_texts(&self, locale: Locale) -> StorageTexts<'_> {
        let storage = &self.catalog(locale).storage;
        StorageTexts {
            units: &storage.units,
            si_accessible: &storage.si_accessible,
            iec_accessible: &storage.iec_accessible,
            mount_point: &storage.mount_point,
            read: &storage.read,
            write: &storage.write,
            file_system: &storage.file_system,
            available: &storage.available,
            total: &storage.total,
            block_activity: &storage.block_activity,
            device: &storage.device,
            read_iops: &storage.read_iops,
            write_iops: &storage.write_iops,
            busy: &storage.busy,
        }
    }

    pub fn network_texts(&self, locale: Locale) -> NetworkTexts<'_> {
        let network = &self.catalog(locale).network;
        NetworkTexts {
            interface: &network.interface,
            kind: &network.kind,
            kind_loopback: &network.kind_loopback,
            kind_unknown: &network.kind_unknown,
            received: &network.received,
            sent: &network.sent,
        }
    }

    pub fn process_texts(&self, locale: Locale) -> ProcessTexts<'_> {
        let catalog = self.catalog(locale);
        let processes = &catalog.processes;
        ProcessTexts {
            list_title: &processes.list_title,
            filter_placeholder: &processes.filter_placeholder,
            filter_accessible_label: &processes.filter_accessible_label,
            column_process: &processes.column_process,
            pid_label: &processes.pid_label,
            parent_pid_label: &processes.parent_pid_label,
            cpu_label: &processes.cpu_label,
            sort_unsorted_description: &processes.sort_unsorted_description,
            sort_ascending_description: &processes.sort_ascending_description,
            sort_descending_description: &processes.sort_descending_description,
            empty_filtered: &processes.empty_filtered,
            waiting_connection: &catalog.connection_status.waiting_for_agent,
            started_label: &processes.started_label,
            memory_label: &processes.memory_label,
            memory_accessible_label: &processes.memory_accessible_label,
            select_prompt: &processes.select_prompt,
            tree_title: &processes.tree_title,
            tree_loading: &processes.tree_loading,
            tree_truncated: &processes.tree_truncated,
            tree_empty: &processes.tree_empty,
            tree_process_missing: &processes.tree_process_missing,
            tree_error: &processes.tree_error,
        }
    }

    pub fn connection_status_texts(&self, locale: Locale) -> ConnectionStatusTexts<'_> {
        let statuses = &self.catalog(locale).connection_status;
        ConnectionStatusTexts {
            agent_disconnected: &statuses.agent_disconnected,
            waiting_for_agent: &statuses.waiting_for_agent,
            gpu_unavailable: &statuses.gpu_unavailable,
            battery_unavailable: &statuses.battery_unavailable,
        }
    }

    fn catalog(&self, locale: Locale) -> &UiCatalog {
        match locale {
            Locale::PtBr => &self.pt_br,
            Locale::EnUs => &self.en_us,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ChartHistoryRange, CountKind, EmptyDeviceCollection, ProcessAgeUnit, ProcessTreeStatus,
        SampleAgeLabel, UiCatalogs, UnsupportedCapability,
    };
    use crate::Locale;
    use native_monitor_agent_lifecycle::StartupMode;

    #[test]
    fn embedded_catalogs_cover_zero_singular_plural_and_grouped_counts() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");
        let cases = [
            (
                CountKind::Processes,
                Locale::PtBr,
                [
                    (0, "0 processos visíveis"),
                    (1, "1 processo visível"),
                    (2, "2 processos visíveis"),
                    (1_234, "1.234 processos visíveis"),
                ],
            ),
            (
                CountKind::Processes,
                Locale::EnUs,
                [
                    (0, "0 processes visible"),
                    (1, "1 process visible"),
                    (2, "2 processes visible"),
                    (1_234, "1,234 processes visible"),
                ],
            ),
            (
                CountKind::Volumes,
                Locale::PtBr,
                [
                    (0, "0 volumes"),
                    (1, "1 volume"),
                    (2, "2 volumes"),
                    (1_234, "1.234 volumes"),
                ],
            ),
            (
                CountKind::Volumes,
                Locale::EnUs,
                [
                    (0, "0 volumes"),
                    (1, "1 volume"),
                    (2, "2 volumes"),
                    (1_234, "1,234 volumes"),
                ],
            ),
            (
                CountKind::NetworkInterfaces,
                Locale::PtBr,
                [
                    (0, "0 interfaces"),
                    (1, "1 interface"),
                    (2, "2 interfaces"),
                    (1_234, "1.234 interfaces"),
                ],
            ),
            (
                CountKind::NetworkInterfaces,
                Locale::EnUs,
                [
                    (0, "0 interfaces"),
                    (1, "1 interface"),
                    (2, "2 interfaces"),
                    (1_234, "1,234 interfaces"),
                ],
            ),
        ];

        for (kind, locale, expected_outputs) in cases {
            for (count, expected) in expected_outputs {
                assert_eq!(catalogs.format_count(kind, count, locale), expected);
            }
        }
    }

    #[test]
    fn process_overview_details_use_localized_singular_and_plural_forms() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");

        assert_eq!(
            catalogs.process_detail(1, Locale::PtBr),
            "visível nesta amostra"
        );
        assert_eq!(
            catalogs.process_detail(0, Locale::PtBr),
            "visíveis nesta amostra"
        );
        assert_eq!(
            catalogs.process_detail(1, Locale::EnUs),
            "visible in this sample"
        );
        assert_eq!(
            catalogs.process_detail(2, Locale::EnUs),
            "visible in this sample"
        );
    }

    #[test]
    fn header_text_is_externalized_for_both_supported_locales() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");
        let portuguese = catalogs.header_texts(Locale::PtBr);
        let english = catalogs.header_texts(Locale::EnUs);

        assert_eq!(portuguese.title, "Monitor do Sistema");
        assert_eq!(portuguese.tagline, "Visibilidade clara do seu computador");
        assert_eq!(portuguese.language_pt_br_accessible, "Português (Brasil)");
        assert_eq!(
            portuguese.language_en_us_accessible,
            "English (United States)"
        );
        assert_eq!(portuguese.language_pt_br_short, "PT");
        assert_eq!(portuguese.language_en_us_short, "EN");
        assert_eq!(portuguese.agent_active, "Agente ativo");
        assert_eq!(portuguese.agent_unavailable, "Agente indisponível");
        assert_eq!(portuguese.setup_startup, "Configurar início");
        assert_eq!(portuguese.retry_connection, "Tentar conexão");

        assert_eq!(english.title, "System Monitor");
        assert_eq!(english.tagline, "A clear view of your computer");
        assert_eq!(english.language_pt_br_accessible, "Português (Brasil)");
        assert_eq!(english.language_en_us_accessible, "English (United States)");
        assert_eq!(english.language_pt_br_short, "PT");
        assert_eq!(english.language_en_us_short, "EN");
        assert_eq!(english.agent_active, "Agent active");
        assert_eq!(english.agent_unavailable, "Agent unavailable");
        assert_eq!(english.setup_startup, "Setup startup");
        assert_eq!(english.retry_connection, "Retry connection");
    }

    #[test]
    fn agent_lifecycle_status_is_externalized_for_both_supported_locales() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");
        let portuguese = catalogs.agent_status_texts(Locale::PtBr);
        let english = catalogs.agent_status_texts(Locale::EnUs);

        assert_eq!(portuguese.connecting(), "Conectando ao agente…");
        assert_eq!(english.connecting(), "Connecting to agent…");
        assert_eq!(
            portuguese.lifecycle("ok", Some(StartupMode::SystemdUser), None),
            "Agente ativo · coleta: ok · início no login: systemd --user"
        );
        assert_eq!(
            portuguese.lifecycle("degraded", None, Some("permission denied")),
            "Agente conectado · início automático não configurado: permission denied"
        );
        assert_eq!(
            portuguese.lifecycle("ok", None, None),
            "Agente conectado · coleta: ok"
        );
        assert_eq!(
            portuguese.unavailable("socket not found", None, Some("permission denied")),
            "Agente indisponível · permission denied · IPC: socket not found"
        );
        assert_eq!(
            portuguese.unavailable("socket not found", Some(StartupMode::SystemdUser), None),
            "Agente indisponível · IPC: socket not found · início no login: systemd --user"
        );
        assert_eq!(
            portuguese.unavailable("socket not found", None, None),
            "Agente indisponível · IPC: socket not found"
        );

        assert_eq!(
            english.lifecycle("ok", Some(StartupMode::SystemdUser), None),
            "Agent active · collection: ok · start at login: systemd --user"
        );
        assert_eq!(
            english.lifecycle("degraded", None, Some("permission denied")),
            "Agent connected · automatic startup not configured: permission denied"
        );
        assert_eq!(
            english.lifecycle("ok", None, None),
            "Agent connected · collection: ok"
        );
        assert_eq!(
            english.unavailable("socket not found", None, Some("permission denied")),
            "Agent unavailable · permission denied · IPC: socket not found"
        );
        assert_eq!(
            english.unavailable("socket not found", Some(StartupMode::SystemdUser), None),
            "Agent unavailable · IPC: socket not found · start at login: systemd --user"
        );
        assert_eq!(
            english.unavailable("socket not found", None, None),
            "Agent unavailable · IPC: socket not found"
        );

        assert_eq!(
            portuguese.startup_mode_label(StartupMode::XdgAutostart),
            "autostart XDG (sem restart automático)"
        );
        assert_eq!(
            portuguese.startup_mode_label(StartupMode::LaunchAgent),
            "LaunchAgent do usuário"
        );
        assert_eq!(
            portuguese.startup_mode_label(StartupMode::WindowsLogonTask),
            "tarefa de logon do usuário"
        );
        assert_eq!(
            english.startup_mode_label(StartupMode::XdgAutostart),
            "XDG autostart (automatic restart unavailable)"
        );
        assert_eq!(
            english.startup_mode_label(StartupMode::LaunchAgent),
            "user LaunchAgent"
        );
        assert_eq!(
            english.startup_mode_label(StartupMode::WindowsLogonTask),
            "user logon task"
        );
    }

    #[test]
    fn agent_connection_status_is_externalized_for_both_supported_locales() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");
        let portuguese = catalogs.connection_status_texts(Locale::PtBr);
        let english = catalogs.connection_status_texts(Locale::EnUs);

        assert_eq!(portuguese.agent_disconnected, "Sem conexão com o agente");
        assert_eq!(
            portuguese.waiting_for_agent,
            "Aguardando conexão com o agente…"
        );
        assert_eq!(english.agent_disconnected, "Agent disconnected");
        assert_eq!(english.waiting_for_agent, "Waiting for agent connection…");
        assert_eq!(portuguese.gpu_unavailable, "GPU indisponível");
        assert_eq!(english.gpu_unavailable, "GPU unavailable");
        assert_eq!(portuguese.battery_unavailable, "Bateria indisponível");
        assert_eq!(english.battery_unavailable, "Battery unavailable");
    }

    #[test]
    fn navigation_text_is_externalized_for_both_supported_locales() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");
        let portuguese = catalogs.navigation_texts(Locale::PtBr);
        let english = catalogs.navigation_texts(Locale::EnUs);

        assert_eq!(portuguese.section, "MONITORAMENTO");
        assert_eq!(portuguese.accessible_label, "Navegação principal");
        assert_eq!(portuguese.overview, "Visão geral");
        assert_eq!(portuguese.processes, "Processos");
        assert_eq!(portuguese.performance, "Desempenho");
        assert_eq!(portuguese.storage, "Armazenamento");
        assert_eq!(portuguese.network, "Rede");
        assert_eq!(
            portuguese.collection_continues,
            "A coleta continua\ncom a janela fechada"
        );
        assert_eq!(portuguese.keyboard_shortcut, "Ctrl+1–5: trocar páginas");
        assert_eq!(
            portuguese.keyboard_shortcut_accessible,
            "Atalho: Control mais 1 a 5 troca de página"
        );

        assert_eq!(english.section, "MONITORING");
        assert_eq!(english.accessible_label, "Main navigation");
        assert_eq!(english.overview, "Overview");
        assert_eq!(english.processes, "Processes");
        assert_eq!(english.performance, "Performance");
        assert_eq!(english.storage, "Storage");
        assert_eq!(english.network, "Network");
        assert_eq!(
            english.collection_continues,
            "Collection continues\nwhen the window is closed"
        );
        assert_eq!(english.keyboard_shortcut, "Ctrl+1–5: switch pages");
        assert_eq!(
            english.keyboard_shortcut_accessible,
            "Keyboard shortcut: Control plus 1 through 5 switches pages"
        );
    }

    #[test]
    fn density_text_is_externalized_for_both_supported_locales() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");
        let portuguese = catalogs.density_texts(Locale::PtBr);
        let english = catalogs.density_texts(Locale::EnUs);

        assert_eq!(portuguese.comfortable, "Confortável");
        assert_eq!(portuguese.comfortable_accessible, "Densidade confortável");
        assert_eq!(portuguese.compact, "Compacta");
        assert_eq!(portuguese.compact_accessible, "Densidade compacta");

        assert_eq!(english.comfortable, "Comfortable");
        assert_eq!(english.comfortable_accessible, "Comfortable density");
        assert_eq!(english.compact, "Compact");
        assert_eq!(english.compact_accessible, "Compact density");
    }

    #[test]
    fn overview_text_is_externalized_for_both_supported_locales() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");
        let portuguese = catalogs.overview_texts(Locale::PtBr);
        let english = catalogs.overview_texts(Locale::EnUs);

        assert_eq!(portuguese.live_updates, "Atualização contínua");
        assert_eq!(portuguese.total_cpu, "CPU total");
        assert_eq!(portuguese.memory, "Memória");
        assert_eq!(portuguese.swap, "Memória swap");
        assert_eq!(portuguese.processes, "Processos");
        assert_eq!(portuguese.battery, "Bateria");
        assert_eq!(portuguese.cpu_usage, "Uso de CPU");
        assert_eq!(portuguese.memory_usage, "Uso de memória");

        assert_eq!(english.live_updates, "Live updates");
        assert_eq!(english.total_cpu, "Total CPU");
        assert_eq!(english.memory, "Memory");
        assert_eq!(english.swap, "Swap");
        assert_eq!(english.processes, "Processes");
        assert_eq!(english.battery, "Battery");
        assert_eq!(english.cpu_usage, "CPU usage");
        assert_eq!(english.memory_usage, "Memory usage");
    }

    #[test]
    fn process_list_text_is_externalized_for_both_supported_locales() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");
        let portuguese = catalogs.process_texts(Locale::PtBr);
        let english = catalogs.process_texts(Locale::EnUs);

        assert_eq!(portuguese.list_title, "Lista de processos");
        assert_eq!(portuguese.filter_placeholder, "Filtrar por nome ou PID");
        assert_eq!(
            portuguese.filter_accessible_label,
            "Filtrar processos por nome ou PID"
        );
        assert_eq!(portuguese.column_process, "Processo");
        assert_eq!(portuguese.pid_label, "PID");
        assert_eq!(portuguese.parent_pid_label, "PPID");
        assert_eq!(portuguese.cpu_label, "CPU");
        assert_eq!(portuguese.memory_accessible_label, "memória");
        assert_eq!(
            portuguese.sort_unsorted_description,
            "Não ordenado por esta coluna"
        );
        assert_eq!(
            portuguese.sort_ascending_description,
            "Classificação crescente"
        );
        assert_eq!(
            portuguese.sort_descending_description,
            "Classificação decrescente"
        );
        assert_eq!(
            portuguese.empty_filtered,
            "Nenhum processo corresponde ao filtro."
        );
        assert_eq!(
            portuguese.waiting_connection,
            "Aguardando conexão com o agente…"
        );

        assert_eq!(english.list_title, "Process list");
        assert_eq!(english.filter_placeholder, "Filter by name or PID");
        assert_eq!(
            english.filter_accessible_label,
            "Filter processes by name or PID"
        );
        assert_eq!(english.column_process, "Process");
        assert_eq!(english.pid_label, "PID");
        assert_eq!(english.parent_pid_label, "PPID");
        assert_eq!(english.cpu_label, "CPU");
        assert_eq!(english.memory_accessible_label, "memory");
        assert_eq!(
            english.sort_unsorted_description,
            "Not sorted by this column"
        );
        assert_eq!(english.sort_ascending_description, "Sorted ascending");
        assert_eq!(english.sort_descending_description, "Sorted descending");
        assert_eq!(english.empty_filtered, "No processes match the filter.");
        assert_eq!(english.waiting_connection, "Waiting for agent connection…");
    }

    #[test]
    fn performance_page_text_is_externalized_for_both_supported_locales() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");
        let portuguese = catalogs.performance_texts(Locale::PtBr);
        let english = catalogs.performance_texts(Locale::EnUs);

        assert_eq!(portuguese.one_minute, "1 minuto");
        assert_eq!(portuguese.five_minutes, "5 minutos");
        assert_eq!(portuguese.total_cpu, "CPU total");
        assert_eq!(portuguese.memory_used, "Memória usada");
        assert_eq!(portuguese.cpu_per_core, "CPU por núcleo");
        assert_eq!(
            portuguese.cpu_unavailable,
            "Dados por núcleo indisponíveis nesta amostra."
        );
        assert_eq!(portuguese.gpu_vram, "GPU e VRAM");

        assert_eq!(english.one_minute, "1 minute");
        assert_eq!(english.five_minutes, "5 minutes");
        assert_eq!(english.total_cpu, "Total CPU");
        assert_eq!(english.memory_used, "Memory used");
        assert_eq!(english.cpu_per_core, "CPU per core");
        assert_eq!(
            english.cpu_unavailable,
            "Per-core data unavailable in this sample."
        );
        assert_eq!(english.gpu_vram, "GPU and VRAM");
    }

    #[test]
    fn shared_chart_text_is_externalized_for_both_supported_locales() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");
        let portuguese = catalogs.charts_texts(Locale::PtBr);
        let english = catalogs.charts_texts(Locale::EnUs);

        assert_eq!(portuguese.now, "agora");
        assert_eq!(portuguese.waiting_for_samples, "Aguardando amostras");
        assert_eq!(portuguese.history, "histórico");
        assert_eq!(portuguese.start, "início");
        assert_eq!(portuguese.no_data, "sem dados");
        assert_eq!(english.now, "now");
        assert_eq!(english.waiting_for_samples, "Waiting for samples");
        assert_eq!(english.history, "history");
        assert_eq!(english.start, "start");
        assert_eq!(english.no_data, "no data");
        assert_eq!(
            portuguese.history_range_label(ChartHistoryRange::History, Locale::PtBr),
            "histórico"
        );
        assert_eq!(
            english.history_range_label(ChartHistoryRange::History, Locale::EnUs),
            "history"
        );
        assert_eq!(
            portuguese.history_range_label(ChartHistoryRange::MinutesAgo(5), Locale::PtBr),
            "−5 min"
        );
        assert_eq!(
            portuguese.history_range_label(ChartHistoryRange::MinutesAgo(1_234), Locale::PtBr),
            "−1.234 min"
        );
        assert_eq!(
            english.history_range_label(ChartHistoryRange::MinutesAgo(5), Locale::EnUs),
            "5 min ago"
        );
        assert_eq!(
            english.history_range_label(ChartHistoryRange::MinutesAgo(1_234), Locale::EnUs),
            "1,234 min ago"
        );
        assert_eq!(
            portuguese.history_range_label(ChartHistoryRange::Start, Locale::PtBr),
            "início"
        );
        assert_eq!(
            english.history_range_label(ChartHistoryRange::Start, Locale::EnUs),
            "start"
        );
        assert_eq!(
            portuguese.history_range_label(ChartHistoryRange::NoData, Locale::PtBr),
            "sem dados"
        );
        assert_eq!(
            english.history_range_label(ChartHistoryRange::NoData, Locale::EnUs),
            "no data"
        );
    }

    #[test]
    fn telemetry_metric_status_text_is_externalized_for_both_supported_locales() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");
        let portuguese = catalogs.metric_status_texts(Locale::PtBr);
        let english = catalogs.metric_status_texts(Locale::EnUs);

        for (status, expected_portuguese, expected_english) in [
            ("ok", "Disponível", "Available"),
            ("warming_up", "Aquecendo", "Warming up"),
            ("unsupported", "Não suportado", "Not supported"),
            (
                "temporarily_unavailable",
                "Temporariamente indisponível",
                "Temporarily unavailable",
            ),
            ("error", "Erro", "Error"),
        ] {
            assert_eq!(portuguese.label(status), expected_portuguese);
            assert_eq!(english.label(status), expected_english);
        }

        assert_eq!(portuguese.label("new_backend_status"), "new_backend_status");
        assert_eq!(english.label("new_backend_status"), "new_backend_status");
    }

    #[test]
    fn telemetry_fallback_status_text_is_externalized_for_both_locales() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");
        let portuguese = catalogs.telemetry_status_texts(Locale::PtBr);
        let english = catalogs.telemetry_status_texts(Locale::EnUs);

        assert_eq!(portuguese.unavailable(), "Indisponível");
        assert_eq!(english.unavailable(), "Unavailable");
        assert_eq!(
            portuguese.unavailable_sample(),
            "Indisponível nesta amostra"
        );
        assert_eq!(english.unavailable_sample(), "Unavailable in this sample");
        assert_eq!(portuguese.core_name(1, Locale::PtBr), "Núcleo 1");
        assert_eq!(english.core_name(1, Locale::EnUs), "Core 1");
        assert_eq!(portuguese.core_name(1_234, Locale::PtBr), "Núcleo 1.234");
        assert_eq!(english.core_name(1_234, Locale::EnUs), "Core 1,234");
        assert_eq!(portuguese.unknown_state(), "estado desconhecido");
        assert_eq!(english.unknown_state(), "unknown state");
        assert_eq!(portuguese.power_unavailable(), "potência indisponível");
        assert_eq!(english.power_unavailable(), "power unavailable");
        assert_eq!(
            portuguese.memory_used_percent(50.0, Locale::PtBr),
            "50,0% utilizado"
        );
        assert_eq!(
            english.memory_used_percent(50.0, Locale::EnUs),
            "50.0% used"
        );
        assert_eq!(portuguese.swap_none(), "Sem swap");
        assert_eq!(english.swap_none(), "No swap");
        assert_eq!(portuguese.swap_unconfigured(), "Nenhuma área configurada");
        assert_eq!(english.swap_unconfigured(), "No swap area configured");
        assert_eq!(portuguese.swap_available(), "disponível");
        assert_eq!(english.swap_available(), "available");
    }

    #[test]
    fn capability_and_empty_device_statuses_are_localized_for_both_locales() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");
        let portuguese = catalogs.capability_status_texts(Locale::PtBr);
        let english = catalogs.capability_status_texts(Locale::EnUs);
        let cases = [
            (
                UnsupportedCapability::GpuMetrics,
                "Agente sem suporte a métricas de GPU nesta versão",
                "Agent does not support GPU metrics in this version",
            ),
            (
                UnsupportedCapability::BatteryMetrics,
                "Agente sem suporte a métricas de bateria nesta versão",
                "Agent does not support battery metrics in this version",
            ),
            (
                UnsupportedCapability::MountedVolumes,
                "Agente sem suporte a volumes montados nesta versão",
                "Agent does not support mounted volumes in this version",
            ),
            (
                UnsupportedCapability::PerVolumeIoRates,
                "Agente sem suporte a taxas de I/O por volume nesta versão",
                "Agent does not support per-volume I/O rates in this version",
            ),
            (
                UnsupportedCapability::NetworkInterfaces,
                "Agente sem suporte a interfaces de rede nesta versão",
                "Agent does not support network interfaces in this version",
            ),
            (
                UnsupportedCapability::BlockDeviceIo,
                "Agente sem suporte a I/O de dispositivos nesta versão",
                "Agent does not support block device I/O in this version",
            ),
        ];

        for (capability, expected_portuguese, expected_english) in cases {
            assert_eq!(portuguese.unsupported(capability), expected_portuguese);
            assert_eq!(english.unsupported(capability), expected_english);
        }

        assert_eq!(
            portuguese.none_available(EmptyDeviceCollection::MountedVolumes),
            "Nenhum volume montado disponível."
        );
        assert_eq!(
            english.none_available(EmptyDeviceCollection::MountedVolumes),
            "No mounted volumes available."
        );
        assert_eq!(
            portuguese.none_available(EmptyDeviceCollection::NetworkInterfaces),
            "Nenhuma interface de rede disponível."
        );
        assert_eq!(
            english.none_available(EmptyDeviceCollection::NetworkInterfaces),
            "No network interfaces available."
        );
    }

    #[test]
    fn sample_age_statuses_and_templates_are_localized_for_both_locales() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");
        let portuguese = catalogs.sample_age_texts(Locale::PtBr);
        let english = catalogs.sample_age_texts(Locale::EnUs);
        let cases = [
            (
                SampleAgeLabel::WaitingForFirstSample,
                "Aguardando primeira amostra",
                "Waiting for the first sample",
            ),
            (
                SampleAgeLabel::LessThanSecond,
                "Amostra há menos de 1 s",
                "Sample less than 1 s ago",
            ),
            (
                SampleAgeLabel::SecondsAgo(5),
                "Amostra há 5 s",
                "Sample 5 s ago",
            ),
            (
                SampleAgeLabel::MinutesAgo(1_234),
                "Amostra há 1.234 min",
                "Sample 1,234 min ago",
            ),
            (
                SampleAgeLabel::NoRecentSample,
                "Sem amostra recente",
                "No recent sample",
            ),
        ];

        for (age, expected_portuguese, expected_english) in cases {
            assert_eq!(portuguese.label(age, Locale::PtBr), expected_portuguese);
            assert_eq!(english.label(age, Locale::EnUs), expected_english);
        }
    }

    #[test]
    fn process_age_templates_cover_singular_plural_and_grouping() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");
        let portuguese = catalogs.process_age_texts(Locale::PtBr);
        let english = catalogs.process_age_texts(Locale::EnUs);
        let cases = [
            (ProcessAgeUnit::Seconds, 1, "há 1 s", "1 sec ago"),
            (ProcessAgeUnit::Seconds, 2, "há 2 s", "2 secs ago"),
            (ProcessAgeUnit::Minutes, 1, "há 1 min", "1 min ago"),
            (
                ProcessAgeUnit::Minutes,
                1_234,
                "há 1.234 min",
                "1,234 min ago",
            ),
            (ProcessAgeUnit::Hours, 1, "há 1 h", "1 hr ago"),
            (ProcessAgeUnit::Hours, 2, "há 2 h", "2 hrs ago"),
            (ProcessAgeUnit::Days, 1, "há 1 d", "1 day ago"),
            (ProcessAgeUnit::Days, 2, "há 2 d", "2 days ago"),
        ];

        for (unit, amount, expected_portuguese, expected_english) in cases {
            assert_eq!(
                portuguese.label(unit, amount, Locale::PtBr),
                expected_portuguese
            );
            assert_eq!(english.label(unit, amount, Locale::EnUs), expected_english);
        }
    }

    #[test]
    fn storage_and_network_page_text_is_externalized_for_both_supported_locales() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");
        let portuguese_storage = catalogs.storage_texts(Locale::PtBr);
        let english_storage = catalogs.storage_texts(Locale::EnUs);
        let portuguese_network = catalogs.network_texts(Locale::PtBr);
        let english_network = catalogs.network_texts(Locale::EnUs);

        assert_eq!(portuguese_storage.units, "Unidades");
        assert_eq!(portuguese_storage.si_accessible, "Unidades decimais SI");
        assert_eq!(portuguese_storage.iec_accessible, "Unidades binárias IEC");
        assert_eq!(portuguese_storage.mount_point, "Ponto de montagem");
        assert_eq!(portuguese_storage.read, "Leitura");
        assert_eq!(portuguese_storage.write, "Escrita");
        assert_eq!(portuguese_storage.file_system, "Sistema de arquivos");
        assert_eq!(portuguese_storage.available, "Disponível");
        assert_eq!(portuguese_storage.total, "Total");
        assert_eq!(
            portuguese_storage.block_activity,
            "Atividade por dispositivo de bloco (Linux)"
        );
        assert_eq!(portuguese_storage.device, "Dispositivo");
        assert_eq!(portuguese_storage.read_iops, "IOPS leitura");
        assert_eq!(portuguese_storage.write_iops, "IOPS gravação");
        assert_eq!(portuguese_storage.busy, "Ocupação");

        assert_eq!(english_storage.units, "Units");
        assert_eq!(english_storage.si_accessible, "SI decimal units");
        assert_eq!(english_storage.iec_accessible, "IEC binary units");
        assert_eq!(english_storage.mount_point, "Mount point");
        assert_eq!(english_storage.read, "Read");
        assert_eq!(english_storage.write, "Write");
        assert_eq!(english_storage.file_system, "File system");
        assert_eq!(english_storage.available, "Available");
        assert_eq!(english_storage.total, "Total");
        assert_eq!(
            english_storage.block_activity,
            "Block device activity (Linux)"
        );
        assert_eq!(english_storage.device, "Device");
        assert_eq!(english_storage.read_iops, "Read IOPS");
        assert_eq!(english_storage.write_iops, "Write IOPS");
        assert_eq!(english_storage.busy, "Busy");

        assert_eq!(portuguese_network.interface, "Interface");
        assert_eq!(portuguese_network.kind, "Tipo");
        assert_eq!(portuguese_network.kind_loopback, "Loopback");
        assert_eq!(portuguese_network.kind_unknown, "Desconhecido");
        assert_eq!(portuguese_network.received, "Recebimento");
        assert_eq!(portuguese_network.sent, "Envio");
        assert_eq!(english_network.interface, "Interface");
        assert_eq!(english_network.kind, "Type");
        assert_eq!(english_network.kind_loopback, "Loopback");
        assert_eq!(english_network.kind_unknown, "Unknown");
        assert_eq!(english_network.received, "Received");
        assert_eq!(english_network.sent, "Sent");
    }

    #[test]
    fn process_details_and_tree_status_text_is_localized_for_both_supported_locales() {
        let catalogs = UiCatalogs::load().expect("load and validate both catalogs");
        let portuguese = catalogs.process_texts(Locale::PtBr);
        let english = catalogs.process_texts(Locale::EnUs);
        let statuses = [
            (ProcessTreeStatus::None, "", ""),
            (
                ProcessTreeStatus::Loading,
                "Carregando árvore de processos…",
                "Loading process tree…",
            ),
            (
                ProcessTreeStatus::Truncated,
                "Árvore truncada no limite da resposta.",
                "Tree truncated at the response limit.",
            ),
            (
                ProcessTreeStatus::Empty,
                "Nenhum dado de árvore disponível.",
                "No process tree data available.",
            ),
            (
                ProcessTreeStatus::ProcessMissing,
                "O processo selecionado não existe mais.",
                "The selected process no longer exists.",
            ),
            (
                ProcessTreeStatus::Error,
                "Não foi possível carregar a árvore de processos.",
                "Could not load the process tree.",
            ),
        ];

        assert_eq!(portuguese.started_label, "Início:");
        assert_eq!(portuguese.memory_label, "RAM");
        assert_eq!(
            portuguese.select_prompt,
            "Selecione um processo para ver PID, início e consumo."
        );
        assert_eq!(
            portuguese.tree_title,
            "Árvore de processos (somente leitura)"
        );
        assert_eq!(english.started_label, "Started:");
        assert_eq!(english.memory_label, "Memory");
        assert_eq!(
            english.select_prompt,
            "Select a process to view its PID, start time, and usage."
        );
        assert_eq!(english.tree_title, "Process tree (read-only)");

        for (status, expected_portuguese, expected_english) in statuses {
            assert_eq!(ProcessTreeStatus::from_code(status.code()), status);
            assert_eq!(portuguese.tree_status(status), expected_portuguese);
            assert_eq!(english.tree_status(status), expected_english);
        }
        assert_eq!(
            ProcessTreeStatus::from_code(u8::MAX),
            ProcessTreeStatus::None
        );
    }
}
