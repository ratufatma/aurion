#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

use zenoh::Session;

use crate::error::NetworkError;

/// Network configuration for the Aurion Zenoh brokerless peer mesh.
///
/// All fields have safe defaults appropriate for local-network operation.
/// Override `peer_endpoints` with WAN locators for internet-facing nodes.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// TCP/QUIC listen endpoints for inbound peer connections.
    ///
    /// Example: `["tcp/0.0.0.0:7447"]`
    pub listen_endpoints: Vec<String>,

    /// Known peer endpoints to connect to on startup.
    ///
    /// Example: `["tcp/192.168.1.50:7447", "tcp/seed.aurion.network:7447"]`
    pub peer_endpoints: Vec<String>,

    /// Enable LAN multicast scouting for zero-configuration local peer discovery.
    ///
    /// Set to `false` for production WAN deployments relying solely on explicit locators.
    pub enable_multicast_scouting: bool,

    /// Enable Zenoh Shared Memory transport for zero-copy IPC between local processes.
    ///
    /// Only effective when the `shared-memory` zenoh feature is enabled.
    pub enable_shared_memory: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_endpoints: vec!["tcp/0.0.0.0:7447".to_owned()],
            peer_endpoints: Vec::new(),
            enable_multicast_scouting: true,
            enable_shared_memory: false,
        }
    }
}

impl NetworkConfig {
    /// Construct a configuration with an explicit listen port for test scenarios.
    pub fn with_port(port: u16) -> Self {
        Self {
            listen_endpoints: vec![format!("tcp/127.0.0.1:{port}")],
            peer_endpoints: Vec::new(),
            enable_multicast_scouting: false,
            enable_shared_memory: false,
        }
    }

    /// Serialize this config into a Zenoh JSON5 configuration string
    /// that enforces `Mode::Peer` and no broker dependencies.
    pub fn to_zenoh_json5(&self) -> String {
        let listen_json = endpoints_to_json5_array(&self.listen_endpoints);
        let connect_json = endpoints_to_json5_array(&self.peer_endpoints);
        let multicast_scouting = self.enable_multicast_scouting;
        let shared_memory = self.enable_shared_memory;

        format!(
            r#"{{
                mode: "peer",
                listen: {{
                    endpoints: {listen_json},
                }},
                connect: {{
                    endpoints: {connect_json},
                }},
                scouting: {{
                    multicast: {{
                        enabled: {multicast_scouting},
                        autoconnect: ["peer"],
                    }},
                    gossip: {{
                        enabled: true,
                        autoconnect: ["peer"],
                    }},
                }},
                transport: {{
                    shared_memory: {{
                        enabled: {shared_memory},
                    }},
                }},
            }}"#
        )
    }
}

fn endpoints_to_json5_array(endpoints: &[String]) -> String {
    let items: Vec<String> = endpoints
        .iter()
        .map(|ep| format!(r#""{ep}""#))
        .collect();
    format!("[{}]", items.join(", "))
}

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
    pub async fn bootstrap(config: &NetworkConfig) -> Result<Self, NetworkError> {
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

    /// Compatibility alias for [`PeerNetworkSession::bootstrap`] used by
    /// existing callers and the local mesh integration tests.
    ///
    /// # Errors
    /// Returns [`NetworkError::TransportError`] if session initialisation fails.
    pub async fn init(config: &NetworkConfig) -> Result<Self, NetworkError> {
        Self::bootstrap(config).await
    }

    /// Access the underlying [`zenoh::Session`] for pub/sub and query declarations.
    pub fn session(&self) -> &Session {
        &self.session
    }
}