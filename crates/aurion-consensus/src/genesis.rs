use aurion_core::block::{Block, BlockHeader};
use aurion_core::outpoint::OutPoint;
use aurion_core::tx::{Datum, Transaction, TxInput, TxOutput};
use aurion_primitives::hash::Hash256;
use aurion_primitives::quantum::Quantum;
use crate::difficulty::MAX_TARGET_BITS;
use crate::subsidy::{CREATOR_ALLOCATION_AUR, DEV_ALLOCATION_AUR, QUANTA_PER_AUR};

pub const GENESIS_TIMESTAMP: u64 = 1773446400; // Epoch kanonikal Aurion 2026
pub const GENESIS_PAYLOAD: &[u8] = b"Aurion: Sovereign Monolithic Digital Asset - Pure Truth";
pub const GENESIS_NONCE: u64 = 124776; // Mined canonical nonce satisfying MAX_TARGET_BITS

pub fn create_genesis_block() -> Block {
    let creator_value = Quantum::new(CREATOR_ALLOCATION_AUR.saturating_mul(QUANTA_PER_AUR));
    let dev_value = Quantum::new(DEV_ALLOCATION_AUR.saturating_mul(QUANTA_PER_AUR));

    let coinbase_tx = Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_output: OutPoint::new(Hash256::ZERO, 0xFFFF_FFFF),
            unlocking_script: GENESIS_PAYLOAD.to_vec(),
            sequence: 0xFFFF_FFFF,
            redeemer: None,
        }],
        outputs: vec![
            TxOutput {
                value: creator_value,
                locking_script: vec![0x51], // OP_TRUE / unencumbered creator output
                datum: Datum::None,
            },
            TxOutput {
                value: dev_value,
                locking_script: vec![0x51], // OP_TRUE / unencumbered developer fund output
                datum: Datum::None,
            },
        ],
        locktime: 0,
    };

    let merkle_root = coinbase_tx.txid();

    let header = BlockHeader {
        version: 1,
        prev_block_hash: Hash256::ZERO,
        merkle_root,
        timestamp: GENESIS_TIMESTAMP,
        bits: MAX_TARGET_BITS,
        nonce: GENESIS_NONCE,
        height: 0,
    };

    Block::new(header, vec![coinbase_tx])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::difficulty::check_pow;

    #[test]
    fn test_genesis_block_integrity() {
        let genesis = create_genesis_block();
        println!(
            "GENESIS NONCE: {}, HASH: {}, MERKLE: {}",
            genesis.header.nonce,
            genesis.block_hash(),
            genesis.header.merkle_root
        );
        assert_eq!(genesis.header.height, 0);
        assert_eq!(genesis.header.prev_block_hash, Hash256::ZERO);
        assert_eq!(genesis.transactions.len(), 1);
        assert!(genesis.transactions[0].is_coinbase());
        assert_eq!(genesis.transactions[0].outputs.len(), 2);
        assert_eq!(genesis.calculate_merkle_root(), genesis.header.merkle_root);
        assert!(check_pow(&genesis.block_hash(), genesis.header.bits).is_ok());
    }
}
