use crate::cell::{Line, RichText};
use crate::input::{poll_event, Event};
use crate::node::Node;
use crate::renderer::{RenderMode, Renderer};
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
        let sync = session.sync_updates();

        Ok(Self {
            session,
            renderer: Renderer::new(mode, sync),
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

    pub fn set_sync_updates(&mut self, enabled: bool) {
        self.session.set_sync_updates(enabled);
        self.renderer.set_sync_updates(self.session.sync_updates());
    }

    pub fn set_max_fps(&mut self, fps: u32) {
        self.scheduler.max_fps = fps.max(1);
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
    pub fn render_now(&mut self) -> io::Result<()> {
        self.render_internal()
    }

    /// Standard render entry point (forces immediate frame update).
    pub fn render(&mut self) -> io::Result<()> {
        self.render_now()
    }

    fn render_internal(&mut self) -> io::Result<()> {
        let mut root = match self.root.take() {
            Some(r) => r,
            None => return Ok(()),
        };

        let start = Instant::now();
        let result = self.renderer.render(&mut root, &mut self.session);
        let duration = start.elapsed();

        self.root = Some(root);

        match result {
            Ok((dirty, total, bytes, full)) => {
                self.scheduler
                    .record_frame(dirty, total, bytes, full, duration);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Commits text to immutable scrollback, discarding it from the live framebuffer.
    pub fn commit(&mut self, text: &str) -> io::Result<()> {
        self.renderer.commit(text, &mut self.session)
    }

    /// Commits a laid-out UI node directly to immutable scrollback.
    pub fn commit_node(&mut self, node: &mut Node) -> io::Result<()> {
        self.renderer.commit_node(node, &mut self.session)
    }

    /// Commits rich text to immutable scrollback.
    pub fn commit_rich_text(&mut self, rich: &RichText) -> io::Result<()> {
        let text = if self.session.is_tty {
            rich.to_ansi()
        } else {
            rich.plain_text()
        };
        self.commit(&text)
    }

    /// Commits a sequence of styled lines to immutable scrollback.
    pub fn commit_lines(&mut self, lines: &[Line]) -> io::Result<()> {
        let rich = RichText::from_lines(lines.to_vec());
        self.commit_rich_text(&rich)
    }

    /// Inserts committed lines into scrollback ABOVE the active live region,
    /// preserving the active live region's content, geometry, cursor, and diff state.
    pub fn insert_before_live(&mut self, lines: &[&str]) -> io::Result<()> {
        self.renderer.insert_before_live(lines, &mut self.session)
    }

    /// Inserts a laid-out UI node into scrollback ABOVE the active live region.
    pub fn insert_node_before_live(&mut self, node: &mut Node) -> io::Result<()> {
        self.renderer
            .insert_node_before_live(node, &mut self.session)
    }

    /// Inserts rich text into scrollback ABOVE the active live region.
    pub fn insert_rich_text_before_live(&mut self, rich: &RichText) -> io::Result<()> {
        let rendered = if self.session.is_tty {
            rich.to_ansi()
        } else {
            rich.plain_text()
        };
        let lines: Vec<&str> = rendered.lines().collect();
        self.insert_before_live(&lines)
    }

    /// Clears the live region from the terminal without leaving artifacts.
    pub fn clear_live_region(&mut self) -> io::Result<()> {
        self.renderer.clear_live_region(&mut self.session)
    }

    /// Polls for structured input events.
    pub fn poll_event(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        self.session.enter_interactive()?;
        let ev = poll_event(timeout)?;
        if let Some(Event::Resize(_, _)) = ev {
            self.scheduler.mark_dirty();
        }
        Ok(ev)
    }

    pub fn stats(&self) -> RenderStats {
        self.scheduler.stats
    }

    pub fn restore(&mut self) -> io::Result<()> {
        self.session.restore()
    }
}
