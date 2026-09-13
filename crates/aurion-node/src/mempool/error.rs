#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

use aurion_core::outpoint::OutPoint;
use aurion_primitives::hash::Hash256;
use aurion_primitives::quantum::Quantum;
use std::fmt;

/// Structured rejection taxonomy for mempool gate validation.
#[derive(Debug, PartialEq, Eq)]
pub enum MempoolReject {
    /// Transaction payload exceeds 100 KB limit.
    OversizedTransaction(usize),

    /// Transaction has empty inputs or empty outputs.
    EmptyTransaction,

    /// Coinbase transactions are not permitted in the mempool.
    CoinbaseNotAllowed,

    /// Input outpoint is already reserved by an in-mempool transaction.
    DoubleSpendConflict {
        outpoint: OutPoint,
        conflicting_txid: Hash256,
    },

    /// Input UTXO does not exist in the ledger view.
    MissingUtxo(OutPoint),

    /// Coinbase UTXO is being spent before 100-block maturity window.
    ImmatureCoinbase(String),

    /// Absolute locktime (height or MTP) condition is not satisfied.
    LocktimeReject(String),

    /// Relative BIP-68 sequence condition is not satisfied.
    SequenceReject(String),

    /// Total output value exceeds total input value (monetary creation).
    ValueConservationViolation,

    /// Fee is below the minimum fee-per-byte network policy.
    InsufficientFee(String),

    /// Computed fee is below the required minimum for this transaction.
    FeeTooLow { provided: Quantum, required: Quantum },

    /// Script VM returned an error during execution.
    ScriptExecutionFailed(String),

    /// SPEC-05 contract evaluation returned `false` for the given input index.
    ContractConditionFailed(usize),

    /// Mempool is at capacity and incoming tx does not beat the lowest fee-rate.
    MempoolFull,

    /// Integer overflow encountered during quota or fee calculation.
    ArithmeticOverflow,
}

impl fmt::Display for MempoolReject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OversizedTransaction(sz) => {
                write!(f, "transaction too large: {sz} bytes exceeds 100 KB limit")
            }
            Self::EmptyTransaction => write!(f, "transaction has empty inputs or outputs"),
            Self::CoinbaseNotAllowed => {
                write!(f, "coinbase transactions are not accepted by the mempool")
            }
            Self::DoubleSpendConflict { outpoint, conflicting_txid } => {
                write!(
                    f,
                    "double-spend conflict: outpoint {outpoint} already reserved by tx {conflicting_txid}"
                )
            }
            Self::MissingUtxo(op) => write!(f, "referenced UTXO not found in ledger: {op}"),
            Self::ImmatureCoinbase(msg) => write!(f, "immature coinbase: {msg}"),
            Self::LocktimeReject(msg) => write!(f, "locktime violation: {msg}"),
            Self::SequenceReject(msg) => write!(f, "relative sequence violation: {msg}"),
            Self::ValueConservationViolation => {
                write!(f, "value conservation violated: outputs exceed inputs")
            }
            Self::InsufficientFee(msg) => write!(f, "insufficient fee: {msg}"),
            Self::FeeTooLow { provided, required } => {
                write!(f, "fee too low: provided {provided} < required {required}")
            }
            Self::ScriptExecutionFailed(msg) => write!(f, "script execution failed: {msg}"),
            Self::ContractConditionFailed(idx) => {
                write!(f, "SPEC-05 contract condition failed for input index {idx}")
            }
            Self::MempoolFull => {
                write!(f, "mempool full: incoming transaction does not beat lowest fee-rate")
            }
            Self::ArithmeticOverflow => {
                write!(f, "arithmetic overflow during mempool calculation")
            }
        }
    }
}

impl std::error::Error for MempoolReject {}
