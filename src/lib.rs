//! VoiceType — voice-to-text for Linux Mint (Rust skeleton).

pub mod focus;
pub mod ipc;
pub mod secrets;
pub mod state;

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Library version string.
pub fn version() -> &'static str {
    VERSION
}
