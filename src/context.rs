use crate::cell::RichText;
use crate::input::{poll_event, Event};
use crate::node::Node;
use crate::renderer::{InsertStrategy, RenderMode, Renderer};
use crate::scheduler::{FrameScheduler, RenderStats};
use crate::session::TerminalSession;
use std::io;
use std::time::{Duration, Instant};

/// High-level engine coordinator managing session, rendering, input, and scheduling.
pub struct Context {
    pub session: TerminalSession,
    pub renderer: Renderer,
    pub scheduler: FrameScheduler,
    root: Option<Node>,
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
    pub fn headless(mode: RenderMode, cols: u16, rows: u16) -> Self {
        Self {
            session: TerminalSession::headless(cols, rows),
            renderer: Renderer::new(mode),
            scheduler: FrameScheduler::new(60),
            root: None,
        }
    }

    pub fn set_sync_updates(&mut self, enabled: bool) {
        self.session.set_sync_updates(enabled);
    }

    pub fn set_max_fps(&mut self, fps: u32) {
        self.scheduler.max_fps = fps.max(1);
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

    /// Minimal, non-async runtime step.
    ///
    /// Waits for at most `max_wait` (bounded by the next frame deadline), polls
    /// for input, then renders if the scheduler permits. Input wakeups have
    /// priority over decorative animation: an arriving event shortens the wait
    /// immediately. Returns the input event, if any.
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
                self.sync_renderer_metrics();
                return Ok(crate::painter::PaintContext::default());
            }
        };

        let start = Instant::now();
        let result = self
            .renderer
            .render(&mut root, &mut self.session, &mut std::io::stdout());
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

    /// Mirrors absolute renderer operation counters into the scheduler stats.
    fn sync_renderer_metrics(&mut self) {
        let r = &self.renderer;
        let s = &mut self.scheduler.stats;
        s.anchor_resyncs = r.anchor_resyncs;
        s.history_insertions = r.history_insertions;
        s.fast_insertions = r.fast_insertions;
        s.insertion_repaints = r.fallback_insertions;
        s.insertion_bytes = r.total_insertion_bytes;
        s.commit_bytes = r.total_commit_bytes;
        s.control_bytes = r.total_control_bytes;
    }

    /// Commits structured plain text to immutable scrollback (width-aware
    /// wrapping; embedded escape sequences are treated as literal text and
    /// cannot corrupt the terminal).
    pub fn commit_text(&mut self, text: &str) -> io::Result<()> {
        self.renderer
            .commit_text(text, &mut self.session, &mut std::io::stdout())?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Raw ANSI escape hatch. The caller is responsible for the payload.
    pub fn commit_raw_ansi_unchecked(&mut self, text: &str) -> io::Result<()> {
        self.renderer
            .commit_raw_ansi_unchecked(text, &mut self.session, &mut std::io::stdout())?;
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
            .commit_node(node, &mut self.session, &mut std::io::stdout())?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Commits structured rich text to immutable scrollback, routed through the
    /// same wrapping/alignment engine as live nodes.
    pub fn commit_rich_text(&mut self, rich: &RichText) -> io::Result<()> {
        self.renderer
            .commit_rich_text(rich, &mut self.session, &mut std::io::stdout())?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Inserts already-safe lines into scrollback ABOVE the active live region.
    pub fn insert_before_live(&mut self, lines: &[&str]) -> io::Result<()> {
        self.renderer
            .insert_before_live(lines, &mut self.session, &mut std::io::stdout())?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Inserts a laid-out UI node into scrollback ABOVE the active live region.
    pub fn insert_node_before_live(&mut self, node: &mut Node) -> io::Result<()> {
        self.renderer
            .insert_node_before_live(node, &mut self.session, &mut std::io::stdout())?;
        self.sync_renderer_metrics();
        Ok(())
    }

    /// Inserts rich text into scrollback ABOVE the active live region, using the
    /// width-aware layout engine.
    pub fn insert_rich_text_before_live(&mut self, rich: &RichText) -> io::Result<()> {
        let (term_cols, _) = self.session.terminal_size();
        let mut node = Node::rich_text_wrapped(rich.clone(), crate::node::WrapMode::WordWrap);
        node.layout_style.width = crate::node::Dimension::Length(term_cols as f32);
        self.insert_node_before_live(&mut node)
    }

    /// Strategy used by the most recent insertion, if any.
    pub fn last_insert_strategy(&self) -> Option<InsertStrategy> {
        self.renderer.last_insert_strategy
    }

    /// Clears the live region from the terminal without leaving artifacts.
    pub fn clear_live_region(&mut self) -> io::Result<()> {
        self.renderer
            .clear_live_region(&mut self.session, &mut std::io::stdout())?;
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

    pub fn stats(&mut self) -> RenderStats {
        self.sync_renderer_metrics();
        self.scheduler.stats
    }

    pub fn restore(&mut self) -> io::Result<()> {
        self.session.restore()
    }
}
