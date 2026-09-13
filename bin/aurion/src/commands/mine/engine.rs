//! Mesin pencari nonce Proof-of-Work (dual-engine: wgpu GPU / rayon CPU).
//!
//! `mine_candidate` menemukan nonce sedemikian hingga:
//! `Blake3(encode_canonical(header)) <= target`, lalu mengembalikan header yang
//! sudah terselesaikan. Pencarian berbasis rayon; pipeline WGSL Blake3 untuk GPU
//! masih merupakan area integrasi masa depan, sehingga GPU saat ini hanya
//! dideteksi dan pencarian nonce tetap berjalan di CPU secara deterministik.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use aurion_core::block::BlockHeader;
use aurion_primitives::codec::CanonicalCodec;
use aurion_primitives::hash::Hash256;
use rayon::prelude::*;
use thiserror::Error;

/// Jumlah nonce yang diuji per satu putaran rayon.
pub const BATCH_SIZE_PER_ROUND: u64 = 500_000;

#[derive(Debug, Error)]
pub enum GpuInitError {
    #[error("no compatible GPU adapter found at index {0}")]
    NoAdapterFound(usize),

    #[error("failed to create GPU logical device and queue: {0}")]
    DeviceCreationFailed(String),
}

pub struct GpuContext {
    pub adapter_name: String,
    pub backend: wgpu::Backend,
    // Device/queue dipertahankan hidup selama sesi mining untuk integrasi
    // pipeline WGSL Blake3 di masa depan; belum dibaca pada jalur CPU saat ini.
    #[allow(dead_code)]
    pub device: wgpu::Device,
    #[allow(dead_code)]
    pub queue: wgpu::Queue,
}

pub async fn init_gpu(device_index: usize) -> Result<GpuContext, GpuInitError> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

    let adapters = instance.enumerate_adapters(wgpu::Backends::all());
    if adapters.is_empty() {
        return Err(GpuInitError::NoAdapterFound(device_index));
    }

    let adapter = adapters
        .into_iter()
        .nth(device_index)
        .ok_or(GpuInitError::NoAdapterFound(device_index))?;

    let info = adapter.get_info();

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Aurion GPU Miner Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        )
        .await
        .map_err(|e| GpuInitError::DeviceCreationFailed(e.to_string()))?;

    Ok(GpuContext {
        adapter_name: info.name,
        backend: info.backend,
        device,
        queue,
    })
}

/// Mengenkode target 256-bit menjadi compact `bits` (kebalikan dari
/// `compact_to_target`, viewport kanonik). Selalu round-trip secara eksak
/// terhadap target yang dihasilkan `compact_to_target` (struktur 3-byte block).
pub(crate) fn target_to_bits(target: &[u8; 32]) -> u32 {
    let mut hi = 32usize;
    for (idx, byte) in target.iter().enumerate() {
        if *byte != 0 {
            hi = idx;
            break;
        }
    }

    if hi == 32 {
        return 0;
    }

    let exponent = (32u32).saturating_sub(hi as u32);

    let (m0, m1, m2) = if exponent < 3 {
        (
            byte_at(target, 29),
            byte_at(target, 30),
            byte_at(target, 31),
        )
    } else {
        let start = (32u32.saturating_sub(exponent)) as usize;
        (
            byte_at(target, start),
            byte_at(target, start.saturating_add(1)),
            byte_at(target, start.saturating_add(2)),
        )
    };
    let mut exponent = exponent.max(3);

    let mut mantissa = ((m0 as u32) << 16) | ((m1 as u32) << 8) | (m2 as u32);

    // `compact_to_target` memotong bit ke-23 mantissa; normalisasi eksponen saat
    // mantissa menempati seluruh 24 bit agar encoding kembali eksak.
    if mantissa & 0x0080_0000 != 0 && exponent < 32 {
        exponent = exponent.checked_add(1).unwrap_or(32);
        mantissa >>= 8;
    }

    (exponent << 24) | mantissa
}

fn byte_at(target: &[u8; 32], idx: usize) -> u8 {
    target.get(idx).copied().unwrap_or(0)
}

/// Mencari nonce valid untuk kandidat header melalui pencarian paralel rayon.
///
/// Parameter `target` adalah target 256-bit penuh (dari `BlockTemplate`);
/// `cpu_only` memaksa jalur CPU dan melewati probing GPU.
pub async fn mine_candidate(
    mut header: BlockHeader,
    target: Hash256,
    cpu_only: bool,
) -> Result<BlockHeader, String> {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    if !cpu_only {
        match init_gpu(0).await {
            Ok(ctx) => {
                tracing::info!(
                    adapter = %ctx.adapter_name,
                    backend = ?ctx.backend,
                    "GPU adapter detected; WGSL Blake3 pipeline is a future integration, using CPU nonce search"
                );
            }
            Err(err) => {
                tracing::warn!(error = %err, "GPU unavailable; falling back to multi-threaded CPU mining");
            }
        }
    }

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|e| format!("failed to build CPU miner thread pool: {e}"))?;

    let target_bytes = *target.as_bytes();
    let found = Arc::new(AtomicBool::new(false));
    let total_hashes = Arc::new(AtomicU64::new(0));
    let start_time = Instant::now();
    let mut last_report = Instant::now();
    let mut base_nonce: u64 = 0;

    loop {
        let batch_base = base_nonce;
        let candidate_header = header.clone();
        let tgt = target_bytes;
        let found_flag = Arc::clone(&found);
        let hash_counter = Arc::clone(&total_hashes);

        let found_nonce: Option<u64> = pool.install(|| {
            (0..BATCH_SIZE_PER_ROUND)
                .into_par_iter()
                .find_map_any(|offset| {
                    if found_flag.load(Ordering::Relaxed) {
                        return None;
                    }
                    hash_counter.fetch_add(1, Ordering::Relaxed);

                    let nonce = batch_base.wrapping_add(offset);
                    let mut probe = candidate_header.clone();
                    probe.nonce = nonce;
                    let digest = Hash256::digest(&probe.encode_canonical());

                    if digest.as_bytes() <= &tgt {
                        found_flag.store(true, Ordering::SeqCst);
                        Some(nonce)
                    } else {
                        None
                    }
                })
        });

        if let Some(nonce) = found_nonce {
            header.nonce = nonce;
            let digest = Hash256::digest(&header.encode_canonical());
            tracing::info!(nonce, hash = %digest, "solved candidate Proof-of-Work");
            return Ok(header);
        }

        base_nonce = base_nonce.wrapping_add(BATCH_SIZE_PER_ROUND);

        if last_report.elapsed().as_millis() >= 2_000 {
            let elapsed_ms = start_time.elapsed().as_millis() as u64;
            let total = total_hashes.load(Ordering::Relaxed);
            let kh_s = total
                .checked_div(1_000)
                .unwrap_or(0)
                .saturating_mul(1_000)
                .checked_div(elapsed_ms.max(1))
                .unwrap_or(0);
            tracing::info!(
                kh_s,
                total_hashes = total,
                threads = threads,
                "CPU miner telemetry"
            );
            last_report = Instant::now();
        }

        tokio::task::yield_now().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurion_consensus::difficulty::{check_pow, compact_to_target, MAX_TARGET_BITS};

    #[test]
    fn test_target_to_bits_roundtrip() {
        let target = compact_to_target(MAX_TARGET_BITS).unwrap();
        let bits = target_to_bits(&target);
        let decoded = compact_to_target(bits).unwrap();
        assert_eq!(target, decoded);
    }

    #[test]
    fn test_target_to_bits_tiny_target() {
        let mut tiny = [0u8; 32];
        tiny[31] = 0x5a;
        let bits = target_to_bits(&tiny);
        let decoded = compact_to_target(bits).unwrap();
        assert!(decoded >= tiny);
    }

    #[tokio::test]
    async fn test_mine_candidate_finds_valid_nonce() {
        let target = compact_to_target(MAX_TARGET_BITS).unwrap();
        let header = BlockHeader {
            version: 1,
            prev_block_hash: Hash256::ZERO,
            merkle_root: Hash256::ZERO,
            timestamp: 1_773_446_400,
            bits: MAX_TARGET_BITS,
            nonce: 0,
            height: 1,
        };

        let solved = mine_candidate(header, Hash256::from_bytes(target), true)
            .await
            .unwrap();

        assert!(check_pow(&solved.block_hash(), solved.bits).is_ok());
    }
}