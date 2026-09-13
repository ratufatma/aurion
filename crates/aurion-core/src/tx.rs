use aurion_primitives::codec::{CanonicalCodec, CodecError};
use aurion_primitives::hash::Hash256;
use aurion_primitives::quantum::Quantum;
use crate::outpoint::OutPoint;

fn encode_vec_bytes(bytes: &[u8]) -> Vec<u8> {
    let len = bytes.len() as u32;
    let mut buf = Vec::with_capacity(bytes.len().saturating_add(4));
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(bytes);
    buf
}

fn decode_vec_bytes(cursor: &mut &[u8]) -> Result<Vec<u8>, CodecError> {
    if cursor.len() < 4 {
        return Err(CodecError::UnexpectedEof);
    }
    let (len_bytes, rest) = cursor.split_at(4);
    *cursor = rest;
    let mut arr = [0u8; 4];
    arr.copy_from_slice(len_bytes);
    let len = u32::from_be_bytes(arr) as usize;

    if cursor.len() < len {
        return Err(CodecError::UnexpectedEof);
    }
    let (data, rest) = cursor.split_at(len);
    *cursor = rest;
    Ok(data.to_vec())
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TxInput {
    pub previous_output: OutPoint,
    pub unlocking_script: Vec<u8>,
    pub sequence: u64,
}

impl CanonicalCodec for TxInput {
    fn encode_canonical(&self) -> Vec<u8> {
        let mut buf = self.previous_output.encode_canonical();
        buf.extend(encode_vec_bytes(&self.unlocking_script));
        buf.extend_from_slice(&self.sequence.to_be_bytes());
        buf
    }

    fn decode_from_cursor(cursor: &mut &[u8]) -> Result<Self, CodecError> {
        let previous_output = OutPoint::decode_from_cursor(cursor)?;
        let unlocking_script = decode_vec_bytes(cursor)?;
        if cursor.len() < 8 {
            return Err(CodecError::UnexpectedEof);
        }
        let (seq_bytes, rest) = cursor.split_at(8);
        *cursor = rest;
        let mut arr = [0u8; 8];
        arr.copy_from_slice(seq_bytes);
        let sequence = u64::from_be_bytes(arr);

        Ok(Self {
            previous_output,
            unlocking_script,
            sequence,
        })
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TxOutput {
    pub value: Quantum,
    pub locking_script: Vec<u8>,
}

impl CanonicalCodec for TxOutput {
    fn encode_canonical(&self) -> Vec<u8> {
        let mut buf = self.value.encode_canonical();
        buf.extend(encode_vec_bytes(&self.locking_script));
        buf
    }

    fn decode_from_cursor(cursor: &mut &[u8]) -> Result<Self, CodecError> {
        let value = Quantum::decode_from_cursor(cursor)?;
        let locking_script = decode_vec_bytes(cursor)?;
        Ok(Self { value, locking_script })
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Transaction {
    pub version: u32,
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
    pub locktime: u64,
}

impl Transaction {
    pub fn txid(&self) -> Hash256 {
        Hash256::digest(&self.encode_canonical())
    }

    pub fn is_coinbase(&self) -> bool {
        self.inputs.len() == 1 && self.inputs[0].previous_output.txid == Hash256::ZERO
    }

    /// Menghitung sighash unik untuk input tertentu dengan mengikat data transaksi dan indeks input.
    /// Unlocking script dikosongkan agar signature menandatangani kerangka transaksi (tanpa circular dependency).
    pub fn sighash(&self, input_index: usize) -> Hash256 {
        let mut tx_copy = self.clone();
        for input in &mut tx_copy.inputs {
            input.unlocking_script.clear();
        }
        let mut preimage = tx_copy.encode_canonical();
        preimage.extend_from_slice(&(input_index as u32).to_be_bytes());
        Hash256::digest(&preimage)
    }
}

impl CanonicalCodec for Transaction {
    fn encode_canonical(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.version.to_be_bytes());

        buf.extend_from_slice(&(self.inputs.len() as u32).to_be_bytes());
        for input in &self.inputs {
            buf.extend(input.encode_canonical());
        }

        buf.extend_from_slice(&(self.outputs.len() as u32).to_be_bytes());
        for output in &self.outputs {
            buf.extend(output.encode_canonical());
        }

        buf.extend_from_slice(&self.locktime.to_be_bytes());
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

        if cursor.len() < 4 {
            return Err(CodecError::UnexpectedEof);
        }
        let (in_len_bytes, rest) = cursor.split_at(4);
        *cursor = rest;
        arr4.copy_from_slice(in_len_bytes);
        let in_len = u32::from_be_bytes(arr4) as usize;

        let mut inputs = Vec::with_capacity(in_len);
        for _ in 0..in_len {
            inputs.push(TxInput::decode_from_cursor(cursor)?);
        }

        if cursor.len() < 4 {
            return Err(CodecError::UnexpectedEof);
        }
        let (out_len_bytes, rest) = cursor.split_at(4);
        *cursor = rest;
        arr4.copy_from_slice(out_len_bytes);
        let out_len = u32::from_be_bytes(arr4) as usize;

        let mut outputs = Vec::with_capacity(out_len);
        for _ in 0..out_len {
            outputs.push(TxOutput::decode_from_cursor(cursor)?);
        }

        if cursor.len() < 8 {
            return Err(CodecError::UnexpectedEof);
        }
        let (lock_bytes, rest) = cursor.split_at(8);
        *cursor = rest;
        let mut arr8 = [0u8; 8];
        arr8.copy_from_slice(lock_bytes);
        let locktime = u64::from_be_bytes(arr8);

        Ok(Self {
            version,
            inputs,
            outputs,
            locktime,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tx_codec_roundtrip() {
        let tx = Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_output: OutPoint::new(Hash256::digest(b"prev"), 0),
                unlocking_script: vec![0x01, 0x02],
                sequence: 0xFFFFFFFF,
            }],
            outputs: vec![TxOutput {
                value: Quantum::from_raw(5000),
                locking_script: vec![0xaa, 0xbb],
            }],
            locktime: 0,
        };

        let bytes = tx.encode_canonical();
        let decoded = Transaction::decode_canonical(&bytes).unwrap();
        assert_eq!(tx, decoded);
        assert_eq!(tx.txid(), decoded.txid());
        assert_ne!(tx.sighash(0), Hash256::ZERO);
        assert_ne!(tx.sighash(0), tx.sighash(1));
    }
}
