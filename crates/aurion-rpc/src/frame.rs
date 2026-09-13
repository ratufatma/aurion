//! Codec frame 4-byte Big-Endian length-delimited dengan batas anti-DoS
//! `MAX_RPC_FRAME_SIZE` ($4\text{ MB}$). Panjang header divalidasi sebelum
//! alokasi buffer payload.
use super::error::RpcError;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const MAX_RPC_FRAME_SIZE: usize = 4 * 1024 * 1024; // 4 MB limit

pub async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    payload: &[u8],
) -> Result<(), RpcError> {
    if payload.len() > MAX_RPC_FRAME_SIZE {
        return Err(RpcError::PayloadTooLarge(payload.len()));
    }

    let len = payload.len() as u32;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(payload).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>, RpcError> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;

    if len > MAX_RPC_FRAME_SIZE {
        return Err(RpcError::PayloadTooLarge(len));
    }

    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload).await?;
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    #[tokio::test]
    async fn test_frame_roundtrip() {
        let (mut writer, mut reader) = duplex(8192);
        let payload = b"aurion-frame-canonical-check";

        write_frame(&mut writer, payload).await.unwrap();
        let decoded = read_frame(&mut reader).await.unwrap();

        assert_eq!(decoded, payload);
    }

    #[tokio::test]
    async fn test_write_frame_rejects_oversized_payload() {
        let (mut writer, _reader) = duplex(8 * 1024 * 1024);
        let payload = vec![0u8; MAX_RPC_FRAME_SIZE.saturating_add(1)];

        let err = write_frame(&mut writer, &payload).await.unwrap_err();
        assert!(matches!(err, RpcError::PayloadTooLarge(_)));
    }

    #[tokio::test]
    async fn test_read_frame_rejects_oversized_header() {
        let (mut _writer, mut reader) = duplex(8 * 1024 * 1024);
        let oversized = (MAX_RPC_FRAME_SIZE as u32).saturating_add(1);
        _writer.write_all(&oversized.to_be_bytes()).await.unwrap();

        let err = read_frame(&mut reader).await.unwrap_err();
        assert!(matches!(err, RpcError::PayloadTooLarge(_)));
    }
}