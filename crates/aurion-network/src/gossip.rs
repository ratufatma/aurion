#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

use aurion_core::block::Block;
use aurion_core::tx::Transaction;
use aurion_primitives::codec::CanonicalCodec;
use futures::Stream;
use zenoh::Session;

use crate::error::NetworkError;
use crate::frame::{pack_frame, unpack_frame};
use crate::topics::{TOPIC_BLOCKS_NEW, TOPIC_TX_NEW};

/// Engine for publishing and subscribing to block and transaction gossip
/// across the Aurion brokerless Zenoh peer mesh.
///
/// Owns a cheaply-cloneable [`zenoh::Session`] so the engine can outlive the
/// bootstrap handle and be shared across `tokio::spawn` tasks. Each method
/// wraps payloads in the 52-byte canonical wire frame (Blake3-checksum-protected)
/// before publishing, and validates the frame on receipt before decoding.
pub struct GossipEngine {
    session: Session,
}

impl GossipEngine {
    /// Create a new `GossipEngine` bound to a shared Zenoh session.
    ///
    /// `Session` is cheaply cloneable (Arc-backed); clones keep the underlying
    /// session alive independently of the original bootstrap handle.
    pub fn new(session: Session) -> Self {
        Self { session }
    }

    /// Broadcast a [`Block`] to all peer subscribers on [`TOPIC_BLOCKS_NEW`].
    ///
    /// Encodes the block via `CanonicalCodec`, wraps it in a 52-byte wire frame,
    /// and publishes to the Zenoh key expression.
    ///
    /// # Errors
    /// Returns [`NetworkError::TransportError`] if the publish fails.
    pub async fn broadcast_block(&self, block: &Block) -> Result<(), NetworkError> {
        let payload = block.encode_canonical();
        let frame = pack_frame("block", &payload)?;

        self.session
            .put(TOPIC_BLOCKS_NEW, frame)
            .await
            .map_err(|e| NetworkError::TransportError(e.to_string()))?;

        tracing::debug!(
            topic = TOPIC_BLOCKS_NEW,
            payload_bytes = payload.len(),
            "Block gossip published"
        );

        Ok(())
    }

    /// Broadcast a [`Transaction`] to all peer subscribers on [`TOPIC_TX_NEW`].
    ///
    /// Encodes the transaction via `CanonicalCodec`, wraps it in a 52-byte wire frame,
    /// and publishes to the Zenoh key expression.
    ///
    /// # Errors
    /// Returns [`NetworkError::TransportError`] if the publish fails.
    pub async fn broadcast_transaction(&self, tx: &Transaction) -> Result<(), NetworkError> {
        let payload = tx.encode_canonical();
        let frame = pack_frame("tx", &payload)?;

        self.session
            .put(TOPIC_TX_NEW, frame)
            .await
            .map_err(|e| NetworkError::TransportError(e.to_string()))?;

        tracing::debug!(
            topic = TOPIC_TX_NEW,
            payload_bytes = payload.len(),
            "Transaction gossip published"
        );

        Ok(())
    }

    /// Subscribe to new block announcements on [`TOPIC_BLOCKS_NEW`].
    ///
    /// Declares a Zenoh subscriber and returns a pinned async [`Stream`] yielding
    /// decoded [`Block`] instances wrapped in wire-frame validation checks.
    ///
    /// # Errors
    /// Returns [`NetworkError::TransportError`] if subscriber declaration fails.
    pub async fn subscribe_blocks(
        &self,
    ) -> Result<impl Stream<Item = Result<Block, NetworkError>> + Send + Unpin, NetworkError> {
        let subscriber = self
            .session
            .declare_subscriber(TOPIC_BLOCKS_NEW)
            .await
            .map_err(|e| NetworkError::TransportError(e.to_string()))?;

        let stream = Box::pin(futures::stream::unfold(subscriber, |sub| async move {
            match sub.recv_async().await {
                Ok(sample) => {
                    let raw = sample.payload().to_bytes();
                    let decoded = Self::decode_block_frame(&raw);
                    Some((decoded, sub))
                }
                Err(_) => None,
            }
        }));

        Ok(stream)
    }

    /// Subscribe to new transaction announcements on [`TOPIC_TX_NEW`].
    ///
    /// Declares a Zenoh subscriber and returns a pinned async [`Stream`] yielding
    /// decoded [`Transaction`] instances wrapped in wire-frame validation checks.
    ///
    /// # Errors
    /// Returns [`NetworkError::TransportError`] if subscriber declaration fails.
    pub async fn subscribe_transactions(
        &self,
    ) -> Result<
        impl Stream<Item = Result<Transaction, NetworkError>> + Send + Unpin,
        NetworkError,
    > {
        let subscriber = self
            .session
            .declare_subscriber(TOPIC_TX_NEW)
            .await
            .map_err(|e| NetworkError::TransportError(e.to_string()))?;

        let stream = Box::pin(futures::stream::unfold(subscriber, |sub| async move {
            match sub.recv_async().await {
                Ok(sample) => {
                    let raw = sample.payload().to_bytes();
                    let decoded = Self::decode_tx_frame(&raw);
                    Some((decoded, sub))
                }
                Err(_) => None,
            }
        }));

        Ok(stream)
    }

    /// Decode raw frame bytes into a [`Block`] after validating the wire frame.
    ///
    /// # Errors
    /// Returns [`NetworkError::ChecksumMismatch`], [`NetworkError::IncompleteHeader`],
    /// [`NetworkError::TruncatedPayload`], or [`NetworkError::CodecError`] on decode failure.
    pub fn decode_block_frame(raw: &[u8]) -> Result<Block, NetworkError> {
        let (cmd, payload) = unpack_frame(raw)?;
        if cmd != "block" {
            return Err(NetworkError::CodecError(format!(
                "expected 'block' command, got '{cmd}'"
            )));
        }
        Block::decode_canonical(payload)
            .map_err(|e| NetworkError::CodecError(e.to_string()))
    }

    /// Decode raw frame bytes into a [`Transaction`] after validating the wire frame.
    ///
    /// # Errors
    /// Returns decode errors on wire frame or canonical codec failure.
    pub fn decode_tx_frame(raw: &[u8]) -> Result<Transaction, NetworkError> {
        let (cmd, payload) = unpack_frame(raw)?;
        if cmd != "tx" {
            return Err(NetworkError::CodecError(format!(
                "expected 'tx' command, got '{cmd}'"
            )));
        }
        Transaction::decode_canonical(payload)
            .map_err(|e| NetworkError::CodecError(e.to_string()))
    }
}