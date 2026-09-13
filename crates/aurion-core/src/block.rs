use aurion_primitives::codec::{CanonicalCodec, CodecError};
use aurion_primitives::hash::Hash256;
use crate::merkle::compute_merkle_root;
use crate::tx::Transaction;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BlockHeader {
    pub version: u32,
    pub prev_block_hash: Hash256,
    pub merkle_root: Hash256,
    pub timestamp: u64,
    pub bits: u32,
    pub nonce: u64,
    pub height: u64,
}

impl BlockHeader {
    pub fn block_hash(&self) -> Hash256 {
        Hash256::digest(&self.encode_canonical())
    }
}

impl CanonicalCodec for BlockHeader {
    fn encode_canonical(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + 32 + 32 + 8 + 4 + 8 + 8);
        buf.extend_from_slice(&self.version.to_be_bytes());
        buf.extend_from_slice(self.prev_block_hash.as_bytes());
        buf.extend_from_slice(self.merkle_root.as_bytes());
        buf.extend_from_slice(&self.timestamp.to_be_bytes());
        buf.extend_from_slice(&self.bits.to_be_bytes());
        buf.extend_from_slice(&self.nonce.to_be_bytes());
        buf.extend_from_slice(&self.height.to_be_bytes());
        buf
    }

    fn decode_from_cursor(cursor: &mut &[u8]) -> Result<Self, CodecError> {
        if cursor.len() < 4 {
            return Err(CodecError::UnexpectedEof);
        }
        let (ver_bytes, rest) = cursor.split_at(4);
        *cursor = rest;
        let mut arr4 = [0u8; 4];
        arr4.copy_from_slice(ver_bytes);
        let version = u32::from_be_bytes(arr4);

        let prev_block_hash = Hash256::decode_from_cursor(cursor)?;
        let merkle_root = Hash256::decode_from_cursor(cursor)?;

        if cursor.len() < 28 {
            return Err(CodecError::UnexpectedEof);
        }

        let (ts_bytes, rest) = cursor.split_at(8);
        let mut arr8 = [0u8; 8];
        arr8.copy_from_slice(ts_bytes);
        let timestamp = u64::from_be_bytes(arr8);

        let (bits_bytes, rest) = rest.split_at(4);
        arr4.copy_from_slice(bits_bytes);
        let bits = u32::from_be_bytes(arr4);

        let (nonce_bytes, rest) = rest.split_at(8);
        arr8.copy_from_slice(nonce_bytes);
        let nonce = u64::from_be_bytes(arr8);

        let (height_bytes, rest) = rest.split_at(8);
        *cursor = rest;
        arr8.copy_from_slice(height_bytes);
        let height = u64::from_be_bytes(arr8);

        Ok(Self {
            version,
            prev_block_hash,
            merkle_root,
            timestamp,
            bits,
            nonce,
            height,
        })
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

impl Block {
    pub fn new(header: BlockHeader, transactions: Vec<Transaction>) -> Self {
        Self { header, transactions }
    }

    pub fn block_hash(&self) -> Hash256 {
        self.header.block_hash()
    }

    pub fn calculate_merkle_root(&self) -> Hash256 {
        let txids: Vec<Hash256> = self.transactions.iter().map(|tx| tx.txid()).collect();
        compute_merkle_root(&txids)
    }
}

impl CanonicalCodec for Block {
    fn encode_canonical(&self) -> Vec<u8> {
        let mut buf = self.header.encode_canonical();
        buf.extend_from_slice(&(self.transactions.len() as u32).to_be_bytes());
        for tx in &self.transactions {
            buf.extend(tx.encode_canonical());
        }
        buf
    }

    fn decode_from_cursor(cursor: &mut &[u8]) -> Result<Self, CodecError> {
        let header = BlockHeader::decode_from_cursor(cursor)?;

        if cursor.len() < 4 {
            return Err(CodecError::UnexpectedEof);
        }
        let (len_bytes, rest) = cursor.split_at(4);
        *cursor = rest;
        let mut arr4 = [0u8; 4];
        arr4.copy_from_slice(len_bytes);
        let tx_count = u32::from_be_bytes(arr4) as usize;

        let mut transactions = Vec::with_capacity(tx_count);
        for _ in 0..tx_count {
            transactions.push(Transaction::decode_from_cursor(cursor)?);
        }

        Ok(Self { header, transactions })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outpoint::OutPoint;
    use crate::tx::{TxInput, TxOutput};
    use aurion_primitives::quantum::Quantum;

    #[test]
    fn test_block_codec_roundtrip() {
        let tx = Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_output: OutPoint::new(Hash256::ZERO, 0),
                unlocking_script: vec![0x10],
                sequence: 0,
            }],
            outputs: vec![TxOutput {
                value: Quantum::from_raw(50_000_000),
                locking_script: vec![0x20],
            }],
            locktime: 0,
        };

        let header = BlockHeader {
            version: 1,
            prev_block_hash: Hash256::ZERO,
            merkle_root: tx.txid(),
            timestamp: 1773446400,
            bits: 0x1f00ffff,
            nonce: 42,
            height: 0,
        };

        let block = Block::new(header, vec![tx]);
        let encoded = block.encode_canonical();
        let decoded = Block::decode_canonical(&encoded).unwrap();

        assert_eq!(block, decoded);
        assert_eq!(block.block_hash(), decoded.block_hash());
    }
}
