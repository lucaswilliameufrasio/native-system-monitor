use native_monitor_domain::GpuMetrics;
use std::fs;
use std::io;
use std::path::Path;

const AMD_PCI_VENDOR_ID: &str = "0x1002";

pub fn read_system_amd_gpus() -> io::Result<Vec<GpuMetrics>> {
    read_amd_gpus_from_sysfs(Path::new("/sys/class/drm"))
}

fn read_amd_gpus_from_sysfs(drm_root: &Path) -> io::Result<Vec<GpuMetrics>> {
    let mut cards = fs::read_dir(drm_root)?.collect::<io::Result<Vec<_>>>()?;
    cards.sort_by_key(|entry| entry.file_name());

    let mut gpus = Vec::new();
    for card in cards {
        if !is_drm_card(&card.file_name()) {
            continue;
        }

        let card_device = card.path().join("device");
        if !is_amdgpu_device(&card_device) {
            continue;
        }

        if let Some(gpu) = read_gpu_metrics(&card_device) {
            gpus.push(gpu);
        }
    }

    Ok(gpus)
}

fn is_drm_card(name: &std::ffi::OsStr) -> bool {
    name.to_str()
        .and_then(|name| name.strip_prefix("card"))
        .is_some_and(|number| {
            !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn is_amdgpu_device(device_path: &Path) -> bool {
    let Ok(vendor) = fs::read_to_string(device_path.join("vendor")) else {
        return false;
    };
    if !vendor.trim().eq_ignore_ascii_case(AMD_PCI_VENDOR_ID) {
        return false;
    }

    let Ok(driver) = fs::read_link(device_path.join("driver")) else {
        return false;
    };
    driver.file_name().is_some_and(|name| name == "amdgpu")
}

fn read_gpu_metrics(device_path: &Path) -> Option<GpuMetrics> {
    let canonical_device = device_path.canonicalize().ok()?;
    let id = canonical_device.file_name()?.to_string_lossy().into_owned();
    let (temperature_celsius, graphics_clock_mhz, memory_clock_mhz) =
        read_hwmon_metrics(device_path);

    Some(GpuMetrics {
        id,
        name: read_optional_text(&device_path.join("product_name"))
            .unwrap_or_else(|| "AMD GPU".to_owned()),
        utilization_percent: read_optional_u32(&device_path.join("gpu_busy_percent"))
            .filter(|value| *value <= 100)
            .map(|value| value as f32),
        memory_used_bytes: read_optional_u64(&device_path.join("mem_info_vram_used")),
        memory_total_bytes: read_optional_u64(&device_path.join("mem_info_vram_total")),
        temperature_celsius,
        graphics_clock_mhz,
        memory_clock_mhz,
        // Generic hwmon power can include CPU/SoC draw on APUs, so it is not
        // safe to report as GPU board power without a device-specific source.
        power_watts: None,
    })
}

fn read_hwmon_metrics(device_path: &Path) -> (Option<u32>, Option<u32>, Option<u32>) {
    let hwmon_root = device_path.join("hwmon");
    let Ok(entries) = fs::read_dir(hwmon_root) else {
        return (None, None, None);
    };
    let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    let mut temperature_celsius = None;
    let mut graphics_clock_mhz = None;
    let mut memory_clock_mhz = None;
    for entry in entries {
        let hwmon_path = entry.path();
        if temperature_celsius.is_none() {
            temperature_celsius = read_optional_i64(&hwmon_path.join("temp1_input"))
                .and_then(millicelsius_to_celsius);
        }
        if graphics_clock_mhz.is_none() {
            graphics_clock_mhz =
                read_optional_u64(&hwmon_path.join("freq1_input")).and_then(hertz_to_megahertz);
        }
        if memory_clock_mhz.is_none() {
            memory_clock_mhz =
                read_optional_u64(&hwmon_path.join("freq2_input")).and_then(hertz_to_megahertz);
        }
    }

    (temperature_celsius, graphics_clock_mhz, memory_clock_mhz)
}

fn read_optional_text(path: &Path) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    let value = contents.trim();
    if value.is_empty() || value.chars().any(char::is_control) {
        return None;
    }

    Some(value.to_owned())
}

fn read_optional_u32(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn read_optional_u64(path: &Path) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn read_optional_i64(path: &Path) -> Option<i64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn millicelsius_to_celsius(millicelsius: i64) -> Option<u32> {
    if millicelsius < 0 {
        return None;
    }

    let celsius = u32::try_from(millicelsius / 1_000).ok()?;
    Some(celsius)
}

fn hertz_to_megahertz(hertz: u64) -> Option<u32> {
    u32::try_from(hertz / 1_000_000).ok()
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{hertz_to_megahertz, millicelsius_to_celsius, read_amd_gpus_from_sysfs};
    use native_monitor_domain::GpuMetrics;
    use std::fs;
    use std::io;
    use std::os::unix::fs::symlink;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn fixture_root() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "native-monitor-amdgpu-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn add_card(
        root: &Path,
        card_name: &str,
        pci_address: &str,
        vendor: &str,
        driver: &str,
    ) -> io::Result<std::path::PathBuf> {
        let pci_device = root.join("pci").join(pci_address);
        let card = root.join("drm").join(card_name);
        fs::create_dir_all(&pci_device)?;
        fs::create_dir_all(&card)?;
        fs::create_dir_all(root.join("drivers").join(driver))?;
        fs::write(pci_device.join("vendor"), vendor)?;
        symlink(root.join("drivers").join(driver), pci_device.join("driver"))?;
        symlink(pci_device, card.join("device"))?;
        Ok(card.join("device"))
    }

    #[test]
    fn reads_multi_gpu_sysfs_fixtures_and_converts_documented_units() {
        let root = fixture_root();
        let first = add_card(&root, "card0", "0000:03:00.0", "0x1002\n", "amdgpu")
            .expect("create first AMD fixture");
        let second = add_card(&root, "card1", "0000:0a:00.0", "0X1002\n", "amdgpu")
            .expect("create second AMD fixture");
        let drm_root = root.join("drm");
        fs::write(first.join("product_name"), "Radeon Fixture 1\n").expect("write product name");
        fs::write(first.join("gpu_busy_percent"), "73\n").expect("write GPU utilization");
        fs::write(first.join("mem_info_vram_used"), "1073741824\n").expect("write VRAM used");
        fs::write(first.join("mem_info_vram_total"), "8589934592\n").expect("write VRAM total");
        let hwmon = first.join("hwmon/hwmon0");
        fs::create_dir_all(&hwmon).expect("create hwmon fixture");
        fs::write(hwmon.join("temp1_input"), "62500\n").expect("write temperature");
        fs::write(hwmon.join("freq1_input"), "2450000000\n").expect("write graphics clock");
        fs::write(hwmon.join("freq2_input"), "1000000000\n").expect("write memory clock");
        fs::write(hwmon.join("power1_average"), "150000000\n").expect("write SoC power fixture");
        fs::write(second.join("gpu_busy_percent"), "101\n")
            .expect("write out-of-range utilization");
        fs::write(second.join("mem_info_vram_used"), "invalid\n").expect("write invalid memory");

        let gpus = read_amd_gpus_from_sysfs(&drm_root).expect("collect AMD fixtures");

        assert_eq!(gpus.len(), 2);
        assert_eq!(
            gpus[0],
            GpuMetrics {
                id: "0000:03:00.0".to_owned(),
                name: "Radeon Fixture 1".to_owned(),
                utilization_percent: Some(73.0),
                memory_used_bytes: Some(1_073_741_824),
                memory_total_bytes: Some(8_589_934_592),
                temperature_celsius: Some(62),
                graphics_clock_mhz: Some(2_450),
                memory_clock_mhz: Some(1_000),
                power_watts: None,
            }
        );
        assert_eq!(gpus[1].id, "0000:0a:00.0");
        assert_eq!(gpus[1].name, "AMD GPU");
        assert_eq!(gpus[1].utilization_percent, None);
        assert_eq!(gpus[1].memory_used_bytes, None);
        assert_eq!(gpus[1].memory_total_bytes, None);
        assert_eq!(gpus[1].temperature_celsius, None);
        assert_eq!(gpus[1].graphics_clock_mhz, None);
        assert_eq!(gpus[1].power_watts, None);

        fs::remove_dir_all(root).expect("remove only this test's sysfs fixture");
    }

    #[test]
    fn ignores_non_amd_devices_and_non_card_drm_entries() {
        let root = fixture_root();
        let _wrong_vendor = add_card(&root, "card0", "0000:03:00.0", "0x8086\n", "amdgpu")
            .expect("create non-AMD fixture");
        let _wrong_driver = add_card(&root, "card1", "0000:0a:00.0", "0x1002\n", "radeon")
            .expect("create non-amdgpu fixture");
        let connector = root.join("drm/card2-DP-1");
        fs::create_dir_all(&connector).expect("create connector entry");

        let gpus = read_amd_gpus_from_sysfs(&root.join("drm")).expect("scan DRM fixture");

        assert!(gpus.is_empty());
        fs::remove_dir_all(root).expect("remove only this test's sysfs fixture");
    }

    #[test]
    fn missing_drm_root_is_reported_without_panicking() {
        let root = fixture_root();

        let result = read_amd_gpus_from_sysfs(&root);

        assert_eq!(
            result
                .expect_err("missing DRM root should be an I/O error")
                .kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]
    fn converts_units_without_wrapping_or_accepting_negative_temperature() {
        assert_eq!(millicelsius_to_celsius(62_500), Some(62));
        assert_eq!(millicelsius_to_celsius(-1_000), None);
        assert_eq!(hertz_to_megahertz(2_450_000_000), Some(2_450));
        assert_eq!(hertz_to_megahertz(u64::MAX), None);
    }
}
