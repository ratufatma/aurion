use std::fmt;
use aurion_primitives::codec::{CanonicalCodec, CodecError};
use aurion_primitives::hash::Hash256;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct OutPoint {
    pub txid: Hash256,
    pub vout: u32,
}

impl OutPoint {
    pub const fn new(txid: Hash256, vout: u32) -> Self {
        Self { txid, vout }
    }
}

impl CanonicalCodec for OutPoint {
    fn encode_canonical(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(36);
        buf.extend_from_slice(self.txid.as_bytes());
        buf.extend_from_slice(&self.vout.to_be_bytes());
        buf
    }

    fn decode_from_cursor(cursor: &mut &[u8]) -> Result<Self, CodecError> {
        let txid = Hash256::decode_from_cursor(cursor)?;
        if cursor.len() < 4 {
            return Err(CodecError::UnexpectedEof);
        }
        let (vout_bytes, rest) = cursor.split_at(4);
        *cursor = rest;
        let mut arr = [0u8; 4];
        arr.copy_from_slice(vout_bytes);
        let vout = u32::from_be_bytes(arr);

        Ok(Self { txid, vout })
    }
}

impl fmt::Display for OutPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.txid, self.vout)
    }
}

impl fmt::Debug for OutPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OutPoint({}:{})", self.txid, self.vout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outpoint_codec_roundtrip() {
        let op = OutPoint::new(Hash256::digest(b"tx-source"), 3);
        let encoded = op.encode_canonical();
        assert_eq!(encoded.len(), 36);
        let decoded = OutPoint::decode_canonical(&encoded).unwrap();
        assert_eq!(op, decoded);
    }
}
