//! Task 5 (part 2): AT-SPI adapter boundary.
//!
//! The pure model lives in `focus`. This defines the boundary a real AT-SPI
//! backend implements. No external AT-SPI crate is wired yet; a live backend
//! (crate `atspi` 0.30.0, D-Bus) is added after its API is verified and tested
//! on a real Linux Mint machine. Tests use `MockAtSpi`.

use crate::focus::FocusEvent;

/// Source of focus information from the accessibility layer.
pub trait AtSpiBackend {
    /// Re-check what is currently focused *right now*.
    fn current_focused(&self) -> Option<FocusEvent>;
}

/// In-memory backend for tests / non-desktop environments.
#[derive(Debug, Default, Clone)]
pub struct MockAtSpi {
    pub focused: Option<FocusEvent>,
}

impl AtSpiBackend for MockAtSpi {
    fn current_focused(&self) -> Option<FocusEvent> {
        self.focused.clone()
    }
}

/// On "start now", re-check current focus and return it only if it's a safe
/// editable text field. (Matches ARCHITECTURE §3 + your start-now requirement.)
pub fn recheck_on_start<B: AtSpiBackend>(backend: &B) -> Option<FocusEvent> {
    backend.current_focused().filter(|e| e.is_text_field())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::focus::Rect;

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
    fn recheck_returns_text_field() {
        let mock = MockAtSpi { focused: Some(text_event()) };
        assert!(recheck_on_start(&mock).is_some());
    }

    #[test]
    fn recheck_ignores_password_and_nothing_focused() {
        let mut ev = text_event();
        ev.is_password = true;
        assert!(recheck_on_start(&MockAtSpi { focused: Some(ev) }).is_none());
        assert!(recheck_on_start(&MockAtSpi { focused: None }).is_none());
    }
}
