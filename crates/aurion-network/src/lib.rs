#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

pub mod codec;
pub mod error;
pub mod message;

pub use codec::{NetworkCodec, MAX_FRAME_PAYLOAD_SIZE, NETWORK_MAGIC};
pub use error::NetworkError;
pub use message::NetworkMessage;
