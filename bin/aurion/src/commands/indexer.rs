use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct IndexerArgs {
    /// Path to SQLite database file
    #[arg(short, long, default_value = "./indexer.db")]
    pub db_path: PathBuf,
}

pub async fn run(args: IndexerArgs) {
    tracing::info!(db_path = ?args.db_path, "Starting Aurion SQLite projection indexer worker...");
}
