# Aurion Sovereign Protocol: Constitutional Specifications & Conceptual Blueprint

Welcome to the canonical specification and systemic documentation suite for the **Aurion Sovereign Digital Asset Protocol**.

This directory contains the formal architectural specifications that govern the consensus, ledger physics, storage integrity, network framing, and cryptographic boundaries of the Aurion network.

---

## 1. Systemic Conceptual Blueprint

- [**System Architecture & Conceptual Blueprint (`ARCHITECTURE.md`)**](ARCHITECTURE.md)
  - Comprehensive architectural overview of the Aurion Protocol.
  - The 4 constitutional layers (L0 Stateless Core, L1 Sovereign Authority, L2 Clients, L3 Projections).
  - End-to-end canonical transaction and block lifecycle sequence diagrams.
  - Defensive engineering philosophy and fail-stop state semantics.

---

## 2. Formal Protocol Specifications

The Aurion Protocol is formally specified across six foundational specifications:

| Specification Document | Domain & Focus Area | Key Concepts & Invariants |
| :--- | :--- | :--- |
| [**SPEC-01: Sovereign Single Authority**](SPEC-01-SOVEREIGNTY.md) | Architectural Hierarchy & Layer Boundaries | L0–L3 authority model, single physical database custodian (`redb`), strict storage isolation for client tools (`wallet`, `mine`), `#![forbid(unsafe_code)]`. |
| [**SPEC-02: Ledger Invariants & Mathematical Tripwires**](SPEC-02-INVARIANTS.md) | Ledger Physics & Monetary Invariants | Value conservation, 66,000,000 AUR mathematical hard cap, 99 AUR halving schedule (200k blocks), 100-block coinbase maturity, MTP/BIP-68 locktimes, Turing-incomplete script boundaries. |
| [**SPEC-03: Storage & Persistence Protocol**](SPEC-03-STORAGE-PROTOCOL.md) | Database Integrity & Atomicity | Exclusive `redb` custody, 4 canonical table schemas, single-transaction atomic block commits (`all-or-nothing`), zero-copy canonical serialization. |
| [**SPEC-04: Fault Taxonomy & Fail-Stop Semantics**](SPEC-04-FAULT-TAXONOMY.md) | Runtime Fault Classification & Lifecycle | Class 1 Operational Faults (non-fatal), Class 2 Consensus Faults (candidate rejection), Class 3 Integrity Faults (fatal fail-stop lock to `NodeState::Failed`). |
| [**SPEC-05: Proof-of-Work & Dual-Engine Mining**](SPEC-05-CONSENSUS-AND-MINING.md) | Consensus Hashing & Hardware Negotiation | Blake3 PoW mathematical formulation, compact bits target calculation, dual-engine hardware negotiation (safe pure-Rust `wgpu` WGSL compute with deterministic Rayon CPU fallback), zero-float telemetry. |
| [**SPEC-06: P2P Wire Protocol & Network Boundary**](SPEC-06-P2P-NETWORK-PROTOCOL.md) | Wire Framing & Untrusted Network Boundary | Fixed 52-byte binary header framing, pre-allocation Blake3 payload checksums, 4 MB anti-DoS allocation ceiling, canonical message taxonomy, peer session lifecycle. |

---

## 3. Foundational Protocol Axioms

All implementations within the Aurion ecosystem must strictly adhere to four immutable constitutional axioms:

1. **Pure Mathematical Determinism (L0):**
   Ledger rules are stateless mathematical functions. They consume binary byte slices and return binary verdicts. No ambient I/O, system clock reads, or thread concurrency are permitted in Layer 0.

2. **Single Sovereign Custody (L1):**
   Only the L1 node daemon owns, opens, or mutates the physical `redb` database. Client applications communicate strictly over IPC / JSON-RPC.

3. **Zero-Float Accounting:**
   Floating-point types (`f32`, `f64`) are banned across the workspace. All economic valuations are represented as exact integers in `Quantum` ($10^{-8}\text{ AUR}$, $u128$).

4. **Fail-Stop Invariant Protection:**
   The protocol never attempts speculative recovery upon encountering state corruption or invariant violations. It immediately transitions to a permanent, terminal fail-stop state to protect financial history.
