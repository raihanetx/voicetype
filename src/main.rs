mod atspi_watch;
mod audio;
mod config;
mod inject;
mod overlay;
mod stt;

use atspi::connection::AccessibilityConnection;
use atspi::ObjectRef;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    // File + console logging for easy debugging by non-technical users.
    let log_dir = dirs::data_local_dir().unwrap_or_default().join("voicetype");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_file = std::fs::OpenOptions::new()
        .create(true).append(true)
        .open(log_dir.join("voicetype.log"))
        .ok();
    if let Some(f) = log_file {
        let f = std::sync::Arc::new(std::sync::Mutex::new(f));
        tracing_subscriber::fmt()
            .with_writer(move || {
                f.lock().unwrap_or_else(|e| e.into_inner()).try_clone().unwrap()
            })
            .init();
    } else {
        tracing_subscriber::fmt::init();
    }

    // Diagnostic mode: `voicetype --check`
    if std::env::args().any(|a| a == "--check" || a == "-c") {
        run_checks().await;
        return;
    }

    let mut cfg = config::load();

    // First-run setup wizard via zenity (no heavy GUI dependency)
    if cfg.google_api_key.is_empty() {
        let out = std::process::Command::new("zenity")
            .args(["--entry", "--title=VoiceType Setup",
                   "--text=Paste your Google Cloud Speech-to-Text API key:",
                   "--width=400"])
            .output();
        match out {
            Ok(o) if o.status.success() => {
                let key = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if !key.is_empty() {
                    cfg.google_api_key = key;
                    let _ = config::save(&cfg);
                } else {
                    tracing::warn!("No API key entered; transcription will fail");
                }
            }
            Ok(_) => { tracing::warn!("Setup cancelled; transcription will fail"); }
            Err(e) => { tracing::error!("zenity not found ({e}); install zenity or edit config manually"); }
        }
    }

    let overlay = overlay::Overlay::new();
    let focused: Arc<Mutex<Option<ObjectRef>>> = Arc::new(Mutex::new(None));
    let window_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let (click_tx, mut click_rx) = mpsc::channel::<()>(16);
    let (auto_tx, mut auto_rx) = mpsc::channel::<()>(16);

    let watcher_conn = match AccessibilityConnection::new().await {
        Ok(c) => c,
        Err(e) => { eprintln!("atspi connection failed: {e}"); return; }
    };
    let overlay_w = overlay.clone();
    let focused_w = focused.clone();
    let window_id_w = window_id.clone();
    let auto_record = cfg.auto_record_on_focus;
    let auto_tx_w = auto_tx.clone();
    tokio::spawn(async move {
        atspi_watch::run_watcher(watcher_conn, overlay_w, focused_w, window_id_w, auto_record, auto_tx_w).await;
    });

    let overlay_click = overlay.clone();
    std::thread::spawn(move || loop {
        let rx = overlay_click.click_rx.lock().unwrap_or_else(|e| e.into_inner());
        if rx.recv().is_ok() {
            let _ = click_tx.blocking_send(());
        } else {
            break;
        }
    });

    let is_recording = Arc::new(AtomicBool::new(false));
    let mut current_stop: Option<Arc<AtomicBool>> = None;

    loop {
        tokio::select! {
            Some(()) = auto_rx.recv() => {
                // Drain queued auto-triggers to avoid spam recordings
                while auto_rx.try_recv().is_ok() {}
                if is_recording.load(Ordering::Relaxed) { /* ignore while recording */ } else {
                    is_recording.store(true, Ordering::Relaxed);
                    overlay.set_recording(true);
                    let stop = Arc::new(AtomicBool::new(false));
                    current_stop = Some(stop.clone());
                    let cfg2 = cfg.clone();
                    let overlay2 = overlay.clone();
                    let focused2 = focused.clone();
                    let window_id2 = window_id.clone();
                    let stop2 = stop.clone();
                    let is_rec2 = is_recording.clone();
                    tokio::spawn(async move {
                        let rec = audio::Recorder { stop: stop2.clone() };
                        let path = tokio::task::spawn_blocking(move || {
                            rec.record(cfg2.sample_rate, cfg2.silence_threshold, cfg2.max_record_seconds)
                        }).await.unwrap_or(Err("join".into()));
                        if let Ok(p) = path {
                            match stt::transcribe(&cfg2.google_api_key, &p, &cfg2.language, &cfg2.alt_languages, &cfg2.model, cfg2.sample_rate).await {
                                Ok(text) => {
                                    if !text.is_empty() {
                                        let target = focused2.lock().unwrap_or_else(|e| e.into_inner()).clone();
                                        if let Some(t) = target {
                                            let wid = window_id2.lock().unwrap_or_else(|e| e.into_inner()).clone();
                                            match AccessibilityConnection::new().await {
                                                Ok(conn) => { let _ = inject::inject_text(conn.connection(), &t, &text, wid.as_deref()).await; }
                                                Err(e) => tracing::error!("atspi reconnect failed: {e}"),
                                            }
                                        }
                                    }
                                }
                                Err(e) => tracing::error!("stt: {e}"),
                            }
                            let _ = std::fs::remove_file(p);
                        }
                        is_rec2.store(false, Ordering::Relaxed);
                        overlay2.set_recording(false);
                    });
                }
            }
            Some(()) = click_rx.recv() => {
                if is_recording.load(Ordering::Relaxed) {
                    if let Some(s) = &current_stop { s.store(true, Ordering::Relaxed); }
                    is_recording.store(false, Ordering::Relaxed);
                } else {
                    is_recording.store(true, Ordering::Relaxed);
                    overlay.set_recording(true);
                    let stop = Arc::new(AtomicBool::new(false));
                    current_stop = Some(stop.clone());
                    let cfg2 = cfg.clone();
                    let overlay2 = overlay.clone();
                    let focused2 = focused.clone();
                    let window_id2 = window_id.clone();
                    let stop2 = stop.clone();
                    let is_rec2 = is_recording.clone();
                    tokio::spawn(async move {
                        let rec = audio::Recorder { stop: stop2.clone() };
                        let path = tokio::task::spawn_blocking(move || {
                            rec.record(cfg2.sample_rate, cfg2.silence_threshold, cfg2.max_record_seconds)
                        }).await.unwrap_or(Err("join".into()));
                        if let Ok(p) = path {
                            match stt::transcribe(&cfg2.google_api_key, &p, &cfg2.language, &cfg2.alt_languages, &cfg2.model, cfg2.sample_rate).await {
                                Ok(text) => {
                                    if !text.is_empty() {
                                        let target = focused2.lock().unwrap_or_else(|e| e.into_inner()).clone();
                                        if let Some(t) = target {
                                            let wid = window_id2.lock().unwrap_or_else(|e| e.into_inner()).clone();
                                            match AccessibilityConnection::new().await {
                                                Ok(conn) => { let _ = inject::inject_text(conn.connection(), &t, &text, wid.as_deref()).await; }
                                                Err(e) => tracing::error!("atspi reconnect failed: {e}"),
                                            }
                                        }
                                    }
                                }
                                Err(e) => tracing::error!("stt: {e}"),
                            }
                            let _ = std::fs::remove_file(p);
                        }
                        is_rec2.store(false, Ordering::Relaxed);
                        overlay2.set_recording(false);
                    });
                }
            }
            _ = tokio::signal::ctrl_c() => {
                break;
            }
        }
    }
}
async fn run_checks() {
    use std::process::Command;
    println!("VoiceType diagnostics");
    println!("=====================");
    let cfg = config::load();
    println!("Config: {}", config::config_path().display());
    println!("  API key set: {}", !cfg.google_api_key.is_empty());
    println!("  Language: {} (alt: {:?})", cfg.language, cfg.alt_languages);
    println!("DISPLAY: {:?}", std::env::var("DISPLAY").ok());
    let xd = Command::new("which").arg("xdotool").output();
    println!("xdotool: {}", xd.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_else(|_| "MISSING".into()));
    use cpal::traits::DeviceTrait;
    use cpal::traits::HostTrait;
    let host = cpal::default_host();
    match host.default_input_device() {
        Some(d) => println!("Audio input: {}", d.name().unwrap_or_default()),
        None => println!("Audio input: NONE (no microphone)"),
    }
    match atspi::connection::AccessibilityConnection::new().await {
        Ok(_) => println!("AT-SPI: OK"),
        Err(e) => println!("AT-SPI: FAILED ({e})"),
    }
    println!("\nLog file: {}", dirs::data_local_dir().unwrap_or_default().join("voicetype/voicetype.log").display());
}
