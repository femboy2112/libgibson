//! Polished Agent — a restrained coding-session console.
//!
//! Inline by default: the session header and the committed user request go into
//! **real terminal scrollback**, while a mutable live foreground shows a task
//! plan, a streaming answer, a scrollable code/diff viewport, event feed, a
//! permission modal and a persistent Unicode prompt. `--fullscreen` opts into the
//! alternate-screen dashboard instead.
//!
//! The narrative is a real session shape: plan → tool operations (with one
//! failure and recovery) → permission → code review → completion summary. It
//! never claims global numbers; all counts are demo-local.
//!
//! Modes: `--auto`/`--deterministic`, `--fullscreen`, `--stage=<name>` for fast
//! deterministic start states (`plan`, `permission`, `failure`, `prompt`,
//! `rejected`, `cancelled`), `--debug-scene` for the story inspector, and theme
//! proofs `--light` `--dark` `--no-color`.

use gibson::cell::{Color, Line, RichText, Span, Style, Theme};
use gibson::context::Context;
use gibson::focus::{FocusId, FocusRing};
use gibson::input::{Event, KeyCode, KeyModifiers, TextInputState};
use gibson::node::{Node, WrapMode};
use gibson::scene::{Easing, Effect, Presentation, Scene, SceneEntity, SceneTarget};
use gibson::show;
use gibson::story::{Beat, Condition, Story, StoryAction, StoryDirector, StoryEvent};
use gibson::{BorderType, ThemeStyles, TimeSource, ViewportState};
use std::env;
use std::time::Duration;

const FOCUS_PROMPT: FocusId = FocusId(1);
const FOCUS_CODE: FocusId = FocusId(2);

// ---------------------------------------------------------------------------
// Visual effects wrapper (respects --no-color)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Fx {
    color: bool,
    a: Color,
    b: Color,
    good: Color,
    bad: Color,
    warn: Color,
    dim: Color,
    st: ThemeStyles,
}

impl Fx {
    fn new(theme: Theme, color: bool) -> Self {
        let st = theme.styles();
        let c = |rgb: (u8, u8, u8)| {
            if color {
                Color::Rgb(rgb.0, rgb.1, rgb.2)
            } else {
                Color::Reset
            }
        };
        Self {
            color,
            a: c((64, 196, 255)),
            b: c((180, 120, 255)),
            good: c((80, 230, 150)),
            bad: c((255, 95, 120)),
            warn: c((255, 200, 90)),
            dim: c((96, 106, 126)),
            st,
        }
    }

    fn bar(&self, fraction: f32, width: usize) -> Line {
        if self.color {
            show::progress(fraction, width, self.a, self.b, self.dim)
        } else {
            let filled = ((fraction.clamp(0.0, 1.0)) * width as f32).round() as usize;
            Line::raw(format!(
                "{}{}",
                "█".repeat(filled.min(width)),
                "─".repeat(width.saturating_sub(filled))
            ))
        }
    }

    fn spark(&self, values: &[f32], width: usize) -> Line {
        if self.color {
            show::sparkline(values, width, self.a, self.good)
        } else {
            Line::raw("─".repeat(width))
        }
    }
}

// ---------------------------------------------------------------------------
// Agent state
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolState {
    Queued,
    Running,
    Done,
    Warn,
    Failed,
    /// Plan entry skipped because the session was aborted (permission rejected).
    Skipped,
}

struct Tool {
    verb: &'static str,
    target: &'static str,
    /// Short semantic result shown once the op finishes (READ ranges, match
    /// counts, diff stats, diagnostics).
    detail: &'static str,
    progress: f32,
    state: ToolState,
}

impl Tool {
    fn new(verb: &'static str, target: &'static str, detail: &'static str) -> Self {
        Self {
            verb,
            target,
            detail,
            progress: 0.0,
            state: ToolState::Queued,
        }
    }
}

/// The scripted session plan, including one failure and its recovery loop.
fn initial_plan() -> Vec<Tool> {
    vec![
        Tool::new("research", "src/renderer.rs", "read 214 lines"),
        Tool::new("inspect", "insert_before_live", "3 call sites"),
        Tool::new("reproduce", "clip_near regression", "edge pops confirmed"),
        Tool::new("patch", "src/geom.rs", "+31 −9"),
        Tool::new("test", "cargo test", "FAILED: wide-glyph clip leak"),
        Tool::new("diagnose", "surface.rs blit", "clip wins over glyph"),
        Tool::new("patch", "src/surface.rs", "+24 −6"),
        Tool::new("test", "clip + damage suites", "248 passing"),
        Tool::new("review", "invariant proof", "no edge popping"),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SessionOutcome {
    /// The mutation was approved and the post-approval plan ran.
    Approved,
    /// The user rejected the mutation; nothing was written.
    Rejected,
    /// The operator cancelled (Ctrl-C) before completion.
    Cancelled,
}

/// The story graph: preflight inspection → permission → mutation plan → prompt.
///
/// Permission happens **before** any mutation, and rejection/cancellation are
/// real terminal beats with truthful outcomes. Transitions that fire on tool
/// completion use semantic facts (`preflight-done`, `mutation-done`) rather than
/// hand-refreshed timers.
fn polished_story() -> Story {
    Story::new("inspect")
        .beat(
            Beat::new("inspect")
                .label("Inspect & reproduce")
                .transition(Condition::fact_true("preflight-done"), "permission")
                // Safety net so a stalled run cannot hang forever.
                .after(Duration::from_secs(30), "permission"),
        )
        .beat(
            Beat::new("permission")
                .label("Permission")
                .transition(Condition::on(StoryEvent::PermissionApproved), "patch")
                .transition(Condition::on(StoryEvent::PermissionRejected), "rejected")
                .transition(Condition::on(StoryEvent::Cancelled), "cancelled"),
        )
        .beat(
            Beat::new("patch")
                .label("Apply approved change")
                .on_enter(StoryAction::set_bool("approved", true))
                .transition(Condition::fact_true("mutation-done"), "prompt")
                .transition(Condition::on(StoryEvent::Cancelled), "cancelled")
                .after(Duration::from_secs(30), "prompt"),
        )
        .beat(
            Beat::new("prompt")
                .label("Prompt")
                .on_enter(StoryAction::set_bool("session-active", true))
                .transition(Condition::on(StoryEvent::custom("finish")), "done")
                .transition(Condition::on(StoryEvent::Cancelled), "cancelled"),
        )
        .beat(
            Beat::new("rejected")
                .label("Rejected")
                .on_enter(StoryAction::set_bool("rejected", true))
                .terminal(),
        )
        .beat(
            Beat::new("cancelled")
                .label("Cancelled")
                .on_enter(StoryAction::set_bool("cancelled", true))
                .terminal(),
        )
        .beat(Beat::new("done").label("Done").terminal())
}

const STREAM_TEXT: &str = "I audited the differential pipeline and the scrollback handoff. The live region now tracks an explicit physical anchor, so a resize forces a full re-anchor instead of trusting stale coordinates. Insert-before-live is proven to leave the live framebuffer untouched on the fast path, with a correct repaint fallback otherwise.";

const PERMISSION_OPTIONS: &[(&str, &str)] = &[
    ("Approve once", "apply for this step"),
    ("Approve for session", "trust this class of change"),
    ("Reject", "abort without touching disk"),
];

struct App {
    time: TimeSource,
    fx: Fx,
    tools: Vec<Tool>,
    tool_cursor: usize,
    /// Deterministic story beats + semantic facts (replaces ad-hoc `Phase`).
    director: StoryDirector,
    stream: String,
    stream_shown: usize,
    transcript: Vec<Line>,
    events: Vec<Line>,
    permission: usize,
    input: TextInputState,
    bg_sent: bool,
    frame_bytes: Vec<f32>,
    last_total_bytes: u64,
    render_frames: u64,
    /// (text, first-shown animation second)
    toast: Option<(String, f32)>,
    debug: bool,
    debug_info: String,
    done: bool,
    /// Keyboard focus (prompt vs code viewport). A modal captures focus.
    focus: FocusRing,
    /// Scrollable camera over the synthetic code/diff viewport.
    code_cam: ViewportState,
    summary_committed: bool,
    /// True while a modal owns all keyboard input.
    modal_captured: bool,
    /// Optional story inspector (`--debug-scene`).
    debug_scene: bool,
}

fn select_theme(light: bool, dark: bool, no_color: bool) -> Theme {
    if no_color {
        Theme::no_color()
    } else if light {
        Theme::light()
    } else if dark {
        Theme::dark()
    } else {
        Theme::default()
    }
}

impl App {
    fn new(fx: Fx, deterministic: bool, debug: bool, stage: Option<&str>) -> Self {
        let time = if deterministic {
            TimeSource::fixed(Duration::from_millis(16))
        } else {
            TimeSource::real()
        };
        let mut app = Self {
            time,
            fx,
            tools: initial_plan(),
            tool_cursor: 0,
            director: polished_story().start(),
            stream: STREAM_TEXT.to_string(),
            stream_shown: 0,
            transcript: Vec::new(),
            events: Vec::new(),
            permission: 0,
            input: TextInputState::new(),
            bg_sent: false,
            frame_bytes: Vec::new(),
            last_total_bytes: 0,
            render_frames: 0,
            toast: None,
            debug,
            debug_info: String::new(),
            done: false,
            focus: FocusRing::new([FOCUS_PROMPT, FOCUS_CODE]),
            code_cam: ViewportState::new(),
            summary_committed: false,
            modal_captured: false,
            debug_scene: false,
        };
        if let Some(stage) = stage {
            app.apply_stage(stage);
        }
        app
    }

    /// Deterministic start states for fast tests and goldens (`--stage=`).
    ///
    /// This sets coherent state (tool states + story facts), it does not skip
    /// rendering: the live foreground still paints and PTY interaction is real.
    fn apply_stage(&mut self, stage: &str) {
        match stage {
            "plan" => {}
            "permission" => {
                self.complete_tools(0, 3, None);
                self.director.facts_mut().set_bool("preflight-done", true);
                self.director.jump_to("permission");
                self.begin_permission();
            }
            "failure" => {
                // Post-approval plan paused at the failing test.
                self.complete_tools(0, 4, None);
                self.tools[4].state = ToolState::Failed;
                self.complete_tools(5, 5, None);
                self.director.facts_mut().set_bool("preflight-done", true);
                self.director.facts_mut().set_bool("approved", true);
                self.director.jump_to("patch");
                self.stream_shown = self.stream.chars().count();
            }
            "prompt" | "done" => {
                self.complete_tools(0, 9, Some(4));
                self.director.facts_mut().set_bool("preflight-done", true);
                self.director.facts_mut().set_bool("approved", true);
                self.director.facts_mut().set_bool("mutation-done", true);
                self.stream_shown = self.stream.chars().count();
                if stage == "done" {
                    self.director.jump_to("done");
                } else {
                    self.director.jump_to("prompt");
                }
            }
            "rejected" => {
                self.complete_tools(0, 3, None);
                self.director.facts_mut().set_bool("preflight-done", true);
                self.director.jump_to("permission");
                self.director
                    .update(Duration::ZERO, &[StoryEvent::PermissionRejected]);
            }
            "cancelled" => {
                self.director
                    .update(Duration::ZERO, &[StoryEvent::Cancelled]);
            }
            _ => {}
        }
    }

    /// Marks tools `[start, end)` complete, with an optional failing index.
    fn complete_tools(&mut self, start: usize, end: usize, fail_at: Option<usize>) {
        for i in start..end.min(self.tools.len()) {
            self.tools[i].progress = 1.0;
            self.tools[i].state = if Some(i) == fail_at {
                ToolState::Failed
            } else if i == 1 {
                ToolState::Warn
            } else {
                ToolState::Done
            };
        }
        self.tool_cursor = end.min(self.tools.len());
    }

    fn begin_permission(&mut self) {
        if !self.modal_captured {
            self.focus.capture();
            self.modal_captured = true;
        }
        self.permission = 0;
    }

    /// The current story beat id.
    fn beat(&self) -> &str {
        self.director.current_beat()
    }

    fn in_permission(&self) -> bool {
        self.beat() == "permission"
    }

    /// The truthful session outcome derived from story facts / beats.
    fn outcome(&self) -> SessionOutcome {
        if self.director.facts().bool("rejected") {
            SessionOutcome::Rejected
        } else if self.director.facts().bool("cancelled") {
            SessionOutcome::Cancelled
        } else {
            SessionOutcome::Approved
        }
    }

    fn is_done(&self) -> bool {
        self.director.is_finished()
    }

    fn elapsed(&self) -> f32 {
        self.time.now().as_secs_f32()
    }

    fn spinner(&self) -> usize {
        (self.elapsed() * 12.0) as usize
    }

    fn shimmer_offset(&self) -> usize {
        (self.elapsed() * 20.0) as usize
    }

    fn fps(&self) -> u64 {
        // Renderer frames, not application loop iterations.
        let e = self.elapsed().max(0.05);
        (self.render_frames as f32 / e).round() as u64
    }

    fn sample(&mut self, ctx: &mut Context) {
        // Count actual renderer frames, and record the wire bytes of the most
        // recent frame (not an application sampling interval).
        let stats = ctx.stats();
        if stats.frames > self.render_frames {
            let delta = stats.frame_bytes.saturating_sub(self.last_total_bytes) as f32;
            self.last_total_bytes = stats.frame_bytes;
            self.frame_bytes.push(delta);
            if self.frame_bytes.len() > 200 {
                self.frame_bytes.remove(0);
            }
        }
        self.render_frames = stats.frames;
        if self.debug {
            self.debug_info = format!(
                "FRAMES {} · FULL {} · RESYNC {} · INS {}",
                stats.frames, stats.full_repaints, stats.anchor_resyncs, stats.history_insertions
            );
        }
    }

    fn show_toast(&mut self, text: &str) {
        self.toast = Some((text.to_string(), self.elapsed()));
    }

    /// Advance the story-driven plan timeline.
    ///
    /// The tool runner is gated by the [`StoryDirector`]'s beat: preflight tools
    /// run in `inspect`, mutation tools run in `patch`. Completing a group sets a
    /// semantic fact; the director follows the matching transition. There is no
    /// phase enum and no `*_frames` counter synchronising the visuals.
    fn animate(&mut self) {
        let beat = self.beat().to_string();
        let (start, end) = match beat.as_str() {
            "inspect" => (0usize, 3usize),
            "patch" => (3usize, self.tools.len()),
            _ => (0, 0),
        };
        if end > start {
            self.advance_tool(start, end);
            self.stream_shown = (self.stream_shown + 9).min(self.stream.chars().count());
            if self.tool_cursor == 2 && !self.bg_sent {
                self.bg_sent = true;
                self.add_event("diagnostics refreshed · 0 warnings".into(), self.fx.good);
                self.show_toast("diagnostics refreshed · 0 warnings");
            }
        }

        // Publish completion facts; the director owns the transition.
        if beat == "inspect" && self.tool_cursor >= 3 {
            self.add_event("patch conflict: needs review".into(), Color::Reset);
            self.director.facts_mut().set_bool("preflight-done", true);
        } else if beat == "patch" && self.tool_cursor >= self.tools.len() {
            self.director.facts_mut().set_bool("mutation-done", true);
        }
    }

    /// Advances exactly one tool in `[start, end)` by one progress step.
    fn advance_tool(&mut self, start: usize, end: usize) {
        if self.tool_cursor < start || self.tool_cursor >= end {
            return;
        }
        let idx = self.tool_cursor;
        if let Some(tool) = self.tools.get_mut(idx) {
            if tool.state == ToolState::Queued {
                tool.state = ToolState::Running;
            }
            tool.progress = (tool.progress + 0.06).min(1.0);
            if tool.progress >= 1.0 {
                // Tool 1 (`inspect`) warns; tool 4 (`test`) genuinely fails;
                // everything else passes. A real state machine, not a happy path.
                tool.state = match idx {
                    1 => ToolState::Warn,
                    4 => ToolState::Failed,
                    _ => ToolState::Done,
                };
                let line = tool_line(tool, &self.fx);
                let (verb, detail, state) = (tool.verb, tool.detail, tool.state);
                self.transcript.push(line);
                self.on_tool_complete(verb, detail, state);
                self.tool_cursor += 1;
            }
        }
    }

    /// Emits narrative reactions for a completed tool operation.
    fn on_tool_complete(&mut self, verb: &str, detail: &str, state: ToolState) {
        if state == ToolState::Warn {
            self.add_event(format!("{verb}: {detail} · needs attention"), self.fx.warn);
            return;
        }
        match (verb, state) {
            ("test", ToolState::Failed) => {
                self.add_event(format!("test failed: {detail}"), self.fx.bad);
                self.show_toast("test failed · wide-glyph clip leak");
                self.reveal_code_at(0);
            }
            ("diagnose", _) => {
                self.add_event(
                    "root cause: clipping must win over glyph shape".into(),
                    self.fx.good,
                );
            }
            ("test", ToolState::Done) => {
                self.add_event("re-run green · clip + logical damage".into(), self.fx.good);
                self.show_toast("tests green · invariant proven");
            }
            ("patch", _) => {
                self.add_event(format!("patch applied · {detail}"), self.fx.good);
            }
            _ => {}
        }
    }

    /// Scrolls the code viewport to the top so a highlight is visible.
    fn reveal_code_at(&mut self, y: i32) {
        self.code_cam.offset_y = y.max(0);
    }

    fn auto_step(&mut self) {
        match self.beat() {
            "inspect" | "patch" => self.animate(),
            "permission" => {
                // Scripted runs choose the first (Approve once) option.
                self.permission = 0;
                self.resolve_permission();
            }
            "prompt" => {
                // Hostile Unicode end-to-end: ASCII, emoji, CJK, combining mark,
                // ZWJ sequence and a regional-indicator flag.
                for ch in
                    "review the fast-path invariant 🦀 你好世界 e\u{0301} 🇺🇸 👩\u{200D}💻".chars()
                {
                    self.input.insert_char(ch);
                }
                self.bg_sent = true;
                self.add_event("index finished while you were typing".into(), self.fx.good);
                self.finish();
            }
            _ => {
                self.finish();
            }
        }
    }

    fn add_event(&mut self, text: String, color: Color) {
        let stamp = format!("{:04}", self.render_frames);
        let style = if self.fx.color {
            Style::new().fg(color)
        } else {
            self.fx.st.muted
        };
        self.events.push(
            Line::new()
                .span(Span::styled(format!("{stamp} "), self.fx.st.faint))
                .span(Span::styled(text, style)),
        );
        if self.events.len() > 40 {
            self.events.remove(0);
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if is_cancel(event) {
            self.cancel_session();
            return true;
        }
        match event {
            Event::Key(k) => {
                // A modal captures ALL relevant keyboard input.
                if self.in_permission() {
                    return self.handle_permission_key(k);
                }
                if self.beat() == "prompt" {
                    return self.handle_prompt_key(event, k);
                }
                // After a terminal beat (rejected/cancelled/done) the session is
                // over: Enter/Esc acknowledges and commits the truthful summary.
                if self.director.is_finished() && matches!(k.code, KeyCode::Enter | KeyCode::Esc) {
                    self.finish();
                    return true;
                }
                false
            }
            Event::Paste(_) => self.beat() == "prompt" && self.input.handle_event(event),
            _ => false,
        }
    }

    /// Modal input policy: every relevant key is consumed here; nothing leaks
    /// through to the dashboard or prompt beneath.
    fn handle_permission_key(&mut self, k: &gibson::input::KeyEvent) -> bool {
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.permission = if self.permission == 0 {
                    2
                } else {
                    self.permission - 1
                };
                false
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.permission = (self.permission + 1) % 3;
                false
            }
            KeyCode::Char(c @ '1'..='3') => {
                self.permission = (c as usize) - ('1' as usize);
                false
            }
            KeyCode::Char('a') => {
                self.permission = 1;
                self.resolve_permission();
                true
            }
            KeyCode::Enter => {
                self.resolve_permission();
                true
            }
            KeyCode::Esc => {
                self.permission = 2;
                self.resolve_permission();
                true
            }
            _ => false,
        }
    }

    /// Focus-aware input routing.
    ///
    /// `FOCUS_PROMPT`: characters and text-editing keys go to the `TextInput`.
    /// `FOCUS_CODE`: vertical navigation belongs to the code viewport and
    /// characters are ignored (they are never silently typed into the prompt).
    fn handle_prompt_key(&mut self, event: &Event, k: &gibson::input::KeyEvent) -> bool {
        match k.code {
            KeyCode::Tab => {
                self.focus.focus_next();
                return true;
            }
            KeyCode::BackTab => {
                self.focus.focus_prev();
                return true;
            }
            _ => {}
        }
        if self.focus.current() == Some(FOCUS_CODE) {
            match k.code {
                KeyCode::Up => {
                    self.code_cam.scroll_by(0, -1);
                    true
                }
                KeyCode::Down => {
                    self.code_cam.scroll_by(0, 1);
                    true
                }
                KeyCode::PageUp => {
                    self.code_cam.page(CODE_VIEW_H, -1);
                    true
                }
                KeyCode::PageDown => {
                    self.code_cam.page(CODE_VIEW_H, 1);
                    true
                }
                KeyCode::Home => {
                    self.code_cam.home();
                    true
                }
                KeyCode::End => {
                    self.code_cam.end(CODE_VIEW.len() as u16, CODE_VIEW_H);
                    true
                }
                KeyCode::Esc => {
                    self.finish();
                    true
                }
                KeyCode::Enter => {
                    self.submit_prompt();
                    true
                }
                _ => false,
            }
        } else {
            match k.code {
                KeyCode::Esc => {
                    self.finish();
                    true
                }
                KeyCode::Enter => {
                    self.submit_prompt();
                    true
                }
                _ => self.input.handle_event(event),
            }
        }
    }

    fn submit_prompt(&mut self) {
        let text = self.input.text.clone();
        self.transcript.push(
            Line::new()
                .span(Span::styled("❯ ", self.fx.st.accent))
                .span(Span::styled(text, self.fx.st.text)),
        );
        self.input = TextInputState::new();
    }

    /// Applies the modal decision as a semantic story event and narrates the
    /// truthful result. Approval moves to the mutation beat; rejection marks the
    /// remaining plan entries skipped — permission is never post-hoc.
    fn resolve_permission(&mut self) {
        let approved = self.permission != 2;
        let event = if approved {
            StoryEvent::PermissionApproved
        } else {
            StoryEvent::PermissionRejected
        };
        self.director.update(Duration::ZERO, &[event]);

        if self.modal_captured {
            self.focus.release();
            self.modal_captured = false;
        }

        if approved {
            self.transcript.push(
                Line::new()
                    .span(Span::styled("✔ ", self.fx.st.success))
                    .span(Span::styled(
                        "approved  patch src/geom.rs  (+31 −9)",
                        self.fx.st.text,
                    )),
            );
            let diff: &[(&str, Option<&str>, Option<&str>)] = &[
                (
                    "351",
                    Some("if z < self.near { return None; }"),
                    Some("if !self.is_valid() || z < self.near { return None; }"),
                ),
                (
                    "393",
                    None,
                    Some("let z_limit = limit - limit * CLIP_INSET_ULPS * f32::EPSILON;"),
                ),
            ];
            for (lineno, del, add) in diff {
                if let Some(d) = del {
                    self.transcript.push(
                        Line::new()
                            .span(Span::styled(format!("{lineno:>4} │ "), self.fx.st.muted))
                            .span(Span::styled("− ", self.fx.st.error))
                            .span(Span::styled(*d, self.fx.st.error)),
                    );
                }
                if let Some(a) = add {
                    self.transcript.push(
                        Line::new()
                            .span(Span::styled(format!("{lineno:>4} │ "), self.fx.st.muted))
                            .span(Span::styled("+ ", self.fx.st.success))
                            .span(Span::styled(*a, self.fx.st.success)),
                    );
                }
            }
        } else {
            for t in self.tools.iter_mut() {
                if t.state == ToolState::Queued || t.state == ToolState::Running {
                    t.state = ToolState::Skipped;
                }
            }
            self.transcript.push(
                Line::new()
                    .span(Span::styled("✖ ", self.fx.st.error))
                    .span(Span::styled("patch rejected", self.fx.st.text)),
            );
        }
    }

    /// Ctrl-C: a real cancellation path with a truthful outcome.
    fn cancel_session(&mut self) {
        self.add_event("cancelled by operator (Ctrl-C)".into(), self.fx.bad);
        if !self.director.is_finished() {
            self.director
                .update(Duration::ZERO, &[StoryEvent::Cancelled]);
        }
        self.finish();
    }

    /// Commits the completion summary to the transcript, derived from the actual
    /// [`SessionOutcome`]. Demo-local numbers only — no repository-wide telemetry.
    fn finish(&mut self) {
        if !self.director.is_finished() {
            self.director
                .update(Duration::ZERO, &[StoryEvent::custom("finish")]);
        }
        if self.summary_committed {
            self.done = true;
            return;
        }
        self.summary_committed = true;
        let fx = self.fx;
        self.transcript.push(Line::new());
        let (glyph, glyph_style, title, lines): (&str, Style, &str, &[&str]) = match self.outcome()
        {
            SessionOutcome::Approved => (
                "✔ ",
                fx.st.success,
                "session complete",
                &[
                    "3 files changed  (geom.rs · surface.rs · canvas.rs)",
                    "17 targeted checks added  ·  clip regression suite green",
                    "no edge popping, no wide-glyph clip leak, damage honest",
                ],
            ),
            SessionOutcome::Rejected => (
                "✖ ",
                fx.st.error,
                "session stopped — patch rejected",
                &[
                    "no files changed · working tree untouched",
                    "plan paused after 3 of 9 steps (before any mutation)",
                    "nothing was written to disk",
                ],
            ),
            SessionOutcome::Cancelled => (
                "⊘ ",
                fx.st.warning,
                "session cancelled",
                &[
                    "no files changed · working tree untouched",
                    "operator aborted before completion",
                ],
            ),
        };
        self.transcript.push(
            Line::new()
                .span(Span::styled(glyph, glyph_style))
                .span(Span::styled(title, fx.st.text)),
        );
        for line in lines {
            self.transcript.push(
                Line::new()
                    .span(Span::styled("  · ", fx.st.faint))
                    .span(Span::styled(*line, fx.st.muted)),
            );
        }
        self.done = true;
    }
}

fn is_cancel(event: &Event) -> bool {
    matches!(event, Event::Key(k) if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL))
}

/// Distinct visual grammar per tool event type.
fn verb_marker(verb: &str) -> &'static str {
    match verb {
        "research" | "read" => "≡",
        "inspect" | "search" => "⌕",
        "reproduce" => "◈",
        "patch" => "±",
        "test" => "⚗",
        "diagnose" => "✦",
        "review" => "§",
        "build" => "⚙",
        "shell" => "$",
        _ => "·",
    }
}

fn tool_line(tool: &Tool, fx: &Fx) -> Line {
    let (marker, style) = match tool.state {
        ToolState::Queued => ("·", fx.st.faint),
        ToolState::Running => ("◐", fx.st.accent),
        ToolState::Done => ("✔", fx.st.success),
        ToolState::Warn => ("▲", fx.st.warning),
        ToolState::Failed => ("✖", fx.st.error),
        ToolState::Skipped => ("⊘", fx.st.faint),
    };
    let mut line = Line::new()
        .span(Span::styled(format!("{marker} "), style))
        .span(Span::styled(
            format!("{} ", verb_marker(tool.verb)),
            fx.st.faint,
        ))
        .span(Span::styled(format!("{:<9}", tool.verb), fx.st.code))
        .span(Span::styled(tool.target, fx.st.text));
    if tool.state != ToolState::Queued && !tool.detail.is_empty() {
        let detail_style = match tool.state {
            ToolState::Failed => fx.st.error,
            ToolState::Warn => fx.st.warning,
            _ => fx.st.muted,
        };
        line = line.span(Span::styled(format!("  · {}", tool.detail), detail_style));
    }
    line
}

/// Synthetic code/diff content for the scrollable viewport.
/// `kind`: 0 context, 1 added, -1 removed, 2 highlighted.
const CODE_VIEW: &[(u16, &str, i8)] = &[
    (
        210,
        "pub fn clip_near(&self, a: Vec3, b: Vec3) -> Option<(Vec3, Vec3)> {",
        0,
    ),
    (211, "    let a_in = self.is_visible(a);", 1),
    (212, "    let b_in = self.is_visible(b);", 1),
    (213, "    match (a_in, b_in) {", 0),
    (214, "        (true, true) => Some((a, b)),", 0),
    (215, "        (false, false) => None,", 0),
    (216, "        _ => {", 0),
    (217, "            let limit = self.camera_z - self.near;", 1),
    (
        218,
        "            let t = ((limit - a.z) / denom).clamp(0.0, 1.0);",
        1,
    ),
    (219, "            let mid = Vec3::new(.., z_limit);", 1),
    (
        220,
        "            if a_in { Some((a, mid)) } else { Some((mid, b)) }",
        0,
    ),
    (221, "        }", 0),
    (222, "    }", 0),
    (223, "}", 0),
    (224, "", 0),
    (225, "pub fn blit_transparent_clipped(..) {", 0),
    (226, "    if c.is_continuation { continue; }", 1),
    (227, "    if c.glyph.display_width == 2 {", 2),
    (228, "        let fits = clip.contains(tx + 1, ty);", 1),
    (
        229,
        "        if fits { self.set_cell(tx, ty, c.clone()); }",
        1,
    ),
    (230, "    }", 2),
    (231, "}", 0),
    (300, "// logical damage: runs ∪ erase-eol ∪ cleared rows", 2),
    (301, "pub fn logical_dirty_count(&self) -> usize {", 0),
    (302, "    self.explicit_dirty_count()", 0),
    (303, "        + erase_region + cleared_rows", 1),
    (304, "}", 0),
];

fn truncate(s: &str, w: usize) -> String {
    use unicode_segmentation::UnicodeSegmentation as _;
    if unicode_width::UnicodeWidthStr::width(s) <= w {
        return s.to_string();
    }
    let mut out = String::new();
    let mut used = 0usize;
    for g in s.graphemes(true) {
        let gw = unicode_width::UnicodeWidthStr::width(g);
        if used + gw > w.saturating_sub(1) {
            break;
        }
        out.push_str(g);
        used += gw;
    }
    out.push('…');
    out
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

/// A panel with tight vertical padding (title border + one content row minimum).
fn tpanel(title: &str, style: Style) -> Node {
    let mut p = Node::panel(title.to_string(), BorderType::Rounded, style);
    // One row of top padding keeps children clear of the title row; no bottom
    // padding so short panels stay compact.
    p.layout_style.padding_top = 1.0;
    p.layout_style.padding_bottom = 0.0;
    p
}

fn build_root(app: &App, ctx: &Context) -> Node {
    let (cols, rows) = ctx.session.terminal_size();
    let compact = cols < 66;
    let wide = cols >= 110;

    let mut root = Node::col()
        .percent_width(100.0)
        .percent_height(100.0)
        .gap(0.0)
        .child(banner(app, wide));

    if compact {
        root = root.child(panel_plan(app, cols).flex_grow(2.0).min_width(0.0));
        root = root.child(panel_stream(app).flex_grow(2.0).min_width(0.0));
        if rows >= 20 {
            root = root.child(panel_telemetry(app, cols).flex_grow(1.0).min_width(0.0));
        }
        if rows >= 26 {
            root = root.child(panel_code(app, cols));
        }
    } else {
        let right_w: u16 = if wide { 40 } else { 32 };
        let left_w = cols.saturating_sub(right_w + 1);

        let left = Node::col()
            .flex_grow(1.0)
            .flex_shrink(1.0)
            .min_width(0.0)
            .gap(0.0)
            .child(panel_plan(app, left_w).flex_grow(1.0).min_width(0.0))
            .child(panel_stream(app).flex_grow(2.0).min_width(0.0));

        let right = Node::col()
            .width(right_w as f32)
            .flex_shrink(0.0)
            .min_width(0.0)
            .gap(0.0)
            .child(panel_transcript(app, right_w).flex_grow(1.0).min_width(0.0))
            .child(panel_code(app, right_w))
            .child(panel_events(app, right_w).flex_grow(1.0).min_width(0.0));

        root = root.child(
            Node::row()
                .percent_width(100.0)
                .gap(1.0)
                .flex_grow(1.0)
                .min_height(0.0)
                .child(left)
                .child(right),
        );
    }

    root = root.child(panel_footer(app, cols));

    // Floating overlays. The base dashboard is never rebuilt or reflowed; the
    // overlay is composited on top through the Stack layer. Overlay motion
    // (reveal, slide) comes from the Scene Algebra `Effect`s, not hand-computed
    // padding — the panel itself knows nothing about animation.
    if let Some(overlay) = build_overlay(app, cols, rows) {
        return Node::stack()
            .percent_width(100.0)
            .percent_height(100.0)
            .child(root)
            .child(overlay);
    }
    root
}

/// Builds the scene overlay through the render functor.
///
/// Entities wrap ordinary nodes; [`Effect`]s drive position and visibility. The
/// result is a normal `Node` tree composited over the dashboard.
fn build_overlay(app: &App, cols: u16, rows: u16) -> Option<Node> {
    let mut scene = Scene::new();
    let (w, h) = (cols as f32, rows as f32);
    let mut p = Presentation::new();
    let mut any = false;

    if app.in_permission() {
        let dim = scene.add(
            SceneEntity::new(
                "dim",
                Node::dim().percent_width(100.0).percent_height(100.0),
            )
            .z(0)
            .hidden(),
        );
        let modal = scene.add(
            SceneEntity::new("modal", permission_modal_node(app))
                .z(1)
                .hidden(),
        );
        let (mw, mh) = (46.0f32, 8.0f32);
        let cx = ((w - mw) * 0.5).max(0.0).round();
        let cy = ((h - mh) * 0.5).max(0.0).round();
        let effect = Effect::parallel([
            Effect::reveal(SceneTarget::Id(dim), 0.0, 1.0, Duration::from_millis(140)),
            Effect::reveal(SceneTarget::Id(modal), 0.0, 1.0, Duration::from_millis(180)),
            Effect::translate(
                SceneTarget::Id(modal),
                (cx, cy - 3.0),
                (cx, cy),
                Duration::from_millis(220),
            )
            .eased(Easing::EaseOut),
        ]);
        effect.eval(app.director.time_in_beat(), &scene, &mut p);
        any = true;
    }

    if let Some((text, shown_at)) = &app.toast {
        let age = app.elapsed() - *shown_at;
        // Decorative toasts are suppressed on ultra-narrow terminals where they
        // would cover essential panel titles.
        if age < 2.4 && cols >= 66 {
            let panel_w = ((text.len() as u16) + 10).clamp(24, 46) as f32;
            let toast = scene.add(
                SceneEntity::new("toast", toast_panel_node(app, text, panel_w))
                    .z(2)
                    .hidden(),
            );
            let (ax, ay) = ((w - panel_w - 2.0).max(0.0), 2.0f32);
            let enter = Effect::parallel([
                Effect::reveal(SceneTarget::Id(toast), 0.0, 1.0, Duration::from_millis(160)),
                Effect::translate(
                    SceneTarget::Id(toast),
                    (ax, ay - 2.0),
                    (ax, ay),
                    Duration::from_millis(200),
                )
                .eased(Easing::EaseOut),
            ]);
            let exit = Effect::parallel([
                Effect::reveal(SceneTarget::Id(toast), 1.0, 0.0, Duration::from_millis(400)),
                Effect::translate(
                    SceneTarget::Id(toast),
                    (ax, ay),
                    (ax, ay - 1.0),
                    Duration::from_millis(400),
                ),
            ]);
            let effect = Effect::sequence([
                enter,
                Effect::Delay(Duration::from_millis(1600), Box::new(Effect::identity())),
                exit,
            ]);
            effect.eval(Duration::from_secs_f32(age.max(0.0)), &scene, &mut p);
            any = true;
        }
    }

    if !any {
        return None;
    }
    Some(scene.to_node(&p, w, h))
}

/// The opaque permission modal (the dim veil is a separate scene entity).
fn permission_modal_node(app: &App) -> Node {
    let fx = &app.fx;
    let mut body = RichText::new().line(
        Line::new()
            .span(Span::styled("Allow ", fx.st.muted))
            .span(Span::styled("patch src/geom.rs", fx.st.text))
            .span(Span::styled("?", fx.st.muted)),
    );
    for (i, (label, detail)) in PERMISSION_OPTIONS.iter().enumerate() {
        let sel = i == app.permission;
        let mut line = Line::new()
            .span(Span::styled(
                if sel { "❯ " } else { "  " },
                if sel { fx.st.accent } else { fx.st.faint },
            ))
            .span(Span::styled(format!("[{}] ", i + 1), fx.st.muted))
            .span(Span::styled(
                *label,
                if sel { Style::new().bold() } else { fx.st.text },
            ));
        if detail.len() < 40 {
            line = line.span(Span::styled(format!("  — {detail}"), fx.st.muted));
        }
        body = body.line(line);
    }
    modal_with_shadow(fx, body)
}

/// A floating modal with a block-glyph drop shadow. The shadow is a raster layer
/// offset by one cell so it only peeks along the right/bottom edges; the opaque
/// panel hides the rest. No reflow, no fake alpha.
fn modal_with_shadow(fx: &Fx, body: RichText) -> Node {
    let mut shadow = gibson::surface::Surface::new_transparent(44, 7);
    shadow.fill_rect(
        gibson::surface::Rect::new(0, 0, 44, 7),
        gibson::cell::Cell::new(gibson::cell::Glyph::new("░"), Style::new().dim()),
    );
    Node::stack()
        .width(46.0)
        .height(8.0)
        .child(
            Node::raster(shadow)
                .width(44.0)
                .height(7.0)
                .offset(1.0, 1.0),
        )
        .child(
            Node::panel("PERMISSION", BorderType::Rounded, fx.st.warning)
                .width(44.0)
                .height(7.0)
                .background(Color::Reset)
                .offset(0.0, 0.0)
                .child(Node::rich_text_wrapped(body, WrapMode::NoWrap)),
        )
}

/// The toast panel itself. Its slide/fade is a Scene [`Effect`]; this builder is
/// purely declarative and has no idea it is animated.
fn toast_panel_node(app: &App, text: &str, width: f32) -> Node {
    let fx = &app.fx;
    let body = Line::new()
        .span(Span::styled("◆ ", fx.st.accent))
        .span(Span::styled(text, fx.st.success));
    Node::panel("TOAST", BorderType::Rounded, fx.st.border)
        .width(width)
        .height(3.0)
        .background(Color::Reset)
        .child(Node::line(body))
}

fn banner(app: &App, wide: bool) -> Node {
    let fx = &app.fx;
    let wordmark = if wide {
        "◢ LIBGIBSON AGENT"
    } else {
        "◢ LIBGIBSON"
    };
    let mut line = shimmer_line(wordmark, fx, app.shimmer_offset());
    line = line.span(fx.badge_span("● ONLINE", fx.st.success));
    if wide {
        line = line
            .span(Span::styled("  session ", fx.st.muted))
            .span(Span::styled("hardening", fx.st.text))
            .span(Span::styled("  ·  model ", fx.st.muted))
            .span(Span::styled("gibson-v2", fx.st.code))
            .span(Span::styled("  ·  ", fx.st.muted))
            .span(Span::styled("differential", fx.st.accent));
    } else {
        line = line.span(Span::styled("  ◐ v2", fx.st.muted));
    }
    Node::line(line).height(1.0).flex_shrink(0.0)
}

impl Fx {
    fn badge_span(&self, text: &str, role: Style) -> Span {
        Span::styled(format!("  {text}  "), role)
    }
}

fn shimmer_line(text: &str, fx: &Fx, offset: usize) -> Line {
    if !fx.color {
        return Line::styled(text, Style::new().bold());
    }
    use unicode_segmentation::UnicodeSegmentation as _;
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    let n = graphemes.len().max(1);
    let mut spans = Vec::with_capacity(n);
    for (i, g) in graphemes.iter().enumerate() {
        let t = i as f32 / (n - 1).max(1) as f32;
        let base = fx.a.lerp(fx.b, t);
        let d = ((i + offset) % 24) as f32;
        let boost = (1.0 - (d.min(24.0 - d) / 12.0)).clamp(0.0, 1.0) * 0.5;
        let c = base.lerp(Color::Rgb(255, 255, 255), boost);
        spans.push(Span::styled(*g, Style::new().fg(c).bold()));
    }
    Line::from_spans(spans)
}

fn panel_plan(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(4) as usize;
    let bar_w = inner.saturating_sub(14).clamp(6, 30);

    let mut rt = RichText::new();
    for tool in &app.tools {
        let (marker, mstyle) = match tool.state {
            ToolState::Queued => ("·", fx.st.faint),
            ToolState::Running => ("◐", fx.st.accent),
            ToolState::Done => ("✔", fx.st.success),
            ToolState::Warn => ("▲", fx.st.warning),
            ToolState::Failed => ("✖", fx.st.error),
            ToolState::Skipped => ("⊘", fx.st.faint),
        };
        let progress = if matches!(tool.state, ToolState::Queued | ToolState::Skipped) {
            0.0
        } else {
            tool.progress
        };
        let mut line = Line::new()
            .span(Span::styled(format!("{marker} "), mstyle))
            .span(Span::styled(format!("{:<9}", tool.verb), fx.st.code));
        for s in fx.bar(progress, bar_w).spans {
            line = line.span(s);
        }
        line = line.span(Span::styled(
            format!(" {:>3}%", (progress * 100.0).round() as usize),
            fx.st.muted,
        ));
        rt = rt.line(line);
    }
    tpanel("TASK PLAN", fx.st.border)
        .percent_width(100.0)
        .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
}

fn panel_stream(app: &App) -> Node {
    let fx = &app.fx;
    let shown: String = app.stream.chars().take(app.stream_shown).collect();
    let caret = if ((app.elapsed() * 2.0) as usize).is_multiple_of(2) {
        "▌"
    } else {
        " "
    };
    let rt = RichText::new().line(
        Line::new()
            .span(Span::styled(shown, fx.st.text))
            .span(Span::styled(caret, fx.st.accent)),
    );
    tpanel("STREAM", fx.st.border)
        .percent_width(100.0)
        .child(Node::rich_text_wrapped(rt, WrapMode::WordWrap))
}

/// Height heuristic used to clamp the code viewport camera. The viewport itself
/// clips authoritatively; this only keeps Home/End/PgUp/PgDn honest.
const CODE_VIEW_H: u16 = 8;

fn panel_code(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(6);
    let mut rt = RichText::new();
    for (num, text, kind) in CODE_VIEW {
        let (sign, prefix_style, text_style) = match kind {
            -1 => ("−", fx.st.error, fx.st.error),
            1 => ("+", fx.st.success, fx.st.success),
            2 => ("◆", fx.st.warning, Style::new().bold()),
            _ => (" ", fx.st.muted, fx.st.text),
        };
        rt = rt.line(
            Line::new()
                .span(Span::styled(format!("{num:>4} {sign} "), prefix_style))
                .span(Span::styled(*text, text_style)),
        );
    }
    let content_h = CODE_VIEW.len() as u16;
    let mut cam = app.code_cam;
    cam.clamp(inner, content_h.max(CODE_VIEW_H), inner, CODE_VIEW_H);
    let focused = app.focus.current() == Some(FOCUS_CODE);
    tpanel(
        if focused {
            "CODE VIEW ●"
        } else {
            "CODE VIEW"
        },
        if focused { fx.st.accent } else { fx.st.border },
    )
    .percent_width(100.0)
    // Fixed height: the camera clips a taller buffer, so the panel must not be
    // sized by the buffer's intrinsic height.
    .height((CODE_VIEW_H + 2) as f32)
    .flex_shrink(0.0)
    .min_width(0.0)
    .child(
        // A fixed-height camera keeps the panel's layout basis small so the
        // column's flex distribution stays honest, while the viewport clips and
        // scrolls the (much taller) code buffer.
        Node::viewport(cam.offset_x, cam.offset_y)
            .percent_width(100.0)
            .height(CODE_VIEW_H as f32)
            .max_height(CODE_VIEW_H as f32)
            .min_height(0.0)
            .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap).width(inner as f32)),
    )
}

fn panel_transcript(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(4) as usize;
    let mut rt = RichText::new();
    rt = rt.line(
        Line::new()
            .span(Span::styled("❯ ", fx.st.accent))
            .span(Span::styled(
                truncate(
                    "Audit the renderer, fix clipping, and prove it.",
                    inner.saturating_sub(2),
                ),
                fx.st.text,
            )),
    );
    for line in &app.transcript {
        rt = rt.line(line.clone());
    }
    // A clipped camera keeps the transcript pinned to the newest lines without
    // reflowing the panel; long histories scroll through the viewport.
    let content_h = rt.lines.len() as u16;
    let view_h = 6u16;
    let mut cam = gibson::ViewportState::new();
    cam.offset_y = content_h.saturating_sub(view_h) as i32;
    cam.clamp(inner as u16, content_h.max(view_h), inner as u16, view_h);
    tpanel("TRANSCRIPT", fx.st.border)
        .percent_width(100.0)
        .child(
            Node::viewport(cam.offset_x, cam.offset_y)
                .percent_width(100.0)
                .percent_height(100.0)
                .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap).width(inner as f32)),
        )
}

fn panel_telemetry(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let spark_w = (width.saturating_sub(4) as usize).clamp(8, 46);
    let latest = app.frame_bytes.last().copied().unwrap_or(0.0) as u64;
    let mut rt = RichText::new()
        .line(
            Line::new()
                .span(Span::styled("frame ", fx.st.muted))
                .span(Span::styled(format!("{latest:>5} B"), fx.st.text))
                .span(Span::styled("   frames ", fx.st.muted))
                .span(Span::styled(
                    format!("{:<5}", app.render_frames),
                    fx.st.text,
                )),
        )
        .line(fx.spark(&app.frame_bytes, spark_w));

    // A braille oscilloscope of real per-frame bytes (1 color per cell, 2x4
    // dots per cell) demonstrates the sub-cell canvas.
    let max = app.frame_bytes.iter().cloned().fold(1.0_f32, f32::max);
    let vals: Vec<f32> = if app.frame_bytes.is_empty() {
        vec![0.0; 8]
    } else {
        app.frame_bytes
            .iter()
            .map(|v| (v / max) * 2.0 - 1.0)
            .collect()
    };
    let scope = gibson::braille_oscilloscope(&vals, spark_w as u16, 2);
    for l in scope.to_lines() {
        rt = rt.line(Line::styled(l, fx.st.accent));
    }

    rt = rt.line(
        Line::new()
            .span(Span::styled("fps ", fx.st.muted))
            .span(Span::styled(format!("{:<4}", app.fps()), fx.st.text))
            .span(Span::styled("spinner ", fx.st.muted))
            .span(Span::styled(
                gibson::node::SPINNER_BRAILLE[app.spinner() % gibson::node::SPINNER_BRAILLE.len()],
                fx.st.accent,
            ))
            .span(Span::styled("  state ", fx.st.muted))
            .span(Span::styled(
                if app.is_done() { "idle" } else { "streaming" },
                fx.st.accent,
            )),
    );
    tpanel("TELEMETRY", fx.st.border)
        .percent_width(100.0)
        .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
}

fn panel_events(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(4) as usize;
    let mut rt = RichText::new();
    if app.debug_scene {
        // Optional story inspector: current beat, facts and time.
        for line in app.director.debug_lines() {
            rt = rt.line(Line::new().span(Span::styled(truncate(&line, inner), fx.st.accent)));
        }
    } else if app.events.is_empty() {
        rt = rt.line(Line::new().span(Span::styled("waiting for events…", fx.st.faint)));
    } else {
        for e in app.events.iter().rev().take(5).rev() {
            let mut line = Line::new();
            for span in &e.spans {
                line = line.span(Span::styled(
                    truncate(span.text.as_str(), inner),
                    span.style,
                ));
            }
            rt = rt.line(line);
        }
    }
    tpanel(
        if app.debug_scene {
            "STORY INSPECTOR"
        } else {
            "EVENTS"
        },
        fx.st.border,
    )
    .percent_width(100.0)
    .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
}

fn panel_footer(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let _ = width;
    // The footer stays a stable prompt; the permission decision floats above it
    // as an overlay modal rather than reflowing the layout.
    let prompt = Node::row()
        .gap(1.0)
        .child(Node::text("❯", fx.st.accent))
        .child(
            Node::text_input(
                &app.input.text,
                app.input.cursor_grapheme,
                Some("type a follow-up…"),
                fx.st.text,
            )
            .scroll_offset(app.input.scroll_offset)
            .flex_grow(1.0),
        );
    tpanel(
        if app.beat() == "prompt" {
            "PROMPT ●"
        } else {
            "PROMPT"
        },
        fx.st.border,
    )
    .percent_width(100.0)
    .height(3.0)
    // The prompt must never be squeezed off-screen by a tall content column.
    .flex_shrink(0.0)
    .child(prompt)
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

/// Capability overrides for fallback proofs: `--mono`, `--ansi16`, `--ansi256`,
/// `--truecolor`, `--no-sync`, `--no-insert-line`.
fn apply_capability_flags(ctx: &mut Context, args: &[String], no_color: bool) {
    use gibson::capability::ColorDepth;
    let has = |f: &str| args.iter().any(|a| a == f);
    let depth = if has("--mono") {
        Some(ColorDepth::Mono)
    } else if has("--ansi16") {
        Some(ColorDepth::Ansi16)
    } else if has("--ansi256") {
        Some(ColorDepth::Ansi256)
    } else if has("--truecolor") {
        Some(ColorDepth::TrueColor)
    } else if no_color {
        Some(ColorDepth::Mono)
    } else {
        None
    };
    if let Some(d) = depth {
        ctx.set_color_depth(d);
    }
    if has("--no-sync") {
        ctx.set_sync_updates(false);
    }
    if has("--no-insert-line") {
        let mut caps = ctx.capabilities();
        caps.insert_line = gibson::Capability::Unsupported;
        ctx.set_capabilities(caps);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let deterministic = args
        .iter()
        .any(|a| a == "--deterministic" || a == "--frames");
    let freeze_at: Option<usize> = args
        .iter()
        .find_map(|a| a.strip_prefix("--freeze-at=").and_then(|v| v.parse().ok()));
    let auto = deterministic
        || args
            .iter()
            .any(|a| a == "--auto" || a == "--scripted" || a == "--headless");
    // Inline is the product identity: normal terminal scrollback plus a stable
    // mutable live foreground. `--fullscreen` opts into the alternate screen.
    let inline = !args.iter().any(|a| a == "--fullscreen");
    let light = args.iter().any(|a| a == "--light");
    let dark = args.iter().any(|a| a == "--dark");
    let no_color = args.iter().any(|a| a == "--no-color");
    let debug = args.iter().any(|a| a == "--debug-renderer");
    let stage = args
        .iter()
        .find_map(|a| a.strip_prefix("--stage=").map(str::to_string));

    let fx = Fx::new(select_theme(light, dark, no_color), !no_color);
    let mut ctx = if inline {
        Context::inline()?
    } else {
        Context::fullscreen()?
    };
    if no_color {
        // Central color ladder: Mono strips every color attribute, even those
        // produced by sub-cell canvases.
        ctx.set_color_depth(gibson::ColorDepth::Mono);
    }
    ctx.set_max_fps(if auto { 240 } else { 60 });
    ctx.set_animation_interval(Duration::from_millis(if auto { 8 } else { 66 }));
    apply_capability_flags(&mut ctx, &args, no_color);

    let mut app = App::new(fx, deterministic, debug, stage.as_deref());
    app.debug_scene = args.iter().any(|a| a == "--debug-scene");

    // ACT 1 — real session open. In inline mode the header and the user request
    // are committed to genuine terminal scrollback, above the live foreground.
    if inline {
        ctx.commit_text(
            "$ gibson-agent .\n\
             workspace  LibGibson\n\
             branch     deepseek/visual-fx-frontier\n\
             model      gibson-v2    session  hardening\n\
             ❯ Audit the renderer, fix clipping, and prove it.",
        )?;
    }

    let mut iterations = 0usize;
    let max_iterations = if auto { 5000 } else { usize::MAX };
    while !app.done && iterations < max_iterations {
        iterations += 1;

        // In deterministic mode, stop advancing state/animation once we reach the
        // frozen frame so the rendered screen is stable and reproducible.
        let frozen = deterministic && freeze_at.is_some_and(|n| iterations > n);
        if !frozen {
            // Advance animation time exactly once per frame.
            let prev = app.time.now();
            let _t = app.time.advance();
            let dt = app.time.now().saturating_sub(prev);
            app.sample(&mut ctx);
            // The director owns beat timing, facts and mounted effect bundles.
            app.director.update(dt, &[]);
            if auto {
                app.auto_step();
            } else {
                app.animate();
            }
        }

        ctx.set_root(build_root(&app, &ctx));

        if auto {
            ctx.request_render();
            ctx.run_once(ctx.animation_interval())?;
        } else if let Some(event) = ctx.run_once(Duration::from_millis(60))? {
            let text_changed = app.handle_event(&event);
            if text_changed {
                ctx.request_render();
            }
        }

        if frozen {
            std::thread::sleep(Duration::from_millis(30));
        }

        // A background event while the user is typing. In inline mode it is
        // pushed into real terminal scrollback above the live dashboard.
        if !auto && app.beat() == "prompt" && !app.bg_sent && app.render_frames > 90 {
            app.bg_sent = true;
            let msg = "index finished while you were typing";
            if inline {
                let rich = RichText::new().line(
                    Line::new()
                        .span(Span::styled("[background] ", app.fx.st.muted))
                        .span(Span::styled(msg, app.fx.st.success)),
                );
                ctx.insert_rich_text_before_live(&rich)?;
            } else {
                app.add_event(msg.to_string(), app.fx.good);
                app.show_toast(msg);
            }
        }
    }

    // Flush one final frame so terminal/exit state is visible before leaving
    // the alternate screen.
    ctx.request_render();
    let _ = ctx.run_once(ctx.animation_interval());

    // ACT 10 — final response is committed to history so the user gets their
    // shell back with an ordinary, greppable record of what actually happened.
    if inline {
        let epilogue = match app.outcome() {
            SessionOutcome::Approved => {
                "✔ session complete · 3 files changed · 17 targeted checks added · clip regression suite green"
            }
            SessionOutcome::Rejected => {
                "✖ session stopped — patch rejected · no files changed · nothing written to disk"
            }
            SessionOutcome::Cancelled => {
                "⊘ session cancelled · no files changed · working tree untouched"
            }
        };
        ctx.commit_text(epilogue)?;
    }
    ctx.restore()?;

    let stats = ctx.stats();
    if inline {
        println!(
            "[metrics] frames {} · insertion {} B (fast {} / fallback {}) · anchor resyncs {}",
            stats.frames,
            stats.insertion_bytes,
            stats.fast_insertions,
            stats.insertion_repaints,
            stats.anchor_resyncs
        );
    } else {
        let label = match app.outcome() {
            SessionOutcome::Approved => "✔ session complete.",
            SessionOutcome::Rejected => "✖ session stopped (patch rejected).",
            SessionOutcome::Cancelled => "⊘ session cancelled.",
        };
        println!(
            "{} {} frames · {} full repaints · {} anchor resyncs · {} B frames · {} B commits",
            label,
            stats.frames,
            stats.full_repaints,
            stats.anchor_resyncs,
            stats.frame_bytes,
            stats.commit_bytes
        );
    }
    Ok(())
}
