#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod config;
pub mod error;
pub mod frame;
pub mod gossip;
pub mod query;
pub mod session;
pub mod topics;

pub use config::NetworkConfig;
pub use error::NetworkError;
pub use frame::{pack_frame, unpack_frame, HEADER_SIZE, MAGIC_BYTES, MAX_PAYLOAD_SIZE};
pub use gossip::GossipEngine;
pub use query::SyncEngine;
pub use session::PeerNetworkSession;
