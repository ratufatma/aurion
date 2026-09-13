use aurion_core::outpoint::OutPoint;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EutxoError {
    #[error("immature coinbase spend: outpoint {outpoint} created at height {creation_height}, spendable at {spendable_height}, current {current_height}")]
    ImmatureCoinbaseSpend {
        outpoint: OutPoint,
        creation_height: u64,
        spendable_height: u64,
        current_height: u64,
    },

    #[error("absolute locktime violation: current {current_val} < required {required_locktime}")]
    LocktimeNotMet {
        required_locktime: u64,
        current_val: u64,
    },

    #[error("relative sequence locktime violation for input {input_index}: required {required_delay}, passed {passed_blocks}")]
    RelativeLocktimeNotMet {
        input_index: usize,
        required_delay: u64,
        passed_blocks: u64,
    },

    #[error("insufficient fee: provided {actual} Aur < required {required} Aur ({fee_per_byte} Aur/byte)")]
    InsufficientFee {
        actual: u128,
        required: u128,
        fee_per_byte: u128,
    },

    #[error("transaction size exceeds limit: {size} > max {max}")]
    TransactionTooLarge { size: usize, max: usize },

    #[error("arithmetic overflow during eUTXO evaluation")]
    ArithmeticOverflow,

    #[error("referenced UTXO not found: {0}")]
    UtxoNotFound(OutPoint),

    #[error("duplicate input spent within same transaction: {0}")]
    DuplicateInput(OutPoint),
}
