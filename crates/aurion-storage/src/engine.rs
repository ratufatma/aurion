use std::path::Path;
use std::sync::Arc;
use redb::{Database, TableDefinition};

use aurion_core::block::Block;
use aurion_core::outpoint::OutPoint;
use aurion_core::tx::TxOutput;
use aurion_primitives::codec::CanonicalCodec;
use aurion_primitives::hash::Hash256;

use crate::error::StorageError;

const BLOCKS_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("canonical_blocks");
const HEIGHT_INDEX: TableDefinition<u64, &[u8]> = TableDefinition::new("height_to_hash");
const UTXO_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("canonical_utxos");
const META_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("canonical_metadata");

#[derive(Clone)]
pub struct StorageEngine {
    db: Arc<Database>,
}

impl StorageEngine {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let db = Database::create(path)?;

        // Inisialisasi seluruh tabel jika belum ada
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(BLOCKS_TABLE)?;
            let _ = write_txn.open_table(HEIGHT_INDEX)?;
            let _ = write_txn.open_table(UTXO_TABLE)?;
            let _ = write_txn.open_table(META_TABLE)?;
        }
        write_txn.commit()?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Mutasi atomik: Simpan blok, update mapping height, hapus spent UTXOs,
    /// sisipkan created UTXOs, dan perbarui tip metadata dalam SATU WriteTransaction.
    pub fn commit_block(
        &self,
        block: &Block,
        spent_inputs: &[OutPoint],
    ) -> Result<(), StorageError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut blocks_table = write_txn.open_table(BLOCKS_TABLE)?;
            let mut height_table = write_txn.open_table(HEIGHT_INDEX)?;
            let mut utxo_table = write_txn.open_table(UTXO_TABLE)?;
            let mut meta_table = write_txn.open_table(META_TABLE)?;

            let block_hash = block.block_hash();
            let block_bytes = block.encode_canonical();

            // 1. Simpan payload blok dan relasi height -> hash
            blocks_table.insert(block_hash.as_bytes().as_slice(), block_bytes.as_slice())?;
            height_table.insert(block.header.height, block_hash.as_bytes().as_slice())?;

            // 2. Hapus UTXO yang telah dibelanjakan (spent)
            for op in spent_inputs {
                let op_bytes = op.encode_canonical();
                utxo_table.remove(op_bytes.as_slice())?;
            }

            // 3. Sisipkan UTXO baru yang diciptakan dalam blok
            for tx in &block.transactions {
                let txid = tx.txid();
                for (vout, output) in tx.outputs.iter().enumerate() {
                    let op = OutPoint::new(txid, vout as u32);
                    let op_bytes = op.encode_canonical();
                    let out_bytes = output.encode_canonical();
                    utxo_table.insert(op_bytes.as_slice(), out_bytes.as_slice())?;
                }
            }

            // 4. Perbarui pointer tip kanonikal
            let height_bytes = block.header.height.to_be_bytes();
            meta_table.insert("tip_height", height_bytes.as_slice())?;
            meta_table.insert("tip_hash", block_hash.as_bytes().as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn get_block(&self, hash: &Hash256) -> Result<Option<Block>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BLOCKS_TABLE)?;
        let guard = table.get(hash.as_bytes().as_slice())?;

        match guard {
            Some(val) => {
                let block = Block::decode_canonical(val.value())?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    pub fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let height_table = read_txn.open_table(HEIGHT_INDEX)?;
        let hash_guard = height_table.get(height)?;

        match hash_guard {
            Some(val) => {
                let mut bytes = [0u8; 32];
                bytes.copy_from_slice(val.value());
                let hash = Hash256::from_bytes(bytes);
                drop(height_table);
                drop(read_txn);
                self.get_block(&hash)
            }
            None => Ok(None),
        }
    }

    pub fn get_utxo(&self, outpoint: &OutPoint) -> Result<Option<TxOutput>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(UTXO_TABLE)?;
        let op_bytes = outpoint.encode_canonical();
        let guard = table.get(op_bytes.as_slice())?;

        match guard {
            Some(val) => {
                let output = TxOutput::decode_canonical(val.value())?;
                Ok(Some(output))
            }
            None => Ok(None),
        }
    }

    pub fn get_tip(&self) -> Result<Option<(u64, Hash256)>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(META_TABLE)?;
        let h_guard = table.get("tip_height")?;
        let hash_guard = table.get("tip_hash")?;

        match (h_guard, hash_guard) {
            (Some(h), Some(hash)) => {
                let mut h_bytes = [0u8; 8];
                h_bytes.copy_from_slice(h.value());
                let height = u64::from_be_bytes(h_bytes);

                let mut hash_bytes = [0u8; 32];
                hash_bytes.copy_from_slice(hash.value());
                let tip_hash = Hash256::from_bytes(hash_bytes);

                Ok(Some((height, tip_hash)))
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurion_consensus::genesis::create_genesis_block;
    use tempfile::NamedTempFile;

    #[test]
    fn test_storage_genesis_commit_and_retrieval() {
        let file = NamedTempFile::new().unwrap();
        let storage = StorageEngine::open(file.path()).unwrap();

        assert_eq!(storage.get_tip().unwrap(), None);

        let genesis = create_genesis_block();
        let genesis_hash = genesis.block_hash();

        storage.commit_block(&genesis, &[]).unwrap();

        let tip = storage.get_tip().unwrap();
        assert_eq!(tip, Some((0, genesis_hash)));

        let loaded_block = storage.get_block(&genesis_hash).unwrap().unwrap();
        assert_eq!(loaded_block.block_hash(), genesis_hash);

        let loaded_by_height = storage.get_block_by_height(0).unwrap().unwrap();
        assert_eq!(loaded_by_height.block_hash(), genesis_hash);

        // Pastikan kedua coinbase UTXO genesis (Creator & Dev Fund) terindeks di storage
        let creator_outpoint = OutPoint::new(genesis.transactions[0].txid(), 0);
        let creator_utxo = storage.get_utxo(&creator_outpoint).unwrap().unwrap();
        assert_eq!(creator_utxo.value, genesis.transactions[0].outputs[0].value);

        let dev_outpoint = OutPoint::new(genesis.transactions[0].txid(), 1);
        let dev_utxo = storage.get_utxo(&dev_outpoint).unwrap().unwrap();
        assert_eq!(dev_utxo.value, genesis.transactions[0].outputs[1].value);
    }
}
