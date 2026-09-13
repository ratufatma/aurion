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
pub use subsidy::{
    calculate_block_subsidy, CREATOR_ALLOCATION_AUR, DEV_ALLOCATION_AUR, HALVING_INTERVAL,
    INITIAL_SUBSIDY, INITIAL_SUBSIDY_AUR, INITIAL_SUBSIDY_QUANTA, MAX_HALVINGS,
    MAX_TOTAL_SUPPLY_AUR, MAX_TOTAL_SUPPLY_QUANTA, QUANTA_PER_AUR, SUBSIDY_HALVING_INTERVAL,
    TOTAL_GENESIS_PREMINE_QUANTA,
};
pub use verification::verify_block_structure;
