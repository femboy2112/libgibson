use crossterm::event::{
    self, Event as CtEvent, KeyCode as CtKeyCode, KeyModifiers as CtKeyModifiers,
};
use std::io;
use std::time::Duration;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct KeyModifiers: u8 {
        const SHIFT = 0b0000_0001;
        const CONTROL = 0b0000_0010;
        const ALT = 0b0000_0100;
    }
}

impl From<CtKeyModifiers> for KeyModifiers {
    fn from(m: CtKeyModifiers) -> Self {
        let mut flags = KeyModifiers::empty();
        if m.contains(CtKeyModifiers::SHIFT) {
            flags |= KeyModifiers::SHIFT;
        }
        if m.contains(CtKeyModifiers::CONTROL) {
            flags |= KeyModifiers::CONTROL;
        }
        if m.contains(CtKeyModifiers::ALT) {
            flags |= KeyModifiers::ALT;
        }
        flags
    }
}

/// Logical key code.
///
/// `#[non_exhaustive]`: the terminal key repertoire grows (function keys, Insert,
/// media keys, …), so downstream `match`es must include a `_` arm and adding a key
/// is a non-breaking `0.1.z` change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyCode {
    Char(char),
    Enter,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Backspace,
    Delete,
    Esc,
    Tab,
    BackTab,
}

/// A structured keyboard event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyEvent {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    pub fn char(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::empty(),
        }
    }
}

/// High-level input event.
///
/// `#[non_exhaustive]`: new event kinds are expected (mouse routing is planned;
/// focus, etc.), so downstream `match`es must include a `_` arm and adding an event
/// is a non-breaking `0.1.z` change.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event {
    Key(KeyEvent),
    Paste(String),
    Resize(u16, u16),
    Tick,
}

/// Polls for an event with a timeout.
pub fn poll_event(timeout: Duration) -> io::Result<Option<Event>> {
    if event::poll(timeout)? {
        let ev = event::read()?;
        Ok(match ev {
            CtEvent::Key(k) => {
                let code = match k.code {
                    CtKeyCode::Char(c) => KeyCode::Char(c),
                    CtKeyCode::Enter => KeyCode::Enter,
                    CtKeyCode::Left => KeyCode::Left,
                    CtKeyCode::Right => KeyCode::Right,
                    CtKeyCode::Up => KeyCode::Up,
                    CtKeyCode::Down => KeyCode::Down,
                    CtKeyCode::Home => KeyCode::Home,
                    CtKeyCode::End => KeyCode::End,
                    CtKeyCode::PageUp => KeyCode::PageUp,
                    CtKeyCode::PageDown => KeyCode::PageDown,
                    CtKeyCode::Backspace => KeyCode::Backspace,
                    CtKeyCode::Delete => KeyCode::Delete,
                    CtKeyCode::Esc => KeyCode::Esc,
                    CtKeyCode::Tab => KeyCode::Tab,
                    CtKeyCode::BackTab => KeyCode::BackTab,
                    _ => return Ok(None),
                };
                Some(Event::Key(KeyEvent {
                    code,
                    modifiers: k.modifiers.into(),
                }))
            }
            CtEvent::Paste(s) => Some(Event::Paste(s)),
            CtEvent::Resize(cols, rows) => Some(Event::Resize(cols, rows)),
            _ => None,
        })
    } else {
        Ok(None)
    }
}

/// Stateful grapheme-aware single-line text input buffer.
///
/// ## Invariant
///
/// After **every** public mutation:
///
/// ```text
/// cursor_grapheme <= text.graphemes(true).count()
/// ```
///
/// Mutations are expressed as byte-range edits and the cursor index is then
/// **re-derived from the segmentation of the complete resulting string**. This
/// matters because grapheme boundaries can merge across an insertion boundary
/// (for example inserting `U+0301 COMBINING ACUTE ACCENT` after `e` produces a
/// single `e◌́` cluster). The old "insert then add the inserted grapheme count"
/// strategy can leave the cursor pointing past the end of the buffer and panic
/// on the next backspace.
///
/// ## Single-line paste policy
///
/// The editor model is single-line, so bracketed paste **normalizes every line
/// break (`\r\n`, `\r`, `\n`) to a single space** before insertion. Multiline
/// content is therefore flattened rather than rejected, which preserves the
/// user's words without introducing newline graphemes into a one-row editor.
#[derive(Debug, Clone, Default)]
pub struct TextInputState {
    pub text: String,
    pub cursor_grapheme: usize,
    pub scroll_offset: usize,
}

impl TextInputState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_text(text: impl Into<String>) -> Self {
        let t = text.into();
        let count = t.graphemes(true).count();
        Self {
            text: t,
            cursor_grapheme: count,
            scroll_offset: 0,
        }
    }

    pub fn grapheme_count(&self) -> usize {
        self.text.graphemes(true).count()
    }

    /// Byte offset of the cursor. Always a valid UTF-8 char boundary.
    fn cursor_byte_pos(&self) -> usize {
        self.text
            .grapheme_indices(true)
            .nth(self.cursor_grapheme)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len())
    }

    /// Re-derives `cursor_grapheme` from a byte offset in the current string.
    fn sync_cursor_from_byte(&mut self, byte: usize) {
        let byte = byte.min(self.text.len());
        debug_assert!(self.text.is_char_boundary(byte));
        self.cursor_grapheme = self.text[..byte].graphemes(true).count();
    }

    /// Clamps the cursor into range. Cheap safety net for deserialized/foreign state.
    pub fn normalize(&mut self) {
        let count = self.grapheme_count();
        if self.cursor_grapheme > count {
            self.cursor_grapheme = count;
        }
    }

    /// Normalizes a paste payload to a single line (see type-level docs).
    pub fn normalize_paste(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut last_was_break = false;
        for c in s.chars() {
            if c == '\r' || c == '\n' {
                if !last_was_break {
                    out.push(' ');
                    last_was_break = true;
                }
            } else {
                last_was_break = false;
                out.push(c);
            }
        }
        out
    }

    /// Inserts a string at the current grapheme position.
    pub fn insert_str(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.normalize();
        let byte_pos = self.cursor_byte_pos();
        self.text.insert_str(byte_pos, s);
        // Re-derive from the *complete* resulting string so boundary merges are respected.
        self.sync_cursor_from_byte(byte_pos + s.len());
        self.normalize();
    }

    /// Inserts a single char at the current grapheme position.
    pub fn insert_char(&mut self, c: char) {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        self.insert_str(s);
    }

    /// Deletes the grapheme cluster preceding the cursor (Backspace).
    pub fn backspace(&mut self) -> bool {
        self.normalize();
        if self.cursor_grapheme == 0 {
            return false;
        }

        let cur_byte = self.cursor_byte_pos();
        let prev_start = self.text[..cur_byte]
            .grapheme_indices(true)
            .next_back()
            .map(|(i, _)| i);

        match prev_start {
            Some(start) => {
                self.text.replace_range(start..cur_byte, "");
                self.sync_cursor_from_byte(start);
                true
            }
            None => false,
        }
    }

    /// Deletes the grapheme cluster at the cursor (Delete).
    pub fn delete(&mut self) -> bool {
        self.normalize();
        let cur_byte = self.cursor_byte_pos();
        if cur_byte >= self.text.len() {
            return false;
        }

        let next_end = self.text[cur_byte..]
            .grapheme_indices(true)
            .nth(1)
            .map(|(i, _)| cur_byte + i)
            .unwrap_or(self.text.len());

        self.text.replace_range(cur_byte..next_end, "");
        self.sync_cursor_from_byte(cur_byte);
        true
    }

    pub fn move_left(&mut self) -> bool {
        self.normalize();
        if self.cursor_grapheme > 0 {
            self.cursor_grapheme -= 1;
            true
        } else {
            false
        }
    }

    pub fn move_right(&mut self) -> bool {
        self.normalize();
        if self.cursor_grapheme < self.grapheme_count() {
            self.cursor_grapheme += 1;
            true
        } else {
            false
        }
    }

    pub fn move_to_start(&mut self) {
        self.cursor_grapheme = 0;
    }

    pub fn move_to_end(&mut self) {
        self.cursor_grapheme = self.grapheme_count();
    }

    /// Handles keyboard and paste events. Returns true if modified.
    pub fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::Key(k) => match k.code {
                KeyCode::Char(c) => {
                    if k.modifiers.contains(KeyModifiers::CONTROL) {
                        match c {
                            'a' => {
                                self.move_to_start();
                                true
                            }
                            'e' => {
                                self.move_to_end();
                                true
                            }
                            'u' => {
                                // Clear line before cursor
                                self.normalize();
                                let byte_pos = self.cursor_byte_pos();
                                self.text.replace_range(..byte_pos, "");
                                self.cursor_grapheme = 0;
                                true
                            }
                            'k' => {
                                // Clear line after cursor
                                self.normalize();
                                let byte_pos = self.cursor_byte_pos();
                                self.text.truncate(byte_pos);
                                true
                            }
                            _ => false,
                        }
                    } else {
                        self.insert_char(c);
                        true
                    }
                }
                KeyCode::Backspace => self.backspace(),
                KeyCode::Delete => self.delete(),
                KeyCode::Left => self.move_left(),
                KeyCode::Right => self.move_right(),
                KeyCode::Home => {
                    self.move_to_start();
                    true
                }
                KeyCode::End => {
                    self.move_to_end();
                    true
                }
                _ => false,
            },
            Event::Paste(s) => {
                let flat = Self::normalize_paste(s);
                self.insert_str(&flat);
                true
            }
            _ => false,
        }
    }

    /// Adjusts viewport scroll offset (in display columns) so the cursor is visible within `visible_width`.
    pub fn update_scroll(&mut self, visible_width: usize) {
        if visible_width == 0 {
            return;
        }

        let cursor_col = self.cursor_display_column();

        if cursor_col < self.scroll_offset {
            self.scroll_offset = cursor_col;
        } else if cursor_col >= self.scroll_offset + visible_width {
            self.scroll_offset = cursor_col.saturating_sub(visible_width) + 1;
        }
    }

    /// Returns the cursor position in monospace terminal display columns.
    pub fn cursor_display_column(&self) -> usize {
        let graphemes: Vec<&str> = self.text.graphemes(true).collect();
        let mut col = 0;
        for (i, &g) in graphemes.iter().enumerate() {
            if i >= self.cursor_grapheme {
                break;
            }
            col += UnicodeWidthStr::width(g).max(1);
        }
        col
    }

    /// Returns the total display width of the buffer in terminal columns.
    pub fn total_display_width(&self) -> usize {
        UnicodeWidthStr::width(self.text.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_input_typing() {
        let mut input = TextInputState::new();
        input.handle_event(&Event::Key(KeyEvent::char('a')));
        input.handle_event(&Event::Key(KeyEvent::char('b')));
        input.handle_event(&Event::Key(KeyEvent::char('c')));

        assert_eq!(input.text, "abc");
        assert_eq!(input.cursor_grapheme, 3);

        input.handle_event(&Event::Key(KeyEvent::new(
            KeyCode::Backspace,
            KeyModifiers::empty(),
        )));
        assert_eq!(input.text, "ab");
        assert_eq!(input.cursor_grapheme, 2);
    }

    #[test]
    fn test_text_input_grapheme_combining_backspace() {
        let mut input = TextInputState::new();
        // Insert 'e' followed by combining acute accent
        input.insert_str("e\u{0301}");
        assert_eq!(input.grapheme_count(), 1);
        assert_eq!(input.cursor_grapheme, 1);

        // A single backspace MUST delete the entire combining character
        input.backspace();
        assert_eq!(input.text, "");
        assert_eq!(input.cursor_grapheme, 0);
    }

    #[test]
    fn test_text_input_emoji_navigation() {
        let mut input = TextInputState::new();
        input.insert_str("🦀🦀");
        assert_eq!(input.grapheme_count(), 2);
        assert_eq!(input.cursor_grapheme, 2);

        input.move_left();
        assert_eq!(input.cursor_grapheme, 1);

        input.insert_str("x");
        assert_eq!(input.text, "🦀x🦀");
        assert_eq!(input.cursor_grapheme, 2);
    }

    #[test]
    fn test_bracketed_paste() {
        let mut input = TextInputState::new();
        input.insert_str("start-");
        input.handle_event(&Event::Paste("pasted content".to_string()));
        input.insert_str("-end");

        assert_eq!(input.text, "start-pasted content-end");
    }

    #[test]
    fn test_text_input_mixed_display_width_scrolling() {
        let mut input = TextInputState::new();
        // 🦀 = 2 cols, A = 1 col, 你 = 2 cols -> 5 cols total
        input.insert_str("🦀A你");
        assert_eq!(input.cursor_grapheme, 3);
        assert_eq!(input.cursor_display_column(), 5);

        // Visible width of 3 columns
        input.update_scroll(3);
        // cursor is at col 5, visible window must contain 5, so scroll_offset = 5 - 3 + 1 = 3
        assert_eq!(input.scroll_offset, 3);

        // Move cursor back to beginning
        input.move_to_start();
        assert_eq!(input.cursor_display_column(), 0);
        input.update_scroll(3);
        assert_eq!(input.scroll_offset, 0);
    }

    // ---------------------------------------------------------------------
    // Unicode mutation correctness (merge-across-boundary regressions)
    // ---------------------------------------------------------------------

    fn assert_invariant(input: &TextInputState) {
        assert!(
            input.cursor_grapheme <= input.grapheme_count(),
            "cursor {} exceeded grapheme_count {} for {:?}",
            input.cursor_grapheme,
            input.grapheme_count(),
            input.text
        );
    }

    #[test]
    fn test_combining_accent_inserted_independently_merges() {
        // The canonical adversarial case from the hardening brief:
        // type 'e', then insert U+0301. Inserted alone it is one cluster, but
        // e + combining acute is ONE extended grapheme cluster.
        let mut input = TextInputState::new();
        input.insert_char('e');
        assert_eq!(input.cursor_grapheme, 1);
        assert_eq!(input.grapheme_count(), 1);

        input.insert_char('\u{0301}');
        // Must be re-segmented: one cluster, cursor at 1 -- never 2!
        assert_eq!(input.text, "e\u{0301}");
        assert_eq!(input.grapheme_count(), 1);
        assert_eq!(input.cursor_grapheme, 1);
        assert_invariant(&input);

        // A backspace must remove the whole cluster and must not panic.
        assert!(input.backspace());
        assert_eq!(input.text, "");
        assert_eq!(input.cursor_grapheme, 0);
    }

    #[test]
    fn test_insert_before_following_grapheme_merges_backwards() {
        // Cursor in the middle: inserting a combining mark must merge with the
        // *preceding* character, not the following one.
        let mut input = TextInputState::with_text("ab");
        input.move_left(); // cursor between 'a' and 'b' (index 1)
        input.insert_char('\u{0301}'); // "a" + combining acute + "b"
        assert_eq!(input.text, "a\u{0301}b");
        assert_eq!(input.grapheme_count(), 2); // "á", "b"
        assert_eq!(input.cursor_grapheme, 1);
        assert_invariant(&input);

        // Typing 'b' again yields "áb"; deleting forward removes 'b'.
        assert!(input.delete());
        assert_eq!(input.text, "a\u{0301}");
    }

    #[test]
    fn test_emoji_skin_tone_modifier_merges() {
        let mut input = TextInputState::new();
        input.insert_str("👍");
        assert_eq!(input.grapheme_count(), 1);
        input.insert_str("🏽"); // fitzpatrick type-5 modifier
        assert_eq!(input.grapheme_count(), 1);
        assert_eq!(input.cursor_grapheme, 1);
        assert_invariant(&input);
    }

    #[test]
    fn test_zwj_emoji_sequence_merges() {
        // 👩 + ZWJ + 💻 = woman technologist (one cluster)
        let mut input = TextInputState::new();
        input.insert_str("👩");
        input.insert_char('\u{200D}');
        assert_eq!(input.grapheme_count(), 1);
        input.insert_str("💻");
        assert_eq!(input.grapheme_count(), 1);
        assert_eq!(input.cursor_grapheme, 1);
        assert_invariant(&input);
    }

    #[test]
    fn test_regional_indicator_flags() {
        let mut input = TextInputState::new();
        // Regional indicator U and S separately form a flag; two at a time pair up.
        input.insert_str("\u{1F1FA}\u{1F1F8}"); // 🇺🇸
        assert_eq!(input.grapheme_count(), 1);
        assert_eq!(input.cursor_grapheme, 1);
        input.insert_str("\u{1F1EF}\u{1F1F5}"); // 🇯🇵
        assert_eq!(input.grapheme_count(), 2);
        assert_eq!(input.cursor_grapheme, 2);
        assert_invariant(&input);
    }

    #[test]
    fn test_mixed_ascii_cjk_emoji_arbitrary_edits_keep_invariant() {
        // Deterministic randomized edit fuzzing with a small LCG.
        let alphabet: Vec<&str> = vec![
            "a",
            "Z",
            " ",
            "é",
            "\u{0301}",
            "\u{0308}",
            "你",
            "好",
            "🦀",
            "👍",
            "🏽",
            "👩",
            "\u{200D}",
            "💻",
            "\u{1F1FA}",
            "\u{1F1F8}",
            "\u{1F1EF}",
            "\u{1F1F5}",
        ];
        let mut state: u64 = 0x9E3779B97F4A7C15;
        let mut input = TextInputState::new();

        for step in 0..4000u64 {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let op = (state >> 33) % 8;
            match op {
                0 => {
                    let g = alphabet[((state >> 8) as usize) % alphabet.len()];
                    input.insert_str(g);
                }
                1 => {
                    input.backspace();
                }
                2 => {
                    input.delete();
                }
                3 => {
                    input.move_left();
                }
                4 => {
                    input.move_right();
                }
                5 => {
                    input.move_to_start();
                }
                6 => {
                    input.move_to_end();
                }
                _ => {
                    let g = alphabet[((state >> 16) as usize) % alphabet.len()];
                    input.handle_event(&Event::Key(KeyEvent::char(g.chars().next().unwrap())));
                    // also exercise control-U/control-K occasionally
                    if step % 7 == 0 {
                        input.handle_event(&Event::Key(KeyEvent::new(
                            KeyCode::Char('u'),
                            KeyModifiers::CONTROL,
                        )));
                    }
                }
            }
            assert_invariant(&input);
        }
    }

    #[test]
    fn test_single_line_paste_normalizes_line_breaks() {
        let mut input = TextInputState::new();
        input.handle_event(&Event::Paste(
            "line one\nline two\r\nline three\rline four".into(),
        ));
        assert_eq!(input.text, "line one line two line three line four");
        assert!(!input.text.contains('\n'));
        assert!(!input.text.contains('\r'));
        assert_invariant(&input);
    }

    #[test]
    fn test_paste_merges_with_existing_combining_sequence() {
        let mut input = TextInputState::with_text("e");
        input.handle_event(&Event::Paste("\u{0301}".into()));
        assert_eq!(input.grapheme_count(), 1);
        assert_eq!(input.cursor_grapheme, 1);
        assert_invariant(&input);
    }
}
