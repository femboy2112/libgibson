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

    /// Renders the current UI tree if marked dirty.
    pub fn render(&mut self) -> io::Result<()> {
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
