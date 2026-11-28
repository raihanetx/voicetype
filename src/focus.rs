//! Task 5 (pure core): focus model + text-field classification.
//!
//! No AT-SPI here. This is the testable logic the AT-SPI adapter feeds:
//! role + state + geometry in, "is this an editable text field?" out.
//! Never treat read-only or password fields as insertion targets.

/// On-screen rectangle of a field, in screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self { x, y, width, height }
    }

    /// Right edge (where the mic would sit).
    pub fn right(&self) -> i32 {
        self.x + self.width
    }

    /// Vertical center.
    pub fn center_y(&self) -> i32 {
        self.y + self.height / 2
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0 || self.height <= 0
    }
}

/// What kind of field this is, for safe insertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// Editable text field; safe insertion target.
    Text,
    /// Password field; never insert, never show mic.
    Password,
    /// Text present but read-only; never insert.
    ReadOnly,
    /// Not a text field.
    NonText,
}

/// Focus information reported by the accessibility layer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FocusEvent {
    pub app: String,
    pub role: String,
    pub rect: Rect,
    pub editable: bool,
    pub read_only: bool,
    pub is_password: bool,
    pub enabled: bool,
}

/// AT-SPI role names (lowercased) that represent text-ish widgets.
const TEXT_ROLES: &[&str] = &[
    "entry",
    "text",
    "terminal",
    "search",
    "editbar",
    "paragraph",
    "document text",
    "document-text",
    "text frame",
    "spin button",
    "combo box",
    "autocomplete",
];

pub fn role_is_text(role: &str) -> bool {
    let r = role.trim().to_ascii_lowercase();
    TEXT_ROLES.iter().any(|t| *t == r) || r.contains("text") || r.contains("entry")
}

/// Classify a field. Order matters: password wins, then read-only, then editable.
pub fn classify(
    role: &str,
    editable: bool,
    read_only: bool,
    is_password: bool,
    enabled: bool,
) -> FieldKind {
    if is_password || role.trim().eq_ignore_ascii_case("password text") {
        return FieldKind::Password;
    }
    if !enabled || !role_is_text(role) {
        return FieldKind::NonText;
    }
    if read_only || !editable {
        return FieldKind::ReadOnly;
    }
    FieldKind::Text
}

impl FocusEvent {
    pub fn kind(&self) -> FieldKind {
        classify(&self.role, self.editable, self.read_only, self.is_password, self.enabled)
    }

    /// Should VoiceType show the mic and allow insertion here?
    pub fn is_text_field(&self) -> bool {
        self.kind() == FieldKind::Text && !self.rect.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_geometry() {
        let r = Rect::new(10, 20, 100, 30);
        assert_eq!(r.right(), 110);
        assert_eq!(r.center_y(), 35);
        assert!(!r.is_empty());
        assert!(Rect::new(0, 0, 0, 10).is_empty());
    }

    #[test]
    fn editable_text_is_target() {
        assert_eq!(classify("entry", true, false, false, true), FieldKind::Text);
        assert_eq!(classify("text", true, false, false, true), FieldKind::Text);
        assert_eq!(classify("terminal", true, false, false, true), FieldKind::Text);
    }

    #[test]
    fn password_is_never_target() {
        assert_eq!(classify("entry", true, false, true, true), FieldKind::Password);
        assert_eq!(classify("password text", true, false, false, true), FieldKind::Password);
    }

    #[test]
    fn read_only_is_not_target() {
        assert_eq!(classify("text", false, true, false, true), FieldKind::ReadOnly);
        assert_eq!(classify("text", true, true, false, true), FieldKind::ReadOnly);
    }

    #[test]
    fn non_text_and_disabled_are_not_targets() {
        assert_eq!(classify("push button", false, false, false, true), FieldKind::NonText);
        assert_eq!(classify("entry", true, false, false, false), FieldKind::NonText);
    }

    #[test]
    fn focus_event_is_text_field_requires_geometry() {
        let mut ev = FocusEvent {
            app: "gedit".into(),
            role: "text".into(),
            rect: Rect::new(0, 0, 200, 24),
            editable: true,
            enabled: true,
            ..Default::default()
        };
        assert!(ev.is_text_field());
        ev.rect = Rect::new(0, 0, 0, 0);
        assert!(!ev.is_text_field());
        ev.rect = Rect::new(0, 0, 200, 24);
        ev.is_password = true;
        assert!(!ev.is_text_field());
    }
}
