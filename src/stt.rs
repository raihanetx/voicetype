use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;

const INTERACTIONS_URL: &str = "https://generativelanguage.googleapis.com/v1beta/interactions";

#[derive(Debug, Serialize)]
struct AudioContent {
    #[serde(rename = "type")]
    kind: &'static str, // "audio"
    data: String,
    #[serde(rename = "mime_type")]
    mime_type: &'static str, // "audio/wav"
}

#[derive(Debug, Serialize)]
struct TextContent {
    #[serde(rename = "type")]
    kind: &'static str, // "text"
    text: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum Input {
    Audio(AudioContent),
    Text(TextContent),
}

#[derive(Debug, Serialize)]
struct TranscriptionConfig {
    language_codes: Vec<String>,
    mode: &'static str, // "verbatim"
}

#[derive(Debug, Serialize)]
struct GenerationConfig {
    transcription_config: TranscriptionConfig,
}

#[derive(Debug, Serialize)]
struct InteractionRequest {
    model: String,
    input: Vec<Input>,
    generation_config: GenerationConfig,
}

// ---- response types ----
#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

#[derive(Debug, Deserialize)]
struct Step {
    #[serde(default)]
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ErrorBody {
    #[serde(default)]
    message: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    code: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct InteractionResponse {
    #[serde(default)]
    status: String,
    #[serde(default)]
    steps: Vec<Step>,
    #[serde(default)]
    error: Option<ErrorBody>,
}

/// Transcribe a WAV file with Gemini 3.5 Transcribe via the Interactions API.
pub async fn transcribe(
    api_key: &str,
    wav_path: &std::path::Path,
    language: &str,
    alt: &[String],
    model: &str,
    _sample_rate: u32,
) -> Result<String, String> {
    let bytes = fs::read(wav_path).map_err(|e| format!("read wav: {e}"))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

    // Primary + alternative language codes for code-switching.
    let mut language_codes = vec![language.to_string()];
    for a in alt {
        if !a.is_empty() && !language_codes.contains(a) {
            language_codes.push(a.clone());
        }
    }

    let body = InteractionRequest {
        model: model.to_string(),
        input: vec![
            Input::Audio(AudioContent {
                kind: "audio",
                data: b64,
                mime_type: "audio/wav",
            }),
            Input::Text(TextContent {
                kind: "text",
                text: "Transcribe the speech in this audio verbatim. \
                       Return only the transcript text with no commentary."
                    .into(),
            }),
        ],
        generation_config: GenerationConfig {
            transcription_config: TranscriptionConfig {
                language_codes,
                mode: "verbatim",
            },
        },
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(INTERACTIONS_URL)
        .header("x-goog-api-key", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;

    let status = resp.status();
    let text_body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        // Try to surface Google's own error message.
        if let Ok(err) = serde_json::from_str::<serde_json::Value>(&text_body) {
            if let Some(msg) = err
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
            {
                return Err(format!("gemini api error ({}): {msg}", status.as_u16()));
            }
        }
        return Err(format!("gemini api error {}: {text_body}", status.as_u16()));
    }

    let parsed: InteractionResponse =
        serde_json::from_str(&text_body).map_err(|e| format!("parse: {e} ({text_body})"))?;

    if parsed.status == "failed" {
        let msg = parsed
            .error
            .map(|e| e.message)
            .unwrap_or_else(|| "unknown failure".into());
        return Err(format!("gemini interaction failed: {msg}"));
    }

    let mut out = String::new();
    for step in parsed.steps {
        for block in step.content {
            if block.kind == "text" {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(block.text.trim());
            }
        }
    }
    Ok(out.trim().to_string())
}

/// Cheap key validation: send a tiny text-only interaction. Returns Ok(()) if
/// the key is accepted, or Err with Google's message.
pub async fn validate_key(api_key: &str) -> Result<(), String> {
    if api_key.is_empty() {
        return Err("API key is empty".into());
    }
    let body = serde_json::json!({
        "model": "gemini-3.5-transcribe",
        "input": [{"type": "text", "text": "Reply with the single word: ok"}],
    });
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(INTERACTIONS_URL)
        .header("x-goog-api-key", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;
    let status = resp.status();
    let txt = resp.text().await.unwrap_or_default();
    if status.is_success() {
        Ok(())
    } else if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
        let msg = v
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or(&txt)
            .to_string();
        Err(msg)
    } else {
        Err(format!("{status}: {txt}"))
    }
}
