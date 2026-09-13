#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod context;
pub mod engine;
pub mod error;
pub mod opcode;
pub mod stack;

pub use context::ScriptContext;
pub use engine::{ScriptEngine, MAX_OPS_PER_SCRIPT};
pub use error::ScriptError;
pub use opcode::OpCode;
pub use stack::{Stack, MAX_ELEMENT_SIZE, MAX_STACK_DEPTH};

