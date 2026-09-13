//! Implementasi daemon node sovereign Aurion: inisialisasi storage `redb`,
//! Mempool, mesin authority, serta pemaparan layanan RPC bagi penambang.

use std::path::PathBuf;
use std::sync::Arc;

use clap::Args;
use tokio::sync::RwLock;

use crate::rpc_handler::AurionNodeService;

use aurion_node::authority::AuthorityEngine;
use aurion_node::mempool::DEFAULT_MAX_MEMPOOL_BYTES;
use aurion_node::mempool::Mempool;
use aurion_rpc::server::RpcServer;
use aurion_storage::StorageEngine;

#[derive(Args, Debug)]
pub struct NodeArgs {
    /// Directory tempat menyimpan data node (database redb).
    #[arg(short, long, default_value = "./data")]
    pub datadir: PathBuf,

    /// Port P2P listening.
    #[arg(short, long, default_value_t = 8333)]
    pub port: u16,

    /// Bind address untuk server RPC node.
    #[arg(long, default_value = "127.0.0.1:8547")]
    pub rpc_bind: String,
}

pub async fn run(
    args: NodeArgs,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!(
        target = "aurion::node",
        datadir = ?args.datadir,
        port = args.port,
        rpc_bind = %args.rpc_bind,
        "Initializing Aurion Sovereign Node daemon..."
    );

    std::fs::create_dir_all(&args.datadir)?;
    let db_path = args.datadir.join("aurion.redb");

    execute_node(args.rpc_bind, db_path).await
}

/// Mem-boot node: membuka storage, Mempool, mesin authority, memuat genesis,
/// lalu menjalankan server RPC sampai sinyal `Ctrl-C` diterima.
pub async fn execute_node(
    rpc_bind: String,
    db_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let storage = Arc::new(StorageEngine::open(&db_path)?);
    let mempool = Arc::new(RwLock::new(Mempool::new(DEFAULT_MAX_MEMPOOL_BYTES)));
    let authority = Arc::new(AuthorityEngine::new(Arc::clone(&storage)));

    authority.initialize_genesis()?;

    let handler = Arc::new(AurionNodeService::new(
        Arc::clone(&storage),
        Arc::clone(&authority),
        Arc::clone(&mempool),
    ));

    let rpc_server = RpcServer::new(rpc_bind.clone(), handler);
    tokio::spawn(async move {
        if let Err(e) = rpc_server.run().await {
            tracing::error!(target = "aurion::rpc", error = %e, "RPC server terminated unexpectedly");
        }
    });

    tracing::info!(
        target = "aurion::node",
        bind = %rpc_bind,
        "Node active, waiting for miner requests"
    );

    tokio::signal::ctrl_c().await?;
    tracing::info!(target = "aurion::node", "shutting down node daemon gracefully...");

    Ok(())
}