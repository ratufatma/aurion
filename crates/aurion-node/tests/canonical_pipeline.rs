#![allow(unused_imports)]
#![allow(clippy::arithmetic_side_effects)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use tempfile::NamedTempFile;

use ed25519_dalek::Signer;
use ed25519_dalek::SigningKey;

use aurion_consensus::difficulty::{check_pow, MAX_TARGET_BITS};
use aurion_consensus::genesis::create_genesis_block;
use aurion_consensus::subsidy::INITIAL_SUBSIDY;
use aurion_core::block::{Block, BlockHeader};
use aurion_core::outpoint::OutPoint;
use aurion_core::tx::{Datum, Transaction, TxInput, TxOutput};
use aurion_node::authority::{AuthorityEngine, AuthorityError, InvariantError};
use aurion_node::lifecycle::NodeFault;
use aurion_storage::StorageEngine;
use aurion_primitives::codec::CanonicalCodec;
use aurion_primitives::hash::Hash256;
use aurion_primitives::quantum::Quantum;
use aurion_script::OpCode;

/// Helper untuk menambang blok deterministik dengan mencari nonce yang memenuhi MAX_TARGET_BITS
fn mine_block(mut header: BlockHeader, transactions: Vec<Transaction>) -> Block {
    header.merkle_root = {
        let txids: Vec<Hash256> = transactions.iter().map(|tx| tx.txid()).collect();
        aurion_core::merkle::compute_merkle_root(&txids)
    };

    let mut nonce = 0u64;
    loop {
        header.nonce = nonce;
        let hash = header.block_hash();
        if check_pow(&hash, header.bits).is_ok() {
            break;
        }
        nonce = match nonce.checked_add(1) {
            Some(n) => n,
            None => panic!("nonce overflow during mining test"),
        };
    }

    Block::new(header, transactions)
}

#[test]
fn test_end_to_end_canonical_chain_lifecycle() {
    // 1. Inisialisasi StorageEngine & AuthorityEngine bersih
    let temp_db = NamedTempFile::new().unwrap();
    let storage = Arc::new(StorageEngine::open(temp_db.path()).unwrap());
    let authority = AuthorityEngine::new(storage.clone());

    // 2. Genesis Initialization
    authority.initialize_genesis().expect("genesis init must succeed");
    assert!(authority.state().is_operational());

    let (tip_height, tip_hash) = storage.get_tip().unwrap().unwrap();
    assert_eq!(tip_height, 0);

    let genesis = create_genesis_block();
    assert_eq!(tip_hash, genesis.block_hash());

    // 3. Persiapkan kunci Ed25519 untuk Miner A
    let miner_sk = SigningKey::from_bytes(&[1u8; 32]);
    let miner_pk = miner_sk.verifying_key();

    // Locking script untuk Miner A: <pubkey> OP_CHECKSIG
    let mut miner_locking_script = vec![32];
    miner_locking_script.extend_from_slice(miner_pk.as_bytes());
    miner_locking_script.push(OpCode::OpCheckSig as u8);

    // 4. Tambang Block 1: Berisi Coinbase menuju Miner A
    let coinbase_b1 = Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_output: OutPoint::new(Hash256::ZERO, 0xFFFF_FFFF),
            unlocking_script: b"Mined by Miner A".to_vec(),
            sequence: 0xFFFF_FFFF,
            redeemer: None,
        }],
        outputs: vec![TxOutput {
            value: Quantum::from_raw(INITIAL_SUBSIDY),
            locking_script: miner_locking_script.clone(),
            datum: Datum::None,
        }],
        locktime: 0,
    };

    let header_b1 = BlockHeader {
        version: 1,
        prev_block_hash: tip_hash,
        merkle_root: Hash256::ZERO,
        timestamp: genesis.header.timestamp + 600,
        bits: MAX_TARGET_BITS,
        nonce: 0,
        height: 1,
    };

    let block_1 = mine_block(header_b1, vec![coinbase_b1.clone()]);
    let block_1_hash = block_1.block_hash();

    authority
        .process_and_commit_block(&block_1)
        .expect("block 1 must be committed");

    let (new_height, new_tip) = storage.get_tip().unwrap().unwrap();
    assert_eq!(new_height, 1);
    assert_eq!(new_tip, block_1_hash);

    // 5. Verifikasi bahwa UTXO Coinbase Block 1 tercatat di storage
    let b1_coinbase_op = OutPoint::new(coinbase_b1.txid(), 0);
    let utxo_b1 = storage.get_utxo(&b1_coinbase_op).unwrap().expect("coinbase UTXO must exist");
    assert_eq!(utxo_b1.value, Quantum::from_raw(INITIAL_SUBSIDY));

    // 6. Belanjakan UTXO Block 1 di Block 2 (Transfer ke Recipient B)
    let recipient_locking_script = vec![OpCode::OpTrue as u8]; // Unencumbered output
    let spend_val = Quantum::from_raw(2_000_000_000);
    let change_val = Quantum::from_raw(7_900_000_000); // 2B + 7.9B = 9.9B (INITIAL_SUBSIDY, Zero Fee)

    let mut spend_tx = Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_output: b1_coinbase_op,
            unlocking_script: vec![], // Diisi setelah tanda tangan
            sequence: 0,
            redeemer: None,
        }],
        outputs: vec![
            TxOutput {
                value: spend_val,
                locking_script: recipient_locking_script.clone(),
                datum: Datum::None,
            },
            TxOutput {
                value: change_val,
                locking_script: miner_locking_script.clone(),
                datum: Datum::None,
            },
        ],
        locktime: 0,
    };

    // Buat tanda tangan Ed25519 atas sighash(0)
    let sighash_0 = spend_tx.sighash(0);
    let sig = miner_sk.sign(sighash_0.as_bytes());

    let mut unlocking = vec![64];
    unlocking.extend_from_slice(&sig.to_bytes());
    spend_tx.inputs[0].unlocking_script = unlocking;

    // Tambahkan juga coinbase untuk Block 2
    let coinbase_b2 = Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_output: OutPoint::new(Hash256::ZERO, 0xFFFF_FFFF),
            unlocking_script: b"Coinbase B2".to_vec(),
            sequence: 0xFFFF_FFFF,
            redeemer: None,
        }],
        outputs: vec![TxOutput {
            value: Quantum::from_raw(INITIAL_SUBSIDY),
            locking_script: miner_locking_script.clone(),
            datum: Datum::None,
        }],
        locktime: 0,
    };

    let header_b2 = BlockHeader {
        version: 1,
        prev_block_hash: block_1_hash,
        merkle_root: Hash256::ZERO,
        timestamp: block_1.header.timestamp + 600,
        bits: MAX_TARGET_BITS,
        nonce: 0,
        height: 2,
    };

    let block_2 = mine_block(header_b2, vec![coinbase_b2, spend_tx.clone()]);
    let block_2_hash = block_2.block_hash();

    authority
        .process_and_commit_block(&block_2)
        .expect("block 2 with Ed25519 spend must succeed");

    let (height_2, tip_2) = storage.get_tip().unwrap().unwrap();
    assert_eq!(height_2, 2);
    assert_eq!(tip_2, block_2_hash);

    // 7. Buktikan bahwa UTXO lama (Block 1 Coinbase) sudah SPENT dan terhapus dari redb
    assert!(storage.get_utxo(&b1_coinbase_op).unwrap().is_none());

    // 8. Buktikan bahwa UTXO baru (Recipient & Change) tersimpan di redb
    let recipient_op = OutPoint::new(spend_tx.txid(), 0);
    let change_op = OutPoint::new(spend_tx.txid(), 1);

    let utxo_recipient = storage.get_utxo(&recipient_op).unwrap().expect("recipient UTXO must exist");
    assert_eq!(utxo_recipient.value, spend_val);

    let utxo_change = storage.get_utxo(&change_op).unwrap().expect("change UTXO must exist");
    assert_eq!(utxo_change.value, change_val);
}

#[test]
fn test_double_spend_rejection_protects_canonical_ledger() {
    let temp_db = NamedTempFile::new().unwrap();
    let storage = Arc::new(StorageEngine::open(temp_db.path()).unwrap());
    let authority = AuthorityEngine::new(storage.clone());
    authority.initialize_genesis().unwrap();

    let genesis = create_genesis_block();
    let genesis_hash = genesis.block_hash();

    // Coinbase genesis OutPoint (height 0)
    let genesis_outpoint = OutPoint::new(genesis.transactions[0].txid(), 0);

    // Buat transaksi belanja sah dari genesis
    let tx1 = Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_output: genesis_outpoint,
            unlocking_script: vec![],
            sequence: 0,
            redeemer: None,
        }],
        outputs: vec![TxOutput {
            value: Quantum::from_raw(INITIAL_SUBSIDY),
            locking_script: vec![OpCode::OpTrue as u8],
            datum: Datum::None,
        }],
        locktime: 0,
    };

    // Buat transaksi double spend pada input yang sama di dalam blok yang sama
    let tx2 = Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_output: genesis_outpoint,
            unlocking_script: vec![],
            sequence: 1,
            redeemer: None,
        }],
        outputs: vec![TxOutput {
            value: Quantum::from_raw(INITIAL_SUBSIDY),
            locking_script: vec![OpCode::OpTrue as u8],
            datum: Datum::None,
        }],
        locktime: 0,
    };

    let coinbase = Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_output: OutPoint::new(Hash256::ZERO, 0xFFFF_FFFF),
            unlocking_script: vec![],
            sequence: 0,
            redeemer: None,
        }],
        outputs: vec![TxOutput {
            value: Quantum::from_raw(INITIAL_SUBSIDY),
            locking_script: vec![OpCode::OpTrue as u8],
            datum: Datum::None,
        }],
        locktime: 0,
    };

    let header = BlockHeader {
        version: 1,
        prev_block_hash: genesis_hash,
        merkle_root: Hash256::ZERO,
        timestamp: genesis.header.timestamp + 600,
        bits: MAX_TARGET_BITS,
        nonce: 0,
        height: 1,
    };

    let bad_block = mine_block(header, vec![coinbase, tx1, tx2]);
    let result = authority.process_and_commit_block(&bad_block);

    assert!(matches!(
        result,
        Err(AuthorityError::Invariant(InvariantError::DuplicateInputSpend(op))) if op == genesis_outpoint
    ));

    // Ledger tetap di height 0 (tidak tercemar)
    let (h, _) = storage.get_tip().unwrap().unwrap();
    assert_eq!(h, 0);
}
