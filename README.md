# Aurion Sovereign Protocol

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](Cargo.toml)
[![Rust](https://img.shields.io/badge/rust-2021%20edition-orange.svg)](Cargo.toml)
[![Safety](https://img.shields.io/badge/unsafe-forbid-brightgreen.svg)](Cargo.toml)
[![Arithmetic](https://img.shields.io/badge/floats-denied-red.svg)](Cargo.toml)

Aurion is a sovereign pure Rust monorepo protocol (*greenfield rebuild*) engineered with strict sovereignty boundaries (**Single Sovereign Authority**) and zero-float arithmetic guarantees.

---

## Architectural Hierarchy

The Aurion system is structured into four distinct layers of authority:

- **L0 - Consensus Core:** Pure stateless crates without I/O dependencies.
  - [`crates/aurion-primitives`](crates/aurion-primitives/): Fundamental types (`Hash256`, `Quantum: u128`).
  - [`crates/aurion-core`](crates/aurion-core/): Core protocol models and ledger definitions.
  - [`crates/aurion-script`](crates/aurion-script/): Deterministic script execution engine.
  - [`crates/aurion-consensus`](crates/aurion-consensus/): State transition rules and block validation.
- **L1 - Sovereign Runtime (`aurion-node`):**
  - The single canonical authority. Sole custodian of persistent `redb` storage, network protocol coordinator, and state machine driver.
- **L2 - Clients (`aurion-cli`):**
  - Command-line interface and client tool. Interacts strictly via IPC/RPC; direct access to physical storage is forbidden.
- **L3 - Observers (`aurion-indexer`):**
  - Derived read model and indexing service powered by an asynchronous event stream and SQLite.

---

## Strict Compiler Guarantees

Aurion enforces aggressive safety invariants at the workspace compiler level:

```toml
[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
float_arithmetic = "deny"
arithmetic_side_effects = "deny"
unwrap_used = "warn"
expect_used = "warn"
```

1. **No Unsafe Code:** Absolutely zero `unsafe` blocks permitted in the workspace.
2. **Zero-Float Accounting:** Floating-point operations are strictly forbidden to ensure deterministic consensus across heterogeneous architectures.
3. **Tripwires:** Value conservation and invariant violations trigger immediate fail-stop handling.

---

## Formal Specifications

Refer to the formal constitutional specifications in [`docs/`](docs/):

- [**SPEC-01: Sovereign Single Authority Specification**](docs/SPEC-01-SOVEREIGNTY.md)
- [**SPEC-02: Ledger Invariants & Mathematical Tripwires**](docs/SPEC-02-INVARIANTS.md)
- [**SPEC-03: Storage & Persistence Protocol**](docs/SPEC-03-STORAGE-PROTOCOL.md)
- [**SPEC-04: Fault Taxonomy & Fail-Stop Semantics**](docs/SPEC-04-FAULT-TAXONOMY.md)

---

## Getting Started

### Prerequisites
- Rust 1.80+ (stable toolchain)
- Cargo

### Building the Workspace
```bash
cargo build --workspace
```

### Running Static Checks & Linters
```bash
cargo check --workspace
cargo clippy --workspace --all-targets
```

---

## License

Dual-licensed under either of:
- MIT License
- Apache License, Version 2.0
