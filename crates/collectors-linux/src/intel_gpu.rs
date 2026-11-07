use crate::intel_gpu_ffi::LevelZeroApi;
use native_monitor_domain::{GpuMetrics, MetricStatus};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct EngineCounter {
    pub handle: usize,
    pub active_time: u64,
    pub timestamp: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct EnergyCounter {
    pub handle: usize,
    pub energy_microjoules: u64,
    pub timestamp_microseconds: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RawGpuSample {
    pub id: String,
    pub name: String,
    pub engine: Option<EngineCounter>,
    pub memory_used_bytes: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub temperature_celsius: Option<u32>,
    pub graphics_clock_mhz: Option<u32>,
    pub power: Option<EnergyCounter>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct IntelSourceSnapshot {
    pub devices: Vec<RawGpuSample>,
    pub had_device_error: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceError;

trait IntelDeviceSource: Send {
    fn collect(&mut self) -> Result<IntelSourceSnapshot, SourceError>;
}

impl IntelDeviceSource for LevelZeroApi {
    fn collect(&mut self) -> Result<IntelSourceSnapshot, SourceError> {
        self.collect_devices().map_err(|()| SourceError)
    }
}

pub struct IntelCollector {
    source: Option<Box<dyn IntelDeviceSource>>,
    previous_engines: HashMap<(String, usize), (u64, u64)>,
    previous_energy: HashMap<(String, usize), (u64, u64)>,
}

impl IntelCollector {
    pub fn new() -> Self {
        let source = LevelZeroApi::load()
            .ok()
            .map(|api| Box::new(api) as Box<dyn IntelDeviceSource>);
        Self {
            source,
            previous_engines: HashMap::new(),
            previous_energy: HashMap::new(),
        }
    }

    #[cfg(test)]
    fn with_source(source: Option<Box<dyn IntelDeviceSource>>) -> Self {
        Self {
            source,
            previous_engines: HashMap::new(),
            previous_energy: HashMap::new(),
        }
    }

    pub fn collect(&mut self) -> (Vec<GpuMetrics>, MetricStatus) {
        let Some(source) = self.source.as_mut() else {
            return (Vec::new(), MetricStatus::Unsupported);
        };

        let snapshot = match source.collect() {
            Ok(snapshot) => snapshot,
            Err(_) => return (Vec::new(), MetricStatus::Error),
        };
        if snapshot.devices.is_empty() {
            let status = if snapshot.had_device_error {
                MetricStatus::TemporarilyUnavailable
            } else {
                MetricStatus::Unsupported
            };
            self.previous_engines.clear();
            self.previous_energy.clear();
            return (Vec::new(), status);
        }

        let mut devices = Vec::with_capacity(snapshot.devices.len());
        let mut seen_ids = HashSet::with_capacity(snapshot.devices.len());
        let mut next_engines = HashMap::with_capacity(snapshot.devices.len());
        let mut next_energy = HashMap::with_capacity(snapshot.devices.len());
        let mut had_device_error = snapshot.had_device_error;

        for sample in snapshot.devices {
            if !seen_ids.insert(sample.id.clone()) {
                had_device_error = true;
                continue;
            }

            let utilization_percent = sample.engine.and_then(|counter| {
                let key = (sample.id.clone(), counter.handle);
                let previous = self.previous_engines.get(&key).copied();
                next_engines.insert(key, (counter.active_time, counter.timestamp));
                previous.and_then(|previous| {
                    engine_utilization_percent(previous, (counter.active_time, counter.timestamp))
                })
            });
            let power_watts = sample.power.and_then(|counter| {
                let key = (sample.id.clone(), counter.handle);
                let previous = self.previous_energy.get(&key).copied();
                next_energy.insert(
                    key,
                    (counter.energy_microjoules, counter.timestamp_microseconds),
                );
                previous.and_then(|previous| {
                    energy_delta_watts(
                        previous,
                        (counter.energy_microjoules, counter.timestamp_microseconds),
                    )
                })
            });

            devices.push(GpuMetrics {
                id: sample.id,
                name: sample.name,
                utilization_percent,
                memory_used_bytes: sample.memory_used_bytes,
                memory_total_bytes: sample.memory_total_bytes,
                temperature_celsius: sample.temperature_celsius,
                graphics_clock_mhz: sample.graphics_clock_mhz,
                // Memory-clock units need hardware/driver validation before exposure.
                memory_clock_mhz: None,
                power_watts,
            });
        }

        self.previous_engines = next_engines;
        self.previous_energy = next_energy;
        if devices.is_empty() {
            return (devices, MetricStatus::TemporarilyUnavailable);
        }

        let status = if had_device_error {
            MetricStatus::TemporarilyUnavailable
        } else {
            MetricStatus::Ok
        };
        (devices, status)
    }
}

impl Default for IntelCollector {
    fn default() -> Self {
        Self::new()
    }
}

fn engine_utilization_percent(previous: (u64, u64), current: (u64, u64)) -> Option<f32> {
    let active_delta = current.0.checked_sub(previous.0)?;
    let time_delta = current.1.checked_sub(previous.1)?;
    if time_delta == 0 {
        return None;
    }

    let percent = (active_delta as f64 / time_delta as f64) * 100.0;
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
        return None;
    }
    Some(percent as f32)
}

fn energy_delta_watts(previous: (u64, u64), current: (u64, u64)) -> Option<f64> {
    let energy_delta = current.0.checked_sub(previous.0)?;
    let time_delta = current.1.checked_sub(previous.1)?;
    if time_delta == 0 {
        return None;
    }

    let watts = energy_delta as f64 / time_delta as f64;
    if !watts.is_finite() {
        return None;
    }
    Some(watts)
}

#[cfg(test)]
mod tests {
    use super::{
        EnergyCounter, EngineCounter, IntelCollector, IntelDeviceSource, IntelSourceSnapshot,
        RawGpuSample, SourceError, energy_delta_watts, engine_utilization_percent,
    };
    use native_monitor_domain::MetricStatus;
    use std::collections::VecDeque;

    struct FakeSource {
        snapshots: VecDeque<Result<IntelSourceSnapshot, SourceError>>,
    }

    impl IntelDeviceSource for FakeSource {
        fn collect(&mut self) -> Result<IntelSourceSnapshot, SourceError> {
            self.snapshots.pop_front().unwrap_or(Err(SourceError))
        }
    }

    fn sample(id: &str, handle: usize) -> RawGpuSample {
        RawGpuSample {
            id: id.to_owned(),
            name: format!("Intel GPU {id}"),
            engine: Some(EngineCounter {
                handle,
                active_time: 0,
                timestamp: 0,
            }),
            memory_used_bytes: None,
            memory_total_bytes: None,
            temperature_celsius: None,
            graphics_clock_mhz: None,
            power: None,
        }
    }

    #[test]
    fn missing_runtime_and_empty_enumeration_are_unsupported_without_fake_zeros() {
        let mut no_runtime = IntelCollector::with_source(None);
        let (gpus, status) = no_runtime.collect();
        assert!(gpus.is_empty());
        assert_eq!(status, MetricStatus::Unsupported);

        let mut no_devices = IntelCollector::with_source(Some(Box::new(FakeSource {
            snapshots: VecDeque::from([Ok(IntelSourceSnapshot::default())]),
        })));
        let (gpus, status) = no_devices.collect();
        assert!(gpus.is_empty());
        assert_eq!(status, MetricStatus::Unsupported);
    }

    #[test]
    fn multi_gpu_samples_keep_ids_optional_fields_and_counter_baselines_separate() {
        let mut first = sample("uuid-a", 10);
        first.engine = Some(EngineCounter {
            handle: 10,
            active_time: 20,
            timestamp: 100,
        });
        first.power = Some(EnergyCounter {
            handle: 20,
            energy_microjoules: 1_000,
            timestamp_microseconds: 500,
        });
        first.memory_used_bytes = Some(2_000);
        first.memory_total_bytes = Some(8_000);
        first.temperature_celsius = Some(61);
        first.graphics_clock_mhz = Some(1_500);
        let mut second = sample("uuid-b", 11);
        second.engine = Some(EngineCounter {
            handle: 11,
            active_time: 50,
            timestamp: 100,
        });
        let mut next_first = first.clone();
        next_first.engine = Some(EngineCounter {
            handle: 10,
            active_time: 70,
            timestamp: 200,
        });
        next_first.power = Some(EnergyCounter {
            handle: 20,
            energy_microjoules: 2_500,
            timestamp_microseconds: 1_000,
        });
        let mut next_second = second.clone();
        next_second.engine = Some(EngineCounter {
            handle: 11,
            active_time: 60,
            timestamp: 200,
        });
        let mut collector = IntelCollector::with_source(Some(Box::new(FakeSource {
            snapshots: VecDeque::from([
                Ok(IntelSourceSnapshot {
                    devices: vec![first, second],
                    had_device_error: false,
                }),
                Ok(IntelSourceSnapshot {
                    devices: vec![next_first, next_second],
                    had_device_error: false,
                }),
            ]),
        })));

        let (first_gpus, first_status) = collector.collect();
        assert_eq!(first_status, MetricStatus::Ok);
        assert_eq!(first_gpus.len(), 2);
        assert_ne!(first_gpus[0].id, first_gpus[1].id);
        assert!(
            first_gpus
                .iter()
                .all(|gpu| gpu.utilization_percent.is_none())
        );
        assert!(first_gpus.iter().all(|gpu| gpu.power_watts.is_none()));
        assert!(first_gpus.iter().all(|gpu| gpu.memory_clock_mhz.is_none()));
        assert_eq!(first_gpus[0].memory_used_bytes, Some(2_000));
        assert_eq!(first_gpus[0].memory_total_bytes, Some(8_000));
        assert_eq!(first_gpus[0].temperature_celsius, Some(61));
        assert_eq!(first_gpus[0].graphics_clock_mhz, Some(1_500));

        let (next_gpus, next_status) = collector.collect();
        assert_eq!(next_status, MetricStatus::Ok);
        assert_eq!(next_gpus[0].utilization_percent, Some(50.0));
        assert_eq!(next_gpus[0].power_watts, Some(3.0));
        assert_eq!(next_gpus[1].utilization_percent, Some(10.0));
        assert_eq!(next_gpus[1].power_watts, None);
    }

    #[test]
    fn invalid_counter_deltas_are_absent_and_current_values_become_new_baselines() {
        assert_eq!(engine_utilization_percent((10, 20), (20, 20)), None);
        assert_eq!(engine_utilization_percent((20, 20), (10, 30)), None);
        assert_eq!(engine_utilization_percent((10, 20), (120, 30)), None);
        assert_eq!(energy_delta_watts((10, 20), (10, 20)), None);
        assert_eq!(energy_delta_watts((20, 20), (10, 30)), None);
        assert_eq!(energy_delta_watts((10, 20), (20, 20)), None);
        assert_eq!(engine_utilization_percent((10, 20), (20, 30)), Some(100.0));
        assert_eq!(energy_delta_watts((10, 20), (20, 30)), Some(1.0));
    }

    #[test]
    fn counter_regressions_reset_baselines_before_rates_resume() {
        let mut first = sample("uuid-reset", 1);
        first.engine = Some(EngineCounter {
            handle: 1,
            active_time: 100,
            timestamp: 100,
        });
        let mut reset = first.clone();
        reset.engine = Some(EngineCounter {
            handle: 1,
            active_time: 10,
            timestamp: 50,
        });
        let mut after_reset = first.clone();
        after_reset.engine = Some(EngineCounter {
            handle: 1,
            active_time: 30,
            timestamp: 70,
        });
        let mut collector = IntelCollector::with_source(Some(Box::new(FakeSource {
            snapshots: VecDeque::from([
                Ok(IntelSourceSnapshot {
                    devices: vec![first],
                    had_device_error: false,
                }),
                Ok(IntelSourceSnapshot {
                    devices: vec![reset],
                    had_device_error: false,
                }),
                Ok(IntelSourceSnapshot {
                    devices: vec![after_reset],
                    had_device_error: false,
                }),
            ]),
        })));

        assert_eq!(collector.collect().0[0].utilization_percent, None);
        assert_eq!(collector.collect().0[0].utilization_percent, None);
        assert_eq!(collector.collect().0[0].utilization_percent, Some(100.0));
    }

    #[test]
    fn device_failures_preserve_valid_gpus_and_degrade_status() {
        let mut collector = IntelCollector::with_source(Some(Box::new(FakeSource {
            snapshots: VecDeque::from([Ok(IntelSourceSnapshot {
                devices: vec![sample("valid", 1)],
                had_device_error: true,
            })]),
        })));

        let (gpus, status) = collector.collect();

        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].id, "valid");
        assert_eq!(status, MetricStatus::TemporarilyUnavailable);
    }

    #[test]
    fn duplicate_device_ids_are_not_emitted_twice() {
        let mut collector = IntelCollector::with_source(Some(Box::new(FakeSource {
            snapshots: VecDeque::from([Ok(IntelSourceSnapshot {
                devices: vec![sample("duplicate", 1), sample("duplicate", 2)],
                had_device_error: false,
            })]),
        })));

        let (gpus, status) = collector.collect();

        assert_eq!(gpus.len(), 1);
        assert_eq!(status, MetricStatus::TemporarilyUnavailable);
    }

    #[test]
    fn global_runtime_errors_are_reported_without_panicking() {
        let mut collector = IntelCollector::with_source(Some(Box::new(FakeSource {
            snapshots: VecDeque::from([Err(SourceError)]),
        })));

        let (gpus, status) = collector.collect();

        assert!(gpus.is_empty());
        assert_eq!(status, MetricStatus::Error);
    }
}
