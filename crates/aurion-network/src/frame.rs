#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

use crate::error::NetworkError;

/// The canonical Aurion wire protocol magic identifier: `AUR\x01`.
pub const MAGIC_BYTES: [u8; 4] = *b"AUR\x01";

/// Size of the fixed canonical frame header (magic + command + length + checksum).
pub const HEADER_SIZE: usize = 52;

/// Size of the null-padded ASCII command field inside the header.
pub const COMMAND_SIZE: usize = 12;

/// Anti-DoS hard cap: maximum payload bytes that may be allocated (4 MB).
pub const MAX_PAYLOAD_SIZE: usize = 4 * 1024 * 1024;

/// Encode a command name and raw payload into the canonical 52-byte Aurion wire frame:
///
/// ```text
/// [0..4]   Magic bytes  (AUR\x01)
/// [4..16]  Command      (12-byte ASCII null-padded)
/// [16..20] Length       (u32 big-endian)
/// [20..52] Checksum     (Blake3 32-byte digest of payload)
/// [52..]   Payload      (raw bytes)
/// ```
///
/// # Errors
/// - Returns [`NetworkError::CommandTooLong`] if `command` exceeds 12 ASCII bytes.
/// - Returns [`NetworkError::PayloadTooLarge`] if `payload.len() > MAX_PAYLOAD_SIZE`.
pub fn pack_frame(command: &str, payload: &[u8]) -> Result<Vec<u8>, NetworkError> {
    if command.len() > COMMAND_SIZE {
        return Err(NetworkError::CommandTooLong(command.to_owned()));
    }

    if payload.len() > MAX_PAYLOAD_SIZE {
        return Err(NetworkError::PayloadTooLarge {
            declared: payload.len(),
            max: MAX_PAYLOAD_SIZE,
        });
    }

    let checksum = blake3::hash(payload);
    let payload_len = payload.len() as u32;

    let mut frame = Vec::with_capacity(payload.len().saturating_add(HEADER_SIZE));
    frame.extend_from_slice(&MAGIC_BYTES);

    // 12-byte null-padded ASCII command field
    let mut cmd_bytes = [0u8; COMMAND_SIZE];
    cmd_bytes[..command.len()].copy_from_slice(command.as_bytes());
    frame.extend_from_slice(&cmd_bytes);

    frame.extend_from_slice(&payload_len.to_be_bytes());
    frame.extend_from_slice(checksum.as_bytes());
    frame.extend_from_slice(payload);

    Ok(frame)
}

/// Decode a raw byte slice into `(command_str, payload_slice)` by validating the
/// 52-byte canonical wire frame header, magic bytes, and Blake3 checksum.
///
/// # Errors
/// - [`NetworkError::IncompleteHeader`] if `raw.len() < 52`.
/// - [`NetworkError::InvalidMagic`] if magic bytes differ from `AUR\x01`.
/// - [`NetworkError::InvalidCommandEncoding`] if the command field is not valid UTF-8.
/// - [`NetworkError::PayloadTooLarge`] if the declared length exceeds 4 MB.
/// - [`NetworkError::TruncatedPayload`] if the declared length exceeds the bytes present.
/// - [`NetworkError::ChecksumMismatch`] if the Blake3 digest does not match.
pub fn unpack_frame(raw: &[u8]) -> Result<(&str, &[u8]), NetworkError> {
    if raw.len() < HEADER_SIZE {
        return Err(NetworkError::IncompleteHeader(raw.len()));
    }

    let (header, body) = raw.split_at(HEADER_SIZE);

    // ── Magic ──────────────────────────────────────────────────────────────
    let mut magic = [0u8; 4];
    magic.copy_from_slice(&header[0..4]);
    if magic != MAGIC_BYTES {
        return Err(NetworkError::InvalidMagic(magic));
    }

    // ── Command (null-terminated, up to 12 bytes) ──────────────────────────
    let cmd_raw = &header[4..16];
    let end = cmd_raw.iter().position(|&b| b == 0).unwrap_or(cmd_raw.len());
    let cmd_str = std::str::from_utf8(&cmd_raw[..end])
        .map_err(|_| NetworkError::InvalidCommandEncoding)?;

    // ── Payload Length ─────────────────────────────────────────────────────
    let mut len_bytes = [0u8; 4];
    len_bytes.copy_from_slice(&header[16..20]);
    let payload_len = u32::from_be_bytes(len_bytes) as usize;

    if payload_len > MAX_PAYLOAD_SIZE {
        return Err(NetworkError::PayloadTooLarge {
            declared: payload_len,
            max: MAX_PAYLOAD_SIZE,
        });
    }

    if body.len() < payload_len {
        return Err(NetworkError::TruncatedPayload {
            declared: payload_len,
            actual: body.len(),
        });
    }

    let payload = &body[..payload_len];

    // ── Blake3 Checksum ────────────────────────────────────────────────────
    let mut expected = [0u8; 32];
    expected.copy_from_slice(&header[20..52]);
    let actual = blake3::hash(payload);

    if actual.as_bytes() != &expected {
        return Err(NetworkError::ChecksumMismatch);
    }

    Ok((cmd_str, payload))
}