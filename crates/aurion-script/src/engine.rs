use aurion_core::tx::Datum;
use aurion_primitives::codec::CanonicalCodec;

use aurion_primitives::hash::Hash256;
use ed25519_dalek::{Signature, VerifyingKey};

use crate::context::ScriptContext;
use crate::error::ScriptError;
use crate::opcode::OpCode;
use crate::stack::Stack;

pub const MAX_OPS_PER_SCRIPT: usize = 201;

pub struct ScriptEngine<'a> {
    pub stack: Stack,
    pub context: Option<&'a ScriptContext<'a>>,
    pub sighash: Hash256,
}

impl<'a> Default for ScriptEngine<'a> {
    fn default() -> Self {
        Self::new(None)
    }
}

impl<'a> ScriptEngine<'a> {
    pub fn new(context: Option<&'a ScriptContext<'a>>) -> Self {
        let sighash = context
            .and_then(|ctx| {
                if ctx.current_input_index < ctx.tx.inputs.len() {
                    Some(ctx.tx.sighash(ctx.current_input_index))
                } else {
                    None
                }
            })
            .unwrap_or(Hash256::ZERO);

        Self {
            stack: Stack::new(),
            context,
            sighash,
        }
    }

    pub fn with_sighash(context: Option<&'a ScriptContext<'a>>, sighash: Hash256) -> Self {
        Self {
            stack: Stack::new(),
            context,
            sighash,
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
        let mut engine = Self::with_sighash(None, *sighash);
        let mut op_count = 0usize;

        engine.execute_bytecode(unlocking_script, &mut op_count)?;
        engine.execute_bytecode(locking_script, &mut op_count)?;

        let top = engine.stack.pop().map_err(|_| ScriptError::EmptyStackOnTermination)?;
        if !Self::is_truthy(&top) {
            return Err(ScriptError::ScriptFailed);
        }

        Ok(())
    }

    /// Eksekusi kontrak eUTXO berdasarkan model (Datum, Redeemer, ScriptContext).
    /// Execution Flow:
    /// - Step a: Push Datum if Inline or Hash (skip if None).
    /// - Step b: Push redeemer bytes if Some.
    /// - Step c: Execute unlocking_script bytecode.
    /// - Step d: Execute locking_script bytecode.
    /// - Step e: Ensure stack has exactly 1 element and its value is truthy.
    pub fn execute_contract(
        &mut self,
        locking_script: &[u8],
        unlocking_script: &[u8],
        datum: &Datum,
        redeemer: Option<&[u8]>,
    ) -> Result<bool, ScriptError> {
        self.stack = Stack::new();

        // Step a: Push Datum if Inline or Hash (skip if None).
        match datum {
            Datum::None => {}
            Datum::Hash(hash) => {
                self.stack.push(hash.as_bytes().to_vec())?;
            }
            Datum::Inline(data) => {
                self.stack.push(data.clone())?;
            }
        }

        // Step b: Push redeemer bytes if Some.
        if let Some(r) = redeemer {
            self.stack.push(r.to_vec())?;
        }

        let mut op_count = 0usize;

        // Step c: Execute unlocking_script bytecode.
        self.execute_bytecode(unlocking_script, &mut op_count)?;

        // Step d: Execute locking_script bytecode.
        self.execute_bytecode(locking_script, &mut op_count)?;

        // Step e: Ensure stack has exactly 1 element and its value is truthy.
        if self.stack.len() != 1 {
            return Ok(false);
        }

        let top = self.stack.pop()?;
        Ok(Self::is_truthy(&top))
    }

    fn execute_bytecode(
        &mut self,
        bytecode: &[u8],
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
                    let valid = verify_ed25519(&pubkey_bytes, &sig_bytes, self.sighash.as_bytes());
                    if valid {
                        self.stack.push(vec![1])?;
                    } else {
                        self.stack.push(vec![])?;
                    }
                }
                OpCode::OpCheckSigVerify => {
                    let pubkey_bytes = self.stack.pop()?;
                    let sig_bytes = self.stack.pop()?;
                    if !verify_ed25519(&pubkey_bytes, &sig_bytes, self.sighash.as_bytes()) {
                        return Err(ScriptError::VerifyFailed);
                    }
                }
                OpCode::OpTxInputsCount => {
                    let ctx = self.context.ok_or(ScriptError::ContextUnavailable)?;
                    let count = u32::try_from(ctx.tx.inputs.len())
                        .map_err(|_| ScriptError::ArithmeticOverflow)?;
                    self.stack.push(count.to_le_bytes().to_vec())?;
                }
                OpCode::OpCurrentInputIdx => {
                    let ctx = self.context.ok_or(ScriptError::ContextUnavailable)?;
                    let idx = u32::try_from(ctx.current_input_index)
                        .map_err(|_| ScriptError::ArithmeticOverflow)?;
                    self.stack.push(idx.to_le_bytes().to_vec())?;
                }
                OpCode::OpTxOutputsCount => {
                    let ctx = self.context.ok_or(ScriptError::ContextUnavailable)?;
                    let count = u32::try_from(ctx.tx.outputs.len())
                        .map_err(|_| ScriptError::ArithmeticOverflow)?;
                    self.stack.push(count.to_le_bytes().to_vec())?;
                }
                OpCode::OpTxOutputValue => {
                    let ctx = self.context.ok_or(ScriptError::ContextUnavailable)?;
                    let idx_bytes = self.stack.pop()?;
                    let idx = decode_u32_index(&idx_bytes)?;
                    let output = ctx
                        .tx
                        .outputs
                        .get(idx)
                        .ok_or(ScriptError::IndexOutOfBounds(idx))?;
                    let val = output.value.as_u128();
                    self.stack.push(val.to_le_bytes().to_vec())?;
                }
                OpCode::OpTxOutputScript => {
                    let ctx = self.context.ok_or(ScriptError::ContextUnavailable)?;
                    let idx_bytes = self.stack.pop()?;
                    let idx = decode_u32_index(&idx_bytes)?;
                    let output = ctx
                        .tx
                        .outputs
                        .get(idx)
                        .ok_or(ScriptError::IndexOutOfBounds(idx))?;
                    let digest = Hash256::digest(&output.locking_script);
                    self.stack.push(digest.as_bytes().to_vec())?;
                }
                OpCode::OpTxOutputDatum => {
                    let ctx = self.context.ok_or(ScriptError::ContextUnavailable)?;
                    let idx_bytes = self.stack.pop()?;
                    let idx = decode_u32_index(&idx_bytes)?;
                    let output = ctx
                        .tx
                        .outputs
                        .get(idx)
                        .ok_or(ScriptError::IndexOutOfBounds(idx))?;
                    let hash = match &output.datum {
                        Datum::None => [0u8; 32],
                        datum => *Hash256::digest(&datum.encode_canonical()).as_bytes(),
                    };
                    self.stack.push(hash.to_vec())?;
                }
                OpCode::OpTxLocktime => {
                    let ctx = self.context.ok_or(ScriptError::ContextUnavailable)?;
                    self.stack.push(ctx.tx.locktime.to_le_bytes().to_vec())?;
                }
                OpCode::OpValidationHeight => {
                    let ctx = self.context.ok_or(ScriptError::ContextUnavailable)?;
                    self.stack
                        .push(ctx.validation_height.to_le_bytes().to_vec())?;
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

fn decode_u32_index(bytes: &[u8]) -> Result<usize, ScriptError> {
    if bytes.is_empty() {
        return Ok(0);
    }
    if bytes.len() > 4 {
        return Err(ScriptError::ArithmeticOverflow);
    }
    let mut arr = [0u8; 4];
    arr[..bytes.len()].copy_from_slice(bytes);
    let val = u32::from_le_bytes(arr);
    usize::try_from(val).map_err(|_| ScriptError::ArithmeticOverflow)
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
    use aurion_core::tx::{OutPoint, Transaction, TxInput, TxOutput};
    use aurion_eutxo::state::ExtendedUtxo;
    use aurion_primitives::Quantum;
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

    #[test]
    fn test_introspection_opcodes() {
        let tx = Transaction {
            version: 1,
            inputs: vec![
                TxInput {
                    previous_output: OutPoint::new(Hash256::ZERO, 0),
                    unlocking_script: vec![],
                    sequence: 0xFFFF_FFFF,
                    redeemer: None,
                },
                TxInput {
                    previous_output: OutPoint::new(Hash256::ZERO, 1),
                    unlocking_script: vec![],
                    sequence: 0xFFFF_FFFF,
                    redeemer: None,
                },
            ],
            outputs: vec![
                TxOutput {
                    value: Quantum::from_raw(100),
                    locking_script: vec![OpCode::OpTrue as u8],
                    datum: Datum::None,
                },
                TxOutput {
                    value: Quantum::from_raw(200),
                    locking_script: vec![OpCode::OpTrue as u8],
                    datum: Datum::Inline(vec![0xAA, 0xBB]),
                },
            ],
            locktime: 12345,
        };

        let spent_utxo = ExtendedUtxo::new(
            Quantum::from_raw(500),
            vec![OpCode::OpTrue as u8],
            10,
            false,
            Datum::None,
        );

        let context = ScriptContext {
            tx: &tx,
            current_input_index: 1,
            current_spent_utxo: &spent_utxo,
            fee: Quantum::from_raw(10),
            validation_height: 999,
        };

        // Test OpTxInputsCount
        {
            let mut engine = ScriptEngine::new(Some(&context));
            let locking = vec![OpCode::OpTxInputsCount as u8];
            let mut op_count = 0;
            engine.execute_bytecode(&locking, &mut op_count).unwrap();
            let popped = engine.stack.pop().unwrap();
            assert_eq!(popped, 2u32.to_le_bytes().to_vec());
        }

        // Test OpCurrentInputIdx
        {
            let mut engine = ScriptEngine::new(Some(&context));
            let locking = vec![OpCode::OpCurrentInputIdx as u8];
            let mut op_count = 0;
            engine.execute_bytecode(&locking, &mut op_count).unwrap();
            let popped = engine.stack.pop().unwrap();
            assert_eq!(popped, 1u32.to_le_bytes().to_vec());
        }

        // Test OpTxOutputsCount
        {
            let mut engine = ScriptEngine::new(Some(&context));
            let locking = vec![OpCode::OpTxOutputsCount as u8];
            let mut op_count = 0;
            engine.execute_bytecode(&locking, &mut op_count).unwrap();
            let popped = engine.stack.pop().unwrap();
            assert_eq!(popped, 2u32.to_le_bytes().to_vec());
        }

        // Test OpTxOutputValue for index 1 (value: 200)
        {
            let mut engine = ScriptEngine::new(Some(&context));
            // Push 1 (1 byte), OpTxOutputValue
            let locking = vec![1, 1, OpCode::OpTxOutputValue as u8];
            let mut op_count = 0;
            engine.execute_bytecode(&locking, &mut op_count).unwrap();
            let popped = engine.stack.pop().unwrap();
            assert_eq!(popped, 200u128.to_le_bytes().to_vec());
        }

        // Test OpValidationHeight and OpTxLocktime
        {
            let mut engine = ScriptEngine::new(Some(&context));
            let locking = vec![
                OpCode::OpValidationHeight as u8,
                OpCode::OpTxLocktime as u8,
            ];
            let mut op_count = 0;
            engine.execute_bytecode(&locking, &mut op_count).unwrap();
            let locktime = engine.stack.pop().unwrap();
            let height = engine.stack.pop().unwrap();
            assert_eq!(locktime, 12345u64.to_le_bytes().to_vec());
            assert_eq!(height, 999u64.to_le_bytes().to_vec());
        }

        // Test ContextUnavailable error when context is None
        {
            let mut engine = ScriptEngine::new(None);
            let locking = vec![OpCode::OpTxOutputsCount as u8];
            let mut op_count = 0;
            assert_eq!(
                engine.execute_bytecode(&locking, &mut op_count).unwrap_err(),
                ScriptError::ContextUnavailable
            );
        }
    }

    #[test]
    fn test_covenant_output_enforcement() {
        // Covenant: Output 0 must have value exactly equal to 500 Quantum.
        // Script:
        // Push 0 (index 0) -> OpTxOutputValue -> Push 500u128 -> OpEqual
        let mut locking = vec![
            1, 0, // push 1 byte: 0 (index)
            OpCode::OpTxOutputValue as u8,
            16, // push 16 bytes: 500u128
        ];
        locking.extend_from_slice(&500u128.to_le_bytes());
        locking.push(OpCode::OpEqual as u8);

        let spent_utxo = ExtendedUtxo::new(
            Quantum::from_raw(1000),
            locking.clone(),
            1,
            false,
            Datum::None,
        );

        // Case 1: Valid output amount (500)
        let valid_tx = Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_output: OutPoint::new(Hash256::ZERO, 0),
                unlocking_script: vec![],
                sequence: 0xFFFF_FFFF,
                redeemer: None,
            }],
            outputs: vec![TxOutput {
                value: Quantum::from_raw(500),
                locking_script: vec![OpCode::OpTrue as u8],
                datum: Datum::None,
            }],
            locktime: 0,
        };

        let valid_context = ScriptContext {
            tx: &valid_tx,
            current_input_index: 0,
            current_spent_utxo: &spent_utxo,
            fee: Quantum::from_raw(500),
            validation_height: 10,
        };

        let mut engine = ScriptEngine::new(Some(&valid_context));
        let res = engine
            .execute_contract(&locking, &[], &Datum::None, None)
            .unwrap();
        assert!(res, "Covenant should accept transaction with exact 500 Quantum output");

        // Case 2: Invalid output amount (400)
        let invalid_tx = Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_output: OutPoint::new(Hash256::ZERO, 0),
                unlocking_script: vec![],
                sequence: 0xFFFF_FFFF,
                redeemer: None,
            }],
            outputs: vec![TxOutput {
                value: Quantum::from_raw(400),
                locking_script: vec![OpCode::OpTrue as u8],
                datum: Datum::None,
            }],
            locktime: 0,
        };

        let invalid_context = ScriptContext {
            tx: &invalid_tx,
            current_input_index: 0,
            current_spent_utxo: &spent_utxo,
            fee: Quantum::from_raw(600),
            validation_height: 10,
        };

        let mut engine_invalid = ScriptEngine::new(Some(&invalid_context));
        let res_invalid = engine_invalid
            .execute_contract(&locking, &[], &Datum::None, None)
            .unwrap();
        assert!(!res_invalid, "Covenant should reject transaction with 400 Quantum output");
    }

    #[test]
    fn test_datum_redeemer_contract_resolution() {
        // Contract requiring: Datum + Redeemer == 15
        // Flow:
        // Datum is pushed first (10)
        // Redeemer is pushed second (5)
        // Unlocking script: empty
        // Locking script: OpAdd -> Push 15 (8 bytes) -> OpEqual
        let mut locking = vec![
            OpCode::OpAdd as u8,
            8, // push 8 bytes
        ];
        locking.extend_from_slice(&15i64.to_le_bytes());
        locking.push(OpCode::OpEqual as u8);

        let datum = Datum::Inline(10i64.to_le_bytes().to_vec());
        let valid_redeemer = 5i64.to_le_bytes();
        let invalid_redeemer = 7i64.to_le_bytes();

        // Valid execution: 10 + 5 == 15
        let mut engine = ScriptEngine::new(None);
        let res = engine
            .execute_contract(&locking, &[], &datum, Some(&valid_redeemer))
            .unwrap();
        assert!(res, "Datum(10) + Redeemer(5) should equal 15");

        // Invalid execution: 10 + 7 != 15
        let mut engine = ScriptEngine::new(None);
        let res_invalid = engine
            .execute_contract(&locking, &[], &datum, Some(&invalid_redeemer))
            .unwrap();
        assert!(!res_invalid, "Datum(10) + Redeemer(7) should not equal 15");
    }
}
