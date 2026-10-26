use native_monitor_domain::BatteryMetrics;
use std::fs;
use std::io;
use std::path::Path;

const SYS_POWER_SUPPLY: &str = "/sys/class/power_supply";

pub fn read_system_batteries() -> io::Result<Vec<BatteryMetrics>> {
    read_batteries_from(Path::new(SYS_POWER_SUPPLY))
}

fn read_batteries_from(root: &Path) -> io::Result<Vec<BatteryMetrics>> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut batteries = Vec::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let battery_type = read_text(&path.join("type"));
        if battery_type.as_deref() != Some("Battery") {
            continue;
        }
        batteries.push(read_battery(
            &path,
            entry.file_name().to_string_lossy().into_owned(),
        ));
    }

    Ok(batteries)
}

fn read_battery(path: &Path, id: String) -> BatteryMetrics {
    let energy_now = read_u64(&path.join("energy_now"));
    let energy_full = read_u64(&path.join("energy_full"));
    let power_now = read_u64(&path.join("power_now"));
    let current_now = read_u64(&path.join("current_now"));
    let voltage_now = read_u64(&path.join("voltage_now"));
    let power_watts = power_now
        .map(|microwatts| microwatts as f64 / 1_000_000.0)
        .or_else(|| Some(current_now? as f64 * voltage_now? as f64 / 1_000_000_000_000.0));

    BatteryMetrics {
        id,
        model: read_text(&path.join("model_name")),
        status: read_text(&path.join("status")),
        capacity_percent: read_u64(&path.join("capacity"))
            .filter(|capacity| *capacity <= 100)
            .and_then(|capacity| u8::try_from(capacity).ok()),
        energy_now_wh: energy_now.map(|microwatt_hours| microwatt_hours as f64 / 1_000_000.0),
        energy_full_wh: energy_full.map(|microwatt_hours| microwatt_hours as f64 / 1_000_000.0),
        power_watts,
        temperature_celsius: read_i64(&path.join("temp"))
            .map(|tenths_celsius| tenths_celsius as f64 / 10.0),
        cycle_count: read_u64(&path.join("cycle_count"))
            .and_then(|cycles| u32::try_from(cycles).ok()),
    }
}

fn read_text(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn read_u64(path: &Path) -> Option<u64> {
    read_text(path)?.parse().ok()
}

fn read_i64(path: &Path) -> Option<i64> {
    read_text(path)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::read_batteries_from;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn fixture_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "native-monitor-power-supply-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn parses_battery_values_and_keeps_unavailable_fields_absent() {
        let root = fixture_root();
        let battery = root.join("BAT0");
        fs::create_dir_all(&battery).expect("create battery fixture");
        fs::write(battery.join("type"), "Battery\n").expect("write battery type");
        fs::write(battery.join("model_name"), "Example pack\n").expect("write model");
        fs::write(battery.join("status"), "Discharging\n").expect("write status");
        fs::write(battery.join("capacity"), "75\n").expect("write capacity");
        fs::write(battery.join("energy_now"), "42000000\n").expect("write energy now");
        fs::write(battery.join("energy_full"), "56000000\n").expect("write energy full");
        fs::write(battery.join("power_now"), "12000000\n").expect("write power");
        fs::write(battery.join("temp"), "315\n").expect("write temperature");
        fs::write(battery.join("cycle_count"), "220\n").expect("write cycles");
        let batteries = read_batteries_from(&root).expect("read fixture batteries");

        assert_eq!(batteries.len(), 1);
        assert_eq!(batteries[0].id, "BAT0");
        assert_eq!(batteries[0].model.as_deref(), Some("Example pack"));
        assert_eq!(batteries[0].status.as_deref(), Some("Discharging"));
        assert_eq!(batteries[0].capacity_percent, Some(75));
        assert_eq!(batteries[0].energy_now_wh, Some(42.0));
        assert_eq!(batteries[0].energy_full_wh, Some(56.0));
        assert_eq!(batteries[0].power_watts, Some(12.0));
        assert_eq!(batteries[0].temperature_celsius, Some(31.5));
        assert_eq!(batteries[0].cycle_count, Some(220));

        fs::remove_dir_all(&root).expect("remove only this test fixture");
    }

    #[test]
    fn ignores_non_battery_supplies_and_rejects_invalid_capacity() {
        let root = fixture_root();
        let battery = root.join("BAT1");
        let ac = root.join("AC");
        fs::create_dir_all(&battery).expect("create battery fixture");
        fs::create_dir_all(&ac).expect("create AC fixture");
        fs::write(battery.join("type"), "Battery\n").expect("write battery type");
        fs::write(battery.join("capacity"), "101\n").expect("write invalid capacity");
        fs::write(ac.join("type"), "Mains\n").expect("write AC type");
        let batteries = read_batteries_from(&root).expect("read fixture supplies");

        assert_eq!(batteries.len(), 1);
        assert_eq!(batteries[0].capacity_percent, None);
        assert_eq!(batteries[0].energy_now_wh, None);
        assert_eq!(batteries[0].power_watts, None);

        fs::remove_dir_all(&root).expect("remove only this test fixture");
    }

    #[test]
    fn power_can_be_derived_from_current_and_voltage_counters() {
        let root = fixture_root();
        let battery = root.join("BAT2");
        fs::create_dir_all(&battery).expect("create battery fixture");
        fs::write(battery.join("type"), "Battery\n").expect("write battery type");
        fs::write(battery.join("current_now"), "2000000\n").expect("write current");
        fs::write(battery.join("voltage_now"), "12000000\n").expect("write voltage");
        let batteries = read_batteries_from(&root).expect("read fixture battery");

        assert_eq!(batteries[0].power_watts, Some(24.0));

        fs::remove_dir_all(&root).expect("remove only this test fixture");
    }
}
