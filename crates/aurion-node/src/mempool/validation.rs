#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

use std::collections::HashMap;

use aurion_core::outpoint::OutPoint;
use aurion_core::tx::Transaction;
use aurion_eutxo::fee::{verify_transaction_fee, MAX_TX_SIZE_BYTES, MIN_FEE_PER_BYTE_AUR};
use aurion_eutxo::locktime::{verify_relative_sequence, verify_transaction_locktime};
use aurion_eutxo::maturity::verify_coinbase_maturity;
use aurion_eutxo::view::UtxoView;
use aurion_primitives::codec::CanonicalCodec;
use aurion_primitives::hash::Hash256;
use aurion_primitives::quantum::Quantum;
use aurion_script::{ScriptContext, ScriptEngine};

use crate::mempool::error::MempoolReject;

/// 6-Gate Validation Pipeline for the in-memory transaction mempool.
///
/// Enforces structural, semantic (eUTXO), and SPEC-05 smart contract invariants
/// before a transaction is admitted to the mempool staging area.
///
/// Returns the computed network fee (`total_inputs - total_outputs`) on success.
///
/// # Gates
///
/// 1. **Size & Structure**: Reject oversized (>100 KB) or structurally empty transactions.
/// 2. **Coinbase Guard**: Reject coinbase transactions unconditionally.
/// 3. **Mempool Double-Spend**: Reject inputs already reserved by in-mempool transactions.
/// 4. **eUTXO Semantics**: Locktime, coinbase maturity, BIP-68 relative sequences.
/// 5. **SPEC-05 Smart Contracts & Value Conservation**: Full $(D, R, C)$ contract evaluation.
/// 6. **Fee Policy**: Verify fee ≥ `MIN_FEE_PER_BYTE_AUR × size_bytes`.
pub fn validate_for_mempool<V: UtxoView>(
    tx: &Transaction,
    utxo_view: &V,
    mempool_spent: &HashMap<OutPoint, Hash256>,
    current_tip_height: u64,
    median_time_past: u64,
) -> Result<Quantum, MempoolReject> {
    // ─── Gate 1: Size & Structure ────────────────────────────────────────────
    let tx_size = tx.encode_canonical().len();
    if tx_size > MAX_TX_SIZE_BYTES {
        return Err(MempoolReject::OversizedTransaction(tx_size));
    }
    if tx.inputs.is_empty() || tx.outputs.is_empty() {
        return Err(MempoolReject::EmptyTransaction);
    }

    // ─── Gate 2: Coinbase Guard ───────────────────────────────────────────────
    if tx.is_coinbase() {
        return Err(MempoolReject::CoinbaseNotAllowed);
    }

    // ─── Gate 3: Mempool Double-Spend ─────────────────────────────────────────
    for input in &tx.inputs {
        let op = input.previous_output;
        if let Some(&conflicting_txid) = mempool_spent.get(&op) {
            return Err(MempoolReject::DoubleSpendConflict {
                outpoint: op,
                conflicting_txid,
            });
        }
    }

    // ─── Gate 4: eUTXO Semantics ──────────────────────────────────────────────
    let next_block_height = current_tip_height
        .checked_add(1)
        .ok_or(MempoolReject::ArithmeticOverflow)?;

    // Absolute locktime check
    verify_transaction_locktime(tx, next_block_height, median_time_past)
        .map_err(|e| MempoolReject::LocktimeReject(e.to_string()))?;

    // Resolve UTXOs and verify per-input semantic rules
    let mut total_input_val = Quantum::ZERO;
    let mut spent_utxos = Vec::with_capacity(tx.inputs.len());

    for (idx, input) in tx.inputs.iter().enumerate() {
        let op = input.previous_output;
        let utxo = utxo_view
            .get_utxo(&op)
            .ok_or(MempoolReject::MissingUtxo(op))?;

        // Coinbase maturity check
        verify_coinbase_maturity(&op, &utxo, next_block_height)
            .map_err(|e| MempoolReject::ImmatureCoinbase(e.to_string()))?;

        // BIP-68 relative sequence check
        verify_relative_sequence(input.sequence, &utxo, next_block_height, idx)
            .map_err(|e| MempoolReject::SequenceReject(e.to_string()))?;

        total_input_val = total_input_val
            .checked_add(utxo.value)
            .map_err(|_| MempoolReject::ArithmeticOverflow)?;

        spent_utxos.push((idx, input, utxo));
    }

    // ─── Gate 5: Value Conservation & SPEC-05 Smart Contracts ────────────────
    let mut total_output_val = Quantum::ZERO;
    for output in &tx.outputs {
        total_output_val = total_output_val
            .checked_add(output.value)
            .map_err(|_| MempoolReject::ArithmeticOverflow)?;
    }

    if total_input_val < total_output_val {
        return Err(MempoolReject::ValueConservationViolation);
    }

    let tx_fee = total_input_val
        .checked_sub(total_output_val)
        .map_err(|_| MempoolReject::ArithmeticOverflow)?;

    // SPEC-05: Execute (D, R, C) contract for every input
    for (input_idx, input, utxo) in &spent_utxos {
        let ctx = ScriptContext {
            tx,
            current_input_index: *input_idx,
            current_spent_utxo: utxo,
            fee: tx_fee,
            validation_height: next_block_height,
        };

        let mut engine = ScriptEngine::new(Some(&ctx));
        let valid = engine
            .execute_contract(
                &utxo.locking_script,
                &input.unlocking_script,
                &utxo.datum,
                input.redeemer.as_deref(),
            )
            .map_err(|e| MempoolReject::ScriptExecutionFailed(e.to_string()))?;

        if !valid {
            return Err(MempoolReject::ContractConditionFailed(*input_idx));
        }
    }

    // ─── Gate 6: Fee Policy ───────────────────────────────────────────────────
    let required_fee = Quantum::from_raw(
        (tx_size as u128)
            .checked_mul(MIN_FEE_PER_BYTE_AUR)
            .ok_or(MempoolReject::ArithmeticOverflow)?,
    );

    // Delegate to aurion-eutxo fee verifier (reusing its logic for consistency)
    verify_transaction_fee(tx, total_input_val.as_u128(), total_output_val.as_u128())
        .map_err(|e| MempoolReject::InsufficientFee(e.to_string()))?;

    if tx_fee < required_fee {
        return Err(MempoolReject::FeeTooLow {
            provided: tx_fee,
            required: required_fee,
        });
    }

    Ok(tx_fee)
}
