#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

//! Backwards-compatible re-export module.
//!
//! [`NetworkConfig`] now lives in [`crate::session`] alongside
//! [`crate::session::PeerNetworkSession`]; this module keeps the historical
//! `aurion_network::config::NetworkConfig` import path working.

pub use crate::session::NetworkConfig;