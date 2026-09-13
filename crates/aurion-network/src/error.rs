#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

use thiserror::Error;

/// Structured error taxonomy for the Aurion P2P network layer.
///
/// Mirrors the canonical 52-byte wire protocol states: header validation,
/// command encoding, payload bounds, and Blake3 checksum verification.
/// All variants are `Clone + PartialEq + Eq` so they can be asserted
/// deterministically in tests and propagated across task boundaries.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NetworkError {
    /// The raw frame is shorter than the fixed 52-byte canonical header.
    #[error("incomplete header: received {0} bytes, need at least 52")]
    IncompleteHeader(usize),

    /// The first 4 bytes of the frame are not `AUR\x01`.
    #[error("invalid magic bytes: {0:?}")]
    InvalidMagic([u8; 4]),

    /// The command field is longer than the 12-byte ASCII budget.
    #[error("command too long: '{0}' exceeds the 12-byte ASCII command field")]
    CommandTooLong(String),

    /// The command field contains non-UTF-8 bytes.
    #[error("invalid command encoding: command field is not valid UTF-8")]
    InvalidCommandEncoding,

    /// Declared payload length exceeds the 4 MB anti-DoS hard cap.
    #[error("payload too large: declared {declared} bytes exceeds maximum allowed {max} bytes")]
    PayloadTooLarge { declared: usize, max: usize },

    /// Declared payload length exceeds the bytes actually present on the wire.
    #[error("truncated payload: declared {declared} bytes but only {actual} bytes present")]
    TruncatedPayload { declared: usize, actual: usize },

    /// Blake3 digest in the 52-byte header does not match the received payload.
    #[error("checksum mismatch: payload corrupt or tampered with")]
    ChecksumMismatch,

    /// Canonical codec serialization/deserialization failure.
    #[error("canonical codec error: {0}")]
    CodecError(String),

    /// Zenoh transport or session-level failure.
    #[error("transport error: {0}")]
    TransportError(String),

    /// Arithmetic overflow during frame construction or bounds computation.
    #[error("arithmetic overflow in network encoding")]
    ArithmeticOverflow,
}