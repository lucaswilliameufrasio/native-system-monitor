use crate::Locale;
use anyhow::{Context, Result, bail};
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
    #[serde(rename = "empty-filtered")]
    empty_filtered: String,
    #[serde(rename = "waiting-connection")]
    waiting_connection: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UiCatalog {
    header: HeaderCatalog,
    navigation: NavigationCatalog,
    density: DensityCatalog,
    overview: OverviewCatalog,
    performance: PerformanceCatalog,
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
            (
                "processes.empty-filtered",
                self.processes.empty_filtered.as_str(),
            ),
            (
                "processes.waiting-connection",
                self.processes.waiting_connection.as_str(),
            ),
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
    pub agent_active: &'a str,
    pub agent_unavailable: &'a str,
    pub setup_startup: &'a str,
    pub retry_connection: &'a str,
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

pub struct ProcessTexts<'a> {
    pub list_title: &'a str,
    pub filter_placeholder: &'a str,
    pub filter_accessible_label: &'a str,
    pub column_process: &'a str,
    pub empty_filtered: &'a str,
    pub waiting_connection: &'a str,
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
            agent_active: &header.agent_active,
            agent_unavailable: &header.agent_unavailable,
            setup_startup: &header.setup_startup,
            retry_connection: &header.retry_connection,
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

    pub fn process_texts(&self, locale: Locale) -> ProcessTexts<'_> {
        let processes = &self.catalog(locale).processes;
        ProcessTexts {
            list_title: &processes.list_title,
            filter_placeholder: &processes.filter_placeholder,
            filter_accessible_label: &processes.filter_accessible_label,
            column_process: &processes.column_process,
            empty_filtered: &processes.empty_filtered,
            waiting_connection: &processes.waiting_connection,
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
    use super::{CountKind, UiCatalogs};
    use crate::Locale;

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
        assert_eq!(portuguese.agent_active, "Agente ativo");
        assert_eq!(portuguese.agent_unavailable, "Agente indisponível");
        assert_eq!(portuguese.setup_startup, "Configurar início");
        assert_eq!(portuguese.retry_connection, "Tentar conexão");

        assert_eq!(english.title, "System Monitor");
        assert_eq!(english.tagline, "A clear view of your computer");
        assert_eq!(english.agent_active, "Agent active");
        assert_eq!(english.agent_unavailable, "Agent unavailable");
        assert_eq!(english.setup_startup, "Setup startup");
        assert_eq!(english.retry_connection, "Retry connection");
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
}
