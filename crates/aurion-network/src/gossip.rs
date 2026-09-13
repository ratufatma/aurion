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
use crate::topics::{TOPIC_BLOCK_GOSSIP, TOPIC_TX_GOSSIP};

/// Engine for publishing and subscribing to block and transaction gossip
/// across the Aurion brokerless Zenoh peer mesh.
///
/// Each method wraps payloads in the 52-byte canonical wire frame
/// (Blake3-checksum-protected) before publishing, and validates the frame
/// on receipt before decoding.
pub struct GossipEngine<'s> {
    session: &'s Session,
}

impl<'s> GossipEngine<'s> {
    /// Create a new `GossipEngine` bound to the given Zenoh session.
    pub fn new(session: &'s Session) -> Self {
        Self { session }
    }

    /// Broadcast a [`Block`] to all peer subscribers on [`TOPIC_BLOCK_GOSSIP`].
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
            .put(TOPIC_BLOCK_GOSSIP, frame)
            .await
            .map_err(|e| NetworkError::TransportError(e.to_string()))?;

        tracing::debug!(
            topic = TOPIC_BLOCK_GOSSIP,
            payload_bytes = payload.len(),
            "Block gossip published"
        );

        Ok(())
    }

    /// Broadcast a [`Transaction`] to all peer subscribers on [`TOPIC_TX_GOSSIP`].
    ///
    /// Encodes the transaction via `CanonicalCodec`, wraps it in a 52-byte wire frame,
    /// and publishes to the Zenoh key expression.
    ///
    /// # Errors
    /// Returns [`NetworkError::TransportError`] if the publish fails.
    pub async fn broadcast_tx(&self, tx: &Transaction) -> Result<(), NetworkError> {
        let payload = tx.encode_canonical();
        let frame = pack_frame("tx", &payload)?;

        self.session
            .put(TOPIC_TX_GOSSIP, frame)
            .await
            .map_err(|e| NetworkError::TransportError(e.to_string()))?;

        tracing::debug!(
            topic = TOPIC_TX_GOSSIP,
            payload_bytes = payload.len(),
            "Transaction gossip published"
        );

        Ok(())
    }

    /// Subscribe to new block announcements on [`TOPIC_BLOCK_GOSSIP`].
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
            .declare_subscriber(TOPIC_BLOCK_GOSSIP)
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

    /// Subscribe to new transaction announcements on [`TOPIC_TX_GOSSIP`].
    ///
    /// Declares a Zenoh subscriber and returns a pinned async [`Stream`] yielding
    /// decoded [`Transaction`] instances wrapped in wire-frame validation checks.
    ///
    /// # Errors
    /// Returns [`NetworkError::TransportError`] if subscriber declaration fails.
    pub async fn subscribe_txs(
        &self,
    ) -> Result<impl Stream<Item = Result<Transaction, NetworkError>> + Send + Unpin, NetworkError> {
        let subscriber = self
            .session
            .declare_subscriber(TOPIC_TX_GOSSIP)
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
    /// Returns [`NetworkError::CorruptedChecksum`], [`NetworkError::IncompleteFrame`],
    /// or [`NetworkError::SerializationError`] on decode failure.
    pub fn decode_block_frame(raw: &[u8]) -> Result<Block, NetworkError> {
        let (cmd, payload) = unpack_frame(raw)?;
        if cmd != "block" {
            return Err(NetworkError::SerializationError(format!(
                "expected 'block' command, got '{cmd}'"
            )));
        }
        Block::decode_canonical(payload)
            .map_err(|e| NetworkError::SerializationError(e.to_string()))
    }

    /// Decode raw frame bytes into a [`Transaction`] after validating the wire frame.
    ///
    /// # Errors
    /// Returns decode errors on wire frame or canonical codec failure.
    pub fn decode_tx_frame(raw: &[u8]) -> Result<Transaction, NetworkError> {
        let (cmd, payload) = unpack_frame(raw)?;
        if cmd != "tx" {
            return Err(NetworkError::SerializationError(format!(
                "expected 'tx' command, got '{cmd}'"
            )));
        }
        Transaction::decode_canonical(payload)
            .map_err(|e| NetworkError::SerializationError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurion_core::block::BlockHeader;
    use aurion_core::outpoint::OutPoint;
    use aurion_core::tx::{Datum, Transaction, TxInput, TxOutput};
    use aurion_primitives::hash::Hash256;
    use aurion_primitives::quantum::Quantum;

    fn sample_tx() -> Transaction {
        Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_output: OutPoint::new(Hash256::from_bytes([0xAB; 32]), 0),
                unlocking_script: vec![0x51],
                sequence: 0xFFFF_FFFF,
                redeemer: None,
            }],
            outputs: vec![TxOutput {
                value: Quantum::from_raw(1_000_000),
                locking_script: vec![0x51],
                datum: Datum::None,
            }],
            locktime: 0,
        }
    }

    fn sample_block() -> Block {
        let tx = sample_tx();
        let merkle_root = tx.txid();
        Block {
            header: BlockHeader {
                version: 1,
                prev_block_hash: Hash256::ZERO,
                merkle_root,
                timestamp: 1_773_446_400,
                bits: 0x1d00_ffff,
                nonce: 0,
                height: 1,
            },
            transactions: vec![tx],
        }
    }

    #[test]
    fn test_decode_block_frame_roundtrip() {
        let block = sample_block();
        let payload = block.encode_canonical();
        let frame = pack_frame("block", &payload).unwrap();
        let decoded = GossipEngine::decode_block_frame(&frame).unwrap();
        assert_eq!(decoded, block);
    }

    #[test]
    fn test_decode_tx_frame_roundtrip() {
        let tx = sample_tx();
        let payload = tx.encode_canonical();
        let frame = pack_frame("tx", &payload).unwrap();
        let decoded = GossipEngine::decode_tx_frame(&frame).unwrap();
        assert_eq!(decoded, tx);
    }

    #[test]
    fn test_decode_block_frame_rejects_wrong_command() {
        let block = sample_block();
        let payload = block.encode_canonical();
        let frame = pack_frame("tx", &payload).unwrap();
        assert!(matches!(
            GossipEngine::decode_block_frame(&frame).unwrap_err(),
            NetworkError::SerializationError(_)
        ));
    }

    #[test]
    fn test_decode_tx_frame_rejects_corrupted_checksum() {
        let tx = sample_tx();
        let payload = tx.encode_canonical();
        let mut frame = pack_frame("tx", &payload).unwrap();
        let last = frame.len().saturating_sub(1);
        frame[last] ^= 0xFF;
        assert_eq!(
            GossipEngine::decode_tx_frame(&frame).unwrap_err(),
            NetworkError::CorruptedChecksum
        );
    }
}
