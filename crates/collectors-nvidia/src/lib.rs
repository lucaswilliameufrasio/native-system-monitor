#[cfg(target_os = "linux")]
mod platform {
    use native_monitor_domain::{GpuMetrics, MetricStatus};
    use nvml_wrapper::Nvml;
    use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};

    pub struct NvidiaCollector {
        nvml: Option<Nvml>,
        available: bool,
    }

    impl NvidiaCollector {
        pub fn new() -> Self {
            let nvml = Nvml::init().ok();
            let available = nvml
                .as_ref()
                .is_some_and(|nvml| nvml.device_count().is_ok_and(|count| count > 0));
            Self { nvml, available }
        }

        pub fn is_available(&self) -> bool {
            self.available
        }

        pub fn collect(&self) -> (Vec<GpuMetrics>, MetricStatus) {
            let Some(nvml) = self.nvml.as_ref() else {
                return (Vec::new(), MetricStatus::Unsupported);
            };
            let device_count = match nvml.device_count() {
                Ok(count) => count,
                Err(_) => return (Vec::new(), MetricStatus::Error),
            };
            if device_count == 0 {
                return (Vec::new(), MetricStatus::Unsupported);
            }

            let mut gpus = Vec::with_capacity(device_count as usize);
            let mut had_device_error = false;
            for index in 0..device_count {
                let device = match nvml.device_by_index(index) {
                    Ok(device) => device,
                    Err(_) => {
                        had_device_error = true;
                        continue;
                    }
                };
                let id = match device.uuid() {
                    Ok(id) => id,
                    Err(_) => {
                        had_device_error = true;
                        continue;
                    }
                };
                let name = match device.name() {
                    Ok(name) => name,
                    Err(_) => {
                        had_device_error = true;
                        continue;
                    }
                };
                let memory = device.memory_info().ok();
                let utilization = device.utilization_rates().ok();
                let power_watts = device
                    .power_usage()
                    .ok()
                    .map(|milliwatts| f64::from(milliwatts) / 1_000.0);
                let metrics = GpuMetrics {
                    id,
                    name,
                    utilization_percent: utilization.map(|rates| rates.gpu as f32),
                    memory_used_bytes: memory.as_ref().map(|memory| memory.used),
                    memory_total_bytes: memory.as_ref().map(|memory| memory.total),
                    temperature_celsius: device.temperature(TemperatureSensor::Gpu).ok(),
                    graphics_clock_mhz: device.clock_info(Clock::Graphics).ok(),
                    memory_clock_mhz: device.clock_info(Clock::Memory).ok(),
                    power_watts,
                };
                gpus.push(metrics);
            }

            if gpus.is_empty() || had_device_error {
                return (gpus, MetricStatus::TemporarilyUnavailable);
            }
            (gpus, MetricStatus::Ok)
        }
    }

    impl Default for NvidiaCollector {
        fn default() -> Self {
            Self::new()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::NvidiaCollector;
        use native_monitor_domain::MetricStatus;

        #[test]
        fn missing_driver_does_not_panic_or_report_zero_devices_as_supported() {
            let collector = NvidiaCollector {
                nvml: None,
                available: false,
            };
            let (gpus, status) = collector.collect();
            assert!(gpus.is_empty());
            assert_eq!(status, MetricStatus::Unsupported);
        }

        #[test]
        fn nvml_collection_reports_present_hardware_without_fabricating_metrics() {
            let collector = NvidiaCollector::new();
            let (gpus, status) = collector.collect();
            if collector.nvml.is_none() {
                assert!(gpus.is_empty());
                assert_eq!(status, MetricStatus::Unsupported);
                return;
            }
            if gpus.is_empty() {
                assert_eq!(status, MetricStatus::Unsupported);
                return;
            }
            assert!(
                gpus.iter()
                    .all(|gpu| !gpu.id.is_empty() && !gpu.name.is_empty())
            );
            assert!(matches!(
                status,
                MetricStatus::Ok | MetricStatus::TemporarilyUnavailable
            ));
        }
    }
}

#[cfg(target_os = "linux")]
pub use platform::NvidiaCollector;

#[cfg(not(target_os = "linux"))]
pub struct NvidiaCollector;

#[cfg(not(target_os = "linux"))]
impl NvidiaCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn collect(
        &self,
    ) -> (
        Vec<native_monitor_domain::GpuMetrics>,
        native_monitor_domain::MetricStatus,
    ) {
        (Vec::new(), native_monitor_domain::MetricStatus::Unsupported)
    }

    pub fn is_available(&self) -> bool {
        false
    }
}

#[cfg(not(target_os = "linux"))]
impl Default for NvidiaCollector {
    fn default() -> Self {
        Self::new()
    }
}
