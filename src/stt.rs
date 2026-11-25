use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Serialize)]
struct RecognitionAudio {
    content: String,
}

#[derive(Debug, Serialize)]
struct RecognitionConfig {
    encoding: String,
    #[serde(rename = "sampleRateHertz")]
    sample_rate_hertz: u32,
    language_code: String,
    #[serde(rename = "alternativeLanguageCodes")]
    alternative_language_codes: Vec<String>,
    model: String,
    #[serde(rename = "enableAutomaticPunctuation")]
    enable_automatic_punctuation: bool,
}

#[derive(Debug, Serialize)]
struct RecognizeRequest {
    config: RecognitionConfig,
    audio: RecognitionAudio,
}

#[derive(Debug, Deserialize)]
struct SpeechRecognitionAlternative {
    transcript: String,
}

#[derive(Debug, Deserialize)]
struct SpeechRecognitionResult {
    alternatives: Vec<SpeechRecognitionAlternative>,
}

#[derive(Debug, Deserialize)]
struct RecognizeResponse {
    results: Option<Vec<SpeechRecognitionResult>>,
}

pub async fn transcribe(
    api_key: &str,
    wav_path: &std::path::Path,
    language: &str,
    alt: &[String],
    model: &str,
    sample_rate: u32,
) -> Result<String, String> {
    let bytes = fs::read(wav_path).map_err(|e| e.to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

    let body = RecognizeRequest {
        config: RecognitionConfig {
            encoding: "LINEAR16".into(),
            sample_rate_hertz: sample_rate,
            language_code: language.into(),
            alternative_language_codes: alt.to_vec(),
            model: model.into(),
            enable_automatic_punctuation: true,
        },
        audio: RecognitionAudio { content: b64 },
    };

    let url = format!(
        "https://speech.googleapis.com/v1/speech:recognize?key={}",
        api_key
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build().map_err(|e| e.to_string())?;
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        let t = resp.text().await.unwrap_or_default();
        return Err(format!("google stt error: {t}"));
    }

    let parsed: RecognizeResponse = resp.json().await.map_err(|e| e.to_string())?;
    let mut out = String::new();
    if let Some(results) = parsed.results {
        for r in results {
            if let Some(alt0) = r.alternatives.into_iter().next() {
                out.push_str(&alt0.transcript);
                out.push(' ');
            }
        }
    }
    Ok(out.trim().to_string())
}
