#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

pub mod authority;
pub mod lifecycle;

pub use authority::{AuthorityEngine, AuthorityError, InvariantError};
pub use lifecycle::{NodeFault, NodeState};
