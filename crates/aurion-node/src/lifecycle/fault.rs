use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NodeFault {
    #[error("atomic storage commit failure: {0}")]
    StorageCommitFailure(String),

    #[error("critical ledger invariant violation: {0}")]
    InvariantViolation(String),

    #[error("consensus corruption: {0}")]
    ConsensusViolation(String),

    #[error("state corrupted: {0}")]
    StateCorruption(String),

    #[error("hardware safety violation: {0}")]
    HardwareViolation(String),
}
