use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub google_api_key: String,
    /// Primary language code, e.g. "en-US" or "bn-BD".
    pub language: String,
    /// Alternative language codes for code-switching.
    pub alt_languages: Vec<String>,
    /// Google Speech model, e.g. "latest_long".
    pub model: String,
    /// Sample rate for recording.
    pub sample_rate: u32,
    /// Silence threshold (0..1) for auto-stop.
    pub silence_threshold: f32,
    /// Max recording seconds.
    pub max_record_seconds: u32,
    /// If true, start recording automatically when a text field gains focus
    /// (hands-free, no clicks). Stops on ~2s of silence.
    pub auto_record_on_focus: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            google_api_key: String::new(),
            language: "en-US".into(),
            alt_languages: vec!["bn-BD".into()],
            model: "latest_long".into(),
            sample_rate: 16000,
            silence_threshold: 0.015,
            max_record_seconds: 60,
            auto_record_on_focus: false,
        }
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("voicetype")
        .join("config.toml")
}

pub fn load() -> Config {
    let p = config_path();
    if let Ok(s) = fs::read_to_string(&p) {
        toml::from_str(&s).unwrap_or_default()
    } else {
        let c = Config::default();
        let _ = save(&c);
        c
    }
}

pub fn save(c: &Config) -> std::io::Result<()> {
    let p = config_path();
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
    }
    let s = toml::to_string_pretty(c).unwrap_or_default();
    fs::write(p, s)
}
