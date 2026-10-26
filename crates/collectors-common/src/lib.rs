use native_monitor_collector_api::{Capability, CollectorError, SystemCollector};
#[cfg(target_os = "linux")]
use native_monitor_domain::BlockDeviceMetrics;
use native_monitor_domain::{
    CpuMetrics, MemoryMetrics, MetricStatus, NetworkInterfaceMetrics, ProcessIdentity,
    ProcessMetrics, SystemSnapshot, VolumeMetrics,
};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{DiskRefreshKind, Disks, Networks, ProcessesToUpdate, System};

#[derive(Clone, Copy)]
struct NetworkCounters {
    received_bytes: u64,
    transmitted_bytes: u64,
    sampled_at: Instant,
}

pub struct SysinfoCollector {
    system: System,
    disks: Disks,
    networks: Networks,
    previous_network_counters: HashMap<String, NetworkCounters>,
    #[cfg(target_os = "linux")]
    previous_block_device_counters:
        HashMap<String, (native_monitor_collectors_linux::DiskCounters, Instant)>,
    cpu_has_baseline: bool,
}

impl SysinfoCollector {
    pub fn new() -> Self {
        Self {
            system: System::new(),
            disks: Disks::new(),
            networks: Networks::new(),
            previous_network_counters: HashMap::new(),
            #[cfg(target_os = "linux")]
            previous_block_device_counters: HashMap::new(),
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
        self.disks
            .refresh_specifics(true, DiskRefreshKind::nothing().with_storage());
        self.networks.refresh(true);

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
        let volumes = self
            .disks
            .list()
            .iter()
            .map(|disk| VolumeMetrics {
                mount_point: disk.mount_point().to_string_lossy().into_owned(),
                file_system: disk.file_system().to_string_lossy().into_owned(),
                total_bytes: (disk.total_space() > 0).then_some(disk.total_space()),
                available_bytes: (disk.total_space() > 0).then_some(disk.available_space()),
            })
            .collect();
        let sampled_at = Instant::now();
        let mut current_network_counters = HashMap::new();
        let network_interfaces = self
            .networks
            .iter()
            .map(|(name, network)| {
                let current = NetworkCounters {
                    received_bytes: network.total_received(),
                    transmitted_bytes: network.total_transmitted(),
                    sampled_at,
                };
                let previous = self.previous_network_counters.get(name);
                let elapsed = previous.map(|counter| sampled_at.duration_since(counter.sampled_at));
                let metrics = NetworkInterfaceMetrics {
                    name: name.clone(),
                    received_bytes_per_second: previous.and_then(|counter| {
                        counter_rate(current.received_bytes, counter.received_bytes, elapsed?)
                    }),
                    transmitted_bytes_per_second: previous.and_then(|counter| {
                        counter_rate(
                            current.transmitted_bytes,
                            counter.transmitted_bytes,
                            elapsed?,
                        )
                    }),
                };
                current_network_counters.insert(name.clone(), current);
                metrics
            })
            .collect();
        self.previous_network_counters = current_network_counters;
        #[cfg(target_os = "linux")]
        let (block_devices, block_device_io_status) = self.collect_block_devices(sampled_at);
        #[cfg(not(target_os = "linux"))]
        let (block_devices, block_device_io_status) = (Vec::new(), MetricStatus::Unsupported);
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
            volumes,
            network_interfaces,
            block_devices,
            block_device_io_status,
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
            Capability {
                name: "mounted_volumes",
                status: "supported",
                source: "sysinfo",
            },
            Capability {
                name: "network_interfaces",
                status: "supported",
                source: "sysinfo",
            },
            Capability {
                name: "block_device_io",
                #[cfg(target_os = "linux")]
                status: "supported",
                #[cfg(not(target_os = "linux"))]
                status: "unsupported",
                #[cfg(target_os = "linux")]
                source: "/proc/diskstats",
                #[cfg(not(target_os = "linux"))]
                source: "no native adapter",
            },
        ]
    }
}

#[cfg(target_os = "linux")]
impl SysinfoCollector {
    fn collect_block_devices(
        &mut self,
        sampled_at: Instant,
    ) -> (Vec<BlockDeviceMetrics>, MetricStatus) {
        let counters = match native_monitor_collectors_linux::read_system_disk_counters() {
            Ok(counters) => counters,
            Err(_) => return (Vec::new(), MetricStatus::Error),
        };
        let mut current_counters = HashMap::new();
        let mut devices = Vec::with_capacity(counters.len());

        for counters in counters {
            let id = counters.id();
            let previous = self.previous_block_device_counters.get(&id);
            let elapsed = previous.map(|(_, previous_at)| sampled_at.duration_since(*previous_at));
            let metrics = BlockDeviceMetrics {
                id: id.clone(),
                name: counters.name.clone(),
                read_bytes_per_second: previous.and_then(|(previous, _)| {
                    counter_rate_scaled(
                        counters.sectors_read,
                        previous.sectors_read,
                        elapsed?,
                        512.0,
                    )
                }),
                write_bytes_per_second: previous.and_then(|(previous, _)| {
                    counter_rate_scaled(
                        counters.sectors_written,
                        previous.sectors_written,
                        elapsed?,
                        512.0,
                    )
                }),
                read_operations_per_second: previous.and_then(|(previous, _)| {
                    counter_rate_scaled(
                        counters.reads_completed,
                        previous.reads_completed,
                        elapsed?,
                        1.0,
                    )
                }),
                write_operations_per_second: previous.and_then(|(previous, _)| {
                    counter_rate_scaled(
                        counters.writes_completed,
                        previous.writes_completed,
                        elapsed?,
                        1.0,
                    )
                }),
                busy_percent: previous.and_then(|(previous, _)| {
                    let elapsed_millis = elapsed?.as_secs_f64() * 1_000.0;
                    if elapsed_millis <= 0.0 {
                        return None;
                    }
                    let busy_millis = counters.io_millis.checked_sub(previous.io_millis)? as f64;
                    Some((busy_millis / elapsed_millis * 100.0).clamp(0.0, 100.0))
                }),
            };
            current_counters.insert(id, (counters, sampled_at));
            devices.push(metrics);
        }

        self.previous_block_device_counters = current_counters;
        let status = if devices.is_empty() {
            MetricStatus::TemporarilyUnavailable
        } else if devices.iter().all(|device| {
            device.read_bytes_per_second.is_none()
                && device.write_bytes_per_second.is_none()
                && device.read_operations_per_second.is_none()
                && device.write_operations_per_second.is_none()
                && device.busy_percent.is_none()
        }) {
            MetricStatus::WarmingUp
        } else {
            MetricStatus::Ok
        };
        (devices, status)
    }
}

fn counter_rate(current: u64, previous: u64, elapsed: Duration) -> Option<f64> {
    let elapsed_seconds = elapsed.as_secs_f64();
    if elapsed_seconds <= 0.0 {
        return None;
    }

    Some(current.checked_sub(previous)? as f64 / elapsed_seconds)
}

#[cfg(target_os = "linux")]
fn counter_rate_scaled(current: u64, previous: u64, elapsed: Duration, scale: f64) -> Option<f64> {
    let elapsed_seconds = elapsed.as_secs_f64();
    if elapsed_seconds <= 0.0 {
        return None;
    }

    Some(current.checked_sub(previous)? as f64 * scale / elapsed_seconds)
}

fn compare_cpu_usage(left: Option<f32>, right: Option<f32>) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => right.total_cmp(&left),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::counter_rate;
    #[cfg(target_os = "linux")]
    use super::counter_rate_scaled;
    use std::time::Duration;

    #[test]
    fn network_counter_rate_handles_deltas_resets_and_zero_elapsed_time() {
        assert_eq!(
            counter_rate(2_000, 1_000, Duration::from_secs(2)),
            Some(500.0)
        );
        assert_eq!(counter_rate(10, 20, Duration::from_secs(1)), None);
        assert_eq!(counter_rate(10, 0, Duration::ZERO), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn block_device_rate_uses_linux_sector_size_and_handles_counter_resets() {
        assert_eq!(
            counter_rate_scaled(12, 4, Duration::from_secs(2), 512.0),
            Some(2_048.0)
        );
        assert_eq!(
            counter_rate_scaled(4, 12, Duration::from_secs(2), 512.0),
            None
        );
        assert_eq!(counter_rate_scaled(12, 4, Duration::ZERO, 512.0), None);
    }
}
