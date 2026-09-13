#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod codec;
pub mod error;
pub mod message;

pub use codec::{NetworkCodec, MAX_FRAME_PAYLOAD_SIZE, NETWORK_MAGIC};
pub use error::NetworkError;
pub use message::NetworkMessage;
