use thiserror::Error;
use aurion_primitives::hash::Hash256;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConsensusError {
    #[error("proof of work target exceeded: hash {hash} > target {target}")]
    ProofOfWorkExceeded {
        hash: String,
        target: String,
    },
    #[error("invalid difficulty compact bits: {0:#010x}")]
    InvalidCompactBits(u32),
    #[error("empty block transactions")]
    EmptyBlock,
    #[error("first transaction in block must be coinbase")]
    MissingCoinbase,
    #[error("multiple coinbase transactions detected in block")]
    MultipleCoinbase,
    #[error("merkle root mismatch: expected {expected}, calculated {actual}")]
    MerkleRootMismatch {
        expected: Hash256,
        actual: Hash256,
    },
    #[error("coinbase subsidy exceeded: max allowed {allowed}, claimed {claimed}")]
    SubsidyExceeded {
        allowed: u128,
        claimed: u128,
    },
    #[error("block height mismatch: header height {header_height}, expected {expected_height}")]
    HeightMismatch {
        header_height: u64,
        expected_height: u64,
    },
}
