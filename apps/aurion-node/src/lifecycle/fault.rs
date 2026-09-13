use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NodeFault {
    #[error("atomic storage commit failure: {0}")]
    StorageCommitFailure(String),

    #[error("canonical state corruption: {0}")]
    StateCorruption(String),

    #[error("mathematical invariant violation: {0}")]
    InvariantViolation(String),

    #[error("consensus rules violation: {0}")]
    ConsensusViolation(String),

    #[error("serialization boundary trap: {0}")]
    SerializationTrap(String),
}
