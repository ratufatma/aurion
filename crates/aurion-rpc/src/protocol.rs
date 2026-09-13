//! Skema permintaan (`RpcRequest`) dan respons (`RpcResponse`) IPC/RPC yang
//! dienkode deterministik via `CanonicalCodec` dengan tag nonce satu byte.
use super::template::{BlockTemplate, SubmitResult};
use aurion_core::{Block, Transaction};
use aurion_primitives::{CanonicalCodec, CodecError, Hash256};

const TAG_GET_BLOCK_TEMPLATE: u8 = 0x00;
const TAG_SUBMIT_BLOCK: u8 = 0x01;
const TAG_SEND_RAW_TRANSACTION: u8 = 0x02;
const TAG_GET_TIP_STATUS: u8 = 0x03;

const TAG_BLOCK_TEMPLATE: u8 = 0x00;
const TAG_SUBMIT_BLOCK_RESULT: u8 = 0x01;
const TAG_TRANSACTION_ACCEPTED: u8 = 0x02;
const TAG_TIP_STATUS: u8 = 0x03;
const TAG_ERROR: u8 = 0x04;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RpcRequest {
    /// Penambang meminta template blok baru
    GetBlockTemplate {
        payout_address: Vec<u8>,
    },
    /// Penambang mengirimkan blok yang berhasil ditambang
    SubmitBlock {
        block: Block,
    },
    /// Dompet menyiarkan transaksi baru ke mempool
    SendRawTransaction {
        tx: Transaction,
    },
    /// Cek status rantai dan tip blok terkini
    GetTipStatus,
}

impl CanonicalCodec for RpcRequest {
    fn encode_canonical(&self) -> Vec<u8> {
        match self {
            RpcRequest::GetBlockTemplate { payout_address } => {
                let mut buf = Vec::with_capacity(payout_address.len().saturating_add(5));
                buf.push(TAG_GET_BLOCK_TEMPLATE);
                buf.extend(encode_vec_u8(payout_address));
                buf
            }
            RpcRequest::SubmitBlock { block } => {
                let mut buf = Vec::with_capacity(block.encode_canonical().len().saturating_add(1));
                buf.push(TAG_SUBMIT_BLOCK);
                buf.extend(block.encode_canonical());
                buf
            }
            RpcRequest::SendRawTransaction { tx } => {
                let mut buf = Vec::with_capacity(tx.encode_canonical().len().saturating_add(1));
                buf.push(TAG_SEND_RAW_TRANSACTION);
                buf.extend(tx.encode_canonical());
                buf
            }
            RpcRequest::GetTipStatus => vec![TAG_GET_TIP_STATUS],
        }
    }

    fn decode_from_cursor(cursor: &mut &[u8]) -> Result<Self, CodecError> {
        if cursor.is_empty() {
            return Err(CodecError::UnexpectedEof);
        }
        let tag = cursor[0];
        *cursor = &cursor[1..];
        match tag {
            TAG_GET_BLOCK_TEMPLATE => {
                let payout_address = decode_vec_u8(cursor)?;
                Ok(RpcRequest::GetBlockTemplate { payout_address })
            }
            TAG_SUBMIT_BLOCK => {
                let block = Block::decode_from_cursor(cursor)?;
                Ok(RpcRequest::SubmitBlock { block })
            }
            TAG_SEND_RAW_TRANSACTION => {
                let tx = Transaction::decode_from_cursor(cursor)?;
                Ok(RpcRequest::SendRawTransaction { tx })
            }
            TAG_GET_TIP_STATUS => Ok(RpcRequest::GetTipStatus),
            other => Err(CodecError::InvalidData(format!(
                "invalid RpcRequest tag: {other:#04x}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RpcResponse {
    BlockTemplate(BlockTemplate),
    SubmitBlock(SubmitResult),
    TransactionAccepted(Hash256),
    TipStatus {
        height: u64,
        best_block_hash: Hash256,
        mempool_size: usize,
    },
    Error(String),
}

impl CanonicalCodec for RpcResponse {
    fn encode_canonical(&self) -> Vec<u8> {
        match self {
            RpcResponse::BlockTemplate(tmpl) => {
                let mut buf = Vec::with_capacity(tmpl.encode_canonical().len().saturating_add(1));
                buf.push(TAG_BLOCK_TEMPLATE);
                buf.extend(tmpl.encode_canonical());
                buf
            }
            RpcResponse::SubmitBlock(result) => {
                let mut buf = Vec::with_capacity(result.encode_canonical().len().saturating_add(1));
                buf.push(TAG_SUBMIT_BLOCK_RESULT);
                buf.extend(result.encode_canonical());
                buf
            }
            RpcResponse::TransactionAccepted(txid) => {
                let mut buf = Vec::with_capacity(1 + 32);
                buf.push(TAG_TRANSACTION_ACCEPTED);
                buf.extend_from_slice(txid.as_bytes());
                buf
            }
            RpcResponse::TipStatus {
                height,
                best_block_hash,
                mempool_size,
            } => {
                let mut buf = Vec::with_capacity(1 + 8 + 32 + 8);
                buf.push(TAG_TIP_STATUS);
                buf.extend_from_slice(&height.to_be_bytes());
                buf.extend_from_slice(best_block_hash.as_bytes());
                buf.extend_from_slice(&(*mempool_size as u64).to_be_bytes());
                buf
            }
            RpcResponse::Error(err) => {
                let mut buf = Vec::with_capacity(err.len().saturating_add(5));
                buf.push(TAG_ERROR);
                buf.extend(encode_vec_u8(err.as_bytes()));
                buf
            }
        }
    }

    fn decode_from_cursor(cursor: &mut &[u8]) -> Result<Self, CodecError> {
        if cursor.is_empty() {
            return Err(CodecError::UnexpectedEof);
        }
        let tag = cursor[0];
        *cursor = &cursor[1..];
        match tag {
            TAG_BLOCK_TEMPLATE => {
                let tmpl = BlockTemplate::decode_from_cursor(cursor)?;
                Ok(RpcResponse::BlockTemplate(tmpl))
            }
            TAG_SUBMIT_BLOCK_RESULT => {
                let result = SubmitResult::decode_from_cursor(cursor)?;
                Ok(RpcResponse::SubmitBlock(result))
            }
            TAG_TRANSACTION_ACCEPTED => {
                let txid = Hash256::decode_from_cursor(cursor)?;
                Ok(RpcResponse::TransactionAccepted(txid))
            }
            TAG_TIP_STATUS => {
                let height = decode_u64(cursor)?;
                let best_block_hash = Hash256::decode_from_cursor(cursor)?;
                let mempool_size = decode_u64(cursor)? as usize;
                Ok(RpcResponse::TipStatus {
                    height,
                    best_block_hash,
                    mempool_size,
                })
            }
            TAG_ERROR => {
                let reason = decode_string(cursor)?;
                Ok(RpcResponse::Error(reason))
            }
            other => Err(CodecError::InvalidData(format!(
                "invalid RpcResponse tag: {other:#04x}"
            ))),
        }
    }
}

fn encode_vec_u8(bytes: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(bytes.len().saturating_add(4));
    buf.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    buf.extend_from_slice(bytes);
    buf
}

fn decode_vec_u8(cursor: &mut &[u8]) -> Result<Vec<u8>, CodecError> {
    let len = decode_u32(cursor)? as usize;
    if cursor.len() < len {
        return Err(CodecError::UnexpectedEof);
    }
    let (bytes, rest) = cursor.split_at(len);
    *cursor = rest;
    Ok(bytes.to_vec())
}

fn decode_string(cursor: &mut &[u8]) -> Result<String, CodecError> {
    let len = decode_u32(cursor)? as usize;
    if cursor.len() < len {
        return Err(CodecError::UnexpectedEof);
    }
    let (bytes, rest) = cursor.split_at(len);
    *cursor = rest;
    String::from_utf8(bytes.to_vec())
        .map_err(|err| CodecError::InvalidData(format!("invalid UTF-8: {err}")))
}

fn decode_u32(cursor: &mut &[u8]) -> Result<u32, CodecError> {
    if cursor.len() < 4 {
        return Err(CodecError::UnexpectedEof);
    }
    let (bytes, rest) = cursor.split_at(4);
    *cursor = rest;
    let mut arr = [0u8; 4];
    arr.copy_from_slice(bytes);
    Ok(u32::from_be_bytes(arr))
}

fn decode_u64(cursor: &mut &[u8]) -> Result<u64, CodecError> {
    if cursor.len() < 8 {
        return Err(CodecError::UnexpectedEof);
    }
    let (bytes, rest) = cursor.split_at(8);
    *cursor = rest;
    let mut arr = [0u8; 8];
    arr.copy_from_slice(bytes);
    Ok(u64::from_be_bytes(arr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurion_core::tx::{Datum, TxInput, TxOutput};
    use aurion_core::OutPoint;
    use aurion_primitives::Quantum;

    fn sample_tx() -> Transaction {
        Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_output: OutPoint::new(Hash256::ZERO, 0),
                unlocking_script: vec![0x10],
                sequence: 0,
                redeemer: None,
            }],
            outputs: vec![TxOutput {
                value: Quantum::from_raw(50_000_000),
                locking_script: vec![0x20],
                datum: Datum::None,
            }],
            locktime: 0,
        }
    }

    fn sample_block() -> Block {
        let header = aurion_core::BlockHeader {
            version: 1,
            prev_block_hash: Hash256::ZERO,
            merkle_root: Hash256::ZERO,
            timestamp: 1_773_446_400,
            bits: 0x1f00ffff,
            nonce: 42,
            height: 10,
        };
        Block::new(header, vec![sample_tx()])
    }

    fn sample_template() -> BlockTemplate {
        BlockTemplate {
            height: 10,
            previous_block_hash: Hash256::ZERO,
            target: Hash256::digest(b"target"),
            timestamp: 1_773_446_400,
            coinbase_subsidy: Quantum::from_raw(99_000_000),
            total_fee: Quantum::from_raw(5_000),
            transactions: vec![sample_tx()],
        }
    }

    #[test]
    fn test_rpc_request_roundtrip() {
        let requests = vec![
            RpcRequest::GetBlockTemplate {
                payout_address: vec![0xaa; 29],
            },
            RpcRequest::SubmitBlock {
                block: sample_block(),
            },
            RpcRequest::SendRawTransaction { tx: sample_tx() },
            RpcRequest::GetTipStatus,
        ];

        for request in requests {
            let encoded = request.encode_canonical();
            let decoded = RpcRequest::decode_canonical(&encoded).unwrap();
            assert_eq!(request, decoded);
        }
    }

    #[test]
    fn test_rpc_response_roundtrip() {
        let responses = vec![
            RpcResponse::BlockTemplate(sample_template()),
            RpcResponse::SubmitBlock(SubmitResult::Accepted {
                block_hash: Hash256::digest(b"block"),
                height: 10,
            }),
            RpcResponse::SubmitBlock(SubmitResult::Rejected {
                reason: "stale template".into(),
            }),
            RpcResponse::TransactionAccepted(Hash256::digest(b"txid")),
            RpcResponse::TipStatus {
                height: 10,
                best_block_hash: Hash256::digest(b"tip"),
                mempool_size: 1_048_576,
            },
            RpcResponse::Error("internal failure".into()),
        ];

        for response in responses {
            let encoded = response.encode_canonical();
            let decoded = RpcResponse::decode_canonical(&encoded).unwrap();
            assert_eq!(response, decoded);
        }
    }

    #[test]
    fn test_protocol_rejects_unknown_tag() {
        let unknown = vec![0xff];
        assert!(RpcResponse::decode_canonical(&unknown).is_err());
        assert!(RpcRequest::decode_canonical(&unknown).is_err());
    }
}