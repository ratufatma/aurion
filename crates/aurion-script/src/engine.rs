use aurion_primitives::hash::Hash256;
use ed25519_dalek::{Signature, VerifyingKey};

use crate::error::ScriptError;
use crate::opcode::OpCode;
use crate::stack::Stack;

pub const MAX_OPS_PER_SCRIPT: usize = 201;

#[derive(Default)]
pub struct ScriptEngine {
    stack: Stack,
}

impl ScriptEngine {
    pub fn new() -> Self {
        Self {
            stack: Stack::new(),
        }
    }

    pub fn is_truthy(data: &[u8]) -> bool {
        if data.is_empty() {
            return false;
        }
        for &byte in data {
            if byte != 0 {
                return true;
            }
        }
        false
    }

    /// Evaluasi dua tahap standar eUTXO:
    /// 1. Unlocking script dieksekusi terlebih dahulu (mendorong saksi/parameter ke stack).
    /// 2. Locking script dieksekusi pada stack yang sama.
    /// 3. Hasil akhir valid jika elemen teratas bernilai benar (truthy non-zero).
    pub fn verify(
        unlocking_script: &[u8],
        locking_script: &[u8],
        sighash: &Hash256,
    ) -> Result<(), ScriptError> {
        let mut engine = Self::new();
        let mut op_count = 0usize;

        engine.execute_bytecode(unlocking_script, sighash, &mut op_count)?;
        engine.execute_bytecode(locking_script, sighash, &mut op_count)?;

        let top = engine.stack.pop().map_err(|_| ScriptError::EmptyStackOnTermination)?;
        if !Self::is_truthy(&top) {
            return Err(ScriptError::ScriptFailed);
        }

        Ok(())
    }

    fn execute_bytecode(
        &mut self,
        bytecode: &[u8],
        sighash: &Hash256,
        op_count: &mut usize,
    ) -> Result<(), ScriptError> {
        let mut cursor = bytecode;

        while !cursor.is_empty() {
            if *op_count >= MAX_OPS_PER_SCRIPT {
                return Err(ScriptError::MaxOpCountExceeded(MAX_OPS_PER_SCRIPT));
            }
            *op_count = match op_count.checked_add(1) {
                Some(val) => val,
                None => return Err(ScriptError::MaxOpCountExceeded(MAX_OPS_PER_SCRIPT)),
            };

            let op_byte = cursor[0];
            cursor = &cursor[1..];

            // Direct Push data: 0x01..=0x4b mendorong 1 sampai 75 byte berikutnya
            if (0x01..=0x4b).contains(&op_byte) {
                let push_len = op_byte as usize;
                if cursor.len() < push_len {
                    return Err(ScriptError::UnexpectedEof);
                }
                let (push_data, rest) = cursor.split_at(push_len);
                cursor = rest;
                self.stack.push(push_data.to_vec())?;
                continue;
            }

            let op = OpCode::from_u8(op_byte).ok_or(ScriptError::InvalidOpCode(op_byte))?;
            match op {
                OpCode::OpFalse => {
                    self.stack.push(vec![])?;
                }
                OpCode::OpTrue => {
                    self.stack.push(vec![1])?;
                }
                OpCode::OpNop => {}
                OpCode::OpReturn => {
                    return Err(ScriptError::OpReturnTriggered);
                }
                OpCode::OpVerify => {
                    let item = self.stack.pop()?;
                    if !Self::is_truthy(&item) {
                        return Err(ScriptError::VerifyFailed);
                    }
                }
                OpCode::OpDup => {
                    self.stack.dup()?;
                }
                OpCode::OpDrop => {
                    self.stack.drop()?;
                }
                OpCode::OpSwap => {
                    self.stack.swap()?;
                }
                OpCode::OpEqual => {
                    let a = self.stack.pop()?;
                    let b = self.stack.pop()?;
                    if a == b {
                        self.stack.push(vec![1])?;
                    } else {
                        self.stack.push(vec![])?;
                    }
                }
                OpCode::OpEqualVerify => {
                    let a = self.stack.pop()?;
                    let b = self.stack.pop()?;
                    if a != b {
                        return Err(ScriptError::EqualVerifyFailed);
                    }
                }
                OpCode::OpAdd => {
                    let b_bytes = self.stack.pop()?;
                    let a_bytes = self.stack.pop()?;
                    let a_val = decode_script_int(&a_bytes)?;
                    let b_val = decode_script_int(&b_bytes)?;
                    let sum = a_val
                        .checked_add(b_val)
                        .ok_or(ScriptError::ArithmeticOverflow)?;
                    self.stack.push(sum.to_le_bytes().to_vec())?;
                }
                OpCode::OpSub => {
                    let b_bytes = self.stack.pop()?;
                    let a_bytes = self.stack.pop()?;
                    let a_val = decode_script_int(&a_bytes)?;
                    let b_val = decode_script_int(&b_bytes)?;
                    let diff = a_val
                        .checked_sub(b_val)
                        .ok_or(ScriptError::ArithmeticOverflow)?;
                    self.stack.push(diff.to_le_bytes().to_vec())?;
                }
                OpCode::OpHash256 => {
                    let item = self.stack.pop()?;
                    let digest = Hash256::digest(&item);
                    self.stack.push(digest.as_bytes().to_vec())?;
                }
                OpCode::OpCheckSig => {
                    let pubkey_bytes = self.stack.pop()?;
                    let sig_bytes = self.stack.pop()?;
                    let valid = verify_ed25519(&pubkey_bytes, &sig_bytes, sighash.as_bytes());
                    if valid {
                        self.stack.push(vec![1])?;
                    } else {
                        self.stack.push(vec![])?;
                    }
                }
                OpCode::OpCheckSigVerify => {
                    let pubkey_bytes = self.stack.pop()?;
                    let sig_bytes = self.stack.pop()?;
                    if !verify_ed25519(&pubkey_bytes, &sig_bytes, sighash.as_bytes()) {
                        return Err(ScriptError::VerifyFailed);
                    }
                }
            }
        }

        Ok(())
    }
}

fn decode_script_int(bytes: &[u8]) -> Result<i64, ScriptError> {
    if bytes.is_empty() {
        return Ok(0);
    }
    if bytes.len() > 8 {
        return Err(ScriptError::ArithmeticOverflow);
    }
    let mut arr = [0u8; 8];
    arr[..bytes.len()].copy_from_slice(bytes);
    Ok(i64::from_le_bytes(arr))
}

fn verify_ed25519(pubkey_bytes: &[u8], sig_bytes: &[u8], message: &[u8]) -> bool {
    if pubkey_bytes.len() != 32 || sig_bytes.len() != 64 {
        return false;
    }

    let mut pk_arr = [0u8; 32];
    pk_arr.copy_from_slice(pubkey_bytes);

    let verifying_key = match VerifyingKey::from_bytes(&pk_arr) {
        Ok(k) => k,
        Err(_) => return false,
    };

    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(sig_bytes);
    let signature = Signature::from_bytes(&sig_arr);

    verifying_key.verify_strict(message, &signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn test_op_true_evaluation() {
        let locking = vec![OpCode::OpTrue as u8];
        assert!(ScriptEngine::verify(&[], &locking, &Hash256::ZERO).is_ok());
    }

    #[test]
    fn test_op_false_fails() {
        let locking = vec![OpCode::OpFalse as u8];
        assert_eq!(
            ScriptEngine::verify(&[], &locking, &Hash256::ZERO).unwrap_err(),
            ScriptError::ScriptFailed
        );
    }

    #[test]
    fn test_hash256_equalverify_flow() {
        let secret = b"aurion-truth-preimage";
        let hash = Hash256::digest(secret);

        // Locking script: OP_DUP OP_HASH256 <hash> OP_EQUALVERIFY
        let mut locking = vec![OpCode::OpDup as u8, OpCode::OpHash256 as u8, 32];
        locking.extend_from_slice(hash.as_bytes());
        locking.push(OpCode::OpEqualVerify as u8);

        // Unlocking script: <secret>
        let mut unlocking = vec![secret.len() as u8];
        unlocking.extend_from_slice(secret);

        assert!(ScriptEngine::verify(&unlocking, &locking, &Hash256::ZERO).is_ok());
    }

    #[test]
    fn test_ed25519_checksig_flow() {
        let rng_bytes = [7u8; 32];
        let signing_key = SigningKey::from_bytes(&rng_bytes);
        let verifying_key = signing_key.verifying_key();
        let sighash = Hash256::digest(b"canonical-tx-spend-bytes");

        let signature = signing_key.sign(sighash.as_bytes());

        // Locking script: <pubkey> OP_CHECKSIG
        let mut locking = vec![32];
        locking.extend_from_slice(verifying_key.as_bytes());
        locking.push(OpCode::OpCheckSig as u8);

        // Unlocking script: <signature>
        let mut unlocking = vec![64];
        unlocking.extend_from_slice(&signature.to_bytes());

        assert!(ScriptEngine::verify(&unlocking, &locking, &sighash).is_ok());
    }

    #[test]
    fn test_op_return_rejection() {
        let locking = vec![OpCode::OpReturn as u8];
        assert_eq!(
            ScriptEngine::verify(&[], &locking, &Hash256::ZERO).unwrap_err(),
            ScriptError::OpReturnTriggered
        );
    }
}
