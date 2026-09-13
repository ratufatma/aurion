//! # Aurion IPC/RPC Transport (`aurion-rpc`)
//!
//! Jembatan asinkron berkecepatan tinggi (*sub-millisecond local transport*) yang
//! menghubungkan daemon L1 `aurion node` dengan proses eksternal `aurion mine`
//! dan `aurion wallet`.
//!
//! Prinsip arsitektur:
//!
//! - **Zero-Unsafe** — `#![forbid(unsafe_code)]` di seluruh krate.
//! - **Zero-Float** — `#![deny(clippy::float_arithmetic)]`; semua nilai moneter
//!   berada dalam `Quantum` (`u128`, $10^{-8}$ AUR).
//! - **Enkoding deterministik** — seluruh pesan mengimplementasikan
//!   [`CanonicalCodec`](aurion_primitives::CanonicalCodec) yang menolak
//!   trailing bytes (`CodecError::TrailingBytes`).
//! - **Pembatasan alokasi defensif** — setiap frame dibatasi $\le 4\text{ MB}$
//!   dan panjangnya divalidasi sebelum alokasi payload.
//!
//! Pertukaran pesan terjadi dalam frame 4-byte Big-Endian length-delimited
//! (lihat [`frame`]) yang membungkus serialisasi kanonikal
//! [`RpcRequest`] / [`RpcResponse`] (lihat [`protocol`]). Server
//! ([`server`]) memisahkan lapisan transport dari konsensus L1 melalui trait
//! [`NodeRpcHandler`], sedangkan [`client`] menyediakan `RpcClient` tipis
//! untuk miner dan dompet. Struktur data mining (`BlockTemplate`,
//! `SubmitResult`) didefinisikan di [`template`].
//!
//! Spesifikasi formal transport ini: `docs/SPEC-07-IPC-RPC-PROTOCOL.md`.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod client;
pub mod error;
pub mod frame;
pub mod protocol;
pub mod server;
pub mod template;

pub use client::RpcClient;
pub use error::RpcError;
pub use frame::{read_frame, write_frame, MAX_RPC_FRAME_SIZE};
pub use protocol::{RpcRequest, RpcResponse};
pub use server::{NodeRpcHandler, RpcServer};
pub use template::{BlockTemplate, SubmitResult};