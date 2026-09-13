//! Implementasi konkret `NodeRpcHandler` untuk daemon node sovereign Aurion.
//!
//! `AurionNodeService` menggabungkan mesin penyimpanan `redb`, Mempool, dan
//! mesin konsensus (authority) menjadi satu layanan RPC yang dapat dijalankan
//! melalui `RpcServer` dari crate `aurion-rpc`.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use aurion_consensus::{compact_to_target, MAX_TARGET_BITS};
use aurion_consensus::genesis::GENESIS_TIMESTAMP;
use aurion_core::{Block, OutPoint, Transaction};
use aurion_eutxo::state::ExtendedUtxo;
use aurion_eutxo::view::UtxoView;
use aurion_node::authority::AuthorityEngine;
use aurion_node::mempool::Mempool;
use aurion_network::GossipEngine;
use aurion_primitives::{Hash256, Quantum};
use aurion_rpc::server::NodeRpcHandler;
use aurion_rpc::template::BlockTemplate;
use aurion_rpc::template::SubmitResult;
use aurion_storage::StorageEngine;
use tokio::sync::RwLock;

/// Jarak tinggi blok antar halving (interval subsidi).
pub const HALVING_INTERVAL_BLOCKS: u64 = 200_000;
/// Subsidi awal per blok dalam satuan Aur (AUR).
pub const INITIAL_SUBSIDY_AUR: u128 = 99;
/// Jumlah kuantum per satu Aur.
pub const QUANTUM_PER_AUR: u128 = 100_000_000;
/// Batas ukuran payload blok untuk seleksi Mempool ke dalam template.
pub const MAX_BLOCK_PAYLOAD_BYTES: usize = 2 * 1024 * 1024;
/// Jumlah blok yang dipakai sebagai jendela Median Time Past (MTP).
pub const MTP_WINDOW_BLOCKS: u64 = 11;

/// Menghitung subsidi blok untuk ketinggian tertentu. Setiap `HALVING_INTERVAL_BLOCKS`
/// blok, subsidi dibelah dua hingga habis setelah `64` halving.
pub fn calculate_block_subsidy(height: u64) -> Quantum {
    if height.checked_div(HALVING_INTERVAL_BLOCKS).unwrap_or(0) >= 64 {
        return Quantum::ZERO;
    }
    let halvings = height / HALVING_INTERVAL_BLOCKS;
    let base_units = INITIAL_SUBSIDY_AUR
        .checked_mul(QUANTUM_PER_AUR)
        .unwrap_or(0);
    let subsidy_units = base_units.checked_shr(halvings as u32).unwrap_or(0);
    Quantum::from_raw(subsidy_units)
}

/// Layanan RPC node yang memegang storage `redb`, Mempool, mesin authority,
/// dan mesin gossip P2P opsional (None = mode offline/standalone).
pub struct AurionNodeService {
    storage: Arc<StorageEngine>,
    mempool: Arc<RwLock<Mempool>>,
    authority: Arc<AuthorityEngine>,
    gossip: Option<Arc<GossipEngine>>,
}

impl AurionNodeService {
    /// Konstruktor layanan RPC node. `storage` dan `authority` harus berbagi
    /// instance `StorageEngine` yang sama agar tidak terjadi konflik kunci redb.
    ///
    /// `gossip` bersifat opsional: `None` menonaktifkan semua siaran P2P keluar
    /// dan dipakai pada mode offline/standalone serta unit test.
    pub fn new(
        storage: Arc<StorageEngine>,
        authority: Arc<AuthorityEngine>,
        mempool: Arc<RwLock<Mempool>>,
        gossip: Option<Arc<GossipEngine>>,
    ) -> Self {
        Self {
            storage,
            mempool,
            authority,
            gossip,
        }
    }

    /// Mengembalikan (tinggi, hash) dari blok tip pada rantai kanonik;
    /// fallback ke (0, zero-hash) bila ledger kosong.
    fn tip(&self) -> Result<(u64, Hash256), String> {
        match self.storage.get_tip() {
            Ok(Some(tip)) => Ok(tip),
            Ok(None) => Ok((0, Hash256::ZERO)),
            Err(e) => Err(format!("storage read error: {e}")),
        }
    }

    /// Menghitung Median Time Past (MTP) dari jendela `MTP_WINDOW_BLOCKS` blok
    /// terakhir sebelum tip. Fallback ke timestamp genesis bila sampel kosong.
    fn median_time_past(&self, tip_hash: &Hash256) -> Result<u64, String> {
        median_time_past(&self.storage, tip_hash)
    }

    /// Menghitung target work untuk blok berikutnya dari `bits` blok tip.
    /// Fallback ke target maksimum bila tip tidak tersedia atau `bits` tidak valid.
    fn next_work_target(&self, tip_hash: &Hash256) -> Result<Hash256, String> {
        let bits = match self
            .storage
            .get_block(tip_hash)
            .map_err(|e| format!("storage read error: {e}"))?
        {
            Some(block) => block.header.bits,
            None => MAX_TARGET_BITS,
        };
        match compact_to_target(bits) {
            Ok(target_bytes) => Ok(Hash256::from_bytes(target_bytes)),
            Err(_) => {
                tracing::warn!(target = "aurion::mining", "invalid bits {bits:#x}; falling back to max target");
                compact_to_target(MAX_TARGET_BITS)
                    .map(Hash256::from_bytes)
                    .map_err(|e| format!("target computation error: {e}"))
            }
        }
    }

    /// Menghitung biaya transaksi sebagai selisih nilai UTXO input terhadap output.
    /// Menolak transaksi yang merujuk UTXO yang tidak ada di ledger.
    fn calculate_tx_fee(&self, tx: &Transaction) -> Result<Quantum, String> {
        let view = self.utxo_view();
        let mut total_input = Quantum::ZERO;

        for input in &tx.inputs {
            let utxo = view
                .get_utxo(&input.previous_output)
                .ok_or_else(|| format!("input UTXO not in ledger: {}", input.previous_output))?;
            total_input = total_input
                .checked_add(utxo.value)
                .map_err(|_| "input sum overflow".to_string())?;
        }

        let mut total_output = Quantum::ZERO;
        for output in &tx.outputs {
            total_output = total_output
                .checked_add(output.value)
                .map_err(|_| "output sum overflow".to_string())?;
        }

        total_input
            .checked_sub(total_output)
            .map_err(|_| "fee underflow: total outputs exceed total inputs".to_string())
    }

    /// Membuat view UTXO berlapis di atas storage `redb` untuk konsumsi Mempool.
    ///
    /// Catatan model data: redb menyimpan `TxOutput` tanpa metadata `is_coinbase`.
    /// View memperlakukan seluruh UTXO sebagai keluaran non-coinbase yang sudah
    /// matang sehingga pemeriksaan kematangan coinbase dilewati di Mempool.
    fn utxo_view(&self) -> impl UtxoView + 'static {
        let storage = Arc::clone(&self.storage);
        move |outpoint: &OutPoint| -> Option<ExtendedUtxo> {
            let output = storage.get_utxo(outpoint).ok()??;
            Some(ExtendedUtxo::from_output(&output, 0, false))
        }
    }
}

/// Menghitung Median Time Past (MTP) dari jendela `MTP_WINDOW_BLOCKS` blok
/// terakhir sebelum tip. Fallback ke timestamp genesis bila sampel kosong.
///
/// Dipakai bersama oleh layanan RPC node dan listener transaksi inbound
/// (yang tidak memiliki akses ke `AurionNodeService`).
pub(crate) fn median_time_past(
    storage: &StorageEngine,
    tip_hash: &Hash256,
) -> Result<u64, String> {
    let mut samples: Vec<u64> = Vec::with_capacity(MTP_WINDOW_BLOCKS as usize);

    if let Some(tip_block) = storage
        .get_block(tip_hash)
        .map_err(|e| format!("storage read error: {e}"))?
    {
        let mut height = tip_block.header.height;
        while let Some(block) = storage
            .get_block_by_height(height)
            .map_err(|e| format!("storage read error: {e}"))?
        {
            samples.push(block.header.timestamp);
            if height == 0 {
                break;
            }
            height = height.saturating_sub(1);
            if samples.len() >= MTP_WINDOW_BLOCKS as usize {
                break;
            }
        }
    }

    if samples.is_empty() {
        return Ok(GENESIS_TIMESTAMP);
    }
    samples.sort_unstable();
    Ok(samples[samples.len() / 2])
}

#[async_trait]
impl NodeRpcHandler for AurionNodeService {
    async fn handle_get_block_template(&self, _payout_address: &[u8]) -> Result<BlockTemplate, String> {
        let (tip_height, tip_hash) = self.tip()?;
        let next_height = tip_height
            .checked_add(1)
            .ok_or_else(|| "block height overflow".to_string())?;

        let target = self.next_work_target(&tip_hash)?;

        let selected_txs = self
            .mempool
            .read()
            .await
            .select_transactions_for_block(MAX_BLOCK_PAYLOAD_BYTES);

        let mut total_fee = Quantum::ZERO;
        let mut transactions = Vec::with_capacity(selected_txs.len());
        for tx in selected_txs {
            if let Ok(fee) = self.calculate_tx_fee(&tx) {
                total_fee = total_fee
                    .checked_add(fee)
                    .map_err(|_| "fee accumulator overflow".to_string())?;
            }
            transactions.push((*tx).clone());
        }

        let coinbase_subsidy = calculate_block_subsidy(next_height);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("system time error: {e}"))?
            .as_secs();

        let mtp = self.median_time_past(&tip_hash)?;
        let timestamp = if now > mtp {
            now
        } else {
            mtp.checked_add(1)
                .ok_or_else(|| "timestamp overflow".to_string())?
        };

        Ok(BlockTemplate {
            height: next_height,
            previous_block_hash: tip_hash,
            target,
            timestamp,
            coinbase_subsidy,
            total_fee,
            transactions,
        })
    }

    async fn handle_submit_block(&self, block: Block) -> Result<SubmitResult, String> {
        let block_hash = block.block_hash();
        let height = block.header.height;

        match self.authority.process_and_commit_block(&block) {
            Ok(()) => {
                self.mempool
                    .write()
                    .await
                    .process_committed_block(&block);
                tracing::info!(
                    target = "aurion::node",
                    height,
                    hash = %block_hash,
                    txs = block.transactions.len(),
                    "canonical block committed and accepted"
                );

                if let Some(ref gossip) = self.gossip {
                    let gossip_clone = Arc::clone(gossip);
                    let block_clone = block.clone();
                    tokio::spawn(async move {
                        if let Err(e) = gossip_clone.broadcast_block(&block_clone).await {
                            tracing::warn!(
                                target = "aurion::gossip",
                                error = %e,
                                "Failed to broadcast local block to peer mesh"
                            );
                        }
                    });
                }

                Ok(SubmitResult::Accepted { block_hash, height })
            }
            Err(consensus_err) => {
                tracing::warn!(
                    target = "aurion::node",
                    hash = %block_hash,
                    reason = %consensus_err,
                    "block rejected by authority"
                );
                Ok(SubmitResult::Rejected {
                    reason: format!("{consensus_err:?}"),
                })
            }
        }
    }

    async fn handle_send_raw_tx(&self, tx: Transaction) -> Result<Hash256, String> {
        let (tip_height, tip_hash) = self.tip()?;
        let mtp = self.median_time_past(&tip_hash)?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("system time error: {e}"))?
            .as_secs();

        let view = self.utxo_view();
        let tx_for_broadcast = tx.clone();
        let txid = {
            let mut mempool_guard = self.mempool.write().await;
            let txid = mempool_guard
                .insert(tx, &view, tip_height, mtp, now)
                .map_err(|reject| format!("{reject}"))?;
            drop(mempool_guard);
            txid
        };

        tracing::info!(target = "aurion::node", %txid, "transaction admitted to Mempool");

        if let Some(ref gossip) = self.gossip {
            let gossip_clone = Arc::clone(gossip);
            let tx_clone = tx_for_broadcast;
            tokio::spawn(async move {
                if let Err(e) = gossip_clone.broadcast_transaction(&tx_clone).await {
                    tracing::warn!(
                        target = "aurion::gossip",
                        error = %e,
                        "Failed to broadcast local tx to peer mesh"
                    );
                }
            });
        }

        Ok(txid)
    }

    async fn handle_get_tip_status(&self) -> Result<(u64, Hash256, usize), String> {
        let (height, best_block_hash) = self.tip()?;
        let mempool_size = self.mempool.read().await.len();
        Ok((height, best_block_hash, mempool_size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurion_consensus::create_genesis_block;
    use aurion_core::tx::{Datum, TxInput, TxOutput};
    use tempfile::NamedTempFile;

    fn build_service() -> (NamedTempFile, Arc<AurionNodeService>) {
        let file = NamedTempFile::new().unwrap();
        let storage = Arc::new(StorageEngine::open(file.path()).unwrap());
        let mempool = Arc::new(RwLock::new(Mempool::new(
            aurion_node::mempool::DEFAULT_MAX_MEMPOOL_BYTES,
        )));
        let authority = Arc::new(AuthorityEngine::new(Arc::clone(&storage)));
        authority.initialize_genesis().unwrap();
        let service = Arc::new(AurionNodeService::new(storage, authority, mempool, None));
        (file, service)
    }

    #[tokio::test]
    async fn test_get_block_template_after_genesis() {
        let (_file, service) = build_service();

        let template = service
            .handle_get_block_template(b"miner-payout-alice")
            .await
            .unwrap();

        let genesis = create_genesis_block();
        assert_eq!(template.height, 1);
        assert_eq!(template.previous_block_hash, genesis.block_hash());
        assert_eq!(template.coinbase_subsidy, calculate_block_subsidy(1));
        assert_eq!(template.total_fee, Quantum::ZERO);
        assert!(template.transactions.is_empty());
        assert_eq!(
            template.target,
            Hash256::from_bytes(compact_to_target(MAX_TARGET_BITS).unwrap())
        );
    }

    #[tokio::test]
    async fn test_get_tip_status_after_genesis() {
        let (_file, service) = build_service();

        let (height, hash, mempool_size) = service.handle_get_tip_status().await.unwrap();
        assert_eq!(height, 0);
        assert_eq!(hash, create_genesis_block().block_hash());
        assert_eq!(mempool_size, 0);
    }

    #[tokio::test]
    async fn test_submit_block_rejected_on_genesis() {
        let (_file, service) = build_service();

        let genesis = create_genesis_block();
        let result = service.handle_submit_block(genesis).await.unwrap();

        assert!(matches!(result, SubmitResult::Rejected { reason } if !reason.is_empty()));
    }

    #[tokio::test]
    async fn test_send_raw_tx_rejects_missing_utxo() {
        let (_file, service) = build_service();

        let tx = Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_output: OutPoint::new(Hash256::from_bytes([0xAB; 32]), 0),
                unlocking_script: vec![],
                sequence: 0xFFFF_FFFF,
                redeemer: None,
            }],
            outputs: vec![TxOutput {
                value: Quantum::from_raw(1),
                locking_script: vec![0x51],
                datum: Datum::None,
            }],
            locktime: 0,
        };

        let err = service.handle_send_raw_tx(tx).await.unwrap_err();
        assert!(err.contains("not found"), "unexpected error: {err}");
    }

    #[tokio::test]
    async fn test_mined_height1_block_accepted_via_miner_loop() {
        use crate::commands::mine::coinbase::build_coinbase_transaction;
        use crate::commands::mine::engine::{mine_candidate, target_to_bits};
        use aurion_core::{compute_merkle_root, Block, BlockHeader};

        let (_file, service) = build_service();

        let template = service.handle_get_block_template(b"payout").await.unwrap();
        assert_eq!(template.height, 1);

        let coinbase = build_coinbase_transaction(
            template.height,
            vec![0x51],
            template.coinbase_subsidy,
            Quantum::ZERO,
            1,
        )
        .unwrap();

        let txs = vec![coinbase];
        let txids: Vec<Hash256> = txs.iter().map(|tx| tx.txid()).collect();
        let merkle_root = compute_merkle_root(&txids);

        let candidate = BlockHeader {
            version: 1,
            prev_block_hash: template.previous_block_hash,
            merkle_root,
            timestamp: template.timestamp,
            bits: target_to_bits(template.target.as_bytes()),
            nonce: 0,
            height: template.height,
        };

        let solved = mine_candidate(candidate, template.target, true).await.unwrap();
        let block = Block::new(solved, txs);

        let result = service.handle_submit_block(block).await.unwrap();
        assert!(matches!(
            result,
            SubmitResult::Accepted { height: 1, .. }
        ));
    }
}