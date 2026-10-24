use crate::{
    PROTOCOL_MAJOR, PROTOCOL_MINOR, frame,
    wire::{Operation, Request, Response},
};
use anyhow::{Context, Result, bail};
use interprocess::local_socket::{tokio::Stream as LocalSocketStream, traits::tokio::Stream as _};
use std::time::Duration;
use tokio::time::timeout;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

pub async fn request(operation: Operation, process_limit: u32, sort_by: &str) -> Result<Response> {
    let request = Request {
        protocol_major: PROTOCOL_MAJOR,
        protocol_minor: PROTOCOL_MINOR,
        operation: operation as i32,
        process_limit,
        sort_by: sort_by.to_owned(),
    };

    let response: Response = timeout(REQUEST_TIMEOUT, async {
        let mut stream = connect()
            .await
            .context("connect to the local monitor agent")?;
        frame::write_message(&mut stream, &request)
            .await
            .context("send request to the local monitor agent")?;
        frame::read_message(&mut stream)
            .await
            .context("read response from the local monitor agent")
    })
    .await
    .context("local monitor agent did not respond within three seconds")??;

    if response.protocol_major != PROTOCOL_MAJOR {
        bail!(
            "agent IPC major version {} is incompatible with client version {}",
            response.protocol_major,
            PROTOCOL_MAJOR
        );
    }

    if !response.error_code.is_empty() {
        bail!(
            "agent returned {}: {}",
            response.error_code,
            response.error_message
        );
    }

    Ok(response)
}

#[cfg(unix)]
async fn connect() -> std::io::Result<LocalSocketStream> {
    use interprocess::local_socket::{GenericFilePath, ToFsName};

    let path = socket_path();
    let name = path.to_fs_name::<GenericFilePath>()?;
    LocalSocketStream::connect(name).await
}

#[cfg(windows)]
async fn connect() -> std::io::Result<LocalSocketStream> {
    use interprocess::local_socket::{GenericNamespaced, ToNsName};

    let name = pipe_name();
    let socket_name = name.to_ns_name::<GenericNamespaced>()?;
    LocalSocketStream::connect(socket_name).await
}

#[cfg(unix)]
fn socket_path() -> std::path::PathBuf {
    let user_id = super::unix_identity::effective_user_id();
    std::env::temp_dir().join(format!("native-system-monitor-{user_id}.sock"))
}

#[cfg(windows)]
fn pipe_name() -> String {
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_owned());
    let safe_user = user
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .take(48)
        .collect::<String>();
    let safe_user = if safe_user.is_empty() {
        "unknown".to_owned()
    } else {
        safe_user
    };

    format!("native-system-monitor-{safe_user}")
}
