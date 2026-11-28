//! Real read-only Gemini key validation over HTTP (ureq).
//!
//! GET `.../v1beta/models` with header `x-goog-api-key`. **Read-only.**
//! Never logs the key; errors are redacted. Model `gemini-3.5-transcribe`.
//! We already verified this endpoint returns HTTP 200 for a valid key; the
//! full transcription endpoint is NOT wired yet. Compile-checked by CI; not
//! runtime-tested in CI (no real key / no network guarantee).

use crate::secrets::{redact, GeminiValidator, Secret};

/// Gemini API base.
pub const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com";
/// Read-only model list endpoint.
pub const MODELS_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";
/// Speech-to-text model.
pub const GEMINI_TRANSCRIBE_MODEL: &str = "gemini-3.5-transcribe";

/// Real Gemini validator using `ureq`. No key in logs; errors redacted.
#[derive(Debug, Default, Clone, Copy)]
pub struct GeminiHttpValidator;

impl GeminiValidator for GeminiHttpValidator {
    fn validate(&self, key: &Secret) -> Result<(), String> {
        let raw = key.expose();
        let result = ureq::get(MODELS_URL)
            .header("x-goog-api-key", raw)
            .call();

        match result {
            Ok(mut resp) => {
                let status = resp.status();
                let body = resp
                    .body_mut()
                    .read_to_string()
                    .map_err(|e| redact(&format!("read error: {e}"), &[raw]))?;
                if status == 200 && body.contains("\"models\"") {
                    Ok(())
                } else {
                    Err(redact(&format!("unexpected response {status}"), &[raw]))
                }
            }
            Err(e) => Err(redact(&format!("request failed: {e}"), &[raw])),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_correct() {
        assert_eq!(GEMINI_TRANSCRIBE_MODEL, "gemini-3.5-transcribe");
        assert!(MODELS_URL.ends_with("/v1beta/models"));
        assert_eq!(GEMINI_API_BASE, "https://generativelanguage.googleapis.com");
    }
}
