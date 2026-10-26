use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentSettings {
    pub collection_interval_ms: u64,
    pub history_capacity: usize,
    pub max_processes_per_response: usize,
    pub export: ExportSettings,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExportSettings {
    pub prometheus: PrometheusSettings,
    pub remote_write: RemoteWriteSettings,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct RemoteWriteSettings {
    pub enabled: bool,
    pub url: Option<String>,
    pub interval_ms: u64,
    pub families: Vec<String>,
    pub max_memory_queue_mib: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PrometheusSettings {
    pub enabled: bool,
    pub listen: SocketAddr,
    pub families: Vec<String>,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            collection_interval_ms: 1_000,
            history_capacity: 300,
            max_processes_per_response: 1_000,
            export: ExportSettings::default(),
        }
    }
}

impl Default for PrometheusSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            listen: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9_464),
            families: Vec::new(),
        }
    }
}

impl Default for RemoteWriteSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            url: None,
            interval_ms: 10_000,
            families: Vec::new(),
            max_memory_queue_mib: 16,
        }
    }
}

impl AgentSettings {
    pub fn parse_toml(input: &str) -> Result<Self, SettingsError> {
        let settings: Self = toml::from_str(input)?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn validate(&self) -> Result<(), SettingsError> {
        if !(250..=30_000).contains(&self.collection_interval_ms) {
            return Err(SettingsError::InvalidCollectionInterval);
        }

        if !(1..=300).contains(&self.history_capacity) {
            return Err(SettingsError::InvalidHistoryCapacity);
        }

        if !(1..=1_000).contains(&self.max_processes_per_response) {
            return Err(SettingsError::InvalidProcessLimit);
        }

        if !self.export.prometheus.listen.ip().is_loopback()
            || (self.export.prometheus.enabled && self.export.prometheus.listen.port() == 0)
        {
            return Err(SettingsError::InvalidPrometheusListener);
        }

        for family in &self.export.prometheus.families {
            validate_metric_family(family)?;
        }

        let remote_write = &self.export.remote_write;
        if !(1_000..=300_000).contains(&remote_write.interval_ms) {
            return Err(SettingsError::InvalidRemoteWriteInterval);
        }
        if !(1..=16).contains(&remote_write.max_memory_queue_mib) {
            return Err(SettingsError::InvalidRemoteWriteQueueCapacity);
        }
        for family in &remote_write.families {
            validate_metric_family(family)?;
        }
        if remote_write.enabled {
            let endpoint = remote_write
                .url
                .as_deref()
                .ok_or(SettingsError::MissingRemoteWriteUrl)?;
            let endpoint =
                url::Url::parse(endpoint).map_err(|_| SettingsError::InvalidRemoteWriteUrl)?;
            if endpoint.username() != ""
                || endpoint.password().is_some()
                || endpoint.query().is_some()
                || endpoint.fragment().is_some()
                || endpoint.port() == Some(0)
                || !matches!(endpoint.scheme(), "https" | "http")
            {
                return Err(SettingsError::InvalidRemoteWriteUrl);
            }
            if endpoint.scheme() == "http"
                && !endpoint.host_str().is_some_and(|host| {
                    host == "localhost"
                        || host
                            .parse::<IpAddr>()
                            .is_ok_and(|address| address.is_loopback())
                })
            {
                return Err(SettingsError::InsecureRemoteWriteUrl);
            }
            if endpoint.host().is_none() {
                return Err(SettingsError::InvalidRemoteWriteUrl);
            }
        }

        Ok(())
    }
}

fn validate_metric_family(family: &str) -> Result<(), SettingsError> {
    if matches!(
        family,
        "cpu" | "memory" | "storage" | "network" | "gpu" | "battery" | "health"
    ) {
        Ok(())
    } else {
        Err(SettingsError::InvalidPrometheusFamily(family.to_owned()))
    }
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("invalid TOML settings: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("collection_interval_ms must be between 250 and 30000")]
    InvalidCollectionInterval,
    #[error("history_capacity must be between 1 and 300")]
    InvalidHistoryCapacity,
    #[error("max_processes_per_response must be between 1 and 1000")]
    InvalidProcessLimit,
    #[error("prometheus.listen must use a loopback address and a non-zero port when enabled")]
    InvalidPrometheusListener,
    #[error("unsupported Prometheus metric family: {0}")]
    InvalidPrometheusFamily(String),
    #[error("remote_write.interval_ms must be between 1000 and 300000")]
    InvalidRemoteWriteInterval,
    #[error("remote_write.max_memory_queue_mib must be between 1 and 16")]
    InvalidRemoteWriteQueueCapacity,
    #[error("remote_write.url is required when remote_write.enabled is true")]
    MissingRemoteWriteUrl,
    #[error("remote_write.url must be a valid HTTPS URL without credentials, query, or fragment")]
    InvalidRemoteWriteUrl,
    #[error("remote_write.url may use HTTP only for loopback receivers")]
    InsecureRemoteWriteUrl,
}

#[cfg(test)]
mod tests {
    use super::{AgentSettings, SettingsError};

    #[test]
    fn defaults_are_valid_and_parse_empty_configuration() {
        let parsed = AgentSettings::parse_toml("").expect("empty config uses defaults");

        assert_eq!(parsed, AgentSettings::default());
    }

    #[test]
    fn rejects_unknown_fields_instead_of_silently_ignoring_typos() {
        let error = AgentSettings::parse_toml("collection_intervl_ms = 1000")
            .expect_err("unknown settings must be rejected");

        assert!(matches!(error, SettingsError::Parse(_)));
    }

    #[test]
    fn rejects_collection_intervals_outside_safe_range() {
        let error = AgentSettings::parse_toml("collection_interval_ms = 50")
            .expect_err("sub-250 ms collection is not accepted");

        assert!(matches!(error, SettingsError::InvalidCollectionInterval));
    }

    #[test]
    fn prometheus_export_is_disabled_by_default_and_uses_loopback() {
        let parsed = AgentSettings::parse_toml("").expect("default settings parse");

        assert!(!parsed.export.prometheus.enabled);
        assert_eq!(
            parsed.export.prometheus.listen.to_string(),
            "127.0.0.1:9464"
        );
    }

    #[test]
    fn accepts_an_explicit_loopback_prometheus_listener() {
        let settings = AgentSettings::parse_toml(
            "[export.prometheus]\nenabled = true\nlisten = '127.0.0.1:19464'\n",
        )
        .expect("valid loopback exporter configuration");

        assert!(settings.export.prometheus.enabled);
        assert_eq!(
            settings.export.prometheus.listen.to_string(),
            "127.0.0.1:19464"
        );
    }

    #[test]
    fn rejects_non_loopback_prometheus_listeners() {
        let error = AgentSettings::parse_toml(
            "[export.prometheus]\nenabled = true\nlisten = '0.0.0.0:9464'\n",
        )
        .expect_err("exporter must not bind to a non-loopback address");

        assert!(matches!(error, SettingsError::InvalidPrometheusListener));
    }

    #[test]
    fn rejects_ephemeral_prometheus_port_when_enabled() {
        let error = AgentSettings::parse_toml(
            "[export.prometheus]\nenabled = true\nlisten = '127.0.0.1:0'\n",
        )
        .expect_err("enabled exporter needs a discoverable fixed port");

        assert!(matches!(error, SettingsError::InvalidPrometheusListener));
    }

    #[test]
    fn accepts_only_supported_prometheus_metric_families() {
        let settings = AgentSettings::parse_toml(
            "[export.prometheus]\nfamilies = ['cpu', 'memory', 'storage']\n",
        )
        .expect("known metric families are accepted");
        assert_eq!(
            settings.export.prometheus.families,
            vec!["cpu", "memory", "storage"]
        );

        let error = AgentSettings::parse_toml("[export.prometheus]\nfamilies = ['processes']\n")
            .expect_err("unknown metric family is rejected");
        assert!(matches!(error, SettingsError::InvalidPrometheusFamily(_)));
    }

    #[test]
    fn remote_write_is_disabled_by_default_and_requires_a_safe_url_when_enabled() {
        let defaults = AgentSettings::parse_toml("").expect("default settings are valid");
        assert!(!defaults.export.remote_write.enabled);
        assert_eq!(defaults.export.remote_write.max_memory_queue_mib, 16);

        let error = AgentSettings::parse_toml("[export.remote_write]\nenabled = true\n")
            .expect_err("enabled remote write needs an endpoint");
        assert!(matches!(error, SettingsError::MissingRemoteWriteUrl));

        let settings = AgentSettings::parse_toml(
            "[export.remote_write]\nenabled = true\nurl = 'https://metrics.example.test/api/v1/write'\n",
        )
        .expect("HTTPS endpoint is allowed");
        assert!(settings.export.remote_write.enabled);
    }

    #[test]
    fn rejects_plaintext_remote_write_and_secret_bearing_urls() {
        for url in [
            "http://metrics.example.test/api/v1/write",
            "https://user:secret@metrics.example.test/api/v1/write",
            "https://metrics.example.test/api/v1/write?token=secret",
            "https://metrics.example.test:0/api/v1/write",
        ] {
            let config = format!("[export.remote_write]\nenabled = true\nurl = '{url}'\n");
            assert!(
                AgentSettings::parse_toml(&config).is_err(),
                "unsafe URL must be rejected"
            );
        }

        let local = AgentSettings::parse_toml(
            "[export.remote_write]\nenabled = true\nurl = 'http://127.0.0.1:8428/api/v1/write'\n",
        )
        .expect("loopback HTTP is allowed for local receiver labs");
        assert_eq!(
            local.export.remote_write.url.as_deref(),
            Some("http://127.0.0.1:8428/api/v1/write")
        );
    }
}
