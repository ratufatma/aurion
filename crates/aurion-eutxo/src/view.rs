use aurion_core::outpoint::OutPoint;
use crate::state::ExtendedUtxo;

/// Trait pembacaan murni untuk ledger eUTXO tanpa dependensi I/O disk.
pub trait UtxoView {
    fn get_utxo(&self, outpoint: &OutPoint) -> Option<ExtendedUtxo>;
}

impl<F> UtxoView for F
where
    F: Fn(&OutPoint) -> Option<ExtendedUtxo>,
{
    fn get_utxo(&self, outpoint: &OutPoint) -> Option<ExtendedUtxo> {
        self(outpoint)
    }
}
