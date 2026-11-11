use crate::cell::RichText;
use crate::input::{poll_event, Event, KeyReleaseReporting};
use crate::node::Node;
use crate::renderer::{AnchorState, FrameReport, InsertStrategy, Renderer};
use crate::scheduler::{FramePacing, FrameScheduler, RenderStats};
use crate::session::TerminalSession;
use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// `RenderMode` is consumed by [`Context::new`] and [`Context::headless`], so it
/// is re-exported here for ergonomic imports (`gibson::context::RenderMode`) in
/// addition to the crate root (`gibson::RenderMode`).
pub use crate::renderer::RenderMode;

/// High-level engine coordinator managing session, rendering, input, and scheduling.
///
/// On Unix, a terminal Context pins its renderer output to an owned duplicate
/// of the stdout descriptor observed during construction. The caller must
/// still keep process stdout stable through terminal acquisition, session mode
/// changes, panic handling and restoration: those operations currently write
/// process stdout separately and are not protected by this output pin.
pub struct Context {
    /// Session for this context. Replacing it after output begins does not
    /// transfer the renderer's destination state; later context output is
    /// rejected. A fresh headless context may be configured before first output.
    pub session: TerminalSession,
    session_identity: Arc<()>,
    /// Renderer for this context. Direct changes to its public physical-state
    /// fields or calls to its output methods make destination state ambiguous.
    /// Once output begins, replacement does too. Later context output is
    /// rejected; restore and create a new context. Standalone renderer users
    /// must manage their own output destination and cannot assume different
    /// writers match.
    pub renderer: Renderer,
    renderer_identity: Arc<()>,
    renderer_generation: Option<u64>,
    renderer_fingerprint: (RenderMode, u16, u16, u16, Option<bool>, AnchorState),
    pub scheduler: FrameScheduler,
    root: Option<Node>,
    /// Where rendered bytes go: a pinned terminal descriptor on Unix interactive
    /// contexts, stdout otherwise, or an in-memory headless buffer.
    output: OutputSink,
    /// Count of `render`/`render_now` calls issued with no root set — the most
    /// likely "forgot `set_root`" integration mistake (issue #48 E-01). A
    /// rootless render is a silent wire-level no-op; this counter makes it
    /// observable and distinct from a legitimately empty settled frame.
    empty_root_renders: u64,
}

/// Destination for a [`Context`]'s rendered bytes.
///
/// On Unix, interactive contexts write to a duplicate of the acquired stdout.
/// Headless contexts capture into memory; noninteractive contexts retain stdout.
enum OutputSink {
    Stdout,
    #[cfg(unix)]
    PinnedTerminal(std::fs::File),
    Buffer(Vec<u8>),
}

impl Write for OutputSink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            OutputSink::Stdout => io::stdout().write(buf),
            #[cfg(unix)]
            OutputSink::PinnedTerminal(file) => file.write(buf),
            OutputSink::Buffer(b) => b.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            OutputSink::Stdout => io::stdout().flush(),
            #[cfg(unix)]
            OutputSink::PinnedTerminal(file) => file.flush(),
            OutputSink::Buffer(b) => b.flush(),
        }
    }
}

// Keep an independently owned descriptor so later changes to fd 1 cannot
// redirect a Context's renderer output. This does not pin the session's
// separate terminal-mode, panic-hook, or restoration writes.
#[cfg(unix)]
fn pin_interactive_stdout() -> io::Result<Option<std::fs::File>> {
    use std::os::fd::FromRawFd;
    if !io::stdout().is_terminal() {
        return Ok(None);
    }
    io::stdout().flush()?;
    // SAFETY: fd 1 is borrowed; a nonnegative return is a new owned descriptor.
    let fd = unsafe { libc::fcntl(libc::STDOUT_FILENO, libc::F_DUPFD_CLOEXEC, 3) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fcntl returned a new descriptor exclusively owned by this File.
    let file = unsafe { std::fs::File::from_raw_fd(fd) };
    if !file.is_terminal() {
        return Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "stdout changed during terminal acquisition",
        ));
    }
    Ok(Some(file))
}

// This detects ordinary acquisition-time retargeting. It is not synchronization
// against another thread concurrently replacing fd 1 during a mode transition.
#[cfg(unix)]
fn stdout_matches_pin(file: &std::fs::File) -> io::Result<bool> {
    use std::os::fd::AsRawFd;
    let mut pinned = std::mem::MaybeUninit::<libc::stat>::uninit();
    let mut current = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: fstat writes the supplied stat structure on success.
    if unsafe { libc::fstat(file.as_raw_fd(), pinned.as_mut_ptr()) } != 0
        || unsafe { libc::fstat(libc::STDOUT_FILENO, current.as_mut_ptr()) } != 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: both fstat calls succeeded.
    let (pinned, current) = unsafe { (pinned.assume_init(), current.assume_init()) };
    Ok(pinned.st_dev == current.st_dev
        && pinned.st_ino == current.st_ino
        && pinned.st_rdev == current.st_rdev)
}

/// A read-only view of one resolved cell in the most recently rendered frame, with its style
/// already quantized to the depth that frame was *emitted* at (so `style` matches the ANSI wire
/// actually written — even if the requested depth has since changed and not yet been repainted).
/// Returned by [`Context::frame_cell`]; borrows the frame surface for the call's lifetime.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameCell<'a> {
    /// The grapheme cluster painted in this cell (empty for a blank or continuation cell).
    pub glyph: &'a str,
    /// Display columns the glyph occupies: 0 (continuation/empty), 1, or 2.
    pub width: u8,
    /// True if this cell is the trailing half of a wide (width-2) glyph.
    pub is_continuation: bool,
    /// True if the cell is transparent (contributes nothing when composited).
    pub transparent: bool,
    /// The cell's style, quantized to the depth the frame was emitted at — fg/bg downgraded or
    /// dropped exactly as the emitted SGR was (see [`Context::set_color_depth`]).
    pub style: crate::cell::Style,
}

impl Context {
    fn frame_state_owned(&self) -> bool {
        Arc::ptr_eq(&self.renderer_identity, &self.renderer.identity())
            && Arc::ptr_eq(&self.session_identity, &self.session.identity())
            && self.renderer_generation.is_some()
            && self.renderer_generation == self.renderer.output_generation()
            && self.renderer_fingerprint == self.renderer.physical_fingerprint()
    }

    fn ensure_output_authorized(&self) -> io::Result<()> {
        let physical_stdout =
            matches!(self.output, OutputSink::Stdout) && io::stdout().is_terminal();
        #[cfg(unix)]
        let physical_stdout =
            physical_stdout || matches!(self.output, OutputSink::PinnedTerminal(_));
        // A freshly constructed virtual context may be configured before its
        // first output. Once used, its renderer/session belong to that sink.
        let unbound_virtual = matches!(self.output, OutputSink::Buffer(_))
            && self.renderer_generation == Some(0)
            && self.renderer.output_generation() == Some(0);
        if !unbound_virtual && !Arc::ptr_eq(&self.session_identity, &self.session.identity()) {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "context session was replaced; create a new context for the new session",
            ));
        }
        if !unbound_virtual && !Arc::ptr_eq(&self.renderer_identity, &self.renderer.identity()) {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "context renderer was replaced; create a new context for the new renderer",
            ));
        }
        if self.renderer_generation.is_none()
            || self.renderer_generation != self.renderer.output_generation()
            || self.renderer_fingerprint != self.renderer.physical_fingerprint()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "context renderer was modified outside the context; create a new context",
            ));
        }
        if physical_stdout && !self.session.owns_terminal_lease() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "stdout terminal has no session lease",
            ));
        }
        self.session.ensure_render_authorized()
    }

    fn managed_output<T>(
        &mut self,
        f: impl FnOnce(&mut Renderer, &mut TerminalSession, &mut OutputSink) -> io::Result<T>,
    ) -> io::Result<T> {
        self.ensure_output_authorized()?;
        self.renderer_identity = self.renderer.identity();
        self.session_identity = self.session.identity();
        let result = f(&mut self.renderer, &mut self.session, &mut self.output);
        // Record even on an ordinary error: counted-write recovery belongs to
        // this target. A panic skips this assignment and thus fails closed.
        self.renderer_generation = self.renderer.output_generation();
        self.renderer_fingerprint = self.renderer.physical_fingerprint();
        result
    }

    pub fn new(mode: RenderMode) -> io::Result<Self> {
        #[cfg(unix)]
        let pinned = pin_interactive_stdout()?;
        let mut session = TerminalSession::new()?;
        #[cfg(unix)]
        let output = match (session.owns_terminal_lease(), pinned) {
            (true, Some(file)) if stdout_matches_pin(&file)? => OutputSink::PinnedTerminal(file),
            (false, None) => OutputSink::Stdout,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::NotConnected,
                    "stdout changed during terminal acquisition",
                ))
            }
        };
        #[cfg(not(unix))]
        let output = OutputSink::Stdout;
        if mode == RenderMode::Fullscreen {
            session.enter_alternate_screen()?;
        }

        let session_identity = session.identity();
        let renderer = Renderer::new(mode);
        let renderer_identity = renderer.identity();
        let renderer_generation = renderer.output_generation();
        let renderer_fingerprint = renderer.physical_fingerprint();
        Ok(Self {
            session,
            session_identity,
            renderer,
            renderer_identity,
            renderer_generation,
            renderer_fingerprint,
            scheduler: FrameScheduler::new(60),
            root: None,
            output,
            empty_root_renders: 0,
        })
    }

    pub fn inline() -> io::Result<Self> {
        Self::new(RenderMode::Inline)
    }

    pub fn fullscreen() -> io::Result<Self> {
        Self::new(RenderMode::Fullscreen)
    }

    /// Creates a context backed by a headless, virtual terminal session of a
    /// fixed geometry. Intended for automation, snapshotting and tests.
    ///
    /// A headless context renders into an in-memory buffer instead of the process
    /// stdout, so no escape codes reach a real terminal. Read the captured bytes
    /// back with [`Context::rendered_bytes`] or drain them with
    /// [`Context::take_rendered_bytes`] (or the lossy-UTF-8 [`Context::take_output`]):
    ///
    /// ```
    /// use gibson::cell::Style;
    /// use gibson::context::{Context, RenderMode};
    /// use gibson::node::Node;
    ///
    /// let mut ctx = Context::headless(RenderMode::Inline, 40, 8);
    /// ctx.set_root(Node::col().child(Node::text("hello", Style::default())));
    /// ctx.render().unwrap();
    /// assert!(ctx.take_output().contains("hello"));
    /// ```
    ///
    /// A headless session defaults to **truecolor** capabilities, unlike a real
    /// terminal session, which detects color depth from the environment
    /// (`$COLORTERM` / `$TERM`, often 256- or 16-color). To reproduce a
    /// lower-depth terminal in a capture, override it with
    /// [`Context::set_color_depth`] or [`Context::set_capabilities`].
    pub fn headless(mode: RenderMode, cols: u16, rows: u16) -> Self {
        let session = TerminalSession::headless(cols, rows);
        let session_identity = session.identity();
        let renderer = Renderer::new(mode);
        let renderer_identity = renderer.identity();
        let renderer_generation = renderer.output_generation();
        let renderer_fingerprint = renderer.physical_fingerprint();
        Self {
            session,
            session_identity,
            renderer,
            renderer_identity,
            renderer_generation,
            renderer_fingerprint,
            scheduler: FrameScheduler::new(60),
            root: None,
            output: OutputSink::Buffer(Vec::new()),
            empty_root_renders: 0,
        }
    }

    /// Enable or disable synchronized-update framing (`CSI ?2026 h` … `CSI ?2026 l`) around each
    /// frame; on by default and emitted only on a TTY. It asks the terminal to present each update
    /// as a unit. It is a presentation request, **not** atomic I/O: a write can still fail after a
    /// prefix reached the terminal, and the renderer then re-establishes the screen on the next
    /// frame (`docs/RELEASE_CONTRACT.md` §9).
    pub fn set_sync_updates(&mut self, enabled: bool) {
        self.session.set_sync_updates(enabled);
    }

    /// Overrides detected terminal capabilities.
    pub fn set_capabilities(&mut self, caps: crate::capability::TerminalCapabilities) {
        self.session.set_capabilities(caps);
    }

    /// Overrides the color depth used for live frames and scrollback output.
    pub fn set_color_depth(&mut self, depth: crate::capability::ColorDepth) {
        self.session.set_color_depth(depth);
    }

    pub fn capabilities(&self) -> crate::capability::TerminalCapabilities {
        self.session.capabilities()
    }

    /// Enables recording of per-frame dirty-cell coordinates (for damage-map
    /// debug overlays). Off by default because it allocates per frame.
    pub fn set_capture_damage(&mut self, on: bool) {
        self.renderer.capture_damage = on;
    }

    /// Coordinates of the cells changed by the most recent frame (empty unless
    /// [`Context::set_capture_damage`] was enabled).
    pub fn last_dirty_cells(&self) -> &[(u16, u16)] {
        if self.frame_state_owned() {
            self.renderer.last_dirty_cells()
        } else {
            &[]
        }
    }

    /// The `(cols, rows)` geometry of the most recently rendered frame, or `None` before the first
    /// render. Returns `None` if direct renderer use or replacement has made the
    /// context's frame ownership uncertain. A stdout-backed context in a non-TTY
    /// process also stays `None` (its render is a suppressed no-op).
    pub fn frame_size(&self) -> Option<(u16, u16)> {
        if !self.frame_state_owned() {
            return None;
        }
        self.renderer
            .last_frame_surface()
            .map(|s| (s.width, s.height))
    }

    /// A display-accurate view of the cell at `(x, y)` in the most recently rendered frame — the
    /// resolved *styled cell grid*, i.e. what the frame paints, not the logical node tree — with
    /// its style quantized to the depth that frame was *emitted* at. A color-depth request made
    /// after painting does not retouch an already-emitted frame; it takes effect on the next
    /// repaint (Gate F2). `None` if there is no frame yet or `(x, y)` is out of bounds.
    pub fn frame_cell(&self, x: u16, y: u16) -> Option<FrameCell<'_>> {
        if !self.frame_state_owned() {
            return None;
        }
        let surface = self.renderer.last_frame_surface()?;
        let cell = surface.get(x, y)?;
        // Quantize with the depth the frame was EMITTED at, not a depth requested after painting.
        // These are set together after a successful commit, so a `Some(surface)` above implies a
        // `Some(depth)` here; the fallback is only belt-and-suspenders (Gate F2).
        let depth = self
            .renderer
            .last_emitted_depth()
            .unwrap_or_else(|| self.session.color_depth());
        Some(FrameCell {
            glyph: cell.glyph.grapheme.as_str(),
            width: cell.glyph.display_width,
            is_continuation: cell.is_continuation,
            transparent: cell.transparent,
            style: crate::capability::quantize_style(cell.style, depth),
        })
    }

    pub fn set_max_fps(&mut self, fps: u32) {
        self.scheduler.set_max_fps(fps);
    }

    /// Selects how frame deadlines are derived from the configured FPS ceiling.
    ///
    /// The default [`FramePacing::CompletionRelative`] preserves historical
    /// behavior. [`FramePacing::PhaseLocked`] anchors deadlines to the scheduled
    /// cadence so render/write time consumes the current period instead of being
    /// added after it. This improves high-cadence animation and temporal-rendering
    /// experiments, but it is still an emission clock — not terminal/display vsync.
    pub fn set_frame_pacing(&mut self, pacing: FramePacing) {
        self.scheduler.set_pacing(pacing);
    }

    pub fn frame_pacing(&self) -> FramePacing {
        self.scheduler.pacing()
    }

    /// Scheduled emission deadlines the most recent phase-locked frame overran.
    ///
    /// Always `0` under [`FramePacing::CompletionRelative`]. A temporal
    /// controller reads this alongside [`Context::last_frame_report`] to decide
    /// whether local cadence is healthy enough to keep modulating, or whether to
    /// fall back to static realization.
    pub fn missed_periods_last_frame(&self) -> u32 {
        self.scheduler.missed_periods_last_frame()
    }

    /// Cumulative phase-locked missed deadlines since construction (monotonic).
    ///
    /// A controller samples the delta over a window for cadence-health hysteresis
    /// rather than reacting to a single late frame.
    pub fn missed_periods_total(&self) -> u64 {
        self.scheduler.missed_periods_total()
    }

    /// Sets the recommended cadence for decorative animation (spinners etc.).
    pub fn set_animation_interval(&mut self, interval: Duration) {
        self.scheduler.set_animation_interval(interval);
    }

    pub fn set_root(&mut self, root: Node) {
        self.root = Some(root);
        self.scheduler.mark_dirty();
    }

    pub fn mark_dirty(&mut self) {
        self.scheduler.mark_dirty();
    }

    /// Explicit request to schedule a frame render pass.
    pub fn request_render(&mut self) {
        self.scheduler.request_render();
    }

    /// Checks if a frame should be rendered according to frame budget.
    pub fn should_render(&mut self) -> bool {
        self.scheduler.should_render()
    }

    /// Time until the next frame is *allowed* under the FPS budget, whether or not one
    /// is pending. Once a budget has elapsed this stays zero, so do not use it to bound
    /// an idle input wait; use [`time_until_frame_needed`](Self::time_until_frame_needed).
    pub fn time_until_next_frame(&self) -> Duration {
        self.scheduler.time_until_next_frame()
    }

    /// Time until a frame is *needed*: `None` when nothing is pending, otherwise the
    /// time until the pending frame may render (zero when it may render now). This is
    /// the deadline [`run_once`](Self::run_once) bounds its input wait by (issue #264).
    pub fn time_until_frame_needed(&self) -> Option<Duration> {
        self.scheduler.time_until_frame_needed()
    }

    pub fn frame_budget(&self) -> Duration {
        self.scheduler.frame_budget()
    }

    /// Recommended cadence for decorative animation.
    pub fn animation_interval(&self) -> Duration {
        self.scheduler.animation_interval()
    }

    /// Renders a frame only if marked dirty and the target FPS budget permits.
    /// Returns Ok(true) if a frame was rendered, Ok(false) if throttled or clean.
    pub fn render_if_due(&mut self) -> io::Result<bool> {
        if !self.scheduler.should_render() {
            return Ok(false);
        }
        self.render_internal()?;
        Ok(true)
    }

    /// Forces immediate rendering regardless of frame budget.
    pub fn render_now(&mut self) -> io::Result<crate::painter::PaintContext> {
        self.render_internal()
    }

    /// Standard render entry point (forces immediate frame update).
    pub fn render(&mut self) -> io::Result<crate::painter::PaintContext> {
        self.render_now()
    }

    /// Bytes captured by a buffer-backed (headless) context since the last
    /// [`Context::take_output`] or [`Context::take_rendered_bytes`]. Always empty
    /// for an interactive context, whose output goes straight to the process stdout.
    pub fn rendered_bytes(&self) -> &[u8] {
        match &self.output {
            OutputSink::Buffer(b) => b.as_slice(),
            OutputSink::Stdout => &[],
            #[cfg(unix)]
            OutputSink::PinnedTerminal(_) => &[],
        }
    }

    /// Drains the capture buffer and returns it as a `String` (lossy UTF-8).
    /// Empty for an interactive, stdout-backed context. After this call the
    /// buffer is empty until the next render.
    pub fn take_output(&mut self) -> String {
        match &mut self.output {
            OutputSink::Buffer(b) => {
                let bytes = std::mem::take(b);
                String::from_utf8_lossy(&bytes).into_owned()
            }
            OutputSink::Stdout => String::new(),
            #[cfg(unix)]
            OutputSink::PinnedTerminal(_) => String::new(),
        }
    }

    /// Drains the capture buffer and returns its bytes unchanged (binary-safe,
    /// unlike the lossy [`Context::take_output`]). Empty for an interactive,
    /// stdout-backed context. After this call the buffer is empty until the next
    /// render, and its allocation is released.
    pub fn take_rendered_bytes(&mut self) -> Vec<u8> {
        match &mut self.output {
            OutputSink::Buffer(b) => std::mem::take(b),
            OutputSink::Stdout => Vec::new(),
            #[cfg(unix)]
            OutputSink::PinnedTerminal(_) => Vec::new(),
        }
    }

    /// Whether this context drives a live, interactive terminal — a real TTY
    /// that is not a headless capture session.
    ///
    /// When this is `false`, [`Context::render`] emits nothing to a live screen
    /// and [`Context::poll_event`] never yields input, so an interactive loop
    /// built on `render` + `poll_event` idles silently forever (issue #46).
    /// Consumers of the interactive constructors ([`Context::inline`] /
    /// [`Context::fullscreen`] / [`Context::new`]) should check this and fall
    /// back to a headless/`--dump` path (or exit with a diagnostic) when it is
    /// `false`. Headless contexts return `false`: they render to a buffer, not a
    /// live screen, and have no interactive input source.
    pub fn is_interactive(&self) -> bool {
        self.session.is_tty && !self.session.is_headless()
    }

    /// Number of [`Context::render`]/[`Context::render_now`] calls issued while
    /// no root was set (issue #48 E-01).
    ///
    /// Rendering without a root is a silent wire-level no-op — forgetting
    /// [`Context::set_root`] is the most common new-consumer mistake and
    /// otherwise produces the same (empty) observables as a legitimate settled
    /// frame. A non-zero value here is a strong signal of that mistake; a
    /// root-backed render (even of an empty tree) never increments it.
    pub fn empty_root_renders(&self) -> u64 {
        self.empty_root_renders
    }

    /// Exact semantic changed-cell count of the most recent [`Context::render`]
    /// (issue #47): the number of cells whose final state actually changed on
    /// that frame — distinct from the logical affected footprint in
    /// [`RenderStats::dirty_cells`] and from emitted bytes. This reads a scalar
    /// the renderer already computed, so a consumer needs no second
    /// layout/paint/diff pass to observe it.
    pub fn last_exact_changed_cells(&self) -> usize {
        if self.frame_state_owned() {
            self.renderer.last_exact_changed()
        } else {
            0
        }
    }

    /// Timing/accounting snapshot for the most recent live frame (#64).
    ///
    /// Generation covers layout/paint/diff/ANSI transaction construction;
    /// write covers the blocking writer write/flush. Neither duration claims to
    /// measure terminal-compositor or physical display presentation.
    pub fn last_frame_report(&self) -> FrameReport {
        if self.frame_state_owned() {
            self.renderer.last_frame_report()
        } else {
            FrameReport::default()
        }
    }

    /// Visible text of the most recently composed live frame, one `String` per
    /// row (issue #48 E-02): "what the screen says", with wide-glyph continuation
    /// cells collapsed and trailing blanks trimmed. Complements the raw wire
    /// bytes from [`Context::rendered_bytes`] / [`Context::take_output`] and lets
    /// headless tests assert on visible text without a private terminal emulator.
    pub fn last_frame_lines(&self) -> Vec<String> {
        if self.frame_state_owned() {
            self.renderer.last_frame_lines()
        } else {
            Vec::new()
        }
    }

    /// Minimal, non-async runtime step — the core of the **canonical interactive
    /// loop** (issue #48 E-07).
    ///
    /// Waits for input for at most `max_wait`, then renders if a frame is pending and
    /// the FPS budget permits. Input wakeups have priority over decorative animation:
    /// an arriving event shortens the wait immediately. Returns the input event, if any.
    ///
    /// The wait is bounded by the next frame that is *needed*, not by the next one that
    /// would be *allowed* ([`time_until_frame_needed`](Self::time_until_frame_needed)):
    ///
    /// * nothing pending (the scheduler is clean): the call waits the full `max_wait`,
    ///   however long ago the last frame was, so an idle loop sleeps instead of spinning
    ///   (issue #264);
    /// * a frame pending (`set_root`, `mark_dirty`, `request_render`, or a resize): the
    ///   wait ends no later than when that frame becomes eligible under the FPS budget
    ///   (or at `max_wait`, if sooner). The frame renders before the call returns if it is
    ///   eligible by then; when input or a shorter `max_wait` ends the wait first, the
    ///   call returns without it and a later call renders it.
    ///
    /// So `run_once` never wakes on its own to animate. An animated view marks itself
    /// pending each step (rebuild with `set_root`, or `mark_dirty`) and passes a
    /// `max_wait` no longer than its step interval, for example
    /// [`animation_interval`](Self::animation_interval).
    ///
    /// The canonical loop a new consumer should copy — rebuild the tree on
    /// change, then let `run_once` drive both input and frame pacing:
    ///
    /// ```no_run
    /// # fn build_ui() -> gibson::node::Node { gibson::node::Node::col() }
    /// use gibson::context::{Context, RenderMode};
    /// use gibson::{Event, KeyCode};
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut ctx = Context::new(RenderMode::Inline)?;
    ///     if !ctx.is_interactive() {
    ///         // Non-TTY (issue #46): a live loop would render and receive nothing.
    ///         // Fall back to a headless/`--dump` path instead of idling blank.
    ///         return Ok(());
    ///     }
    ///     let mut dirty = true;
    ///     loop {
    ///         if dirty {
    ///             ctx.set_root(build_ui()); // your view
    ///             dirty = false;
    ///         }
    ///         match ctx.run_once(Duration::from_millis(100))? {
    ///             Some(Event::Key(k)) if k.code == KeyCode::Char('q') => break,
    ///             Some(_) => dirty = true, // handled input; rebuild next iteration
    ///             None => {}               // timed out: any due animation frame already rendered
    ///         }
    ///     }
    ///     ctx.restore()?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// `Event::Tick` is **not** emitted here — the loop is input-driven and
    /// animation is paced by the scheduler (`animation_interval` + the internal
    /// `render_if_due`). For a batteries-included loop that also delivers
    /// per-iteration ticks (`AppEvent::Tick`), use `gibson::ui::App`.
    pub fn run_once(&mut self, max_wait: Duration) -> io::Result<Option<Event>> {
        let wait = self.scheduler.input_wait(max_wait);
        let event = if self.session.is_tty && !self.session.is_headless() {
            self.poll_event(wait)?
        } else {
            // No input source off-TTY or on a headless (virtual) session: sleep for the
            // frame slice instead of busy-spinning, then render if due. A headless context
            // takes input only through an injected source (`poll_from`), never from the
            // process terminal (#259).
            if !wait.is_zero() {
                std::thread::sleep(wait);
            }
            None
        };
        if self.scheduler.should_render() {
            self.render_internal()?;
        }
        Ok(event)
    }

    fn render_internal(&mut self) -> io::Result<crate::painter::PaintContext> {
        self.ensure_output_authorized()?;
        let mut root = match self.root.take() {
            Some(r) => r,
            None => {
                // Rendering with no root is almost always a forgotten
                // `set_root` (issue #48 E-01). It is a wire-level no-op, so make
                // it observable rather than silent — without panicking a valid
                // production flow or changing the ABI-1 stats struct.
                self.empty_root_renders += 1;
                // Nothing can be presented, so nothing is pending any more. Left dirty,
                // a rootless context that was marked dirty (a resize, `mark_dirty`) would
                // keep `run_once`'s wait at zero forever (issue #264). `set_root` marks it
                // dirty again.
                self.scheduler.is_dirty = false;
                self.sync_renderer_metrics();
                return Ok(crate::painter::PaintContext::default());
            }
        };

        let start = Instant::now();
        let result = self.managed_output(|renderer, session, output| {
            renderer.render(&mut root, session, output)
        });
        let duration = start.elapsed();

        self.root = Some(root);
        self.sync_renderer_metrics();

        match result {
            Ok((dirty, total, bytes, full, paint_ctx)) => {
                self.scheduler
                    .record_frame(dirty, total, bytes, full, duration);
                Ok(paint_ctx)
            }
            Err(e) => Err(e),
        }
    }

    /// Overlays the renderer's absolute operation counters onto a copy of the
    /// scheduler's accumulated frame stats, returning the result by value
    /// without mutating anything. This is the read-only core shared by
    /// [`Context::stats`] and [`Context::sync_renderer_metrics`].
    fn overlaid_stats(&self) -> RenderStats {
        let r = &self.renderer;
        let mut s = self.scheduler.stats;
        s.anchor_resyncs = r.anchor_resyncs;
        s.history_insertions = r.history_insertions;
        s.fast_insertions = r.fast_insertions;
        s.insertion_repaints = r.fallback_insertions;
        s.insertion_bytes = r.total_insertion_bytes;
        s.commit_bytes = r.total_commit_bytes;
        s.control_bytes = r.total_control_bytes;
        s
    }

    /// Mirrors absolute renderer operation counters into the scheduler stats.
    fn sync_renderer_metrics(&mut self) {
        self.scheduler.stats = self.overlaid_stats();
    }

    /// Commits structured plain text to immutable scrollback (width-aware
    /// wrapping; embedded escape sequences are treated as literal text and
    /// cannot corrupt the terminal).
    pub fn commit_text(&mut self, text: &str) -> io::Result<()> {
        self.ensure_output_authorized()?;
        self.managed_output(|renderer, session, output| {
            renderer.commit_text(text, session, output)
        })?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Commits plain text to immutable scrollback with an explicit wrap policy
    /// (issue #45). `WrapMode::NoWrap` preserves preformatted content — aligned
    /// tables, diffs, ledgers — at its natural column width instead of re-flowing
    /// (`WordWrap`) or silently clipping it; embedded control sequences remain
    /// neutralized by the cell model.
    pub fn commit_text_with_mode(
        &mut self,
        text: &str,
        wrap: crate::node::WrapMode,
    ) -> io::Result<()> {
        self.ensure_output_authorized()?;
        self.managed_output(|renderer, session, output| {
            renderer.commit_text_with_mode(text, wrap, session, output)
        })?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Raw ANSI escape hatch. The caller is responsible for the payload.
    pub fn commit_raw_ansi_unchecked(&mut self, text: &str) -> io::Result<()> {
        self.ensure_output_authorized()?;
        self.managed_output(|renderer, session, output| {
            renderer.commit_raw_ansi_unchecked(text, session, output)
        })?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Backwards-compatible alias for [`Context::commit_text`].
    pub fn commit(&mut self, text: &str) -> io::Result<()> {
        self.commit_text(text)
    }

    /// Commits a laid-out UI node directly to immutable scrollback.
    pub fn commit_node(&mut self, node: &mut Node) -> io::Result<()> {
        self.ensure_output_authorized()?;
        self.managed_output(|renderer, session, output| {
            renderer.commit_node(node, session, output)
        })?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Commits structured rich text to immutable scrollback, routed through the
    /// same wrapping/alignment engine as live nodes.
    pub fn commit_rich_text(&mut self, rich: &RichText) -> io::Result<()> {
        self.ensure_output_authorized()?;
        self.managed_output(|renderer, session, output| {
            renderer.commit_rich_text(rich, session, output)
        })?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Inserts **raw** lines into scrollback above the live region. The lines are
    /// treated as terminal byte streams and are **not** sanitized — escape
    /// sequences reach the terminal. Prefer the safe insertion methods.
    pub fn insert_raw_lines_before_live_unchecked(&mut self, lines: &[&str]) -> io::Result<()> {
        self.ensure_output_authorized()?;
        self.managed_output(|renderer, session, output| {
            renderer.insert_raw_lines_before_live_unchecked(lines, session, output)
        })?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Inserts safe, width-aware plain text into scrollback above the live region.
    /// Control characters are neutralized by the cell model.
    pub fn insert_text_before_live(&mut self, text: &str) -> io::Result<()> {
        self.ensure_output_authorized()?;
        self.managed_output(|renderer, session, output| {
            renderer.insert_text_before_live(text, session, output)
        })?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Inserts safe plain text into scrollback above the live region with an
    /// explicit wrap policy (issue #45). `WrapMode::NoWrap` preserves preformatted
    /// aligned artifacts at their natural width (no re-flow, no silent clip);
    /// control characters are neutralized by the cell model.
    pub fn insert_text_before_live_with_mode(
        &mut self,
        text: &str,
        wrap: crate::node::WrapMode,
    ) -> io::Result<()> {
        self.ensure_output_authorized()?;
        self.managed_output(|renderer, session, output| {
            renderer.insert_text_before_live_with_mode(text, wrap, session, output)
        })?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Inserts a laid-out UI node into scrollback ABOVE the active live region.
    pub fn insert_node_before_live(&mut self, node: &mut Node) -> io::Result<()> {
        self.ensure_output_authorized()?;
        self.managed_output(|renderer, session, output| {
            renderer.insert_node_before_live(node, session, output)
        })?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Inserts safe rich text into scrollback ABOVE the active live region, using
    /// the width-aware layout engine.
    pub fn insert_rich_text_before_live(&mut self, rich: &RichText) -> io::Result<()> {
        self.ensure_output_authorized()?;
        self.managed_output(|renderer, session, output| {
            renderer.insert_rich_text_before_live(rich, session, output)
        })?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Strategy used by the most recent insertion, if any.
    pub fn last_insert_strategy(&self) -> Option<InsertStrategy> {
        self.renderer.last_insert_strategy
    }

    /// Clears the live region from the terminal without leaving artifacts.
    pub fn clear_live_region(&mut self) -> io::Result<()> {
        self.ensure_output_authorized()?;
        self.managed_output(|renderer, session, output| {
            renderer.clear_live_region(session, output)
        })?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Puts the terminal into interactive mode now: raw mode and bracketed paste, exactly what
    /// the first [`poll_event`](Self::poll_event) (or [`run_once`](Self::run_once)) does.
    ///
    /// [`Context::inline`] and [`Context::fullscreen`] leave this to the first poll. Output shown
    /// before that poll (a READY line, a first frame) is therefore on a **cooked** tty: the kernel
    /// echoes anything typed in that window, and its ICRNL turns Enter (`\r`) into `\n`, which raw
    /// decoding then reads as Ctrl+J, so a line typed at "ready" is never submitted (#296). Call
    /// this before the first output so that "the screen says ready" implies "the terminal is
    /// interactive":
    ///
    /// ```no_run
    /// use gibson::cell::Style;
    /// use gibson::context::Context;
    /// use gibson::node::Node;
    ///
    /// let mut ctx = Context::inline()?;
    /// ctx.enter_interactive()?;
    /// ctx.set_root(Node::text("READY", Style::default()));
    /// ctx.render()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// This replaces the workaround of calling `poll_event(Duration::ZERO)` before the first
    /// output, which also consumed (and could drop) an event.
    ///
    /// * **Idempotent.** A second call, and every later poll, finds raw mode and bracketed paste
    ///   already on and writes nothing.
    /// * **No-op returning `Ok`** on a headless context and when stdout is not a terminal.
    /// * **Restoration is unchanged.** What it enables is tracked by the session exactly as when
    ///   the first poll enables it, so [`restore`](Self::restore), `Drop` and the panic hook undo
    ///   it. If raw mode cannot be entered, the error is returned and nothing is left enabled; the
    ///   first poll fails the same way.
    /// * **After [`restore`](Self::restore)** the context has handed the terminal back and released
    ///   its ownership lease, so this returns an error and does not touch the terminal: raw mode
    ///   re-entered then would never be undone.
    pub fn enter_interactive(&mut self) -> io::Result<()> {
        if !self.session.is_tty || self.session.is_headless() {
            return Ok(());
        }
        if self.session.restore_attempted() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "enter_interactive after restore: this context has released the terminal",
            ));
        }
        self.session.enter_interactive()
    }

    /// Polls the controlling terminal for structured input events.
    ///
    /// Returns `Ok(None)` at once, without touching the process terminal, when there is no
    /// terminal input source: on a redirected / non-TTY stdout, and on a **headless**
    /// context ([`Context::headless`]), whose session is virtual (#259). A headless context's
    /// input comes from whatever source the caller passes to [`poll_from`](Self::poll_from):
    /// an injected source keeps it synthetic; passing
    /// [`TerminalSource`](crate::event_source::TerminalSource) there explicitly reads the real
    /// terminal, as that source always does.
    pub fn poll_event(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        if !self.session.is_tty || self.session.is_headless() {
            return Ok(None);
        }
        self.session.sync_key_release_reporting();
        // Before entering interactive mode: the panic hook left raw mode, and this restores it.
        let ended = self.session.take_key_releases_ended();
        if let Err(e) = self.session.enter_interactive() {
            if ended {
                // Not delivered: the next poll delivers it.
                self.session.keep_key_releases_ended();
            }
            return Err(e);
        }
        if ended {
            return Ok(Some(Event::KeyReleaseReporting(
                KeyReleaseReporting::Unsupported,
            )));
        }
        let ev = poll_event(timeout)?;
        if let Some(Event::Resize(_, _)) = ev {
            // Geometry is authoritative on the next render via observe_geometry;
            // marking dirty is enough to schedule it.
            self.scheduler.mark_dirty();
        }
        self.session.sync_key_release_reporting();
        Ok(ev)
    }

    /// Polls the next event from an injected
    /// [`EventSource`](crate::event_source::EventSource) instead of the terminal,
    /// so a headless, scripted, or recorded context can be driven through the same
    /// event path as a live TTY (the general injected-input path — SmartChem
    /// #131/G). Unlike [`poll_event`](Self::poll_event) this does **not** require a
    /// TTY: the injected source is the authority on event availability.
    ///
    /// A delivered `Resize(cols, rows)` marks the scheduler dirty, so the next render
    /// re-observes geometry. Where that geometry comes from depends on the session:
    ///
    /// * **Headless** ([`Context::headless`]): the session is virtual, so the delivered
    ///   resize *is* its new terminal size. It is applied before this returns, exactly
    ///   as the C ABI's `gibson_poll_event` does (both call the same implementation;
    ///   #293), and the next render lays out against it as against a real terminal: a
    ///   [`RenderMode::Fullscreen`] frame is `cols`×`rows`; an
    ///   [`Inline`](crate::RenderMode::Inline) frame is `cols` wide and as tall as its
    ///   content, at least 1 and at most `max(rows - 1, 1)` rows. A resize with a zero
    ///   dimension is still returned but is **not applied**: the geometry stays as it
    ///   was. The C ABI refuses to queue such a resize (`GIBSON_ERR_INVALID_PARAM`), so
    ///   on either side a headless context never takes a zero geometry from a resize.
    /// * **Terminal**: the geometry stays observed from the terminal itself; an injected
    ///   resize schedules a render but does not override what the terminal reports.
    pub fn poll_from<S>(&mut self, source: &mut S, timeout: Duration) -> io::Result<Option<Event>>
    where
        S: crate::event_source::EventSource + ?Sized,
    {
        self.session.sync_key_release_reporting();
        if self.session.take_key_releases_ended() {
            return Ok(Some(Event::KeyReleaseReporting(
                KeyReleaseReporting::Unsupported,
            )));
        }
        let ev = source.poll_event(timeout)?;
        if let Some(Event::Resize(cols, rows)) = ev {
            self.deliver_resize(cols, rows);
        }
        self.session.sync_key_release_reporting();
        Ok(ev)
    }

    /// The effect of a delivered `Resize` on an injected-input path: on a headless
    /// session, a non-zero geometry becomes the virtual terminal's size; on any session
    /// the next render is scheduled. Shared by [`poll_from`](Self::poll_from) and the
    /// C ABI's poll, so both apply a resize the same way (#293).
    fn deliver_resize(&mut self, cols: u16, rows: u16) {
        if self.session.is_headless() && cols != 0 && rows != 0 {
            self.session.set_terminal_size(cols, rows);
        }
        self.scheduler.mark_dirty();
    }

    /// Returns a snapshot of render statistics.
    ///
    /// Takes `&self`: it overlays the renderer's absolute operation counters
    /// onto the scheduler's accumulated frame stats and returns the result by
    /// value, without mutating the context, so it can be read through a shared
    /// borrow. (Those same counters are mirrored into the scheduler on every
    /// render/commit path, so reading never needs to write.) `RenderStats` is
    /// `Copy`.
    pub fn stats(&self) -> RenderStats {
        self.overlaid_stats()
    }

    pub fn restore(&mut self) -> io::Result<()> {
        let result = self.session.restore();
        // A completed one-shot restore releases the lease even on an I/O error;
        // do not keep a duplicate terminal descriptor open until Context drop.
        #[cfg(unix)]
        if matches!(self.output, OutputSink::PinnedTerminal(_)) {
            self.output = OutputSink::Stdout;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_rendered_bytes_round_trips_non_utf8_bytes() {
        let mut ctx = Context::headless(RenderMode::Inline, 20, 4);
        // A lone continuation byte, an overlong lead and 0xFF: `take_output` would replace these.
        let raw: &[u8] = b"\x80\xC0\xAF\xFFok";
        ctx.output.write_all(raw).unwrap();
        assert_eq!(ctx.rendered_bytes(), raw);
        assert_eq!(ctx.take_rendered_bytes(), raw);
        assert!(ctx.rendered_bytes().is_empty());
        assert!(ctx.take_rendered_bytes().is_empty());
    }
}
