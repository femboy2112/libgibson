use crate::capability::{Capability, ColorDepth, TerminalCapabilities};
use crossterm::{
    cursor::Show,
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, stdout, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};

static PANIC_HOOK_SET: AtomicBool = AtomicBool::new(false);

/// Manages the terminal lifecycle, raw mode, alternate screen, and restoration guards.
pub struct TerminalSession {
    pub is_tty: bool,
    /// When true, all direct OS terminal manipulation is suppressed while the
    /// session still behaves like a TTY for the renderer. This enables
    /// byte-stream tests against a virtual terminal (e.g. `vt100`).
    headless: bool,
    headless_size: (u16, u16),
    raw_mode_enabled: bool,
    alt_screen_active: bool,
    cursor_hidden: bool,
    bracketed_paste_enabled: bool,
    sync_updates_enabled: bool,
    capabilities: TerminalCapabilities,
}

impl TerminalSession {
    /// Creates a new terminal session, detecting whether stdout is an interactive terminal.
    pub fn new() -> io::Result<Self> {
        let is_tty = stdout().is_terminal();
        let session = Self {
            is_tty,
            headless: false,
            headless_size: (80, 24),
            raw_mode_enabled: false,
            alt_screen_active: false,
            cursor_hidden: false,
            bracketed_paste_enabled: false,
            sync_updates_enabled: true,
            capabilities: TerminalCapabilities::detect_from_env(),
        };

        Self::ensure_panic_hook();

        Ok(session)
    }

    /// Creates a deterministic, non-destructive session that reports itself as a
    /// TTY of the requested geometry but never touches the real terminal.
    ///
    /// This is the controlled geometry abstraction used to drive the *whole*
    /// renderer pipeline (`Renderer -> TerminalTransaction -> bytes`) through a
    /// virtual terminal in tests.
    pub fn headless(cols: u16, rows: u16) -> Self {
        Self {
            is_tty: true,
            headless: true,
            headless_size: (cols, rows),
            raw_mode_enabled: false,
            alt_screen_active: false,
            cursor_hidden: false,
            bracketed_paste_enabled: false,
            sync_updates_enabled: true,
            capabilities: TerminalCapabilities::truecolor(),
        }
    }

    /// Current passive capability snapshot.
    pub fn capabilities(&self) -> TerminalCapabilities {
        self.capabilities
    }

    /// Overrides capabilities (used by capability-injection tests and callers
    /// that know better than the environment).
    pub fn set_capabilities(&mut self, caps: TerminalCapabilities) {
        self.capabilities = caps;
    }

    /// Convenience: current color depth.
    pub fn color_depth(&self) -> ColorDepth {
        self.capabilities.color_depth
    }

    /// Convenience: override color depth while keeping other capabilities.
    pub fn set_color_depth(&mut self, depth: ColorDepth) {
        self.capabilities.color_depth = depth;
    }

    /// Whether `CSI L` insertion is explicitly known to be supported.
    pub fn insert_line_supported(&self) -> bool {
        self.capabilities.insert_line == Capability::Supported
    }

    /// Updates the geometry reported by a headless session. Used to simulate
    /// terminal resize without a real PTY.
    pub fn set_terminal_size(&mut self, cols: u16, rows: u16) {
        self.headless_size = (cols, rows);
    }

    /// True when this session must not perform direct OS terminal manipulation.
    pub fn is_headless(&self) -> bool {
        self.headless
    }

    /// Sets up a global panic hook to restore the terminal safely if a panic occurs.
    fn ensure_panic_hook() {
        if !PANIC_HOOK_SET.swap(true, Ordering::SeqCst) {
            let default_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                // Best-effort terminal restoration on panic
                let _ = disable_raw_mode();
                let mut out = stdout();
                let _ = execute!(out, Show, LeaveAlternateScreen, DisableBracketedPaste);
                let _ = out.write_all(b"\x1b[0m\x1b[?2026l\x1b[?7h\x1b[?25h\r\n");
                let _ = out.flush();

                default_hook(info);
            }));
        }
    }

    /// Enables raw mode and bracketed paste for interactive operation.
    pub fn enter_interactive(&mut self) -> io::Result<()> {
        if !self.is_tty || self.headless {
            return Ok(());
        }

        if !self.raw_mode_enabled {
            enable_raw_mode()?;
            self.raw_mode_enabled = true;
        }

        if !self.bracketed_paste_enabled {
            let mut out = stdout();
            execute!(out, EnableBracketedPaste)?;
            self.bracketed_paste_enabled = true;
        }

        Ok(())
    }

    /// Enters alternate screen mode (fullscreen).
    pub fn enter_alternate_screen(&mut self) -> io::Result<()> {
        if !self.is_tty || self.headless || self.alt_screen_active {
            return Ok(());
        }

        let mut out = stdout();
        execute!(out, EnterAlternateScreen)?;
        self.alt_screen_active = true;
        Ok(())
    }

    /// Leaves alternate screen mode.
    pub fn leave_alternate_screen(&mut self) -> io::Result<()> {
        if !self.is_tty || !self.alt_screen_active {
            return Ok(());
        }

        let mut out = stdout();
        execute!(out, LeaveAlternateScreen)?;
        self.alt_screen_active = false;
        Ok(())
    }

    /// Hides the hardware cursor.
    pub fn hide_cursor(&mut self) -> Option<&'static [u8]> {
        if !self.is_tty || self.cursor_hidden {
            return None;
        }
        self.cursor_hidden = true;
        Some(b"\x1b[?25l")
    }

    /// Shows the hardware cursor.
    pub fn show_cursor(&mut self) -> Option<&'static [u8]> {
        if !self.is_tty || !self.cursor_hidden {
            return None;
        }
        self.cursor_hidden = false;
        Some(b"\x1b[?25h")
    }

    /// Sets whether synchronized-update mode (CSI ? 2026) is used.
    pub fn set_sync_updates(&mut self, enabled: bool) {
        self.sync_updates_enabled = enabled;
    }

    pub fn sync_updates(&self) -> bool {
        self.sync_updates_enabled && self.is_tty
    }

    /// Restores all modified terminal settings back to normal.
    pub fn restore(&mut self) -> io::Result<()> {
        if !self.is_tty || self.headless {
            return Ok(());
        }

        let mut out = stdout();

        if self.cursor_hidden {
            let _ = execute!(out, Show);
            self.cursor_hidden = false;
        }

        if self.bracketed_paste_enabled {
            let _ = execute!(out, DisableBracketedPaste);
            self.bracketed_paste_enabled = false;
        }

        if self.alt_screen_active {
            let _ = execute!(out, LeaveAlternateScreen);
            self.alt_screen_active = false;
        }

        // Reset styling, synchronized update state, enable autowrap, show cursor
        let _ = out.write_all(b"\x1b[0m\x1b[?2026l\x1b[?7h\x1b[?25h");
        let _ = out.flush();

        if self.raw_mode_enabled {
            let _ = disable_raw_mode();
            self.raw_mode_enabled = false;
        }

        Ok(())
    }

    /// Returns current terminal dimensions (columns, rows).
    pub fn terminal_size(&self) -> (u16, u16) {
        if self.headless {
            self.headless_size
        } else if self.is_tty {
            crossterm::terminal::size().unwrap_or((80, 24))
        } else {
            (80, 24)
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
