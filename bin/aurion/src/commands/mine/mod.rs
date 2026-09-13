//! Penambangan Aurion: loop RPC asinkron yang mengambil `BlockTemplate`,
//! menyusun coinbase, mencari nonce, lalu menyerahkan blok ke node.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

pub mod coinbase;
pub mod engine;

use std::time::Duration;

use aurion_core::{compute_merkle_root, Block, BlockHeader};
use aurion_primitives::{Hash256, Quantum};
use aurion_rpc::client::RpcClient;
use aurion_rpc::template::SubmitResult;
use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct MineArgs {
    /// RPC endpoint of the running aurion node daemon
    #[arg(long, default_value = "127.0.0.1:8547")]
    pub rpc: String,

    /// Hex-encoded payout locking script for coinbase rewards
    #[arg(long, default_value = "76a914000000000000000000000000000000000000000088ac")]
    pub payout_script: String,

    /// Force CPU mining via Rayon (bypass GPU)
    #[arg(long, default_value_t = false)]
    pub cpu_only: bool,
}

pub async fn run(args: MineArgs) -> Result<(), Box<dyn std::error::Error>> {
    run_miner(args).await
}

/// Loop penambangan adaptif: mengambil template dari node, membangun kandidat
/// blok, mengeksekusi pencarian PoW, dan menyerahkan hasilnya. Tidak pernah
/// panik ketika RPC sedang tidak tersedia — menunggu jeda 2 detik lalu mencoba
/// lagi.
pub async fn run_miner(args: MineArgs) -> Result<(), Box<dyn std::error::Error>> {
    let payout_script = hex::decode(&args.payout_script)
        .map_err(|e| std::io::Error::other(format!("invalid payout script hex: {e}")))?;
    let client = RpcClient::new(args.rpc.clone());
    let mut extra_nonce: u64 = 0;

    tracing::info!(rpc = %args.rpc, cpu_only = args.cpu_only, "starting Aurion mining loop");

    loop {
        let template = match client.get_block_template(&payout_script).await {
            Ok(template) => template,
            Err(err) => {
                tracing::warn!(error = %err, "failed to fetch block template; retrying in 2s");
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        extra_nonce = extra_nonce.wrapping_add(1);

        // Validator L1 (`verify_block_structure`) membatasi output coinbase
        // hingga subsisdi protokol; biaya Mempool belum dikreditkan ke penambang.
        let coinbase = coinbase::build_coinbase_transaction(
            template.height,
            payout_script.clone(),
            template.coinbase_subsidy,
            Quantum::ZERO,
            extra_nonce,
        )
        .map_err(std::io::Error::other)?;

        let mut txs = Vec::with_capacity(template.transactions.len().saturating_add(1));
        txs.push(coinbase);
        txs.extend(template.transactions);

        let txids: Vec<Hash256> = txs.iter().map(|tx| tx.txid()).collect();
        let merkle_root = compute_merkle_root(&txids);

        let candidate = BlockHeader {
            version: 1,
            prev_block_hash: template.previous_block_hash,
            merkle_root,
            timestamp: template.timestamp,
            bits: engine::target_to_bits(template.target.as_bytes()),
            nonce: 0,
            height: template.height,
        };

        tracing::info!(height = template.height, "mining candidate block");
        let solved = engine::mine_candidate(candidate, template.target, args.cpu_only)
            .await
            .map_err(std::io::Error::other)?;
        let block = Block::new(solved, txs);

        match client.submit_block(block).await {
            Ok(SubmitResult::Accepted { block_hash, height }) => {
                tracing::info!(%block_hash, height, "block accepted by node");
            }
            Ok(SubmitResult::Rejected { reason }) => {
                tracing::error!(reason, "block rejected by node consensus");
            }
            Err(err) => {
                tracing::error!(error = %err, "block submission transport failure");
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}