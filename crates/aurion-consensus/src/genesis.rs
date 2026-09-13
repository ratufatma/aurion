use aurion_core::block::{Block, BlockHeader};
use aurion_core::outpoint::OutPoint;
use aurion_core::tx::{Transaction, TxInput, TxOutput};
use aurion_primitives::hash::Hash256;
use aurion_primitives::quantum::Quantum;
use crate::difficulty::MAX_TARGET_BITS;
use crate::subsidy::INITIAL_SUBSIDY;

pub const GENESIS_TIMESTAMP: u64 = 1773446400; // Epoch kanonikal Aurion 2026
pub const GENESIS_PAYLOAD: &[u8] = b"Aurion: Sovereign Monolithic Digital Asset - Pure Truth";

pub fn create_genesis_block() -> Block {
    let coinbase_tx = Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_output: OutPoint::new(Hash256::ZERO, 0xFFFF_FFFF),
            unlocking_script: GENESIS_PAYLOAD.to_vec(),
            sequence: 0xFFFF_FFFF,
        }],
        outputs: vec![TxOutput {
            value: Quantum::from_raw(INITIAL_SUBSIDY),
            locking_script: vec![0x51], // OP_TRUE / unencumbered foundation output
        }],
        locktime: 0,
    };

    let merkle_root = coinbase_tx.txid();

    let header = BlockHeader {
        version: 1,
        prev_block_hash: Hash256::ZERO,
        merkle_root,
        timestamp: GENESIS_TIMESTAMP,
        bits: MAX_TARGET_BITS,
        nonce: 0,
        height: 0,
    };

    Block::new(header, vec![coinbase_tx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_block_integrity() {
        let genesis = create_genesis_block();
        assert_eq!(genesis.header.height, 0);
        assert_eq!(genesis.header.prev_block_hash, Hash256::ZERO);
        assert_eq!(genesis.transactions.len(), 1);
        assert!(genesis.transactions[0].is_coinbase());
        assert_eq!(genesis.calculate_merkle_root(), genesis.header.merkle_root);
    }
}
