use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;
use std::sync::Arc;
use thiserror::Error;

use aurion_consensus::verification::verify_block_structure;
use aurion_core::block::Block;
use aurion_primitives::hash::Hash256;
use aurion_storage::StorageEngine;

use crate::authority::invariants::{verify_block_invariants, InvariantError};
use crate::lifecycle::fault::NodeFault;
use crate::lifecycle::state::NodeState;

#[derive(Debug, Error)]
pub enum AuthorityError {
    #[error("node halted or in fatal state")]
    NodeHalted,

    #[error("consensus violation: {0}")]
    Consensus(#[from] aurion_consensus::error::ConsensusError),

    #[error("invariant tripwire triggered: {0}")]
    Invariant(#[from] InvariantError),

    #[error("fatal storage fault: {0}")]
    Storage(String),

    #[error("invalid block connection: expected parent {expected}, got {actual}")]
    InvalidParent {
        expected: Hash256,
        actual: Hash256,
    },
}

pub struct AuthorityEngine {
    state: RwLock<NodeState>,
    storage: Arc<StorageEngine>,
    cancel_signal: AtomicBool,
}

impl AuthorityEngine {
    pub fn new(storage: Arc<StorageEngine>) -> Self {
        Self {
            state: RwLock::new(NodeState::Starting),
            storage,
            cancel_signal: AtomicBool::new(false),
        }
    }

    #[inline]
    pub fn state(&self) -> NodeState {
        match self.state.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub fn enforce_operational(&self) -> Result<(), AuthorityError> {
        let is_operational = match self.state.read() {
            Ok(guard) => guard.is_operational(),
            Err(poisoned) => poisoned.into_inner().is_operational(),
        };
        if !is_operational {
            return Err(AuthorityError::NodeHalted);
        }
        Ok(())
    }

    /// Menandai sistem dalam status terminal Failed secara permanen.
    /// Tidak ada mutasi lanjutan yang akan diterima.
    pub fn fail_stop(&self, fault: NodeFault) {
        tracing::error!(
            target = "aurion::fatal",
            ?fault,
            "SOVEREIGN RUNTIME ENTERING TERMINAL FAIL-STOP STATE"
        );

        {
            let mut state = match self.state.write() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            *state = NodeState::Failed(fault);
        }

        self.cancel_signal.store(true, Ordering::SeqCst);
    }

    pub fn initialize_genesis(&self) -> Result<(), AuthorityError> {
        {
            let mut state = match self.state.write() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            *state = NodeState::Recovering;
        }

        let tip = self.storage.get_tip().map_err(|e| {
            AuthorityError::Storage(e.to_string())
        })?;

        if tip.is_none() {
            let genesis = aurion_consensus::genesis::create_genesis_block();
            self.storage.commit_block(&genesis, &[]).map_err(|e| {
                AuthorityError::Storage(e.to_string())
            })?;
            tracing::info!("Initialized canonical genesis block in redb storage");
        }

        {
            let mut state = match self.state.write() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            *state = NodeState::Running;
        }
        Ok(())
    }

    /// Single-Commit Atomic Pipeline:
    /// 1. Operational Guard
    /// 2. Structural & PoW Verification
    /// 3. Parent Linkage Verification
    /// 4. Invariant Verification against current UTXO state
    /// 5. Single Atomic Commit into redb
    /// 6. Automatic fail-stop if atomic commit fails
    pub fn process_and_commit_block(&self, block: &Block) -> Result<(), AuthorityError> {
        self.enforce_operational()?;

        let tip = self.storage.get_tip().map_err(|e| {
            self.fail_stop(NodeFault::StateCorruption(e.to_string()));
            AuthorityError::Storage(e.to_string())
        })?;

        let (expected_height, expected_parent) = match tip {
            Some((h, parent_hash)) => (
                h.checked_add(1).ok_or(AuthorityError::NodeHalted)?,
                parent_hash,
            ),
            None => (0, Hash256::ZERO),
        };

        if block.header.prev_block_hash != expected_parent {
            return Err(AuthorityError::InvalidParent {
                expected: expected_parent,
                actual: block.header.prev_block_hash,
            });
        }

        verify_block_structure(block, expected_height)?;

        let spent_inputs = verify_block_invariants(block, |op| {
            self.storage.get_utxo(op).unwrap_or(None)
        })?;

        if let Err(err) = self.storage.commit_block(block, &spent_inputs) {
            let fault = NodeFault::StorageCommitFailure(err.to_string());
            self.fail_stop(fault);
            return Err(AuthorityError::Storage(err.to_string()));
        }

        tracing::info!(
            height = block.header.height,
            hash = %block.block_hash(),
            "Canonical block committed to authoritative ledger"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_authority_fail_stop_locks_mutation() {
        let file = NamedTempFile::new().unwrap();
        let storage = Arc::new(StorageEngine::open(file.path()).unwrap());
        let authority = AuthorityEngine::new(storage);

        authority.initialize_genesis().unwrap();
        assert!(authority.state().is_operational());

        // Picu fail-stop manual
        authority.fail_stop(NodeFault::InvariantViolation("test invariant failure".into()));
        assert!(authority.state().is_failed());

        // Buktikan bahwa semua eksekusi mutasi langsung ditolak (Fail-Stop)
        let genesis = aurion_consensus::genesis::create_genesis_block();
        let err = authority.process_and_commit_block(&genesis).unwrap_err();
        assert!(matches!(err, AuthorityError::NodeHalted));
    }
}
