#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

use thiserror::Error;

/// Structured error taxonomy for the Aurion P2P network layer.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum NetworkError {
    /// Payload exceeds the 4 MB anti-DoS hard cap.
    #[error("payload too large: {0} bytes exceeds 4 MB protocol limit")]
    PayloadTooLarge(usize),

    /// Blake3 digest in the 52-byte header does not match the received payload.
    #[error("corrupted frame: Blake3 checksum mismatch")]
    CorruptedChecksum,

    /// The first 4 bytes of the frame are not `AUR\x01`.
    #[error("invalid magic bytes: {0:?}")]
    InvalidMagic([u8; 4]),

    /// Raw slice is shorter than the 52-byte minimum canonical header.
    #[error("incomplete frame: fewer than 52 bytes received")]
    IncompleteFrame,

    /// Zenoh transport or session-level failure.
    #[error("transport error: {0}")]
    TransportError(String),

    /// Canonical codec serialization/deserialization failure.
    #[error("serialization error: {0}")]
    SerializationError(String),
}
