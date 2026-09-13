use aurion_core::tx::Transaction;
use aurion_eutxo::state::ExtendedUtxo;
use aurion_primitives::quantum::Quantum;

#[derive(Clone, Copy, Debug)]
pub struct ScriptContext<'a> {
    pub tx: &'a Transaction,
    pub current_input_index: usize,
    pub current_spent_utxo: &'a ExtendedUtxo,
    pub fee: Quantum,
    pub validation_height: u64,
}
