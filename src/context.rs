use crate::cell::RichText;
use crate::input::{poll_event, Event};
use crate::node::Node;
use crate::renderer::{FrameReport, InsertStrategy, Renderer};
use crate::scheduler::{FramePacing, FrameScheduler, RenderStats};
use crate::session::TerminalSession;
use std::io::{self, Write};
use std::time::{Duration, Instant};

/// `RenderMode` is consumed by [`Context::new`] and [`Context::headless`], so it
/// is re-exported here for ergonomic imports (`gibson::context::RenderMode`) in
/// addition to the crate root (`gibson::RenderMode`).
pub use crate::renderer::RenderMode;

/// High-level engine coordinator managing session, rendering, input, and scheduling.
pub struct Context {
    pub session: TerminalSession,
    pub renderer: Renderer,
    pub scheduler: FrameScheduler,
    root: Option<Node>,
    /// Where rendered bytes go: the process stdout for interactive contexts, or
    /// an in-memory buffer for headless ones (see [`Context::rendered_bytes`]).
    output: OutputSink,
    /// Count of `render`/`render_now` calls issued with no root set — the most
    /// likely "forgot `set_root`" integration mistake (issue #48 E-01). A
    /// rootless render is a silent wire-level no-op; this counter makes it
    /// observable and distinct from a legitimately empty settled frame.
    empty_root_renders: u64,
}

/// Destination for a [`Context`]'s rendered bytes.
///
/// Interactive contexts write straight to the process stdout. A headless context
/// captures into an in-memory buffer instead, so callers can read the exact bytes
/// back for snapshot/assertion testing without any escape codes reaching a real
/// terminal (issue #27).
enum OutputSink {
    Stdout,
    Buffer(Vec<u8>),
}

impl Write for OutputSink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            OutputSink::Stdout => io::stdout().write(buf),
            OutputSink::Buffer(b) => b.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            OutputSink::Stdout => io::stdout().flush(),
            OutputSink::Buffer(b) => b.flush(),
        }
    }
}

impl Context {
    pub fn new(mode: RenderMode) -> io::Result<Self> {
        let mut session = TerminalSession::new()?;
        if mode == RenderMode::Fullscreen {
            session.enter_alternate_screen()?;
        }

        Ok(Self {
            session,
            renderer: Renderer::new(mode),
            scheduler: FrameScheduler::new(60),
            root: None,
            output: OutputSink::Stdout,
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
    /// [`Context::take_output`]:
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
        Self {
            session: TerminalSession::headless(cols, rows),
            renderer: Renderer::new(mode),
            scheduler: FrameScheduler::new(60),
            root: None,
            output: OutputSink::Buffer(Vec::new()),
            empty_root_renders: 0,
        }
    }

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
        self.renderer.last_dirty_cells()
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

    /// Time until the next frame is allowed under the FPS budget.
    pub fn time_until_next_frame(&self) -> Duration {
        self.scheduler.time_until_next_frame()
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
    /// [`Context::take_output`]. Always empty for an interactive context, whose
    /// output goes straight to the process stdout.
    pub fn rendered_bytes(&self) -> &[u8] {
        match &self.output {
            OutputSink::Buffer(b) => b.as_slice(),
            OutputSink::Stdout => &[],
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
        self.renderer.last_exact_changed()
    }

    /// Timing/accounting snapshot for the most recent live frame (#64).
    ///
    /// Generation covers layout/paint/diff/ANSI transaction construction;
    /// write covers the blocking writer write/flush. Neither duration claims to
    /// measure terminal-compositor or physical display presentation.
    pub fn last_frame_report(&self) -> FrameReport {
        self.renderer.last_frame_report()
    }

    /// Visible text of the most recently composed live frame, one `String` per
    /// row (issue #48 E-02): "what the screen says", with wide-glyph continuation
    /// cells collapsed and trailing blanks trimmed. Complements the raw wire
    /// bytes from [`Context::rendered_bytes`] / [`Context::take_output`] and lets
    /// headless tests assert on visible text without a private terminal emulator.
    pub fn last_frame_lines(&self) -> Vec<String> {
        self.renderer.last_frame_lines()
    }

    /// Minimal, non-async runtime step — the core of the **canonical interactive
    /// loop** (issue #48 E-07).
    ///
    /// Waits for at most `max_wait` (bounded by the next frame deadline), polls
    /// for input, then renders if the scheduler permits. Input wakeups have
    /// priority over decorative animation: an arriving event shortens the wait
    /// immediately. Returns the input event, if any.
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
        let until_frame = self.scheduler.time_until_next_frame();
        let wait = max_wait.min(until_frame);
        let event = if self.session.is_tty {
            self.poll_event(wait)?
        } else {
            // No input source off-TTY: sleep for the frame slice instead of
            // busy-spinning, then render if due.
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
        let mut root = match self.root.take() {
            Some(r) => r,
            None => {
                // Rendering with no root is almost always a forgotten
                // `set_root` (issue #48 E-01). It is a wire-level no-op, so make
                // it observable rather than silent — without panicking a valid
                // production flow or changing the ABI-1 stats struct.
                self.empty_root_renders += 1;
                self.sync_renderer_metrics();
                return Ok(crate::painter::PaintContext::default());
            }
        };

        let start = Instant::now();
        let result = self
            .renderer
            .render(&mut root, &mut self.session, &mut self.output);
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
        self.renderer
            .commit_text(text, &mut self.session, &mut self.output)?;
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
        self.renderer
            .commit_text_with_mode(text, wrap, &mut self.session, &mut self.output)?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Raw ANSI escape hatch. The caller is responsible for the payload.
    pub fn commit_raw_ansi_unchecked(&mut self, text: &str) -> io::Result<()> {
        self.renderer
            .commit_raw_ansi_unchecked(text, &mut self.session, &mut self.output)?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Backwards-compatible alias for [`Context::commit_text`].
    pub fn commit(&mut self, text: &str) -> io::Result<()> {
        self.commit_text(text)
    }

    /// Commits a laid-out UI node directly to immutable scrollback.
    pub fn commit_node(&mut self, node: &mut Node) -> io::Result<()> {
        self.renderer
            .commit_node(node, &mut self.session, &mut self.output)?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Commits structured rich text to immutable scrollback, routed through the
    /// same wrapping/alignment engine as live nodes.
    pub fn commit_rich_text(&mut self, rich: &RichText) -> io::Result<()> {
        self.renderer
            .commit_rich_text(rich, &mut self.session, &mut self.output)?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Inserts **raw** lines into scrollback above the live region. The lines are
    /// treated as terminal byte streams and are **not** sanitized — escape
    /// sequences reach the terminal. Prefer the safe insertion methods.
    pub fn insert_raw_lines_before_live_unchecked(&mut self, lines: &[&str]) -> io::Result<()> {
        self.renderer.insert_raw_lines_before_live_unchecked(
            lines,
            &mut self.session,
            &mut self.output,
        )?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Inserts safe, width-aware plain text into scrollback above the live region.
    /// Control characters are neutralized by the cell model.
    pub fn insert_text_before_live(&mut self, text: &str) -> io::Result<()> {
        self.renderer
            .insert_text_before_live(text, &mut self.session, &mut self.output)?;
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
        self.renderer.insert_text_before_live_with_mode(
            text,
            wrap,
            &mut self.session,
            &mut self.output,
        )?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Inserts a laid-out UI node into scrollback ABOVE the active live region.
    pub fn insert_node_before_live(&mut self, node: &mut Node) -> io::Result<()> {
        self.renderer
            .insert_node_before_live(node, &mut self.session, &mut self.output)?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Inserts safe rich text into scrollback ABOVE the active live region, using
    /// the width-aware layout engine.
    pub fn insert_rich_text_before_live(&mut self, rich: &RichText) -> io::Result<()> {
        self.renderer
            .insert_rich_text_before_live(rich, &mut self.session, &mut self.output)?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Strategy used by the most recent insertion, if any.
    pub fn last_insert_strategy(&self) -> Option<InsertStrategy> {
        self.renderer.last_insert_strategy
    }

    /// Clears the live region from the terminal without leaving artifacts.
    pub fn clear_live_region(&mut self) -> io::Result<()> {
        self.renderer
            .clear_live_region(&mut self.session, &mut self.output)?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Polls for structured input events.
    pub fn poll_event(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        if !self.session.is_tty {
            // No interactive input source on a redirected / non-TTY stdout.
            return Ok(None);
        }
        self.session.enter_interactive()?;
        let ev = poll_event(timeout)?;
        if let Some(Event::Resize(_, _)) = ev {
            // Geometry is authoritative on the next render via observe_geometry;
            // marking dirty is enough to schedule it.
            self.scheduler.mark_dirty();
        }
        Ok(ev)
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
        self.session.restore()
    }
}
