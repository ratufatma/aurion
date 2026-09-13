#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize, PartialEq, Eq)]
pub enum RpcError {
    #[error("connection refused or node unavailable: {0}")]
    ConnectionFailed(String),

    #[error("method not found: {0}")]
    MethodNotFound(String),

    #[error("invalid parameters: {0}")]
    InvalidParams(String),

    #[error("internal server error: {0}")]
    InternalError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeStatusResponse {
    pub version: String,
    pub height: u64,
    pub best_block_hash: String,
    pub peer_count: usize,
    pub is_syncing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetBlockRequest {
    pub hash_or_height: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubmitTxRequest {
    pub raw_tx_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubmitTxResponse {
    pub txid: String,
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MiningTemplateResponse {
    pub prev_block_hash: String,
    pub height: u64,
    pub bits: u32,
    pub subsidy: u128,
}
