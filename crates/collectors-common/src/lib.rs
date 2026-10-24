use native_monitor_collector_api::{Capability, CollectorError, SystemCollector};
use native_monitor_domain::{
    CpuMetrics, MemoryMetrics, MetricStatus, ProcessIdentity, ProcessMetrics, SystemSnapshot,
};
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{ProcessesToUpdate, System};

pub struct SysinfoCollector {
    system: System,
    cpu_has_baseline: bool,
}

impl SysinfoCollector {
    pub fn new() -> Self {
        Self {
            system: System::new(),
            cpu_has_baseline: false,
        }
    }
}

impl Default for SysinfoCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemCollector for SysinfoCollector {
    fn collect(&mut self) -> Result<SystemSnapshot, CollectorError> {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.system.refresh_processes(ProcessesToUpdate::All, true);

        let cpu_status = if self.cpu_has_baseline {
            MetricStatus::Ok
        } else {
            MetricStatus::WarmingUp
        };
        let cpu_is_ready = self.cpu_has_baseline;
        let total_usage_percent = cpu_is_ready.then(|| self.system.global_cpu_usage());
        let per_core_usage_percent = if cpu_is_ready {
            self.system
                .cpus()
                .iter()
                .map(|cpu| cpu.cpu_usage())
                .collect()
        } else {
            Vec::new()
        };
        let total_memory = self.system.total_memory();
        let used_memory = self.system.used_memory();
        let memory_status = if total_memory > 0 {
            MetricStatus::Ok
        } else {
            MetricStatus::TemporarilyUnavailable
        };

        let mut processes = self
            .system
            .processes()
            .iter()
            .map(|(pid, process)| ProcessMetrics {
                identity: ProcessIdentity {
                    pid: pid.as_u32(),
                    start_time_unix_seconds: process.start_time(),
                },
                parent_pid: process.parent().map(|parent| parent.as_u32()),
                name: process.name().to_string_lossy().chars().take(256).collect(),
                cpu_usage_percent: cpu_is_ready.then(|| process.cpu_usage()),
                memory_bytes: process.memory(),
            })
            .collect::<Vec<_>>();
        processes.sort_by(|left, right| {
            compare_cpu_usage(left.cpu_usage_percent, right.cpu_usage_percent)
                .then_with(|| left.identity.pid.cmp(&right.identity.pid))
        });
        self.cpu_has_baseline = true;

        let captured_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| CollectorError::Collection(error.to_string()))?
            .as_millis()
            .try_into()
            .map_err(|error: std::num::TryFromIntError| {
                CollectorError::Collection(error.to_string())
            })?;

        Ok(SystemSnapshot {
            captured_at_unix_ms,
            cpu: CpuMetrics {
                total_usage_percent,
                per_core_usage_percent,
                status: cpu_status,
            },
            memory: MemoryMetrics {
                total_bytes: (total_memory > 0).then_some(total_memory),
                used_bytes: (total_memory > 0).then_some(used_memory),
                swap_total_bytes: Some(self.system.total_swap()),
                swap_used_bytes: Some(self.system.used_swap()),
                status: memory_status,
            },
            processes,
            cpu_history_percent: Vec::new(),
            memory_history_percent: Vec::new(),
            collection_status: "ok".to_owned(),
        })
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability {
                name: "cpu",
                status: "supported",
                source: "sysinfo",
            },
            Capability {
                name: "memory",
                status: "supported",
                source: "sysinfo",
            },
            Capability {
                name: "processes",
                status: "supported",
                source: "sysinfo",
            },
        ]
    }
}

fn compare_cpu_usage(left: Option<f32>, right: Option<f32>) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => right.total_cmp(&left),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}
