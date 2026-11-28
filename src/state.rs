//! Core synchronization: one state machine + one ordered event queue.
//!
//! A single owner applies events in order. No shared mutable state, no races.
//! See the "Synchronization design" in the build plan.

use std::collections::VecDeque;

/// Application state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum State {
    /// VoiceType is off: no mic, no listening.
    #[default]
    Off,
    /// VoiceType is on; nothing happening right now.
    OnIdle,
    /// Capturing audio.
    Recording,
    /// Audio sent; waiting for Gemini text.
    Transcribing,
    /// Inserting text into the target field.
    Inserting,
}

/// Inputs/outputs processed in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// User sent the "start now" signal.
    StartNow,
    /// Stop VoiceType entirely.
    Stop,
    /// Begin recording (mic click / shortcut).
    RecordStart,
    /// Stop recording; send to transcription.
    RecordStop,
    /// Cancel current recording/transcription/insertion; back to idle.
    Cancel,
    /// Transcription succeeded.
    TranscriptReady,
    /// Transcription failed.
    TranscriptFailed,
    /// Insertion succeeded.
    Inserted,
    /// Insertion failed.
    InsertFailed,
    /// Focus/geometry info; does not change recording state.
    FocusChanged,
}

/// Single owner of state.
#[derive(Debug, Default)]
pub struct Machine {
    state: State,
}

impl Machine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> State {
        self.state
    }

    /// Apply one event, in order. Unknown transitions are ignored safely.
    pub fn handle(&mut self, event: Event) {
        self.state = match (self.state, event) {
            // start / stop
            (_, Event::Stop) => State::Off,
            (State::Off, Event::StartNow) => State::OnIdle,
            (s, Event::StartNow) => s, // already on: no-op

            // recording
            (State::OnIdle, Event::RecordStart) => State::Recording,
            (State::Recording, Event::RecordStop) => State::Transcribing,
            (State::Recording, Event::Cancel) => State::OnIdle,

            // transcription
            (State::Transcribing, Event::TranscriptReady) => State::Inserting,
            (State::Transcribing, Event::TranscriptFailed) => State::OnIdle,
            (State::Transcribing, Event::Cancel) => State::OnIdle,

            // insertion
            (State::Inserting, Event::Inserted) => State::OnIdle,
            (State::Inserting, Event::InsertFailed) => State::OnIdle,
            (State::Inserting, Event::Cancel) => State::OnIdle,

            // focus/geometry does not change state; everything else no-op
            (s, _) => s,
        };
    }
}

/// FIFO queue; events applied in the order they arrived.
#[derive(Debug, Default)]
pub struct EventQueue {
    queue: VecDeque<Event>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: Event) {
        self.queue.push_back(event);
    }

    pub fn pop(&mut self) -> Option<Event> {
        self.queue.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Drain in order, applying each event to the machine.
    pub fn apply_all(&mut self, machine: &mut Machine) {
        while let Some(event) = self.pop() {
            machine.handle(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_happy_path() {
        let mut m = Machine::new();
        assert_eq!(m.state(), State::Off);
        m.handle(Event::StartNow);
        assert_eq!(m.state(), State::OnIdle);
        m.handle(Event::RecordStart);
        assert_eq!(m.state(), State::Recording);
        m.handle(Event::RecordStop);
        assert_eq!(m.state(), State::Transcribing);
        m.handle(Event::TranscriptReady);
        assert_eq!(m.state(), State::Inserting);
        m.handle(Event::Inserted);
        assert_eq!(m.state(), State::OnIdle);
    }

    #[test]
    fn cancel_returns_to_idle() {
        let mut m = Machine::new();
        m.handle(Event::StartNow);
        m.handle(Event::RecordStart);
        m.handle(Event::Cancel);
        assert_eq!(m.state(), State::OnIdle);
    }

    #[test]
    fn stop_turns_off_from_anywhere() {
        let mut m = Machine::new();
        m.handle(Event::StartNow);
        m.handle(Event::RecordStart);
        m.handle(Event::Stop);
        assert_eq!(m.state(), State::Off);
    }

    #[test]
    fn record_requires_on_idle() {
        let mut m = Machine::new();
        m.handle(Event::RecordStart); // off: ignored
        assert_eq!(m.state(), State::Off);
    }

    #[test]
    fn queue_is_fifo_and_ordered() {
        let mut q = EventQueue::new();
        q.push(Event::StartNow);
        q.push(Event::RecordStart);
        q.push(Event::RecordStop);
        q.push(Event::TranscriptReady);
        q.push(Event::Inserted);
        assert_eq!(q.len(), 5);
        let mut m = Machine::new();
        q.apply_all(&mut m);
        assert!(q.is_empty());
        assert_eq!(m.state(), State::OnIdle);
    }
}
