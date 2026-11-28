//! Task 6: mic overlay placement + visibility (pure core).
//!
//! Custom-drawn mic, **no background circle**, right side, vertically centered
//! (ARCHITECTURE §2). Shown only when VoiceType is on AND a safe text field is
//! focused AND it has non-empty geometry. The actual transparent overlay
//! window (GTK / X11, Wayland adapter later) is not wired yet; this keeps
//! placement + visibility testable without a desktop.

use crate::focus::{FocusEvent, Rect};

/// Default mic glyph size in px.
pub const MIC_SIZE: i32 = 20;
/// Default inset from the field's right edge in px.
pub const MIC_MARGIN: i32 = 4;

/// Whether the mic is currently shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MicVisibility {
    #[default]
    Hidden,
    Shown,
}

/// Compute where to draw the mic: right side, vertically centered, inset by margin.
pub fn mic_rect(field: &Rect, mic_size: i32, margin: i32) -> Rect {
    Rect::new(
        field.right() - margin - mic_size,
        field.center_y() - mic_size / 2,
        mic_size,
        mic_size,
    )
}

/// Should the mic be shown? on + safe text field + non-empty geometry.
pub fn should_show(voicetype_on: bool, focus: Option<&FocusEvent>) -> bool {
    voicetype_on && focus.map(|f| f.is_text_field()).unwrap_or(false)
}

/// Overlay state.
#[derive(Debug, Default)]
pub struct Overlay {
    visibility: MicVisibility,
    rect: Option<Rect>,
}

impl Overlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn visibility(&self) -> MicVisibility {
        self.visibility
    }

    pub fn rect(&self) -> Option<Rect> {
        self.rect
    }

    /// Recompute visibility + placement. `mic_size`/`margin` in px.
    pub fn update(&mut self, voicetype_on: bool, focus: Option<&FocusEvent>, mic_size: i32, margin: i32) {
        if should_show(voicetype_on, focus) {
            let field = &focus.expect("checked").rect;
            self.visibility = MicVisibility::Shown;
            self.rect = Some(mic_rect(field, mic_size, margin));
        } else {
            self.visibility = MicVisibility::Hidden;
            self.rect = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::focus::FocusEvent;

    fn text_event() -> FocusEvent {
        FocusEvent {
            app: "x".into(),
            role: "entry".into(),
            rect: Rect::new(100, 100, 200, 30),
            editable: true,
            enabled: true,
            ..Default::default()
        }
    }

    #[test]
    fn mic_is_right_edge_vertically_centered() {
        let field = Rect::new(100, 100, 200, 30); // right=300, center_y=115
        let mic = mic_rect(&field, MIC_SIZE, MIC_MARGIN);
        assert_eq!(mic.x, 300 - MIC_MARGIN - MIC_SIZE); // 276
        assert_eq!(mic.y, 115 - MIC_SIZE / 2); // 105
        assert_eq!(mic.width, MIC_SIZE);
        assert_eq!(mic.height, MIC_SIZE);
    }

    #[test]
    fn hidden_when_off_or_no_field() {
        assert!(!should_show(false, Some(&text_event())));
        assert!(!should_show(true, None));
    }

    #[test]
    fn shown_only_for_safe_text_field() {
        assert!(should_show(true, Some(&text_event())));
        let mut pw = text_event();
        pw.is_password = true;
        assert!(!should_show(true, Some(&pw)));
    }

    #[test]
    fn overlay_update_toggles() {
        let mut o = Overlay::new();
        o.update(true, Some(&text_event()), MIC_SIZE, MIC_MARGIN);
        assert_eq!(o.visibility(), MicVisibility::Shown);
        assert!(o.rect().is_some());
        o.update(false, Some(&text_event()), MIC_SIZE, MIC_MARGIN);
        assert_eq!(o.visibility(), MicVisibility::Hidden);
        assert!(o.rect().is_none());
    }
}
