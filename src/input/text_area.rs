//! Multi-line text editing state: EXPERIMENTAL, additive, v0.9.1.
//!
//! The contract lives in `docs/design/v0.9.1-text-area.md` and is pinned by
//! `tests/text_area.rs`. [`TextInputState`](super::TextInputState) is single-line by
//! contract (issue #252) and is not touched by this type.
//!
//! `TextAreaState` never commits. Enter inserts `'\n'`; the consumer provides an
//! explicit Save control and owns cancel/restore.

use super::{editor_cell_width, editor_cells, Event, KeyCode, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;

/// Cells a TAB occupies: one grapheme, fixed width, no tab stops, for wrap, cursor and
/// painter alike (the buffer keeps the `'\t'` byte-exact).
const TAB_CELLS: usize = 4;

/// A line break grapheme (a `"\r\n"` cluster also breaks).
fn is_break(g: &str) -> bool {
    g == "\n" || g == "\r\n"
}

/// Cells of one grapheme in a text area: the editor width contract, except a TAB.
fn cell_width(g: &str) -> usize {
    if g == "\t" {
        TAB_CELLS
    } else {
        editor_cell_width(g)
    }
}

/// How the text area places grapheme `g`: like [`editor_cells`], but a TAB is four
/// blank cells. Used by the painter so it agrees with the wrap function cell for cell.
pub(crate) fn area_cells(g: &str, mut cell: impl FnMut(&str, usize)) {
    if g == "\t" {
        for _ in 0..TAB_CELLS {
            cell(" ", 1);
        }
    } else {
        editor_cells(g, cell);
    }
}

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
/// EXPERIMENTAL (v0.9.1). See `docs/design/v0.9.1-text-area.md`.
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

    /// A state holding `text` (sanitized as a paste) with the cursor at its end.
    pub fn with_text(text: impl Into<String>) -> Self {
        let raw: String = text.into();
        let text = Self::normalize_paste(&raw);
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

    /// The text a paste (or `insert_str`) actually inserts: `'\n'` and `'\t'` kept,
    /// `\r\n`, lone `\r`, U+2028 and U+2029 normalized to `'\n'`, every other C0/C1
    /// control and every ESC/C1-introduced sequence removed.
    pub fn normalize_paste(s: &str) -> String {
        let cs: Vec<char> = s.chars().collect();
        let mut out = String::with_capacity(s.len());
        let mut i = 0;
        while i < cs.len() {
            let c = cs[i];
            i += 1;
            match c {
                '\r' => {
                    out.push('\n');
                    if cs.get(i) == Some(&'\n') {
                        i += 1;
                    }
                }
                '\n' | '\u{2028}' | '\u{2029}' => out.push('\n'),
                '\t' => out.push('\t'),
                '\u{1b}' => i = skip_escape(&cs, i),
                '\u{9b}' => i = skip_csi(&cs, i),
                '\u{9d}' => i = skip_string(&cs, i, true),
                '\u{90}' | '\u{98}' | '\u{9e}' | '\u{9f}' => i = skip_string(&cs, i, false),
                c if c.is_control() => {}
                c => out.push(c),
            }
        }
        out
    }

    /// Inserts `s` at the cursor, sanitized by [`normalize_paste`](Self::normalize_paste).
    pub fn insert_str(&mut self, s: &str) {
        let clean = Self::normalize_paste(s);
        if clean.is_empty() {
            return;
        }
        let at = self.byte_at(self.cursor_grapheme);
        self.text.insert_str(at, &clean);
        self.cursor_grapheme = self.text[..at + clean.len()].graphemes(true).count();
        self.goal_col = None;
    }

    /// Handles a key or paste event for a field `width` cells wide and `rows` rows
    /// tall (`rows` is the PageUp/PageDown stride). Returns true if the event was
    /// handled (buffer, cursor or goal column may have changed). Never signals a
    /// commit. Does not scroll: call [`update_scroll`](Self::update_scroll) after.
    pub fn handle_event(&mut self, event: &Event, width: u16, rows: u16) -> bool {
        match event {
            Event::Paste(s) => self.insert_changed(s),
            Event::Key(k) => {
                let m = k.modifiers;
                match k.code {
                    KeyCode::Char(c) => {
                        if m.contains(KeyModifiers::ALT) {
                            false
                        } else if m.contains(KeyModifiers::CONTROL) {
                            match c {
                                'a' => self.move_in_row(width, false),
                                'e' => self.move_in_row(width, true),
                                _ => false,
                            }
                        } else {
                            self.insert_changed(c.encode_utf8(&mut [0; 4]))
                        }
                    }
                    KeyCode::Enter => {
                        if m.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
                            false
                        } else {
                            self.insert_str("\n");
                            true
                        }
                    }
                    KeyCode::Backspace => self.backspace(),
                    KeyCode::Delete => self.delete(),
                    KeyCode::Left => self.step(false),
                    KeyCode::Right => self.step(true),
                    KeyCode::Up => self.move_rows(width, false, 1),
                    KeyCode::Down => self.move_rows(width, true, 1),
                    KeyCode::PageUp => self.move_rows(width, false, usize::from(rows.max(1))),
                    KeyCode::PageDown => self.move_rows(width, true, usize::from(rows.max(1))),
                    KeyCode::Home => self.move_in_row(width, false),
                    KeyCode::End => self.move_in_row(width, true),
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// THE wrap function: the visual rows of the buffer at `width` cells. The
    /// cursor model, movement and the painting path all use this and nothing else.
    pub fn visual_rows(&self, width: u16) -> Vec<VisualRow> {
        let gs: Vec<&str> = self.text.graphemes(true).collect();
        wrap(&gs, width)
    }

    /// The cursor's visual `(row, column in cells)` at `width`.
    pub fn cursor_rc(&self, width: u16) -> (usize, usize) {
        let gs: Vec<&str> = self.text.graphemes(true).collect();
        let rows = wrap(&gs, width);
        self.rc_in(&gs, &rows)
    }

    /// Adjusts `scroll_row` so the cursor row is inside `rows` visible rows, moving
    /// as little as possible. `rows == 0` or `width == 0` is a no-op.
    pub fn update_scroll(&mut self, width: u16, rows: u16) {
        if width == 0 || rows == 0 {
            return;
        }
        let gs: Vec<&str> = self.text.graphemes(true).collect();
        let vrows = wrap(&gs, width);
        let (row, _) = self.rc_in(&gs, &vrows);
        let visible = usize::from(rows);
        let scroll = self.scroll_row.min(vrows.len() - 1);
        self.scroll_row = if row < scroll {
            row
        } else if row >= scroll + visible {
            row + 1 - visible
        } else {
            scroll
        };
    }

    fn byte_at(&self, g: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .nth(g)
            .map_or(self.text.len(), |(i, _)| i)
    }

    /// Inserts `s` (sanitized) and reports whether the buffer changed.
    fn insert_changed(&mut self, s: &str) -> bool {
        let before = self.text.len();
        self.insert_str(s);
        self.text.len() != before
    }

    fn remove_grapheme(&mut self, idx: usize) {
        let (start, end) = (self.byte_at(idx), self.byte_at(idx + 1));
        self.text.replace_range(start..end, "");
    }

    fn backspace(&mut self) -> bool {
        let g = self.cursor_grapheme.min(self.grapheme_count());
        if g == 0 {
            return false;
        }
        self.remove_grapheme(g - 1);
        self.cursor_grapheme = g - 1;
        self.goal_col = None;
        true
    }

    fn delete(&mut self) -> bool {
        let g = self.cursor_grapheme;
        if g >= self.grapheme_count() {
            return false;
        }
        self.remove_grapheme(g);
        self.goal_col = None;
        true
    }

    /// One grapheme left or right (crossing `"\n"`).
    fn step(&mut self, forward: bool) -> bool {
        let n = self.grapheme_count();
        let g = self.cursor_grapheme.min(n);
        let target = if forward {
            if g >= n {
                return false;
            }
            g + 1
        } else {
            if g == 0 {
                return false;
            }
            g - 1
        };
        self.cursor_grapheme = target;
        self.goal_col = None;
        true
    }

    /// Home (`to_end == false`) or End of the visual row the cursor is on.
    fn move_in_row(&mut self, width: u16, to_end: bool) -> bool {
        let gs: Vec<&str> = self.text.graphemes(true).collect();
        let rows = wrap(&gs, width);
        let (row, _) = self.rc_in(&gs, &rows);
        self.cursor_grapheme = if to_end {
            row_max_pos(&rows, row)
        } else {
            rows[row].start
        };
        self.goal_col = None;
        true
    }

    /// Up/Down/PageUp/PageDown: `by` visual rows, clamped at the first/last row, the
    /// column coming from `goal_col` (set on first use).
    fn move_rows(&mut self, width: u16, down: bool, by: usize) -> bool {
        let gs: Vec<&str> = self.text.graphemes(true).collect();
        let rows = wrap(&gs, width);
        let (row, col) = self.rc_in(&gs, &rows);
        let target = if down {
            (row + by).min(rows.len() - 1)
        } else {
            row.saturating_sub(by)
        };
        if target == row {
            return false;
        }
        let goal = self.goal_col.unwrap_or(col);
        let max = row_max_pos(&rows, target);
        let mut pos = rows[target].start;
        let mut acc = 0;
        while pos < max {
            let w = cell_width(gs[pos]);
            if acc + w > goal {
                break;
            }
            acc += w;
            pos += 1;
        }
        self.cursor_grapheme = pos;
        self.goal_col = Some(goal);
        true
    }

    /// The cursor's `(row, column)` among `rows` (the last row starting at or before it).
    fn rc_in(&self, gs: &[&str], rows: &[VisualRow]) -> (usize, usize) {
        let g = self.cursor_grapheme.min(gs.len());
        let row = rows.iter().rposition(|r| r.start <= g).unwrap_or(0);
        let start = rows[row].start.min(g);
        let col: usize = gs[start..g].iter().map(|s| cell_width(s)).sum();
        (row, col)
    }
}

/// The furthest grapheme index the cursor can take while staying on row `i`: the
/// row's `end`, except on a soft-wrapped row, where `end` belongs to the next row, so
/// it is the row's last grapheme.
fn row_max_pos(rows: &[VisualRow], i: usize) -> usize {
    let r = rows[i];
    if rows.get(i + 1).is_some_and(|next| next.start == r.end) {
        r.end.saturating_sub(1).max(r.start)
    } else {
        r.end
    }
}

/// The wrap rule (design section 3) over the buffer's graphemes.
fn wrap(gs: &[&str], width: u16) -> Vec<VisualRow> {
    let limit = usize::from(width).max(2) - 1;
    let ws: Vec<usize> = gs
        .iter()
        .map(|g| if is_break(g) { 0 } else { cell_width(g) })
        .collect();
    let n = gs.len();
    let mut rows = Vec::new();
    let mut line_start = 0;
    loop {
        let line_end = (line_start..n).find(|&i| is_break(gs[i])).unwrap_or(n);
        wrap_line(gs, &ws, line_start, line_end, limit, &mut rows);
        if line_end >= n {
            break;
        }
        line_start = line_end + 1;
    }
    rows
}

/// Wraps the logical line `start..end` into `rows`: greedy by word token (a run of
/// non-space graphemes with its trailing space), hard-breaking an over-long token by
/// grapheme. An empty line is one empty row.
fn wrap_line(
    gs: &[&str],
    ws: &[usize],
    start: usize,
    end: usize,
    limit: usize,
    rows: &mut Vec<VisualRow>,
) {
    let mut row_start = start;
    let mut row_w = 0;
    let mut i = start;
    while i < end {
        let mut j = i;
        while j < end && gs[j] != " " {
            j += 1;
        }
        if j < end {
            j += 1;
        }
        let token_w: usize = ws[i..j].iter().sum();
        if row_w + token_w <= limit {
            row_w += token_w;
        } else {
            if i > row_start {
                rows.push(VisualRow {
                    start: row_start,
                    end: i,
                    width: row_w,
                });
                row_start = i;
                row_w = 0;
            }
            for (off, &w) in ws[i..j].iter().enumerate() {
                let k = i + off;
                if k > row_start && row_w + w > limit {
                    rows.push(VisualRow {
                        start: row_start,
                        end: k,
                        width: row_w,
                    });
                    row_start = k;
                    row_w = 0;
                }
                row_w += w;
            }
        }
        i = j;
    }
    rows.push(VisualRow {
        start: row_start,
        end,
        width: row_w,
    });
}

/// Index just past an ESC-introduced sequence whose ESC precedes `i`.
fn skip_escape(cs: &[char], i: usize) -> usize {
    match cs.get(i) {
        None => i,
        Some('[') => skip_csi(cs, i + 1),
        Some(']') => skip_string(cs, i + 1, true),
        Some('P' | 'X' | '^' | '_') => skip_string(cs, i + 1, false),
        Some(_) => {
            let mut i = i;
            while cs.get(i).is_some_and(|c| ('\u{20}'..='\u{2f}').contains(c)) {
                i += 1;
            }
            if cs.get(i).is_some_and(|c| ('\u{30}'..='\u{7e}').contains(c)) {
                i += 1;
            }
            i
        }
    }
}

/// Index just past a CSI body starting at `i` (parameters, intermediates, one final
/// byte). A byte outside those ranges ends the sequence there, unconsumed.
fn skip_csi(cs: &[char], mut i: usize) -> usize {
    while cs.get(i).is_some_and(|c| ('\u{30}'..='\u{3f}').contains(c)) {
        i += 1;
    }
    while cs.get(i).is_some_and(|c| ('\u{20}'..='\u{2f}').contains(c)) {
        i += 1;
    }
    if cs.get(i).is_some_and(|c| ('\u{40}'..='\u{7e}').contains(c)) {
        i += 1;
    }
    i
}

/// Index just past a string sequence body starting at `i`: up to ST (`ESC \` or
/// U+009C), or BEL when `bel` (OSC). An unterminated one swallows the rest.
fn skip_string(cs: &[char], mut i: usize, bel: bool) -> usize {
    while i < cs.len() {
        match cs[i] {
            '\u{7}' if bel => return i + 1,
            '\u{9c}' => return i + 1,
            '\u{1b}' if cs.get(i + 1) == Some(&'\\') => return i + 2,
            _ => i += 1,
        }
    }
    cs.len()
}
