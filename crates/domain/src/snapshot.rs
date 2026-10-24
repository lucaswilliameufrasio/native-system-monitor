use crate::{MetricHistory, ProcessMetrics};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetricStatus {
    Ok,
    WarmingUp,
    Unsupported,
    TemporarilyUnavailable,
    Error,
}

impl MetricStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::WarmingUp => "warming_up",
            Self::Unsupported => "unsupported",
            Self::TemporarilyUnavailable => "temporarily_unavailable",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CpuMetrics {
    pub total_usage_percent: Option<f32>,
    pub per_core_usage_percent: Vec<f32>,
    pub status: MetricStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MemoryMetrics {
    pub total_bytes: Option<u64>,
    pub used_bytes: Option<u64>,
    pub swap_total_bytes: Option<u64>,
    pub swap_used_bytes: Option<u64>,
    pub status: MetricStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SystemSnapshot {
    pub captured_at_unix_ms: u64,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub processes: Vec<ProcessMetrics>,
    pub cpu_history_percent: Vec<f32>,
    pub memory_history_percent: Vec<f32>,
    pub collection_status: String,
}

impl SystemSnapshot {
    pub fn starting() -> Self {
        Self {
            captured_at_unix_ms: 0,
            cpu: CpuMetrics {
                total_usage_percent: None,
                per_core_usage_percent: Vec::new(),
                status: MetricStatus::WarmingUp,
            },
            memory: MemoryMetrics {
                total_bytes: None,
                used_bytes: None,
                swap_total_bytes: None,
                swap_used_bytes: None,
                status: MetricStatus::WarmingUp,
            },
            processes: Vec::new(),
            cpu_history_percent: Vec::new(),
            memory_history_percent: Vec::new(),
            collection_status: "starting".to_owned(),
        }
    }

    pub fn unavailable(reason: String) -> Self {
        let mut snapshot = Self::starting();
        snapshot.cpu.status = MetricStatus::Error;
        snapshot.memory.status = MetricStatus::Error;
        snapshot.collection_status = reason;
        snapshot
    }

    pub fn add_history(&mut self, cpu: &MetricHistory, memory: &MetricHistory) {
        self.cpu_history_percent = cpu.values();
        self.memory_history_percent = memory.values();
    }
}
