use std::time::Duration;
use thiserror::Error;

const REMOTE_WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_REMOTE_WRITE_BODY_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeliveryStatus {
    Accepted,
    Retryable {
        status: u16,
        retry_after: Option<Duration>,
    },
    Rejected {
        status: u16,
    },
}

#[derive(Debug, Error)]
pub enum RemoteWriteClientError {
    #[error("remote-write endpoint must use HTTPS, except HTTP loopback test receivers")]
    InsecureEndpoint,
    #[error("invalid remote-write endpoint: {0}")]
    InvalidEndpoint(#[from] url::ParseError),
    #[error("could not initialize HTTPS remote-write client")]
    ClientInitialization,
    #[error("remote-write request failed before an HTTP response")]
    Transport,
    #[error("remote-write request body exceeds the 16 MiB safety limit")]
    PayloadTooLarge,
}

#[derive(Clone)]
pub struct RemoteWriteClient {
    client: reqwest::Client,
    endpoint: reqwest::Url,
}

impl RemoteWriteClient {
    pub fn new(endpoint: &str) -> Result<Self, RemoteWriteClientError> {
        let endpoint = reqwest::Url::parse(endpoint)?;
        if !is_allowed_endpoint(&endpoint) {
            return Err(RemoteWriteClientError::InsecureEndpoint);
        }

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(REMOTE_WRITE_TIMEOUT)
            .build()
            .map_err(|_| RemoteWriteClientError::ClientInitialization)?;
        Ok(Self { client, endpoint })
    }

    pub async fn send(
        &self,
        compressed_write_request: Vec<u8>,
    ) -> Result<DeliveryStatus, RemoteWriteClientError> {
        if compressed_write_request.len() > MAX_REMOTE_WRITE_BODY_BYTES {
            return Err(RemoteWriteClientError::PayloadTooLarge);
        }
        let response = self
            .client
            .post(self.endpoint.clone())
            .header(reqwest::header::CONTENT_TYPE, "application/x-protobuf")
            .header(reqwest::header::CONTENT_ENCODING, "snappy")
            .header("X-Prometheus-Remote-Write-Version", "0.1.0")
            .header(
                reqwest::header::USER_AGENT,
                concat!("native-monitor/", env!("CARGO_PKG_VERSION")),
            )
            .body(compressed_write_request)
            .send()
            .await
            .map_err(|_| RemoteWriteClientError::Transport)?;

        let status = response.status();
        if status.is_success() {
            return Ok(DeliveryStatus::Accepted);
        }

        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs);
        if is_retryable_status(status.as_u16()) {
            return Ok(DeliveryStatus::Retryable {
                status: status.as_u16(),
                retry_after,
            });
        }
        Ok(DeliveryStatus::Rejected {
            status: status.as_u16(),
        })
    }
}

fn is_allowed_endpoint(endpoint: &reqwest::Url) -> bool {
    if endpoint.username() != ""
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
        || endpoint.port() == Some(0)
    {
        return false;
    }
    match endpoint.scheme() {
        "https" => endpoint.host().is_some(),
        "http" => endpoint.host_str().is_some_and(|host| {
            host == "localhost"
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        }),
        _ => false,
    }
}

fn is_retryable_status(status: u16) -> bool {
    status == 408 || status == 425 || status == 429 || (500..=599).contains(&status)
}

#[cfg(test)]
mod tests {
    use super::{DeliveryStatus, RemoteWriteClient, RemoteWriteClientError, is_retryable_status};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn allows_https_and_loopback_http_but_rejects_plain_remote_urls() {
        assert!(RemoteWriteClient::new("https://metrics.example.test/api/v1/write").is_ok());
        assert!(RemoteWriteClient::new("http://127.0.0.1:8428/api/v1/write").is_ok());
        assert!(matches!(
            RemoteWriteClient::new("http://metrics.example.test/api/v1/write"),
            Err(RemoteWriteClientError::InsecureEndpoint)
        ));
        assert!(matches!(
            RemoteWriteClient::new("https://user:password@metrics.example.test/write"),
            Err(RemoteWriteClientError::InsecureEndpoint)
        ));
    }

    #[test]
    fn classifies_only_transient_http_statuses_as_retryable() {
        for status in [408, 425, 429, 500, 503, 599] {
            assert!(is_retryable_status(status), "status {status} retries");
        }
        for status in [301, 400, 401, 403, 404] {
            assert!(!is_retryable_status(status), "status {status} is permanent");
        }
    }

    #[tokio::test]
    async fn sends_remote_write_v1_headers_and_classifies_receiver_status() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local fake receiver");
        let address = listener.local_addr().expect("listener address");
        let receiver = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept sender");
            let mut request = Vec::new();
            let mut chunk = [0_u8; 512];
            loop {
                let read = stream.read(&mut chunk).await.expect("read request");
                assert_ne!(read, 0, "client sent a complete request");
                request.extend_from_slice(&chunk[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let headers = String::from_utf8_lossy(&request).to_ascii_lowercase();
            assert!(headers.contains("content-type: application/x-protobuf"));
            assert!(headers.contains("content-encoding: snappy"));
            assert!(headers.contains("x-prometheus-remote-write-version: 0.1.0"));
            stream
                .write_all(b"HTTP/1.1 503 Service Unavailable\r\nRetry-After: 7\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await
                .expect("send retry response");
        });

        let client = RemoteWriteClient::new(&format!("http://{address}/api/v1/write"))
            .expect("local receiver is allowed for protocol tests");
        let result = client
            .send(vec![1, 2, 3])
            .await
            .expect("receive HTTP response");
        assert_eq!(
            result,
            DeliveryStatus::Retryable {
                status: 503,
                retry_after: Some(std::time::Duration::from_secs(7)),
            }
        );
        receiver.await.expect("fake receiver completes");
    }
}
