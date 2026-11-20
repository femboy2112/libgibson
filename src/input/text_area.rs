//! Multi-line text editing state: EXPERIMENTAL, additive, v0.9.1 design.
//!
//! **RED slice.** Everything here compiles and is deliberately wrong; the contract
//! lives in `docs/design/v0.9.1-text-area.md` and is pinned by `tests/text_area.rs`.
//! [`TextInputState`](super::TextInputState) is single-line by contract (issue #252)
//! and is not touched by this type.
//!
//! `TextAreaState` never commits. Enter inserts `'\n'`; the consumer provides an
//! explicit Save control and owns cancel/restore.

use super::Event;
use unicode_segmentation::UnicodeSegmentation;

/// One visual (wrapped) row of a [`TextAreaState`] buffer, in grapheme indices.
///
/// `start..end` are the graphemes painted on the row, excluding the `'\n'`
/// grapheme that ends a hard line (that grapheme sits at index `end`); `width` is
/// the row's width in terminal cells under the editor width contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualRow {
    pub start: usize,
    pub end: usize,
    pub width: usize,
}

/// A multi-line editing buffer with a wrapped-row cursor model.
///
/// EXPERIMENTAL (v0.9.1 design). See `docs/design/v0.9.1-text-area.md`.
#[derive(Debug, Clone, Default)]
pub struct TextAreaState {
    pub text: String,
    pub cursor_grapheme: usize,
    /// First visible visual row (not a logical line).
    pub scroll_row: usize,
    /// The visual column Up/Down try to return to across shorter rows.
    pub goal_col: Option<usize>,
}

impl TextAreaState {
    pub fn new() -> Self {
        Self::default()
    }

    /// A state holding `text` with the cursor at its end.
    // RED stub: v0.9.1 design, not implemented (text is kept raw, unsanitized).
    pub fn with_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor_grapheme = text.graphemes(true).count();
        Self {
            text,
            cursor_grapheme,
            scroll_row: 0,
            goal_col: None,
        }
    }

    pub fn grapheme_count(&self) -> usize {
        self.text.graphemes(true).count()
    }

    /// The text a paste (or `insert_str`) actually inserts: `'\n'` kept, `\r\n` and
    /// lone `\r` normalized to `'\n'`, every other C0/C1 control and every
    /// ESC/C1-introduced sequence removed.
    // RED stub: v0.9.1 design, not implemented (returns the input unchanged).
    pub fn normalize_paste(s: &str) -> String {
        s.to_string()
    }

    /// Inserts `s` at the cursor, sanitized by [`normalize_paste`](Self::normalize_paste).
    // RED stub: v0.9.1 design, not implemented (inserts raw text).
    pub fn insert_str(&mut self, s: &str) {
        let at = self
            .text
            .grapheme_indices(true)
            .nth(self.cursor_grapheme)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len());
        self.text.insert_str(at, s);
        self.cursor_grapheme = self.text[..at + s.len()].graphemes(true).count();
    }

    /// Handles a key or paste event for a field `width` cells wide and `rows` rows
    /// tall (`rows` is the PageUp/PageDown stride). Returns true if the event was
    /// handled (buffer, cursor or goal column may have changed). Never signals a
    /// commit. Does not scroll: call [`update_scroll`](Self::update_scroll) after.
    // RED stub: v0.9.1 design, not implemented (only Paste, unsanitized).
    pub fn handle_event(&mut self, event: &Event, width: u16, rows: u16) -> bool {
        let _ = (width, rows);
        if let Event::Paste(s) = event {
            let before = self.text.len();
            self.insert_str(s);
            self.text.len() != before
        } else {
            false
        }
    }

    /// THE wrap function: the visual rows of the buffer at `width` cells. The
    /// cursor model, movement and the painting path all use this and nothing else.
    // RED stub: v0.9.1 design, not implemented (one row spanning the whole buffer).
    pub fn visual_rows(&self, width: u16) -> Vec<VisualRow> {
        let _ = width;
        vec![VisualRow {
            start: 0,
            end: self.grapheme_count(),
            width: 0,
        }]
    }

    /// The cursor's visual `(row, column in cells)` at `width`.
    // RED stub: v0.9.1 design, not implemented.
    pub fn cursor_rc(&self, width: u16) -> (usize, usize) {
        let _ = width;
        (0, 0)
    }

    /// Adjusts `scroll_row` so the cursor row is inside `rows` visible rows.
    // RED stub: v0.9.1 design, not implemented.
    pub fn update_scroll(&mut self, width: u16, rows: u16) {
        let _ = (width, rows);
    }
}
