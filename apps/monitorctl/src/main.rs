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
                "{:<8} {:<9} {:>8} {:>12}  name",
                "PID", "PPID", "CPU %", "RAM MiB"
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
                    "{:<8} {:<9} {:>8} {:>12.1}  {}",
                    process.pid,
                    parent_pid,
                    cpu_usage,
                    process.memory_bytes as f64 / (1024.0 * 1024.0),
                    process.name
                );
            }
        }
        "devices" => {
            let device_type = parse_device_options(arguments)?;
            let response = client::request(Operation::Snapshot, 1, "")
                .await
                .context("request device snapshot")?;
            let required_minor = if device_type == "block" { 4 } else { 3 };
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
                        "{:<16} {:>12} {:>12} {:>10} {:>10} {:>8}",
                        "device", "read", "write", "read IOPS", "write IOPS", "busy"
                    );
                    for device in snapshot.block_devices {
                        println!(
                            "{:<16} {:>12} {:>12} {:>10} {:>10} {:>8}",
                            device.name,
                            format_bytes_per_second(device.read_bytes_per_second),
                            format_bytes_per_second(device.write_bytes_per_second),
                            format_operations_per_second(device.read_operations_per_second),
                            format_operations_per_second(device.write_operations_per_second),
                            device
                                .busy_percent
                                .map(|value| format!("{value:.1}%"))
                                .unwrap_or_else(|| "—".to_owned())
                        );
                    }
                }
                _ => bail!("--type must be disk, network, or block"),
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
                        .context("--type requires disk, network, or block")?,
                );
            }
            _ => bail!("unknown devices option {argument}"),
        }
    }

    let device_type = device_type.context("devices requires --type disk, network, or block")?;
    if device_type != "disk" && device_type != "network" && device_type != "block" {
        bail!("--type must be disk, network, or block");
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

fn ensure_no_more_arguments(mut arguments: impl Iterator<Item = String>) -> Result<()> {
    if let Some(unexpected) = arguments.next() {
        bail!("unexpected argument {unexpected}");
    }

    Ok(())
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
        "Usage:\n  monitorctl status\n  monitorctl capabilities\n  monitorctl processes [--sort cpu|memory] [--limit N]\n  monitorctl devices --type disk|network|block"
    );
}

#[cfg(test)]
mod tests {
    use super::{format_bytes, format_bytes_per_second, parse_device_options};

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
        assert!(parse_device_options(["--type".to_owned(), "gpu".to_owned()].into_iter()).is_err());
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
