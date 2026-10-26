use native_monitor_domain::SystemSnapshot;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tracing::debug;

const MAX_REQUEST_BYTES: usize = 4 * 1024;
const MAX_ACTIVE_CLIENTS: usize = 4;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const METRICS_CONTENT_TYPE: &str = "application/openmetrics-text; version=1.0.0; charset=utf-8";

pub async fn serve(
    listener: TcpListener,
    snapshots: watch::Receiver<Arc<SystemSnapshot>>,
    mut shutdown: watch::Receiver<bool>,
) -> io::Result<()> {
    let mut clients = JoinSet::new();

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            result = clients.join_next(), if !clients.is_empty() => {
                if let Some(Err(error)) = result {
                    debug!(%error, "metrics scrape task failed");
                }
            }
            accepted = listener.accept() => {
                let (stream, peer) = accepted?;
                if !peer.ip().is_loopback() {
                    debug!(%peer, "rejected non-loopback metrics client");
                    continue;
                }
                if clients.len() >= MAX_ACTIVE_CLIENTS {
                    drop(stream);
                    continue;
                }
                let snapshots = snapshots.clone();
                clients.spawn(async move {
                    match timeout(REQUEST_TIMEOUT, handle_client(stream, snapshots)).await {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => debug!(%peer, %error, "metrics scrape failed"),
                        Err(_) => debug!(%peer, "metrics scrape timed out"),
                    }
                });
            }
        }
    }

    while clients.join_next().await.is_some() {}
    Ok(())
}

async fn handle_client(
    mut stream: TcpStream,
    snapshots: watch::Receiver<Arc<SystemSnapshot>>,
) -> io::Result<()> {
    let mut request = Vec::with_capacity(512);
    let mut chunk = [0_u8; 512];

    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(());
        }
        if request.len() + read > MAX_REQUEST_BYTES {
            return write_response(
                stream,
                431,
                "Request Header Fields Too Large",
                "text/plain; charset=utf-8",
                "request headers too large\n",
            )
            .await;
        }
        request.extend_from_slice(&chunk[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }

    let Some(request_line) = request.split(|byte| *byte == b'\n').next() else {
        return write_response(
            stream,
            400,
            "Bad Request",
            "text/plain; charset=utf-8",
            "malformed request\n",
        )
        .await;
    };
    let request_line = std::str::from_utf8(request_line)
        .unwrap_or_default()
        .trim_end_matches('\r');
    let mut parts = request_line.split_ascii_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();
    let version = parts.next().unwrap_or_default();

    if !matches!(version, "HTTP/1.0" | "HTTP/1.1") || parts.next().is_some() {
        return write_response(
            stream,
            400,
            "Bad Request",
            "text/plain; charset=utf-8",
            "malformed request line\n",
        )
        .await;
    }
    if method != "GET" {
        return write_response(
            stream,
            405,
            "Method Not Allowed",
            "text/plain; charset=utf-8",
            "only GET is supported\n",
        )
        .await;
    }
    if path != "/metrics" {
        return write_response(
            stream,
            404,
            "Not Found",
            "text/plain; charset=utf-8",
            "not found\n",
        )
        .await;
    }

    let snapshot = snapshots.borrow().clone();
    let body = render_prometheus(&snapshot);
    write_response(stream, 200, "OK", METRICS_CONTENT_TYPE, &body).await
}

async fn write_response(
    mut stream: TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &str,
) -> io::Result<()> {
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(body.as_bytes()).await?;
    stream.shutdown().await
}

pub fn render_prometheus(snapshot: &SystemSnapshot) -> String {
    let mut output = String::with_capacity(2_048);
    for (name, unit, help) in METRIC_FAMILIES {
        let _ = writeln!(output, "# HELP {name} {help}");
        let _ = writeln!(output, "# TYPE {name} gauge");
        if let Some(unit) = unit {
            let _ = writeln!(output, "# UNIT {name} {unit}");
        }
    }
    let _ = writeln_metric(
        &mut output,
        "native_monitor_snapshot_timestamp_seconds",
        &[],
        (snapshot.captured_at_unix_ms > 0).then_some(snapshot.captured_at_unix_ms as f64 / 1_000.0),
    );
    let _ = writeln_metric(
        &mut output,
        "native_monitor_cpu_utilization_ratio",
        &[],
        snapshot
            .cpu
            .total_usage_percent
            .map(|value| f64::from(value.clamp(0.0, 100.0)) / 100.0),
    );
    let _ = writeln_metric(
        &mut output,
        "native_monitor_memory_total_bytes",
        &[],
        snapshot.memory.total_bytes.map(|value| value as f64),
    );
    let _ = writeln_metric(
        &mut output,
        "native_monitor_memory_used_bytes",
        &[],
        snapshot.memory.used_bytes.map(|value| value as f64),
    );

    for (collector, status) in [
        ("cpu", snapshot.cpu.status),
        ("memory", snapshot.memory.status),
        ("block_device_io", snapshot.block_device_io_status),
        ("gpu", snapshot.gpu_status),
        ("battery", snapshot.battery_status),
    ] {
        let healthy = matches!(status, native_monitor_domain::MetricStatus::Ok);
        let _ = writeln_metric(
            &mut output,
            "native_monitor_collector_up",
            &[("collector", collector)],
            Some(if healthy { 1.0 } else { 0.0 }),
        );
    }

    for (index, device) in snapshot.block_devices.iter().enumerate() {
        let alias = format!("block{index}");
        let labels = [("device", alias.as_str())];
        let _ = writeln_metric(
            &mut output,
            "native_monitor_disk_read_bytes_per_second",
            &labels,
            device.read_bytes_per_second,
        );
        let _ = writeln_metric(
            &mut output,
            "native_monitor_disk_written_bytes_per_second",
            &labels,
            device.write_bytes_per_second,
        );
        let _ = writeln_metric(
            &mut output,
            "native_monitor_disk_read_operations_per_second",
            &labels,
            device.read_operations_per_second,
        );
        let _ = writeln_metric(
            &mut output,
            "native_monitor_disk_write_operations_per_second",
            &labels,
            device.write_operations_per_second,
        );
        let _ = writeln_metric(
            &mut output,
            "native_monitor_disk_busy_ratio",
            &labels,
            device
                .busy_percent
                .map(|value| value.clamp(0.0, 100.0) / 100.0),
        );
    }

    for (index, interface) in snapshot.network_interfaces.iter().enumerate() {
        let alias = format!("interface{index}");
        let labels = [("interface", alias.as_str())];
        let _ = writeln_metric(
            &mut output,
            "native_monitor_network_received_bytes_per_second",
            &labels,
            interface.received_bytes_per_second,
        );
        let _ = writeln_metric(
            &mut output,
            "native_monitor_network_transmitted_bytes_per_second",
            &labels,
            interface.transmitted_bytes_per_second,
        );
    }

    for (index, gpu) in snapshot.gpus.iter().enumerate() {
        let alias = format!("gpu{index}");
        let labels = [("gpu", alias.as_str())];
        let _ = writeln_metric(
            &mut output,
            "native_monitor_gpu_utilization_ratio",
            &labels,
            gpu.utilization_percent
                .map(|value| f64::from(value.clamp(0.0, 100.0)) / 100.0),
        );
        let _ = writeln_metric(
            &mut output,
            "native_monitor_gpu_memory_used_bytes",
            &labels,
            gpu.memory_used_bytes.map(|value| value as f64),
        );
    }

    for (index, battery) in snapshot.batteries.iter().enumerate() {
        let alias = format!("battery{index}");
        let labels = [("battery", alias.as_str())];
        let _ = writeln_metric(
            &mut output,
            "native_monitor_battery_charge_ratio",
            &labels,
            battery
                .capacity_percent
                .map(|value| f64::from(value) / 100.0),
        );
        let _ = writeln_metric(
            &mut output,
            "native_monitor_battery_temperature_celsius",
            &labels,
            battery.temperature_celsius,
        );
    }

    output.push_str("# EOF\n");
    output
}

use std::fmt::Write;

const METRIC_FAMILIES: [(&str, Option<&str>, &str); 16] = [
    (
        "native_monitor_snapshot_timestamp_seconds",
        Some("seconds"),
        "Unix timestamp of the snapshot used for this scrape.",
    ),
    (
        "native_monitor_cpu_utilization_ratio",
        None,
        "Fraction of total CPU capacity currently in use.",
    ),
    (
        "native_monitor_memory_total_bytes",
        Some("bytes"),
        "Total physical memory in bytes.",
    ),
    (
        "native_monitor_memory_used_bytes",
        Some("bytes"),
        "Used physical memory in bytes.",
    ),
    (
        "native_monitor_collector_up",
        None,
        "Whether a collector reported a successful sample.",
    ),
    (
        "native_monitor_disk_read_bytes_per_second",
        Some("bytes"),
        "Physical block-device read throughput in bytes per second.",
    ),
    (
        "native_monitor_disk_written_bytes_per_second",
        Some("bytes"),
        "Physical block-device write throughput in bytes per second.",
    ),
    (
        "native_monitor_disk_read_operations_per_second",
        None,
        "Physical block-device read operations per second.",
    ),
    (
        "native_monitor_disk_write_operations_per_second",
        None,
        "Physical block-device write operations per second.",
    ),
    (
        "native_monitor_disk_busy_ratio",
        None,
        "Fraction of the sample interval with block-device I/O in progress.",
    ),
    (
        "native_monitor_network_received_bytes_per_second",
        Some("bytes"),
        "Interface receive throughput in bytes per second.",
    ),
    (
        "native_monitor_network_transmitted_bytes_per_second",
        Some("bytes"),
        "Interface transmit throughput in bytes per second.",
    ),
    (
        "native_monitor_gpu_utilization_ratio",
        None,
        "Fraction of reported GPU utilization capacity in use.",
    ),
    (
        "native_monitor_gpu_memory_used_bytes",
        Some("bytes"),
        "GPU memory reported as used in bytes.",
    ),
    (
        "native_monitor_battery_charge_ratio",
        None,
        "Battery charge level reported by the operating system.",
    ),
    (
        "native_monitor_battery_temperature_celsius",
        Some("celsius"),
        "Battery temperature reported by the operating system.",
    ),
];

fn writeln_metric(
    output: &mut String,
    name: &str,
    labels: &[(&str, &str)],
    value: Option<f64>,
) -> io::Result<()> {
    let Some(value) = value.filter(|value| value.is_finite()) else {
        return Ok(());
    };
    write!(output, "{name}").map_err(io::Error::other)?;
    if !labels.is_empty() {
        write!(output, "{{").map_err(io::Error::other)?;
        for (index, (key, label_value)) in labels.iter().enumerate() {
            if index > 0 {
                write!(output, ",").map_err(io::Error::other)?;
            }
            write!(output, "{key}=\"{}\"", escape_label(label_value)).map_err(io::Error::other)?;
        }
        write!(output, "}}").map_err(io::Error::other)?;
    }
    writeln!(output, " {value}").map_err(io::Error::other)
}

fn escape_label(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            _ => escaped.push(character),
        }
    }
    escaped
}

pub async fn bind_loopback(address: SocketAddr) -> io::Result<TcpListener> {
    if !address.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Prometheus listener must bind to a loopback address",
        ));
    }
    TcpListener::bind(address).await
}

#[cfg(test)]
mod tests {
    use super::{bind_loopback, render_prometheus, serve};
    use native_monitor_domain::{
        BatteryMetrics, BlockDeviceMetrics, CpuMetrics, MetricStatus, NetworkInterfaceMetrics,
        SystemSnapshot,
    };
    use std::net::{Ipv4Addr, SocketAddr};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio::sync::watch;

    #[test]
    fn renders_supported_metrics_and_omits_absent_values_and_private_names() {
        let mut snapshot = SystemSnapshot::starting();
        snapshot.captured_at_unix_ms = 1_750_000_000_000;
        snapshot.cpu = CpuMetrics {
            total_usage_percent: Some(25.0),
            per_core_usage_percent: Vec::new(),
            status: MetricStatus::Ok,
        };
        snapshot.block_devices.push(BlockDeviceMetrics {
            id: "8:0".to_owned(),
            name: "disk-with-serial-secret".to_owned(),
            read_bytes_per_second: Some(1_024.0),
            write_bytes_per_second: None,
            read_operations_per_second: Some(2.0),
            write_operations_per_second: None,
            read_latency_ms: None,
            write_latency_ms: None,
            busy_percent: Some(50.0),
        });
        snapshot.network_interfaces.push(NetworkInterfaceMetrics {
            name: "wlan0".to_owned(),
            received_bytes_per_second: Some(3.0),
            transmitted_bytes_per_second: None,
        });
        snapshot.batteries.push(BatteryMetrics {
            id: "BAT0".to_owned(),
            model: Some("private-model-name".to_owned()),
            status: Some("Discharging".to_owned()),
            capacity_percent: Some(75),
            energy_now_wh: None,
            energy_full_wh: None,
            power_watts: None,
            temperature_celsius: None,
            cycle_count: None,
        });

        let output = render_prometheus(&snapshot);

        assert!(output.contains("native_monitor_cpu_utilization_ratio 0.25"));
        assert!(
            output.contains("native_monitor_disk_read_bytes_per_second{device=\"block0\"} 1024")
        );
        assert!(output.contains("native_monitor_disk_busy_ratio{device=\"block0\"} 0.5"));
        assert!(
            !output
                .lines()
                .any(|line| { line.starts_with("native_monitor_disk_written_bytes_per_second{") })
        );
        assert!(output.contains("interface=\"interface0\""));
        assert!(output.contains("native_monitor_battery_charge_ratio{battery=\"battery0\"} 0.75"));
        assert!(!output.lines().any(|line| {
            line.starts_with("native_monitor_battery_temperature_celsius{battery=")
        }));
        assert!(!output.contains("disk-with-serial-secret"));
        assert!(!output.contains("wlan0"));
        assert!(!output.contains("private-model-name"));
    }

    #[tokio::test]
    async fn serves_metrics_over_loopback_and_closes_gracefully() {
        let listener = bind_loopback(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .expect("bind ephemeral loopback listener");
        let address = listener.local_addr().expect("read listener address");
        let (snapshot_sender, snapshot_receiver) =
            watch::channel(Arc::new(SystemSnapshot::starting()));
        let (shutdown_sender, shutdown_receiver) = watch::channel(false);
        let server = tokio::spawn(serve(listener, snapshot_receiver, shutdown_receiver));

        let mut client = TcpStream::connect(address)
            .await
            .expect("connect scrape client");
        client
            .write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .expect("send scrape request");
        let mut response = Vec::new();
        client
            .read_to_end(&mut response)
            .await
            .expect("read scrape response");
        let response = String::from_utf8(response).expect("response is UTF-8");
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains("Content-Type: application/openmetrics-text; version=1.0.0"));
        assert!(response.contains("native_monitor_collector_up{collector=\"cpu\"} 0"));
        assert!(response.contains("# EOF\n"));

        let mut not_found_client = TcpStream::connect(address)
            .await
            .expect("connect invalid-path client");
        not_found_client
            .write_all(b"GET /admin HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .expect("send invalid-path request");
        let mut not_found_response = Vec::new();
        not_found_client
            .read_to_end(&mut not_found_response)
            .await
            .expect("read invalid-path response");
        assert!(not_found_response.starts_with(b"HTTP/1.1 404 Not Found\r\n"));

        let mut oversized_client = TcpStream::connect(address)
            .await
            .expect("connect oversized-header client");
        let oversized_header = format!("GET /metrics HTTP/1.1\r\nX-Fill: {}", "a".repeat(5_000));
        oversized_client
            .write_all(oversized_header.as_bytes())
            .await
            .expect("send oversized header");
        let mut oversized_response = Vec::new();
        oversized_client
            .read_to_end(&mut oversized_response)
            .await
            .expect("read oversized-header response");
        assert!(
            oversized_response.starts_with(b"HTTP/1.1 431 Request Header Fields Too Large\r\n")
        );

        shutdown_sender
            .send(true)
            .expect("signal exporter shutdown");
        server
            .await
            .expect("join exporter task")
            .expect("exporter stops cleanly");
        drop(snapshot_sender);
    }

    #[tokio::test]
    async fn refuses_to_bind_on_non_loopback_addresses() {
        let address = SocketAddr::from(([0, 0, 0, 0], 9464));
        let error = bind_loopback(address)
            .await
            .expect_err("non-loopback listener must be refused");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
}
