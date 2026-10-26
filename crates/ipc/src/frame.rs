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
    use crate::{
        PROTOCOL_MAJOR, PROTOCOL_MINOR,
        wire::{Request, Response, Snapshot},
    };
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
            process_filter: "browser".to_owned(),
            sort_descending: Some(true),
            process_sort_key: "name".to_owned(),
        };
        let (mut sender, mut receiver) = tokio::io::duplex(512);

        write_message(&mut sender, &request)
            .await
            .expect("write request");
        let decoded: Request = read_message(&mut receiver).await.expect("read request");

        assert_eq!(decoded, request);
    }

    #[tokio::test]
    async fn snapshot_history_interval_round_trips_over_ipc() {
        let response = Response {
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR,
            snapshot: Some(Snapshot {
                history_interval_ms: 2_500,
                volumes: vec![crate::wire::VolumeMetrics {
                    mount_point: "/".to_owned(),
                    file_system: "ext4".to_owned(),
                    total_bytes: Some(10_000),
                    available_bytes: Some(4_000),
                    read_bytes_per_second: Some(512.0),
                    write_bytes_per_second: None,
                }],
                network_interfaces: vec![crate::wire::NetworkInterfaceMetrics {
                    name: "eth-test".to_owned(),
                    received_bytes_per_second: Some(1_024.0),
                    transmitted_bytes_per_second: None,
                }],
                block_devices: vec![crate::wire::BlockDeviceMetrics {
                    id: "8:0".to_owned(),
                    name: "sda".to_owned(),
                    read_bytes_per_second: Some(4_096.0),
                    write_bytes_per_second: None,
                    read_operations_per_second: Some(2.0),
                    write_operations_per_second: None,
                    busy_percent: Some(12.5),
                    read_latency_ms: Some(2.5),
                    write_latency_ms: None,
                }],
                block_device_io_status: "ok".to_owned(),
                gpus: vec![crate::wire::GpuMetrics {
                    id: "GPU-test".to_owned(),
                    name: "GPU fixture".to_owned(),
                    utilization_percent: Some(42.0),
                    memory_used_bytes: Some(1_024),
                    memory_total_bytes: Some(4_096),
                    temperature_celsius: None,
                    graphics_clock_mhz: Some(900),
                    memory_clock_mhz: None,
                    power_watts: Some(12.5),
                }],
                gpu_status: "ok".to_owned(),
                batteries: vec![crate::wire::BatteryMetrics {
                    id: "BAT0".to_owned(),
                    model: Some("Battery fixture".to_owned()),
                    status: Some("Discharging".to_owned()),
                    capacity_percent: Some(75),
                    energy_now_wh: Some(42.0),
                    energy_full_wh: Some(56.0),
                    power_watts: None,
                    temperature_celsius: Some(31.5),
                    cycle_count: Some(220),
                }],
                battery_status: "ok".to_owned(),
                ..Snapshot::default()
            }),
            ..Response::default()
        };
        let (mut sender, mut receiver) = tokio::io::duplex(512);

        write_message(&mut sender, &response)
            .await
            .expect("write snapshot response");
        let decoded: Response = read_message(&mut receiver)
            .await
            .expect("read snapshot response");

        let snapshot = decoded.snapshot.expect("snapshot payload");
        assert_eq!(snapshot.history_interval_ms, 2_500);
        assert_eq!(snapshot.volumes.len(), 1);
        assert_eq!(snapshot.volumes[0].mount_point, "/");
        assert_eq!(snapshot.volumes[0].file_system, "ext4");
        assert_eq!(snapshot.volumes[0].total_bytes, Some(10_000));
        assert_eq!(snapshot.volumes[0].available_bytes, Some(4_000));
        assert_eq!(snapshot.volumes[0].read_bytes_per_second, Some(512.0));
        assert_eq!(snapshot.volumes[0].write_bytes_per_second, None);
        assert_eq!(snapshot.network_interfaces.len(), 1);
        assert_eq!(snapshot.network_interfaces[0].name, "eth-test");
        assert_eq!(
            snapshot.network_interfaces[0].received_bytes_per_second,
            Some(1_024.0)
        );
        assert_eq!(
            snapshot.network_interfaces[0].transmitted_bytes_per_second,
            None
        );
        assert_eq!(snapshot.block_devices.len(), 1);
        assert_eq!(snapshot.block_devices[0].id, "8:0");
        assert_eq!(
            snapshot.block_devices[0].read_bytes_per_second,
            Some(4_096.0)
        );
        assert_eq!(snapshot.block_devices[0].write_bytes_per_second, None);
        assert_eq!(snapshot.block_devices[0].busy_percent, Some(12.5));
        assert_eq!(snapshot.block_devices[0].read_latency_ms, Some(2.5));
        assert_eq!(snapshot.block_devices[0].write_latency_ms, None);
        assert_eq!(snapshot.gpus.len(), 1);
        assert_eq!(snapshot.gpus[0].id, "GPU-test");
        assert_eq!(snapshot.gpus[0].utilization_percent, Some(42.0));
        assert_eq!(snapshot.gpus[0].temperature_celsius, None);
        assert_eq!(snapshot.gpus[0].power_watts, Some(12.5));
        assert_eq!(snapshot.batteries.len(), 1);
        assert_eq!(snapshot.batteries[0].id, "BAT0");
        assert_eq!(snapshot.batteries[0].capacity_percent, Some(75));
        assert_eq!(snapshot.batteries[0].power_watts, None);
        assert_eq!(snapshot.block_device_io_status, "ok");
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
