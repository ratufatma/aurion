# SPEC-01: Sovereign Single Authority Specification

## 1. Authority Hierarchy Definition
* **L0 - Consensus Core:** Pure stateless crates with zero I/O (`aurion-primitives`, `aurion-core`, `aurion-consensus`, `aurion-script`).
* **L1 - Sovereign Runtime (`aurion-node`):** The sole canonical authority. Single owner of `redb` storage and controller of state transitions.
* **L2 - Clients (`aurion-cli`):** Communicates strictly via IPC/RPC. Direct physical access to storage is strictly prohibited.
* **L3 - Observers (`aurion-indexer`):** Passive observer driven by event streams. Loss or corruption of L3 must never affect L1.

## 2. Sovereignty Invariants
1. No library within the node runtime may maintain an independent lifecycle or autonomous persistent database.
2. Pseudo-fallbacks are strictly prohibited (e.g., `.unwrap_or(0)` on consensus data is forbidden).
