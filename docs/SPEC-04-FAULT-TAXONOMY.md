# SPEC-04: Fault Taxonomy & Fail-Stop Semantics

## 1. Fault Classification Hierarchy

Aurion classifies all protocol faults into three distinct operational domains:

```text
┌─────────────────────────────────────────────────────────────┐
│                      FAULT TAXONOMY                         │
├──────────────────────────┬──────────────────────────────────┤
│ Class 1: Operational     │ Transient boundary events.       │
│          (Non-Fatal)     │ Peer dropped; node continues.    │
├──────────────────────────┼──────────────────────────────────┤
│ Class 2: Consensus       │ Validation rejections.           │
│          (Non-Fatal)     │ Block/tx rejected; tip preserved.│
├──────────────────────────┼──────────────────────────────────┤
│ Class 3: Integrity       │ Internal ledger corruptions.     │
│          (Fatal)         │ Immediate fail-stop lock.        │
└──────────────────────────┴──────────────────────────────────┘
```

---

## 2. Fault Classes & Handling Protocols

### Class 1: Operational Faults (Untrusted Boundary)
- **Examples:**
  - P2P wire framing violation (`MagicBytesMismatch`).
  - Corrupted Blake3 payload checksum.
  - Exceeding maximum packet size ($> 4 \text{ MB}$).
  - Connection timeout or peer disconnection.
- **Handling Protocol:**
  - Non-fatal to the node.
  - Terminate the specific peer connection immediately.
  - Ban peer IP temporarily to mitigate DoS.
  - Ledger state remains untouched.

### Class 2: Consensus Faults (Transaction / Candidate Block Rejection)
- **Examples:**
  - PoW hash does not satisfy target difficulty (`check_pow` failed).
  - Invalid Merkle root in candidate block header.
  - Transaction spending an immature coinbase output ($< 100$ confirmations).
  - Transaction locktime not yet satisfied by MTP.
  - Value conservation check failed ($\sum \text{Inputs} < \sum \text{Outputs} + \text{Fee}$).
  - Ed25519 signature verification failure (`OP_CHECKSIG`).
- **Handling Protocol:**
  - Non-fatal to the node runtime.
  - Reject candidate block or mempool transaction with descriptive error.
  - Do not commit state to `redb`.
  - Canonical tip remains fully preserved.

### Class 3: Integrity Faults (Fatal Fail-Stop)
- **Represented by `NodeFault`:**
  - `StorageCommitFailure(String)`: Atomic `redb::WriteTransaction` failed to persist to disk.
  - `InvariantViolation(String)`: Math or state inconsistency detected in canonical pipeline.
  - `LedgerCorruption(String)`: Missing referenced block, height index gap, or unreadable UTXO record.
  - `ScriptVerificationFailure(String)`: Canonical block signature verification failure.
- **Handling Protocol:**
  - **Immediate Fail-Stop:** Transition runtime state from `NodeState::Running` to `NodeState::Failed(NodeFault)`.
  - Lock all write APIs: Any subsequent mutation call immediately aborts.
  - Terminate background worker tasks (miners, indexer streams).
  - Preserve database file for post-mortem forensics; never attempt speculative self-healing.

---

## 3. Node State Machine Lifecycle

```text
       ┌──────────────┐
       │ Initializing │
       └──────┬───────┘
              │  Genesis verified & storage opened
              ▼
       ┌──────────────┐
       │   Running    │◄──────── (Operational / Consensus faults handled)
       └──────┬───────┘
              │  Integrity fault detected
              ▼
       ┌──────────────┐
       │    Failed    │  (Permanent terminal lock; all mutations rejected)
       └──────────────┘
```

