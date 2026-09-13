#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod authority;
pub mod lifecycle;
pub mod mempool;

pub use authority::{AuthorityEngine, AuthorityError, InvariantError};
pub use lifecycle::{NodeFault, NodeState};
pub use mempool::{Mempool, DEFAULT_MAX_MEMPOOL_BYTES};
