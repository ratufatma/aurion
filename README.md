# Aurion Sovereign Protocol

Aurion is a sovereign pure Rust monorepo protocol (*greenfield rebuild*) with strict sovereignty boundaries (*Single Sovereign Authority*) and zero-float arithmetic guarantees.

## Architecture Hierarchy

- **L0 - Consensus Core:** Pure stateless crates without I/O (`aurion-primitives`, `aurion-core`, `aurion-consensus`, `aurion-script`).
- **L1 - Sovereign Runtime (`aurion-node`):** Single canonical authority, sole owner of `redb` storage and state transitions.
- **L2 - Clients (`aurion-cli`):** Interacts via IPC/RPC only. Physical direct storage access is strictly prohibited.
- **L3 - Observers (`aurion-indexer`):** Passive observer based on event streams.

## Strict Compiler Lints

- `unsafe_code`: **forbid**
- `float_arithmetic`: **deny**
- `arithmetic_side_effects`: **deny**

## Specifications

See the formal constitutional documents in [`docs/`](./docs/):
- [`SPEC-01-SOVEREIGNTY.md`](./docs/SPEC-01-SOVEREIGNTY.md)
- [`SPEC-02-INVARIANTS.md`](./docs/SPEC-02-INVARIANTS.md)
- [`SPEC-03-STORAGE-PROTOCOL.md`](./docs/SPEC-03-STORAGE-PROTOCOL.md)
- [`SPEC-04-FAULT-TAXONOMY.md`](./docs/SPEC-04-FAULT-TAXONOMY.md)
