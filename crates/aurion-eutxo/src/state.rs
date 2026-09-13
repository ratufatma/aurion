use aurion_core::tx::TxOutput;
use aurion_primitives::codec::{CanonicalCodec, CodecError};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExtendedUtxo {
    pub output: TxOutput,
    pub creation_height: u64,
    pub is_coinbase: bool,
}

impl ExtendedUtxo {
    pub fn new(output: TxOutput, creation_height: u64, is_coinbase: bool) -> Self {
        Self {
            output,
            creation_height,
            is_coinbase,
        }
    }
}

impl CanonicalCodec for ExtendedUtxo {
    fn encode_canonical(&self) -> Vec<u8> {
        let mut buf = self.output.encode_canonical();
        buf.extend_from_slice(&self.creation_height.to_be_bytes());
        buf.push(if self.is_coinbase { 1 } else { 0 });
        buf
    }

    fn decode_from_cursor(cursor: &mut &[u8]) -> Result<Self, CodecError> {
        let output = TxOutput::decode_from_cursor(cursor)?;
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

        Ok(Self {
            output,
            creation_height,
            is_coinbase,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurion_primitives::quantum::Quantum;

    #[test]
    fn test_extended_utxo_codec_roundtrip() {
        let utxo = ExtendedUtxo::new(
            TxOutput {
                value: Quantum::from_raw(50_000),
                locking_script: vec![0x51],
            },
            42,
            true,
        );

        let encoded = utxo.encode_canonical();
        let decoded = ExtendedUtxo::decode_canonical(&encoded).unwrap();
        assert_eq!(utxo, decoded);
    }
}
