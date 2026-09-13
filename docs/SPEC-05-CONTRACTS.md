# SPEC-05: Deterministic eUTXO Smart Contracts & Covenants Engine

**Protocol Layer**: Layer 0 (Stateless Pure Core) & Layer 1 (Consensus Engine)  
**Status**: Canonical / Implemented  
**Author**: Principal Crypto Systems Engineer  
**Date**: March 2026  

---

## 1. Executive Abstract

The **SPEC-05 Deterministic eUTXO Smart Contracts & Covenants Engine** specifies the mathematical contract and covenant evaluation model for the Aurion Sovereign Digital Asset Protocol. 

By generalizing the Bitcoin-style UTXO model into the Extended UTXO (**eUTXO**) paradigm, Aurion enables expressive, multi-step state machines, automated escrow covenants, and non-interactive smart contracts while strictly preserving:
1. **Total Determinism**: A transaction's execution cost, state transition, and validity are known prior to block submission.
2. **Local Reasoning**: Contract execution depends exclusively on the spending transaction and the immediate UTXO being consumed—never on ambient or mutable global state.
3. **DoS-Resistant Execution Limits**: Strict stack, operation, and element byte ceilings eliminate halting problem vulnerabilities and CPU exhaustion attacks.
4. **Zero Ambient I/O**: The virtual machine is completely isolated from physical storage, system clocks, network sockets, and non-deterministic concurrency.

---

## 2. The $(D, R, C)$ Semantic Model

In Aurion, smart contract verification evaluates a pure mathematical predicate over three input components:

$$\text{ContractPredicate}(D, R, C) \longrightarrow \{\text{Valid}, \text{Invalid}\}$$

Where:
- **$D$ (Datum)**: The passive state committed to the consumed UTXO output (`TxOutput.datum`).
- **$R$ (Redeemer)**: The active user intent or witness arguments provided by the spending input (`TxInput.redeemer`).
- **$C$ (ScriptContext)**: The zero-copy, read-only introspection context representing the entire spending transaction, its inputs, outputs, locktime, fee, and validation height.

```
┌─────────────────────────────────────────────────────────────┐
│                       eUTXO CONSUMPTION                     │
└─────────────────────────────────────────────────────────────┘
      Spent UTXO                     Spending Transaction Input
 ┌───────────────────┐               ┌─────────────────────────┐
 │ Value: Quantum    │               │ Previous OutPoint       │
 │ Locking Script    │               │ Unlocking Script        │
 │ Datum (D)         │               │ Sequence                │
 └─────────┬─────────┘               │ Redeemer (R)            │
           │                         └────────────┬────────────┘
           │                                      │
           └──────────────────┬───────────────────┘
                              ▼
           ┌─────────────────────────────────────┐
           │       Zero-Copy ScriptContext (C)   │
           │  • tx: &Transaction                 │
           │  • current_input_index: usize       │
           │  • current_spent_utxo: &ExtendedUtxo│
           │  • fee: Quantum                     │
           │  • validation_height: u64           │
           └──────────────────┬──────────────────┘
                              ▼
           ┌─────────────────────────────────────┐
           │         ScriptEngine VM (L0)        │
           │  Step 1: Push Datum (D)             │
           │  Step 2: Push Redeemer (R)          │
           │  Step 3: Eval Unlocking Bytecode    │
           │  Step 4: Eval Locking Bytecode      │
           │  Step 5: Verify Final Stack Truthy  │
           └─────────────────────────────────────┘
```

---

## 3. Canonical Binary Codecs & Limits

### 3.1 Defensive Resource Limits
To prevent unbounded memory allocation and CPU consumption, all execution parameters are bounded by constitutional limits:

| Parameter | Limit | Enforcement Mechanism |
| :--- | :--- | :--- |
| `MAX_STACK_DEPTH` | 1024 elements | `Stack::push` returns `ScriptError::StackOverflow` |
| `MAX_ELEMENT_SIZE` | 520 bytes | `Stack::push` returns `ScriptError::ElementTooLarge` |
| `MAX_OPS_PER_SCRIPT` | 201 opcodes | `execute_bytecode` returns `ScriptError::MaxOpCountExceeded` |
| `MAX_DATUM_SIZE` | 520 bytes | `Datum::decode_from_cursor` returns `CodecError::NonCanonicalEncoding` |
| `MAX_REDEEMER_SIZE` | 520 bytes | `TxInput::decode_from_cursor` returns `CodecError::NonCanonicalEncoding` |

### 3.2 `Datum` Serialization Framing
The `Datum` enum encodes passive state associated with a UTXO:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Datum {
    None,
    Hash(Hash256),
    Inline(Vec<u8>),
}
```

- **Tag `0x00` (`Datum::None`)**: Serialized as a single byte `0x00`.
- **Tag `0x01` (`Datum::Hash`)**: Serialized as `0x01` followed by 32 bytes of the cryptographic digest.
- **Tag `0x02` (`Datum::Inline`)**: Serialized as `0x02` followed by a 2-byte Big-Endian length prefix ($L \le 520$) and $L$ payload bytes. Encodings where $L > 520$ are unconditionally rejected.

### 3.3 `TxInput` and `TxOutput` Codec Framing
- **`TxOutput`**: Serialized as `value` (16 bytes BE) $\to$ `locking_script` (2 bytes BE length prefix + payload) $\to$ `datum` (canonical datum encoding).
- **`TxInput`**: Serialized as `previous_output` (32 bytes hash + 4 bytes BE index) $\to$ `unlocking_script` (2 bytes BE length prefix + payload) $\to$ `sequence` (4 bytes BE) $\to$ `redeemer` (`0x00` for None; `0x01` + 2 bytes BE length prefix + payload for Some).

---

## 4. Introspection Covenant Opcodes (`0xC0..=0xC7`)

Covenants enable UTXOs to dictate how their descendants may be spent, locking funds into verifiable state machines without custodial intermediaries. Aurion implements covenants via eight deterministic introspection opcodes:

| Opcode | Hex | Stack Effect | Description |
| :--- | :---: | :--- | :--- |
| `OpTxInputsCount` | `0xC0` | `[] -> [u32_le]` | Pushes the number of inputs in the spending transaction as 4-byte LE. |
| `OpCurrentInputIdx` | `0xC1` | `[] -> [u32_le]` | Pushes the index of the input currently being evaluated as 4-byte LE. |
| `OpTxOutputsCount` | `0xC2` | `[] -> [u32_le]` | Pushes the number of outputs in the spending transaction as 4-byte LE. |
| `OpTxOutputValue` | `0xC3` | `[idx: u32] -> [u128_le]` | Pops output index `idx`. Pushes `outputs[idx].value` as 16-byte LE. Fails with `IndexOutOfBounds` if `idx >= outputs.len()`. |
| `OpTxOutputScript` | `0xC4` | `[idx: u32] -> [hash: 32B]` | Pops output index `idx`. Pushes Blake3 hash of `outputs[idx].locking_script`. Fails with `IndexOutOfBounds` if invalid index. |
| `OpTxOutputDatum` | `0xC5` | `[idx: u32] -> [hash: 32B]` | Pops output index `idx`. Pushes Blake3 hash of `outputs[idx].datum.encode_canonical()` (or `[0u8; 32]` if `Datum::None`). |
| `OpTxLocktime` | `0xC6` | `[] -> [u64_le]` | Pushes the spending transaction's `locktime` as 8-byte LE. |
| `OpValidationHeight` | `0xC7` | `[] -> [u64_le]` | Pushes the validation block height from `ScriptContext` as 8-byte LE. |

If an introspection opcode is executed without an active `ScriptContext` (e.g., in a standalone script context), the engine halts with `ScriptError::ContextUnavailable`.

---

## 5. VM Execution Flow & Termination Guarantees

Contract execution in `ScriptEngine::execute_contract` proceeds through an atomic 5-stage pipeline:

1. **Stack Initialization**: Reset the evaluation stack to clean state (`depth == 0`).
2. **Datum Injection**:
   - If `Datum::Inline(bytes)`: push raw `bytes` to stack.
   - If `Datum::Hash(hash)`: push 32-byte `hash` to stack.
   - If `Datum::None`: no action.
3. **Redeemer Injection**:
   - If `Some(redeemer_bytes)`: push `redeemer_bytes` to stack.
   - If `None`: no action.
4. **Bytecode Interpretation**:
   - Execute `unlocking_script` byte sequence.
   - Execute `locking_script` byte sequence on the same stack.
   - Increment opcode counter on every operation; abort with `MaxOpCountExceeded` if counter exceeds 201.
5. **Acceptance Invariant**:
   - The contract succeeds (`Ok(true)`) if and only if:
     1. The stack has **exactly one element** remaining (`stack.len() == 1`).
     2. The top element is **truthy** (contains at least one non-zero byte).
   - Any stack underflow, unconsumed multiple elements, zero/empty top element, or unhandled error results in transaction rejection.

---

## 6. SIGHASH Preimage Binding

To ensure unforgeable cryptographic spending authority:
1. `Transaction::sighash(input_index)` generates the Blake3 hash of the canonical transaction preimage.
2. In accordance with BIP-143 / Milestone 10 principles, all `input.unlocking_script` fields are cleared to empty vectors before serializing the preimage.
3. Crucially, the transaction preimage **strictly binds**:
   - Every input's `previous_output`, `sequence`, and `redeemer`.
   - Every output's `value`, `locking_script`, and `datum`.
   - Transaction `version` and `locktime`.

Any tampering with outputs, datums, or sibling redeemers invalidates all Ed25519 signatures across the transaction.

---

## 7. Fail-Stop Ledger Integration

In `aurion-node`'s `verify_block_invariants`:
1. Every non-coinbase input instantiates a borrowed `ScriptContext` bound to the active block header height and calculated transaction fee.
2. If any contract returns `Ok(false)` or `Err(ScriptError)`, the authority engine rejects the entire candidate block with `InvariantError::ScriptExecutionFailed`.
3. Invariant violations are classified as consensus faults (candidate block rejection) or class-3 faults (fail-stop trigger if encountered in authoritative ledger reorgs), guaranteeing zero state corruption.
