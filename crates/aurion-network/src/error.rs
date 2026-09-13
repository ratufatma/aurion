use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NetworkError {
    #[error("invalid network magic bytes: expected {expected:?}, got {actual:?}")]
    InvalidMagic { expected: [u8; 4], actual: [u8; 4] },

    #[error("payload size exceeds protocol limit: size {size} > max {max}")]
    PayloadTooLarge { size: usize, max: usize },

    #[error("network message checksum mismatch")]
    ChecksumMismatch,

    #[error("unknown or invalid network command string")]
    InvalidCommand,

    #[error("unexpected end of frame stream")]
    UnexpectedEof,

    #[error("unconsumed trailing bytes in network message payload")]
    TrailingPayloadBytes,
}
