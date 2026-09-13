#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

pub mod engine;
pub mod error;

pub use engine::StorageEngine;
pub use error::StorageError;
