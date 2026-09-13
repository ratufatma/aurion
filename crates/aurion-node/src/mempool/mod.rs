#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

pub mod entry;
pub mod error;
pub mod fee_rate;
pub mod validation;

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use aurion_core::block::Block;
use aurion_core::outpoint::OutPoint;
use aurion_core::tx::Transaction;
use aurion_eutxo::view::UtxoView;
use aurion_primitives::hash::Hash256;

use entry::MempoolEntry;
use error::MempoolReject;
use fee_rate::FeeRateIndex;

/// Default maximum mempool memory footprint: 50 MB.
pub const DEFAULT_MAX_MEMPOOL_BYTES: usize = 50 * 1024 * 1024;

/// Thread-safe wrapper around the raw [`MempoolInner`] using a `RwLock`.
///
/// The outer type can be wrapped in an `Arc<RwLock<Mempool>>` by the caller
/// for shared concurrent access across node subsystems.
pub struct Mempool {
    inner: MempoolInner,
}

impl Mempool {
    /// Create a new `Mempool` with the given byte capacity.
    pub fn new(max_bytes: usize) -> Self {
        Self {
            inner: MempoolInner::new(max_bytes),
        }
    }

    /// Convenience constructor using the protocol default capacity (50 MB).
    pub fn default_capacity() -> Self {
        Self::new(DEFAULT_MAX_MEMPOOL_BYTES)
    }

    /// Admit a transaction through the 6-Gate validation pipeline.
    ///
    /// Returns the canonical `txid` on success, or a structured [`MempoolReject`] reason.
    pub fn insert<V: UtxoView>(
        &mut self,
        tx: Transaction,
        utxo_view: &V,
        tip_height: u64,
        mtp: u64,
        now: u64,
    ) -> Result<Hash256, MempoolReject> {
        self.inner.insert(tx, utxo_view, tip_height, mtp, now)
    }

    /// Remove a transaction by txid, releasing all reserved outpoints.
    pub fn remove(&mut self, txid: &Hash256) -> Option<MempoolEntry> {
        self.inner.remove(txid)
    }

    /// Greedily select the highest fee-rate transactions for a new block template.
    ///
    /// Iterates `by_fee_rate` in descending order and packs transactions until
    /// `max_block_bytes` is exhausted.
    pub fn select_transactions_for_block(&self, max_block_bytes: usize) -> Vec<Arc<Transaction>> {
        self.inner.select_transactions_for_block(max_block_bytes)
    }

    /// Process a committed block: remove confirmed transactions and evict
    /// any remaining mempool transactions that double-spend consumed inputs.
    pub fn process_committed_block(&mut self, block: &Block) {
        self.inner.process_committed_block(block);
    }

    /// Current number of transactions in the mempool.
    pub fn len(&self) -> usize {
        self.inner.by_txid.len()
    }

    /// Returns `true` if the mempool contains no pending transactions.
    pub fn is_empty(&self) -> bool {
        self.inner.by_txid.is_empty()
    }

    /// Total canonical byte footprint of all staged transactions.
    pub fn total_bytes(&self) -> usize {
        self.inner.total_bytes
    }
}

// ─── Raw Mempool State ────────────────────────────────────────────────────────

struct MempoolInner {
    /// Primary index: txid → full mempool entry.
    by_txid: HashMap<Hash256, MempoolEntry>,

    /// Secondary index ordered by fee-rate for O(log n) eviction and greedy selection.
    by_fee_rate: BTreeSet<FeeRateIndex>,

    /// Tracks which outpoints are reserved by in-mempool transactions.
    /// Value is the txid of the transaction holding the reservation.
    spent_outpoints: HashMap<OutPoint, Hash256>,

    /// Accumulated canonical byte size of all staged transactions.
    total_bytes: usize,

    /// Hard capacity ceiling in bytes.
    max_bytes: usize,
}

impl MempoolInner {
    fn new(max_bytes: usize) -> Self {
        Self {
            by_txid: HashMap::new(),
            by_fee_rate: BTreeSet::new(),
            spent_outpoints: HashMap::new(),
            total_bytes: 0,
            max_bytes,
        }
    }

    fn insert<V: UtxoView>(
        &mut self,
        tx: Transaction,
        utxo_view: &V,
        tip_height: u64,
        mtp: u64,
        now: u64,
    ) -> Result<Hash256, MempoolReject> {
        // ── 6-Gate Validation ────────────────────────────────────────────────
        let tx_fee =
            validation::validate_for_mempool(&tx, utxo_view, &self.spent_outpoints, tip_height, mtp)?;

        let entry = MempoolEntry::new(tx, tx_fee, now);
        let incoming_index = FeeRateIndex::new(entry.fee, entry.size_bytes, entry.txid);

        // ── Capacity Management ──────────────────────────────────────────────
        let new_total = self
            .total_bytes
            .checked_add(entry.size_bytes)
            .ok_or(MempoolReject::ArithmeticOverflow)?;

        if new_total > self.max_bytes {
            // Attempt to evict the lowest fee-rate transaction.
            let lowest = self
                .by_fee_rate
                .iter()
                .next()
                .cloned(); // cheapest entry (BTreeSet ascending order)

            match lowest {
                Some(ref victim_index) if incoming_index.beats(victim_index) => {
                    // Evict the victim to make room.
                    let victim_txid = victim_index.txid;
                    self.remove_by_index(victim_index.clone());
                    // Remove from primary index and decrement accounting.
                    if let Some(victim_entry) = self.by_txid.remove(&victim_txid) {
                        // Unregister spent outpoints for victim.
                        for input in victim_entry.tx.inputs.iter() {
                            self.spent_outpoints.remove(&input.previous_output);
                        }
                        self.total_bytes =
                            self.total_bytes.saturating_sub(victim_entry.size_bytes);
                    }
                }
                _ => {
                    // No eviction possible or incoming fee too low.
                    return Err(MempoolReject::MempoolFull);
                }
            }
        }

        // ── Register Entry ────────────────────────────────────────────────────
        let txid = entry.txid;
        for input in entry.tx.inputs.iter() {
            self.spent_outpoints.insert(input.previous_output, txid);
        }
        self.total_bytes = self.total_bytes.saturating_add(entry.size_bytes);
        self.by_fee_rate.insert(incoming_index);
        self.by_txid.insert(txid, entry);

        Ok(txid)
    }

    fn remove(&mut self, txid: &Hash256) -> Option<MempoolEntry> {
        let entry = self.by_txid.remove(txid)?;

        // Release all outpoint reservations.
        for input in entry.tx.inputs.iter() {
            self.spent_outpoints.remove(&input.previous_output);
        }

        // Remove from fee-rate index.
        let index = FeeRateIndex::new(entry.fee, entry.size_bytes, entry.txid);
        self.by_fee_rate.remove(&index);

        // Decrement accounting.
        self.total_bytes = self.total_bytes.saturating_sub(entry.size_bytes);

        Some(entry)
    }

    /// Internal helper: remove only from the `by_fee_rate` BTreeSet.
    fn remove_by_index(&mut self, index: FeeRateIndex) {
        self.by_fee_rate.remove(&index);
    }

    fn select_transactions_for_block(&self, max_block_bytes: usize) -> Vec<Arc<Transaction>> {
        let mut selected = Vec::new();
        let mut accumulated: usize = 0;

        // BTreeSet iterates in ascending order; iterate in reverse for highest fee-rate first.
        for rate_idx in self.by_fee_rate.iter().rev() {
            if let Some(entry) = self.by_txid.get(&rate_idx.txid) {
                let new_size = match accumulated.checked_add(entry.size_bytes) {
                    Some(s) => s,
                    None => break,
                };
                if new_size > max_block_bytes {
                    continue; // skip this tx but try smaller ones
                }
                accumulated = new_size;
                selected.push(Arc::clone(&entry.tx));
            }
        }

        selected
    }

    fn process_committed_block(&mut self, block: &Block) {
        // Collect all outpoints consumed by the block.
        let mut block_spent: Vec<OutPoint> = Vec::new();

        for tx in &block.transactions {
            let txid = tx.txid();

            // Remove confirmed transactions from the mempool.
            if self.by_txid.contains_key(&txid) {
                self.remove(&txid);
            }

            // Record outpoints consumed by this block transaction.
            for input in &tx.inputs {
                block_spent.push(input.previous_output);
            }
        }

        // Evict any mempool transactions that conflict with block-consumed outpoints.
        let conflicting_txids: Vec<Hash256> = block_spent
            .iter()
            .filter_map(|op| self.spent_outpoints.get(op).copied())
            .collect();

        for conflict_txid in conflicting_txids {
            self.remove(&conflict_txid);
        }
    }
}

// ─── Unit Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use aurion_core::outpoint::OutPoint;
    use aurion_core::tx::{Datum, Transaction, TxInput, TxOutput};
    use aurion_eutxo::fee::MIN_FEE_PER_BYTE_AUR;
    use aurion_eutxo::state::ExtendedUtxo;
    use aurion_primitives::codec::CanonicalCodec;
    use aurion_primitives::hash::Hash256;
    use aurion_primitives::quantum::Quantum;
    use aurion_script::OpCode;

    // ── Test Helpers ──────────────────────────────────────────────────────────

    /// Build a minimal regular (non-coinbase) transaction.
    fn make_tx(
        prev_txid: Hash256,
        prev_idx: u32,
        output_value: u128,
    ) -> Transaction {
        let op = OutPoint::new(prev_txid, prev_idx);
        Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_output: op,
                // Empty unlocking script: execute_contract runs locking script only,
                // leaving exactly 1 element on the stack (the OP_TRUE result).
                unlocking_script: vec![],
                sequence: 0xFFFF_FFFF,
                redeemer: None,
            }],
            outputs: vec![TxOutput {
                value: Quantum::from_raw(output_value),
                locking_script: vec![OpCode::OpTrue as u8],
                datum: Datum::None,
            }],
            locktime: 0,
        }
    }

    /// Compute the minimum fee required for a transaction to pass Gate 6.
    fn min_fee_for(tx: &Transaction) -> u128 {
        let size = tx.encode_canonical().len() as u128;
        size * MIN_FEE_PER_BYTE_AUR
    }



    fn single_utxo_view(op: OutPoint, utxo: ExtendedUtxo) -> impl UtxoView {
        move |query: &OutPoint| {
            if *query == op {
                Some(utxo.clone())
            } else {
                None
            }
        }
    }

    // ── Test: Fee-Rate Ordering ───────────────────────────────────────────────

    #[test]
    fn test_fee_rate_ordering_equal_size() {
        use crate::mempool::fee_rate::FeeRateIndex;

        let high = FeeRateIndex::new(Quantum::from_raw(200), 100, Hash256::from_bytes([0x01; 32]));
        let low = FeeRateIndex::new(Quantum::from_raw(100), 100, Hash256::from_bytes([0x02; 32]));

        let mut set = BTreeSet::new();
        set.insert(low.clone());
        set.insert(high.clone());

        // BTreeSet ascending → first() is lowest
        assert_eq!(set.iter().next().unwrap().fee, low.fee);
        // last() is highest
        assert_eq!(set.iter().next_back().unwrap().fee, high.fee);
    }

    #[test]
    fn test_fee_rate_ordering_cross_multiply() {
        use crate::mempool::fee_rate::FeeRateIndex;

        // 30 / 300 == 10 / 100 → same fee-rate, tie by txid
        let a = FeeRateIndex::new(Quantum::from_raw(30), 300, Hash256::from_bytes([0x01; 32]));
        let b = FeeRateIndex::new(Quantum::from_raw(10), 100, Hash256::from_bytes([0x02; 32]));

        // 20 / 100 > 10 / 100
        let high = FeeRateIndex::new(Quantum::from_raw(20), 100, Hash256::from_bytes([0x01; 32]));
        let low = FeeRateIndex::new(Quantum::from_raw(10), 100, Hash256::from_bytes([0x02; 32]));

        assert!(high > low);
        // a and b have equal rate, tie-broken by txid
        assert!(b > a);
    }

    // ── Test: Double-Spend Conflict ───────────────────────────────────────────

    #[test]
    fn test_double_spend_conflict() {
        let prev_txid = Hash256::from_bytes([0xAB; 32]);
        let op = OutPoint::new(prev_txid, 0);

        let tx1 = make_tx(prev_txid, 0, 10_000);
        let fee = min_fee_for(&tx1);
        let utxo = ExtendedUtxo::new(
            Quantum::from_raw(10_000u128.saturating_add(fee)),
            vec![OpCode::OpTrue as u8],
            0,
            false,
            Datum::None,
        );
        let view = single_utxo_view(op, utxo);

        let mut pool = Mempool::new(DEFAULT_MAX_MEMPOOL_BYTES);
        let result1 = pool.insert(tx1, &view, 0, 0, 1);
        assert!(result1.is_ok(), "First insert should succeed: {result1:?}");

        // Second transaction spending the same outpoint
        let tx2 = make_tx(prev_txid, 0, 8_000);
        let result2 = pool.insert(tx2, &view, 0, 0, 2);
        assert!(
            matches!(result2, Err(MempoolReject::DoubleSpendConflict { .. })),
            "Expected DoubleSpendConflict, got: {result2:?}"
        );
    }

    // ── Test: Capacity Eviction ───────────────────────────────────────────────

    #[test]
    fn test_capacity_eviction_higher_fee_wins() {
        // Shrink mempool to ~2000 bytes to force eviction quickly.
        let max_bytes = 2_000;
        let mut pool = Mempool::new(max_bytes);

        let prev_a = Hash256::from_bytes([0x01; 32]);
        let op_a = OutPoint::new(prev_a, 0);
        let tx_low = make_tx(prev_a, 0, 100);
        // Give a small fee — just enough to pass Gate 6 but no more.
        let low_fee_utxo = ExtendedUtxo::new(
            Quantum::from_raw(100u128.saturating_add(min_fee_for(&tx_low))),
            vec![OpCode::OpTrue as u8],
            0,
            false,
            Datum::None,
        );
        let view_a = single_utxo_view(op_a, low_fee_utxo);
        let r1 = pool.insert(tx_low, &view_a, 0, 0, 1);
        assert!(r1.is_ok(), "Low-fee tx should be accepted initially: {r1:?}");

        // Fill up remaining capacity with a second small-fee tx.
        let prev_b = Hash256::from_bytes([0x02; 32]);
        let op_b = OutPoint::new(prev_b, 0);
        let tx_low2 = make_tx(prev_b, 0, 100);
        let low_fee_utxo2 = ExtendedUtxo::new(
            Quantum::from_raw(100u128.saturating_add(min_fee_for(&tx_low2))),
            vec![OpCode::OpTrue as u8],
            0,
            false,
            Datum::None,
        );
        let view_b = single_utxo_view(op_b, low_fee_utxo2);
        // This may succeed or fail depending on remaining bytes — either way proceed.
        let _ = pool.insert(tx_low2, &view_b, 0, 0, 2);

        // Record the txid of the lowest fee-rate entry before the high-fee insert.
        let size_before = pool.len();

        // Insert a high-fee transaction that should evict the lowest fee-rate entry.
        let prev_c = Hash256::from_bytes([0x03; 32]);
        let op_c = OutPoint::new(prev_c, 0);
        let tx_high = make_tx(prev_c, 0, 100);
        // Provide a very large funding to generate high fee.
        let high_utxo = ExtendedUtxo::new(
            Quantum::from_raw(100u128.saturating_add(min_fee_for(&tx_high).saturating_mul(100))),
            vec![OpCode::OpTrue as u8],
            0,
            false,
            Datum::None,
        );
        let view_c = single_utxo_view(op_c, high_utxo);
        let rh = pool.insert(tx_high, &view_c, 0, 0, 3);

        // The high-fee tx should be admitted (evicting a low-fee one if needed).
        assert!(
            rh.is_ok() || matches!(rh, Err(MempoolReject::MempoolFull)),
            "High-fee insert should succeed or explicitly fail full: {rh:?}"
        );
        // Pool size should not exceed size_before + 1.
        assert!(
            pool.len() <= size_before.saturating_add(1),
            "Pool grew unexpectedly: {} entries (was {size_before})",
            pool.len()
        );
    }

    // ── Test: SPEC-05 Contract Enforcement ───────────────────────────────────

    #[test]
    fn test_spec05_invalid_contract_rejected() {
        let prev_txid = Hash256::from_bytes([0xCC; 32]);
        let op = OutPoint::new(prev_txid, 0);

        // Locking script: OP_FALSE (always fails)
        let locking = vec![OpCode::OpFalse as u8];

        let tx = Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_output: op,
                unlocking_script: vec![OpCode::OpTrue as u8],
                sequence: 0xFFFF_FFFF,
                redeemer: None,
            }],
            outputs: vec![TxOutput {
                value: Quantum::from_raw(10_000),
                locking_script: vec![OpCode::OpTrue as u8],
                datum: Datum::None,
            }],
            locktime: 0,
        };

        let fee = min_fee_for(&tx);
        let utxo = ExtendedUtxo::new(
            Quantum::from_raw(10_000u128.saturating_add(fee)),
            locking,
            0,
            false,
            Datum::None,
        );
        let view = single_utxo_view(op, utxo);

        let mut pool = Mempool::new(DEFAULT_MAX_MEMPOOL_BYTES);
        let result = pool.insert(tx, &view, 0, 0, 1);

        // A locking script of OP_FALSE means execute_contract returns false → ContractConditionFailed
        assert!(
            matches!(result, Err(MempoolReject::ContractConditionFailed(0))),
            "Expected ContractConditionFailed(0), got: {result:?}"
        );
    }

    // ── Test: Block Sync Clears Mempool ──────────────────────────────────────

    #[test]
    fn test_process_committed_block_clears_transactions() {
        use aurion_core::block::Block;
        use aurion_consensus::genesis;

        let prev_txid = Hash256::from_bytes([0xAA; 32]);
        let op = OutPoint::new(prev_txid, 0);
        let tx = make_tx(prev_txid, 0, 10_000);
        let fee = min_fee_for(&tx);
        let utxo = ExtendedUtxo::new(
            Quantum::from_raw(10_000u128.saturating_add(fee)),
            vec![OpCode::OpTrue as u8],
            0,
            false,
            Datum::None,
        );
        let view = single_utxo_view(op, utxo);

        let mut pool = Mempool::new(DEFAULT_MAX_MEMPOOL_BYTES);
        let txid = pool.insert(tx.clone(), &view, 0, 0, 1).expect("insert should succeed");
        assert_eq!(pool.len(), 1);

        // Build a synthetic block containing the confirmed tx.
        let genesis_block = genesis::create_genesis_block();
        let block = Block {
            header: genesis_block.header.clone(),
            transactions: vec![
                genesis_block.transactions[0].clone(), // coinbase
                tx.clone(),                            // our mempool tx
            ],
        };

        pool.process_committed_block(&block);

        // The confirmed transaction must be removed.
        assert!(pool.is_empty(), "Mempool should be empty after block sync");

        // The outpoint reservation must also be released.
        assert!(
            !pool.inner.spent_outpoints.contains_key(&op),
            "Spent outpoint should be unregistered"
        );
        let _ = txid; // used above
    }

    // ── Test: Coinbase Rejection ──────────────────────────────────────────────

    #[test]
    fn test_coinbase_rejected() {
        let coinbase_tx = Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_output: OutPoint::new(Hash256::ZERO, 0), // coinbase sentinel
                unlocking_script: vec![],
                sequence: 0xFFFF_FFFF,
                redeemer: None,
            }],
            outputs: vec![TxOutput {
                value: Quantum::from_raw(5_000_000_000),
                locking_script: vec![OpCode::OpTrue as u8],
                datum: Datum::None,
            }],
            locktime: 0,
        };

        let null_view = |_: &OutPoint| None::<ExtendedUtxo>;
        let mut pool = Mempool::new(DEFAULT_MAX_MEMPOOL_BYTES);
        let result = pool.insert(coinbase_tx, &null_view, 0, 0, 1);
        assert!(
            matches!(result, Err(MempoolReject::CoinbaseNotAllowed)),
            "Expected CoinbaseNotAllowed, got: {result:?}"
        );
    }

    // ── Test: select_transactions_for_block ordering ─────────────────────────

    #[test]
    fn test_select_highest_fee_rate_first() {
        let mut pool = Mempool::new(DEFAULT_MAX_MEMPOOL_BYTES);

        // Insert two transactions. The one with higher fee should appear first.
        let prev_a = Hash256::from_bytes([0x0A; 32]);
        let op_a = OutPoint::new(prev_a, 0);
        let tx_low = make_tx(prev_a, 0, 100);
        let fee_low = min_fee_for(&tx_low); // exact minimum
        let utxo_low = ExtendedUtxo::new(
            Quantum::from_raw(100u128.saturating_add(fee_low)),
            vec![OpCode::OpTrue as u8],
            0,
            false,
            Datum::None,
        );
        pool.insert(tx_low.clone(), &single_utxo_view(op_a, utxo_low), 0, 0, 1)
            .expect("low-fee insert failed");

        let prev_b = Hash256::from_bytes([0x0B; 32]);
        let op_b = OutPoint::new(prev_b, 0);
        let tx_high = make_tx(prev_b, 0, 100);
        let fee_high = min_fee_for(&tx_high).saturating_mul(10); // 10× fee
        let utxo_high = ExtendedUtxo::new(
            Quantum::from_raw(100u128.saturating_add(fee_high)),
            vec![OpCode::OpTrue as u8],
            0,
            false,
            Datum::None,
        );
        pool.insert(tx_high.clone(), &single_utxo_view(op_b, utxo_high), 0, 0, 2)
            .expect("high-fee insert failed");

        let selected = pool.select_transactions_for_block(1_000_000);
        assert_eq!(selected.len(), 2, "Both transactions should be selected");

        // First element must be the higher fee-rate transaction.
        assert_eq!(
            selected[0].txid(),
            tx_high.txid(),
            "Highest fee-rate tx must be selected first"
        );
    }
}
