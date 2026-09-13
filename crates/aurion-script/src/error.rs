use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ScriptError {
    #[error("stack underflow: attempted to pop from an empty stack")]
    StackUnderflow,

    #[error("stack overflow: stack depth exceeded maximum limit ({0})")]
    StackOverflow(usize),

    #[error("element size limit exceeded: item size {size} > max {max}")]
    ElementTooLarge { size: usize, max: usize },

    #[error("opcode count limit exceeded: {0}")]
    MaxOpCountExceeded(usize),

    #[error("unknown or unsupported opcode: {0:#04x}")]
    InvalidOpCode(u8),

    #[error("OP_VERIFY failed: top stack element evaluated to false")]
    VerifyFailed,

    #[error("OP_EQUALVERIFY failed: elements are not equal")]
    EqualVerifyFailed,

    #[error("OP_RETURN executed: script explicitly marked unspendable")]
    OpReturnTriggered,

    #[error("invalid Ed25519 public key encoding")]
    InvalidPublicKey,

    #[error("invalid Ed25519 signature encoding")]
    InvalidSignature,

    #[error("script evaluation completed with empty stack")]
    EmptyStackOnTermination,

    #[error("script execution failed: top stack element evaluated to false")]
    ScriptFailed,

    #[error("arithmetic overflow during script math execution")]
    ArithmeticOverflow,

    #[error("unexpected end of script bytes during pushdata read")]
    UnexpectedEof,
}
