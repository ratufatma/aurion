use aurion_core::tx::{Datum, TxOutput};
use aurion_primitives::codec::{CanonicalCodec, CodecError};
use aurion_primitives::quantum::Quantum;

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
pub struct ExtendedUtxo {
    pub value: Quantum,
    pub locking_script: Vec<u8>,
    pub creation_height: u64,
    pub is_coinbase: bool,
    pub datum: Datum,
}

impl ExtendedUtxo {
    pub fn new(
        value: Quantum,
        locking_script: Vec<u8>,
        creation_height: u64,
        is_coinbase: bool,
        datum: Datum,
    ) -> Self {
        Self {
            value,
            locking_script,
            creation_height,
            is_coinbase,
            datum,
        }
    }

    pub fn from_output(output: &TxOutput, creation_height: u64, is_coinbase: bool) -> Self {
        Self {
            value: output.value,
            locking_script: output.locking_script.clone(),
            creation_height,
            is_coinbase,
            datum: output.datum.clone(),
        }
    }

    pub fn to_output(&self) -> TxOutput {
        TxOutput {
            value: self.value,
            locking_script: self.locking_script.clone(),
            datum: self.datum.clone(),
        }
    }
}

impl CanonicalCodec for ExtendedUtxo {
    fn encode_canonical(&self) -> Vec<u8> {
        let mut buf = self.value.encode_canonical();
        buf.extend(encode_vec_bytes(&self.locking_script));
        buf.extend_from_slice(&self.creation_height.to_be_bytes());
        buf.push(if self.is_coinbase { 1 } else { 0 });
        buf.extend(self.datum.encode_canonical());
        buf
    }

    fn decode_from_cursor(cursor: &mut &[u8]) -> Result<Self, CodecError> {
        let value = Quantum::decode_from_cursor(cursor)?;
        let locking_script = decode_vec_bytes(cursor)?;

        if cursor.len() < 9 {
            return Err(CodecError::UnexpectedEof);
        }
        let (h_bytes, rest) = cursor.split_at(8);
        *cursor = rest;
        let mut arr8 = [0u8; 8];
        arr8.copy_from_slice(h_bytes);
        let creation_height = u64::from_be_bytes(arr8);

        let is_coinbase_byte = cursor[0];
        *cursor = &cursor[1..];
        let is_coinbase = is_coinbase_byte == 1;

        let datum = Datum::decode_from_cursor(cursor)?;

        Ok(Self {
            value,
            locking_script,
            creation_height,
            is_coinbase,
            datum,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extended_utxo_codec_roundtrip() {
        let utxo = ExtendedUtxo::new(
            Quantum::from_raw(50_000),
            vec![0x51],
            42,
            true,
            Datum::Inline(vec![1, 2, 3]),
        );

        let encoded = utxo.encode_canonical();
        let decoded = ExtendedUtxo::decode_canonical(&encoded).unwrap();
        assert_eq!(utxo, decoded);
    }
}
