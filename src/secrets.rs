//! Secrets handling (Task 3).
//!
//! Rules: secrets are never printed, logged, committed, or included in error
//! messages; they are read from a local secure source; validation is read-only
//! and done through a trait so tests can mock it. The Gemini key is separate
//! from any GitHub token. Real secure OS-keyring storage and real Gemini
//! network validation are added later; this module keeps it testable.

/// A secret string that refuses to leak via `Debug`/`Display`.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Secret(value.into())
    }

    /// Access the raw value only at the point of use (e.g. an API call).
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.trim().is_empty()
    }
}

// No `Debug`/`Display` impl on purpose: `{:?}` cannot print the secret.
// `Clone` is fine; we never log it.

/// Where the Gemini API key comes from.
pub trait KeySource {
    fn gemini_key(&self) -> Option<Secret>;
}

/// Reads `GEMINI_API_KEY` from the process environment (not from the repo).
pub struct EnvKeySource;

impl KeySource for EnvKeySource {
    fn gemini_key(&self) -> Option<Secret> {
        std::env::var("GEMINI_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(Secret::new)
    }
}

/// Read-only Gemini key validation. Real impl calls Gemini; tests mock it.
pub trait GeminiValidator {
    fn validate(&self, key: &Secret) -> Result<(), String>;
}

/// Validate a non-empty key with the given validator. Never logs the key.
pub fn validate_gemini_key<V: GeminiValidator>(
    key: &Secret,
    validator: &V,
) -> Result<(), String> {
    if key.is_empty() {
        return Err("Gemini API key is empty".to_string());
    }
    validator.validate(key)
}

/// Replace any occurrence of secrets in `text` with `[redacted]`.
pub fn redact(text: &str, secrets: &[&str]) -> String {
    let mut out = text.to_string();
    for s in secrets {
        if !s.is_empty() {
            out = out.replace(s, "[redacted]");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockValidator(Result<(), String>);
    impl GeminiValidator for MockValidator {
        fn validate(&self, _key: &Secret) -> Result<(), String> {
            self.0.clone()
        }
    }

    struct FixedSource(Option<String>);
    impl KeySource for FixedSource {
        fn gemini_key(&self) -> Option<Secret> {
            self.0.clone().map(Secret::new)
        }
    }

    #[test]
    fn secret_has_no_debug_leak() {
        let s = Secret::new("AQ.supersecret");
        // Secret does not implement Debug; this just checks it holds the value.
        assert_eq!(s.expose(), "AQ.supersecret");
    }

    #[test]
    fn empty_key_rejected_before_validator() {
        let v = MockValidator(Ok(()));
        assert!(validate_gemini_key(&Secret::new("   "), &v).is_err());
    }

    #[test]
    fn validator_decides_validity() {
        let key = Secret::new("AQ.real-looking");
        assert!(validate_gemini_key(&key, &MockValidator(Ok(()))).is_ok());
        assert!(validate_gemini_key(&key, &MockValidator(Err("bad key".into()))).is_err());
    }

    #[test]
    fn key_source_reads_local_source_not_repo() {
        let src = FixedSource(Some("AQ.key".to_string()));
        assert_eq!(src.gemini_key().unwrap().expose(), "AQ.key");
        assert!(FixedSource(None).gemini_key().is_none());
    }

    #[test]
    fn redacts_secrets_in_logs() {
        let msg = "error using AQ.abc123 key for repo ghp_xyz";
        let out = redact(msg, &["AQ.abc123", "ghp_xyz"]);
        assert_eq!(out, "error using [redacted] key for repo [redacted]");
        assert!(!out.contains("AQ.abc123"));
        assert!(!out.contains("ghp_xyz"));
    }
}
