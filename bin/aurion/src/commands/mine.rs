use clap::Args;

#[derive(Args, Debug)]
pub struct MineArgs {
    /// Number of mining threads
    #[arg(short, long, default_value_t = 1)]
    pub threads: usize,

    /// Target payout address
    #[arg(short, long)]
    pub address: Option<String>,
}

pub async fn run(args: MineArgs) {
    tracing::info!(threads = args.threads, address = ?args.address, "Starting standalone PoW Blake3 miner...");
}
