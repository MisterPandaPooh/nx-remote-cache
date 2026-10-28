//! nx-remote-cache: a fast, self-hosted implementation of the Nx remote cache API.
//!
//! The crate is split so the HTTP router can be driven from integration tests
//! without spawning the binary.

pub mod app;
pub mod auth;
pub mod config;
pub mod routes;
pub mod stats;
pub mod store;
pub mod ui;

pub use app::{router, AppState};
pub use config::Config;

/// Install the ring crypto provider for rustls once per process. Safe to call repeatedly.
pub fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}
