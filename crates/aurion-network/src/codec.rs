use aurion_primitives::codec::CodecError;
use aurion_primitives::hash::Hash256;

use crate::error::NetworkError;
use crate::message::NetworkMessage;

pub const NETWORK_MAGIC: [u8; 4] = [0x41, 0x55, 0x52, 0x01]; // AUR v1
pub const MAX_FRAME_PAYLOAD_SIZE: usize = 4 * 1024 * 1024; // 4 MB

pub struct NetworkCodec;

impl NetworkCodec {
    /// Format frame biner kanonikal (52 bytes header + payload):
    /// [0..4]   : Magic bytes (0x41555201)
    /// [4..16]  : Command ASCII (12 bytes null-padded)
    /// [16..20] : Payload length (u32 BE)
    /// [20..52] : Payload checksum (Blake3 32-byte digest)
    /// [52..]   : Raw Payload
    pub fn encode_frame(message: &NetworkMessage) -> Vec<u8> {
        let cmd = message.command_name();
        let payload = message.encode_payload();
        let payload_len = payload.len() as u32;
        let checksum = Hash256::digest(&payload);

        let mut frame = Vec::with_capacity(52 + payload.len());
        frame.extend_from_slice(&NETWORK_MAGIC);

        let mut cmd_bytes = [0u8; 12];
        let copy_len = cmd.len().min(12);
        cmd_bytes[..copy_len].copy_from_slice(&cmd.as_bytes()[..copy_len]);
        frame.extend_from_slice(&cmd_bytes);

        frame.extend_from_slice(&payload_len.to_be_bytes());
        frame.extend_from_slice(checksum.as_bytes());
        frame.extend_from_slice(&payload);
        frame
    }

    pub fn decode_frame(cursor: &[u8]) -> Result<NetworkMessage, NetworkError> {
        if cursor.len() < 52 {
            return Err(NetworkError::UnexpectedEof);
        }

        let (header, payload_and_rest) = cursor.split_at(52);

        // 1. Validasi Magic Bytes
        let mut magic = [0u8; 4];
        magic.copy_from_slice(&header[0..4]);
        if magic != NETWORK_MAGIC {
            return Err(NetworkError::InvalidMagic {
                expected: NETWORK_MAGIC,
                actual: magic,
            });
        }

        // 2. Parse Nama Command
        let cmd_raw = &header[4..16];
        let end = cmd_raw.iter().position(|&b| b == 0).unwrap_or(cmd_raw.len());
        let cmd_str = std::str::from_utf8(&cmd_raw[..end])
            .map_err(|_| NetworkError::InvalidCommand)?;

        // 3. Validasi Batas Ukuran Payload
        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&header[16..20]);
        let payload_len = u32::from_be_bytes(len_bytes) as usize;

        if payload_len > MAX_FRAME_PAYLOAD_SIZE {
            return Err(NetworkError::PayloadTooLarge {
                size: payload_len,
                max: MAX_FRAME_PAYLOAD_SIZE,
            });
        }

        if payload_and_rest.len() < payload_len {
            return Err(NetworkError::UnexpectedEof);
        }

        let (payload, remaining) = payload_and_rest.split_at(payload_len);
        if !remaining.is_empty() {
            return Err(NetworkError::TrailingPayloadBytes);
        }

        // 4. Verifikasi Blake3 Checksum
        let mut expected_checksum = [0u8; 32];
        expected_checksum.copy_from_slice(&header[20..52]);
        let actual_checksum = Hash256::digest(payload);

        if actual_checksum.as_bytes() != &expected_checksum {
            return Err(NetworkError::ChecksumMismatch);
        }

        // 5. Decode Isi Payload
        NetworkMessage::decode_payload(cmd_str, payload).map_err(|err| match err {
            CodecError::UnexpectedEof => NetworkError::UnexpectedEof,
            CodecError::TrailingBytes => NetworkError::TrailingPayloadBytes,
            _ => NetworkError::InvalidCommand,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_codec_ping_pong_roundtrip() {
        let msg = NetworkMessage::Ping(0x0102030405060708);
        let frame = NetworkCodec::encode_frame(&msg);
        assert_eq!(frame.len(), 52 + 8);

        let decoded = NetworkCodec::decode_frame(&frame).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_network_codec_version_roundtrip() {
        let msg = NetworkMessage::Version {
            version: 1,
            height: 100,
            timestamp: 1773446400,
        };
        let frame = NetworkCodec::encode_frame(&msg);
        let decoded = NetworkCodec::decode_frame(&frame).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_network_codec_rejects_corrupted_checksum() {
        let msg = NetworkMessage::Ping(42);
        let mut frame = NetworkCodec::encode_frame(&msg);

        // Ubah byte terakhir payload
        let last = frame.len() - 1;
        frame[last] ^= 0xff;

        let err = NetworkCodec::decode_frame(&frame).unwrap_err();
        assert_eq!(err, NetworkError::ChecksumMismatch);
    }

    #[test]
    fn test_network_codec_rejects_invalid_magic() {
        let msg = NetworkMessage::Verack;
        let mut frame = NetworkCodec::encode_frame(&msg);
        frame[0] = 0x00;

        let err = NetworkCodec::decode_frame(&frame).unwrap_err();
        assert!(matches!(err, NetworkError::InvalidMagic { .. }));
    }
}
