# SPEC-01: Sovereign Single Authority Specification

## 1. Authority Hierarchy Definition

The Aurion system is partitioned into four strict constitutional authority layers:

* **L0 — Pure Consensus Core (Stateless, Zero-I/O):**
  - Crates: `aurion-primitives`, `aurion-core`, `aurion-consensus`, `aurion-script`, `aurion-eutxo`.
  - Characteristics: Stateless, deterministic mathematical domain libraries with zero disk, network, or OS clock I/O.
  - Invariant: May never import higher layers (L1, L2, L3) or persistent database drivers.

* **L1 — Sovereign Runtime & Custody:**
  - Crates: `aurion-storage`, `aurion-network`, `aurion-node`.
  - Characteristics: The sole canonical authority of the Aurion protocol. Exclusive owner and custodian of physical persistent storage (`redb`) and state machine transitions.
  - Invariant: Single-commit atomic transaction boundary. Any state mutation or invariant violation locks the node into terminal `NodeState::Failed`.

* **L2 — Client Interfaces & Protocols:**
  - Crates: `aurion-rpc`, `bin/aurion` (`wallet`, `mine`).
  - Characteristics: Thin client tools and IPC/RPC interfaces.
  - Invariant: **Strict Storage Isolation**. Client modules must **NEVER** import `aurion-storage` or open physical database handles directly. All ledger interactions must be routed through authenticated IPC/RPC to the running L1 node daemon.

* **L3 — Derived Observers & Projections:**
  - Crates: `aurion-indexer`, `bin/aurion` (`indexer`).
  - Characteristics: Passive secondary projection engine (SQLite) optimized for complex queries, history analysis, and client APIs.
  - Invariant: L3 is strictly a downstream consumer of canonical block events. Corruption, crash, or lag in L3 must never impede or corrupt L1 node consensus.

---

## 2. Sovereignty & Architectural Invariants

1. **Single Persistent Custodian:**
   Only the L1 node runtime (`aurion node`) has permission to open, read, or write the canonical `redb` database. No other workspace member or executable subcommand may link or access storage files directly.

2. **Zero Unsafe Code:**
   Every workspace member enforces `#![forbid(unsafe_code)]`. Memory safety is statically guaranteed across all crates.

3. **Zero-Float Consensus Accounting:**
   Floating-point types (`f32`, `f64`) are strictly denied via Clippy lints across all workspace targets. All asset amounts use `Quantum(u128)`.

4. **No Pseudo-Fallbacks:**
   Consensus data must never be defaulted or coerced using fallbacks (e.g., `.unwrap_or(0)` or silent clamping on corrupted ledger entries is strictly prohibited). Missing or invalid consensus data constitutes an integrity fault.

5. **Fail-Stop Terminality:**
   When an integrity fault or invariant violation is detected, `AuthorityEngine` transitions to `NodeState::Failed(NodeFault)`. Once entered, this state is irrevocable and all mutation operations are permanently locked until manual operator intervention.

