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
        "--help" | "-h" | "help" => print_help(),
        _ => bail!("unknown command {command}; use `monitorctl help`"),
    }

    Ok(())
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
        "Usage:\n  monitorctl status\n  monitorctl capabilities\n  monitorctl processes [--sort cpu|memory] [--limit N]"
    );
}
