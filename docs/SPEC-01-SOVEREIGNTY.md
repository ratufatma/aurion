# SPEC-01: Sovereign Single Authority Specification

## 1. Definisi Hierarki Otoritas
* **L0 - Consensus Core:** Crate murni stateless tanpa I/O (`aurion-primitives`, `aurion-core`, `aurion-consensus`, `aurion-script`).
* **L1 - Sovereign Runtime (`aurion-node`):** Satu-satunya otoritas kanonikal. Pemilik tunggal storage `redb` dan pengendali state transition.
* **L2 - Clients (`aurion-cli`):** Berkomunikasi hanya via IPC/RPC. Dilarang keras mengakses storage fisik secara langsung.
* **L3 - Observers (`aurion-indexer`):** Pengamat pasif berbasis event stream. Kehilangan atau korupsi L3 tidak boleh memengaruhi L1.

## 2. Invariant Kedaulatan
1. Tidak ada library yang memiliki lifecycle atau persistent database mandiri di dalam node.
2. Tidak ada toleransi fallback semu (`.unwrap_or(0)` pada data konsensus dilarang keras).
