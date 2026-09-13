#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

use zenoh::{Session, Wait};

use crate::error::NetworkError;
use crate::frame::{pack_frame, unpack_frame, HEADER_SIZE};
use crate::topics::{TOPIC_IPC_MINER_TEMPLATE, TOPIC_SYNC_BLOCKS, TOPIC_SYNC_HEADERS};

/// Query/Reply synchronisation engine for Initial Block Download (IBD)
/// and miner IPC over the Aurion Zenoh brokerless peer mesh.
///
/// Uses Zenoh's `get()` / `declare_queryable()` primitives for request/reply patterns
/// without any centralised coordinator.
pub struct SyncEngine<'s> {
    session: &'s Session,
}

impl<'s> SyncEngine<'s> {
    /// Create a new `SyncEngine` bound to the given Zenoh session.
    pub fn new(session: &'s Session) -> Self {
        Self { session }
    }

    /// Query a peer for a block template on [`TOPIC_IPC_MINER_TEMPLATE`].
    ///
    /// Returns the raw payload bytes for the caller to validate and decode.
    ///
    /// # Errors
    /// Returns [`NetworkError::TransportError`] if the query fails or times out.
    pub async fn request_block_template(&self) -> Result<Vec<u8>, NetworkError> {
        let replies = self
            .session
            .get(TOPIC_IPC_MINER_TEMPLATE)
            .await
            .map_err(|e| NetworkError::TransportError(e.to_string()))?;

        if let Ok(reply) = replies.recv_async().await {
            match reply.result() {
                Ok(sample) => {
                    let raw: Vec<u8> = sample.payload().to_bytes().to_vec();
                    if raw.len() < HEADER_SIZE {
                        return Err(NetworkError::IncompleteHeader(raw.len()));
                    }
                    let (_, payload) = unpack_frame(&raw)?;
                    return Ok(payload.to_vec());
                }
                Err(e) => {
                    return Err(NetworkError::TransportError(format!(
                        "query reply error: {e:?}"
                    )));
                }
            }
        }

        Err(NetworkError::TransportError(
            "no reply received for block template request".into(),
        ))
    }

    /// Register this node as a block template provider on [`TOPIC_IPC_MINER_TEMPLATE`].
    ///
    /// The `handler` is called once per incoming query, and should return the
    /// raw bytes to include in the reply payload (without wire framing — this function
    /// applies the 52-byte frame before replying).
    ///
    /// # Errors
    /// Returns [`NetworkError::TransportError`] if the queryable declaration fails.
    pub async fn register_block_template_provider<F>(
        &self,
        handler: F,
    ) -> Result<zenoh::query::Queryable<()>, NetworkError>
    where
        F: Fn() -> Vec<u8> + Send + Sync + 'static,
    {
        let queryable = self
            .session
            .declare_queryable(TOPIC_IPC_MINER_TEMPLATE)
            .callback(move |query| {
                let payload = handler();
                let frame = match pack_frame("template", &payload) {
                    Ok(f) => f,
                    Err(e) => {
                        tracing::error!("Failed to pack template frame: {e}");
                        return;
                    }
                };
                if let Err(e) = query.reply(TOPIC_IPC_MINER_TEMPLATE, frame).wait() {
                    tracing::error!("Failed to send template reply: {e}");
                }
            })
            .await
            .map_err(|e| NetworkError::TransportError(e.to_string()))?;

        Ok(queryable)
    }

    /// Perform a chain header synchronisation query on [`TOPIC_SYNC_HEADERS`].
    ///
    /// Sends a framed request payload (e.g. a height range) and returns
    /// the raw response payload bytes for the caller to decode.
    ///
    /// # Errors
    /// Returns [`NetworkError::TransportError`] on transport failure.
    pub async fn sync_headers(&self, request_payload: &[u8]) -> Result<Vec<u8>, NetworkError> {
        let frame = pack_frame("getheaders", request_payload)?;

        let replies = self
            .session
            .get(TOPIC_SYNC_HEADERS)
            .payload(frame)
            .await
            .map_err(|e| NetworkError::TransportError(e.to_string()))?;

        if let Ok(reply) = replies.recv_async().await {
            match reply.result() {
                Ok(sample) => {
                    let raw: Vec<u8> = sample.payload().to_bytes().to_vec();
                    if raw.len() < HEADER_SIZE {
                        return Err(NetworkError::IncompleteHeader(raw.len()));
                    }
                    let (_, payload) = unpack_frame(&raw)?;
                    return Ok(payload.to_vec());
                }
                Err(e) => {
                    return Err(NetworkError::TransportError(format!(
                        "sync reply error: {e:?}"
                    )));
                }
            }
        }

        Err(NetworkError::TransportError(
            "no reply received for sync headers query".into(),
        ))
    }

    /// Perform a block range synchronisation query on [`TOPIC_SYNC_BLOCKS`].
    ///
    /// # Errors
    /// Returns [`NetworkError::TransportError`] on transport failure.
    pub async fn sync_blocks(&self, request_payload: &[u8]) -> Result<Vec<u8>, NetworkError> {
        let frame = pack_frame("getblocks", request_payload)?;

        let replies = self
            .session
            .get(TOPIC_SYNC_BLOCKS)
            .payload(frame)
            .await
            .map_err(|e| NetworkError::TransportError(e.to_string()))?;

        if let Ok(reply) = replies.recv_async().await {
            match reply.result() {
                Ok(sample) => {
                    let raw: Vec<u8> = sample.payload().to_bytes().to_vec();
                    if raw.len() < HEADER_SIZE {
                        return Err(NetworkError::IncompleteHeader(raw.len()));
                    }
                    let (_, payload) = unpack_frame(&raw)?;
                    return Ok(payload.to_vec());
                }
                Err(e) => {
                    return Err(NetworkError::TransportError(format!(
                        "sync reply error: {e:?}"
                    )));
                }
            }
        }

        Err(NetworkError::TransportError(
            "no reply received for sync blocks query".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::pack_frame;

    #[test]
    fn test_sync_engine_frame_codec_consistency() {
        let request = b"height-range-100-200";
        let frame = pack_frame("getheaders", request).unwrap();
        let (cmd, payload) = unpack_frame(&frame).unwrap();
        assert_eq!(cmd, "getheaders");
        assert_eq!(payload, request);
    }

    #[test]
    fn test_sync_blocks_codec_consistency() {
        let frame = pack_frame("getblocks", b"hash-xyz").unwrap();
        let (cmd, payload) = unpack_frame(&frame).unwrap();
        assert_eq!(cmd, "getblocks");
        assert_eq!(payload, b"hash-xyz");
    }
}
