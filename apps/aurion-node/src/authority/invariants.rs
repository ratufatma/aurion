use aurion_core::block::Block;
use aurion_core::outpoint::OutPoint;
use aurion_core::tx::TxOutput;
use aurion_primitives::quantum::Quantum;
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InvariantError {
    #[error("duplicate input spent in the same block: {0}")]
    DuplicateInputSpend(OutPoint),

    #[error("input not found in canonical UTXO set: {0}")]
    MissingUtxo(OutPoint),

    #[error("value conservation violation in tx {txid}: inputs {inputs} < outputs {outputs}")]
    ValueInflation {
        txid: String,
        inputs: String,
        outputs: String,
    },

    #[error("arithmetic overflow during invariant calculation")]
    ArithmeticOverflow,
}

/// Memverifikasi hukum konservasi nilai dan ketiadaan pembelanjaan ganda (double-spend)
/// sebelum mutasi diizinkan masuk ke tahap commit redb.
pub fn verify_block_invariants<F>(
    block: &Block,
    utxo_lookup: F,
) -> Result<Vec<OutPoint>, InvariantError>
where
    F: Fn(&OutPoint) -> Option<TxOutput>,
{
    let mut spent_inputs = Vec::new();
    let mut spent_set = HashSet::new();

    for tx in &block.transactions {
        if tx.is_coinbase() {
            continue;
        }

        let mut total_input_val = Quantum::ZERO;
        for input in &tx.inputs {
            let op = input.previous_output;
            if !spent_set.insert(op) {
                return Err(InvariantError::DuplicateInputSpend(op));
            }

            let utxo = utxo_lookup(&op).ok_or(InvariantError::MissingUtxo(op))?;
            total_input_val = total_input_val
                .checked_add(utxo.value)
                .map_err(|_| InvariantError::ArithmeticOverflow)?;

            spent_inputs.push(op);
        }

        let mut total_output_val = Quantum::ZERO;
        for output in &tx.outputs {
            total_output_val = total_output_val
                .checked_add(output.value)
                .map_err(|_| InvariantError::ArithmeticOverflow)?;
        }

        if total_input_val < total_output_val {
            return Err(InvariantError::ValueInflation {
                txid: tx.txid().to_string(),
                inputs: total_input_val.to_string(),
                outputs: total_output_val.to_string(),
            });
        }
    }

    Ok(spent_inputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurion_core::tx::{Transaction, TxInput};
    use aurion_primitives::hash::Hash256;
    use std::collections::HashMap;

    #[test]
    fn test_value_conservation_rejection() {
        let prev_op = OutPoint::new(Hash256::digest(b"funding"), 0);
        let mut utxos = HashMap::new();
        utxos.insert(prev_op, TxOutput {
            value: Quantum::from_raw(100),
            locking_script: vec![],
        });

        // Transaksi mencoba menghasilkan 150 kuanta dari 100 kuanta input
        let inflating_tx = Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_output: prev_op,
                unlocking_script: vec![],
                sequence: 0,
            }],
            outputs: vec![TxOutput {
                value: Quantum::from_raw(150),
                locking_script: vec![],
            }],
            locktime: 0,
        };

        let block = Block::new(
            aurion_core::block::BlockHeader {
                version: 1,
                prev_block_hash: Hash256::ZERO,
                merkle_root: Hash256::ZERO,
                timestamp: 0,
                bits: 0,
                nonce: 0,
                height: 1,
            },
            vec![inflating_tx],
        );

        let result = verify_block_invariants(&block, |op| utxos.get(op).cloned());
        assert!(matches!(result, Err(InvariantError::ValueInflation { .. })));
    }
}
