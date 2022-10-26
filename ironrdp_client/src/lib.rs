mod errors;
mod utils;

use ironrdp::{gcc, nego};
use tokio::net::TcpStream;

pub mod active_session;
pub mod connection_sequence;
pub mod transport;

pub use self::active_session::process_active_stage;
pub use self::connection_sequence::{process_connection_sequence, ConnectionSequenceResult, UpgradedStream};
pub use self::errors::RdpError;

pub struct GraphicsConfig {
    pub avc444: bool,
    pub h264: bool,
    pub thin_client: bool,
    pub small_cache: bool,
    pub capabilities: u32,
}

pub struct InputConfig {
    pub credentials: sspi::AuthIdentity,
    pub security_protocol: nego::SecurityProtocol,
    pub keyboard_type: gcc::KeyboardType,
    pub keyboard_subtype: u32,
    pub keyboard_functional_keys_count: u32,
    pub ime_file_name: String,
    pub dig_product_id: String,
    pub width: u16,

    pub height: u16,
    pub global_channel_name: String,
    pub user_channel_name: String,
    pub graphics_config: Option<GraphicsConfig>,
}

#[cfg(all(feature = "native-tls", not(feature = "rustls")))]
use async_native_tls::TlsStream;

#[cfg(feature = "rustls")]
use tokio_rustls::client::TlsStream;

pub type TlsStreamType = TlsStream<TcpStream>;
