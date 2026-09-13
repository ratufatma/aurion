# Aurion Sovereign Protocol

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](Cargo.toml)
[![Rust](https://img.shields.io/badge/rust-2021%20edition-orange.svg)](Cargo.toml)
[![Safety](https://img.shields.io/badge/unsafe-forbid-brightgreen.svg)](Cargo.toml)
[![Arithmetic](https://img.shields.io/badge/floats-denied-red.svg)](Cargo.toml)
[![Tests](https://img.shields.io/badge/tests-35%20passed-success.svg)](tests/)

Aurion is a sovereign, pure-Rust cryptocurrency protocol engineered with strict sovereignty boundaries (**Single Sovereign Authority**), zero-float arithmetic guarantees, and deterministic fail-stop execution semantics.

The repository is organized as an industrial **Modular Monorepo with a Unified Single-Binary Target** (`bin/aurion` + `crates/*`).

---

## Architectural Hierarchy

Aurion decomposes protocol logic into four distinct constitutional authority layers:

```text
aurion/
├── Cargo.toml                      # Workspace root (resolver = "2", shared lints, members)
├── GEMINI.md                       # Protocol systems engineering directives
├── docs/                           # Constitutional specifications (SPEC-01 to SPEC-04)
├── crates/                         # Pure domain library crates
│   ├── aurion-primitives/          # L0: Hash256 (Blake3), Quantum(u128), CanonicalCodec
│   ├── aurion-core/                # L0: OutPoint, Transaction, sighash, Merkle, Block
│   ├── aurion-consensus/           # L0: PoW check_pow, Halving subsidy, Genesis, Block verification
│   ├── aurion-script/              # L0: Stack VM, OpCodes, Ed25519 verify_strict, Blake3 hashing
│   ├── aurion-eutxo/               # L0: Coinbase maturity, Locktime MTP/BIP-68, Fee engine, State
│   ├── aurion-storage/             # L1: Atomic redb storage engine (blocks, utxos, metadata)
│   ├── aurion-network/             # L1: Wire framing (52-byte header), Blake3 checksum, anti-DoS
│   ├── aurion-node/                # L1: Node lifecycle, Authority engine, Fail-stop tripwires
│   ├── aurion-rpc/                 # L2: IPC / JSON-RPC protocol schemas and DTOs
│   └── aurion-indexer/             # L3: Relational SQLite projection library (read-only observer)
└── bin/
    └── aurion/                     # UNIFIED EXECUTABLE TARGET (Daemon & CLI)
        ├── Cargo.toml
        └── src/
            ├── main.rs             # Clap CLI router & Tokio async runtime entrypoint
            └── commands/           # Command handlers: node, wallet, mine, indexer
```

### Layer Authority Model

- **L0 — Consensus Core (Pure Stateless Libraries, Zero I/O):**
  - [`crates/aurion-primitives`](crates/aurion-primitives/): Foundational types (`Hash256`, zero-float `Quantum(u128)`), and canonical byte codec with trailing byte rejection.
  - [`crates/aurion-core`](crates/aurion-core/): Canonical ledger data structures (`Block`, `Transaction`, `OutPoint`, `compute_merkle_root`), and deterministic preimage `sighash`.
  - [`crates/aurion-consensus`](crates/aurion-consensus/): State transition rules, deterministic Blake3 Proof-of-Work (`check_pow`), subsidy halving schedule, and Genesis definition.
  - [`crates/aurion-script`](crates/aurion-script/): Isolated stack-based contract VM (15 opcodes, 1024 stack ceiling, 201 ops limit, Ed25519 `verify_strict`, Blake3).
  - [`crates/aurion-eutxo`](crates/aurion-eutxo/): Pure eUTXO rules engine enforcing 100-block coinbase maturity, MTP/BIP-68 sequence locktimes, and fee policies.

- **L1 — Sovereign Runtime & Physical Engines:**
  - [`crates/aurion-storage`](crates/aurion-storage/): Sole custodian of physical persistent storage (`redb`), providing single-transaction atomic commits across blocks, height indices, and UTXO deltas.
  - [`crates/aurion-network`](crates/aurion-network/): P2P wire protocol framing with fixed 52-byte headers, Blake3 payload checksum verification, and strict 4 MB anti-DoS allocation ceilings.
  - [`crates/aurion-node`](crates/aurion-node/): Runtime state machine and `AuthorityEngine`. Enforces ledger invariants and immediate fail-stop locking upon detecting data corruption or invariant breach.

- **L2 — Interface Protocols & Clients:**
  - [`crates/aurion-rpc`](crates/aurion-rpc/): Type-safe IPC / RPC schemas and data transfer objects connecting thin clients to the node daemon.
  - [`bin/aurion`](bin/aurion/): The sole binary executable target providing unified CLI subcommands (`node`, `wallet`, `mine`, `indexer`).

- **L3 — Observers & Projections:**
  - [`crates/aurion-indexer`](crates/aurion-indexer/): Secondary relational projection service (SQLite) for queries and analytics. Operates purely as an asynchronous, read-only observer. Corruption or failure of L3 never cascades to L1 consensus.

---

## Strict Compiler Guarantees

Aurion enforces uncompromising security and safety invariants at the workspace compiler level:

```toml
[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
float_arithmetic = "deny"
arithmetic_side_effects = "deny"
unwrap_used = "warn"
expect_used = "warn"
```

1. **`#![forbid(unsafe_code)]`:** Zero `unsafe` blocks are permitted anywhere across the workspace.
2. **Zero-Float Arithmetic:** IEEE 754 floating-point operations (`f32`, `f64`) are strictly denied. All financial calculations use `Quantum(u128)` with checked arithmetic to guarantee identical consensus execution across heterogeneous architectures.
3. **Data Sovereignty (SPEC-01 & SPEC-03):** Only `aurion node` holds write/read access to physical `redb` storage. Client commands (`wallet`, `mine`) operate as thin clients over IPC/RPC.
4. **Fail-Stop Tripwires:** Value conservation violations, double-spend attempts, or storage commit errors immediately transition the node to terminal `NodeState::Failed(NodeFault)`.

---

## Formal Specifications & Architecture

Protocol specifications and conceptual architecture are formally documented in [`docs/`](docs/):

- [**System Architecture & Conceptual Blueprint**](docs/ARCHITECTURE.md) — Protocol topology, 4-layer hierarchy, and end-to-end transaction lifecycle.
- [**SPEC-01: Sovereign Single Authority Specification**](docs/SPEC-01-SOVEREIGNTY.md) — Architectural hierarchy, single authority principle, and data access boundaries.
- [**SPEC-02: Ledger Invariants & Mathematical Tripwires**](docs/SPEC-02-INVARIANTS.md) — Value conservation, 66M Hard Cap, coinbase maturity, locktime semantics, and script invariants.
- [**SPEC-03: Storage & Persistence Protocol**](docs/SPEC-03-STORAGE-PROTOCOL.md) — Atomicity invariants, `redb` schema, and fail-stop persistence semantics.
- [**SPEC-04: Fault Taxonomy & Fail-Stop Semantics**](docs/SPEC-04-FAULT-TAXONOMY.md) — Operational vs. Integrity faults and fail-stop state machine lifecycle.
- [**SPEC-05: Proof-of-Work & Dual-Engine Mining**](docs/SPEC-05-CONSENSUS-AND-MINING.md) — Blake3 PoW math, compact bits target calculation, safe `wgpu` GPU / Rayon CPU fallback.
- [**SPEC-06: P2P Wire Protocol & Network Boundary**](docs/SPEC-06-P2P-NETWORK-PROTOCOL.md) — Binary 52-byte header framing, Blake3 payload checksum, 4 MB anti-DoS ceiling.

---

## Unified Binary CLI (`bin/aurion`)

The Aurion workspace builds into a single executable `aurion` that serves as both the node daemon and the primary operator CLI:

```bash
Aurion Sovereign Digital Asset Protocol — Unified CLI & Node Runtime

Usage: aurion <COMMAND>

Commands:
  node     Run the L1 Sovereign node daemon
  wallet   Manage sovereign wallets and transactions
  mine     Run the standalone PoW Blake3 miner
  indexer  Run the secondary SQLite projection indexer worker
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### Subcommand Examples

- **Start Node Daemon:**
  ```bash
  cargo run -p aurion -- node --datadir ./data --port 8333
  ```
- **Manage Wallets:**
  ```bash
  cargo run -p aurion -- wallet new
  cargo run -p aurion -- wallet balance --address <HEX_ADDRESS>
  ```
- **Run PoW Dual-Engine Miner:**
  ```bash
  # Auto-detect GPU with graceful CPU fallback
  cargo run -p aurion -- mine --backend auto --threads 4

  # Force multi-threaded CPU mining
  cargo run -p aurion -- mine --backend cpu --threads 8

  # Force GPU acceleration on device index 0
  cargo run -p aurion -- mine --backend gpu --device 0
  ```
- **Start Projection Indexer:**
  ```bash
  cargo run -p aurion -- indexer --db-path ./indexer.sqlite
  ```

---

## Getting Started

### Prerequisites
- **Rust Toolchain:** 1.80+ (stable)
- **Cargo**

### Building the Workspace
```bash
cargo build --workspace
```

### Static Analysis & Linters
```bash
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets
```

### Running the Test Suite
Aurion features a comprehensive suite of **35 unit and integration tests** validating cryptographic primitives, canonical codecs, script execution, eUTXO rules, atomic storage, wire framing, the 66M hard cap monetary schedule, and end-to-end chain lifecycles:

```bash
cargo test --workspace
```

---

## License

Dual-licensed under either of:
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)

