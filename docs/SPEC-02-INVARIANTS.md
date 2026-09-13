# SPEC-02: Ledger Invariants & Mathematical Tripwires

## 1. Mathematical Conservation of Value

1. **Transaction Value Conservation:**
   For every non-coinbase transaction $T$:
   $$\sum_{i \in \text{Inputs}(T)} \text{value}(i) = \sum_{o \in \text{Outputs}(T)} \text{value}(o) + \text{fee}(T)$$
   Where:
   - $\text{fee}(T) \ge 0$
   - All valuations are denominated in `Quantum(u128)`.
   - Overflow, underflow, or negative values are strictly prohibited through checked arithmetic.

2. **Block Subsidy & Inflation Schedule:**
   The block subsidy begins at $50 \times 10^9$ Quantum (50 AUR) and halves every $210{,}000$ blocks:
   $$\text{Subsidy}(h) = \begin{cases} \lfloor \frac{50 \times 10^9}{2^{\lfloor h / 210000 \rfloor}} \rfloor & \text{if } \lfloor h / 210000 \rfloor < 64 \\ 0 & \text{otherwise} \end{cases}$$
   The total coinbase reward for block $B$ at height $h$ cannot exceed:
   $$\text{CoinbaseReward}(B) \le \text{Subsidy}(h) + \sum_{T \in B \setminus \{\text{coinbase}\}} \text{fee}(T)$$

---

## 2. Extended UTXO (eUTXO) Invariants

1. **OutPoint Determinism & Uniqueness:**
   An `OutPoint` is uniquely defined by $(txid, vout)$ where $txid = \text{Blake3}(\text{encode}(T))$ with `unlocking_script`s cleared. Every active UTXO in the ledger is uniquely identified by this tuple.

2. **Coinbase Maturity Threshold:**
   Coinbase outputs cannot be spent until they have achieved at least $100$ confirmations:
   $$\text{Height}(\text{spending\_block}) - \text{Height}(\text{coinbase\_block}) \ge 100$$
   Attempts to spend immature coinbase outputs trigger an immediate rejection.

3. **Median Time Past (MTP) & Locktime Invariants:**
   - Transactions specifying `locktime > 0` are evaluated against the Median Time Past (MTP) of the previous 11 blocks (or block height if $\text{locktime} < 500{,}000{,}000$).
   - Input relative sequence locks enforce BIP-68 relative time-locks against the confirmation height or timestamp of the referenced UTXO.

4. **Double-Spend Prevention (Tripwire):**
   - Each `OutPoint` in the active UTXO set may be spent exactly once.
   - Attempting to spend an unknown, missing, or already-spent `OutPoint` immediately aborts block ingestion.
   - If an internal UTXO set inconsistency is detected, the `AuthorityEngine` trips into terminal `NodeState::Failed`.

---

## 3. Script Execution Invariants (`aurion-script`)

1. **Stack Depth Ceiling:**
   Maximum stack depth cannot exceed $1{,}024$ items. Any opcode that pushes to a full stack triggers `ScriptError::StackOverflow`.

2. **Operation Limit:**
   A script program may execute at most $201$ non-push opcodes. Exceeding this bound triggers `ScriptError::MaxOpcodeCountExceeded`.

3. **Strict Cryptographic Signatures:**
   `OP_CHECKSIG` verifies Ed25519 signatures using `verify_strict`. Signatures with non-canonical encodings, small order public keys, or invalid scalar components are unconditionally rejected.

4. **Deterministic Preimage Hashing:**
   `sighash` computes the Blake3 digest of the canonical serialization of the transaction with all `unlocking_script`s set to empty slices. This eliminates circular signature dependencies and guarantees malleability resistance.

