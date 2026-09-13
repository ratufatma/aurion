#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod engine;
pub mod error;
pub mod opcode;
pub mod stack;

pub use engine::{ScriptEngine, MAX_OPS_PER_SCRIPT};
pub use error::ScriptError;
pub use opcode::OpCode;
pub use stack::{Stack, MAX_ELEMENT_SIZE, MAX_STACK_DEPTH};
