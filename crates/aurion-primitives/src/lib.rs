#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod codec;
pub mod hash;
pub mod quantum;

pub use codec::{CanonicalCodec, CodecError};
pub use hash::Hash256;
pub use quantum::{Quantum, QuantumError};
