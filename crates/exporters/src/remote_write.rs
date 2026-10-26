use super::remote_write_client::{DeliveryStatus, RemoteWriteClient, RemoteWriteClientError};
use super::render_prometheus_for_families;
use native_monitor_domain::SystemSnapshot;
use prost::Message;
use snap::raw::Encoder;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::watch;
use tokio::time::{Instant, MissedTickBehavior, interval};
use tracing::{debug, warn};

const MAX_QUEUED_BATCHES: usize = 1_024;
const MAX_REMOTE_WRITE_QUEUE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug)]
struct RemoteWriteQueue {
    max_bytes: usize,
    bytes: usize,
    dropped_batches: u64,
    batches: VecDeque<Vec<u8>>,
}

impl RemoteWriteQueue {
    fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            bytes: 0,
            dropped_batches: 0,
            batches: VecDeque::new(),
        }
    }

    /// Rejects the newest batch when capacity is full and preserves FIFO order
    /// for batches already accepted into the volatile queue.
    fn enqueue(&mut self, batch: Vec<u8>) -> bool {
        let next_bytes = self.bytes.saturating_add(batch.len());
        if batch.is_empty()
            || next_bytes > self.max_bytes
            || self.batches.len() >= MAX_QUEUED_BATCHES
        {
            self.dropped_batches = self.dropped_batches.saturating_add(1);
            return false;
        }
        self.bytes = next_bytes;
        self.batches.push_back(batch);
        true
    }

    fn front(&self) -> Option<&[u8]> {
        self.batches.front().map(Vec::as_slice)
    }

    fn pop_front(&mut self) -> Option<Vec<u8>> {
        let batch = self.batches.pop_front()?;
        self.bytes = self.bytes.saturating_sub(batch.len());
        Some(batch)
    }

    fn queued_bytes(&self) -> usize {
        self.bytes
    }

    fn queued_batches(&self) -> usize {
        self.batches.len()
    }

    fn dropped_batches(&self) -> u64 {
        self.dropped_batches
    }
}

#[derive(Clone, PartialEq, Message)]
struct WriteRequest {
    #[prost(message, repeated, tag = "1")]
    timeseries: Vec<TimeSeries>,
}

#[derive(Clone, PartialEq, Message)]
struct TimeSeries {
    #[prost(message, repeated, tag = "1")]
    labels: Vec<Label>,
    #[prost(message, repeated, tag = "2")]
    samples: Vec<Sample>,
}

#[derive(Clone, PartialEq, Message)]
struct Label {
    #[prost(string, tag = "1")]
    name: String,
    #[prost(string, tag = "2")]
    value: String,
}

#[derive(Clone, PartialEq, Message)]
struct Sample {
    #[prost(double, tag = "1")]
    value: f64,
    #[prost(int64, tag = "2")]
    timestamp: i64,
}

#[derive(Debug, Error)]
pub enum RemoteWriteError {
    #[error("cannot Snappy-compress Prometheus Remote Write request: {0}")]
    Snappy(#[from] snap::Error),
    #[error("internal OpenMetrics sample could not be encoded")]
    InvalidSample,
    #[error("Remote Write interval and queue capacity must be within validated limits")]
    InvalidOptions,
    #[error(transparent)]
    Client(#[from] RemoteWriteClientError),
}

#[derive(Clone, Debug)]
pub struct RemoteWriteOptions {
    pub endpoint: String,
    pub interval: Duration,
    pub max_queue_bytes: usize,
    pub enabled_families: Vec<String>,
}

/// Sends snapshots from the existing collector watch channel. The queue is
/// volatile and bounded; collection never waits for the remote receiver.
pub async fn run_remote_write(
    snapshots: watch::Receiver<Arc<SystemSnapshot>>,
    mut shutdown: watch::Receiver<bool>,
    options: RemoteWriteOptions,
) -> Result<(), RemoteWriteError> {
    if options.interval.is_zero()
        || options.max_queue_bytes == 0
        || options.max_queue_bytes > MAX_REMOTE_WRITE_QUEUE_BYTES
    {
        return Err(RemoteWriteError::InvalidOptions);
    }
    let client = RemoteWriteClient::new(&options.endpoint)?;
    let mut ticker = interval(options.interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut queue = RemoteWriteQueue::new(options.max_queue_bytes);
    let mut retry_attempt = 0_u32;
    let mut retry_after = Instant::now();
    let mut last_snapshot_timestamp_ms = 0_u64;

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            _ = ticker.tick() => {
                let snapshot = snapshots.borrow().clone();
                if snapshot.captured_at_unix_ms == 0 {
                    continue;
                }
                if snapshot.captured_at_unix_ms <= last_snapshot_timestamp_ms {
                    continue;
                }
                last_snapshot_timestamp_ms = snapshot.captured_at_unix_ms;
                let sample_timestamp = i64::try_from(snapshot.captured_at_unix_ms)
                    .unwrap_or(i64::MAX);
                let batch = encode_v1(
                    &snapshot,
                    &options.enabled_families,
                    sample_timestamp,
                )?;
                if !queue.enqueue(batch) {
                    debug!(
                        queued_batches = queue.queued_batches(),
                        queued_bytes = queue.queued_bytes(),
                        dropped_batches = queue.dropped_batches(),
                        "remote-write queue full; newest batch dropped"
                    );
                }

                if Instant::now() < retry_after {
                    continue;
                }
                let Some(batch) = queue.front().map(ToOwned::to_owned) else {
                    continue;
                };

                match client.send(batch).await {
                    Ok(DeliveryStatus::Accepted) => {
                        queue.pop_front();
                        retry_attempt = 0;
                        retry_after = Instant::now();
                    }
                    Ok(DeliveryStatus::Retryable { status, retry_after: server_delay }) => {
                        retry_attempt = retry_attempt.saturating_add(1);
                        retry_after = Instant::now() + retry_delay(retry_attempt, server_delay);
                        debug!(status, retry_attempt, "remote-write receiver requested retry");
                    }
                    Ok(DeliveryStatus::Rejected { status }) => {
                        queue.pop_front();
                        retry_attempt = 0;
                        retry_after = Instant::now();
                        warn!(status, "remote-write receiver rejected a batch; batch discarded");
                    }
                    Err(error) => {
                        retry_attempt = retry_attempt.saturating_add(1);
                        retry_after = Instant::now() + retry_delay(retry_attempt, None);
                        debug!(%error, retry_attempt, "remote-write delivery failed; batch retained");
                    }
                }
            }
        }
    }

    if queue.queued_batches() > 0 {
        warn!(
            queued_batches = queue.queued_batches(),
            queued_bytes = queue.queued_bytes(),
            dropped_batches = queue.dropped_batches(),
            "agent stopped with volatile remote-write batches not delivered"
        );
    }
    Ok(())
}

fn retry_delay(attempt: u32, server_delay: Option<Duration>) -> Duration {
    const MAX_BACKOFF_SECONDS: u64 = 60;
    const MAX_SERVER_RETRY_SECONDS: u64 = 300;

    if let Some(delay) = server_delay {
        return delay.min(Duration::from_secs(MAX_SERVER_RETRY_SECONDS));
    }

    let exponent = attempt.saturating_sub(1).min(6);
    let base = 1_u64 << exponent;
    let base = base.min(MAX_BACKOFF_SECONDS);
    let jitter_seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let jitter_percent = 75 + (jitter_seed % 51);
    Duration::from_millis(
        base.saturating_mul(jitter_percent.into())
            .saturating_mul(10)
            .min(MAX_BACKOFF_SECONDS.saturating_mul(1_000)),
    )
}

/// Encodes a snapshot as a Prometheus Remote Write v1 protobuf message with
/// Snappy block compression. The returned bytes are suitable for the HTTP
/// request body; the caller supplies the sample timestamp in Unix milliseconds.
pub fn encode_v1(
    snapshot: &SystemSnapshot,
    enabled_families: &[String],
    timestamp_unix_ms: i64,
) -> Result<Vec<u8>, RemoteWriteError> {
    let exposition = render_prometheus_for_families(snapshot, enabled_families);
    let mut timeseries = Vec::new();

    for line in exposition.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((series, value)) = line.split_once(' ') else {
            return Err(RemoteWriteError::InvalidSample);
        };
        let value = value
            .parse::<f64>()
            .map_err(|_| RemoteWriteError::InvalidSample)?;
        if !value.is_finite() {
            return Err(RemoteWriteError::InvalidSample);
        }

        let (metric_name, labels) = parse_series(series)?;
        let mut labels = labels;
        labels.push(Label {
            name: "__name__".to_owned(),
            value: metric_name.to_owned(),
        });
        labels.sort_by(|left, right| left.name.cmp(&right.name));
        timeseries.push(TimeSeries {
            labels,
            samples: vec![Sample {
                value,
                timestamp: timestamp_unix_ms,
            }],
        });
    }

    let request = WriteRequest { timeseries };
    Ok(Encoder::new().compress_vec(&request.encode_to_vec())?)
}

fn parse_series(series: &str) -> Result<(&str, Vec<Label>), RemoteWriteError> {
    let Some((metric_name, encoded_labels)) = series.split_once('{') else {
        if series.is_empty() {
            return Err(RemoteWriteError::InvalidSample);
        }
        return Ok((series, Vec::new()));
    };
    let Some(encoded_labels) = encoded_labels.strip_suffix('}') else {
        return Err(RemoteWriteError::InvalidSample);
    };
    if metric_name.is_empty() {
        return Err(RemoteWriteError::InvalidSample);
    }

    let mut labels = Vec::new();
    for item in encoded_labels.split(',') {
        let Some((name, value)) = item.split_once("=\"") else {
            return Err(RemoteWriteError::InvalidSample);
        };
        let Some(value) = value.strip_suffix('"') else {
            return Err(RemoteWriteError::InvalidSample);
        };
        labels.push(Label {
            name: name.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok((metric_name, labels))
}

#[cfg(test)]
mod tests {
    use super::{
        RemoteWriteError, RemoteWriteOptions, RemoteWriteQueue, Sample, WriteRequest, encode_v1,
        retry_delay, run_remote_write,
    };
    use native_monitor_domain::SystemSnapshot;
    use prost::Message;
    use snap::raw::Decoder;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::watch;

    #[test]
    fn encodes_remote_write_v1_with_sorted_labels_and_sample_timestamp() {
        let mut snapshot = SystemSnapshot::starting();
        snapshot.cpu.total_usage_percent = Some(25.0);
        let enabled_families = vec!["cpu".to_owned()];
        let compressed =
            encode_v1(&snapshot, &enabled_families, 1_234_567).expect("valid snapshot is encoded");
        let decompressed = Decoder::new()
            .decompress_vec(&compressed)
            .expect("remote-write body uses Snappy block compression");
        let request = WriteRequest::decode(decompressed.as_slice())
            .expect("remote-write body uses the protobuf wire contract");

        let cpu = request
            .timeseries
            .iter()
            .find(|series| {
                series.labels.iter().any(|label| {
                    label.name == "__name__"
                        && label.value == "native_monitor_cpu_utilization_ratio"
                })
            })
            .expect("CPU series is included");
        assert_eq!(
            cpu.labels
                .iter()
                .map(|label| label.name.as_str())
                .collect::<Vec<_>>(),
            vec!["__name__"]
        );
        assert_eq!(
            cpu.samples,
            vec![Sample {
                value: 0.25,
                timestamp: 1_234_567,
            }]
        );
        assert!(request.timeseries.iter().any(|series| {
            series
                .labels
                .iter()
                .any(|label| label.name == "__name__" && label.value == "native_monitor_build_info")
        }));
        assert!(!request.timeseries.iter().any(|series| {
            series.labels.iter().any(|label| {
                label.name == "__name__" && label.value == "native_monitor_memory_total_bytes"
            })
        }));
    }

    #[test]
    fn encodes_aliases_instead_of_private_device_names() {
        let mut snapshot = SystemSnapshot::starting();
        snapshot
            .block_devices
            .push(native_monitor_domain::BlockDeviceMetrics {
                id: "private-id".to_owned(),
                name: "private-device-name".to_owned(),
                read_bytes_per_second: Some(512.0),
                write_bytes_per_second: None,
                read_operations_per_second: None,
                write_operations_per_second: None,
                read_latency_ms: None,
                write_latency_ms: None,
                busy_percent: None,
            });
        let encoded = encode_v1(&snapshot, &[], 10).expect("snapshot encodes");
        let decompressed = Decoder::new()
            .decompress_vec(&encoded)
            .expect("Snappy block decompresses");
        let request = WriteRequest::decode(decompressed.as_slice()).expect("protobuf decodes");
        let disk = request
            .timeseries
            .iter()
            .find(|series| {
                series.labels.iter().any(|label| {
                    label.name == "__name__"
                        && label.value == "native_monitor_disk_read_bytes_per_second"
                })
            })
            .expect("disk series included");
        assert!(
            disk.labels
                .iter()
                .any(|label| { label.name == "device" && label.value == "block0" })
        );
        assert!(
            !disk
                .labels
                .iter()
                .any(|label| label.value == "private-device-name")
        );
    }

    #[test]
    fn queue_is_bounded_and_rejects_new_batches_without_reordering_accepted_work() {
        let mut queue = RemoteWriteQueue::new(5);
        assert!(queue.enqueue(vec![1, 2, 3]));
        assert!(!queue.enqueue(vec![4, 5, 6]));
        assert_eq!(queue.queued_bytes(), 3);
        assert_eq!(queue.queued_batches(), 1);
        assert_eq!(queue.dropped_batches(), 1);
        assert_eq!(queue.front(), Some([1, 2, 3].as_slice()));
        assert_eq!(queue.pop_front(), Some(vec![1, 2, 3]));
        assert_eq!(queue.queued_bytes(), 0);
        assert!(queue.enqueue(vec![7, 8, 9]));
        assert_eq!(queue.front(), Some([7, 8, 9].as_slice()));
    }

    #[test]
    fn retry_delay_is_capped_and_respects_bounded_server_retry_after() {
        let local = retry_delay(100, None);
        assert!(local >= std::time::Duration::from_secs(45));
        assert!(local <= std::time::Duration::from_secs(60));
        assert_eq!(
            retry_delay(3, Some(std::time::Duration::from_secs(9))),
            std::time::Duration::from_secs(9)
        );
        assert_eq!(
            retry_delay(3, Some(std::time::Duration::from_secs(999))),
            std::time::Duration::from_secs(300)
        );
    }

    #[tokio::test]
    async fn worker_rejects_unbounded_runtime_options_instead_of_panicking() {
        let snapshot = Arc::new(SystemSnapshot::starting());
        let (_snapshot_sender, snapshot_receiver) = watch::channel(snapshot);
        let (_shutdown_sender, shutdown_receiver) = watch::channel(false);
        let result = run_remote_write(
            snapshot_receiver,
            shutdown_receiver,
            RemoteWriteOptions {
                endpoint: "https://metrics.example.test/api/v1/write".to_owned(),
                interval: Duration::ZERO,
                max_queue_bytes: 17 * 1024 * 1024,
                enabled_families: Vec::new(),
            },
        )
        .await;

        assert!(matches!(result, Err(RemoteWriteError::InvalidOptions)));
    }

    #[tokio::test]
    async fn worker_sends_a_snapshot_to_the_receiver_and_stops_on_shutdown() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local fake remote-write receiver");
        let address = listener.local_addr().expect("receiver address");
        let (shutdown_sender, shutdown_receiver) = watch::channel(false);
        let mut snapshot = SystemSnapshot::starting();
        snapshot.captured_at_unix_ms = 1_234;
        snapshot.cpu.total_usage_percent = Some(50.0);
        let (_snapshot_sender, snapshot_receiver) = watch::channel(Arc::new(snapshot));

        let receiver = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept sender");
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1_024];
            let header_end = loop {
                let read = stream.read(&mut chunk).await.expect("read request");
                assert_ne!(read, 0, "sender completes an HTTP request");
                request.extend_from_slice(&chunk[..read]);
                if let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                    break header_end + 4;
                }
            };
            let headers = String::from_utf8_lossy(&request[..header_end]).to_ascii_lowercase();
            let content_length = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .expect("content length is present")
                .trim()
                .parse::<usize>()
                .expect("content length is valid");
            while request.len() < header_end + content_length {
                let read = stream.read(&mut chunk).await.expect("read request body");
                assert_ne!(read, 0, "request body is complete");
                request.extend_from_slice(&chunk[..read]);
            }
            assert!(headers.contains("content-encoding: snappy"));
            assert!(headers.contains("x-prometheus-remote-write-version: 0.1.0"));
            let body = request[header_end..header_end + content_length].to_vec();
            stream
                .write_all(
                    b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .expect("acknowledge remote-write request");
            body
        });

        let worker = tokio::spawn(run_remote_write(
            snapshot_receiver,
            shutdown_receiver,
            RemoteWriteOptions {
                endpoint: format!("http://{address}/api/v1/write"),
                interval: Duration::from_secs(30),
                max_queue_bytes: 4_096,
                enabled_families: vec!["cpu".to_owned()],
            },
        ));
        let body = receiver.await.expect("receiver completed");
        shutdown_sender.send_replace(true);
        worker
            .await
            .expect("worker task completed")
            .expect("worker shut down cleanly");

        let decompressed = Decoder::new()
            .decompress_vec(&body)
            .expect("worker sent a Snappy block");
        let request = WriteRequest::decode(decompressed.as_slice())
            .expect("worker sent a Remote Write v1 protobuf");
        assert!(request.timeseries.iter().any(|series| {
            series
                .samples
                .iter()
                .any(|sample| sample.timestamp == 1_234 && sample.value == 0.5)
        }));
    }
}
