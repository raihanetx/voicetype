//! VoiceType — voice-to-text for Linux Mint (Rust skeleton).

pub mod atspi;
pub mod audio;
pub mod focus;
pub mod gemini_http;
pub mod gemini_transcribe;
pub mod indicator;
pub mod insert;
pub mod ipc;
pub mod overlay;
pub mod pipeline;
pub mod secrets;
pub mod state;
pub mod transcribe;

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Library version string.
pub fn version() -> &'static str {
    VERSION
}
