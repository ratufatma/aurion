#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

mod authority;
mod lifecycle;
mod network;
mod rpc;
mod storage;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Aurion Sovereign Node (L1 Runtime)...");
}
