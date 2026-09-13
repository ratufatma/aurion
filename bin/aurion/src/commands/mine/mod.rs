#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

pub mod cpu;
pub mod gpu;

use clap::{Args, ValueEnum};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum MiningBackend {
    /// Attempt GPU acceleration first; gracefully fall back to CPU
    #[default]
    Auto,
    /// Force GPU acceleration (fails if no compatible GPU is found)
    Gpu,
    /// Force CPU multi-threaded worker pool
    Cpu,
}

#[derive(Args, Debug, Clone)]
pub struct MineArgs {
    /// Mining execution engine backend
    #[arg(short, long, value_enum, default_value_t = MiningBackend::Auto)]
    pub backend: MiningBackend,

    /// Number of CPU mining threads (defaults to available system cores)
    #[arg(short, long)]
    pub threads: Option<usize>,

    /// GPU device/adapter index
    #[arg(short, long, default_value_t = 0)]
    pub device: usize,

    /// Target payout address
    #[arg(short, long)]
    pub address: Option<String>,
}

pub async fn run(args: MineArgs) -> Result<(), Box<dyn std::error::Error>> {
    let threads = args.threads.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    });

    match args.backend {
        MiningBackend::Auto => {
            tracing::info!("Probing GPU adapters for hardware acceleration...");
            match gpu::init_gpu(args.device).await {
                Ok(gpu_ctx) => {
                    tracing::info!(
                        adapter = %gpu_ctx.adapter_name,
                        backend = ?gpu_ctx.backend,
                        "Compatible GPU adapter detected. Launching GPU compute pipeline..."
                    );
                    gpu::start_gpu_miner(gpu_ctx).await?;
                }
                Err(err) => {
                    tracing::warn!(
                        "GPU unavailable ({err}). Gracefully falling back to multi-threaded CPU mining."
                    );
                    cpu::start_cpu_miner(threads).await?;
                }
            }
        }
        MiningBackend::Gpu => {
            tracing::info!(device = args.device, "Enforcing GPU backend...");
            let gpu_ctx = gpu::init_gpu(args.device).await?;
            tracing::info!(
                adapter = %gpu_ctx.adapter_name,
                backend = ?gpu_ctx.backend,
                "Compatible GPU adapter detected. Launching GPU compute pipeline..."
            );
            gpu::start_gpu_miner(gpu_ctx).await?;
        }
        MiningBackend::Cpu => {
            tracing::info!(threads = threads, "Enforcing CPU backend...");
            cpu::start_cpu_miner(threads).await?;
        }
    }

    Ok(())
}
