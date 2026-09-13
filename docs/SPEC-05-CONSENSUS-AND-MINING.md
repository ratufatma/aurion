# SPEC-05: Proof-of-Work Consensus & Dual-Engine Mining Specification

## 1. Consensus Proof-of-Work Mathematics

### A. Blake3 PoW Hashing
The Aurion consensus mechanism uses **Blake3** as its Proof-of-Work hashing primitive. The block hash is defined as:
$$\text{BlockHash} = \text{Blake3}(\text{CanonicalEncode}(\text{BlockHeader}))$$

Where the `BlockHeader` consists of:
- `version: u32` (Protocol version, currently `1`)
- `prev_block_hash: Hash256` (Blake3 hash of the parent block)
- `merkle_root: Hash256` (Blake3 binary Merkle root of all block transactions)
- `timestamp: u64` (Unix epoch timestamp in seconds)
- `bits: u32` (Compact representation of the target difficulty)
- `nonce: u64` (64-bit integer search space for miners)
- `height: u64` (Canonical block height)

### B. Compact Bits & Target Expansion
Difficulty is encoded in a 32-bit compact format (`bits`):
- High 8 bits: Exponent $E = \text{bits} \gg 24$
- Low 24 bits: Mantissa $M = \text{bits} \ \& \ \text{0x00FFFFFF}$

The expanded 256-bit target $T$ is computed deterministically without floating-point arithmetic:
1. Initialize a 32-byte target buffer to zeros: $T = [0\text{u8}; 32]$.
2. If $E = 0$, return $T$ (impossible target).
3. Shift index: $S = 32 - E$. If $S + 3 > 32$, reject as `InvalidCompactBits`.
4. Place the mantissa bytes into $T[S..S+3]$.
5. A block header satisfies the PoW condition if and only if:
$$\text{BlockHash} \le T$$

The maximum allowable target (minimum difficulty) is defined as:
$$\text{MAX\_TARGET\_BITS} = \text{0x207FFFFF}$$

---

## 2. Monetary Invariants & Emission Schedule

Aurion enforces a mathematical hard cap of **66,000,000 AUR** ($6{,}600{,}000{,}000{,}000{,}000\text{ Quanta}$) governed by:

```text
Genesis Allocation (Height 0, 40%):
  ├── Creator Allocation (30%): 19,800,000 AUR (1,980,000,000,000,000 Quanta)
  └── Dev Fund Allocation (10%): 6,600,000 AUR (660,000,000,000,000 Quanta)
PoW Mining Emission (Height > 0, 60%):
  ├── Initial Block Subsidy (S_0): 99 AUR (9,900,000,000 Quanta)
  ├── Halving Interval (I): 200,000 Blocks
  ├── Maximum Halvings: 64
  └── Total PoW Target: 39,600,000 AUR (3,960,000,000,000,000 Quanta)
```

The block subsidy at height $h > 0$ is evaluated deterministically as:
$$\text{Subsidy}(h) = \begin{cases} \text{INITIAL\_SUBSIDY\_QUANTA} \gg \lfloor \frac{h}{200000} \rfloor & \text{if } \lfloor \frac{h}{200000} \rfloor < 64 \\ 0 & \text{otherwise} \end{cases}$$

---

## 3. Dual-Engine Mining Architecture

The standalone miner subsystem in `bin/aurion/src/commands/mine/` features **Dynamic Hardware Negotiation**, enabling automated hardware detection and zero-downtime fallback.

```text
                             ┌───────────────────────┐
                             │    aurion mine run    │
                             └───────────┬───────────┘
                                         │
                         Check --backend argument
                                         │
                 ┌───────────────────────┼───────────────────────┐
                 │                       │                       │
           backend: Auto           backend: Gpu            backend: Cpu
                 │                       │                       │
                 ▼                       ▼                       ▼
      ┌─────────────────────┐ ┌─────────────────────┐ ┌─────────────────────┐
      │ Probe wgpu Adapters │ │ Probe wgpu Adapters │ │ Spawn Rayon Worker  │
      └──────────┬──────────┘ └──────────┬──────────┘ │ Pool (thread count) │
                 │                       │            └──────────┬──────────┘
        Adapter Found?          Adapter Found?                   │
        ┌────────┴────────┐     ┌────────┴────────┐              │
       Yes                No   Yes                No             │
        │                  │    │                  │             │
        ▼                  ▼    ▼                  ▼             │
 ┌─────────────┐   ┌───────────────┐        ┌─────────────┐      │
 │ Launch WGSL │   │ Log Warning   │        │ Terminate   │      │
 │ GPU Compute │   │ Fallback to   │        │ with Error  │      │
 │ Pipeline    │   │ CPU Pool      │        │ (Exit 1)    │      │
 └─────────────┘   └───────┬───────┘        └─────────────┘      │
                           │                                     │
                           └─────────────────────────────────────┘
                                           │
                                           ▼
                            ┌─────────────────────────────┐
                            │ Parallel Nonce Batch Search │
                            │ (500,000 nonces / round)    │
                            └─────────────────────────────┘
```

### A. Architectural Mandates
1. **Zero-Unsafe & Pure Rust:**
   - Zero C/C++ OpenCL or CUDA bindings.
   - GPU abstraction utilizes the pure-Rust, memory-safe `wgpu` library targeting Vulkan, DirectX 12, Metal, and native compute backends.
2. **Deterministic Fallback Philosophy:**
   - In `Auto` mode: If GPU probing fails (e.g., headless VPS, missing graphic drivers, or invalid adapter index), the system logs a structured warning and seamlessly shifts execution to CPU Rayon worker pools without panicking or terminating.
   - In `Gpu` mode: If GPU probing fails, execution terminates cleanly with an explicit error.
   - In `Cpu` mode: GPU discovery is skipped entirely and CPU worker pools spawn immediately.

### B. WGSL Compute Pipeline (`gpu.rs`)
- Compiles a native WebGPU Shading Language (WGSL) compute shader dispatched via `wgpu::ComputePipeline`.
- Allocates storage buffers for parallel nonce candidate generation across GPU shader execution units.
- Submits command buffers through `wgpu::Queue` and monitors cancellation signals asynchronously via `tokio::select!`.

### C. Multi-Threaded Rayon Worker Pool (`cpu.rs`)
- Configures a dedicated Rayon thread pool (`rayon::ThreadPoolBuilder::new().num_threads(threads)`).
- Partitions the search space into discrete batches of $500{,}000\text{ nonces}$ (`BATCH_SIZE_PER_ROUND`).
- Nonce hashing is performed in parallel across threads:
  - Preimage buffer: `[0u8; 40]` containing `template_header_hash (32 bytes)` + `nonce.to_be_bytes() (8 bytes)`.
  - Hashing: `Hash256::digest(&preimage)`.
  - Condition check: `check_pow(&hash, MAX_TARGET_BITS)`.
- Coordination across worker threads uses `std::sync::atomic::AtomicBool` for zero-allocation cancellation.
- Hashrate telemetry is computed using checked integer arithmetic:
  $$\text{kH/s} = \frac{\lfloor \text{total\_hashes} / 1000 \rfloor \times 1000}{\max(1, \text{elapsed\_ms})}$$
  Ensuring **zero floating-point operations** while providing accurate telemetry.
