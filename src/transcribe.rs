//! Task 8: Gemini transcription (pure core + boundary).
//!
//! Model: `gemini-3.5-transcribe`. Audio clip in, ordered text out. No partial
//! or out-of-order results; one clip -> one transcript -> inserted once. The
//! real Gemini network call is not wired yet; no HTTP crate is guessed; the
//! API key comes from `secrets` (local, never logged). Tests use `MockTranscriber`.

use crate::audio::Clip;
use crate::secrets::Secret;

/// Gemini speech-to-text model.
pub const GEMINI_TRANSCRIBE_MODEL: &str = "gemini-3.5-transcribe";

/// Gemini API base (read-only model list worked; generation endpoint used later).
pub const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com";

/// Successful transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    pub text: String,
    pub model: String,
}

/// Why transcription failed. Never contains the API key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscribeError {
    EmptyClip,
    MissingApiKey,
    Timeout,
    /// Request rejected (e.g. bad key); message already redacted.
    Rejected(String),
    /// Network/transport problem.
    Network(String),
}

/// Speech-to-text backend. Real impl calls Gemini; tests mock it.
pub trait Transcriber {
    fn transcribe(&self, clip: &Clip, key: &Secret) -> Result<Transcript, TranscribeError>;
}

/// In-memory transcriber for tests / non-network environments.
#[derive(Debug, Clone)]
pub struct MockTranscriber {
    pub result: Result<Transcript, TranscribeError>,
}

impl Transcriber for MockTranscriber {
    fn transcribe(&self, _clip: &Clip, _key: &Secret) -> Result<Transcript, TranscribeError> {
        self.result.clone()
    }
}

/// Transcribe a clip once. Rejects empty clips; requires a key; single ordered result.
pub fn transcribe_clip<T: Transcriber>(
    transcriber: &T,
    clip: &Clip,
    key: &Secret,
) -> Result<Transcript, TranscribeError> {
    if clip.data.is_empty() {
        return Err(TranscribeError::EmptyClip);
    }
    if key.is_empty() {
        return Err(TranscribeError::MissingApiKey);
    }
    transcriber.transcribe(clip, key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::AudioFormat;

    fn clip() -> Clip {
        Clip { format: AudioFormat::default(), data: vec![1, 2, 3, 4] }
    }

    #[test]
    fn model_name_is_gemini_3_5_transcribe() {
        assert_eq!(GEMINI_TRANSCRIBE_MODEL, "gemini-3.5-transcribe");
    }

    #[test]
    fn empty_clip_rejected() {
        let t = MockTranscriber { result: Ok(Transcript { text: "x".into(), model: GEMINI_TRANSCRIBE_MODEL.into() }) };
        let empty = Clip { format: AudioFormat::default(), data: vec![] };
        assert_eq!(transcribe_clip(&t, &empty, &Secret::new("k")), Err(TranscribeError::EmptyClip));
    }

    #[test]
    fn missing_key_rejected() {
        let t = MockTranscriber { result: Ok(Transcript { text: "x".into(), model: GEMINI_TRANSCRIBE_MODEL.into() }) };
        assert_eq!(transcribe_clip(&t, &clip(), &Secret::new("  ")), Err(TranscribeError::MissingApiKey));
    }

    #[test]
    fn returns_single_ordered_transcript() {
        let want = Transcript { text: "hello world".into(), model: GEMINI_TRANSCRIBE_MODEL.into() };
        let t = MockTranscriber { result: Ok(want.clone()) };
        let got = transcribe_clip(&t, &clip(), &Secret::new("AQ.key")).unwrap();
        assert_eq!(got, want);
    }

    #[test]
    fn propagates_failure_without_key() {
        let t = MockTranscriber { result: Err(TranscribeError::Rejected("[redacted]".into())) };
        let err = transcribe_clip(&t, &clip(), &Secret::new("AQ.key")).unwrap_err();
        assert_eq!(err, TranscribeError::Rejected("[redacted]".into()));
    }
}
