---
name: crypto-systems-engineer
description: >-
  Comprehensive systems engineering, blockchain and cryptocurrency architecture, web services,
  Linux kernel and Debian OS internals, and Git/GitHub workflows using Rust, Go, and Python with strict
  architectural discipline and enterprise-grade industry Rust standards. Use when building, architecting,
  or reviewing low-level OS/kernel code, crypto protocols, web backends, Debian deployments, or Git CI/CD pipelines.
---

# crypto-systems-engineer

Expert engineering guidance and robust code implementation spanning cryptocurrency protocols, blockchain infrastructure, high-concurrency web systems, Linux kernel/OS programming, Debian administration, and Git/GitHub workflows in Rust, Go, and Python.

## When to Use

- Designing or implementing cryptocurrencies, blockchain protocols (L1/L2), consensus engines, or smart contracts.
- Building high-concurrency web services, REST/gRPC/WebSocket APIs, and backend architectures.
- Writing low-level systems code, Linux kernel modules, eBPF programs, or OS-level IPC/concurrency routines.
- Configuring, packaging, and hardening Debian Linux environments, systemd services, and daemon processes.
- Setting up professional Git/GitHub workflows, repository structures, CI/CD pipelines, and branching strategies.
- Reviewing code for memory safety, concurrency, cryptographic integrity, and architectural rigor.

---

## Core Architectural Principles

1. **Layered and Decoupled Architecture**
   - Separate domain logic from transport (web/RPC), persistence (databases/key-value stores), and system-level I/O.
   - Use Clean/Hexagonal Architecture (Ports and Adapters) for deterministic state machines and testable business logic.
   - Enforce sovereign layer boundaries: Layer 0 (Primitives, Core, Consensus, VM, eUTXO), Layer 1 (Storage, Node Runtime, Authority, P2P), Layer 2 (RPC, CLI), Layer 3 (Indexers, Explorers).

2. **Determinism, Safety, and Concurrency**
   - Maintain strict determinism in state machines and financial/crypto balance calculations (no floating-point; use fixed-point, `u128`, or `u256`).
   - Enforce memory safety, thread safety, and race-free concurrency across low-level and high-level components.
   - Employ standard, verified cryptographic libraries and secure memory handling.

3. **Language-Specific Engineering Standards**
   - **Rust**: Focus on memory safety without garbage collection, zero-cost abstractions, strict error handling (`Result`, `thiserror`), safe concurrency (`tokio`, `crossbeam`, `parking_lot`), and low-level FFI/kernel bindings.
   - **Go**: Emphasize explicit error handling, clean interface composition, goroutine lifecycle management with `context.Context`, low-latency network I/O, and race detection (`go test -race`).
   - **Python**: Use for rapid prototyping, cryptographic simulation, system automation, and web backends (`FastAPI`/`asyncio`) with strict type hints (`mypy`, `pydantic`).

---

## Enterprise-Grade Industry Standard Rust Specialization (Crypto Systems)

In mission-critical cryptocurrency protocols, blockchain daemons, and financial settlement engines, Rust must be written according to sovereign enterprise standards:

### 1. Strict Compiler Lints & Zero-Unsafe Policy
- Enforce `#![forbid(unsafe_code)]` at all crate roots.
- Deny floating-point arithmetic at the workspace level:
  ```toml
  [workspace.lints.rust]
  unsafe_code = "forbid"

  [workspace.lints.clippy]
  float_arithmetic = "deny"
  arithmetic_side_effects = "deny"
  unwrap_used = "warn"
  expect_used = "warn"
  ```
- No hidden panics in production consensus code: replace `.unwrap()` with exhaustive pattern matching or structured error propagation via `?`.

### 2. Numerical Integrity & Zero-Float Financial Ledger
- **Never** use IEEE 754 floating-point types (`f32`, `f64`) in consensus rules, monetary calculations, fees, or balances.
- Use explicit newtypes wrapping unsigned integers (e.g. `Quantum(u128)`).
- Every arithmetic operation must use checked or saturating primitives (`checked_add`, `checked_sub`, `checked_mul`, `checked_div`, `saturating_sub`). Arithmetic overflow must trigger an explicit invariant error.

### 3. Canonical Binary Codecs & Anti-Malleability
- Implement deterministic binary encoding (`CanonicalCodec`) without non-canonical representations.
- Reject trailing unconsumed bytes on deserialization (`TrailingBytes` error).
- Pre-validate length prefixes against hard memory ceilings before allocating byte buffers to prevent memory exhaustion attacks (anti-DoS).
- Break circular signature dependencies: when computing transaction `sighash`, zero out or clear `unlocking_script`s so signatures commit strictly to the transaction skeleton and effects.

### 4. Deterministic State Machines & Fail-Stop Semantics
- Model node lifecycle states via explicit finite state machines: `Starting -> Recovering -> Running -> Stopped | Failed(NodeFault)`.
- If an invariant violation or storage corruption occurs, the engine must immediately lock into a terminal **Fail-Stop** state. It must refuse all subsequent mutations rather than running on inconsistent state.
- Keep state validation pure and stateless: use abstractions like `UtxoView` to separate validation logic from disk/storage I/O.

### 5. High-Concurrency Async Runtime & Resource Isolation
- Use Tokio with explicit cooperative scheduling, bounded channels (`tokio::sync::mpsc`), and structured concurrency (`CancellationToken`, `tokio::select!`).
- Enforce backpressure: bounded network buffers and rate-limited connection pools.
- Thread-safe interior mutability using `parking_lot` non-poisoning locks or `Arc<RwLock<T>>` with deterministic lock acquisition ordering to avoid deadlocks.

### 6. Cryptographic Rigor
- Use industry-standard, audit-grade cryptographic libraries (`ed25519-dalek`, `blake3`, `ring`, `subtle`).
- Verify Ed25519 signatures using `verify_strict` to reject malleable signature variants.
- Constant-time verification for authentication tokens and cryptographic digests.

---

## Domain-Specific Guidelines

1. **Cryptocurrency and Blockchain Architecture**
   - Ledger models: Choose between UTXO/eUTXO and Account-based state models.
   - Consensus and Networking: Model deterministic state transitions, P2P gossip networks, mempool management, block validation, and cryptographic signing.
   - eUTXO Semantics: Enforce Coinbase Maturity (e.g., 100 blocks), Locktime MTP (Median Time Past) validation, Relative Sequence delays, and minimum fee-per-byte requirements.

2. **Web and High-Concurrency Backend Systems**
   - Build modular API layers supporting REST, gRPC, and WebSocket streaming.
   - Implement rate limiting, connection pooling, backpressure handling, and secure authentication (JWT, mTLS).
   - Ensure structured logging, distributed tracing (OpenTelemetry / `tracing`), and health probe endpoints.

3. **Kernel, Operating Systems, and Debian Linux**
   - OS and Kernel Internals: Implement robust syscall handling, virtual memory management concepts, POSIX compliance, IPC (shared memory, Unix domain sockets), and eBPF tracing/filtering.
   - Debian OS Management: Follow Debian packaging standards (`dpkg`, `apt`), manage services via systemd unit files, and enforce system security (least privilege, AppArmor, firewalling).

4. **Git and GitHub Best Practices**
   - Maintain clean commit history with conventional commits (`feat:`, `fix:`, `refactor:`, `chore:`, `test:`).
   - Enforce branch protection, automated CI/CD checks (GitHub Actions for linting, testing, and security auditing), and semantic versioning (SemVer).
   - Manage multi-repository dependencies using Cargo workspaces, Go modules, or submodules.

---

## Core Workflow

1. **Specification and Architecture Design**
   - Define functional boundaries, interfaces, data schemas (Protobuf, JSON, Canonical binary), and threat models.
   - Select appropriate runtime environments, kernel/OS dependencies, and data storage solutions (`redb`, `RocksDB`, `PostgreSQL`, `SQLite`).

2. **Implementation and System Integration**
   - Implement core domain logic and isolate low-level system or network interactions.
   - Write unit, integration, and fuzz tests covering edge cases, network partitions, and malformed inputs.
   - Verify complete workspaces with zero warnings on strict compiler suites.

3. **Deployment and Environment Configuration**
   - Create reproducible builds, Dockerfiles, and Debian deployment scripts/packages.
   - Set up systemd service configurations with proper restart policies, resource limits (`LimitNOFILE`, `MemoryMax`), and security isolation (`ProtectSystem`, `NoNewPrivileges`).

---

## Gotchas & Anti-Patterns

- **State and Financial Calculations**: Never use IEEE 754 floating-point numbers in consensus, tokenomics, or financial accounting.
- **Resource Leaks**: Always ensure proper cleanup of file descriptors, sockets, and OS memory mappings.
- **Concurrency Deadlocks**: Avoid lock contention or unbounded goroutine/thread spawning; use bounded worker pools and context cancellation.
- **Hardening Gaps**: Never run network-exposed crypto or web daemons as root on Debian; use dedicated system users and minimal privileges.
- **Circular Sighash Dependency**: Never include the signature itself in the data preimage hashed for verification.
