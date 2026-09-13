#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

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
        let scouting = self.enable_multicast_scouting;

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
                        enabled: {scouting},
                        autoconnect: ["peer"],
                    }},
                    gossip: {{
                        enabled: true,
                        autoconnect: ["peer"],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_produces_peer_mode_json5() {
        let cfg = NetworkConfig::default();
        let json5 = cfg.to_zenoh_json5();
        assert!(json5.contains(r#"mode: "peer""#), "Must enforce peer mode");
        assert!(json5.contains("tcp/0.0.0.0:7447"), "Must include default listen endpoint");
    }

    #[test]
    fn test_with_port_disables_scouting() {
        let cfg = NetworkConfig::with_port(49152);
        let json5 = cfg.to_zenoh_json5();
        assert!(json5.contains("49152"), "Must include configured port");
        assert!(json5.contains("enabled: false"), "Scouting must be disabled in test config");
    }
}
