use crossterm::event::{
    self, Event as CtEvent, KeyCode as CtKeyCode, KeyModifiers as CtKeyModifiers,
};
use std::io;
use std::time::Duration;
use unicode_segmentation::UnicodeSegmentation;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    Enter,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
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
#[derive(Debug, Clone, PartialEq, Eq)]
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

    /// Inserts a string at the current grapheme position.
    pub fn insert_str(&mut self, s: &str) {
        let graphemes: Vec<&str> = self.text.graphemes(true).collect();
        let byte_pos = if self.cursor_grapheme >= graphemes.len() {
            self.text.len()
        } else {
            graphemes[..self.cursor_grapheme]
                .iter()
                .map(|g| g.len())
                .sum()
        };

        self.text.insert_str(byte_pos, s);
        self.cursor_grapheme += s.graphemes(true).count();
    }

    /// Inserts a single char at the current grapheme position.
    pub fn insert_char(&mut self, c: char) {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        self.insert_str(s);
    }

    /// Deletes the grapheme cluster preceding the cursor (Backspace).
    pub fn backspace(&mut self) -> bool {
        if self.cursor_grapheme == 0 {
            return false;
        }

        let graphemes: Vec<&str> = self.text.graphemes(true).collect();
        let target_idx = self.cursor_grapheme - 1;

        let start_byte: usize = graphemes[..target_idx].iter().map(|g| g.len()).sum();
        let end_byte = start_byte + graphemes[target_idx].len();

        self.text.replace_range(start_byte..end_byte, "");
        self.cursor_grapheme -= 1;
        true
    }

    /// Deletes the grapheme cluster at the cursor (Delete).
    pub fn delete(&mut self) -> bool {
        let graphemes: Vec<&str> = self.text.graphemes(true).collect();
        if self.cursor_grapheme >= graphemes.len() {
            return false;
        }

        let start_byte: usize = graphemes[..self.cursor_grapheme]
            .iter()
            .map(|g| g.len())
            .sum();
        let end_byte = start_byte + graphemes[self.cursor_grapheme].len();

        self.text.replace_range(start_byte..end_byte, "");
        true
    }

    pub fn move_left(&mut self) -> bool {
        if self.cursor_grapheme > 0 {
            self.cursor_grapheme -= 1;
            true
        } else {
            false
        }
    }

    pub fn move_right(&mut self) -> bool {
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
                                let graphemes: Vec<&str> = self.text.graphemes(true).collect();
                                let byte_pos: usize = graphemes[..self.cursor_grapheme]
                                    .iter()
                                    .map(|g| g.len())
                                    .sum();
                                self.text.replace_range(..byte_pos, "");
                                self.cursor_grapheme = 0;
                                true
                            }
                            'k' => {
                                // Clear line after cursor
                                let graphemes: Vec<&str> = self.text.graphemes(true).collect();
                                let byte_pos: usize = graphemes[..self.cursor_grapheme]
                                    .iter()
                                    .map(|g| g.len())
                                    .sum();
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
                self.insert_str(s);
                true
            }
            _ => false,
        }
    }

    /// Adjusts viewport scroll offset so the cursor is visible within `visible_width`.
    pub fn update_scroll(&mut self, visible_width: usize) {
        if visible_width == 0 {
            return;
        }

        if self.cursor_grapheme < self.scroll_offset {
            self.scroll_offset = self.cursor_grapheme;
        } else if self.cursor_grapheme >= self.scroll_offset + visible_width {
            self.scroll_offset = self.cursor_grapheme - visible_width + 1;
        }
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
}
