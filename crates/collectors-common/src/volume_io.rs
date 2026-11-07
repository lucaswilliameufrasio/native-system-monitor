use native_monitor_domain::VolumeMetrics;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Hash, PartialEq, Eq)]
pub(super) struct VolumeIdentity {
    pub mount_point: PathBuf,
    pub file_system: OsString,
    pub device_name: OsString,
}

pub(super) struct VolumeSample {
    pub identity: VolumeIdentity,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub read_bytes: u64,
    pub written_bytes: u64,
}

struct Counters {
    read_bytes: u64,
    written_bytes: u64,
    sampled_at: Instant,
}

#[derive(Default)]
pub(super) struct VolumeIoTracker {
    previous: HashMap<VolumeIdentity, Counters>,
}

impl VolumeIoTracker {
    pub fn collect(
        &mut self,
        samples: impl IntoIterator<Item = VolumeSample>,
        sampled_at: Instant,
    ) -> Vec<VolumeMetrics> {
        let mut current = HashMap::new();
        let mut volumes = Vec::new();
        for sample in samples {
            let previous = self.previous.get(&sample.identity);
            let elapsed =
                previous.and_then(|counter| sampled_at.checked_duration_since(counter.sampled_at));
            volumes.push(VolumeMetrics {
                mount_point: sample.identity.mount_point.to_string_lossy().into_owned(),
                file_system: sample.identity.file_system.to_string_lossy().into_owned(),
                total_bytes: (sample.total_bytes > 0).then_some(sample.total_bytes),
                available_bytes: (sample.total_bytes > 0).then_some(sample.available_bytes),
                read_bytes_per_second: previous.and_then(|counter| {
                    super::counter_rate(sample.read_bytes, counter.read_bytes, elapsed?)
                }),
                write_bytes_per_second: previous.and_then(|counter| {
                    super::counter_rate(sample.written_bytes, counter.written_bytes, elapsed?)
                }),
            });
            current.insert(
                sample.identity,
                Counters {
                    read_bytes: sample.read_bytes,
                    written_bytes: sample.written_bytes,
                    sampled_at,
                },
            );
        }
        // Only volumes observed in this cycle may provide the next baseline.
        self.previous = current;
        volumes
    }
}

#[cfg(test)]
mod tests {
    use super::{VolumeIdentity, VolumeIoTracker, VolumeSample};
    use std::time::{Duration, Instant};

    fn sample(mount: &str, device: &str, read: u64, written: u64) -> VolumeSample {
        VolumeSample {
            identity: VolumeIdentity {
                mount_point: mount.into(),
                file_system: "testfs".into(),
                device_name: device.into(),
            },
            total_bytes: 10_000,
            available_bytes: 4_000,
            read_bytes: read,
            written_bytes: written,
        }
    }

    #[test]
    fn hotplug_preserves_continuous_rates_and_warms_new_volumes() {
        let mut tracker = VolumeIoTracker::default();
        let start = Instant::now();
        let first = tracker.collect([sample("/a", "a", 1_000, 2_000)], start);
        assert_eq!(first[0].read_bytes_per_second, None);
        assert_eq!(first[0].write_bytes_per_second, None);
        assert_eq!(first[0].total_bytes, Some(10_000));
        assert_eq!(first[0].available_bytes, Some(4_000));

        let second = tracker.collect(
            [
                sample("/a", "a", 2_000, 4_000),
                sample("/b", "b", 9_000, 8_000),
            ],
            start + Duration::from_secs(2),
        );
        assert_eq!(second[0].read_bytes_per_second, Some(500.0));
        assert_eq!(second[0].write_bytes_per_second, Some(1_000.0));
        assert_eq!(second[1].read_bytes_per_second, None);
        assert_eq!(second[1].write_bytes_per_second, None);

        let third = tracker.collect(
            [sample("/b", "b", 10_000, 8_000)],
            start + Duration::from_secs(3),
        );
        assert_eq!(third.len(), 1);
        assert_eq!(third[0].mount_point, "/b");
        assert_eq!(third[0].read_bytes_per_second, Some(1_000.0));
        assert_eq!(third[0].write_bytes_per_second, Some(0.0));
        assert_eq!(tracker.previous.len(), 1);

        let fourth = tracker.collect(
            [sample("/a", "a", 20_000, 30_000)],
            start + Duration::from_secs(4),
        );
        assert_eq!(fourth[0].read_bytes_per_second, None);
        assert_eq!(fourth[0].write_bytes_per_second, None);
        assert!(
            tracker
                .collect([], start + Duration::from_secs(5))
                .is_empty()
        );
        assert!(tracker.previous.is_empty());
    }

    #[test]
    fn replacement_device_or_filesystem_at_same_mount_starts_a_new_baseline() {
        let mut tracker = VolumeIoTracker::default();
        let start = Instant::now();
        tracker.collect([sample("/a", "a", 100, 200)], start);
        let replacement = tracker.collect(
            [sample("/a", "b", 1_000, 2_000)],
            start + Duration::from_secs(1),
        );
        assert_eq!(replacement[0].read_bytes_per_second, None);
        assert_eq!(replacement[0].write_bytes_per_second, None);
        let mut changed = sample("/a", "b", 2_000, 4_000);
        changed.identity.file_system = "otherfs".into();
        changed.total_bytes = 0;
        let changed = tracker.collect([changed], start + Duration::from_secs(2));
        assert_eq!(changed[0].read_bytes_per_second, None);
        assert_eq!(changed[0].write_bytes_per_second, None);
        assert_eq!(changed[0].total_bytes, None);
        assert_eq!(changed[0].available_bytes, None);
    }

    #[test]
    fn counter_resets_and_invalid_intervals_remain_absent_then_recover() {
        let mut tracker = VolumeIoTracker::default();
        let start = Instant::now();
        tracker.collect([sample("/a", "a", 1_000, 2_000)], start);
        let reset = tracker.collect(
            [sample("/a", "a", 10, 3_000)],
            start + Duration::from_secs(1),
        );
        assert_eq!(reset[0].read_bytes_per_second, None);
        assert_eq!(reset[0].write_bytes_per_second, Some(1_000.0));
        let recovered =
            tracker.collect([sample("/a", "a", 110, 20)], start + Duration::from_secs(2));
        assert_eq!(recovered[0].read_bytes_per_second, Some(100.0));
        assert_eq!(recovered[0].write_bytes_per_second, None);
        for sampled_at in [start + Duration::from_secs(2), start] {
            let invalid = tracker.collect([sample("/a", "a", 200, 200)], sampled_at);
            assert_eq!(invalid[0].read_bytes_per_second, None);
            assert_eq!(invalid[0].write_bytes_per_second, None);
        }
    }
}
