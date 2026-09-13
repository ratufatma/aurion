//! Implementasi daemon node sovereign Aurion: inisialisasi storage `redb`,
//! Mempool, mesin authority, sesi P2P Zenoh brokerless (Mode::Peer), serta
//! pemaparan layanan RPC bagi penambang.
//!
//! Bila tidak berjalan dalam mode `--offline`, node mem-bootstrap sesi mesh
//! peer, memasang listener inbound untuk blok dan transaksi dari peer, dan
//! menempelkan `GossipEngine` pada layanan RPC agar blok/transaksi yang
//! dikirim lokal langsung disiarkan keluar.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Args;
use futures::StreamExt;
use tokio::sync::RwLock;

use aurion_core::OutPoint;
use aurion_eutxo::state::ExtendedUtxo;
use aurion_network::{GossipEngine, NetworkConfig, PeerNetworkSession};
use aurion_node::authority::AuthorityEngine;
use aurion_node::mempool::DEFAULT_MAX_MEMPOOL_BYTES;
use aurion_node::mempool::Mempool;
use aurion_primitives::Hash256;
use aurion_rpc::server::RpcServer;
use aurion_storage::StorageEngine;

use crate::rpc_handler::{median_time_past, AurionNodeService};

#[derive(Args, Debug, Clone)]
pub struct NodeArgs {
    /// Directory tempat menyimpan data node (database redb).
    #[arg(short, long, default_value = "./data")]
    pub datadir: PathBuf,

    /// Bind address untuk server RPC node.
    #[arg(long, default_value = "127.0.0.1:8547")]
    pub rpc_bind: String,

    /// Locator P2P listening untuk Zenoh (brokerless Mode::Peer).
    #[arg(long, default_value = "tcp/0.0.0.0:7447")]
    pub p2p_listen: String,

    /// Endpoint peer keluar (dapat diulang: --peer tcp/127.0.0.1:7447).
    #[arg(long = "peer")]
    pub peers: Vec<String>,

    /// Jalankan node dalam mode standalone / offline (P2P nonaktif).
    #[arg(long, default_value_t = false)]
    pub offline: bool,
}

/// Mem-bootstrap sesi peer mesh Zenoh brokerless bila tidak offline.
///
/// Mengembalikan `None` pada mode `--offline`, atau `Arc<GossipEngine>` yang
/// meng-own clone dari `zenoh::Session` sehingga tetap hidup melampaui handle
/// bootstrap awal (dipakai bersama listener dan siaran keluar dari layanan RPC).
async fn bootstrap_gossip(
    args: &NodeArgs,
) -> Result<Option<Arc<GossipEngine>>, Box<dyn std::error::Error + Send + Sync>> {
    if args.offline {
        tracing::info!(
            target = "aurion::network",
            "Node running in offline mode (P2P disabled)"
        );
        return Ok(None);
    }

    let net_cfg = NetworkConfig {
        listen_endpoints: vec![args.p2p_listen.clone()],
        peer_endpoints: args.peers.clone(),
        enable_multicast_scouting: true,
        enable_shared_memory: true,
    };

    tracing::info!(
        target = "aurion::network",
        listen = %args.p2p_listen,
        peers = ?args.peers,
        "Initializing brokerless Zenoh P2P session (Mode::Peer)..."
    );

    let p2p_session = PeerNetworkSession::bootstrap(&net_cfg)
        .await
        .map_err(|e| format!("P2P bootstrap failed: {e}"))?;

    let engine = Arc::new(GossipEngine::new(p2p_session.session().clone()));
    Ok(Some(engine))
}

/// Memasang listener inbound blok dan transaksi di atas mesh `GossipEngine`.
///
/// Setiap kesalahan frame / blok / transaksi dari peer hanya dicatat via log
/// (`warn`/`debug`) dan tidak menghentikan loop listener.
fn spawn_inbound_listeners(
    engine: &Arc<GossipEngine>,
    storage: &Arc<StorageEngine>,
    mempool: &Arc<RwLock<Mempool>>,
    authority: &Arc<AuthorityEngine>,
) {
    // ── Inbound Blocks Listener ─────────────────────────────────────────────
    let block_engine = Arc::clone(engine);
    let authority_sync = Arc::clone(authority);
    let mempool_sync = Arc::clone(mempool);

    tokio::spawn(async move {
        match block_engine.subscribe_blocks().await {
            Ok(mut stream) => {
                tracing::info!(
                    target = "aurion::network",
                    "Subscribed to inbound blocks topic"
                );
                while let Some(res) = stream.next().await {
                    match res {
                        Ok(block) => {
                            let hash = block.block_hash();
                            let height = block.header.height;
                            match authority_sync.process_and_commit_block(&block) {
                                Ok(()) => {
                                    let mut mp = mempool_sync.write().await;
                                    mp.process_committed_block(&block);
                                    drop(mp);
                                    tracing::info!(
                                        target = "aurion::sync",
                                        %hash,
                                        height,
                                        txs = block.transactions.len(),
                                        "Inbound block accepted and committed from peer mesh"
                                    );
                                }
                                Err(e) => {
                                    tracing::debug!(
                                        target = "aurion::sync",
                                        %hash,
                                        height,
                                        reason = %e,
                                        "Inbound block ignored or rejected"
                                    );
                                }
                            }
                        }
                        Err(err) => {
                            tracing::warn!(
                                target = "aurion::network",
                                error = %err,
                                "Error unpacking inbound block frame"
                            );
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    target = "aurion::network",
                    error = %e,
                    "Failed to subscribe to inbound blocks"
                );
            }
        }
    });

    // ── Inbound Transactions Listener ───────────────────────────────────────
    let tx_engine = Arc::clone(engine);
    let storage_tx = Arc::clone(storage);
    let mempool_tx = Arc::clone(mempool);

    tokio::spawn(async move {
        // View UTXO yang dibangun di atas storage `redb` untuk validasi Mempool.
        let storage_view = Arc::clone(&storage_tx);
        let utxo_view = move |outpoint: &OutPoint| -> Option<ExtendedUtxo> {
            let output = storage_view.get_utxo(outpoint).ok()??;
            Some(ExtendedUtxo::from_output(&output, 0, false))
        };

        match tx_engine.subscribe_transactions().await {
            Ok(mut stream) => {
                tracing::info!(
                    target = "aurion::network",
                    "Subscribed to inbound transactions topic"
                );
                while let Some(res) = stream.next().await {
                    match res {
                        Ok(tx) => {
                            let (tip_height, tip_hash) = match storage_tx.get_tip() {
                                Ok(Some(tip)) => tip,
                                Ok(None) => (0, Hash256::ZERO),
                                Err(_) => continue,
                            };
                            let mtp = median_time_past(&storage_tx, &tip_hash).unwrap_or(0);
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();

                            let mut mp = mempool_tx.write().await;
                            match mp.insert(tx, &utxo_view, tip_height, mtp, now) {
                                Ok(txid) => {
                                    drop(mp);
                                    tracing::info!(
                                        target = "aurion::mempool",
                                        %txid,
                                        "Inbound transaction accepted into mempool"
                                    );
                                }
                                Err(e) => {
                                    drop(mp);
                                    tracing::debug!(
                                        target = "aurion::mempool",
                                        reason = %e,
                                        "Inbound transaction rejected"
                                    );
                                }
                            }
                        }
                        Err(err) => {
                            tracing::warn!(
                                target = "aurion::network",
                                error = %err,
                                "Error unpacking inbound tx frame"
                            );
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    target = "aurion::network",
                    error = %e,
                    "Failed to subscribe to inbound transactions"
                );
            }
        }
    });
}

pub async fn run(
    args: NodeArgs,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!(
        target = "aurion::node",
        datadir = ?args.datadir,
        rpc_bind = %args.rpc_bind,
        p2p_listen = %args.p2p_listen,
        peers = ?args.peers,
        offline = args.offline,
        "Initializing Aurion Sovereign Node daemon..."
    );

    std::fs::create_dir_all(&args.datadir)?;
    let db_path = args.datadir.join("aurion.redb");

    let gossip_engine = bootstrap_gossip(&args).await?;

    execute_node(args.rpc_bind.clone(), db_path, gossip_engine).await
}

/// Mem-boot node: membuka storage, Mempool, sesi P2P, mesin authority, memuat
/// genesis, memasang listener inbound gossip, lalu menjalankan server RPC
/// sampai sinyal `Ctrl-C` diterima.
pub async fn execute_node(
    rpc_bind: String,
    db_path: PathBuf,
    gossip_engine: Option<Arc<GossipEngine>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let storage = Arc::new(StorageEngine::open(&db_path)?);
    let mempool = Arc::new(RwLock::new(Mempool::new(DEFAULT_MAX_MEMPOOL_BYTES)));
    let authority = Arc::new(AuthorityEngine::new(Arc::clone(&storage)));

    authority.initialize_genesis()?;

    if let Some(ref engine) = gossip_engine {
        spawn_inbound_listeners(engine, &storage, &mempool, &authority);
    }

    let handler = Arc::new(AurionNodeService::new(
        Arc::clone(&storage),
        Arc::clone(&authority),
        Arc::clone(&mempool),
        gossip_engine,
    ));

    let rpc_server = RpcServer::new(rpc_bind.clone(), handler);
    tokio::spawn(async move {
        if let Err(e) = rpc_server.run().await {
            tracing::error!(
                target = "aurion::rpc",
                error = %e,
                "RPC server terminated unexpectedly"
            );
        }
    });

    tracing::info!(
        target = "aurion::node",
        bind = %rpc_bind,
        "Node active, waiting for miner and peer traffic"
    );

    tokio::signal::ctrl_c().await?;
    tracing::info!(target = "aurion::node", "shutting down node daemon gracefully...");

    Ok(())
}