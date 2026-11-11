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
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

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

/// Rust-side timing/accounting snapshot for the most recent live frame.
///
/// This deliberately does not extend the ABI-v1 RenderStats layout. Generation
/// covers layout, paint, diff and ANSI transaction construction; write covers the
/// blocking writer write/flush. It measures only stages LibGibson owns and does
/// not claim anything about compositor/display presentation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct FrameReport {
    pub exact_changed_cells: usize,
    pub affected_cells: usize,
    pub total_cells: usize,
    pub bytes_emitted: usize,
    pub full_repaint: bool,
    pub generation_duration: Duration,
    pub write_duration: Duration,
}

impl FrameReport {
    pub fn total_duration(&self) -> Duration {
        self.generation_duration + self.write_duration
    }
}

/// The core differential terminal renderer.
pub struct Renderer {
    pub mode: RenderMode,
    identity: Arc<()>,
    bound_target: Arc<AtomicU8>,
    bound_initial: bool,
    // A Context records this before and after its own output operations. Direct
    // public Renderer output may have used an arbitrary writer, so the Context
    // must not reuse the resulting differential state as physical truth.
    output_generation: u64,
    output_generation_exhausted: bool,
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

    /// When true, the renderer records the explicit coordinates of this frame's
    /// dirty cells (for damage-map debug overlays). Off by default: it allocates.
    pub capture_damage: bool,
    last_dirty_cells: Vec<(u16, u16)>,
    /// Exact semantic changed-cell count of the most recent rendered frame
    /// (issue #47) — the precomputed `SurfaceDiff::exact_changed_cell_count()`,
    /// distinct from the logical `last_dirty_cells` footprint. Always updated
    /// (a single scalar, so no `capture_damage` gate).
    last_exact_changed: usize,
    last_frame_report: FrameReport,
    /// The color depth the most recent frame was emitted at. A mid-session change
    /// away from this (via `Context::set_color_depth`) forces an in-place restyle
    /// repaint on the next frame, so a depth change after the first render is no
    /// longer left stale on screen (closes the J1a/J1b "set depth before the first
    /// render" caveat).
    last_emitted_depth: Option<crate::capability::ColorDepth>,
    /// A commit FAILED, possibly after a partial write (RXIV Gate G3): the terminal may hold a torn
    /// prefix of a frame, so the renderer's belief about the physical screen (the previous surface,
    /// the SGR the compiler thinks is set, the cursor, the inline region) is void. The next frame
    /// re-establishes everything instead of diffing against a belief that is no longer true.
    physical_uncertain: bool,
    // Inline history operations can recover their region independently of
    // cursor visibility. Keep that uncertainty until a frame sets visibility.
    cursor_visibility_uncertain: bool,
    /// A frame that had to RE-ANCHOR (the terminal geometry changed) failed without delivering a
    /// byte: the geometry was already observed, so the retry must still owe the canvas reset the
    /// resize required (RXIV review finding 3). Cleared when a frame lands.
    owed_reanchor: bool,
}

/// An exclusive renderer/session/writer binding. The caller must keep the
/// writer directed to the same destination and prevent other writes to that
/// destination for the binding's lifetime. A `Write` cannot enforce either
/// promise for aliased or internally retargetable destinations. Successful
/// writes and flushes report writer acceptance, not terminal presentation.
pub struct BoundTarget<'a> {
    renderer: &'a mut Renderer,
    session: &'a mut TerminalSession,
    writer: &'a mut dyn Write,
}

/// Initial contents are known only for the first operation on a Context-owned
/// virtual display. An arbitrary writer may already contain unrelated cells.
#[derive(Clone, Copy)]
pub(crate) enum TargetBaseline {
    Unknown = 1,
    FreshBlankVirtual = 2,
}

pub(crate) struct TargetScope(Arc<AtomicU8>);
impl Drop for TargetScope {
    fn drop(&mut self) {
        self.0.store(0, Ordering::Relaxed);
    }
}

impl BoundTarget<'_> {
    /// Update the headless terminal geometry without changing the destination.
    pub fn set_terminal_size(&mut self, cols: u16, rows: u16) {
        self.session.set_terminal_size(cols, rows);
    }
    pub fn set_capabilities(&mut self, caps: crate::capability::TerminalCapabilities) {
        self.session.set_capabilities(caps);
    }
    pub fn set_color_depth(&mut self, depth: crate::capability::ColorDepth) {
        self.session.set_color_depth(depth);
    }
    pub fn set_sync_updates(&mut self, enabled: bool) {
        self.session.set_sync_updates(enabled);
    }
    pub fn set_capture_damage(&mut self, enabled: bool) {
        self.renderer.capture_damage = enabled;
    }
    pub fn last_frame_report(&self) -> FrameReport {
        self.renderer.last_frame_report()
    }
    pub fn last_frame_lines(&self) -> Vec<String> {
        self.renderer.last_frame_lines()
    }
    pub fn last_dirty_cells(&self) -> &[(u16, u16)] {
        self.renderer.last_dirty_cells()
    }
    pub fn last_exact_changed(&self) -> usize {
        self.renderer.last_exact_changed()
    }
    /// Height of the current live region in terminal rows.
    pub fn live_region_height(&self) -> u16 {
        self.renderer.live_region_height
    }
    /// Most recently committed cursor row relative to the live region.
    pub fn last_cursor_y(&self) -> u16 {
        self.renderer.last_cursor_y
    }
    /// Number of renderer anchor resynchronizations.
    pub fn anchor_resyncs(&self) -> u64 {
        self.renderer.anchor_resyncs
    }
    /// Strategy used by the most recent insertion, if any.
    pub fn last_insert_strategy(&self) -> Option<InsertStrategy> {
        self.renderer.last_insert_strategy
    }
    /// Number of history insertions that used the fast path.
    pub fn fast_insertions(&self) -> u64 {
        self.renderer.fast_insertions
    }
    fn run<T>(
        &mut self,
        f: impl FnOnce(&mut Renderer, &mut TerminalSession, &mut dyn Write) -> io::Result<T>,
    ) -> io::Result<T> {
        let _scope = self.renderer.context_target_scope(TargetBaseline::Unknown);
        let before = self.session.cursor_hidden_for_render();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f(self.renderer, self.session, self.writer)
        })) {
            Ok(result) => result,
            Err(payload) => {
                self.session.reconcile_failed_cursor_write(before, true);
                self.renderer.poison_physical_state();
                std::panic::resume_unwind(payload)
            }
        }
    }
    pub fn render(
        &mut self,
        node: &mut Node,
    ) -> io::Result<(usize, usize, usize, bool, crate::painter::PaintContext)> {
        self.run(|r, s, w| r.render(node, s, w))
    }
    pub fn commit_text(&mut self, text: &str) -> io::Result<usize> {
        self.run(|r, s, w| r.commit_text(text, s, w))
    }
    /// Bound projection of the legacy safe-text commit alias.
    #[deprecated(note = "use `commit_text`, `commit_rich_text`, or `commit_node`")]
    #[allow(deprecated)]
    pub fn commit(&mut self, text: &str) -> io::Result<()> {
        self.run(|r, s, w| r.commit(text, s, w))
    }
    pub fn commit_text_with_mode(&mut self, text: &str, wrap: WrapMode) -> io::Result<usize> {
        self.run(|r, s, w| r.commit_text_with_mode(text, wrap, s, w))
    }
    pub fn commit_raw_ansi_unchecked(&mut self, text: &str) -> io::Result<usize> {
        self.run(|r, s, w| r.commit_raw_ansi_unchecked(text, s, w))
    }
    pub fn commit_node(&mut self, node: &mut Node) -> io::Result<usize> {
        self.run(|r, s, w| r.commit_node(node, s, w))
    }
    pub fn commit_rich_text(&mut self, rich: &RichText) -> io::Result<usize> {
        self.run(|r, s, w| r.commit_rich_text(rich, s, w))
    }
    pub fn insert_text_before_live(&mut self, text: &str) -> io::Result<(usize, InsertStrategy)> {
        self.run(|r, s, w| r.insert_text_before_live(text, s, w))
    }
    pub fn insert_text_before_live_with_mode(
        &mut self,
        text: &str,
        wrap: WrapMode,
    ) -> io::Result<(usize, InsertStrategy)> {
        self.run(|r, s, w| r.insert_text_before_live_with_mode(text, wrap, s, w))
    }
    pub fn insert_raw_lines_before_live_unchecked(
        &mut self,
        lines: &[&str],
    ) -> io::Result<(usize, InsertStrategy)> {
        self.run(|r, s, w| r.insert_raw_lines_before_live_unchecked(lines, s, w))
    }
    pub fn insert_node_before_live(
        &mut self,
        node: &mut Node,
    ) -> io::Result<(usize, InsertStrategy)> {
        self.run(|r, s, w| r.insert_node_before_live(node, s, w))
    }
    pub fn insert_rich_text_before_live(
        &mut self,
        rich: &RichText,
    ) -> io::Result<(usize, InsertStrategy)> {
        self.run(|r, s, w| r.insert_rich_text_before_live(rich, s, w))
    }
    pub fn clear_live_region(&mut self) -> io::Result<usize> {
        self.run(|r, s, w| r.clear_live_region(s, w))
    }
    pub fn physical_state_uncertain(&self) -> bool {
        self.renderer.physical_state_uncertain()
    }
}

impl Renderer {
    /// Binds output state to one writer for the lifetime of the returned guard.
    /// Only a fresh renderer can be bound, because earlier destination state is
    /// not transferable. The writer must not be retargeted or written to elsewhere.
    pub fn bind_target<'a>(
        &'a mut self,
        session: &'a mut TerminalSession,
        writer: &'a mut dyn Write,
    ) -> io::Result<BoundTarget<'a>> {
        session.ensure_render_authorized()?;
        if self.output_generation() != Some(0)
            || self.physical_fingerprint() != (self.mode, 0, 0, 0, None, AnchorState::Invalid)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "binding requires a fresh renderer",
            ));
        }
        self.begin_output();
        self.bound_initial = true;
        Ok(BoundTarget {
            renderer: self,
            session,
            writer,
        })
    }
    pub fn new(mode: RenderMode) -> Self {
        Self {
            mode,
            identity: Arc::new(()),
            bound_target: Arc::new(AtomicU8::new(0)),
            bound_initial: false,
            output_generation: 0,
            output_generation_exhausted: false,
            previous_surface: None,
            last_emitted_depth: None,
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
            capture_damage: false,
            last_dirty_cells: Vec::new(),
            last_exact_changed: 0,
            last_frame_report: FrameReport::default(),
            physical_uncertain: false,
            cursor_visibility_uncertain: false,
            owed_reanchor: false,
        }
    }

    pub(crate) fn identity(&self) -> Arc<()> {
        Arc::clone(&self.identity)
    }

    pub(crate) fn context_target_scope(&self, baseline: TargetBaseline) -> TargetScope {
        assert_eq!(self.bound_target.swap(baseline as u8, Ordering::Relaxed), 0);
        TargetScope(Arc::clone(&self.bound_target))
    }

    fn has_bound_target(&self) -> bool {
        self.bound_target.load(Ordering::Relaxed) != 0
    }

    fn has_fresh_blank_virtual_target(&self) -> bool {
        self.bound_target.load(Ordering::Relaxed) == TargetBaseline::FreshBlankVirtual as u8
    }

    fn require_bound_target(&self, session: &TerminalSession) -> io::Result<()> {
        if session.is_tty && !self.has_bound_target() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "interactive output requires a bound renderer target",
            ));
        }
        Ok(())
    }

    pub(crate) fn output_generation(&self) -> Option<u64> {
        (!self.output_generation_exhausted).then_some(self.output_generation)
    }

    pub(crate) fn physical_fingerprint(
        &self,
    ) -> (RenderMode, u16, u16, u16, Option<bool>, AnchorState) {
        (
            self.mode,
            self.live_region_height,
            self.last_cursor_y,
            self.last_cursor_x,
            self.last_cursor_visible,
            self.anchor,
        )
    }

    fn begin_output(&mut self) {
        match self.output_generation.checked_add(1) {
            Some(next) => self.output_generation = next,
            None => self.output_generation_exhausted = true,
        }
    }

    /// Whether a failed terminal write left the screen or cursor visibility
    /// uncertain. A successful frame re-establishes the renderer's state.
    pub fn physical_state_uncertain(&self) -> bool {
        self.physical_uncertain || self.cursor_visibility_uncertain
    }

    /// Record a failed commit: the terminal may hold any prefix of the bytes that were attempted.
    /// Nothing can un-send them, so the honest response is to void every belief about the physical
    /// state and let the next frame re-establish it (RXIV Gate G3).
    fn poison_physical_state(&mut self) {
        self.physical_uncertain = true;
        self.cursor_visibility_uncertain = true;
        self.previous_surface = None;
        self.last_cursor_visible = None;
        self.last_cursor_desired = None;
        // The inline region is addressed by relative motion from a cursor row we no longer know:
        // it cannot be repaired in place, only abandoned.
        self.live_region_height = 0;
        self.last_cursor_y = 0;
        self.last_cursor_x = 0;
    }

    /// Coordinates of the cells changed by the most recent rendered frame.
    ///
    /// Empty unless [`Renderer::capture_damage`] was enabled for that frame.
    pub fn last_dirty_cells(&self) -> &[(u16, u16)] {
        &self.last_dirty_cells
    }

    /// Exact semantic changed-cell count of the most recent rendered frame
    /// (issue #47): the number of cells whose final state actually changed,
    /// distinct from the logical affected footprint ([`Renderer::last_dirty_cells`]
    /// / [`crate::RenderStats::dirty_cells`]) and from emitted bytes.
    pub fn last_exact_changed(&self) -> usize {
        self.last_exact_changed
    }

    /// Timing/accounting snapshot of the most recent live render.
    pub fn last_frame_report(&self) -> FrameReport {
        self.last_frame_report
    }

    /// Visible text of the most recently composed live frame, one `String` per
    /// row (issue #48 E-02): continuation cells collapsed, empty cells rendered
    /// as spaces, trailing blanks trimmed. Empty before the first render. Raw
    /// wire bytes remain available via the owning [`crate::Context`].
    pub fn last_frame_lines(&self) -> Vec<String> {
        self.previous_surface
            .as_ref()
            .map(Surface::to_visible_lines)
            .unwrap_or_default()
    }

    /// The resolved cell surface of the most recently composed live frame, or `None` before the
    /// first render. Carries the raw, un-quantized styles; [`crate::Context::frame_cell`] quantizes
    /// them to the frame's *emitted* color depth (see [`Renderer::last_emitted_depth`]) for a
    /// display-accurate readback.
    pub(crate) fn last_frame_surface(&self) -> Option<&Surface> {
        self.previous_surface.as_ref()
    }

    /// The color depth the most recently *committed* frame was emitted at, or `None` before the
    /// first successful render. Advanced only after a successful commit and alongside
    /// `previous_surface` (Gate F1), so it always corresponds to the surface
    /// [`Renderer::last_frame_surface`] returns. A readback of that frame must quantize with this —
    /// the depth actually on the wire — not a depth requested afterward (Gate F2).
    pub(crate) fn last_emitted_depth(&self) -> Option<crate::capability::ColorDepth> {
        self.last_emitted_depth
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
    ///
    /// Without [`Renderer::bind_target`], interactive fullscreen calls reset
    /// and repaint the entire canvas on every call: `Write` does not identify a
    /// destination. Corresponding damage and frame reports describe a full
    /// repaint. Interactive inline rendering requires a bound target. Context
    /// uses its own managed output path.
    ///
    /// Returns `(affected_cells, total_cells, bytes_emitted, is_full_repaint,
    /// PaintContext)`. `affected_cells` is the logical affected footprint
    /// (`SurfaceDiff::logical_dirty_count`), **not** an exact state delta; see
    /// [`crate::SurfaceDiff`] and [`crate::RenderStats::dirty_cells`].
    pub fn render(
        &mut self,
        root: &mut Node,
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<(usize, usize, usize, bool, crate::painter::PaintContext)> {
        self.begin_output();
        session.ensure_render_authorized()?;
        if !session.is_tty {
            // Non-TTY / CI mode: suppress live interactive frames.
            self.last_frame_report = FrameReport::default();
            return Ok((0, 0, 0, false, crate::painter::PaintContext::default()));
        }
        if !self.has_bound_target() {
            if self.mode == RenderMode::Inline {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "inline rendering requires a bound renderer target",
                ));
            }
            // Arbitrary writers have no stable identity. Re-establish every
            // fullscreen cell and cursor request on each unbound call.
            self.poison_physical_state();
        }

        let frame_start = Instant::now();
        let (term_cols, term_rows) = session.terminal_size();
        // Color quality is decided centrally from the session's capabilities.
        self.compiler.color_depth = session.color_depth();
        // A mid-session color-depth change (J1a's `gibson_set_color_depth`) must not
        // leave stale SGR on cells the RAW diff considers unchanged. Force an in-place
        // RESTYLE repaint — re-emit every content cell at the new depth — on the first
        // frame whose depth differs from what we last emitted. Gated on an existing
        // frame: the first frame is already a full repaint, and a resize (reanchor)
        // nulls `previous_surface`, so restyle collides with neither and never
        // reallocates Inline rows (which would ghost the old region H-1 rows below).
        let restyle = self.previous_surface.is_some()
            && self
                .last_emitted_depth
                .is_some_and(|d| d != self.compiler.color_depth);
        // `last_emitted_depth` is *physical-emission* state: it records the depth of the SGR now on
        // the terminal's cells, and it is advanced only AFTER a successful commit (alongside
        // `previous_surface`), never here. Advancing it before the write — as this once did — made
        // a FAILED depth-change write mark the new depth as emitted, so the retry computed
        // `restyle == false` and silently skipped the repaint, leaving stale old-depth SGR on
        // screen (Gate F1). On a failed commit the old value survives, so the retry restyles.
        let reanchor = self.observe_geometry(term_cols, term_rows) || self.owed_reanchor;
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

        // 3. Diff pass. On a restyle, diff against a blank baseline so every content
        // cell is re-emitted at the new depth (the raw surface is unchanged, so a
        // normal diff would emit nothing); the region is erased first (below) so no
        // stale old-depth bytes survive.
        let diff = if restyle {
            compute_diff(None, &next_surface)
        } else {
            compute_diff(self.previous_surface.as_ref(), &next_surface)
        };
        // Issue #47: expose the exact semantic changed-cell scalar the diff
        // already computed, so a consumer need not rerun layout/paint/diff to
        // observe it. Updated on every render, including settled/empty frames.
        self.last_exact_changed = diff.exact_changed_cell_count();

        let total_cells = (surface_width as usize) * (surface_height as usize);
        // Report *logical* damage (runs + erase-to-EOL + cleared rows), not just
        // explicit run cells: a shrinking line erases real cells even when its
        // wire cost is one CSI K. Wire cost is returned separately as bytes.
        // A fullscreen resize invalidates the whole physical canvas, including
        // blank cells omitted by a fresh diff. Its clear addresses every cell.
        // RXIV Gate G3: after a failed (possibly torn) commit the canvas holds unknown content, so a
        // fullscreen recovery frame clears it exactly like a reanchor does.
        let recover = self.physical_uncertain;
        let initial_target = self.has_bound_target() && self.bound_initial;
        let force_cursor =
            recover || self.cursor_visibility_uncertain || initial_target || is_full_repaint;
        let clears_canvas = self.mode == RenderMode::Fullscreen
            && (reanchor || restyle || recover || is_full_repaint);
        let clears_inline = self.mode == RenderMode::Inline
            && ((is_full_repaint
                && self.has_bound_target()
                && !self.has_fresh_blank_virtual_target())
                || restyle);
        let cleared_inline_height = if restyle {
            self.live_region_height.max(surface_height)
        } else {
            surface_height
        };
        let grows_inline = self.mode == RenderMode::Inline
            && !is_full_repaint
            && self.live_region_height > 0
            && surface_height > self.live_region_height;
        let dirty_coords = if grows_inline {
            let mut cells = diff.logical_dirty_cells();
            cells.retain(|&(_, y)| y < self.live_region_height || y >= surface_height);
            cells.extend(
                (self.live_region_height..surface_height)
                    .flat_map(|y| (0..surface_width).map(move |x| (x, y))),
            );
            Some(cells)
        } else {
            None
        };
        let dirty_cells = if clears_canvas {
            total_cells
        } else if clears_inline {
            surface_width as usize * cleared_inline_height as usize
        } else if let Some(cells) = &dirty_coords {
            cells.len()
        } else {
            diff.logical_dirty_count()
        };
        if self.capture_damage {
            self.last_dirty_cells = if clears_canvas {
                (0..surface_height)
                    .flat_map(|y| (0..surface_width).map(move |x| (x, y)))
                    .collect()
            } else if clears_inline {
                (0..cleared_inline_height)
                    .flat_map(|y| (0..surface_width).map(move |x| (x, y)))
                    .collect()
            } else if let Some(cells) = dirty_coords {
                cells
            } else {
                diff.logical_dirty_cells()
            };
        } else {
            self.last_dirty_cells.clear();
        }

        // Cursor visibility/position are part of physical truth: an unchanged
        // framebuffer does not imply "nothing needs emitting".
        let desired_visible = paint_ctx.cursor_position.is_some();
        let cursor_state_changed = self.cursor_visibility_uncertain
            || self.last_cursor_visible != Some(desired_visible)
            || self.last_cursor_desired != paint_ctx.cursor_position;

        if diff.is_empty() && !is_full_repaint && !cursor_state_changed && !grows_inline {
            self.last_frame_report = FrameReport {
                exact_changed_cells: self.last_exact_changed,
                affected_cells: 0,
                total_cells,
                bytes_emitted: 0,
                full_repaint: false,
                generation_duration: frame_start.elapsed(),
                write_duration: Duration::ZERO,
            };
            return Ok((0, total_cells, 0, false, paint_ctx));
        }

        // 4. ANSI compilation and execution (single transaction — NOT atomic I/O, see Gate G3)
        // Snapshot the emission state this frame is about to advance, so a failure that delivered
        // ZERO bytes can restore it exactly (the terminal is then provably unchanged).
        let pre_frame = (
            self.compiler.cursor_x,
            self.compiler.cursor_y,
            self.compiler.current_style,
            self.last_cursor_x,
            self.last_cursor_y,
            self.live_region_height,
        );
        let cursor_hidden_before = session.cursor_hidden_for_render();
        let mut tx = TerminalTransaction::new(writer, session.sync_updates());
        tx.begin();

        match self.mode {
            RenderMode::Fullscreen => {
                // Home the *physical* cursor before compiling: the compiler's
                // relative motion is meaningless unless the real cursor is where
                // the compiler thinks it is. Without this, each frame drifts by
                // the previous frame's final cursor and eventually scrolls.
                tx.push(b"\x1b[H");
                if clears_canvas {
                    // Fresh diffs elide default blanks. After a resize those
                    // blanks can contain reflowed old content; establish the
                    // blank baseline inside this same transaction. A recovery
                    // frame also re-enables auto-wrap, which a torn frame may
                    // have left off (RXIV review finding 5).
                    if recover {
                        tx.push(b"\x1b[?7h");
                    }
                    tx.push(b"\x1b[0m\x1b[2J");
                    self.compiler.current_style = Style::default();
                }
                self.compiler.reset_cursor(0, 0);
                let bytes = self.compiler.compile(&diff);
                tx.push(&bytes);

                if let Some((cx, cy)) = paint_ctx.cursor_position {
                    self.compiler.move_to(cx, cy, &mut tx.buffer);
                    self.last_cursor_x = cx;
                    self.last_cursor_y = cy;
                    let cursor_command = session.show_cursor();
                    if force_cursor {
                        tx.push(b"\x1b[?25h");
                    } else if let Some(cmd) = cursor_command {
                        tx.push(cmd);
                    }
                } else {
                    let cursor_command = session.hide_cursor();
                    if force_cursor {
                        tx.push(b"\x1b[?25l");
                    } else if let Some(cmd) = cursor_command {
                        tx.push(cmd);
                    }
                }
            }
            RenderMode::Inline => {
                if recover {
                    // The torn region is abandoned to scrollback: reset SGR, step to a fresh line
                    // below wherever the cursor was left, erase everything beneath it, and
                    // allocate a new region there (the full-repaint path below).
                    tx.push(b"\x1b[?7h\x1b[0m\r\n\x1b[J");
                    self.compiler.current_style = Style::default();
                }
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
                    if clears_inline {
                        // Establish blank cells only in the allocated live rows;
                        // leave unrelated rows below the inline region intact.
                        tx.push(b"\x1b[0m");
                        for row in 0..surface_height {
                            tx.push(b"\x1b[2K");
                            if row + 1 < surface_height {
                                tx.push(b"\r\n");
                            }
                        }
                        if surface_height > 1 {
                            write!(tx.buffer, "\x1b[{}A\r", surface_height - 1).ok();
                        } else {
                            tx.push(b"\r");
                        }
                        self.compiler.current_style = Style::default();
                    }
                } else {
                    // Rewind cursor from last position back to row 0 of live region
                    if self.last_cursor_y > 0 {
                        write!(tx.buffer, "\x1b[{}A\r", self.last_cursor_y).ok();
                    } else {
                        tx.push(b"\r");
                    }
                    self.compiler.reset_cursor(0, 0);
                    if restyle {
                        // Clear only the existing live rows, preserving unrelated
                        // content below them; then return to the region's top.
                        tx.push(b"\x1b[0m");
                        for row in 0..self.live_region_height {
                            tx.push(b"\x1b[2K");
                            if row + 1 < self.live_region_height {
                                tx.push(b"\r\n");
                            }
                        }
                        if self.live_region_height > 1 {
                            write!(tx.buffer, "\x1b[{}A\r", self.live_region_height - 1).ok();
                        } else {
                            tx.push(b"\r");
                        }
                        self.compiler.current_style = Style::default();
                        self.compiler.reset_cursor(0, 0);
                    }

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
                            tx.push(b"\r\n\x1b[0m\x1b[2K");
                        }
                        self.compiler.current_style = Style::default();
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
                    let cursor_command = session.show_cursor();
                    if force_cursor {
                        tx.push(b"\x1b[?25h");
                    } else if let Some(cmd) = cursor_command {
                        tx.push(cmd);
                    }
                } else {
                    let bottom_y = surface_height.saturating_sub(1);
                    self.compiler.move_to(0, bottom_y, &mut tx.buffer);
                    self.last_cursor_x = 0;
                    self.last_cursor_y = bottom_y;
                    let cursor_command = session.hide_cursor();
                    if force_cursor {
                        tx.push(b"\x1b[?25l");
                    } else if let Some(cmd) = cursor_command {
                        tx.push(cmd);
                    }
                }

                self.live_region_height = surface_height;
            }
        }

        // Exact wire bytes, including the synchronized-update terminator that
        // `commit_counted` appends. Do not sample `buffer.len()` before committing.
        // Split CPU-side generation from the blocking write/flush (#64).
        let generation_duration = frame_start.elapsed();
        let write_start = Instant::now();
        let bytes_emitted = if tx.buffered_len() > 0 {
            match tx.commit_counted() {
                Ok(n) => n,
                Err(f) if f.is_certain_noop() => {
                    session.reconcile_failed_cursor_write(cursor_hidden_before, false);
                    // Not one byte of this frame reached the writer: the screen is exactly what
                    // the renderer believes. Undo this frame's emission-state advance so a retry
                    // diffs in place (Gate F1's in-place restyle survives) — and if this frame was
                    // a re-anchor, the retry still owes its canvas reset (review finding 3).
                    self.owed_reanchor = reanchor;
                    (
                        self.compiler.cursor_x,
                        self.compiler.cursor_y,
                        self.compiler.current_style,
                        self.last_cursor_x,
                        self.last_cursor_y,
                        self.live_region_height,
                    ) = pre_frame;
                    return Err(f.error);
                }
                Err(f) => {
                    session.reconcile_failed_cursor_write(cursor_hidden_before, true);
                    // A prefix (or an unconfirmed flush) may have reached the terminal (Gate G3):
                    // never claim it was left unchanged — void the physical belief so the next
                    // frame re-establishes the screen.
                    self.poison_physical_state();
                    return Err(f.error);
                }
            }
        } else {
            0
        };
        let write_duration = write_start.elapsed();

        self.last_frame_report = FrameReport {
            exact_changed_cells: self.last_exact_changed,
            affected_cells: dirty_cells,
            total_cells,
            bytes_emitted,
            full_repaint: is_full_repaint,
            generation_duration,
            write_duration,
        };

        self.previous_surface = Some(next_surface);
        self.physical_uncertain = false;
        self.cursor_visibility_uncertain = false;
        self.bound_initial = false;
        self.owed_reanchor = false;
        // Physical-emission truth, advanced only now that the transaction committed: the cells on
        // screen carry this depth's SGR. A failed commit returns via `?` above and leaves the prior
        // value, so a retry re-detects the depth change and restyles (Gate F1).
        self.last_emitted_depth = Some(self.compiler.color_depth);
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
        self.begin_output();
        session.ensure_render_authorized()?;
        self.require_bound_target(session)?;
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
        if self.physical_uncertain {
            // A previous write tore (Gate G3): the live region was abandoned, so start the
            // committed lines on a fresh, erased line with a known SGR state.
            tx.push(b"\x1b[?7h\x1b[0m\r\n\x1b[J");
            self.compiler.current_style = Style::default();
        }

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

        let bytes = match tx.commit_counted() {
            Ok(n) => n,
            // Nothing reached the writer: the terminal is unchanged, the belief stays valid.
            Err(f) if f.is_certain_noop() => return Err(f.error),
            Err(f) => {
                // Possibly torn (Gate G3): void the physical belief before propagating.
                self.poison_physical_state();
                return Err(f.error);
            }
        };

        // Committing finalizes live state; the next live frame starts fresh.
        self.invalidate_anchor(false);
        // INLINE: the lines landed on a fresh line with a known SGR state, so the physical position
        // is known again (a torn region before them stays abandoned in scrollback). FULLSCREEN: the
        // canvas is still uncertain — the next frame must re-establish it (review finding 4).
        if self.mode == RenderMode::Inline {
            self.physical_uncertain = false;
        }
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
        self.begin_output();
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
        self.begin_output();
        self.commit_text(text, session, writer).map(|_| ())
    }

    /// Commits plain structured text to immutable scrollback, word-wrapped to the
    /// terminal width. For preformatted content use
    /// [`Renderer::commit_text_with_mode`].
    pub fn commit_text(
        &mut self,
        text: &str,
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<usize> {
        self.begin_output();
        self.commit_text_with_mode(text, WrapMode::WordWrap, session, writer)
    }

    /// Commits plain text to immutable scrollback with an explicit wrap policy
    /// (issue #45). `WrapMode::WordWrap` re-flows to the terminal width;
    /// `WrapMode::NoWrap` preserves the text **preformatted** — rendered at its
    /// natural column width (never clamped to the terminal), so aligned tables,
    /// diffs and ledgers keep their columns and no cells are silently dropped.
    /// Either way, control characters are neutralized at the cell-model boundary,
    /// so untrusted text cannot inject terminal control sequences.
    pub fn commit_text_with_mode(
        &mut self,
        text: &str,
        wrap: WrapMode,
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<usize> {
        self.begin_output();
        let (term_cols, _) = session.terminal_size();
        let render_width = wrap_render_width(text, wrap, term_cols);
        let mut node = Node::text_wrapped(text, Style::default(), wrap);
        node.layout_style.width = crate::node::Dimension::Length(render_width as f32);
        let lines = render_node_to_lines_with_depth(
            &mut node,
            session.is_tty,
            render_width,
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
        self.begin_output();
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
        self.begin_output();
        session.ensure_render_authorized()?;
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
        self.begin_output();
        self.insert_lines_before_live(lines, session, writer)
    }

    /// Inserts **safe** plain text into scrollback above the live region,
    /// word-wrapped to the terminal width. For preformatted content (aligned
    /// tables, diffs) use [`Renderer::insert_text_before_live_with_mode`].
    pub fn insert_text_before_live(
        &mut self,
        text: &str,
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<(usize, InsertStrategy)> {
        self.begin_output();
        self.insert_text_before_live_with_mode(text, WrapMode::WordWrap, session, writer)
    }

    /// Inserts **safe** plain text into scrollback above the live region with an
    /// explicit wrap policy (issue #45). `WrapMode::NoWrap` preserves the text
    /// preformatted at its natural column width (never clamped to the terminal),
    /// so aligned artifacts keep their columns and no cells are silently dropped;
    /// `WrapMode::WordWrap` re-flows to the terminal width. Control characters are
    /// neutralized at the cell-model boundary in both modes, so untrusted text
    /// cannot inject terminal controls.
    pub fn insert_text_before_live_with_mode(
        &mut self,
        text: &str,
        wrap: WrapMode,
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<(usize, InsertStrategy)> {
        self.begin_output();
        let (term_cols, _) = session.terminal_size();
        let render_width = wrap_render_width(text, wrap, term_cols);
        let mut node = Node::text_wrapped(text, Style::default(), wrap);
        node.layout_style.width = crate::node::Dimension::Length(render_width as f32);
        let lines = render_node_to_lines_with_depth(
            &mut node,
            session.is_tty,
            render_width,
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
        self.begin_output();
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
        session.ensure_render_authorized()?;
        self.require_bound_target(session)?;
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
        let (term_cols, term_rows) = session.terminal_size();
        let combined_height = m.saturating_add(self.live_region_height);

        // A preformatted (NoWrap, issue #45) line wider than the terminal
        // soft-wraps into extra physical rows that the one-row-per-line `CSI L`
        // fast path cannot account for, which would drift the live region below.
        // Force the always-correct repaint fallback for those. (WordWrap/CharWrap
        // content is bounded to the terminal width, so this never fires for them.)
        let any_line_exceeds_width = lines
            .iter()
            .any(|l| UnicodeWidthStr::width(strip_ansi_escapes(l).as_str()) > term_cols as usize);

        // The fast path relies on trustworthy relative cursor/region state and on
        // `CSI L` actually being supported. Unknown capability is NOT treated as
        // supported: we fall back to the always-correct repaint path.
        let anchor_stable = matches!(self.anchor, AnchorState::Stable { .. });
        let use_fast = self.live_region_height > 0
            && anchor_stable
            && session.insert_line_supported()
            && combined_height <= term_rows
            && !any_line_exceeds_width
            && self.previous_surface.is_some();

        let cursor_hidden_before = session.cursor_hidden_for_render();
        let mut tx = TerminalTransaction::new(writer, session.sync_updates());
        tx.begin();
        let strategy;
        if self.physical_uncertain {
            // A previous write tore (Gate G3): the fast path is already off (no trusted previous
            // surface); begin the fallback on a fresh, erased line with a known SGR state.
            tx.push(b"\x1b[?7h\x1b[0m\r\n\x1b[J");
            self.compiler.current_style = Style::default();
        }

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

                // Restore the requested cursor position and, if known, visibility.
                // Existing cursor uncertainty survives until a successful frame.
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

        let bytes = match tx.commit_counted() {
            Ok(n) => n,
            // Nothing reached the writer: the terminal is unchanged, the belief stays valid.
            Err(f) if f.is_certain_noop() => {
                session.reconcile_failed_cursor_write(cursor_hidden_before, false);
                return Err(f.error);
            }
            Err(f) => {
                session.reconcile_failed_cursor_write(cursor_hidden_before, true);
                // Possibly torn (Gate G3): void the physical belief before propagating.
                self.poison_physical_state();
                return Err(f.error);
            }
        };
        // A successful INLINE insertion re-establishes a known cursor position (Gate G3); a
        // fullscreen canvas stays uncertain until a frame re-establishes it (review finding 4).
        if self.mode == RenderMode::Inline {
            self.physical_uncertain = false;
        }

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
        self.begin_output();
        session.ensure_render_authorized()?;
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
        self.begin_output();
        session.ensure_render_authorized()?;
        self.require_bound_target(session)?;
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

        let bytes = match tx.commit_counted() {
            Ok(n) => n,
            // Nothing reached the writer: the terminal is unchanged, the belief stays valid.
            Err(f) if f.is_certain_noop() => return Err(f.error),
            Err(f) => {
                // Possibly torn (Gate G3): void the physical belief before propagating.
                self.poison_physical_state();
                return Err(f.error);
            }
        };

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

/// Render width for a wrap policy (issue #45): `WordWrap` wraps to the terminal
/// width; `NoWrap` uses the content's widest line so preformatted rows are never
/// clipped, and never narrower than the terminal.
fn wrap_render_width(text: &str, wrap: WrapMode, term_cols: u16) -> u16 {
    match wrap {
        // Wrapping modes fit within the terminal width.
        WrapMode::WordWrap | WrapMode::CharWrap => term_cols.max(1),
        // Preformatted: render at the content's widest line so nothing is
        // clipped, and never narrower than the terminal.
        WrapMode::NoWrap => text
            .split('\n')
            .map(|line| UnicodeWidthStr::width(line) as u16)
            .max()
            .unwrap_or(0)
            .max(term_cols)
            .max(1),
    }
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
