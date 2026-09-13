#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

use std::net::TcpListener;
use std::time::Duration;

use aurion_core::block::{Block, BlockHeader};
use aurion_core::outpoint::OutPoint;
use aurion_core::tx::{Datum, Transaction, TxInput, TxOutput};
use aurion_primitives::codec::CanonicalCodec;
use aurion_primitives::hash::Hash256;
use aurion_primitives::quantum::Quantum;

use crate::config::NetworkConfig;
use crate::error::NetworkError;
use crate::frame::{pack_frame, unpack_frame, HEADER_SIZE, MAX_PAYLOAD_SIZE};
use crate::gossip::GossipEngine;
use crate::query::SyncEngine;
use crate::session::PeerNetworkSession;

use futures::StreamExt;

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

fn sample_block() -> Block {
    let tx = sample_transaction();
    let merkle_root = tx.txid();
    Block {
        header: BlockHeader {
            version: 1,
            prev_block_hash: Hash256::ZERO,
            merkle_root,
            timestamp: 1_773_446_400,
            bits: 0x1d00_ffff,
            nonce: 0,
            height: 1,
        },
        transactions: vec![tx],
    }
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind free port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

// ── 1. Wire Frame Tests ───────────────────────────────────────────────────────

#[test]
fn test_frame_roundtrip_52_byte_header() {
    let payload = b"canonical-block-data-payload-hash-root";
    let frame = pack_frame("block", payload).expect("pack frame");
    assert_eq!(frame.len(), HEADER_SIZE.saturating_add(payload.len()));

    let (command, unpacked) = unpack_frame(&frame).expect("unpack frame");
    assert_eq!(command, "block");
    assert_eq!(unpacked, payload);
}

#[test]
fn test_frame_rejects_oversized_payload() {
    let oversized = vec![0u8; MAX_PAYLOAD_SIZE.saturating_add(1)];
    let err = pack_frame("block", &oversized).expect_err("should reject payload > 4 MB");
    assert_eq!(
        err,
        NetworkError::PayloadTooLarge {
            declared: MAX_PAYLOAD_SIZE.saturating_add(1),
            max: MAX_PAYLOAD_SIZE,
        }
    );
}

#[test]
fn test_frame_rejects_invalid_magic() {
    let frame = pack_frame("block", b"data").expect("pack frame");
    let mut corrupt = frame.clone();
    corrupt[0] = b'X';
    let err = unpack_frame(&corrupt).expect_err("should reject bad magic");
    assert_eq!(err, NetworkError::InvalidMagic(*b"XUR\x01"));
}

#[test]
fn test_frame_rejects_checksum_mismatch() {
    let payload = b"critical-financial-transaction";
    let mut frame = pack_frame("block", payload).expect("pack frame");
    let corrupt_idx = HEADER_SIZE;
    frame[corrupt_idx] ^= 0xA5;
    let err = unpack_frame(&frame).expect_err("should reject corrupted checksum");
    assert_eq!(err, NetworkError::ChecksumMismatch);
}

#[test]
fn test_frame_rejects_truncated_payload() {
    let payload = vec![0xAB; 16];
    let frame = pack_frame("block", &payload).expect("pack frame");
    let truncated = frame[..HEADER_SIZE.saturating_add(12)].to_vec();
    let err = unpack_frame(&truncated).expect_err("should reject truncated payload");
    assert_eq!(
        err,
        NetworkError::TruncatedPayload {
            declared: 16,
            actual: 12,
        }
    );
}

#[test]
fn test_frame_rejects_command_too_long() {
    let err = pack_frame("blockprotocolxx", b"x").expect_err("should reject command > 12 bytes");
    assert_eq!(
        err,
        NetworkError::CommandTooLong("blockprotocolxx".to_owned())
    );
}

#[test]
fn test_unpack_rejects_incomplete_header() {
    let err = unpack_frame(&[0u8; 10]).expect_err("should reject < 52 byte frame");
    assert_eq!(err, NetworkError::IncompleteHeader(10));
}

// ── 2. NetworkConfig / Peer-Mode Enforcement Tests ────────────────────────────

#[test]
fn test_default_config_enforces_peer_mode() {
    let cfg = NetworkConfig::default();
    let json5 = cfg.to_zenoh_json5();
    assert!(json5.contains(r#"mode: "peer""#), "Must enforce peer mode");
    assert!(
        json5.contains("tcp/0.0.0.0:7447"),
        "Must include default listen endpoint"
    );
}

#[test]
fn test_with_port_disables_scouting() {
    let cfg = NetworkConfig::with_port(49152);
    let json5 = cfg.to_zenoh_json5();
    assert!(json5.contains("49152"), "Must include configured port");
    assert!(
        json5.contains("enabled: false"),
        "Scouting must be disabled in test config"
    );
}

#[test]
fn test_shared_memory_toggle_serializes() {
    let cfg = NetworkConfig {
        enable_shared_memory: true,
        ..NetworkConfig::default()
    };
    assert!(cfg.to_zenoh_json5().contains("enabled: true"));
}

// ── 3. Gossip Frame Decode Tests ──────────────────────────────────────────────

#[test]
fn test_decode_block_frame_roundtrip() {
    let block = sample_block();
    let payload = block.encode_canonical();
    let frame = pack_frame("block", &payload).expect("pack block frame");
    let decoded = GossipEngine::decode_block_frame(&frame).expect("decode block");
    assert_eq!(decoded, block);
}

#[test]
fn test_decode_tx_frame_roundtrip() {
    let tx = sample_transaction();
    let payload = tx.encode_canonical();
    let frame = pack_frame("tx", &payload).expect("pack tx frame");
    let decoded = GossipEngine::decode_tx_frame(&frame).expect("decode tx");
    assert_eq!(decoded, tx);
}

#[test]
fn test_decode_block_frame_rejects_wrong_command() {
    let block = sample_block();
    let payload = block.encode_canonical();
    let frame = pack_frame("tx", &payload).expect("pack frame");
    assert!(matches!(
        GossipEngine::decode_block_frame(&frame).expect_err("must reject wrong command"),
        NetworkError::CodecError(_)
    ));
}

#[test]
fn test_decode_tx_frame_rejects_corrupted_checksum() {
    let tx = sample_transaction();
    let payload = tx.encode_canonical();
    let mut frame = pack_frame("tx", &payload).expect("pack tx frame");
    let last = frame.len().saturating_sub(1);
    frame[last] ^= 0xFF;
    let err = GossipEngine::decode_tx_frame(&frame).expect_err("must reject corrupted checksum");
    assert_eq!(err, NetworkError::ChecksumMismatch);
}

// ── 4. Zenoh Brokerless Peer Mesh Integration Tests ───────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn test_brokerless_peer_mesh_local_gossip() {
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

    assert!(
        config_a.to_zenoh_json5().contains(r#"mode: "peer""#),
        "Both sessions must bootstrap in brokerless peer mode"
    );

    let session_a = PeerNetworkSession::bootstrap(&config_a)
        .await
        .expect("Session A bootstrap");
    let session_b = PeerNetworkSession::bootstrap(&config_b)
        .await
        .expect("Session B bootstrap");

    // Allow Zenoh P2P direct connection to establish
    tokio::time::sleep(Duration::from_millis(300)).await;

    let gossip_b = GossipEngine::new(session_b.session());
    let mut tx_stream = gossip_b
        .subscribe_transactions()
        .await
        .expect("Session B subscribe");

    // Allow subscription routing to propagate across the peer link
    tokio::time::sleep(Duration::from_millis(300)).await;

    let expected_tx = sample_transaction();
    let gossip_a = GossipEngine::new(session_a.session());
    gossip_a
        .broadcast_transaction(&expected_tx)
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

    let session_a = PeerNetworkSession::bootstrap(&config_a)
        .await
        .expect("Session A bootstrap");
    let session_b = PeerNetworkSession::bootstrap(&config_b)
        .await
        .expect("Session B bootstrap");

    // Allow Zenoh P2P direct connection to establish
    tokio::time::sleep(Duration::from_millis(300)).await;

    let expected_template = b"aurion-genesis-block-template-height-1".to_vec();
    let template_clone = expected_template.clone();

    let sync_a = SyncEngine::new(session_a.session());
    let _queryable = sync_a
        .register_block_template_provider(move || template_clone.clone())
        .await
        .expect("Register template provider");

    // Allow queryable declaration to propagate across the peer link
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