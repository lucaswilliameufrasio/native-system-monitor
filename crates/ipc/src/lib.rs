pub mod client;
pub mod frame;
pub mod server;
#[cfg(unix)]
mod unix_endpoint;
#[cfg(unix)]
mod unix_identity;
pub mod wire {
    include!(concat!(env!("OUT_DIR"), "/native_monitor.v1.rs"));
}

pub const PROTOCOL_MAJOR: u32 = 1;
pub const PROTOCOL_MINOR: u32 = 1;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const MAX_ACTIVE_CLIENTS: usize = 8;
