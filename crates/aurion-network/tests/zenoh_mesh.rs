#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::TcpListener;
use std::time::Duration;

use aurion_core::outpoint::OutPoint;
use aurion_core::tx::{Datum, Transaction, TxInput, TxOutput};
use aurion_network::config::NetworkConfig;
use aurion_network::error::NetworkError;
use aurion_network::frame::{pack_frame, unpack_frame, HEADER_SIZE, MAX_PAYLOAD_SIZE};
use aurion_network::gossip::GossipEngine;
use aurion_network::query::SyncEngine;
use aurion_network::session::PeerNetworkSession;
use aurion_primitives::hash::Hash256;
use aurion_primitives::quantum::Quantum;
use futures::StreamExt;

/// Find an available OS-assigned TCP port.
fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind free port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

fn sample_transaction() -> Transaction {
    Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_output: OutPoint::new(Hash256::from_bytes([0xEE; 32]), 0),
            unlocking_script: vec![0x51],
            sequence: 0xFFFF_FFFF,
            redeemer: None,
        }],
        outputs: vec![TxOutput {
            value: Quantum::from_raw(50_000_000),
            locking_script: vec![0x51],
            datum: Datum::None,
        }],
        locktime: 0,
    }
}

// ── 1. Wire Frame Tests ───────────────────────────────────────────────────────

#[test]
fn test_wire_frame_roundtrip_52b() {
    let payload = b"canonical-block-data-payload-hash-root";
    let frame = pack_frame("block", payload).expect("pack frame");
    assert_eq!(frame.len(), HEADER_SIZE.saturating_add(payload.len()));

    let (command, unpacked) = unpack_frame(&frame).expect("unpack frame");
    assert_eq!(command, "block");
    assert_eq!(unpacked, payload);
}

#[test]
fn test_wire_frame_rejects_corrupted_checksum() {
    let payload = b"critical-financial-transaction";
    let mut frame = pack_frame("tx", payload).expect("pack frame");

    // Corrupt one byte of the payload
    let target_idx = HEADER_SIZE;
    frame[target_idx] ^= 0xA5;

    let err = unpack_frame(&frame).expect_err("should reject corrupted checksum");
    assert_eq!(err, NetworkError::CorruptedChecksum);
}

#[test]
fn test_wire_frame_rejects_oversized_payload() {
    let oversized = vec![0u8; MAX_PAYLOAD_SIZE.saturating_add(1)];
    let err = pack_frame("block", &oversized).expect_err("should reject payload > 4 MB");
    assert_eq!(
        err,
        NetworkError::PayloadTooLarge(MAX_PAYLOAD_SIZE.saturating_add(1))
    );
}

// ── 2. Zenoh Brokerless Peer Integration Test ─────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn test_brokerless_peer_mesh_tx_propagation() {
    let port_a = free_port();
    let port_b = free_port();

    let config_a = NetworkConfig {
        listen_endpoints: vec![format!("tcp/127.0.0.1:{port_a}")],
        peer_endpoints: Vec::new(),
        enable_multicast_scouting: false,
        enable_shared_memory: false,
    };

    let config_b = NetworkConfig {
        listen_endpoints: vec![format!("tcp/127.0.0.1:{port_b}")],
        peer_endpoints: vec![format!("tcp/127.0.0.1:{port_a}")],
        enable_multicast_scouting: false,
        enable_shared_memory: false,
    };

    let session_a = PeerNetworkSession::init(&config_a)
        .await
        .expect("Session A init");
    let session_b = PeerNetworkSession::init(&config_b)
        .await
        .expect("Session B init");

    // Allow Zenoh P2P direct connection to establish
    tokio::time::sleep(Duration::from_millis(300)).await;

    let gossip_b = GossipEngine::new(session_b.session());
    let mut tx_stream = gossip_b.subscribe_txs().await.expect("Session B subscribe");

    // Allow subscription routing to propagate across peer link
    tokio::time::sleep(Duration::from_millis(300)).await;

    let expected_tx = sample_transaction();
    let gossip_a = GossipEngine::new(session_a.session());
    gossip_a
        .broadcast_tx(&expected_tx)
        .await
        .expect("Session A broadcast");

    let received = tokio::time::timeout(Duration::from_secs(5), tx_stream.next())
        .await
        .expect("timeout waiting for tx gossip")
        .expect("stream ended prematurely")
        .expect("decode failed");

    assert_eq!(
        received, expected_tx,
        "Received transaction must match broadcasted transaction"
    );
}

// ── 3. Query/Reply Miner Template Test ────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn test_query_reply_block_template_sync() {
    let port_a = free_port();
    let port_b = free_port();

    let config_a = NetworkConfig {
        listen_endpoints: vec![format!("tcp/127.0.0.1:{port_a}")],
        peer_endpoints: Vec::new(),
        enable_multicast_scouting: false,
        enable_shared_memory: false,
    };

    let config_b = NetworkConfig {
        listen_endpoints: vec![format!("tcp/127.0.0.1:{port_b}")],
        peer_endpoints: vec![format!("tcp/127.0.0.1:{port_a}")],
        enable_multicast_scouting: false,
        enable_shared_memory: false,
    };

    let session_a = PeerNetworkSession::init(&config_a)
        .await
        .expect("Session A init");
    let session_b = PeerNetworkSession::init(&config_b)
        .await
        .expect("Session B init");

    // Allow Zenoh P2P direct connection to establish
    tokio::time::sleep(Duration::from_millis(300)).await;

    let expected_template = b"aurion-genesis-block-template-height-1".to_vec();
    let template_clone = expected_template.clone();

    let sync_a = SyncEngine::new(session_a.session());
    let _queryable = sync_a
        .register_block_template_provider(move || template_clone.clone())
        .await
        .expect("Register template provider");

    // Allow queryable declaration to propagate across peer link
    tokio::time::sleep(Duration::from_millis(300)).await;

    let sync_b = SyncEngine::new(session_b.session());
    let received = tokio::time::timeout(Duration::from_secs(5), sync_b.request_block_template())
        .await
        .expect("timeout waiting for block template query")
        .expect("query failed");

    assert_eq!(
        received, expected_template,
        "Received block template must match provider output"
    );
}
