use aurion_core::tx::Transaction;
use aurion_primitives::codec::CanonicalCodec;
use crate::error::EutxoError;

pub const MAX_TX_SIZE_BYTES: usize = 100_000;      // 100 KB
pub const MIN_FEE_PER_BYTE_AUR: u128 = 10;         // 10 kuanta per byte

pub fn verify_transaction_fee(
    tx: &Transaction,
    total_input_val: u128,
    total_output_val: u128,
) -> Result<u128, EutxoError> {
    let tx_bytes = tx.encode_canonical().len();

    if tx_bytes > MAX_TX_SIZE_BYTES {
        return Err(EutxoError::TransactionTooLarge {
            size: tx_bytes,
            max: MAX_TX_SIZE_BYTES,
        });
    }

    let required_fee = (tx_bytes as u128)
        .checked_mul(MIN_FEE_PER_BYTE_AUR)
        .ok_or(EutxoError::ArithmeticOverflow)?;

    let actual_fee = total_input_val
        .checked_sub(total_output_val)
        .ok_or(EutxoError::ArithmeticOverflow)?;

    if actual_fee < required_fee {
        return Err(EutxoError::InsufficientFee {
            actual: actual_fee,
            required: required_fee,
            fee_per_byte: MIN_FEE_PER_BYTE_AUR,
        });
    }

    Ok(actual_fee)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurion_core::outpoint::OutPoint;
    use aurion_core::tx::{TxInput, TxOutput};
    use aurion_primitives::hash::Hash256;
    use aurion_primitives::quantum::Quantum;

    #[test]
    #[allow(clippy::arithmetic_side_effects)]
    fn test_fee_calculation() {
        let tx = Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_output: OutPoint::new(Hash256::ZERO, 0),
                unlocking_script: vec![0u8; 10],
                sequence: 0,
            }],
            outputs: vec![TxOutput {
                value: Quantum::from_raw(10_000),
                locking_script: vec![0u8; 10],
            }],
            locktime: 0,
        };

        let tx_size = tx.encode_canonical().len();
        let expected_min_fee = (tx_size as u128) * MIN_FEE_PER_BYTE_AUR;

        // Input persis mencukupi fee
        let total_in = 10_000 + expected_min_fee;
        assert!(verify_transaction_fee(&tx, total_in, 10_000).is_ok());

        // Input kurang 1 kuanta
        let low_in = total_in - 1;
        assert!(matches!(
            verify_transaction_fee(&tx, low_in, 10_000),
            Err(EutxoError::InsufficientFee { .. })
        ));
    }
}
