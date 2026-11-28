//! Task 9: synchronized text insertion (pure core + boundary).
//!
//! Insert the transcript once, at the cursor, in the *same* field recorded in.
//! Before inserting, verify the target is still focused + safe. Never insert
//! into read-only or password fields. Restore the user's clipboard. Atomic
//! ordering; no double insertion. Real backends (AT-SPI editable -> synthesized
//! keystroke fallback) are not wired yet; tests use `MockInserter`.

use crate::focus::FocusEvent;

/// Snapshot of the target field captured when recording started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetSnapshot {
    pub app: String,
    pub role: String,
    pub rect: crate::focus::Rect,
    pub editable: bool,
    pub read_only: bool,
    pub is_password: bool,
    pub enabled: bool,
}

impl TargetSnapshot {
    pub fn from_focus(ev: &FocusEvent) -> Self {
        Self {
            app: ev.app.clone(),
            role: ev.role.clone(),
            rect: ev.rect,
            editable: ev.editable,
            read_only: ev.read_only,
            is_password: ev.is_password,
            enabled: ev.enabled,
        }
    }
}

/// Is this a safe target right now? Never read-only/password/disabled.
pub fn target_is_safe(snap: &TargetSnapshot) -> bool {
    snap.enabled && snap.editable && !snap.read_only && !snap.is_password && !snap.rect.is_empty()
}

/// Is `current` still the same field we recorded in, and still safe/focused?
pub fn still_same_target(snap: &TargetSnapshot, current: &FocusEvent) -> bool {
    target_is_safe(snap)
        && current.is_text_field()
        && current.app == snap.app
        && current.role == snap.role
        && current.rect == snap.rect
}

/// Why insertion failed. Never contains secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertError {
    EmptyText,
    /// Field lost focus or changed; don't insert into the wrong place.
    TargetChanged,
    /// read-only / password / disabled / not a text field.
    UnsafeTarget,
    /// Backend failed; message redacted.
    Backend(String),
}

/// Text-insertion backend. Real impls try AT-SPI editable then keystroke.
pub trait Inserter {
    fn insert(&mut self, snapshot: &TargetSnapshot, text: &str) -> Result<(), InsertError>;
}

/// In-memory inserter for tests.
#[derive(Debug, Default)]
pub struct MockInserter {
    pub inserted: Vec<(String, String)>, // (app, text)
    pub fail: Option<InsertError>,
}

impl Inserter for MockInserter {
    fn insert(&mut self, snapshot: &TargetSnapshot, text: &str) -> Result<(), InsertError> {
        if let Some(e) = &self.fail {
            return Err(e.clone());
        }
        self.inserted.push((snapshot.app.clone(), text.to_string()));
        Ok(())
    }
}

/// Clipboard save/restore boundary.
pub trait Clipboard {
    fn save(&mut self) -> Option<String>;
    fn restore(&mut self, saved: Option<String>);
}

/// Insert once, in order, after verifying target; restore clipboard after.
pub fn insert_synchronized<I: Inserter, C: Clipboard>(
    inserter: &mut I,
    clipboard: &mut C,
    snapshot: &TargetSnapshot,
    current_focus: &FocusEvent,
    text: &str,
) -> Result<(), InsertError> {
    if text.is_empty() {
        return Err(InsertError::EmptyText);
    }
    if !target_is_safe(snapshot) {
        return Err(InsertError::UnsafeTarget);
    }
    if !still_same_target(snapshot, current_focus) {
        return Err(InsertError::TargetChanged);
    }

    let saved = clipboard.save();
    let result = inserter.insert(snapshot, text);
    clipboard.restore(saved); // always restore user clipboard
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::focus::Rect;

    #[derive(Default)]
    struct MockClipboard {
        saved: Option<String>,
        restored: Option<String>,
    }
    impl Clipboard for MockClipboard {
        fn save(&mut self) -> Option<String> {
            self.saved = Some("user-clip".into());
            self.saved.clone()
        }
        fn restore(&mut self, saved: Option<String>) {
            self.restored = saved;
        }
    }

    fn safe_focus() -> FocusEvent {
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
    fn inserts_once_into_same_safe_field_and_restores_clipboard() {
        let snap = TargetSnapshot::from_focus(&safe_focus());
        let mut ins = MockInserter::default();
        let mut clip = MockClipboard::default();
        insert_synchronized(&mut ins, &mut clip, &snap, &safe_focus(), "hello").unwrap();
        assert_eq!(ins.inserted, vec![("gedit".to_string(), "hello".to_string())]);
        assert_eq!(clip.restored, Some("user-clip".to_string()));
    }

    #[test]
    fn rejects_empty_text() {
        let snap = TargetSnapshot::from_focus(&safe_focus());
        let mut ins = MockInserter::default();
        let mut clip = MockClipboard::default();
        assert_eq!(
            insert_synchronized(&mut ins, &mut clip, &snap, &safe_focus(), ""),
            Err(InsertError::EmptyText)
        );
        assert!(ins.inserted.is_empty());
    }

    #[test]
    fn never_inserts_into_readonly_password_or_offscreen() {
        let base = safe_focus();
        // read-only
        let mut snap = TargetSnapshot::from_focus(&base);
        snap.read_only = true;
        let mut ins = MockInserter::default();
        let mut clip = MockClipboard::default();
        assert_eq!(
            insert_synchronized(&mut ins, &mut clip, &snap, &base, "x"),
            Err(InsertError::UnsafeTarget)
        );
        // password
        let mut snap = TargetSnapshot::from_focus(&base);
        snap.is_password = true;
        assert!(!target_is_safe(&snap));
        // not editable
        let mut snap = TargetSnapshot::from_focus(&base);
        snap.editable = false;
        assert!(!target_is_safe(&snap));
    }

    #[test]
    fn rejects_if_target_changed_or_focus_moved() {
        let snap = TargetSnapshot::from_focus(&safe_focus());
        let mut ins = MockInserter::default();
        let mut clip = MockClipboard::default();

        // focus moved to another field
        let mut moved = safe_focus();
        moved.rect = Rect::new(500, 500, 100, 20);
        assert_eq!(
            insert_synchronized(&mut ins, &mut clip, &snap, &moved, "x"),
            Err(InsertError::TargetChanged)
        );

        // focus moved to non-text
        let mut other = safe_focus();
        other.role = "push button".into();
        assert_eq!(
            insert_synchronized(&mut ins, &mut clip, &snap, &other, "x"),
            Err(InsertError::TargetChanged)
        );
        assert!(ins.inserted.is_empty());
    }

    #[test]
    fn restores_clipboard_even_on_backend_failure() {
        let snap = TargetSnapshot::from_focus(&safe_focus());
        let mut ins = MockInserter { inserted: vec![], fail: Some(InsertError::Backend("nope".into())) };
        let mut clip = MockClipboard::default();
        assert!(insert_synchronized(&mut ins, &mut clip, &snap, &safe_focus(), "x").is_err());
        assert_eq!(clip.restored, Some("user-clip".to_string())); // restored anyway
    }
}
