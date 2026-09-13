#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

use aurion_primitives::hash::Hash256;
use aurion_primitives::quantum::Quantum;
use std::cmp::Ordering;

/// Zero-float fee-rate comparator for ordering mempool transactions.
///
/// Compares fee density as the ratio `fee / size_bytes` using integer
/// wide cross-multiplication to avoid any floating-point arithmetic:
///
/// `fee_a / size_a  vs  fee_b / size_b`
/// `  ≡  fee_a * size_b  vs  fee_b * size_a`
///
/// Both products are computed as `u256`-equivalent (hi, lo) pairs via
/// pure bitwise arithmetic — no `u256` crate dependency required.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeRateIndex {
    /// Network fee for this transaction in Quantum units.
    pub fee: Quantum,

    /// Canonical serialised byte length of the transaction.
    pub size_bytes: usize,

    /// Blake3 txid used as a deterministic tie-breaker.
    pub txid: Hash256,
}

/// Compute `a * b` where both are `u128`, returning `(hi: u128, lo: u128)`
/// representing a 256-bit unsigned product in big-endian pair notation.
///
/// Uses only `u64`-width intermediate multiplications, which are always
/// exact in standard Rust arithmetic (no overflow, no float).
fn wide_mul_u128(a: u128, b: u128) -> (u128, u128) {
    // Split operands into 64-bit halves.
    let a_lo = a & u128::from(u64::MAX);
    let a_hi = a >> 64;
    let b_lo = b & u128::from(u64::MAX);
    let b_hi = b >> 64;

    // Four 64×64 partial products — each fits in u128 without overflow.
    let ll = a_lo.wrapping_mul(b_lo);
    let lh = a_lo.wrapping_mul(b_hi);
    let hl = a_hi.wrapping_mul(b_lo);
    let hh = a_hi.wrapping_mul(b_hi);

    // Combine partial products into (hi128, lo128).
    let lo = ll.wrapping_add(lh.wrapping_shl(64)).wrapping_add(hl.wrapping_shl(64));
    let carry_lh = lh >> 64;
    let carry_hl = hl >> 64;
    let carry_lo_overflow =
        (ll >> 64).wrapping_add(lh & u128::from(u64::MAX)).wrapping_add(hl & u128::from(u64::MAX)) >> 64;
    let hi = hh
        .wrapping_add(carry_lh)
        .wrapping_add(carry_hl)
        .wrapping_add(carry_lo_overflow);

    (hi, lo)
}

impl Ord for FeeRateIndex {
    fn cmp(&self, other: &Self) -> Ordering {
        // Cross-multiply: fee_self * size_other  vs  fee_other * size_self
        let lhs = wide_mul_u128(self.fee.as_u128(), other.size_bytes as u128);
        let rhs = wide_mul_u128(other.fee.as_u128(), self.size_bytes as u128);

        match lhs.cmp(&rhs) {
            Ordering::Equal => {
                // Deterministic tie-breaker: lexicographic txid comparison.
                self.txid.as_bytes().cmp(other.txid.as_bytes())
            }
            other_ord => other_ord,
        }
    }
}

impl PartialOrd for FeeRateIndex {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FeeRateIndex {
    /// Construct a new `FeeRateIndex` from fee, canonical byte size, and txid.
    pub fn new(fee: Quantum, size_bytes: usize, txid: Hash256) -> Self {
        Self { fee, size_bytes, txid }
    }

    /// Returns `true` if `self` has a strictly higher fee-rate than `other`.
    pub fn beats(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Greater
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idx(fee: u128, size: usize, seed: u8) -> FeeRateIndex {
        FeeRateIndex::new(
            Quantum::from_raw(fee),
            size,
            Hash256::from_bytes([seed; 32]),
        )
    }

    #[test]
    fn test_fee_rate_equal_size_higher_fee_ranks_higher() {
        // 200 / 100  >  100 / 100
        let high = idx(200, 100, 0x01);
        let low = idx(100, 100, 0x02);
        assert!(high > low);
        assert!(high.beats(&low));
    }

    #[test]
    fn test_fee_rate_different_sizes_cross_multiply() {
        // 30 / 300  ==  10 / 100  (both = 0.1 sat/byte conceptually)
        let a = idx(30, 300, 0x01);
        let b = idx(10, 100, 0x02);
        // Equal fee-rate — tie broken by txid
        // [0x01; 32] > [0x00; 32] but [0x02; 32] > [0x01; 32]
        assert!(b > a);

        // 20 / 100  >  10 / 100
        let high = idx(20, 100, 0x01);
        let low = idx(10, 100, 0x02);
        assert!(high > low);
    }

    #[test]
    fn test_fee_rate_txid_tiebreaker() {
        // Identical fee-rate (fee==size for both)
        let a = idx(100, 100, 0x01);
        let b = idx(100, 100, 0x02);
        // Txid [0x02;32] > [0x01;32]
        assert!(b > a);
        assert_eq!(a, a.clone());
    }

    #[test]
    fn test_wide_mul_u128_large_values() {
        // Verify no panic or wrong result for near-max values
        let max = u128::MAX;
        let (hi, lo) = wide_mul_u128(max, 1);
        // max * 1 = max, so hi=0, lo=max
        assert_eq!(hi, 0);
        assert_eq!(lo, max);
    }
}
