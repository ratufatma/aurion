# SPEC-02: Ledger Invariants & Mathematical Tripwires

## 1. Mathematical Conservation of Value

1. **Transaction Value Conservation:**
   For every non-coinbase transaction $T$:
   $$\sum_{i \in \text{Inputs}(T)} \text{value}(i) = \sum_{o \in \text{Outputs}(T)} \text{value}(o) + \text{fee}(T)$$
   Where:
   - $\text{fee}(T) \ge 0$
   - All valuations are denominated in `Quantum(u128)`.
   - Overflow, underflow, or negative values are strictly prohibited through checked arithmetic.

2. **Monetary Hard Cap & Emission Schedule (66,000,000 AUR):**
   - **Scale & Unit:** $1 \text{ AUR} = 100{,}000{,}000 \text{ Quanta}$ (8 decimals).
   - **Hard Cap:** $\text{MAX\_TOTAL\_SUPPLY} = 66{,}000{,}000 \text{ AUR} = 6{,}600{,}000{,}000{,}000{,}000 \text{ Quanta}$.
   - **Genesis Premine Allocations (Height 0, 40%):**
     - Creator Allocation (30%): $19{,}800{,}000 \text{ AUR} = 1{,}980{,}000{,}000{,}000{,}000 \text{ Quanta}$ (Output 0).
     - Developer Fund (10%): $6{,}600{,}000 \text{ AUR} = 660{,}000{,}000{,}000{,}000 \text{ Quanta}$ (Output 1).
     - Total Genesis Premine: $26{,}400{,}000 \text{ AUR} = 2{,}640{,}000{,}000{,}000{,}000 \text{ Quanta}$.
   - **PoW Mining Subsidy (Height > 0, 60%):**
     - Total PoW Target Emission: $39{,}600{,}000 \text{ AUR} = 3{,}960{,}000{,}000{,}000{,}000 \text{ Quanta}$.
     - Initial Subsidy ($S_0$): $99 \text{ AUR} = 9{,}900{,}000{,}000 \text{ Quanta}$ per block.
     - Halving Interval ($I$): $200{,}000 \text{ blocks}$.
     - Proof of exact integer emission:
       $$200{,}000 \times 99 \times 2 = 39{,}600{,}000 \text{ AUR}$$
     $$\text{Subsidy}(h) = \begin{cases} \lfloor \frac{9{,}900{,}000{,}000}{2^{\lfloor h / 200000 \rfloor}} \rfloor & \text{if } \lfloor h / 200000 \rfloor < 64 \\ 0 & \text{otherwise} \end{cases}$$
   - For block $B$ at height $h > 0$:
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

