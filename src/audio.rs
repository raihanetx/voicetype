//! Task 7: audio capture (pure core + boundary).
//!
//! Precise start/stop, ordered chunks, no dropped audio. The real backend
//! (PipeWire / PulseAudio, native) is not wired yet; tests use `MockAudio`.
//! Short clip is produced for transcription (Task 8).

/// Audio format of captured chunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

impl Default for AudioFormat {
    fn default() -> Self {
        // 16 kHz mono is sufficient for speech transcription.
        Self { sample_rate: 16_000, channels: 1 }
    }
}

/// A complete captured clip ready for transcription.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Clip {
    pub format: AudioFormat,
    pub data: Vec<u8>,
}

/// Recording state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecordingState {
    #[default]
    Idle,
    Recording,
}

/// A source of microphone audio.
pub trait AudioSource {
    fn start(&mut self);
    /// Stop and return ordered captured bytes. Nothing dropped.
    fn stop(&mut self) -> Vec<u8>;
}

/// In-memory audio source for tests / non-desktop environments.
#[derive(Debug, Default)]
pub struct MockAudio {
    pub chunks: Vec<Vec<u8>>,
    recording: bool,
}

impl AudioSource for MockAudio {
    fn start(&mut self) {
        self.recording = true;
    }
    fn stop(&mut self) -> Vec<u8> {
        self.recording = false;
        let out: Vec<u8> = self.chunks.iter().flatten().copied().collect();
        out
    }
}

/// Ordered recorder built on an `AudioSource`.
pub struct Recorder<S: AudioSource> {
    source: S,
    format: AudioFormat,
    state: RecordingState,
}

impl<S: AudioSource> Recorder<S> {
    pub fn new(source: S) -> Self {
        Self { source, format: AudioFormat::default(), state: RecordingState::Idle }
    }

    pub fn state(&self) -> RecordingState {
        self.state
    }

    /// Begin capturing. No-op if already recording.
    pub fn start(&mut self) {
        if self.state == RecordingState::Recording {
            return;
        }
        self.source.start();
        self.state = RecordingState::Recording;
    }

    /// Stop and return a complete ordered clip. Empty if not recording.
    pub fn stop(&mut self) -> Clip {
        if self.state != RecordingState::Recording {
            return Clip { format: self.format, data: Vec::new() };
        }
        let data = self.source.stop();
        self.state = RecordingState::Idle;
        Clip { format: self.format, data }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_format_is_16k_mono() {
        let f = AudioFormat::default();
        assert_eq!(f.sample_rate, 16_000);
        assert_eq!(f.channels, 1);
    }

    #[test]
    fn start_stop_returns_all_bytes_in_order() {
        let mock = MockAudio { chunks: vec![vec![1, 2], vec![3], vec![4, 5, 6]], recording: false };
        let mut rec = Recorder::new(mock);
        rec.start();
        assert_eq!(rec.state(), RecordingState::Recording);
        let clip = rec.stop();
        assert_eq!(rec.state(), RecordingState::Idle);
        assert_eq!(clip.data, vec![1, 2, 3, 4, 5, 6]); // nothing dropped, ordered
    }

    #[test]
    fn double_start_is_noop_and_stop_when_idle_is_empty() {
        let mut rec = Recorder::new(MockAudio::default());
        rec.start();
        rec.start();
        assert_eq!(rec.state(), RecordingState::Recording);
        let _ = rec.stop();
        let clip = rec.stop(); // idle
        assert!(clip.data.is_empty());
        assert_eq!(rec.state(), RecordingState::Idle);
    }
}
