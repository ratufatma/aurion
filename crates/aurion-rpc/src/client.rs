//! `RpcClient` tipis untuk `aurion mine` dan `aurion wallet`: setiap panggilan
//! membuka satu koneksi, menulis satu frame permintaan, membaca satu frame
//! respons, lalu menutup koneksi (sessionless).
use super::error::RpcError;
use super::protocol::{RpcRequest, RpcResponse};
use super::template::{BlockTemplate, SubmitResult};
use aurion_core::Block;
use aurion_primitives::CanonicalCodec;
use tokio::net::TcpStream;

pub struct RpcClient {
    endpoint: String,
}

impl RpcClient {
    pub fn new(endpoint: String) -> Self {
        Self { endpoint }
    }

    pub async fn get_block_template(&self, payout_address: &[u8]) -> Result<BlockTemplate, RpcError> {
        let req = RpcRequest::GetBlockTemplate {
            payout_address: payout_address.to_vec(),
        };
        match self.call(req).await? {
            RpcResponse::BlockTemplate(tmpl) => Ok(tmpl),
            RpcResponse::Error(err) => Err(RpcError::NodeRejected(err)),
            _ => Err(RpcError::ProtocolMismatch),
        }
    }

    pub async fn submit_block(&self, block: Block) -> Result<SubmitResult, RpcError> {
        let req = RpcRequest::SubmitBlock { block };
        match self.call(req).await? {
            RpcResponse::SubmitBlock(res) => Ok(res),
            RpcResponse::Error(err) => Err(RpcError::NodeRejected(err)),
            _ => Err(RpcError::ProtocolMismatch),
        }
    }

    async fn call(&self, request: RpcRequest) -> Result<RpcResponse, RpcError> {
        let mut stream = TcpStream::connect(&self.endpoint)
            .await
            .map_err(|e| RpcError::ConnectionFailed(e.to_string()))?;
        let (mut reader, mut writer) = stream.split();

        let req_bytes = request.encode_canonical();
        super::frame::write_frame(&mut writer, &req_bytes).await?;

        let resp_bytes = super::frame::read_frame(&mut reader).await?;
        let response = RpcResponse::decode_canonical(&resp_bytes)
            .map_err(|e| RpcError::Serialization(format!("{e:?}")))?;

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{NodeRpcHandler, RpcServer};
    use aurion_core::{BlockHeader, Transaction};
    use aurion_primitives::{Hash256, Quantum};
    use std::sync::Arc;
    use tokio::net::TcpListener;

    struct TestNode {
        reject_submits: bool,
    }

    #[async_trait::async_trait]
    impl NodeRpcHandler for TestNode {
        async fn handle_get_block_template(
            &self,
            payout_address: &[u8],
        ) -> Result<BlockTemplate, String> {
            assert_eq!(payout_address, b"aurion-test-addr");
            Ok(BlockTemplate {
                height: 7,
                previous_block_hash: Hash256::ZERO,
                target: Hash256::digest(b"target"),
                timestamp: 1_773_446_400,
                coinbase_subsidy: Quantum::from_raw(99_000_000),
                total_fee: Quantum::from_raw(2_300),
                transactions: vec![],
            })
        }

        async fn handle_submit_block(&self, _block: Block) -> Result<SubmitResult, String> {
            if self.reject_submits {
                Err("subsidy parity mismatch".into())
            } else {
                Ok(SubmitResult::Accepted {
                    block_hash: Hash256::digest(b"mined"),
                    height: 7,
                })
            }
        }

        async fn handle_send_raw_tx(&self, _tx: Transaction) -> Result<Hash256, String> {
            Ok(Hash256::digest(b"test-txid"))
        }

        async fn handle_get_tip_status(&self) -> Result<(u64, Hash256, usize), String> {
            Ok((7, Hash256::digest(b"tip"), 3))
        }
    }

    fn sample_block() -> Block {
        let header = BlockHeader {
            version: 1,
            prev_block_hash: Hash256::ZERO,
            merkle_root: Hash256::ZERO,
            timestamp: 1_773_446_400,
            bits: 0x1f00ffff,
            nonce: 42,
            height: 7,
        };
        Block::new(header, vec![])
    }

    async fn spawn_test_server(reject_submits: bool) -> RpcClient {
        let probe = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = probe.local_addr().unwrap();
        drop(probe);

        let handler = Arc::new(TestNode { reject_submits });
        let server = RpcServer::new(addr.to_string(), handler);
        tokio::spawn(async move {
            let _ = server.run().await;
        });

        RpcClient::new(addr.to_string())
    }

    #[tokio::test]
    async fn test_get_block_template() {
        let client = spawn_test_server(false).await;
        let template = client.get_block_template(b"aurion-test-addr").await.unwrap();

        assert_eq!(template.height, 7);
        assert_eq!(template.coinbase_subsidy, Quantum::from_raw(99_000_000));
        assert_eq!(template.total_fee, Quantum::from_raw(2_300));
    }

    #[tokio::test]
    async fn test_submit_block_accepted() {
        let client = spawn_test_server(false).await;
        let result = client.submit_block(sample_block()).await.unwrap();

        assert_eq!(
            result,
            SubmitResult::Accepted {
                block_hash: Hash256::digest(b"mined"),
                height: 7,
            }
        );
    }

    #[tokio::test]
    async fn test_submit_block_rejected() {
        let client = spawn_test_server(true).await;
        let err = client.submit_block(sample_block()).await.unwrap_err();

        assert!(matches!(err, RpcError::NodeRejected(_)));
    }

    #[tokio::test]
    async fn test_send_raw_tx_and_tip_status() {
        let client = spawn_test_server(false).await;

        let txid = match client
            .call(RpcRequest::SendRawTransaction {
                tx: Transaction {
                    version: 1,
                    inputs: vec![],
                    outputs: vec![],
                    locktime: 0,
                },
            })
            .await
            .unwrap()
        {
            RpcResponse::TransactionAccepted(txid) => txid,
            other => panic!("unexpected response: {other:?}"),
        };
        assert_eq!(txid, Hash256::digest(b"test-txid"));

        let tip = match client.call(RpcRequest::GetTipStatus).await.unwrap() {
            RpcResponse::TipStatus {
                height,
                best_block_hash,
                mempool_size,
            } => (height, best_block_hash, mempool_size),
            other => panic!("unexpected response: {other:?}"),
        };
        assert_eq!(tip, (7, Hash256::digest(b"tip"), 3));
    }

    #[tokio::test]
    async fn test_client_connection_refused() {
        let client = RpcClient::new("127.0.0.1:1".into());
        let err = client.call(RpcRequest::GetTipStatus).await.unwrap_err();
        assert!(matches!(err, RpcError::ConnectionFailed(_)));
    }
}