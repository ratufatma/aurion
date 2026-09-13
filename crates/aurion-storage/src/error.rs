use thiserror::Error;
use aurion_primitives::codec::CodecError;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("redb database error: {0}")]
    Database(#[from] redb::DatabaseError),
    #[error("redb internal error: {0}")]
    Error(#[from] redb::Error),
    #[error("redb transaction error: {0}")]
    Transaction(#[from] redb::TransactionError),
    #[error("redb table error: {0}")]
    Table(#[from] redb::TableError),
    #[error("redb commit error: {0}")]
    Commit(#[from] redb::CommitError),
    #[error("redb storage error: {0}")]
    Storage(#[from] redb::StorageError),
    #[error("canonical serialization failure: {0}")]
    Serialization(#[from] CodecError),
    #[error("inconsistent metadata: {0}")]
    InconsistentMetadata(String),
}
