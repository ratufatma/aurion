use aurion_core::block::Block;
use aurion_core::tx::Transaction;
use aurion_primitives::codec::{CanonicalCodec, CodecError};
use aurion_primitives::hash::Hash256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkMessage {
    Ping(u64),
    Pong(u64),
    Version {
        version: u32,
        height: u64,
        timestamp: u64,
    },
    Verack,
    Block(Block),
    Tx(Transaction),
    Inv(Vec<Hash256>),
    GetData(Vec<Hash256>),
}

impl NetworkMessage {
    pub fn command_name(&self) -> &'static str {
        match self {
            Self::Ping(_) => "ping",
            Self::Pong(_) => "pong",
            Self::Version { .. } => "version",
            Self::Verack => "verack",
            Self::Block(_) => "block",
            Self::Tx(_) => "tx",
            Self::Inv(_) => "inv",
            Self::GetData(_) => "getdata",
        }
    }

    pub fn encode_payload(&self) -> Vec<u8> {
        match self {
            Self::Ping(nonce) | Self::Pong(nonce) => nonce.to_be_bytes().to_vec(),
            Self::Version {
                version,
                height,
                timestamp,
            } => {
                let mut buf = Vec::with_capacity(20);
                buf.extend_from_slice(&version.to_be_bytes());
                buf.extend_from_slice(&height.to_be_bytes());
                buf.extend_from_slice(&timestamp.to_be_bytes());
                buf
            }
            Self::Verack => Vec::new(),
            Self::Block(block) => block.encode_canonical(),
            Self::Tx(tx) => tx.encode_canonical(),
            Self::Inv(hashes) | Self::GetData(hashes) => {
                let mut buf = Vec::with_capacity(hashes.len().saturating_mul(32).saturating_add(4));
                buf.extend_from_slice(&(hashes.len() as u32).to_be_bytes());
                for h in hashes {
                    buf.extend_from_slice(h.as_bytes());
                }
                buf
            }
        }
    }

    pub fn decode_payload(cmd: &str, mut payload: &[u8]) -> Result<Self, CodecError> {
        let msg = match cmd {
            "ping" => {
                if payload.len() != 8 {
                    return Err(CodecError::UnexpectedEof);
                }
                let mut arr = [0u8; 8];
                arr.copy_from_slice(payload);
                payload = &[];
                Self::Ping(u64::from_be_bytes(arr))
            }
            "pong" => {
                if payload.len() != 8 {
                    return Err(CodecError::UnexpectedEof);
                }
                let mut arr = [0u8; 8];
                arr.copy_from_slice(payload);
                payload = &[];
                Self::Pong(u64::from_be_bytes(arr))
            }
            "version" => {
                if payload.len() != 20 {
                    return Err(CodecError::UnexpectedEof);
                }
                let mut v_bytes = [0u8; 4];
                v_bytes.copy_from_slice(&payload[..4]);
                let mut h_bytes = [0u8; 8];
                h_bytes.copy_from_slice(&payload[4..12]);
                let mut t_bytes = [0u8; 8];
                t_bytes.copy_from_slice(&payload[12..20]);
                payload = &[];
                Self::Version {
                    version: u32::from_be_bytes(v_bytes),
                    height: u64::from_be_bytes(h_bytes),
                    timestamp: u64::from_be_bytes(t_bytes),
                }
            }
            "verack" => {
                if !payload.is_empty() {
                    return Err(CodecError::TrailingBytes);
                }
                Self::Verack
            }
            "block" => {
                let block = Block::decode_from_cursor(&mut payload)?;
                Self::Block(block)
            }
            "tx" => {
                let tx = Transaction::decode_from_cursor(&mut payload)?;
                Self::Tx(tx)
            }
            "inv" | "getdata" => {
                if payload.len() < 4 {
                    return Err(CodecError::UnexpectedEof);
                }
                let (count_bytes, rest) = payload.split_at(4);
                let mut arr = [0u8; 4];
                arr.copy_from_slice(count_bytes);
                let count = u32::from_be_bytes(arr) as usize;

                let mut hashes = Vec::with_capacity(count);
                let mut cursor = rest;
                for _ in 0..count {
                    hashes.push(Hash256::decode_from_cursor(&mut cursor)?);
                }
                payload = cursor;
                if cmd == "inv" {
                    Self::Inv(hashes)
                } else {
                    Self::GetData(hashes)
                }
            }
            _ => {
                return Err(CodecError::InvalidData(format!(
                    "unsupported wire command: {cmd}"
                )))
            }
        };

        if !payload.is_empty() {
            return Err(CodecError::TrailingBytes);
        }

        Ok(msg)
    }
}
