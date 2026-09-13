#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

use aurion_core::tx::Transaction;
use aurion_primitives::codec::CanonicalCodec;
use aurion_primitives::hash::Hash256;
use aurion_primitives::quantum::Quantum;
use std::sync::Arc;

/// A single entry in the mempool staging area.
///
/// Holds a reference-counted transaction alongside its precomputed
/// canonical metadata: txid, wire-size in bytes, network fee, and
/// the Unix timestamp at which it was admitted.
#[derive(Clone, Debug)]
pub struct MempoolEntry {
    /// Shared ownership of the transaction.
    pub tx: Arc<Transaction>,

    /// Blake3 canonical transaction identifier.
    pub txid: Hash256,

    /// Serialized byte length via `CanonicalCodec::encode_canonical`.
    pub size_bytes: usize,

    /// Network fee (total_inputs - total_outputs) in Quantum units.
    pub fee: Quantum,

    /// Monotonic admission timestamp (Unix seconds, provided by caller).
    pub added_timestamp: u64,
}

impl MempoolEntry {
    /// Construct a new `MempoolEntry`, computing `txid` and `size_bytes`
    /// deterministically from the canonical encoding of `tx`.
    pub fn new(tx: Transaction, fee: Quantum, added_timestamp: u64) -> Self {
        let txid = tx.txid();
        let size_bytes = tx.encode_canonical().len();
        Self {
            tx: Arc::new(tx),
            txid,
            size_bytes,
            fee,
            added_timestamp,
        }
    }
}
