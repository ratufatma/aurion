use aurion_core::tx::Transaction;
use crate::error::EutxoError;
use crate::state::ExtendedUtxo;

pub const LOCKTIME_THRESHOLD_UNIX: u64 = 500_000_000;
pub const SEQUENCE_DISABLE_FLAG: u64 = 0x8000_0000;

pub fn verify_transaction_locktime(
    tx: &Transaction,
    block_height: u64,
    median_time_past: u64,
) -> Result<(), EutxoError> {
    // Jika semua input memiliki sequence final (0xFFFF_FFFF), locktime dinonaktifkan
    let has_active_lock = tx.inputs.iter().any(|i| i.sequence != 0xFFFF_FFFF);
    if !has_active_lock || tx.locktime == 0 {
        return Ok(());
    }

    if tx.locktime < LOCKTIME_THRESHOLD_UNIX {
        if block_height < tx.locktime {
            return Err(EutxoError::LocktimeNotMet {
                required_locktime: tx.locktime,
                current_val: block_height,
            });
        }
    } else if median_time_past < tx.locktime {
        return Err(EutxoError::LocktimeNotMet {
            required_locktime: tx.locktime,
            current_val: median_time_past,
        });
    }

    Ok(())
}

pub fn verify_relative_sequence(
    input_sequence: u64,
    utxo: &ExtendedUtxo,
    current_height: u64,
    input_index: usize,
) -> Result<(), EutxoError> {
    // Jika bit-31 aktif, relative sequence locktime tidak diberlakukan
    if (input_sequence & SEQUENCE_DISABLE_FLAG) != 0 {
        return Ok(());
    }

    let required_delay = input_sequence & 0x0000_FFFF;
    let passed_blocks = current_height.saturating_sub(utxo.creation_height);

    if passed_blocks < required_delay {
        return Err(EutxoError::RelativeLocktimeNotMet {
            input_index,
            required_delay,
            passed_blocks,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurion_core::outpoint::OutPoint;
    use aurion_core::tx::TxInput;
    use aurion_primitives::hash::Hash256;

    #[test]
    fn test_locktime_by_height() {
        let tx = Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_output: OutPoint::new(Hash256::ZERO, 0),
                unlocking_script: vec![],
                sequence: 0,
            }],
            outputs: vec![],
            locktime: 500,
        };

        assert!(verify_transaction_locktime(&tx, 499, 0).is_err());
        assert!(verify_transaction_locktime(&tx, 500, 0).is_ok());
    }

    #[test]
    fn test_locktime_by_mtp() {
        let tx = Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_output: OutPoint::new(Hash256::ZERO, 0),
                unlocking_script: vec![],
                sequence: 0,
            }],
            outputs: vec![],
            locktime: 1_700_000_000,
        };

        assert!(verify_transaction_locktime(&tx, 0, 1_699_999_999).is_err());
        assert!(verify_transaction_locktime(&tx, 0, 1_700_000_000).is_ok());
    }
}
