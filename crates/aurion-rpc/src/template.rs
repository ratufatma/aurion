//! Struktur data domain mining: `BlockTemplate` (subsidi, fee, target, transaksi
//! terpilih) dan `SubmitResult` (Accepted / Rejected).
use aurion_core::Transaction;
use aurion_primitives::{CanonicalCodec, CodecError, Hash256, Quantum};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockTemplate {
    pub height: u64,
    pub previous_block_hash: Hash256,
    pub target: Hash256,
    pub timestamp: u64,
    pub coinbase_subsidy: Quantum,
    pub total_fee: Quantum,
    pub transactions: Vec<Transaction>,
}

impl CanonicalCodec for BlockTemplate {
    fn encode_canonical(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8 + 32 + 32 + 8 + 16 + 16 + 4);
        buf.extend_from_slice(&self.height.to_be_bytes());
        buf.extend_from_slice(self.previous_block_hash.as_bytes());
        buf.extend_from_slice(self.target.as_bytes());
        buf.extend_from_slice(&self.timestamp.to_be_bytes());
        buf.extend(self.coinbase_subsidy.encode_canonical());
        buf.extend(self.total_fee.encode_canonical());
        buf.extend_from_slice(&(self.transactions.len() as u32).to_be_bytes());
        for tx in &self.transactions {
            buf.extend(tx.encode_canonical());
        }
        buf
    }

    fn decode_from_cursor(cursor: &mut &[u8]) -> Result<Self, CodecError> {
        let height = decode_u64(cursor)?;
        let previous_block_hash = Hash256::decode_from_cursor(cursor)?;
        let target = Hash256::decode_from_cursor(cursor)?;
        let timestamp = decode_u64(cursor)?;
        let coinbase_subsidy = Quantum::decode_from_cursor(cursor)?;
        let total_fee = Quantum::decode_from_cursor(cursor)?;

        let tx_count = decode_u32(cursor)? as usize;
        let mut transactions = Vec::with_capacity(tx_count);
        for _ in 0..tx_count {
            transactions.push(Transaction::decode_from_cursor(cursor)?);
        }

        Ok(Self {
            height,
            previous_block_hash,
            target,
            timestamp,
            coinbase_subsidy,
            total_fee,
            transactions,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmitResult {
    Accepted { block_hash: Hash256, height: u64 },
    Rejected { reason: String },
}

impl CanonicalCodec for SubmitResult {
    fn encode_canonical(&self) -> Vec<u8> {
        match self {
            SubmitResult::Accepted { block_hash, height } => {
                let mut buf = Vec::with_capacity(1 + 32 + 8);
                buf.push(0x00);
                buf.extend_from_slice(block_hash.as_bytes());
                buf.extend_from_slice(&height.to_be_bytes());
                buf
            }
            SubmitResult::Rejected { reason } => {
                let mut buf = Vec::with_capacity(reason.len().saturating_add(5));
                buf.push(0x01);
                buf.extend_from_slice(&(reason.len() as u32).to_be_bytes());
                buf.extend_from_slice(reason.as_bytes());
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
            0x00 => {
                let block_hash = Hash256::decode_from_cursor(cursor)?;
                let height = decode_u64(cursor)?;
                Ok(SubmitResult::Accepted { block_hash, height })
            }
            0x01 => {
                let reason = decode_string(cursor)?;
                Ok(SubmitResult::Rejected { reason })
            }
            other => Err(CodecError::InvalidData(format!(
                "invalid SubmitResult tag: {other:#04x}"
            ))),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use aurion_core::tx::{Datum, TxInput, TxOutput};

    fn sample_tx() -> Transaction {
        Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_output: aurion_core::OutPoint::new(Hash256::ZERO, 0),
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

    #[test]
    fn test_block_template_roundtrip() {
        let template = BlockTemplate {
            height: 201_000,
            previous_block_hash: Hash256::digest(b"prev"),
            target: Hash256::digest(b"target"),
            timestamp: 1_773_446_400,
            coinbase_subsidy: Quantum::from_raw(99_000_000),
            total_fee: Quantum::from_raw(12_500),
            transactions: vec![sample_tx(), sample_tx()],
        };

        let encoded = template.encode_canonical();
        let decoded = BlockTemplate::decode_canonical(&encoded).unwrap();
        assert_eq!(template, decoded);
    }

    #[test]
    fn test_submit_result_roundtrip() {
        let accepted = SubmitResult::Accepted {
            block_hash: Hash256::digest(b"block"),
            height: 1024,
        };
        let rejected = SubmitResult::Rejected {
            reason: "subsidy parity mismatch".into(),
        };

        assert_eq!(
            SubmitResult::decode_canonical(&accepted.encode_canonical()).unwrap(),
            accepted
        );
        assert_eq!(
            SubmitResult::decode_canonical(&rejected.encode_canonical()).unwrap(),
            rejected
        );
    }
}