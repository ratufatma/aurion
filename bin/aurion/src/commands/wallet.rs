use clap::{Args, Subcommand};

#[derive(Args, Debug)]
pub struct WalletArgs {
    #[command(subcommand)]
    pub command: WalletCommands,
}

#[derive(Subcommand, Debug)]
pub enum WalletCommands {
    /// Generate a new keypair
    New,
    /// Get balance for an address
    Balance {
        address: String,
    },
    /// Send funds
    Send {
        to: String,
        amount: u128,
    },
}

pub async fn run(args: WalletArgs) {
    match args.command {
        WalletCommands::New => {
            tracing::info!("Generating new sovereign keypair...");
        }
        WalletCommands::Balance { address } => {
            tracing::info!(address = %address, "Querying balance via node RPC...");
        }
        WalletCommands::Send { to, amount } => {
            tracing::info!(to = %to, amount = %amount, "Crafting and broadcasting transaction via node RPC...");
        }
    }
}
