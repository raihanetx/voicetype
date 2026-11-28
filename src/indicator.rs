//! Task 11: on/off indicator (pure core).
//!
//! A clear indicator that VoiceType is running. Off = nothing happens; on =
//! works in any text field. **Manual start only** (no auto-start at login);
//! you start it by sending "start now". Real tray/menu UI not wired yet;
//! this keeps the state + label testable.

use crate::state::{Machine, State};

/// On/off plus activity, derived from the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Indicator {
    /// VoiceType off; no mic, no listening.
    #[default]
    Off,
    /// On; idle.
    On,
    /// On; recording.
    Recording,
    /// On; transcribing.
    Transcribing,
    /// On; inserting.
    Inserting,
}

impl Indicator {
    pub fn from_machine(m: &Machine) -> Self {
        match m.state() {
            State::Off => Indicator::Off,
            State::OnIdle => Indicator::On,
            State::Recording => Indicator::Recording,
            State::Transcribing => Indicator::Transcribing,
            State::Inserting => Indicator::Inserting,
        }
    }

    pub fn is_on(self) -> bool {
        !matches!(self, Indicator::Off)
    }

    /// Short human label (plain English).
    pub fn label(self) -> &'static str {
        match self {
            Indicator::Off => "Off",
            Indicator::On => "On",
            Indicator::Recording => "Recording",
            Indicator::Transcribing => "Transcribing",
            Indicator::Inserting => "Inserting",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Event;

    #[test]
    fn off_by_default_and_not_on() {
        let m = Machine::new();
        let i = Indicator::from_machine(&m);
        assert_eq!(i, Indicator::Off);
        assert!(!i.is_on());
        assert_eq!(i.label(), "Off");
    }

    #[test]
    fn manual_start_makes_on_no_auto_start() {
        let mut m = Machine::new();
        // No auto-start at login: stays Off until a "start now" is sent.
        assert_eq!(Indicator::from_machine(&m), Indicator::Off);
        m.handle(Event::StartNow);
        assert_eq!(Indicator::from_machine(&m), Indicator::On);
        assert!(Indicator::from_machine(&m).is_on());
    }

    #[test]
    fn reflects_activity_and_stop() {
        let mut m = Machine::new();
        m.handle(Event::StartNow);
        m.handle(Event::RecordStart);
        assert_eq!(Indicator::from_machine(&m), Indicator::Recording);
        m.handle(Event::RecordStop);
        assert_eq!(Indicator::from_machine(&m), Indicator::Transcribing);
        m.handle(Event::Stop);
        assert_eq!(Indicator::from_machine(&m), Indicator::Off);
    }
}
