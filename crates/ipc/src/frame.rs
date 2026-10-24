use prost::Message;
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub async fn read_message<R, M>(reader: &mut R) -> io::Result<M>
where
    R: AsyncRead + Unpin,
    M: Message + Default,
{
    let mut length_bytes = [0_u8; 4];
    reader.read_exact(&mut length_bytes).await?;
    let length = u32::from_be_bytes(length_bytes) as usize;
    validate_length(length)?;

    let mut payload = vec![0_u8; length];
    reader.read_exact(&mut payload).await?;
    M::decode(payload.as_slice()).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub async fn write_message<W, M>(writer: &mut W, message: &M) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
    M: Message,
{
    let payload = message.encode_to_vec();
    validate_length(payload.len())?;
    let length = u32::try_from(payload.len())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    writer.write_all(&length.to_be_bytes()).await?;
    writer.write_all(&payload).await?;
    writer.flush().await
}

fn validate_length(length: usize) -> io::Result<()> {
    if length == 0 || length > super::MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("IPC frame length {length} is outside the allowed range"),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{read_message, validate_length, write_message};
    use crate::{PROTOCOL_MAJOR, PROTOCOL_MINOR, wire::Request};
    use std::io;
    use tokio::io::AsyncWriteExt;

    #[test]
    fn rejects_empty_and_oversized_frames_before_allocating() {
        assert_eq!(
            validate_length(0).expect_err("empty frame").kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            validate_length(super::super::MAX_FRAME_BYTES + 1)
                .expect_err("oversized frame")
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[tokio::test]
    async fn protobuf_message_round_trips_over_length_prefixed_frame() {
        let request = Request {
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR,
            operation: 3,
            process_limit: 25,
            sort_by: "cpu".to_owned(),
        };
        let (mut sender, mut receiver) = tokio::io::duplex(128);

        write_message(&mut sender, &request)
            .await
            .expect("write request");
        let decoded: Request = read_message(&mut receiver).await.expect("read request");

        assert_eq!(decoded, request);
    }

    #[tokio::test]
    async fn rejects_truncated_frame() {
        let (mut sender, mut receiver) = tokio::io::duplex(16);
        sender
            .write_all(&[0, 0, 0, 3, 8])
            .await
            .expect("write partial frame");
        sender.shutdown().await.expect("close partial frame");

        let result = read_message::<_, Request>(&mut receiver).await;

        assert_eq!(
            result.expect_err("frame is truncated").kind(),
            io::ErrorKind::UnexpectedEof
        );
    }
}
