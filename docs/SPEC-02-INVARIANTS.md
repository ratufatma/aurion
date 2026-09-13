# SPEC-02: Ledger Invariants & Mathematical Tripwires

## 1. Conservation of Value
* The sum of eUTXO quantum inputs must equal or exceed output + fee.
* All quantum calculations strictly use pure `u128` unsigned integers (Zero-Float arithmetic).

## 2. eUTXO Immutability
* Spent inputs can never be resurrected or respent.
* Outpoint identifiers are deterministic, canonical, and globally unique.
