#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

mod commands;

use clap::{Parser, Subcommand};
use commands::{indexer::IndexerArgs, mine::MineArgs, node::NodeArgs, wallet::WalletArgs};

#[derive(Parser, Debug)]
#[command(
    name = "aurion",
    about = "Aurion Sovereign Digital Asset Protocol — Unified CLI & Node Runtime",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the L1 Sovereign node daemon
    Node(NodeArgs),
    /// Manage sovereign wallets and transactions
    Wallet(WalletArgs),
    /// Run the standalone PoW Blake3 miner
    Mine(MineArgs),
    /// Run the secondary SQLite projection indexer worker
    Indexer(IndexerArgs),
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Node(args) => commands::node::run(args).await,
        Commands::Wallet(args) => commands::wallet::run(args).await,
        Commands::Mine(args) => {
            if let Err(err) = commands::mine::run(args).await {
                tracing::error!("Mining terminated with error: {err}");
                std::process::exit(1);
            }
        }
        Commands::Indexer(args) => commands::indexer::run(args).await,
    }
}
