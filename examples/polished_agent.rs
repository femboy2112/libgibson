//! Polished Agent — a restrained, full-screen agent console.
//!
//! It owns the character-cell framebuffer and renders a persistent multi-panel
//! dashboard: a gradient shimmer banner, a transcript, a live task plan with
//! sub-cell progress meters, real telemetry (frame-byte sparkline), a streaming
//! result panel, an event feed, and a permission/prompt footer.
//!
//! Modes: `--auto` (deterministic), `--inline` (uses the scrollback insertion
//! path), and theme proofs `--light` `--dark` `--no-color`.

use gibson::cell::{Color, Line, RichText, Span, Style, Theme};
use gibson::context::Context;
use gibson::input::{Event, KeyCode, KeyModifiers, TextInputState};
use gibson::node::{Node, WrapMode};
use gibson::show;
use gibson::{BorderType, ThemeStyles};
use std::env;
use std::time::{Duration, Instant};

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
}

struct Tool {
    verb: &'static str,
    target: &'static str,
    progress: f32,
    state: ToolState,
}

impl Tool {
    fn new(verb: &'static str, target: &'static str) -> Self {
        Self {
            verb,
            target,
            progress: 0.0,
            state: ToolState::Queued,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Plan,
    Permission,
    Prompt,
    Done,
}

const STREAM_TEXT: &str = "I audited the differential pipeline and the scrollback handoff. The live region now tracks an explicit physical anchor, so a resize forces a full re-anchor instead of trusting stale coordinates. Insert-before-live is proven to leave the live framebuffer untouched on the fast path, with a correct repaint fallback otherwise.";

const PERMISSION_OPTIONS: &[(&str, &str)] = &[
    ("Approve once", "apply for this step"),
    ("Approve for session", "trust this class of change"),
    ("Reject", "abort without touching disk"),
];

struct App {
    started: Instant,
    fx: Fx,
    tools: Vec<Tool>,
    tool_cursor: usize,
    phase: Phase,
    stream: String,
    stream_shown: usize,
    transcript: Vec<Line>,
    events: Vec<Line>,
    permission: usize,
    approved: bool,
    input: TextInputState,
    bg_sent: bool,
    frame_bytes: Vec<f32>,
    last_total_bytes: u64,
    frames: u64,
    done: bool,
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
    fn new(fx: Fx) -> Self {
        Self {
            started: Instant::now(),
            fx,
            tools: vec![
                Tool::new("research", "src/renderer.rs"),
                Tool::new("search", "insert_before_live"),
                Tool::new("build", "cargo test"),
                Tool::new("patch", "src/ansi.rs"),
            ],
            tool_cursor: 0,
            phase: Phase::Plan,
            stream: STREAM_TEXT.to_string(),
            stream_shown: 0,
            transcript: Vec::new(),
            events: Vec::new(),
            permission: 0,
            approved: false,
            input: TextInputState::new(),
            bg_sent: false,
            frame_bytes: Vec::new(),
            last_total_bytes: 0,
            frames: 0,
            done: false,
        }
    }

    fn elapsed(&self) -> f32 {
        self.started.elapsed().as_secs_f32()
    }

    fn spinner(&self) -> usize {
        (self.elapsed() * 12.0) as usize
    }

    fn shimmer_offset(&self) -> usize {
        (self.elapsed() * 20.0) as usize
    }

    fn fps(&self) -> u64 {
        let e = self.elapsed().max(0.05);
        (self.frames as f32 / e).round() as u64
    }

    fn sample(&mut self, ctx: &mut Context) {
        self.frames += 1;
        let total = ctx.stats().frame_bytes;
        let delta = total.saturating_sub(self.last_total_bytes) as f32;
        self.last_total_bytes = total;
        self.frame_bytes.push(delta);
        if self.frame_bytes.len() > 200 {
            self.frame_bytes.remove(0);
        }
    }

    /// Advance decorative animation and the plan timeline.
    fn animate(&mut self) {
        if self.phase == Phase::Plan {
            if let Some(tool) = self.tools.get_mut(self.tool_cursor) {
                if tool.state == ToolState::Queued {
                    tool.state = ToolState::Running;
                }
                tool.progress = (tool.progress + 0.06).min(1.0);
                if tool.progress >= 1.0 {
                    tool.state = match self.tool_cursor {
                        2 => ToolState::Warn,
                        3 => ToolState::Failed,
                        _ => ToolState::Done,
                    };
                    let line = tool_line(tool, &self.fx);
                    self.transcript.push(line);
                    self.tool_cursor += 1;
                }
            }
            self.stream_shown = (self.stream_shown + 9).min(self.stream.chars().count());
            if self.tool_cursor == 2 && !self.bg_sent {
                self.bg_sent = true;
                self.add_event("diagnostics refreshed · 0 warnings".into(), self.fx.good);
            }
            if self.tool_cursor >= self.tools.len() {
                self.add_event("patch conflict: needs review".into(), Color::Reset);
                self.phase = Phase::Permission;
            }
        }
    }

    fn auto_step(&mut self) {
        match self.phase {
            Phase::Plan => self.animate(),
            Phase::Permission => {
                self.permission = 0;
                self.confirm_permission();
            }
            Phase::Prompt => {
                for ch in "review the fast-path invariant 🦀 你好世界 e\u{0301} 🇺🇸".chars()
                {
                    self.input.insert_char(ch);
                }
                self.bg_sent = true;
                self.add_event("index finished while you were typing".into(), self.fx.good);
                self.phase = Phase::Done;
                self.done = true;
            }
            Phase::Done => self.done = true,
        }
    }

    fn add_event(&mut self, text: String, color: Color) {
        let stamp = format!("{:04}", self.frames);
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
            self.add_event("cancelled by operator (Ctrl-C)".into(), self.fx.bad);
            self.phase = Phase::Done;
            self.done = true;
            return true;
        }
        match event {
            Event::Key(k) => match self.phase {
                Phase::Permission => match k.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.permission = if self.permission == 0 {
                            2
                        } else {
                            self.permission - 1
                        };
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.permission = (self.permission + 1) % 3
                    }
                    KeyCode::Char(c @ '1'..='3') => self.permission = (c as usize) - ('1' as usize),
                    KeyCode::Char('a') => {
                        self.permission = 1;
                        self.confirm_permission();
                    }
                    KeyCode::Enter => self.confirm_permission(),
                    KeyCode::Esc => {
                        self.permission = 2;
                        self.confirm_permission();
                    }
                    _ => {}
                },
                Phase::Prompt => match k.code {
                    KeyCode::Enter => {
                        let text = self.input.text.clone();
                        self.transcript.push(
                            Line::new()
                                .span(Span::styled("❯ ", self.fx.st.accent))
                                .span(Span::styled(text, self.fx.st.text)),
                        );
                        self.input = TextInputState::new();
                    }
                    KeyCode::Esc => {
                        self.phase = Phase::Done;
                        self.done = true;
                    }
                    _ => return self.input.handle_event(event),
                },
                _ => {}
            },
            Event::Paste(_) => {
                return self.phase == Phase::Prompt && self.input.handle_event(event)
            }
            _ => {}
        }
        false
    }

    fn confirm_permission(&mut self) {
        self.approved = self.permission != 2;
        let (glyph, style, msg) = if self.approved {
            (
                "✔ ",
                self.fx.st.success,
                "patch applied  src/ansi.rs  (+18 −2)",
            )
        } else {
            ("✖ ", self.fx.st.error, "patch rejected")
        };
        self.transcript.push(
            Line::new()
                .span(Span::styled(glyph, style))
                .span(Span::styled(msg, self.fx.st.text)),
        );
        self.phase = Phase::Prompt;
    }
}

fn is_cancel(event: &Event) -> bool {
    matches!(event, Event::Key(k) if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL))
}

fn tool_line(tool: &Tool, fx: &Fx) -> Line {
    let (marker, style) = match tool.state {
        ToolState::Queued => ("·", fx.st.faint),
        ToolState::Running => ("◐", fx.st.accent),
        ToolState::Done => ("✔", fx.st.success),
        ToolState::Warn => ("▲", fx.st.warning),
        ToolState::Failed => ("✖", fx.st.error),
    };
    Line::new()
        .span(Span::styled(format!("{marker} "), style))
        .span(Span::styled(format!("{:<9}", tool.verb), fx.st.code))
        .span(Span::styled(tool.target, fx.st.text))
}

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
            root = root.child(panel_telemetry(app).flex_grow(1.0).min_width(0.0));
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
            .child(panel_telemetry(app).flex_grow(1.0).min_width(0.0))
            .child(panel_events(app, right_w).flex_grow(1.0).min_width(0.0));

        root = root.child(
            Node::row()
                .percent_width(100.0)
                .gap(1.0)
                .flex_grow(1.0)
                .child(left)
                .child(right),
        );
    }

    root = root.child(panel_footer(app, cols));
    root
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
    Node::line(line).height(1.0)
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
        };
        let progress = if tool.state == ToolState::Queued {
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

fn panel_transcript(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(4) as usize;
    let mut rt = RichText::new();
    rt = rt.line(
        Line::new()
            .span(Span::styled("❯ ", fx.st.accent))
            .span(Span::styled(
                truncate(
                    "Harden the renderer invariants and make the demos exceptional",
                    inner.saturating_sub(2),
                ),
                fx.st.text,
            )),
    );
    for line in &app.transcript {
        rt = rt.line(line.clone());
    }
    tpanel("TRANSCRIPT", fx.st.border)
        .percent_width(100.0)
        .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
}

fn panel_telemetry(app: &App) -> Node {
    let fx = &app.fx;
    let spark_w = 30usize;
    let latest = app.frame_bytes.last().copied().unwrap_or(0.0) as u64;
    let rt = RichText::new()
        .line(
            Line::new()
                .span(Span::styled("frame ", fx.st.muted))
                .span(Span::styled(format!("{latest:>5} B"), fx.st.text))
                .span(Span::styled("   frames ", fx.st.muted))
                .span(Span::styled(format!("{:<5}", app.frames), fx.st.text)),
        )
        .line(fx.spark(&app.frame_bytes, spark_w))
        .line(
            Line::new()
                .span(Span::styled("fps ", fx.st.muted))
                .span(Span::styled(format!("{:<4}", app.fps()), fx.st.text))
                .span(Span::styled("spinner ", fx.st.muted))
                .span(Span::styled(
                    gibson::node::SPINNER_BRAILLE
                        [app.spinner() % gibson::node::SPINNER_BRAILLE.len()],
                    fx.st.accent,
                ))
                .span(Span::styled("  state ", fx.st.muted))
                .span(Span::styled(
                    if app.phase == Phase::Done {
                        "idle"
                    } else {
                        "streaming"
                    },
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
    if app.events.is_empty() {
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
    tpanel("EVENTS", fx.st.border)
        .percent_width(100.0)
        .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
}

fn panel_footer(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(4) as usize;
    if app.phase == Phase::Permission {
        let mut rt = RichText::new().line(
            Line::new()
                .span(Span::styled("Allow ", fx.st.muted))
                .span(Span::styled("patch src/ansi.rs", fx.st.text))
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
            if inner > 44 {
                line = line.span(Span::styled(format!("  — {detail}"), fx.st.muted));
            }
            rt = rt.line(line);
        }
        tpanel("PERMISSION", fx.st.warning)
            .percent_width(100.0)
            .height(6.0)
            .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
    } else {
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
        tpanel("PROMPT", fx.st.border)
            .percent_width(100.0)
            .height(3.0)
            .child(prompt)
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let auto = args
        .iter()
        .any(|a| a == "--auto" || a == "--scripted" || a == "--headless");
    let inline = args.iter().any(|a| a == "--inline");
    let light = args.iter().any(|a| a == "--light");
    let dark = args.iter().any(|a| a == "--dark");
    let no_color = args.iter().any(|a| a == "--no-color");

    let fx = Fx::new(select_theme(light, dark, no_color), !no_color);
    let mut ctx = if inline {
        Context::inline()?
    } else {
        Context::fullscreen()?
    };
    ctx.set_max_fps(if auto { 240 } else { 60 });
    ctx.set_animation_interval(Duration::from_millis(if auto { 8 } else { 66 }));

    let mut app = App::new(fx);

    let mut iterations = 0usize;
    let max_iterations = if auto { 5000 } else { usize::MAX };
    while !app.done && iterations < max_iterations {
        iterations += 1;

        app.sample(&mut ctx);
        if auto {
            app.auto_step();
        } else {
            app.animate();
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

        // A background event while the user is typing. In inline mode it is
        // pushed into real terminal scrollback above the live dashboard.
        if !auto && app.phase == Phase::Prompt && !app.bg_sent && app.frames > 90 {
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
            }
        }
    }

    // Flush one final frame so terminal/exit state is visible before leaving
    // the alternate screen.
    ctx.request_render();
    let _ = ctx.run_once(ctx.animation_interval());
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
        println!(
            "{} {} frames · {} full repaints · {} anchor resyncs · {} B frames · {} B commits",
            if app.approved {
                "✔ session complete."
            } else {
                "session complete."
            },
            stats.frames,
            stats.full_repaints,
            stats.anchor_resyncs,
            stats.frame_bytes,
            stats.commit_bytes
        );
    }
    Ok(())
}
