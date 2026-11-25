use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{WavSpec, WavWriter};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Recorder {
    pub stop: Arc<AtomicBool>,
}

impl Recorder {
    pub fn new() -> Self {
        Self { stop: Arc::new(AtomicBool::new(false)) }
    }

    /// Record audio to a temp WAV file.
    /// Stops on: manual stop flag, max duration, or ~2s of silence after speech.
    pub fn record(&self, sample_rate: u32, silence_threshold: f32, max_seconds: u32) -> Result<PathBuf, String> {
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or("no input device")?;
        let config = cpal::StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let spec = WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let tmp = std::env::temp_dir().join(format!("voicetype-{}.wav", std::process::id()));
        let writer = WavWriter::create(&tmp, spec).map_err(|e| e.to_string())?;
        let writer = Arc::new(Mutex::new(Some(writer)));

        let stop = self.stop.clone();
        let writer_clone = writer.clone();
        let last_sound = Arc::new(Mutex::new(Instant::now()));
        let last_sound_cb = last_sound.clone();
        let threshold = silence_threshold;

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _| {
                let mut guard = writer_clone.lock().unwrap();
                if let Some(w) = guard.as_mut() {
                    for &s in data {
                        let _ = w.write_sample((s * i16::MAX as f32) as i16);
                    }
                }
                // RMS of chunk
                let mut sum = 0.0f32;
                for &s in data { sum += s * s; }
                let rms = if data.is_empty() { 0.0 } else { (sum / data.len() as f32).sqrt() };
                if rms >= threshold {
                    *last_sound_cb.lock().unwrap() = Instant::now();
                }
            },
            |e| tracing::error!("stream err: {e}"),
            None,
        ).map_err(|e| e.to_string())?;

        stream.play().map_err(|e| e.to_string())?;

        let start = Instant::now();
        let max = Duration::from_secs(max_seconds as u64);
        let mut had_speech = false;
        loop {
            std::thread::sleep(Duration::from_millis(200));
            if stop.load(Ordering::Relaxed) { break; }
            if start.elapsed() >= max { break; }
            let silence_for = last_sound.lock().unwrap().elapsed();
            // only auto-stop after we've heard speech, then 2s silence
            if silence_for < Duration::from_millis(50) {
                had_speech = true;
            }
            if had_speech && silence_for >= Duration::from_secs(2) {
                break;
            }
        }

        drop(stream);
        let mut guard = writer.lock().unwrap();
        if let Some(w) = guard.take() {
            w.finalize().map_err(|e| e.to_string())?;
        }
        Ok(tmp)
    }
}
