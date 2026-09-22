use crate::ansi::AnsiCompiler;
use crate::diff::compute_diff;
use crate::layout::compute_layout;
use crate::node::Node;
use crate::painter::paint;
use crate::session::TerminalSession;
use crate::surface::Surface;
use std::io::{self, stdout, Write};

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
    pub fn new(mode: RenderMode, sync_updates: bool) -> Self {
        Self {
            mode,
            previous_surface: None,
            compiler: AnsiCompiler::new(sync_updates),
            live_region_height: 0,
            last_cursor_y: 0,
            last_cursor_x: 0,
        }
    }

    pub fn set_sync_updates(&mut self, enabled: bool) {
        self.compiler.sync_updates = enabled;
    }

    /// Renders a UI node tree onto the terminal.
    /// Returns (dirty_cells_count, total_cells_count, bytes_emitted, is_full_repaint).
    pub fn render(
        &mut self,
        root: &mut Node,
        session: &mut TerminalSession,
    ) -> io::Result<(usize, usize, usize, bool)> {
        if !session.is_tty {
            // Non-TTY / CI mode: suppress live interactive frames
            return Ok((0, 0, 0, false));
        }

        let (term_cols, term_rows) = session.terminal_size();

        let available_height = match self.mode {
            RenderMode::Fullscreen => term_rows,
            RenderMode::Inline => 0, // Unconstrained height: layout determines intrinsic height
        };

        // 1. Layout pass
        let rect = compute_layout(root, term_cols, available_height).map_err(io::Error::other)?;

        let surface_width = term_cols;
        let surface_height = match self.mode {
            RenderMode::Fullscreen => term_rows,
            RenderMode::Inline => rect.height.max(1),
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
            return Ok((0, total_cells, 0, false));
        }

        // 4. ANSI compilation and execution
        let mut out = Vec::new();

        match self.mode {
            RenderMode::Fullscreen => {
                self.compiler.reset_cursor(0, 0);
                let bytes = self.compiler.compile(&diff);
                out.extend_from_slice(&bytes);

                if let Some((cx, cy)) = paint_ctx.cursor_position {
                    self.compiler.move_to(cx, cy, &mut out);
                    let _ = session.show_cursor();
                } else {
                    let _ = session.hide_cursor();
                }
            }
            RenderMode::Inline => {
                if is_full_repaint {
                    // First frame of this live region:
                    // Make room in the terminal scrollback by emitting (H - 1) newlines,
                    // then rewind to the top of the newly allocated region.
                    if surface_height > 1 {
                        for _ in 0..(surface_height - 1) {
                            out.extend_from_slice(b"\r\n");
                        }
                        use std::io::Write as _;
                        write!(out, "\x1b[{}A\r", surface_height - 1).ok();
                    } else {
                        out.push(b'\r');
                    }
                    self.compiler.reset_cursor(0, 0);
                    self.last_cursor_y = 0;
                    self.last_cursor_x = 0;
                } else {
                    // Rewind cursor from last position back to row 0 of live region
                    if self.last_cursor_y > 0 {
                        use std::io::Write as _;
                        write!(out, "\x1b[{}A\r", self.last_cursor_y).ok();
                    } else {
                        out.push(b'\r');
                    }
                    self.compiler.reset_cursor(0, 0);

                    // If live region grew, emit newlines at bottom to allocate extra rows
                    if surface_height > self.live_region_height {
                        let extra = surface_height - self.live_region_height;
                        use std::io::Write as _;
                        write!(out, "\x1b[{}B", self.live_region_height - 1).ok();
                        for _ in 0..extra {
                            out.extend_from_slice(b"\r\n");
                        }
                        write!(out, "\x1b[{}A\r", surface_height - 1).ok();
                        self.compiler.reset_cursor(0, 0);
                    }
                }

                // Compile diff
                let bytes = self.compiler.compile(&diff);
                out.extend_from_slice(&bytes);

                // Position cursor at widget request or at bottom of live region
                if let Some((cx, cy)) = paint_ctx.cursor_position {
                    self.compiler.move_to(cx, cy, &mut out);
                    self.last_cursor_x = cx;
                    self.last_cursor_y = cy;
                    let _ = session.show_cursor();
                } else {
                    // Park cursor at bottom row
                    let bottom_y = surface_height.saturating_sub(1);
                    self.compiler.move_to(0, bottom_y, &mut out);
                    self.last_cursor_x = 0;
                    self.last_cursor_y = bottom_y;
                    let _ = session.hide_cursor();
                }

                self.live_region_height = surface_height;
            }
        }

        let bytes_emitted = out.len();
        if bytes_emitted > 0 {
            let mut stdout_handle = stdout();
            stdout_handle.write_all(&out)?;
            stdout_handle.flush()?;
        }

        self.previous_surface = Some(next_surface);

        Ok((dirty_cells, total_cells, bytes_emitted, is_full_repaint))
    }

    /// Commits text to the immutable terminal scrollback.
    ///
    /// Invariants:
    /// - Live region is cleared/finalized.
    /// - Committed text is printed into native terminal scrollback.
    /// - Committed text is completely removed from mutable framebuffer state.
    /// - Subsequent renders start afresh below the committed text.
    pub fn commit(&mut self, text: &str, session: &mut TerminalSession) -> io::Result<()> {
        let mut stdout_handle = stdout();

        if !session.is_tty {
            // In non-TTY mode, simply write the plain lines
            for line in text.lines() {
                writeln!(stdout_handle, "{}", line)?;
            }
            stdout_handle.flush()?;
            return Ok(());
        }

        let mut out = Vec::new();

        if self.live_region_height > 0 {
            // Rewind to top of live region
            if self.last_cursor_y > 0 {
                write!(out, "\x1b[{}A\r", self.last_cursor_y).ok();
            } else {
                out.push(b'\r');
            }

            // Clear each row of the live region
            for i in 0..self.live_region_height {
                out.extend_from_slice(b"\x1b[K"); // Clear line
                if i + 1 < self.live_region_height {
                    out.extend_from_slice(b"\r\n");
                }
            }

            // Rewind back to top of that area
            if self.live_region_height > 1 {
                write!(out, "\x1b[{}A\r", self.live_region_height - 1).ok();
            } else {
                out.push(b'\r');
            }
        }

        // Print committed lines followed by newline
        for line in text.lines() {
            out.extend_from_slice(line.as_bytes());
            out.extend_from_slice(b"\r\n");
        }

        // Reset live region state
        self.previous_surface = None;
        self.live_region_height = 0;
        self.last_cursor_y = 0;
        self.last_cursor_x = 0;

        stdout_handle.write_all(&out)?;
        stdout_handle.flush()?;

        Ok(())
    }

    /// Clears the live region from the terminal without leaving artifacts.
    pub fn clear_live_region(&mut self, session: &mut TerminalSession) -> io::Result<()> {
        if !session.is_tty || self.live_region_height == 0 {
            return Ok(());
        }

        let mut out = Vec::new();
        if self.last_cursor_y > 0 {
            write!(out, "\x1b[{}A\r", self.last_cursor_y).ok();
        } else {
            out.push(b'\r');
        }

        for i in 0..self.live_region_height {
            out.extend_from_slice(b"\x1b[K");
            if i + 1 < self.live_region_height {
                out.extend_from_slice(b"\r\n");
            }
        }

        if self.live_region_height > 1 {
            write!(out, "\x1b[{}A\r", self.live_region_height - 1).ok();
        } else {
            out.push(b'\r');
        }

        self.previous_surface = None;
        self.live_region_height = 0;
        self.last_cursor_y = 0;
        self.last_cursor_x = 0;

        let mut stdout_handle = stdout();
        stdout_handle.write_all(&out)?;
        stdout_handle.flush()?;

        Ok(())
    }
}
