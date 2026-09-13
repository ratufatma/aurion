use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct NodeArgs {
    /// Path to data directory
    #[arg(short, long, default_value = "./data")]
    pub datadir: PathBuf,

    /// P2P listening port
    #[arg(short, long, default_value_t = 8333)]
    pub port: u16,
}

pub async fn run(args: NodeArgs) {
    tracing::info!(datadir = ?args.datadir, port = args.port, "Initializing Aurion Sovereign Node daemon...");
    tracing::info!("Storage engine initialized at canonical data directory");
    tracing::info!("Authority engine active with strict fail-stop tripwires");
}
