use std::fmt;
use crate::codec::{CanonicalCodec, CodecError};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Hash256(pub [u8; 32]);

impl Hash256 {
    pub const ZERO: Self = Self([0u8; 32]);

    #[inline]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[inline]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn digest(data: &[u8]) -> Self {
        Self(*blake3::hash(data).as_bytes())
    }
}

impl fmt::Display for Hash256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

impl fmt::Debug for Hash256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash256({})", self)
    }
}

impl CanonicalCodec for Hash256 {
    fn encode_canonical(&self) -> Vec<u8> {
        self.0.to_vec()
    }

    fn decode_from_cursor(cursor: &mut &[u8]) -> Result<Self, CodecError> {
        if cursor.len() < 32 {
            return Err(CodecError::UnexpectedEof);
        }
        let (hash_bytes, rest) = cursor.split_at(32);
        *cursor = rest;
        let mut arr = [0u8; 32];
        arr.copy_from_slice(hash_bytes);
        Ok(Self(arr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_blake3_digest() {
        let h1 = Hash256::digest(b"aurion-sovereign-truth");
        let h2 = Hash256::digest(b"aurion-sovereign-truth");
        let h3 = Hash256::digest(b"aurion-sovereign-false");

        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_hash_codec() {
        let h = Hash256::digest(b"canonical-hash");
        let encoded = h.encode_canonical();
        assert_eq!(encoded.len(), 32);
        assert_eq!(Hash256::decode_canonical(&encoded).unwrap(), h);
    }
}
