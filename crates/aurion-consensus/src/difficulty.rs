use aurion_primitives::hash::Hash256;
use crate::error::ConsensusError;

pub const MAX_TARGET_BITS: u32 = 0x1f00ffff;

pub fn compact_to_target(bits: u32) -> Result<[u8; 32], ConsensusError> {
    let exponent = (bits >> 24) as usize;
    let mantissa = bits & 0x007f_ffff;

    if exponent > 32 || (exponent < 3 && mantissa != 0) {
        return Err(ConsensusError::InvalidCompactBits(bits));
    }

    let mut target = [0u8; 32];
    if exponent == 0 {
        return Ok(target);
    }

    let m_bytes = mantissa.to_be_bytes();
    let shift_index = match 32usize.checked_sub(exponent) {
        Some(idx) => idx,
        None => return Err(ConsensusError::InvalidCompactBits(bits)),
    };

    if shift_index.checked_add(3).map_or(true, |end| end > 32) {
        return Err(ConsensusError::InvalidCompactBits(bits));
    }

    target[shift_index] = m_bytes[1];
    target[shift_index.saturating_add(1)] = m_bytes[2];
    target[shift_index.saturating_add(2)] = m_bytes[3];

    Ok(target)
}

pub fn check_pow(hash: &Hash256, bits: u32) -> Result<(), ConsensusError> {
    let target = compact_to_target(bits)?;

    if hash.as_bytes() > &target {
        let mut target_hex = String::with_capacity(64);
        for b in &target {
            use std::fmt::Write;
            let _ = write!(&mut target_hex, "{:02x}", b);
        }
        return Err(ConsensusError::ProofOfWorkExceeded {
            hash: hash.to_string(),
            target: target_hex,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pow_verification() {
        let valid_hash = Hash256::from_bytes([0u8; 32]);
        assert!(check_pow(&valid_hash, MAX_TARGET_BITS).is_ok());

        let invalid_hash = Hash256::from_bytes([0xff; 32]);
        assert!(check_pow(&invalid_hash, MAX_TARGET_BITS).is_err());
    }
}
