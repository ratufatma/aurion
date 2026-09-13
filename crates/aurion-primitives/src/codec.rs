use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CodecError {
    #[error("unexpected end of buffer")]
    UnexpectedEof,
    #[error("trailing unconsumed bytes detected")]
    TrailingBytes,
    #[error("invalid data representation: {0}")]
    InvalidData(String),
}

pub trait CanonicalCodec: Sized {
    fn encode_canonical(&self) -> Vec<u8>;

    fn decode_canonical(bytes: &[u8]) -> Result<Self, CodecError> {
        let mut cursor = bytes;
        let item = Self::decode_from_cursor(&mut cursor)?;
        if !cursor.is_empty() {
            return Err(CodecError::TrailingBytes);
        }
        Ok(item)
    }

    fn decode_from_cursor(cursor: &mut &[u8]) -> Result<Self, CodecError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct U32Be(u32);

    impl CanonicalCodec for U32Be {
        fn encode_canonical(&self) -> Vec<u8> {
            self.0.to_be_bytes().to_vec()
        }

        fn decode_from_cursor(cursor: &mut &[u8]) -> Result<Self, CodecError> {
            if cursor.len() < 4 {
                return Err(CodecError::UnexpectedEof);
            }
            let (bytes, rest) = cursor.split_at(4);
            *cursor = rest;
            let mut arr = [0u8; 4];
            arr.copy_from_slice(bytes);
            Ok(Self(u32::from_be_bytes(arr)))
        }
    }

    #[test]
    fn test_canonical_codec_rejects_trailing_bytes() {
        let encoded = 42u32.to_be_bytes();
        assert!(U32Be::decode_canonical(&encoded).is_ok());

        let mut with_trailing = encoded.to_vec();
        with_trailing.push(0x00);
        assert_eq!(
            U32Be::decode_canonical(&with_trailing).unwrap_err(),
            CodecError::TrailingBytes
        );
    }
}
