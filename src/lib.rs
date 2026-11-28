//! VoiceType — voice-to-text for Linux Mint (Rust skeleton).

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Library version string.
pub fn version() -> &'static str {
    VERSION
}
