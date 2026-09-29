//! Titi protocol core.
//!
//! Pure, platform-independent state machine shared by Android (UniFFI),
//! iOS (UniFFI) and web (wasm-bindgen). No I/O, no threads, no clocks: every
//! entry point receives `now_ms` and returns [`engine::Action`]s.
//!
//! Module map (see `docs/ARCHITECTURE.md`):
//! - [`frame`]    — radio envelope + voice header (ADR-0003)
//! - [`identity`] — Ed25519/X25519 keys, node ids
//! - [`crypto`]   — Noise sessions, group key, AEAD, replay windows (ADR-0004)
//! - [`invite`]   — 3-word+2-digit rotating codes, QR links
//! - [`mesh`]     — dedup, neighbours, topology, routing, flood scheduler
//! - [`floor`]    — MCPTT-lite floor control
//! - [`handover`] — link handover state machine
//! - [`audio`]    — Opus profiles, jitter buffer, mixer
//! - [`store`]    — store-and-forward message queue
//! - [`engine`]   — façade wiring everything together

pub mod audio;
pub mod crypto;
pub mod engine;
pub mod error;
pub mod floor;
pub mod frame;
pub mod handover;
pub mod identity;
pub mod invite;
#[cfg(feature = "json")]
pub mod json;
pub mod link;
pub mod mesh;
pub mod store;
pub mod time;

/// Generated protobuf types (`titi.v1`).
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/titi.v1.rs"));
}

pub use error::Error;
pub type Result<T> = core::result::Result<T, Error>;

pub const PROTOCOL_VERSION: u8 = 1;
