use native_monitor_domain::SystemSnapshot;
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Capability {
    pub name: &'static str,
    pub status: &'static str,
    pub source: &'static str,
}

#[derive(Debug, Error)]
pub enum CollectorError {
    #[error("system collection failed: {0}")]
    Collection(String),
}

pub trait SystemCollector: Send {
    fn collect(&mut self) -> Result<SystemSnapshot, CollectorError>;

    fn capabilities(&self) -> Vec<Capability>;
}
