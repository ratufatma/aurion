# SPEC-04: Fault Taxonomy & Fail-Stop Semantics

## 1. Operational Faults (Non-Fatal)
* Masalah jaringan P2P, timeout peer, format paket peer rusak.
* Aksi: Putus sesi peer, node tetap beroperasi normal.

## 2. Integrity Faults (Fatal Fail-Stop)
* Korupsi storage, inkonsistensi UTXO, pelanggaran invariant, kegagalan commit disk.
* Aksi: Transisi `NodeState::Failed(NodeFault)`, hentikan seluruh worker, tolak seluruh mutasi lanjutan.
