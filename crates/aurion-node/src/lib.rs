#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod authority;
pub mod lifecycle;

pub use authority::{AuthorityEngine, AuthorityError, InvariantError};
pub use lifecycle::{NodeFault, NodeState};
