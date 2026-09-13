//! Taksonomi kesalahan transport IPC/RPC: framing, serialisasi, dan transport.
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("I/O transport failure: {0}")]
    Io(#[from] std::io::Error),

    #[error("IPC frame exceeds 4 MB anti-DoS limit: {0} bytes")]
    PayloadTooLarge(usize),

    #[error("canonical wire serialization failure: {0}")]
    Serialization(String),

    #[error("unable to reach node endpoint: {0}")]
    ConnectionFailed(String),

    #[error("node rejected request: {0}")]
    NodeRejected(String),

    #[error("response did not match the expected protocol variant")]
    ProtocolMismatch,
}