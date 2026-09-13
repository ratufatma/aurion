#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![allow(clippy::result_large_err)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod engine;
pub mod error;

pub use engine::StorageEngine;
pub use error::StorageError;
