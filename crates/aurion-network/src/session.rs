#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

use zenoh::Session;

use crate::config::NetworkConfig;
use crate::error::NetworkError;

/// A brokerless Zenoh peer session anchored to the Aurion P2P mesh.
///
/// Enforces `Mode::Peer` exclusively — no client-broker dependencies,
/// no centralized daemon. Sessions discover peers via multicast scouting
/// (LAN) and explicit locators (WAN).
///
/// # Ownership
///
/// `PeerNetworkSession` owns the underlying `zenoh::Session` and keeps it
/// alive for the duration of the node. Drop this struct to cleanly close
/// the session and release all network resources.
pub struct PeerNetworkSession {
    session: Session,
}

impl PeerNetworkSession {
    /// Bootstrap a new brokerless Zenoh peer session from the given [`NetworkConfig`].
    ///
    /// This function:
    /// 1. Serialises `config` into a JSON5 string enforcing `mode: "peer"`.
    /// 2. Calls [`zenoh::open`] with the peer-mode configuration.
    /// 3. Returns `Err(NetworkError::TransportError)` on any Zenoh failure.
    ///
    /// # Errors
    /// Returns [`NetworkError::TransportError`] if session initialisation fails.
    pub async fn init(config: &NetworkConfig) -> Result<Self, NetworkError> {
        let json5 = config.to_zenoh_json5();

        tracing::debug!("Opening Zenoh peer session with config: {json5}");

        let zenoh_config = zenoh::Config::from_json5(&json5)
            .map_err(|e| NetworkError::TransportError(e.to_string()))?;

        let session = zenoh::open(zenoh_config)
            .await
            .map_err(|e| NetworkError::TransportError(e.to_string()))?;

        tracing::info!("Zenoh brokerless peer session established");

        Ok(Self { session })
    }

    /// Access the underlying [`zenoh::Session`] for pub/sub and query declarations.
    pub fn session(&self) -> &Session {
        &self.session
    }
}
