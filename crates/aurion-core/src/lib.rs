#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

pub mod block;
pub mod merkle;
pub mod outpoint;
pub mod tx;

pub use block::{Block, BlockHeader};
pub use merkle::compute_merkle_root;
pub use outpoint::OutPoint;
pub use tx::{Transaction, TxInput, TxOutput};
