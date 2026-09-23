use crate::ansi::AnsiCompiler;
use crate::cell::{RichText, Style};
use crate::diff::compute_diff;
use crate::layout::compute_layout;
use crate::node::{Node, WrapMode};
use crate::painter::paint;
use crate::session::TerminalSession;
use crate::surface::Surface;
use crate::transaction::TerminalTransaction;
use std::io::{self, Write};

/// Operating mode of the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderMode {
    /// Default inline mode: coexists with normal terminal scrollback history.
    #[default]
    Inline,
    /// Fullscreen mode: owns the terminal using the alternate screen buffer.
    Fullscreen,
}

/// Which mechanism was used to insert history above the live region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertStrategy {
    /// Print the history rows and use `CSI L` (Insert Lines) at the region top,
    /// so the existing live framebuffer is not repainted at all.
    InsertLineFastPath,
    /// Erase from the region top down, print the history rows, then repaint the
    /// preserved live framebuffer and restore the exact cursor state.
    RepaintFallback,
}

impl InsertStrategy {
    pub fn used_fallback(self) -> bool {
        matches!(self, InsertStrategy::RepaintFallback)
    }
}

/// Explicit physical live-region anchor.
///
/// The renderer owns more physical state than the cell framebuffer: the hardware
/// cursor position/visibility and the live-region anchor. A terminal resize
/// (which may reflow) invalidates the relationship between the previous surface,
/// the live-region height and the physical rows, so the anchor becomes
/// [`AnchorState::Invalid`] and must be re-established before ordinary diff
/// rendering resumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorState {
    /// No trustworthy anchor; the next frame must fully re-establish the region.
    Invalid,
    /// The live region was rendered coherently at this terminal geometry.
    Stable {
        cols: u16,
        rows: u16,
        live_height: u16,
    },
}

/// The core differential terminal renderer.
pub struct Renderer {
    pub mode: RenderMode,
    previous_surface: Option<Surface>,
    compiler: AnsiCompiler,
    pub live_region_height: u16,
    pub last_cursor_y: u16,
    pub last_cursor_x: u16,
    /// Physical cursor visibility as last committed.
    pub last_cursor_visible: Option<bool>,
    /// The cursor requested by the last painted frame (region-relative).
    last_cursor_desired: Option<(u16, u16)>,
    pub anchor: AnchorState,
    last_terminal_size: Option<(u16, u16)>,

    // --- accounting (absolute counters, mirrored into Context stats) ---
    pub total_frame_bytes: u64,
    pub total_commit_bytes: u64,
    pub total_insertion_bytes: u64,
    pub total_control_bytes: u64,
    pub anchor_resyncs: u64,
    pub history_insertions: u64,
    pub fast_insertions: u64,
    pub fallback_insertions: u64,
    pub last_insert_strategy: Option<InsertStrategy>,
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
            last_cursor_visible: None,
            last_cursor_desired: None,
            anchor: AnchorState::Invalid,
            last_terminal_size: None,
            total_frame_bytes: 0,
            total_commit_bytes: 0,
            total_insertion_bytes: 0,
            total_control_bytes: 0,
            anchor_resyncs: 0,
            history_insertions: 0,
            fast_insertions: 0,
            fallback_insertions: 0,
            last_insert_strategy: None,
        }
    }

    /// Marks the physical anchor as untrustworthy and discards diff state so the
    /// next frame fully re-establishes the live region.
    fn invalidate_anchor(&mut self, count_resync: bool) {
        if count_resync {
            self.anchor_resyncs += 1;
        }
        self.anchor = AnchorState::Invalid;
        self.previous_surface = None;
        self.live_region_height = 0;
        self.last_cursor_x = 0;
        self.last_cursor_y = 0;
        self.last_cursor_visible = None;
        self.last_cursor_desired = None;
    }

    /// Detects terminal geometry changes and invalidates the physical anchor.
    /// Returns true if a re-anchor (as opposed to a first frame) is required.
    fn observe_geometry(&mut self, cols: u16, rows: u16) -> bool {
        if self.last_terminal_size == Some((cols, rows)) {
            return false;
        }
        let had_prior_state = self.last_terminal_size.is_some()
            && (self.previous_surface.is_some() || self.live_region_height > 0);
        self.last_terminal_size = Some((cols, rows));
        self.invalidate_anchor(had_prior_state);
        had_prior_state
    }

    /// Renders a UI node tree onto the terminal.
    /// Returns (dirty_cells, total_cells, bytes_emitted, is_full_repaint, PaintContext).
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
        // Color quality is decided centrally from the session's capabilities.
        self.compiler.color_depth = session.color_depth();
        let reanchor = self.observe_geometry(term_cols, term_rows);
        let is_full_repaint = self.previous_surface.is_none();

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
        let diff = compute_diff(self.previous_surface.as_ref(), &next_surface);

        let total_cells = (surface_width as usize) * (surface_height as usize);
        let dirty_cells = diff.total_dirty_cells();

        // Cursor visibility/position are part of physical truth: an unchanged
        // framebuffer does not imply "nothing needs emitting".
        let desired_visible = paint_ctx.cursor_position.is_some();
        let cursor_state_changed = self.last_cursor_visible != Some(desired_visible)
            || self.last_cursor_desired != paint_ctx.cursor_position;

        if diff.is_empty() && !is_full_repaint && !cursor_state_changed {
            return Ok((0, total_cells, 0, false, paint_ctx));
        }

        // 4. ANSI compilation and execution (single atomic transaction)
        let mut tx = TerminalTransaction::new(writer, session.sync_updates());
        tx.begin();

        match self.mode {
            RenderMode::Fullscreen => {
                // Home the *physical* cursor before compiling: the compiler's
                // relative motion is meaningless unless the real cursor is where
                // the compiler thinks it is. Without this, each frame drifts by
                // the previous frame's final cursor and eventually scrolls.
                tx.push(b"\x1b[H");
                self.compiler.reset_cursor(0, 0);
                let bytes = self.compiler.compile(&diff);
                tx.push(&bytes);

                if let Some((cx, cy)) = paint_ctx.cursor_position {
                    self.compiler.move_to(cx, cy, &mut tx.buffer);
                    self.last_cursor_x = cx;
                    self.last_cursor_y = cy;
                    if let Some(cmd) = session.show_cursor() {
                        tx.push(cmd);
                    }
                } else {
                    if let Some(cmd) = session.hide_cursor() {
                        tx.push(cmd);
                    }
                }
            }
            RenderMode::Inline => {
                if is_full_repaint {
                    if reanchor {
                        // Erase from the current row downward before rebuilding:
                        // after reflow this discards stale live-region artifacts.
                        tx.push(b"\r\x1b[J");
                    }
                    // First frame of this live region: allocate (H - 1) rows of
                    // terminal space, then rewind to the region top.
                    if surface_height > 1 {
                        for _ in 0..(surface_height - 1) {
                            tx.push(b"\r\n");
                        }
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
                        write!(tx.buffer, "\x1b[{}A\r", self.last_cursor_y).ok();
                    } else {
                        tx.push(b"\r");
                    }
                    self.compiler.reset_cursor(0, 0);

                    // If live region grew, emit newlines at bottom to allocate extra rows
                    if surface_height > self.live_region_height && self.live_region_height > 0 {
                        let extra = surface_height - self.live_region_height;
                        // NOTE: `CSI 0 B` is NOT a no-op in real terminals (0
                        // canonicalizes to 1), so only emit the move when there
                        // is a real distance to travel.
                        if self.live_region_height > 1 {
                            write!(tx.buffer, "\x1b[{}B", self.live_region_height - 1).ok();
                        }
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

                // Position cursor at widget request or park at bottom of live region
                if let Some((cx, cy)) = paint_ctx.cursor_position {
                    self.compiler.move_to(cx, cy, &mut tx.buffer);
                    self.last_cursor_x = cx;
                    self.last_cursor_y = cy;
                    if let Some(cmd) = session.show_cursor() {
                        tx.push(cmd);
                    }
                } else {
                    let bottom_y = surface_height.saturating_sub(1);
                    self.compiler.move_to(0, bottom_y, &mut tx.buffer);
                    self.last_cursor_x = 0;
                    self.last_cursor_y = bottom_y;
                    if let Some(cmd) = session.hide_cursor() {
                        tx.push(cmd);
                    }
                }

                self.live_region_height = surface_height;
            }
        }

        // Exact wire bytes, including the synchronized-update terminator that
        // `commit` appends. Do not sample `buffer.len()` before committing.
        let bytes_emitted = if tx.buffered_len() > 0 {
            tx.commit()?
        } else {
            0
        };

        self.previous_surface = Some(next_surface);
        self.last_cursor_visible = Some(desired_visible);
        self.last_cursor_desired = paint_ctx.cursor_position;
        self.anchor = AnchorState::Stable {
            cols: term_cols,
            rows: term_rows,
            live_height: surface_height,
        };
        self.total_frame_bytes += bytes_emitted as u64;

        Ok((
            dirty_cells,
            total_cells,
            bytes_emitted,
            is_full_repaint,
            paint_ctx,
        ))
    }

    /// Writes already-safe (engine-generated) lines to immutable scrollback,
    /// clearing the active live region first.
    fn write_committed_lines(
        &mut self,
        lines: &[&str],
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<usize> {
        if !session.is_tty {
            let mut bytes = 0usize;
            for line in lines {
                let clean = strip_ansi_escapes(line);
                writeln!(writer, "{}", clean)?;
                bytes += clean.len() + 1;
            }
            writer.flush()?;
            self.total_commit_bytes += bytes as u64;
            return Ok(bytes);
        }

        let mut tx = TerminalTransaction::new(writer, session.sync_updates());
        tx.begin();

        if self.live_region_height > 0 {
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
        }

        for line in lines {
            tx.push(line.as_bytes());
            tx.push(b"\r\n");
        }

        let bytes = tx.commit()?;

        // Committing finalizes live state; the next live frame starts fresh.
        self.invalidate_anchor(false);
        self.total_commit_bytes += bytes as u64;
        Ok(bytes)
    }

    /// Raw ANSI escape hatch. The caller is responsible for the safety and
    /// correctness of `text`; arbitrary OSC/DCS/CSI input is **not** sanitized.
    ///
    /// Use [`Renderer::commit_text`] or [`Renderer::commit_rich_text`] for
    /// structured output.
    pub fn commit_raw_ansi_unchecked(
        &mut self,
        text: &str,
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<usize> {
        let lines: Vec<&str> = text.lines().collect();
        self.write_committed_lines(&lines, session, writer)
    }

    /// Safe, structured plain-text commit.
    ///
    /// Historically `Renderer::commit` was an alias for the **raw** escape hatch
    /// while `Context::commit` was safe text — the same name inverted its safety
    /// meaning depending on abstraction level. The raw alias has been removed;
    /// this now routes to [`Renderer::commit_text`].
    #[deprecated(note = "use `commit_text`, `commit_rich_text`, or `commit_node`")]
    pub fn commit(
        &mut self,
        text: &str,
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<()> {
        self.commit_text(text, session, writer).map(|_| ())
    }

    /// Commits plain structured text to immutable scrollback, routed through the
    /// same width-aware wrapping engine as `Node::text`.
    pub fn commit_text(
        &mut self,
        text: &str,
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<usize> {
        let (term_cols, _) = session.terminal_size();
        let mut node = Node::text_wrapped(text, Style::default(), WrapMode::WordWrap);
        node.layout_style.width = crate::node::Dimension::Length(term_cols as f32);
        let lines = render_node_to_lines_with_depth(
            &mut node,
            session.is_tty,
            term_cols,
            session.color_depth(),
        )?;
        let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        self.write_committed_lines(&refs, session, writer)
    }

    /// Commits structured rich text to immutable scrollback, wrapped and aligned
    /// by the same layout engine used for live nodes.
    pub fn commit_rich_text(
        &mut self,
        rich: &RichText,
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<usize> {
        let (term_cols, _) = session.terminal_size();
        let mut node = Node::rich_text_wrapped(rich.clone(), WrapMode::WordWrap);
        node.layout_style.width = crate::node::Dimension::Length(term_cols as f32);
        let lines = render_node_to_lines_with_depth(
            &mut node,
            session.is_tty,
            term_cols,
            session.color_depth(),
        )?;
        let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        self.write_committed_lines(&refs, session, writer)
    }

    /// Commits a laid-out UI node tree directly to immutable scrollback.
    /// Serializes to styled ANSI for TTY or clean plain text with 0 escapes for non-TTY.
    pub fn commit_node(
        &mut self,
        node: &mut Node,
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<usize> {
        let (term_cols, _) = session.terminal_size();
        let lines = render_node_to_lines_with_depth(
            node,
            session.is_tty,
            term_cols,
            session.color_depth(),
        )?;
        let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        self.write_committed_lines(&refs, session, writer)
    }

    /// Inserts **raw** lines into native terminal scrollback ABOVE the active live
    /// region. The lines are treated as terminal byte streams: **no sanitization
    /// is performed**, so escape/OSC/CSI sequences in `lines` reach the terminal.
    ///
    /// This is the intentionally-ugly escape hatch. Prefer
    /// [`Renderer::insert_text_before_live`],
    /// [`Renderer::insert_rich_text_before_live`] or
    /// [`Renderer::insert_node_before_live`].
    ///
    /// Returns `(bytes, strategy)`. See [`InsertStrategy`] for the two mechanisms.
    pub fn insert_raw_lines_before_live_unchecked(
        &mut self,
        lines: &[&str],
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<(usize, InsertStrategy)> {
        self.insert_lines_before_live(lines, session, writer)
    }

    /// Inserts **safe** plain text into scrollback above the live region.
    ///
    /// The text is wrapped width-aware by the same layout engine used for live
    /// nodes, and control characters are neutralized at the cell model boundary,
    /// so untrusted text cannot inject terminal controls.
    pub fn insert_text_before_live(
        &mut self,
        text: &str,
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<(usize, InsertStrategy)> {
        let (term_cols, _) = session.terminal_size();
        let mut node = Node::text_wrapped(text, Style::default(), WrapMode::WordWrap);
        node.layout_style.width = crate::node::Dimension::Length(term_cols as f32);
        let lines = render_node_to_lines_with_depth(
            &mut node,
            session.is_tty,
            term_cols,
            session.color_depth(),
        )?;
        let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        self.insert_lines_before_live(&refs, session, writer)
    }

    /// Inserts **safe** rich text into scrollback above the live region, wrapped
    /// and aligned by the same layout engine used for live nodes.
    pub fn insert_rich_text_before_live(
        &mut self,
        rich: &RichText,
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<(usize, InsertStrategy)> {
        let (term_cols, _) = session.terminal_size();
        let mut node = Node::rich_text_wrapped(rich.clone(), WrapMode::WordWrap);
        node.layout_style.width = crate::node::Dimension::Length(term_cols as f32);
        self.insert_node_before_live(&mut node, session, writer)
    }

    /// Shared insertion implementation. `lines` are engine-generated bytes.
    fn insert_lines_before_live(
        &mut self,
        lines: &[&str],
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<(usize, InsertStrategy)> {
        if !session.is_tty {
            let mut bytes = 0usize;
            for line in lines {
                let clean = strip_ansi_escapes(line);
                writeln!(writer, "{}", clean)?;
                bytes += clean.len() + 1;
            }
            writer.flush()?;
            self.history_insertions += 1;
            self.fast_insertions += 1;
            self.total_insertion_bytes += bytes as u64;
            self.last_insert_strategy = Some(InsertStrategy::InsertLineFastPath);
            return Ok((bytes, InsertStrategy::InsertLineFastPath));
        }

        if lines.is_empty() {
            return Ok((0, InsertStrategy::InsertLineFastPath));
        }

        let m = lines.len() as u16;
        let (_, term_rows) = session.terminal_size();
        let combined_height = m.saturating_add(self.live_region_height);

        // The fast path relies on trustworthy relative cursor/region state and on
        // `CSI L` actually being supported. Unknown capability is NOT treated as
        // supported: we fall back to the always-correct repaint path.
        let anchor_stable = matches!(self.anchor, AnchorState::Stable { .. });
        let use_fast = self.live_region_height > 0
            && anchor_stable
            && session.insert_line_supported()
            && combined_height <= term_rows
            && self.previous_surface.is_some();

        let mut tx = TerminalTransaction::new(writer, session.sync_updates());
        tx.begin();
        let strategy;

        if !use_fast {
            // == RepaintFallback Strategy ==
            // Always correct: erase from region top down, print history, repaint
            // the preserved live framebuffer, then restore exact cursor state.
            strategy = InsertStrategy::RepaintFallback;

            if self.live_region_height > 0 {
                if self.last_cursor_y > 0 {
                    write!(tx.buffer, "\x1b[{}A\r", self.last_cursor_y).ok();
                } else {
                    tx.push(b"\r");
                }
                tx.push(b"\x1b[J");
            }

            for line in lines {
                tx.push(line.as_bytes());
                tx.push(b"\r\n");
            }

            if let Some(prev) = self.previous_surface.clone() {
                // current cursor row is now region row 0
                self.compiler.reset_cursor(0, 0);
                let diff = compute_diff(None, &prev);
                let bytes = self.compiler.compile(&diff);
                tx.push(&bytes);

                // Restore the exact requested cursor position/visibility.
                if self.last_cursor_visible == Some(true) {
                    self.compiler
                        .move_to(self.last_cursor_x, self.last_cursor_y, &mut tx.buffer);
                    if let Some(cmd) = session.show_cursor() {
                        tx.push(cmd);
                    }
                } else {
                    let bottom_y = prev.height.saturating_sub(1);
                    self.compiler.move_to(0, bottom_y, &mut tx.buffer);
                    if let Some(cmd) = session.hide_cursor() {
                        tx.push(cmd);
                    }
                }
                // previous_surface is preserved: the next diff render emits zero bytes.
            }
        } else {
            // == InsertLineFastPath Strategy ==
            // Insert M blank lines at the region top with CSI L, so the existing
            // live framebuffer is never repainted.
            strategy = InsertStrategy::InsertLineFastPath;

            // Move to region bottom, scroll to make room, then return to top.
            if self.last_cursor_y < self.live_region_height - 1 {
                write!(
                    tx.buffer,
                    "\x1b[{}B",
                    self.live_region_height - 1 - self.last_cursor_y
                )
                .ok();
            }

            for _ in 0..m {
                tx.push(b"\r\n");
            }

            write!(tx.buffer, "\x1b[{}A\r", m + self.live_region_height - 1).ok();
            write!(tx.buffer, "\x1b[{}L", m).ok();

            for line in lines {
                tx.push(line.as_bytes());
                tx.push(b"\r\n");
            }

            // Restore cursor to its original relative position within the region.
            if self.last_cursor_y > 0 {
                write!(tx.buffer, "\x1b[{}B", self.last_cursor_y).ok();
            }
            if self.last_cursor_x > 0 {
                write!(tx.buffer, "\x1b[{}C", self.last_cursor_x).ok();
            }
        }

        let bytes = tx.commit()?;

        self.history_insertions += 1;
        self.total_insertion_bytes += bytes as u64;
        self.last_insert_strategy = Some(strategy);
        match strategy {
            InsertStrategy::InsertLineFastPath => self.fast_insertions += 1,
            InsertStrategy::RepaintFallback => self.fallback_insertions += 1,
        }

        Ok((bytes, strategy))
    }

    /// Inserts a laid-out UI node tree into immutable scrollback ABOVE the active live region.
    pub fn insert_node_before_live(
        &mut self,
        node: &mut Node,
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<(usize, InsertStrategy)> {
        let (term_cols, _) = session.terminal_size();
        let lines = render_node_to_lines_with_depth(
            node,
            session.is_tty,
            term_cols,
            session.color_depth(),
        )?;
        let line_refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        self.insert_lines_before_live(&line_refs, session, writer)
    }

    /// Clears the live region from the terminal without leaving artifacts.
    /// Returns the number of bytes emitted.
    pub fn clear_live_region(
        &mut self,
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<usize> {
        if !session.is_tty || self.live_region_height == 0 {
            return Ok(0);
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

        let bytes = tx.commit()?;

        self.invalidate_anchor(false);
        self.total_control_bytes += bytes as u64;
        Ok(bytes)
    }
}

/// Strips ANSI CSI / SGR escape sequences from a string to yield clean plain text.
///
/// This exists only to render *already engine-generated* escape sequences back
/// to readable text for non-TTY logs. It is explicitly **not** a sanitizer for
/// untrusted input: arbitrary OSC/DCS/CSI must not be passed to the raw API.
pub fn strip_ansi_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_escape = false;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            in_escape = true;
            // Handle CSI and OSC introducers; OSC is terminated by BEL or ST.
            if chars.peek() == Some(&'[') {
                chars.next();
            } else if chars.peek() == Some(&']') {
                chars.next();
                for c in chars.by_ref() {
                    if c == '\x07' {
                        break;
                    }
                    if c == '\x1b' {
                        if chars.peek() == Some(&'\\') {
                            chars.next();
                        }
                        break;
                    }
                }
                in_escape = false;
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
    render_node_to_lines_with_depth(
        node,
        is_tty,
        width,
        crate::capability::ColorDepth::TrueColor,
    )
}

/// Like [`render_node_to_lines`], but quantizes styles for `depth` so scrollback
/// output obeys the same color ladder as live frames.
pub fn render_node_to_lines_with_depth(
    node: &mut Node,
    is_tty: bool,
    width: u16,
    depth: crate::capability::ColorDepth,
) -> io::Result<Vec<String>> {
    use crate::capability::quantize_style;
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
                    let style = quantize_style(cell.style, depth);
                    if style != cur_style {
                        if !cur_style.is_default() {
                            line_str.push_str("\x1b[0m");
                        }
                        if !style.is_default() {
                            style.write_sgr(&mut line_str);
                        }
                        cur_style = style;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Style;
    use crate::node::Node;

    #[test]
    fn test_observe_geometry_invalidates_only_on_change() {
        let mut r = Renderer::new(RenderMode::Inline);
        assert!(!r.observe_geometry(80, 24));
        // Simulate a prior frame having established state.
        r.previous_surface = Some(Surface::new(80, 3));
        r.live_region_height = 3;
        r.anchor = AnchorState::Stable {
            cols: 80,
            rows: 24,
            live_height: 3,
        };
        assert!(r.observe_geometry(40, 12));
        assert_eq!(r.anchor, AnchorState::Invalid);
        assert_eq!(r.anchor_resyncs, 1);
        // Same size again: no further resync.
        assert!(!r.observe_geometry(40, 12));
        assert_eq!(r.anchor_resyncs, 1);
    }

    #[test]
    fn test_non_tty_render_emits_nothing() {
        let mut session = TerminalSession::new().unwrap();
        session.is_tty = false;
        let mut renderer = Renderer::new(RenderMode::Inline);
        let mut root = Node::col().child(Node::text("x", Style::default()));
        let (_d, _t, bytes, _f, _p) = renderer
            .render(&mut root, &mut session, &mut Vec::new())
            .unwrap();
        assert_eq!(bytes, 0);
    }
}
