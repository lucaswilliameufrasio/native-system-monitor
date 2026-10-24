use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentSettings {
    pub collection_interval_ms: u64,
    pub history_capacity: usize,
    pub max_processes_per_response: usize,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            collection_interval_ms: 1_000,
            history_capacity: 300,
            max_processes_per_response: 1_000,
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

        Ok(())
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
}
