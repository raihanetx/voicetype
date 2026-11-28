//! Task 10: wire "start now" -> start recording in the current field.
//!
//! Keep two things separate (think twice, re-confirmed):
//! - `start_now` starts VoiceType (off -> on). It does NOT start recording.
//! - `start_recording` starts recording in the *currently focused* safe text
//!   field; it re-checks focus and snapshots the target. Called separately.
//! Tested apart. No global hotkey yet (a hotkey would just send `start-now`).

use crate::atspi::{recheck_on_start, AtSpiBackend};
use crate::insert::TargetSnapshot;
use crate::state::{Event, Machine, State};

/// Send the "start now" signal: start VoiceType. Separate from recording.
pub fn start_now(machine: &mut Machine) {
    machine.handle(Event::StartNow);
}

/// Stop VoiceType entirely.
pub fn stop(machine: &mut Machine) {
    machine.handle(Event::Stop);
}

/// Start recording in the currently focused field.
///
/// Requires VoiceType `OnIdle`. Re-checks current focus; only a safe editable
/// text field qualifies. Returns a `TargetSnapshot` for synchronized insertion
/// (Task 9). Transitions to `Recording` on success. Returns `None` otherwise.
pub fn start_recording<B: AtSpiBackend>(machine: &mut Machine, backend: &B) -> Option<TargetSnapshot> {
    if machine.state() != State::OnIdle {
        return None;
    }
    let focus = recheck_on_start(backend)?;
    let snapshot = TargetSnapshot::from_focus(&focus);
    machine.handle(Event::RecordStart);
    if machine.state() != State::Recording {
        return None;
    }
    Some(snapshot)
}

/// Stop recording; move to transcription. Separate from insertion.
pub fn stop_recording(machine: &mut Machine) {
    machine.handle(Event::RecordStop);
}

/// Cancel current recording/transcription/insertion back to idle.
pub fn cancel(machine: &mut Machine) {
    machine.handle(Event::Cancel);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atspi::MockAtSpi;
    use crate::focus::{FocusEvent, Rect};

    fn text_event() -> FocusEvent {
        FocusEvent {
            app: "gedit".into(),
            role: "text".into(),
            rect: Rect::new(0, 0, 200, 24),
            editable: true,
            enabled: true,
            ..Default::default()
        }
    }

    #[test]
    fn start_now_is_separate_from_recording() {
        let mut m = Machine::new();
        start_now(&mut m);
        assert_eq!(m.state(), State::OnIdle); // on, NOT recording
    }

    #[test]
    fn start_recording_requires_on_idle_and_safe_focus() {
        // off -> can't record
        let mut m = Machine::new();
        let backend = MockAtSpi { focused: Some(text_event()) };
        assert!(start_recording(&mut m, &backend).is_none());
        assert_eq!(m.state(), State::Off);

        // on + safe field -> recording + snapshot
        start_now(&mut m);
        let snap = start_recording(&mut m, &backend).unwrap();
        assert_eq!(m.state(), State::Recording);
        assert_eq!(snap.app, "gedit");
    }

    #[test]
    fn start_recording_rejects_no_field_and_password() {
        let mut m = Machine::new();
        start_now(&mut m);
        assert!(start_recording(&mut m, &MockAtSpi { focused: None }).is_none());
        assert_eq!(m.state(), State::OnIdle);

        let mut pw = text_event();
        pw.is_password = true;
        assert!(start_recording(&mut m, &MockAtSpi { focused: Some(pw) }).is_none());
        assert_eq!(m.state(), State::OnIdle);
    }

    #[test]
    fn full_chain_start_then_record_then_stop() {
        let mut m = Machine::new();
        let backend = MockAtSpi { focused: Some(text_event()) };
        start_now(&mut m);                       // Task 4 + Task 10a
        let _snap = start_recording(&mut m, &backend).unwrap(); // Task 10b
        stop_recording(&mut m);                  // -> transcribing (Task 8 next)
        assert_eq!(m.state(), State::Transcribing);
    }
}
