use aurion_core::outpoint::OutPoint;
use crate::error::EutxoError;
use crate::state::ExtendedUtxo;

pub const COINBASE_MATURITY: u64 = 100;

pub fn verify_coinbase_maturity(
    outpoint: &OutPoint,
    utxo: &ExtendedUtxo,
    current_height: u64,
) -> Result<(), EutxoError> {
    if !utxo.is_coinbase {
        return Ok(());
    }

    let spendable_height = utxo
        .creation_height
        .checked_add(COINBASE_MATURITY)
        .ok_or(EutxoError::ArithmeticOverflow)?;

    if current_height < spendable_height {
        return Err(EutxoError::ImmatureCoinbaseSpend {
            outpoint: *outpoint,
            creation_height: utxo.creation_height,
            spendable_height,
            current_height,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurion_core::tx::TxOutput;
    use aurion_primitives::hash::Hash256;
    use aurion_primitives::quantum::Quantum;

    #[test]
    fn test_coinbase_maturity_threshold() {
        let op = OutPoint::new(Hash256::ZERO, 0);
        let coinbase_utxo = ExtendedUtxo::new(
            TxOutput {
                value: Quantum::from_raw(100),
                locking_script: vec![],
            },
            10,
            true,
        );

        // Harus ditolak sebelum tinggi 110 (10 + 100)
        assert!(verify_coinbase_maturity(&op, &coinbase_utxo, 109).is_err());
        assert!(verify_coinbase_maturity(&op, &coinbase_utxo, 110).is_ok());
        assert!(verify_coinbase_maturity(&op, &coinbase_utxo, 200).is_ok());

        // Regular non-coinbase bebas dibelanjakan kapan pun
        let regular_utxo = ExtendedUtxo::new(coinbase_utxo.output.clone(), 10, false);
        assert!(verify_coinbase_maturity(&op, &regular_utxo, 11).is_ok());
    }
}
