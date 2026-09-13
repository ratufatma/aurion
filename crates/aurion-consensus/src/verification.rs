use aurion_core::block::Block;
use crate::difficulty::check_pow;
use crate::error::ConsensusError;
use crate::subsidy::calculate_block_subsidy;

pub fn verify_block_structure(block: &Block, expected_height: u64) -> Result<(), ConsensusError> {
    if block.transactions.is_empty() {
        return Err(ConsensusError::EmptyBlock);
    }

    if block.header.height != expected_height {
        return Err(ConsensusError::HeightMismatch {
            header_height: block.header.height,
            expected_height,
        });
    }

    // Genesis block (height 0) is the axiomatic trust anchor and does not require PoW mining.
    if block.header.height > 0 {
        check_pow(&block.block_hash(), block.header.bits)?;
    }

    let calculated_root = block.calculate_merkle_root();
    if block.header.merkle_root != calculated_root {
        return Err(ConsensusError::MerkleRootMismatch {
            expected: block.header.merkle_root,
            actual: calculated_root,
        });
    }

    let coinbase = &block.transactions[0];
    if !coinbase.is_coinbase() {
        return Err(ConsensusError::MissingCoinbase);
    }

    for tx in block.transactions.iter().skip(1) {
        if tx.is_coinbase() {
            return Err(ConsensusError::MultipleCoinbase);
        }
    }

    let mut total_coinbase_output = 0u128;
    for out in &coinbase.outputs {
        total_coinbase_output = total_coinbase_output.saturating_add(out.value.raw());
    }

    let allowed_subsidy = if expected_height == 0 {
        crate::subsidy::TOTAL_GENESIS_PREMINE_QUANTA.raw()
    } else {
        calculate_block_subsidy(expected_height).raw()
    };
    if total_coinbase_output > allowed_subsidy {
        return Err(ConsensusError::SubsidyExceeded {
            allowed: allowed_subsidy,
            claimed: total_coinbase_output,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genesis::create_genesis_block;

    #[test]
    fn test_genesis_verification() {
        let genesis = create_genesis_block();
        assert!(verify_block_structure(&genesis, 0).is_ok());
        assert!(verify_block_structure(&genesis, 1).is_err());
    }
}
