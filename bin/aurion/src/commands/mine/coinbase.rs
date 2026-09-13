//! Perakitan transaksi coinbase deterministik untuk penambangan Aurion.
//!
//! Transaksi coinbase menciptakan imbalan blok dari subsisdi + biaya dan
//! membawa payload BIP-34 (tinggi blok + extranonce) pada skrip pembuka input
//! pertama sehingga setiap kandidat blok memiliki txid coinbase yang unik.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use aurion_core::tx::{Datum, Transaction, TxInput, TxOutput};
use aurion_core::OutPoint;
use aurion_primitives::{Hash256, Quantum};

/// Menyusun transaksi coinbase untuk tinggi blok `height` dengan imbalan total
/// `subsidy + fees` yang dibayarkan ke `payout_script`.
///
/// Input 0 merujuk pada outpoint virtual (txid nol, index `0xFFFF_FFFF`) dan
/// membawa payload BIP-34: `height.to_be_bytes()` diikuti `extra_nonce.to_be_bytes()`.
pub fn build_coinbase_transaction(
    height: u64,
    payout_script: Vec<u8>,
    subsidy: Quantum,
    fees: Quantum,
    extra_nonce: u64,
) -> Result<Transaction, String> {
    let total_reward = subsidy
        .checked_add(fees)
        .map_err(|_| "coinbase reward overflow".to_string())?;

    let input = TxInput {
        previous_output: OutPoint::new(Hash256::ZERO, 0xFFFF_FFFF),
        unlocking_script: bip34_push(height, extra_nonce),
        sequence: 0xFFFF_FFFF,
        redeemer: None,
    };

    let output = TxOutput {
        value: total_reward,
        locking_script: payout_script,
        datum: Datum::None,
    };

    Ok(Transaction {
        version: 1,
        inputs: vec![input],
        outputs: vec![output],
        locktime: 0,
    })
}

/// Serialisasi payload BIP-34 + extranonce untuk skrip pembuka coinbase.
fn bip34_push(height: u64, extra_nonce: u64) -> Vec<u8> {
    let mut script = Vec::with_capacity(16);
    script.extend_from_slice(&height.to_be_bytes());
    script.extend_from_slice(&extra_nonce.to_be_bytes());
    script
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coinbase_constructs_with_subsidy_and_fees() {
        let height = 1_001u64;
        let payout = vec![0x51];
        let subsidy = Quantum::from_raw(9_900_000_000);
        let fees = Quantum::from_raw(2_500);
        let extra_nonce = 42u64;

        let tx =
            build_coinbase_transaction(height, payout.clone(), subsidy, fees, extra_nonce).unwrap();

        assert!(tx.is_coinbase());
        assert_eq!(tx.version, 1);
        assert_eq!(tx.locktime, 0);
        assert_eq!(tx.inputs.len(), 1);
        assert_eq!(tx.outputs.len(), 1);

        let input = &tx.inputs[0];
        assert_eq!(input.previous_output, OutPoint::new(Hash256::ZERO, 0xFFFF_FFFF));
        assert_eq!(input.sequence, 0xFFFF_FFFF);
        assert!(input.redeemer.is_none());

        let mut expected_script = Vec::with_capacity(16);
        expected_script.extend_from_slice(&height.to_be_bytes());
        expected_script.extend_from_slice(&extra_nonce.to_be_bytes());
        assert_eq!(input.unlocking_script, expected_script);

        let output = &tx.outputs[0];
        assert_eq!(output.value, subsidy.checked_add(fees).unwrap());
        assert_eq!(output.locking_script, payout);
        assert_eq!(output.datum, Datum::None);
    }

    #[test]
    fn test_coinbase_bip34_height_and_extranonce_encoding() {
        let height = 300_000u64;
        let extra_nonce = 7u64;

        let tx = build_coinbase_transaction(
            height,
            vec![0x51],
            Quantum::ZERO,
            Quantum::ZERO,
            extra_nonce,
        )
        .unwrap();

        let mut expected = Vec::with_capacity(16);
        expected.extend_from_slice(&height.to_be_bytes());
        expected.extend_from_slice(&extra_nonce.to_be_bytes());
        assert_eq!(tx.inputs[0].unlocking_script, expected);
        assert_eq!(tx.inputs[0].previous_output.txid, Hash256::ZERO);
        assert_eq!(tx.inputs[0].previous_output.vout, 0xFFFF_FFFF);
        assert_eq!(tx.outputs[0].value, Quantum::ZERO);
    }

    #[test]
    fn test_coinbase_reward_overflow_protection() {
        let result = build_coinbase_transaction(
            1,
            vec![0x51],
            Quantum::MAX,
            Quantum::from_raw(1),
            1,
        );
        assert!(result.is_err());
    }
}