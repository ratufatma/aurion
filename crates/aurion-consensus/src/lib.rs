#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

pub mod difficulty;
pub mod error;
pub mod genesis;
pub mod subsidy;
pub mod verification;

pub use difficulty::{check_pow, compact_to_target, MAX_TARGET_BITS};
pub use error::ConsensusError;
pub use genesis::{create_genesis_block, GENESIS_TIMESTAMP};
pub use subsidy::{calculate_block_subsidy, HALVING_INTERVAL, INITIAL_SUBSIDY};
pub use verification::verify_block_structure;
