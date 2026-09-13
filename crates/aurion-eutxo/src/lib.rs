#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

pub mod error;
pub mod fee;
pub mod locktime;
pub mod maturity;
pub mod state;
pub mod view;

pub use error::EutxoError;
pub use fee::{verify_transaction_fee, MAX_TX_SIZE_BYTES, MIN_FEE_PER_BYTE_AUR};
pub use locktime::{verify_relative_sequence, verify_transaction_locktime, LOCKTIME_THRESHOLD_UNIX};
pub use maturity::{verify_coinbase_maturity, COINBASE_MATURITY};
pub use state::ExtendedUtxo;
pub use view::UtxoView;
