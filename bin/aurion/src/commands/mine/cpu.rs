#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use aurion_consensus::difficulty::{check_pow, MAX_TARGET_BITS};
use aurion_primitives::hash::Hash256;
use rayon::prelude::*;

pub const BATCH_SIZE_PER_ROUND: u64 = 500_000;

pub async fn start_cpu_miner(threads: usize) -> Result<(), Box<dyn std::error::Error>> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()?;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            r.store(false, Ordering::SeqCst);
        }
    });

    tracing::info!(
        threads = threads,
        batch_size = BATCH_SIZE_PER_ROUND,
        "CPU mining worker pool active. Press Ctrl+C to terminate."
    );

    let mut base_nonce: u64 = 0;
    let start_time = Instant::now();
    let mut total_hashes: u64 = 0;
    let mut last_report = Instant::now();

    while running.load(Ordering::Relaxed) {
        let current_base = base_nonce;
        let batch_size = BATCH_SIZE_PER_ROUND;
        let found = Arc::new(AtomicBool::new(false));
        let found_clone = found.clone();
        let running_clone = running.clone();

        pool.install(|| {
            (0..batch_size).into_par_iter().for_each(|offset| {
                if !running_clone.load(Ordering::Relaxed) || found_clone.load(Ordering::Relaxed) {
                    return;
                }

                let nonce = current_base.saturating_add(offset);
                let mut preimage = [0u8; 40];
                preimage[..32].copy_from_slice(b"aurion-pow-template-header-hash-");
                preimage[32..].copy_from_slice(&nonce.to_be_bytes());
                let hash = Hash256::digest(&preimage);

                if check_pow(&hash, MAX_TARGET_BITS).is_ok() {
                    found_clone.store(true, Ordering::SeqCst);
                    tracing::info!(
                        nonce = nonce,
                        hash = %hash,
                        "Found valid Proof-of-Work block template solution!"
                    );
                }
            });
        });

        base_nonce = base_nonce.saturating_add(batch_size);
        total_hashes = total_hashes.saturating_add(batch_size);

        if last_report.elapsed().as_millis() >= 2_000 {
            let elapsed_ms = start_time.elapsed().as_millis() as u64;
            let kh_per_sec = total_hashes
                .checked_div(1_000)
                .unwrap_or(0)
                .saturating_mul(1_000)
                .checked_div(elapsed_ms.max(1))
                .unwrap_or(0);

            tracing::info!(
                kh_s = kh_per_sec,
                total_hashes = total_hashes,
                threads = threads,
                "CPU miner telemetry"
            );
            last_report = Instant::now();
        }

        tokio::task::yield_now().await;
    }

    tracing::info!("Received shutdown signal. Stopping CPU miner gracefully...");
    Ok(())
}
