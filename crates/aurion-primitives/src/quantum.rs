use std::fmt;
use thiserror::Error;
use crate::codec::{CanonicalCodec, CodecError};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QuantumError {
    #[error("arithmetic overflow detected")]
    Overflow,
    #[error("arithmetic underflow detected")]
    Underflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Quantum(u128);

impl Quantum {
    pub const ZERO: Self = Self(0);
    pub const MAX: Self = Self(u128::MAX);

    #[inline]
    pub const fn new(raw: u128) -> Self {
        Self(raw)
    }

    #[inline]
    pub const fn from_raw(raw: u128) -> Self {
        Self(raw)
    }

    #[inline]
    pub const fn raw(self) -> u128 {
        self.0
    }

    #[inline]
    pub const fn as_u128(self) -> u128 {
        self.0
    }

    pub fn checked_add(self, rhs: Self) -> Result<Self, QuantumError> {
        self.0
            .checked_add(rhs.0)
            .map(Self)
            .ok_or(QuantumError::Overflow)
    }

    pub fn checked_sub(self, rhs: Self) -> Result<Self, QuantumError> {
        self.0
            .checked_sub(rhs.0)
            .map(Self)
            .ok_or(QuantumError::Underflow)
    }
}

impl fmt::Display for Quantum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl CanonicalCodec for Quantum {
    fn encode_canonical(&self) -> Vec<u8> {
        self.0.to_be_bytes().to_vec()
    }

    fn decode_from_cursor(cursor: &mut &[u8]) -> Result<Self, CodecError> {
        if cursor.len() < 16 {
            return Err(CodecError::UnexpectedEof);
        }
        let (int_bytes, rest) = cursor.split_at(16);
        *cursor = rest;
        let mut arr = [0u8; 16];
        arr.copy_from_slice(int_bytes);
        Ok(Self(u128::from_be_bytes(arr)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantum_arithmetic_bounds() {
        let q1 = Quantum::from_raw(100);
        let q2 = Quantum::from_raw(50);

        assert_eq!(q1.checked_add(q2).unwrap(), Quantum::from_raw(150));
        assert_eq!(q1.checked_sub(q2).unwrap(), Quantum::from_raw(50));
        assert_eq!(q2.checked_sub(q1).unwrap_err(), QuantumError::Underflow);
        assert_eq!(Quantum::MAX.checked_add(Quantum::from_raw(1)).unwrap_err(), QuantumError::Overflow);
    }

    #[test]
    fn test_quantum_codec() {
        let q = Quantum::from_raw(100_000_000);
        let encoded = q.encode_canonical();
        assert_eq!(encoded.len(), 16);
        assert_eq!(Quantum::decode_canonical(&encoded).unwrap(), q);
    }
}
