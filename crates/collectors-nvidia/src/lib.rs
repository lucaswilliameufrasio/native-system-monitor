#[cfg(target_os = "linux")]
mod platform {
    use native_monitor_domain::{GpuMetrics, MetricStatus};
    use nvml_wrapper::Nvml;
    use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};

    #[derive(Debug)]
    struct NvidiaSourceError;

    trait NvidiaDeviceSource {
        fn device_count(&self) -> Result<u32, NvidiaSourceError>;

        fn collect_device(&self, index: u32) -> Result<GpuMetrics, NvidiaSourceError>;
    }

    impl NvidiaDeviceSource for Nvml {
        fn device_count(&self) -> Result<u32, NvidiaSourceError> {
            Nvml::device_count(self).map_err(|_| NvidiaSourceError)
        }

        fn collect_device(&self, index: u32) -> Result<GpuMetrics, NvidiaSourceError> {
            let device = self.device_by_index(index).map_err(|_| NvidiaSourceError)?;
            let id = device.uuid().map_err(|_| NvidiaSourceError)?;
            let name = device.name().map_err(|_| NvidiaSourceError)?;
            let memory = device.memory_info().ok();
            let utilization = device.utilization_rates().ok();
            let power_watts = device
                .power_usage()
                .ok()
                .map(|milliwatts| f64::from(milliwatts) / 1_000.0);

            Ok(GpuMetrics {
                id,
                name,
                utilization_percent: utilization.map(|rates| rates.gpu as f32),
                memory_used_bytes: memory.as_ref().map(|memory| memory.used),
                memory_total_bytes: memory.as_ref().map(|memory| memory.total),
                temperature_celsius: device.temperature(TemperatureSensor::Gpu).ok(),
                graphics_clock_mhz: device.clock_info(Clock::Graphics).ok(),
                memory_clock_mhz: device.clock_info(Clock::Memory).ok(),
                power_watts,
            })
        }
    }

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
            collect_devices(nvml)
        }
    }

    impl Default for NvidiaCollector {
        fn default() -> Self {
            Self::new()
        }
    }

    fn collect_devices(source: &impl NvidiaDeviceSource) -> (Vec<GpuMetrics>, MetricStatus) {
        let device_count = match source.device_count() {
            Ok(count) => count,
            Err(_) => return (Vec::new(), MetricStatus::Error),
        };
        if device_count == 0 {
            return (Vec::new(), MetricStatus::Unsupported);
        }

        let mut gpus = Vec::with_capacity(device_count as usize);
        let mut had_device_error = false;
        for index in 0..device_count {
            match source.collect_device(index) {
                Ok(gpu) => gpus.push(gpu),
                Err(_) => had_device_error = true,
            }
        }

        if gpus.is_empty() || had_device_error {
            return (gpus, MetricStatus::TemporarilyUnavailable);
        }

        (gpus, MetricStatus::Ok)
    }

    #[cfg(test)]
    mod tests {
        use super::{NvidiaCollector, NvidiaDeviceSource, NvidiaSourceError, collect_devices};
        use native_monitor_domain::{GpuMetrics, MetricStatus};

        struct FakeNvidiaSource {
            device_count: Result<u32, NvidiaSourceError>,
            devices: Vec<Result<GpuMetrics, NvidiaSourceError>>,
        }

        impl NvidiaDeviceSource for FakeNvidiaSource {
            fn device_count(&self) -> Result<u32, NvidiaSourceError> {
                match self.device_count {
                    Ok(count) => Ok(count),
                    Err(_) => Err(NvidiaSourceError),
                }
            }

            fn collect_device(&self, index: u32) -> Result<GpuMetrics, NvidiaSourceError> {
                match self.devices.get(index as usize) {
                    Some(Ok(gpu)) => Ok(gpu.clone()),
                    Some(Err(_)) | None => Err(NvidiaSourceError),
                }
            }
        }

        fn gpu(id: &str, name: &str) -> GpuMetrics {
            GpuMetrics {
                id: id.to_owned(),
                name: name.to_owned(),
                utilization_percent: None,
                memory_used_bytes: None,
                memory_total_bytes: None,
                temperature_celsius: None,
                graphics_clock_mhz: None,
                memory_clock_mhz: None,
                power_watts: None,
            }
        }

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
        fn multi_gpu_collection_keeps_distinct_ids_and_absent_optional_fields() {
            let source = FakeNvidiaSource {
                device_count: Ok(2),
                devices: vec![
                    Ok(gpu("GPU-uuid-1", "NVIDIA A")),
                    Ok(gpu("GPU-uuid-2", "NVIDIA B")),
                ],
            };

            let (gpus, status) = collect_devices(&source);

            assert_eq!(status, MetricStatus::Ok);
            assert_eq!(gpus.len(), 2);
            assert_ne!(gpus[0].id, gpus[1].id);
            assert!(gpus.iter().all(|gpu| gpu.power_watts.is_none()));
            assert!(gpus.iter().all(|gpu| gpu.memory_total_bytes.is_none()));
        }

        #[test]
        fn partial_device_failure_preserves_valid_gpu_and_reports_degraded_status() {
            let source = FakeNvidiaSource {
                device_count: Ok(2),
                devices: vec![Ok(gpu("GPU-uuid-1", "NVIDIA A")), Err(NvidiaSourceError)],
            };

            let (gpus, status) = collect_devices(&source);

            assert_eq!(gpus.len(), 1);
            assert_eq!(gpus[0].id, "GPU-uuid-1");
            assert_eq!(status, MetricStatus::TemporarilyUnavailable);
        }

        #[test]
        fn failed_collection_of_every_device_reports_temporary_unavailability() {
            let source = FakeNvidiaSource {
                device_count: Ok(2),
                devices: vec![Err(NvidiaSourceError), Err(NvidiaSourceError)],
            };

            let (gpus, status) = collect_devices(&source);

            assert!(gpus.is_empty());
            assert_eq!(status, MetricStatus::TemporarilyUnavailable);
        }

        #[test]
        fn nvml_failure_and_zero_devices_have_distinct_statuses() {
            let failed_source = FakeNvidiaSource {
                device_count: Err(NvidiaSourceError),
                devices: Vec::new(),
            };
            let empty_source = FakeNvidiaSource {
                device_count: Ok(0),
                devices: Vec::new(),
            };

            let (failed_gpus, failed_status) = collect_devices(&failed_source);
            let (empty_gpus, empty_status) = collect_devices(&empty_source);

            assert!(failed_gpus.is_empty());
            assert_eq!(failed_status, MetricStatus::Error);
            assert!(empty_gpus.is_empty());
            assert_eq!(empty_status, MetricStatus::Unsupported);
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
