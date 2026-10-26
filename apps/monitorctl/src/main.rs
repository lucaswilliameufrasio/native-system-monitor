use anyhow::{Context, Result, bail};
use native_monitor_ipc::client;
use native_monitor_ipc::wire::Operation;
use std::env;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("monitorctl: {error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<()> {
    let mut arguments = env::args().skip(1);
    let command = arguments.next().unwrap_or_else(|| "--help".to_owned());

    match command.as_str() {
        "status" => {
            ensure_no_more_arguments(arguments)?;
            let response = client::request(Operation::Status, 0, "").await?;
            println!("agent: {}", response.status);
            println!(
                "IPC protocol: {}.{}",
                response.protocol_major, response.protocol_minor
            );
        }
        "capabilities" => {
            ensure_no_more_arguments(arguments)?;
            let response = client::request(Operation::Capabilities, 0, "").await?;
            println!("agent: {}", response.status);
            println!("{:<16} {:<28} source", "capability", "status");
            for capability in response.capabilities {
                println!(
                    "{:<16} {:<28} {}",
                    capability.name, capability.status, capability.source
                );
            }
        }
        "processes" => {
            let (sort_by, limit) = parse_process_options(arguments)?;
            let response = client::request(Operation::Processes, limit, &sort_by)
                .await
                .context("request process list")?;
            let snapshot = response
                .snapshot
                .context("agent did not return a process snapshot")?;

            println!(
                "{:<8} {:<9} {:<12} {:>8} {:>12}  name",
                "PID", "PPID", "START (UNIX)", "CPU %", "RAM MiB"
            );
            for process in snapshot.processes {
                let parent_pid = process
                    .parent_pid
                    .map(|pid| pid.to_string())
                    .unwrap_or_else(|| "-".to_owned());
                let cpu_usage = process
                    .cpu_usage_percent
                    .map(|value| format!("{value:.1}"))
                    .unwrap_or_else(|| "—".to_owned());
                println!(
                    "{:<8} {:<9} {:<12} {:>8} {:>12.1}  {}",
                    process.pid,
                    parent_pid,
                    process.start_time_unix_seconds,
                    cpu_usage,
                    process.memory_bytes as f64 / (1024.0 * 1024.0),
                    process.name
                );
            }
        }
        "process-tree" => {
            let (pid, start_time_unix_seconds) = parse_process_identity(arguments)?;
            let response = client::request_process_tree(pid, start_time_unix_seconds)
                .await
                .context("request process tree for the exact process identity")?;
            let snapshot = response
                .snapshot
                .context("agent did not return a process tree snapshot")?;
            let mut depth_by_pid = std::collections::HashMap::new();
            for process in &snapshot.processes {
                let depth = process
                    .parent_pid
                    .and_then(|parent_pid| depth_by_pid.get(&parent_pid).copied())
                    .map_or(0, |parent_depth: usize| parent_depth.saturating_add(1));
                depth_by_pid.insert(process.pid, depth);
                let indent = "  ".repeat(depth.min(64));
                println!(
                    "{indent}{} (PID {}, start {}) — CPU {}%, RAM {:.1} MiB",
                    process.name,
                    process.pid,
                    process.start_time_unix_seconds,
                    process
                        .cpu_usage_percent
                        .map(|value| format!("{value:.1}"))
                        .unwrap_or_else(|| "—".to_owned()),
                    process.memory_bytes as f64 / (1024.0 * 1024.0)
                );
            }
            if snapshot.process_tree_truncated {
                println!("Tree truncated at the configured process response limit.");
            }
        }
        "devices" => {
            let device_type = parse_device_options(arguments)?;
            let response = client::request(Operation::Snapshot, 1, "")
                .await
                .context("request device snapshot")?;
            let required_minor = match device_type.as_str() {
                "block" => 4,
                "gpu" => 7,
                "battery" => 8,
                _ => 3,
            };
            if response.protocol_minor < required_minor {
                bail!("agent does not support {device_type} device queries");
            }
            let snapshot = response
                .snapshot
                .context("agent did not return a device snapshot")?;

            match device_type.as_str() {
                "disk" => {
                    println!(
                        "{:<28} {:<16} {:>12} {:>12} {:>12} {:>12}",
                        "mount point", "file system", "available", "total", "read", "write"
                    );
                    for volume in snapshot.volumes {
                        println!(
                            "{:<28} {:<16} {:>12} {:>12} {:>12} {:>12}",
                            volume.mount_point,
                            volume.file_system,
                            format_bytes(volume.available_bytes),
                            format_bytes(volume.total_bytes),
                            format_bytes_per_second(volume.read_bytes_per_second),
                            format_bytes_per_second(volume.write_bytes_per_second)
                        );
                    }
                }
                "network" => {
                    println!(
                        "{:<28} {:>16} {:>16}",
                        "interface", "received", "transmitted"
                    );
                    for interface in snapshot.network_interfaces {
                        println!(
                            "{:<28} {:>16} {:>16}",
                            interface.name,
                            format_bytes_per_second(interface.received_bytes_per_second),
                            format_bytes_per_second(interface.transmitted_bytes_per_second)
                        );
                    }
                }
                "block" => {
                    println!(
                        "{:<16} {:>12} {:>12} {:>10} {:>10} {:>10} {:>10} {:>8}",
                        "device",
                        "read",
                        "write",
                        "read IOPS",
                        "write IOPS",
                        "read ms",
                        "write ms",
                        "busy"
                    );
                    for device in snapshot.block_devices {
                        println!(
                            "{:<16} {:>12} {:>12} {:>10} {:>10} {:>10} {:>10} {:>8}",
                            device.name,
                            format_bytes_per_second(device.read_bytes_per_second),
                            format_bytes_per_second(device.write_bytes_per_second),
                            format_operations_per_second(device.read_operations_per_second),
                            format_operations_per_second(device.write_operations_per_second),
                            format_latency_ms(device.read_latency_ms),
                            format_latency_ms(device.write_latency_ms),
                            device
                                .busy_percent
                                .map(|value| format!("{value:.1}%"))
                                .unwrap_or_else(|| "—".to_owned())
                        );
                    }
                }
                "gpu" => {
                    println!(
                        "{:<28} {:>8} {:>18} {:>8} {:>12} {:>9}",
                        "GPU", "usage", "VRAM used/total", "temp", "clocks MHz", "power"
                    );
                    for gpu in &snapshot.gpus {
                        let memory = format!(
                            "{} / {}",
                            format_bytes(gpu.memory_used_bytes),
                            format_bytes(gpu.memory_total_bytes)
                        );
                        let clocks = format!(
                            "{} / {}",
                            gpu.graphics_clock_mhz
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "—".to_owned()),
                            gpu.memory_clock_mhz
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "—".to_owned())
                        );
                        println!(
                            "{:<28} {:>8} {:>18} {:>8} {:>12} {:>9}",
                            gpu.name,
                            gpu.utilization_percent
                                .map(|value| format!("{value:.0}%"))
                                .unwrap_or_else(|| "—".to_owned()),
                            memory,
                            gpu.temperature_celsius
                                .map(|value| format!("{value}°C"))
                                .unwrap_or_else(|| "—".to_owned()),
                            clocks,
                            gpu.power_watts
                                .map(|value| format!("{value:.1} W"))
                                .unwrap_or_else(|| "—".to_owned())
                        );
                    }
                    if snapshot.gpus.is_empty() {
                        println!("GPU status: {}", snapshot.gpu_status);
                    }
                }
                "battery" => {
                    println!(
                        "{:<12} {:<20} {:>8} {:>12} {:>12} {:>10} {:>8} {:>10}",
                        "battery",
                        "status",
                        "capacity",
                        "energy",
                        "full",
                        "power",
                        "temp",
                        "cycles"
                    );
                    for battery in &snapshot.batteries {
                        let energy = battery
                            .energy_now_wh
                            .map(|value| format!("{value:.1} Wh"))
                            .unwrap_or_else(|| "—".to_owned());
                        let energy_full = battery
                            .energy_full_wh
                            .map(|value| format!("{value:.1} Wh"))
                            .unwrap_or_else(|| "—".to_owned());
                        println!(
                            "{:<12} {:<20} {:>8} {:>12} {:>12} {:>10} {:>8} {:>10}",
                            battery.id,
                            battery.status.as_deref().unwrap_or("—"),
                            battery
                                .capacity_percent
                                .map(|value| format!("{value}%"))
                                .unwrap_or_else(|| "—".to_owned()),
                            energy,
                            energy_full,
                            battery
                                .power_watts
                                .map(|value| format!("{value:.1} W"))
                                .unwrap_or_else(|| "—".to_owned()),
                            battery
                                .temperature_celsius
                                .map(|value| format!("{value:.1}°C"))
                                .unwrap_or_else(|| "—".to_owned()),
                            battery
                                .cycle_count
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "—".to_owned())
                        );
                    }
                    if snapshot.batteries.is_empty() {
                        println!("Battery status: {}", snapshot.battery_status);
                    }
                }
                _ => bail!("--type must be disk, network, block, gpu, or battery"),
            }
        }
        "--help" | "-h" | "help" => print_help(),
        _ => bail!("unknown command {command}; use `monitorctl help`"),
    }

    Ok(())
}

fn parse_device_options(mut arguments: impl Iterator<Item = String>) -> Result<String> {
    let mut device_type = None;

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--type" => {
                if device_type.is_some() {
                    bail!("--type may only be specified once");
                }
                device_type = Some(
                    arguments
                        .next()
                        .context("--type requires disk, network, block, gpu, or battery")?,
                );
            }
            _ => bail!("unknown devices option {argument}"),
        }
    }

    let device_type =
        device_type.context("devices requires --type disk, network, block, gpu, or battery")?;
    if device_type != "disk"
        && device_type != "network"
        && device_type != "block"
        && device_type != "gpu"
        && device_type != "battery"
    {
        bail!("--type must be disk, network, block, gpu, or battery");
    }

    Ok(device_type)
}

fn format_bytes(bytes: Option<u64>) -> String {
    let Some(bytes) = bytes else {
        return "—".to_owned();
    };

    const KIB: f64 = 1_024.0;
    const MIB: f64 = KIB * 1_024.0;
    const GIB: f64 = MIB * 1_024.0;

    if bytes >= GIB as u64 {
        format!("{:.1} GiB", bytes as f64 / GIB)
    } else if bytes >= MIB as u64 {
        format!("{:.1} MiB", bytes as f64 / MIB)
    } else if bytes >= KIB as u64 {
        format!("{:.1} KiB", bytes as f64 / KIB)
    } else {
        format!("{bytes} B")
    }
}

fn format_bytes_per_second(bytes_per_second: Option<f64>) -> String {
    let Some(bytes_per_second) =
        bytes_per_second.filter(|value| value.is_finite() && *value >= 0.0)
    else {
        return "—".to_owned();
    };

    const KIB: f64 = 1_024.0;
    const MIB: f64 = KIB * 1_024.0;
    const GIB: f64 = MIB * 1_024.0;

    if bytes_per_second >= GIB {
        format!("{:.1} GiB/s", bytes_per_second / GIB)
    } else if bytes_per_second >= MIB {
        format!("{:.1} MiB/s", bytes_per_second / MIB)
    } else if bytes_per_second >= KIB {
        format!("{:.1} KiB/s", bytes_per_second / KIB)
    } else {
        format!("{bytes_per_second:.0} B/s")
    }
}

fn format_operations_per_second(operations_per_second: Option<f64>) -> String {
    operations_per_second
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| format!("{value:.1}"))
        .unwrap_or_else(|| "—".to_owned())
}

fn format_latency_ms(latency_ms: Option<f64>) -> String {
    latency_ms
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "—".to_owned())
}

fn ensure_no_more_arguments(mut arguments: impl Iterator<Item = String>) -> Result<()> {
    if let Some(unexpected) = arguments.next() {
        bail!("unexpected argument {unexpected}");
    }

    Ok(())
}

fn parse_process_identity(mut arguments: impl Iterator<Item = String>) -> Result<(u32, u64)> {
    let identity = arguments
        .next()
        .context("process-tree requires PID:START_UNIX_SECONDS")?;
    ensure_no_more_arguments(arguments)?;
    let (pid, start_time) = identity
        .split_once(':')
        .context("process identity must have the form PID:START_UNIX_SECONDS")?;
    let pid = pid
        .parse::<u32>()
        .context("process PID must be a positive integer")?;
    let start_time = start_time
        .parse::<u64>()
        .context("process start time must be an integer Unix timestamp")?;
    if pid == 0 || start_time == 0 {
        bail!("process PID and start time must both be greater than zero");
    }
    Ok((pid, start_time))
}

fn parse_process_options(mut arguments: impl Iterator<Item = String>) -> Result<(String, u32)> {
    let mut sort_by = "cpu".to_owned();
    let mut limit = 25_u32;

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--sort" => {
                sort_by = arguments
                    .next()
                    .context("--sort requires either cpu or memory")?;
                if sort_by != "cpu" && sort_by != "memory" {
                    bail!("--sort must be either cpu or memory");
                }
            }
            "--limit" => {
                let raw_limit = arguments.next().context("--limit requires an integer")?;
                limit = raw_limit
                    .parse::<u32>()
                    .context("--limit must be an integer between 1 and 1000")?;
                if !(1..=1_000).contains(&limit) {
                    bail!("--limit must be between 1 and 1000");
                }
            }
            _ => bail!("unknown processes option {argument}"),
        }
    }

    Ok((sort_by, limit))
}

fn print_help() {
    println!(
        "Usage:\n  monitorctl status\n  monitorctl capabilities\n  monitorctl processes [--sort cpu|memory] [--limit N]\n  monitorctl process-tree PID:START_UNIX_SECONDS\n  monitorctl devices --type disk|network|block|gpu|battery"
    );
}

#[cfg(test)]
mod tests {
    use super::{
        format_bytes, format_bytes_per_second, parse_device_options, parse_process_identity,
    };

    #[test]
    fn process_tree_requires_a_positive_pid_and_exact_start_time() {
        assert_eq!(
            parse_process_identity(["42:1700000000".to_owned()].into_iter())
                .expect("parse process identity"),
            (42, 1_700_000_000)
        );
        assert!(parse_process_identity(std::iter::empty()).is_err());
        assert!(parse_process_identity(["42".to_owned()].into_iter()).is_err());
        assert!(parse_process_identity(["0:42".to_owned()].into_iter()).is_err());
        assert!(parse_process_identity(["42:0".to_owned()].into_iter()).is_err());
        assert!(parse_process_identity(["42:42:42".to_owned()].into_iter()).is_err());
    }

    #[test]
    fn device_options_require_an_explicit_supported_type() {
        assert_eq!(
            parse_device_options(["--type".to_owned(), "disk".to_owned()].into_iter())
                .expect("parse disk device option"),
            "disk"
        );
        assert_eq!(
            parse_device_options(["--type".to_owned(), "network".to_owned()].into_iter())
                .expect("parse network device option"),
            "network"
        );
        assert_eq!(
            parse_device_options(["--type".to_owned(), "block".to_owned()].into_iter())
                .expect("parse block device option"),
            "block"
        );
        assert!(parse_device_options(std::iter::empty()).is_err());
        assert!(parse_device_options(["--type".to_owned()].into_iter()).is_err());
        assert_eq!(
            parse_device_options(["--type".to_owned(), "gpu".to_owned()].into_iter())
                .expect("parse GPU device option"),
            "gpu"
        );
        assert_eq!(
            parse_device_options(["--type".to_owned(), "battery".to_owned()].into_iter())
                .expect("parse battery device option"),
            "battery"
        );
        assert!(
            parse_device_options(
                [
                    "--type".to_owned(),
                    "disk".to_owned(),
                    "--type".to_owned(),
                    "disk".to_owned()
                ]
                .into_iter()
            )
            .is_err()
        );
    }

    #[test]
    fn byte_formatting_uses_iec_units_and_keeps_unknown_values_absent() {
        assert_eq!(format_bytes(None), "—");
        assert_eq!(format_bytes(Some(0)), "0 B");
        assert_eq!(format_bytes(Some(1_536)), "1.5 KiB");
        assert_eq!(format_bytes_per_second(None), "—");
        assert_eq!(format_bytes_per_second(Some(1_536.0)), "1.5 KiB/s");
    }
}
