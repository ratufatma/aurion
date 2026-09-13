# SPEC-03: Storage & Persistence Protocol

## 1. Exclusive `redb` Authority & Ownership

1. **Sole Custodian:**
   Physical database files (`aurion.redb`) must exclusively be opened and managed by `crates/aurion-storage` under the authority of `crates/aurion-node`. Direct file access by any other process or workspace crate is forbidden.

2. **Zero-Copy & Canonical Serialization:**
   All persistent state written to `redb` must be encoded via `CanonicalCodec`. Trailing bytes or malformed encodings are rejected on deserialization.

---

## 2. Table Definitions & Schema

The storage engine defines four primary tables within the `redb` database:

| Table Name | Key Schema | Value Schema | Description |
| :--- | :--- | :--- | :--- |
| `canonical_blocks` | `&[u8; 32]` (Block Hash) | `&[u8]` (Encoded Block) | Immutable store of all validated blocks. |
| `height_to_hash` | `u64` (Block Height) | `&[u8; 32]` (Block Hash) | Canonical chain index mapping height to block hash. |
| `canonical_utxos` | `&[u8]` (Encoded OutPoint) | `&[u8]` (Encoded TxOut) | Active unspent transaction output state. |
| `canonical_metadata`| `&str` (Metadata Key) | `&[u8]` (Metadata Value) | Node metadata (e.g. `tip_hash`, `tip_height`). |

---

## 3. Single-Commit Atomicity & Durability

1. **Atomic Write Pipeline:**
   When committing block $B$ at height $h$:
   - Insert block into `canonical_blocks`.
   - Update `height_to_hash` index.
   - Evict spent `OutPoint`s from `canonical_utxos`.
   - Insert newly created `ExtendedUtxo`s into `canonical_utxos`.
   - Update `tip_hash` and `tip_height` in `canonical_metadata`.
   All operations execute within a single atomic `redb::WriteTransaction`.

2. **All-or-Nothing Atomicity:**
   If any operation fails during transaction preparation or disk commit, the entire transaction is rolled back with zero disk mutations. Partial or speculative state persistence is prohibited.

3. **Fatal Commit Failure:**
   Any I/O error or failure to commit an atomic write transaction constitutes an unrecoverable *Integrity Fault*. The node immediately transitions to `NodeState::Failed(NodeFault::StorageCommitFailure)`.

