#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

use aurion_primitives::hash::Hash256;
use crate::error::NetworkError;

/// The canonical Aurion wire protocol magic identifier: `AUR\x01`.
pub const MAGIC_BYTES: [u8; 4] = *b"AUR\x01";

/// Size of the canonical 52-byte frame header (magic + command + length + checksum).
pub const HEADER_SIZE: usize = 52;

/// Anti-DoS hard cap: maximum payload bytes that may be allocated (4 MB).
pub const MAX_PAYLOAD_SIZE: usize = 4 * 1024 * 1024;

/// Encode a command name and raw payload into the 52-byte canonical Aurion wire frame:
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
/// Returns [`NetworkError::PayloadTooLarge`] if `payload.len() > MAX_PAYLOAD_SIZE`.
pub fn pack_frame(command: &str, payload: &[u8]) -> Result<Vec<u8>, NetworkError> {
    if payload.len() > MAX_PAYLOAD_SIZE {
        return Err(NetworkError::PayloadTooLarge(payload.len()));
    }

    let checksum = Hash256::digest(payload);
    let payload_len = payload.len() as u32;

    let mut frame = Vec::with_capacity(payload.len().saturating_add(HEADER_SIZE));
    frame.extend_from_slice(&MAGIC_BYTES);

    // 12-byte null-padded ASCII command
    let mut cmd_bytes = [0u8; 12];
    let copy_len = command.len().min(12);
    cmd_bytes[..copy_len].copy_from_slice(&command.as_bytes()[..copy_len]);
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
/// - [`NetworkError::IncompleteFrame`] if `raw.len() < 52`.
/// - [`NetworkError::InvalidMagic`] if magic bytes differ from `AUR\x01`.
/// - [`NetworkError::PayloadTooLarge`] if declared length exceeds 4 MB.
/// - [`NetworkError::CorruptedChecksum`] if Blake3 digest does not match.
pub fn unpack_frame(raw: &[u8]) -> Result<(&str, &[u8]), NetworkError> {
    if raw.len() < HEADER_SIZE {
        return Err(NetworkError::IncompleteFrame);
    }

    let (header, payload_and_rest) = raw.split_at(HEADER_SIZE);

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
        .map_err(|_| NetworkError::SerializationError("non-UTF8 command bytes".into()))?;

    // ── Payload Length ─────────────────────────────────────────────────────
    let mut len_bytes = [0u8; 4];
    len_bytes.copy_from_slice(&header[16..20]);
    let payload_len = u32::from_be_bytes(len_bytes) as usize;

    if payload_len > MAX_PAYLOAD_SIZE {
        return Err(NetworkError::PayloadTooLarge(payload_len));
    }

    if payload_and_rest.len() < payload_len {
        return Err(NetworkError::IncompleteFrame);
    }

    let payload = &payload_and_rest[..payload_len];

    // ── Blake3 Checksum ────────────────────────────────────────────────────
    let mut expected = [0u8; 32];
    expected.copy_from_slice(&header[20..52]);
    let actual = Hash256::digest(payload);

    if actual.as_bytes() != &expected {
        return Err(NetworkError::CorruptedChecksum);
    }

    Ok((cmd_str, payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_unpack_roundtrip() {
        let payload = b"aurion-sovereign-digital-asset-protocol";
        let frame = pack_frame("block", payload).unwrap();
        assert_eq!(frame.len(), HEADER_SIZE + payload.len());

        let (cmd, decoded_payload) = unpack_frame(&frame).unwrap();
        assert_eq!(cmd, "block");
        assert_eq!(decoded_payload, payload);
    }

    #[test]
    fn test_unpack_frame_rejects_corrupted_checksum() {
        let mut frame = pack_frame("tx", b"some-tx-bytes").unwrap();
        // Corrupt a payload byte.
        let corrupt_idx = HEADER_SIZE;
        frame[corrupt_idx] ^= 0x55;

        assert_eq!(
            unpack_frame(&frame).unwrap_err(),
            NetworkError::CorruptedChecksum
        );
    }

    #[test]
    fn test_unpack_frame_rejects_bad_magic() {
        let mut frame = pack_frame("tx", b"data").unwrap();
        frame[0] = 0x00;
        assert!(matches!(
            unpack_frame(&frame).unwrap_err(),
            NetworkError::InvalidMagic(_)
        ));
    }

    #[test]
    fn test_unpack_frame_rejects_incomplete() {
        assert_eq!(
            unpack_frame(&[0u8; 10]).unwrap_err(),
            NetworkError::IncompleteFrame
        );
    }

    #[test]
    fn test_pack_frame_rejects_oversized_payload() {
        // 4 MB + 1 byte should be rejected immediately.
        let oversized = vec![0u8; MAX_PAYLOAD_SIZE.saturating_add(1)];
        assert!(matches!(
            pack_frame("block", &oversized).unwrap_err(),
            NetworkError::PayloadTooLarge(_)
        ));
    }
}
