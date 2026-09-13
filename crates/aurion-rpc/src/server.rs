//! `RpcServer` — listener & dispatcher `tokio::net` serta trait `NodeRpcHandler`
//! yang memisahkan lapisan transport dari basis data konsensus L1.
use super::error::RpcError;
use super::protocol::{RpcRequest, RpcResponse};
use super::template::{BlockTemplate, SubmitResult};
use aurion_core::{Block, Transaction};
use aurion_primitives::{CanonicalCodec, Hash256};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info};

#[async_trait::async_trait]
pub trait NodeRpcHandler: Send + Sync + 'static {
    async fn handle_get_block_template(&self, payout_address: &[u8]) -> Result<BlockTemplate, String>;
    async fn handle_submit_block(&self, block: Block) -> Result<SubmitResult, String>;
    async fn handle_send_raw_tx(&self, tx: Transaction) -> Result<Hash256, String>;
    async fn handle_get_tip_status(&self) -> Result<(u64, Hash256, usize), String>;
}

pub struct RpcServer<H: NodeRpcHandler> {
    handler: Arc<H>,
    bind_addr: String,
}

impl<H: NodeRpcHandler> RpcServer<H> {
    pub fn new(bind_addr: String, handler: Arc<H>) -> Self {
        Self { handler, bind_addr }
    }

    pub async fn run(self) -> Result<(), RpcError> {
        let listener = TcpListener::bind(&self.bind_addr).await?;
        info!(target: "aurion::rpc", addr = %self.bind_addr, "IPC RPC Server aktif mendengarkan...");

        loop {
            let (socket, _) = listener.accept().await?;
            let handler = Arc::clone(&self.handler);

            tokio::spawn(async move {
                if let Err(err) = handle_connection(socket, handler).await {
                    error!(target: "aurion::rpc", error = %err, "Koneksi RPC terputus");
                }
            });
        }
    }
}

async fn handle_connection<H: NodeRpcHandler>(
    mut socket: TcpStream,
    handler: Arc<H>,
) -> Result<(), RpcError> {
    let (mut reader, mut writer) = socket.split();

    loop {
        let frame_bytes = match super::frame::read_frame(&mut reader).await {
            Ok(bytes) => bytes,
            Err(RpcError::Io(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        };

        let request = RpcRequest::decode_canonical(&frame_bytes)
            .map_err(|e| RpcError::Serialization(format!("{e:?}")))?;

        let response = match request {
            RpcRequest::GetBlockTemplate { payout_address } => {
                match handler.handle_get_block_template(&payout_address).await {
                    Ok(tmpl) => RpcResponse::BlockTemplate(tmpl),
                    Err(err) => RpcResponse::Error(err),
                }
            }
            RpcRequest::SubmitBlock { block } => {
                match handler.handle_submit_block(block).await {
                    Ok(res) => RpcResponse::SubmitBlock(res),
                    Err(err) => RpcResponse::Error(err),
                }
            }
            RpcRequest::SendRawTransaction { tx } => {
                match handler.handle_send_raw_tx(tx).await {
                    Ok(txid) => RpcResponse::TransactionAccepted(txid),
                    Err(err) => RpcResponse::Error(err),
                }
            }
            RpcRequest::GetTipStatus => {
                match handler.handle_get_tip_status().await {
                    Ok((height, best_block_hash, mempool_size)) => {
                        RpcResponse::TipStatus { height, best_block_hash, mempool_size }
                    }
                    Err(err) => RpcResponse::Error(err),
                }
            }
        };

        let resp_bytes = response.encode_canonical();
        super::frame::write_frame(&mut writer, &resp_bytes).await?;
    }

    Ok(())
}