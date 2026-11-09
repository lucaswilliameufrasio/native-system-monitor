use native_monitor_domain::NetworkInterfaceKind;
use std::fs;
use std::io;
use std::path::Path;

mod amdgpu;
#[cfg(target_os = "linux")]
mod intel_gpu;
#[cfg(target_os = "linux")]
#[allow(unsafe_code)] // Isolated dynamic Level Zero FFI boundary; see intel_gpu_ffi.rs.
mod intel_gpu_ffi;
mod power_supply;

pub use amdgpu::read_system_amd_gpus;
#[cfg(target_os = "linux")]
pub use intel_gpu::IntelCollector;
pub use power_supply::read_system_batteries;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiskCounters {
    pub major: u32,
    pub minor: u32,
    pub name: String,
    pub reads_completed: u64,
    pub sectors_read: u64,
    pub read_millis: u64,
    pub writes_completed: u64,
    pub sectors_written: u64,
    pub write_millis: u64,
    pub io_millis: u64,
}

impl DiskCounters {
    pub fn id(&self) -> String {
        format!("{}:{}", self.major, self.minor)
    }
}

pub fn read_system_disk_counters() -> io::Result<Vec<DiskCounters>> {
    let diskstats = fs::read_to_string("/proc/diskstats")?;
    let mut counters = parse_diskstats(&diskstats);
    counters.retain(|disk| is_whole_non_stacked_device(disk, Path::new("/sys/dev/block")));
    Ok(counters)
}

const IFF_LOOPBACK: u32 = 0x8;

pub fn classify_network_interface(name: &str) -> NetworkInterfaceKind {
    classify_network_interface_from_sysfs(name, Path::new("/sys/class/net"))
}

fn classify_network_interface_from_sysfs(name: &str, sysfs_root: &Path) -> NetworkInterfaceKind {
    let flags_path = sysfs_root.join(name).join("flags");
    let Ok(contents) = fs::read_to_string(flags_path) else {
        return NetworkInterfaceKind::Unknown;
    };
    let Some(flags) = parse_network_interface_flags(&contents) else {
        return NetworkInterfaceKind::Unknown;
    };

    classify_network_interface_flags(flags)
}

fn parse_network_interface_flags(contents: &str) -> Option<u32> {
    let contents = contents.trim();
    let digits = contents
        .strip_prefix("0x")
        .or_else(|| contents.strip_prefix("0X"))
        .unwrap_or(contents);

    u32::from_str_radix(digits, 16).ok()
}

fn classify_network_interface_flags(flags: u32) -> NetworkInterfaceKind {
    if flags & IFF_LOOPBACK != 0 {
        return NetworkInterfaceKind::Loopback;
    }

    NetworkInterfaceKind::Unknown
}

pub fn parse_diskstats(contents: &str) -> Vec<DiskCounters> {
    let mut counters = Vec::new();

    for line in contents.lines() {
        if let Some(disk) = parse_diskstats_line(line) {
            counters.push(disk);
        }
    }

    counters
}

fn parse_diskstats_line(line: &str) -> Option<DiskCounters> {
    let mut fields = line.split_whitespace();
    let major = fields.next()?.parse().ok()?;
    let minor = fields.next()?.parse().ok()?;
    let name = fields.next()?.to_owned();
    let reads_completed = fields.next()?.parse().ok()?;
    let _reads_merged = fields.next()?.parse::<u64>().ok()?;
    let sectors_read = fields.next()?.parse().ok()?;
    let read_millis = fields.next()?.parse().ok()?;
    let writes_completed = fields.next()?.parse().ok()?;
    let _writes_merged = fields.next()?.parse::<u64>().ok()?;
    let sectors_written = fields.next()?.parse().ok()?;
    let write_millis = fields.next()?.parse().ok()?;
    let _io_in_progress = fields.next()?.parse::<u64>().ok()?;
    let io_millis = fields.next()?.parse().ok()?;

    Some(DiskCounters {
        major,
        minor,
        name,
        reads_completed,
        sectors_read,
        read_millis,
        writes_completed,
        sectors_written,
        write_millis,
        io_millis,
    })
}

fn is_whole_non_stacked_device(disk: &DiskCounters, sysfs_root: &Path) -> bool {
    let device_path = sysfs_root.join(disk.id());
    if !device_path.is_dir() {
        return false;
    }
    if device_path.join("partition").exists() {
        return false;
    }

    match fs::read_dir(device_path.join("slaves")) {
        Ok(mut entries) => entries.next().is_none(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => true,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use super::classify_network_interface_from_sysfs;
    #[cfg(target_os = "linux")]
    use super::is_whole_non_stacked_device;
    use super::{DiskCounters, parse_diskstats};
    use super::{classify_network_interface_flags, parse_network_interface_flags};
    use native_monitor_domain::NetworkInterfaceKind;
    #[cfg(target_os = "linux")]
    use std::fs;
    #[cfg(target_os = "linux")]
    use std::sync::atomic::{AtomicU64, Ordering};

    #[cfg(target_os = "linux")]
    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn parses_counters_and_skips_malformed_diskstats_lines() {
        let contents = "\
             8       0 sda 100 2 300 4 50 1 600 7 0 80 90\n\
             malformed line\n\
             8       1 sda1 10 0 20 0 5 0 30 0 0 4 5\n";

        assert_eq!(
            parse_diskstats(contents),
            vec![
                DiskCounters {
                    major: 8,
                    minor: 0,
                    name: "sda".to_owned(),
                    reads_completed: 100,
                    sectors_read: 300,
                    read_millis: 4,
                    writes_completed: 50,
                    sectors_written: 600,
                    write_millis: 7,
                    io_millis: 80,
                },
                DiskCounters {
                    major: 8,
                    minor: 1,
                    name: "sda1".to_owned(),
                    reads_completed: 10,
                    sectors_read: 20,
                    read_millis: 0,
                    writes_completed: 5,
                    sectors_written: 30,
                    write_millis: 0,
                    io_millis: 4,
                },
            ]
        );
    }

    #[test]
    fn classifies_loopback_only_from_the_native_interface_flag() {
        let loopback_flags = parse_network_interface_flags("0x1009\n")
            .expect("parse Linux loopback interface flags");
        let ethernet_flags = parse_network_interface_flags("0x1003\n")
            .expect("parse Linux non-loopback interface flags");

        assert_eq!(
            classify_network_interface_flags(loopback_flags),
            NetworkInterfaceKind::Loopback
        );
        assert_eq!(
            classify_network_interface_flags(ethernet_flags),
            NetworkInterfaceKind::Unknown
        );
    }

    #[test]
    fn malformed_network_flags_remain_unknown() {
        assert_eq!(parse_network_interface_flags("not-a-flag"), None);
        assert_eq!(
            classify_network_interface_flags(0),
            NetworkInterfaceKind::Unknown
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn sysfs_classifier_uses_flags_not_names_and_defaults_missing_flags_to_unknown() {
        let root = std::env::temp_dir().join(format!(
            "native-monitor-netflags-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("lo")).expect("create loopback sysfs fixture");
        fs::write(root.join("lo/flags"), "0x1003\n").expect("write non-loopback flags");
        fs::create_dir_all(root.join("eth-test")).expect("create ethernet sysfs fixture");
        fs::write(root.join("eth-test/flags"), "0x1009\n").expect("write loopback flags");

        assert_eq!(
            classify_network_interface_from_sysfs("lo", &root),
            NetworkInterfaceKind::Unknown
        );
        assert_eq!(
            classify_network_interface_from_sysfs("eth-test", &root),
            NetworkInterfaceKind::Loopback
        );
        assert_eq!(
            classify_network_interface_from_sysfs("missing", &root),
            NetworkInterfaceKind::Unknown
        );

        fs::remove_dir_all(root).expect("remove only this test's sysfs fixture");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn skips_partitions_and_stacked_devices_without_following_device_names() {
        let root = std::env::temp_dir().join(format!(
            "native-monitor-diskstats-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let sysfs = root.join("dev/block");
        fs::create_dir_all(sysfs.join("8:0/slaves")).expect("create whole-device sysfs fixture");
        fs::create_dir_all(sysfs.join("8:1/slaves")).expect("create partition sysfs fixture");
        fs::create_dir_all(sysfs.join("253:0/slaves/dm-0")).expect("create stacked sysfs fixture");
        fs::write(sysfs.join("8:1/partition"), "1").expect("mark partition fixture");

        assert!(is_whole_non_stacked_device(
            &DiskCounters {
                major: 8,
                minor: 0,
                name: "sda".to_owned(),
                reads_completed: 0,
                sectors_read: 0,
                read_millis: 0,
                writes_completed: 0,
                sectors_written: 0,
                write_millis: 0,
                io_millis: 0,
            },
            &sysfs,
        ));
        assert!(!is_whole_non_stacked_device(
            &DiskCounters {
                major: 8,
                minor: 1,
                name: "sda1".to_owned(),
                reads_completed: 0,
                sectors_read: 0,
                read_millis: 0,
                writes_completed: 0,
                sectors_written: 0,
                write_millis: 0,
                io_millis: 0,
            },
            &sysfs,
        ));
        assert!(!is_whole_non_stacked_device(
            &DiskCounters {
                major: 253,
                minor: 0,
                name: "dm-0".to_owned(),
                reads_completed: 0,
                sectors_read: 0,
                read_millis: 0,
                writes_completed: 0,
                sectors_written: 0,
                write_millis: 0,
                io_millis: 0,
            },
            &sysfs,
        ));

        fs::remove_dir_all(&root).expect("remove only this test's sysfs fixture");
    }
}
