mod atspi_watch;
mod audio;
mod config;
mod inject;
mod overlay;
mod session_log;
mod stt;

use atspi::connection::AccessibilityConnection;
use atspi::ObjectRef;
use session_log::SessionLog;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    // File + console logging for easy debugging by non-technical users.
    let log_file_path = log_path();
    let _ = std::fs::create_dir_all(log_file_path.parent().unwrap_or(&log_file_path));
    let log_file = std::fs::OpenOptions::new()
        .create(true).append(true)
        .open(&log_file_path)
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
    let log = SessionLog::new();
    log.info(format!("VoiceType starting (model: {})", cfg.model));

    // First-run setup wizard. Prefer zenity; fall back to reading from the
    // terminal so a missing zenity doesn't silently leave us with no key.
    if cfg.google_api_key.is_empty() || cfg.google_api_key == "PASTE_YOUR_GOOGLE_CLOUD_API_KEY_HERE" {
        log.info("no API key configured; launching setup");
        let mut key = String::new();
        match std::process::Command::new("zenity")
            .args(["--entry", "--title=VoiceType Setup",
                   "--text=Paste your Google Gemini API key (aistudio.google.com/apikey):",
                   "--width=400"])
            .output()
        {
            Ok(o) if o.status.success() => {
                key = String::from_utf8_lossy(&o.stdout).trim().to_string();
            }
            Ok(_) => log.warn("setup dialog cancelled"),
            Err(e) => {
                // zenity not installed: ask on the terminal instead of dying silently.
                log.warn(format!("zenity not available ({e}); falling back to terminal prompt"));
                use std::io::Write;
                print!("Paste your Google Gemini API key: ");
                let _ = std::io::stdout().flush();
                if std::io::stdin().read_line(&mut key).is_ok() {
                    key = key.trim().to_string();
                }
            }
        }
        if !key.is_empty() {
            cfg.google_api_key = key;
            let _ = config::save(&cfg);
        } else {
            log.warn("no API key entered; transcription will fail");
        }
    }

    // Validate the key up front so a bad/revoked key is visible immediately
    // instead of failing silently on the first dictation.
    if !cfg.google_api_key.is_empty() {
        match stt::validate_key(&cfg.google_api_key).await {
            Ok(()) => {
                log.info("Gemini API key validated");
                notify_ok("VoiceType setup done",
                    "Your Gemini key works. Click the mic next to any text field to dictate.");
            }
            Err(e) => {
                log.error(format!("API key validation failed: {e}"));
                notify_error(&format!(
                    "Your Gemini API key was rejected:\n\n{e}\n\nEdit {} and set a valid key.",
                    config::config_path().display()
                ));
            }
        }
    }

    let overlay = overlay::Overlay::new();
    overlay.show_debug();
    let focused: Arc<Mutex<Option<ObjectRef>>> = Arc::new(Mutex::new(None));
    let window_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let (click_tx, mut click_rx) = mpsc::channel::<overlay::Btn>(16);
    let (auto_tx, mut auto_rx) = mpsc::channel::<()>(16);

    let watcher_conn = match AccessibilityConnection::new().await {
        Ok(c) => c,
        Err(e) => {
            let msg = format!(
                "Could not connect to the accessibility bus.\n\n{e}\n\n\
                 Make sure accessibility is enabled (System Settings → Accessibility) \
                 and that `dbus-send --session --dest=org.a11y.Bus \
                 /org/a11y/bus org.a11y.Bus.GetAddress` works.\n\n\
                 Log: {}", log_path().display()
            );
            log.error(format!("atspi connection failed: {e}"));
            notify_error(&msg);
            std::process::exit(1);
        }
    };
    log.info("connected to accessibility bus");
    let overlay_w = overlay.clone();
    let focused_w = focused.clone();
    let window_id_w = window_id.clone();
    let auto_record = cfg.auto_record_on_focus;
    let auto_tx_w = auto_tx.clone();
    let log_w = log.clone();
    tokio::spawn(async move {
        atspi_watch::run_watcher(watcher_conn, overlay_w, focused_w, window_id_w, auto_record, auto_tx_w, log_w).await;
    });

    let overlay_click = overlay.clone();
    std::thread::spawn(move || loop {
        let btn = {
            let rx = overlay_click.click_rx.lock().unwrap_or_else(|e| e.into_inner());
            rx.recv()
        };
        match btn {
            Ok(b) => { if click_tx.blocking_send(b).is_err() { break; } }
            Err(_) => break,
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
                    start_pipeline(
                        &cfg, &overlay, &focused, &window_id, &is_recording,
                        &mut current_stop, &log,
                    ).await;
                }
            }
            Some(btn) = click_rx.recv() => {
                match btn {
                    overlay::Btn::Debug => {
                        show_report(&log);
                    }
                    overlay::Btn::Mic => {
                        if is_recording.load(Ordering::Relaxed) {
                            if let Some(s) = &current_stop { s.store(true, Ordering::Relaxed); }
                            is_recording.store(false, Ordering::Relaxed);
                            log.info("mic clicked: stop recording");
                        } else {
                            log.info("mic clicked: start recording");
                            start_pipeline(
                                &cfg, &overlay, &focused, &window_id, &is_recording,
                                &mut current_stop, &log,
                            ).await;
                        }
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                log.info("VoiceType stopping (ctrl-c)");
                break;
            }
        }
    }
}

/// Begin a recording, then transcribe + inject in the background.
/// Handles the mic button state machine: Recording -> Processing -> Idle/Error.
async fn start_pipeline(
    cfg: &config::Config,
    overlay: &overlay::Overlay,
    focused: &Arc<Mutex<Option<ObjectRef>>>,
    window_id: &Arc<Mutex<Option<String>>>,
    is_recording: &Arc<AtomicBool>,
    current_stop: &mut Option<Arc<AtomicBool>>,
    log: &SessionLog,
) {
    is_recording.store(true, Ordering::Relaxed);
    overlay.set_state(overlay::OverlayState::Recording);

    let stop = Arc::new(AtomicBool::new(false));
    *current_stop = Some(stop.clone());

    let cfg2 = cfg.clone();
    let overlay2 = overlay.clone();
    let focused2 = focused.clone();
    let window_id2 = window_id.clone();
    let stop2 = stop.clone();
    let is_rec2 = is_recording.clone();
    let log2 = log.clone();

    tokio::spawn(async move {
        log2.info("recording started");
        let rec = audio::Recorder { stop: stop2.clone() };
        let path = tokio::task::spawn_blocking(move || {
            rec.record(cfg2.sample_rate, cfg2.silence_threshold, cfg2.max_record_seconds)
        }).await.unwrap_or_else(|_| Err("recording task join failed".into()));

        // Always clear the recording flag promptly, regardless of outcome.
        is_rec2.store(false, Ordering::Relaxed);

        match path {
            Err(e) => {
                log2.error(format!("recording failed: {e}"));
                overlay2.set_state(overlay::OverlayState::Error);
                notify_error(&format!("Recording failed: {e}"));
                std::thread::sleep(std::time::Duration::from_secs(2));
                overlay2.set_state(overlay::OverlayState::Idle);
            }
            Ok(p) => {
                let secs = std::fs::metadata(&p).map(|m| m.len() / 32000).unwrap_or(0);
                log2.info(format!("recording finished (~{secs}s); sending to Gemini ({})", cfg2.model));
                overlay2.set_state(overlay::OverlayState::Processing);
                let result = stt::transcribe(
                    &cfg2.google_api_key, &p, &cfg2.language,
                    &cfg2.alt_languages, &cfg2.model, cfg2.sample_rate,
                ).await;
                let _ = std::fs::remove_file(&p);

                match result {
                    Ok(text) if !text.is_empty() => {
                        log2.info(format!("transcript: \"{text}\""));
                        let target = focused2.lock().unwrap_or_else(|e| e.into_inner()).clone();
                        if let Some(t) = target {
                            let wid = window_id2.lock().unwrap_or_else(|e| e.into_inner()).clone();
                            match AccessibilityConnection::new().await {
                                Ok(conn) => {
                                    match inject::inject_text(conn.connection(), &t, &text, wid.as_deref()).await {
                                        Ok(()) => log2.info("text injected into field"),
                                        Err(e) => {
                                            log2.error(format!("inject failed: {e}"));
                                            overlay2.set_state(overlay::OverlayState::Error);
                                            notify_error(&format!("Could not insert text: {e}"));
                                            std::thread::sleep(std::time::Duration::from_secs(2));
                                        }
                                    }
                                }
                                Err(e) => {
                                    log2.error(format!("atspi reconnect failed: {e}"));
                                    overlay2.set_state(overlay::OverlayState::Error);
                                }
                            }
                        } else {
                            log2.warn("no target field focused; transcript not inserted");
                        }
                    }
                    Ok(_) => {
                        log2.warn("transcription was empty (no speech?)");
                    }
                    Err(e) => {
                        log2.error(format!("transcription failed: {e}"));
                        overlay2.set_state(overlay::OverlayState::Error);
                        notify_error(&format!("Transcription failed:\n\n{e}"));
                        std::thread::sleep(std::time::Duration::from_secs(2));
                    }
                }
                overlay2.set_state(overlay::OverlayState::Idle);
            }
        }
    });
}

/// Show the session report in a scrollable dialog (debug icon clicked).
fn show_report(log: &SessionLog) {
    let report = format!(
        "VoiceType session report ({} events)\nLog file: {}\n{}\n{}",
        log.len(),
        log_path().display(),
        "=".repeat(48),
        log.report()
    );
    // Try zenity text-info (scrollable, copyable). If unavailable, notify.
    use std::io::Write;
    let child = std::process::Command::new("zenity")
        .args(["--text-info", "--title=VoiceType debug report",
               "--width=760", "--height=520", "--font=monospace"])
        .stdin(std::process::Stdio::piped())
        .spawn();
    match child {
        Ok(mut c) => {
            if let Some(mut stdin) = c.stdin.take() {
                let _ = stdin.write_all(report.as_bytes());
            }
            let _ = c.wait();
        }
        Err(_) => notify_ok("VoiceType debug report", &report),
    }
}
async fn run_checks() {
    use std::process::Command;
    println!("VoiceType diagnostics");
    println!("=====================");
    let cfg = config::load();
    println!("Config: {}", config::config_path().display());
    println!("  API key set: {}", !cfg.google_api_key.is_empty());
    println!("  Model: {}", cfg.model);
    println!("  Language: {} (alt: {:?})", cfg.language, cfg.alt_languages);
    if !cfg.google_api_key.is_empty() {
        print!("  Gemini key test: ... ");
        use std::io::Write;
        let _ = std::io::stdout().flush();
        match stt::validate_key(&cfg.google_api_key).await {
            Ok(()) => println!("OK"),
            Err(e) => println!("FAILED ({e})"),
        }
    }
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
    println!("\nLog file: {}", log_path().display());
}

/// Path to the log file, shared by logging setup and the error dialog.
fn log_path() -> std::path::PathBuf {
    dirs::data_local_dir().unwrap_or_default().join("voicetype").join("voicetype.log")
}

/// Best-effort user-visible error: system notification if available, else
/// stderr. A `Terminal=false` launcher hides stderr, so the notification is
/// what lets a non-technical user see *why* startup failed.
pub fn notify_error(msg: &str) {
    eprintln!("{msg}");
    let notified = std::process::Command::new("notify-send")
        .args(["--urgency=critical", "--app-name=VoiceType", "VoiceType failed to start", msg])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !notified {
        // Last resort: try zenity's error dialog.
        let _ = std::process::Command::new("zenity")
            .args(["--error", "--title=VoiceType failed to start", &format!("--text={msg}")])
            .status();
    }
}

/// Non-error confirmation notification (e.g. "setup done").
pub fn notify_ok(title: &str, msg: &str) {
    let _ = std::process::Command::new("notify-send")
        .args(["--app-name=VoiceType", title, msg])
        .status();
}
