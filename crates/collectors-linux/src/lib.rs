use std::fs;
use std::io;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiskCounters {
    pub major: u32,
    pub minor: u32,
    pub name: String,
    pub reads_completed: u64,
    pub sectors_read: u64,
    pub writes_completed: u64,
    pub sectors_written: u64,
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
    let _read_millis = fields.next()?.parse::<u64>().ok()?;
    let writes_completed = fields.next()?.parse().ok()?;
    let _writes_merged = fields.next()?.parse::<u64>().ok()?;
    let sectors_written = fields.next()?.parse().ok()?;
    let _write_millis = fields.next()?.parse::<u64>().ok()?;
    let _io_in_progress = fields.next()?.parse::<u64>().ok()?;
    let io_millis = fields.next()?.parse().ok()?;

    Some(DiskCounters {
        major,
        minor,
        name,
        reads_completed,
        sectors_read,
        writes_completed,
        sectors_written,
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
    use super::{DiskCounters, is_whole_non_stacked_device, parse_diskstats};
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

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
                    writes_completed: 50,
                    sectors_written: 600,
                    io_millis: 80,
                },
                DiskCounters {
                    major: 8,
                    minor: 1,
                    name: "sda1".to_owned(),
                    reads_completed: 10,
                    sectors_read: 20,
                    writes_completed: 5,
                    sectors_written: 30,
                    io_millis: 4,
                },
            ]
        );
    }

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
                writes_completed: 0,
                sectors_written: 0,
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
                writes_completed: 0,
                sectors_written: 0,
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
                writes_completed: 0,
                sectors_written: 0,
                io_millis: 0,
            },
            &sysfs,
        ));

        fs::remove_dir_all(Path::new(&root)).expect("remove only this test's sysfs fixture");
    }
}
