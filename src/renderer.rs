use crate::ansi::AnsiCompiler;
use crate::cell::Style;
use crate::diff::compute_diff;
use crate::layout::compute_layout;
use crate::node::Node;
use crate::painter::paint;
use crate::session::TerminalSession;
use crate::surface::Surface;
use std::io::{self, stdout, Write};
use crate::transaction::TerminalTransaction;

/// Operating mode of the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderMode {
    /// Default inline mode: coexists with normal terminal scrollback history.
    #[default]
    Inline,
    /// Fullscreen mode: owns the terminal using the alternate screen buffer.
    Fullscreen,
}

/// The core differential terminal renderer.
pub struct Renderer {
    pub mode: RenderMode,
    previous_surface: Option<Surface>,
    compiler: AnsiCompiler,
    pub live_region_height: u16,
    pub last_cursor_y: u16,
    pub last_cursor_x: u16,
}

impl Renderer {
    pub fn new(mode: RenderMode) -> Self {
        Self {
            mode,
            previous_surface: None,
            compiler: AnsiCompiler::new(),
            live_region_height: 0,
            last_cursor_y: 0,
            last_cursor_x: 0,
        }
    }


    /// Renders a UI node tree onto the terminal.
    /// Returns (dirty_cells_count, total_cells_count, bytes_emitted, is_full_repaint).
    pub fn render(
        &mut self,
        root: &mut Node,
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<(usize, usize, usize, bool, crate::painter::PaintContext)> {
        if !session.is_tty {
            // Non-TTY / CI mode: suppress live interactive frames
            return Ok((0, 0, 0, false, crate::painter::PaintContext::default()));
        }

        let (term_cols, term_rows) = session.terminal_size();
        let max_inline_rows = term_rows.saturating_sub(1).max(1);

        let available_height = match self.mode {
            RenderMode::Fullscreen => term_rows,
            RenderMode::Inline => max_inline_rows,
        };

        // 1. Layout pass
        let rect = compute_layout(root, term_cols, available_height).map_err(io::Error::other)?;

        let surface_width = term_cols;
        let surface_height = match self.mode {
            RenderMode::Fullscreen => term_rows,
            RenderMode::Inline => rect.height.max(1).min(max_inline_rows),
        };

        // 2. Paint pass into next surface
        let mut next_surface = Surface::new(surface_width, surface_height);
        let paint_ctx = paint(root, &mut next_surface);

        // 3. Diff pass
        let is_full_repaint = self.previous_surface.is_none();
        let diff = compute_diff(self.previous_surface.as_ref(), &next_surface);

        let total_cells = (surface_width as usize) * (surface_height as usize);
        let dirty_cells = diff.total_dirty_cells();

        // If frames are identical and dimensions didn't change, emit nothing
        if diff.is_empty() && !is_full_repaint {
            return Ok((0, total_cells, 0, false, paint_ctx));
        }

        // 4. ANSI compilation and execution
        let mut tx = TerminalTransaction::new(writer, session.sync_updates());
        tx.begin();

        match self.mode {
            RenderMode::Fullscreen => {
                self.compiler.reset_cursor(0, 0);
                let bytes = self.compiler.compile(&diff);
                tx.push(&bytes);

                if let Some((cx, cy)) = paint_ctx.cursor_position {
                    self.compiler.move_to(cx, cy, &mut tx.buffer);
                    if let Some(cmd) = session.show_cursor() { tx.push(cmd); }
                } else {
                    if let Some(cmd) = session.hide_cursor() { tx.push(cmd); }
                }
            }
            RenderMode::Inline => {
                if is_full_repaint {
                    // First frame of this live region:
                    // Make room in the terminal scrollback by emitting (H - 1) newlines,
                    // then rewind to the top of the newly allocated region.
                    if surface_height > 1 {
                        for _ in 0..(surface_height - 1) {
                            tx.push(b"\r\n");
                        }
                        use std::io::Write as _;
                        write!(tx.buffer, "\x1b[{}A\r", surface_height - 1).ok();
                    } else {
                        tx.push(b"\r");
                    }
                    self.compiler.reset_cursor(0, 0);
                    self.last_cursor_y = 0;
                    self.last_cursor_x = 0;
                } else {
                    // Rewind cursor from last position back to row 0 of live region
                    if self.last_cursor_y > 0 {
                        use std::io::Write as _;
                        write!(tx.buffer, "\x1b[{}A\r", self.last_cursor_y).ok();
                    } else {
                        tx.push(b"\r");
                    }
                    self.compiler.reset_cursor(0, 0);

                    // If live region grew, emit newlines at bottom to allocate extra rows
                    if surface_height > self.live_region_height {
                        let extra = surface_height - self.live_region_height;
                        use std::io::Write as _;
                        write!(tx.buffer, "\x1b[{}B", self.live_region_height - 1).ok();
                        for _ in 0..extra {
                            tx.push(b"\r\n");
                        }
                        write!(tx.buffer, "\x1b[{}A\r", surface_height - 1).ok();
                        self.compiler.reset_cursor(0, 0);
                    }
                }

                // Compile diff
                let bytes = self.compiler.compile(&diff);
                tx.push(&bytes);

                // Position cursor at widget request or at bottom of live region
                if let Some((cx, cy)) = paint_ctx.cursor_position {
                    self.compiler.move_to(cx, cy, &mut tx.buffer);
                    self.last_cursor_x = cx;
                    self.last_cursor_y = cy;
                    if let Some(cmd) = session.show_cursor() { tx.push(cmd); }
                } else {
                    // Park cursor at bottom row
                    let bottom_y = surface_height.saturating_sub(1);
                    self.compiler.move_to(0, bottom_y, &mut tx.buffer);
                    self.last_cursor_x = 0;
                    self.last_cursor_y = bottom_y;
                    if let Some(cmd) = session.hide_cursor() { tx.push(cmd); }
                }

                self.live_region_height = surface_height;
            }
        }

        let bytes_emitted = tx.buffer.len();
        if bytes_emitted > 0 {
            tx.commit()?;
        }

        self.previous_surface = Some(next_surface);

        Ok((dirty_cells, total_cells, bytes_emitted, is_full_repaint, paint_ctx))
    }

    /// Commits text to the immutable terminal scrollback.
    ///
    /// Invariants:
    /// - Live region is cleared/finalized.
    /// - Committed text is printed into native terminal scrollback.
    /// - Committed text is completely removed from mutable framebuffer state.
    /// - Subsequent renders start afresh below the committed text.
    pub fn commit(&mut self, text: &str, session: &mut TerminalSession, writer: &mut dyn Write) -> io::Result<()> {
        let mut stdout_handle = stdout();

        if !session.is_tty {
            // In non-TTY mode, strip any ANSI escapes to ensure clean plain text
            for line in text.lines() {
                let clean = strip_ansi_escapes(line);
                writeln!(stdout_handle, "{}", clean)?;
            }
            stdout_handle.flush()?;
            return Ok(());
        }

        let mut tx = TerminalTransaction::new(writer, session.sync_updates());
        tx.begin();

        if self.live_region_height > 0 {
            // Rewind to top of live region
            if self.last_cursor_y > 0 {
                write!(tx.buffer, "\x1b[{}A\r", self.last_cursor_y).ok();
            } else {
                tx.push(b"\r");
            }

            // Clear each row of the live region
            for i in 0..self.live_region_height {
                tx.push(b"\x1b[K"); // Clear line
                if i + 1 < self.live_region_height {
                    tx.push(b"\r\n");
                }
            }

            // Rewind back to top of that area
            if self.live_region_height > 1 {
                write!(tx.buffer, "\x1b[{}A\r", self.live_region_height - 1).ok();
            } else {
                tx.push(b"\r");
            }
        }

        // Print committed lines followed by newline
        for line in text.lines() {
            tx.push(line.as_bytes());
            tx.push(b"\r\n");
        }

        // Reset live region state
        self.previous_surface = None;
        self.live_region_height = 0;
        self.last_cursor_y = 0;
        self.last_cursor_x = 0;

        tx.commit()?;

        Ok(())
    }

    /// Inserts committed lines into native terminal scrollback ABOVE the active live region,
    /// preserving the active live region's content, geometry, cursor, and diff state.
    pub fn insert_before_live(
        &mut self,
        lines: &[&str],
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<(usize, bool)> {
        if !session.is_tty {
            for line in lines {
                writeln!(writer, "{}", strip_ansi_escapes(line))?;
            }
            writer.flush()?;
            return Ok((0, false));
        }

        if lines.is_empty() {
            return Ok((0, false));
        }

        let m = lines.len() as u16;
        let (_, term_rows) = session.terminal_size();
        let combined_height = m.saturating_add(self.live_region_height);

        let mut tx = TerminalTransaction::new(writer, session.sync_updates());
        tx.begin();

        if self.live_region_height == 0 || combined_height <= term_rows {
            // == ScrollingRegionInsertion Strategy ==
            if self.live_region_height > 0 {
                if self.last_cursor_y < self.live_region_height - 1 {
                    write!(tx.buffer, "\x1b[{}B", self.live_region_height - 1 - self.last_cursor_y).ok();
                }

                for _ in 0..m {
                    tx.push(b"\r\n");
                }

                write!(tx.buffer, "\x1b[{}A\r", m + self.live_region_height - 1).ok();
                write!(tx.buffer, "\x1b[{}L", m).ok();
            }

            for line in lines {
                tx.push(line.as_bytes());
                tx.push(b"\r\n");
            }

            if self.live_region_height > 0 {
                if self.last_cursor_y > 0 {
                    write!(tx.buffer, "\x1b[{}B", self.last_cursor_y).ok();
                }
                if self.last_cursor_x > 0 {
                    write!(tx.buffer, "\x1b[{}C", self.last_cursor_x).ok();
                }
            }
        } else {
            // == RepaintFallback Strategy ==
            if self.live_region_height > 1 {
                if self.last_cursor_y > 0 {
                    write!(tx.buffer, "\x1b[{}A", self.last_cursor_y).ok();
                }
                tx.push(b"\r\x1b[J");
            } else if self.live_region_height == 1 {
                tx.push(b"\r\x1b[K");
            }

            for line in lines {
                tx.push(line.as_bytes());
                tx.push(b"\r\n");
            }

            if self.live_region_height > 1 {
                for _ in 0..(self.live_region_height - 1) {
                    tx.push(b"\r\n");
                }
                write!(tx.buffer, "\x1b[{}A\r", self.live_region_height - 1).ok();
            } else if self.live_region_height == 1 {
                tx.push(b"\r");
            }

            if let Some(ref prev) = self.previous_surface {
                self.compiler.reset_cursor(0, 0);
                let diff = crate::diff::compute_diff(None, prev);
                let bytes = self.compiler.compile(&diff);
                tx.push(&bytes);
                self.last_cursor_y = prev.height.saturating_sub(1);
                // cursor_x is handled by paint next frame anyway, but we should roughly place it
            }
        }

        let bytes = tx.buffer.len();
        tx.commit()?;
        let used_fallback = combined_height > term_rows;
        Ok((bytes, used_fallback))
    }

    /// Commits a laid-out UI node tree directly to immutable scrollback.
    /// Serializes to styled ANSI for TTY or clean plain text with 0 escapes for non-TTY.
    pub fn commit_node(
        &mut self,
        node: &mut Node,
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<()> {
        let (term_cols, _) = session.terminal_size();
        let lines = render_node_to_lines(node, session.is_tty, term_cols)?;
        let joined = lines.join("\n");
        self.commit(&joined, session, writer)
    }

    /// Inserts a laid-out UI node tree into immutable scrollback ABOVE the active live region.
    pub fn insert_node_before_live(
        &mut self,
        node: &mut Node,
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<(usize, bool)> {
        let (term_cols, _) = session.terminal_size();
        let lines = render_node_to_lines(node, session.is_tty, term_cols)?;
        let line_refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        self.insert_before_live(&line_refs, session, writer)
    }

    /// Clears the live region from the terminal without leaving artifacts.
    pub fn clear_live_region(&mut self, session: &mut TerminalSession, writer: &mut dyn Write) -> io::Result<()> {
        if !session.is_tty || self.live_region_height == 0 {
            return Ok(());
        }

        let mut tx = TerminalTransaction::new(writer, session.sync_updates());
        tx.begin();
        if self.last_cursor_y > 0 {
            write!(tx.buffer, "\x1b[{}A\r", self.last_cursor_y).ok();
        } else {
            tx.push(b"\r");
        }

        for i in 0..self.live_region_height {
            tx.push(b"\x1b[K");
            if i + 1 < self.live_region_height {
                tx.push(b"\r\n");
            }
        }

        if self.live_region_height > 1 {
            write!(tx.buffer, "\x1b[{}A\r", self.live_region_height - 1).ok();
        } else {
            tx.push(b"\r");
        }

        self.previous_surface = None;
        self.live_region_height = 0;
        self.last_cursor_y = 0;
        self.last_cursor_x = 0;

        tx.commit()?;

        Ok(())
    }
}

/// Strips ANSI CSI / SGR escape sequences from a string to yield clean plain text.
pub fn strip_ansi_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_escape = false;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            in_escape = true;
            if chars.peek() == Some(&'[') {
                chars.next();
            }
            continue;
        }
        if in_escape {
            // SGR and CSI codes terminate on letters ('a'..='z', 'A'..='Z', '@', '~', etc.)
            if c.is_ascii_alphabetic() || c == '~' || c == '@' {
                in_escape = false;
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// Renders a UI node to lines of text.
/// If `is_tty` is true, renders styled ANSI lines.
/// If `is_tty` is false, renders plain UTF-8 text with ZERO escape codes.
pub fn render_node_to_lines(node: &mut Node, is_tty: bool, width: u16) -> io::Result<Vec<String>> {
    let rect = compute_layout(node, width, 0).map_err(io::Error::other)?;
    let height = rect.height.max(1);
    let mut surface = Surface::new(width, height);
    paint(node, &mut surface);

    let mut lines = Vec::with_capacity(height as usize);

    for y in 0..height {
        if is_tty {
            let mut line_str = String::new();
            let mut cur_style = Style::default();
            let mut last_col = 0;
            for x in 0..width {
                if let Some(c) = surface.get(x, y) {
                    if (!c.glyph.is_empty() && c.glyph.grapheme.as_str() != " ")
                        || !c.style.is_default()
                    {
                        last_col = x + 1;
                    }
                }
            }

            for x in 0..last_col {
                if let Some(cell) = surface.get(x, y) {
                    if cell.is_continuation {
                        continue;
                    }
                    if cell.style != cur_style {
                        if !cur_style.is_default() {
                            line_str.push_str("\x1b[0m");
                        }
                        if !cell.style.is_default() {
                            cell.style.write_sgr(&mut line_str);
                        }
                        cur_style = cell.style;
                    }
                    if cell.glyph.is_empty() {
                        line_str.push(' ');
                    } else {
                        line_str.push_str(cell.glyph.grapheme.as_str());
                    }
                }
            }
            if !cur_style.is_default() {
                line_str.push_str("\x1b[0m");
            }
            lines.push(line_str);
        } else {
            let mut line_str = String::new();
            let mut last_col = 0;
            for x in 0..width {
                if let Some(c) = surface.get(x, y) {
                    if !c.glyph.is_empty() && c.glyph.grapheme.as_str() != " " {
                        last_col = x + 1;
                    }
                }
            }
            for x in 0..last_col {
                if let Some(cell) = surface.get(x, y) {
                    if cell.is_continuation {
                        continue;
                    }
                    if cell.glyph.is_empty() {
                        line_str.push(' ');
                    } else {
                        line_str.push_str(cell.glyph.grapheme.as_str());
                    }
                }
            }
            lines.push(line_str);
        }
    }

    Ok(lines)
}
