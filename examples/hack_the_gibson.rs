//! HACK THE GIBSON — maximalist full-screen Gibson terminal.
//!
//! Owns the framebuffer and renders a live dashboard: a gradient/shimmer banner,
//! mainframe gauges, an animated `garbage.bin` hex dump, a sweeping Da Vinci
//! scan, a network route animation, a scrolling trace feed, a big gradient
//! transfer meter with a throughput sparkline, a tactical payload selector and a
//! root shell. Zero raw ANSI literals — everything is RichText/Span/Theme.
//!
//! `--auto` is deterministic; `--inline` uses the scrollback path; `--light`,
//! `--dark`, `--no-color` are theme proofs.

use gibson::cell::{Color, Line, RichText, Span, Style, Theme};
use gibson::context::Context;
use gibson::input::{Event, KeyCode, KeyModifiers, TextInputState};
use gibson::node::{Node, WrapMode};
use gibson::show;
use gibson::{BorderType, ThemeStyles, TimeSource};
use std::env;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Neon effects
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Fx {
    color: bool,
    green: Color,
    cyan: Color,
    magenta: Color,
    amber: Color,
    red: Color,
    dim: Color,
    st: ThemeStyles,
}

impl Fx {
    fn new(theme: Theme, color: bool) -> Self {
        let st = theme.styles();
        let c = |t: (u8, u8, u8)| {
            if color {
                Color::Rgb(t.0, t.1, t.2)
            } else {
                Color::Reset
            }
        };
        Self {
            color,
            green: c((60, 255, 140)),
            cyan: c((70, 220, 255)),
            magenta: c((255, 90, 220)),
            amber: c((255, 200, 70)),
            red: c((255, 70, 90)),
            dim: c((60, 90, 80)),
            st,
        }
    }

    fn bar(&self, f: f32, w: usize, a: Color, b: Color) -> Line {
        if self.color {
            show::progress(f, w, a, b, self.dim)
        } else {
            let filled = ((f.clamp(0.0, 1.0)) * w as f32).round() as usize;
            Line::raw(format!(
                "{}{}",
                "█".repeat(filled.min(w)),
                "─".repeat(w.saturating_sub(filled))
            ))
        }
    }

    fn spark(&self, v: &[f32], w: usize) -> Line {
        if self.color {
            show::sparkline(v, w, self.green, self.cyan)
        } else {
            Line::raw("─".repeat(w))
        }
    }
}

fn tpanel(title: &str, style: Style) -> Node {
    let mut p = Node::panel(title.to_string(), BorderType::Rounded, style);
    p.layout_style.padding_top = 1.0;
    p.layout_style.padding_bottom = 0.0;
    p
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
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Handshake,
    Breach,
    Transfer,
    Tactical,
    Shell,
    Done,
}

const TACTICAL: &[&str] = &[
    "🌊 Override the Olympic-sized pool on the roof",
    "🐛 Neutralize the Da Vinci virus",
    "🛹 Summon Acid Burn, Cereal Killer & Lord Nikon",
    "🕶  Rollerblade away before Agent Gill arrives",
];

struct App {
    time: TimeSource,
    fx: Fx,
    phase: Phase,
    breach: f32,
    transfer: f32,
    scan: f32,
    hex_seed: u64,
    events: Vec<Line>,
    throughput: Vec<f32>,
    selected: usize,
    input: TextInputState,
    shell_log: Vec<Line>,
    frames: u64,
    last_total: u64,
    done: bool,
    seen: [bool; 8],
}

fn select_theme(light: bool, dark: bool, no_color: bool) -> Theme {
    if no_color {
        Theme::no_color()
    } else if light {
        Theme::light()
    } else if dark {
        Theme::dark()
    } else {
        Theme {
            accent: Color::BrightGreen,
            success: Color::BrightGreen,
            warning: Color::BrightMagenta,
            error: Color::BrightRed,
            border: Color::BrightBlack,
            rail: Color::BrightGreen,
            code: Color::BrightYellow,
            ..Theme::default()
        }
    }
}

impl App {
    fn new(fx: Fx, deterministic: bool) -> Self {
        let time = if deterministic {
            TimeSource::fixed(Duration::from_millis(16))
        } else {
            TimeSource::real()
        };
        Self {
            time,
            fx,
            phase: Phase::Handshake,
            breach: 0.0,
            transfer: 0.0,
            scan: 0.0,
            hex_seed: 1,
            events: Vec::new(),
            throughput: vec![0.0; 48],
            selected: 0,
            input: TextInputState::new(),
            shell_log: Vec::new(),
            frames: 0,
            last_total: 0,
            done: false,
            seen: [false; 8],
        }
    }

    fn elapsed(&self) -> f32 {
        self.time.now().as_secs_f32()
    }

    fn sample(&mut self, ctx: &mut Context) {
        self.frames += 1;
        // Renderer frames, not application loop iterations, drive the wire-byte
        // throughput graph.
        let stats = ctx.stats();
        let delta = stats.frame_bytes.saturating_sub(self.last_total) as f32;
        self.last_total = stats.frame_bytes;
        self.throughput.push(delta);
        if self.throughput.len() > 48 {
            self.throughput.remove(0);
        }
    }

    fn add_event(&mut self, text: &str, color: Color) {
        let stamp = format!("{:04}", self.frames);
        let style = if self.fx.color && color != Color::Reset {
            Style::new().fg(color)
        } else {
            self.fx.st.muted
        };
        self.events.push(
            Line::new()
                .span(Span::styled(format!("{stamp} "), self.fx.st.faint))
                .span(Span::styled(text, style)),
        );
        if self.events.len() > 60 {
            self.events.remove(0);
        }
    }

    fn once(&mut self, idx: usize, text: &str, color: Color) {
        if !self.seen[idx] {
            self.seen[idx] = true;
            self.add_event(text, color);
        }
    }

    fn animate(&mut self) {
        match self.phase {
            Phase::Handshake => {
                self.once(0, "[modem] 28.8k acoustic coupler locked", self.fx.cyan);
                if self.elapsed() > 0.8 {
                    self.once(1, "✔ connected to Gibson Supercomputer", self.fx.green);
                    self.phase = Phase::Breach;
                }
            }
            Phase::Breach => {
                self.breach = (self.breach + 0.02).min(1.0);
                self.scan = (self.scan + 0.10) % 100.0;
                self.hex_seed = self.hex_seed.wrapping_add(1);
                let p = self.breach;
                if p > 0.2 {
                    self.once(2, "[trace] routed through node 0xA0", self.fx.cyan);
                }
                if p > 0.5 {
                    self.once(
                        3,
                        "[trace] Zero Cool & Acid Burn signatures matched",
                        self.fx.magenta,
                    );
                }
                if p > 0.8 {
                    self.once(4, "[quarantine] Da Vinci worm isolated", self.fx.amber);
                }
                if self.breach >= 1.0 {
                    self.phase = Phase::Transfer;
                }
            }
            Phase::Transfer => {
                self.transfer = (self.transfer + 0.02).min(1.0);
                self.hex_seed = self.hex_seed.wrapping_add(1);
                if self.transfer > 0.5 {
                    self.once(5, "[net] reroute through cut fiber, 3ms", self.fx.amber);
                }
                if self.transfer >= 1.0 {
                    self.once(6, "✔ garbage.bin checksum verified", self.fx.green);
                    self.phase = Phase::Tactical;
                }
            }
            Phase::Tactical => {}
            Phase::Shell => {}
            Phase::Done => {}
        }
    }

    fn auto_step(&mut self) {
        match self.phase {
            Phase::Tactical => {
                self.selected = 3;
                self.commit_tactical();
            }
            Phase::Shell => {
                for cmd in ["status", "trace", "pool", "da-vinci", "metrics", "exit"] {
                    self.input = TextInputState::with_text(cmd);
                    self.run_command(cmd);
                    self.input = TextInputState::new();
                }
                self.phase = Phase::Done;
                self.done = true;
            }
            Phase::Done => self.done = true,
            _ => {}
        }
    }

    fn commit_tactical(&mut self) {
        let choice = TACTICAL[self.selected.min(TACTICAL.len() - 1)];
        let line = Line::new()
            .span(Span::styled("[DIRECTIVE] ", self.fx.st.warning))
            .span(Span::styled(choice, self.fx.st.text));
        self.shell_log.push(line);
        self.once(7, "\"HACK THE PLANET! HACK THE PLANET!\"", self.fx.magenta);
        self.phase = Phase::Shell;
    }

    fn run_command(&mut self, cmd: &str) {
        let out = match cmd {
            "help" => vec![(
                "commands: help status pool garbage da-vinci trace metrics exit",
                self.fx.st.muted,
            )],
            "status" => vec![(
                "banks 04/07 ONLINE · route 7 hops · da-vinci SCANNING",
                self.fx.st.code,
            )],
            "pool" => vec![
                (
                    "There is no pool on the roof of Ellingson Mineral!",
                    self.fx.st.accent,
                ),
                (
                    "…sprinkler override initiated. Water pressure critical.",
                    self.fx.st.warning,
                ),
            ],
            "garbage" => vec![(
                "garbage.bin located at /usr/spool/garbage (256 MB)",
                self.fx.st.text,
            )],
            "da-vinci" => vec![(
                "da-vinci worm neutralized; $25,000,000 siphon halted",
                self.fx.st.text,
            )],
            "trace" => vec![("route 66 → gibson-core → zero-cool", self.fx.st.code)],
            "metrics" => vec![(
                "metrics available after exit (see stdout summary)",
                self.fx.st.muted,
            )],
            "exit" | "quit" => vec![(
                "connection severed by foreign host. Skate fast.",
                self.fx.st.error,
            )],
            "" => vec![],
            other => vec![
                (
                    "bash: command not found in /usr/local/bin",
                    self.fx.st.error,
                ),
                (other, self.fx.st.muted),
            ],
        };
        for (text, style) in out {
            self.shell_log.push(
                Line::new()
                    .span(Span::styled("  ", self.fx.st.faint))
                    .span(Span::styled(text, style)),
            );
        }
        if self.shell_log.len() > 200 {
            self.shell_log.remove(0);
        }
    }

    fn handle(&mut self, event: &Event) -> bool {
        if is_cancel(event) {
            self.done = true;
            return true;
        }
        if let Event::Key(k) = event {
            match self.phase {
                Phase::Tactical => match k.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.selected = if self.selected == 0 {
                            TACTICAL.len() - 1
                        } else {
                            self.selected - 1
                        };
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.selected = (self.selected + 1) % TACTICAL.len()
                    }
                    KeyCode::Char(c @ '1'..='4') => self.selected = (c as usize) - ('1' as usize),
                    KeyCode::Enter => self.commit_tactical(),
                    KeyCode::Esc => self.done = true,
                    _ => {}
                },
                Phase::Shell => match k.code {
                    KeyCode::Enter => {
                        let cmd = self.input.text.trim().to_lowercase();
                        self.shell_log.push(
                            Line::new()
                                .span(Span::styled("root@gibson:~# ", self.fx.st.success))
                                .span(Span::styled(self.input.text.clone(), self.fx.st.text)),
                        );
                        self.run_command(&cmd);
                        self.input = TextInputState::new();
                        if cmd == "exit" || cmd == "quit" {
                            self.done = true;
                        }
                    }
                    KeyCode::Esc => self.done = true,
                    _ => return self.input.handle_event(event),
                },
                _ => {}
            }
        } else if let Event::Paste(_) = event {
            if self.phase == Phase::Shell {
                return self.input.handle_event(event);
            }
        }
        false
    }
}

fn is_cancel(event: &Event) -> bool {
    matches!(event, Event::Key(k) if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL))
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn build_root(app: &App, ctx: &Context) -> Node {
    let (cols, rows) = ctx.session.terminal_size();
    let compact = cols < 72;
    let wide = cols >= 118;

    let mut root = Node::col()
        .percent_width(100.0)
        .percent_height(100.0)
        .gap(0.0)
        .child(banner(app, cols, wide));

    if compact {
        root = root.child(panel_mainframe(app, cols).flex_grow(2.0).min_width(0.0));
        root = root.child(panel_garbage(app, cols).flex_grow(2.0).min_width(0.0));
        if rows >= 26 {
            root = root.child(panel_davinci(app, cols).flex_grow(2.0).min_width(0.0));
        }
    } else {
        let right_w: u16 = if wide { 46 } else { 38 };
        let left_w = cols.saturating_sub(right_w + 1);

        let left = Node::col()
            .flex_grow(2.0)
            .min_width(0.0)
            .gap(0.0)
            .child(panel_mainframe(app, left_w).flex_grow(1.0).min_width(0.0))
            .child(panel_garbage(app, left_w).flex_grow(2.0).min_width(0.0));

        let right = Node::col()
            .width(right_w as f32)
            .flex_shrink(0.0)
            .min_width(0.0)
            .gap(0.0)
            .child(panel_davinci(app, right_w).flex_grow(1.0).min_width(0.0))
            .child(panel_trace(app, right_w).flex_grow(2.0).min_width(0.0));

        root = root.child(
            Node::row()
                .percent_width(100.0)
                .gap(1.0)
                .flex_grow(2.0)
                .child(left)
                .child(right),
        );
    }

    root = root.child(panel_transfer(app, cols));
    root = root.child(bottom_panel(app, cols));

    // CRT-ish scanline overlay: a dim band sweeps vertically without touching
    // the layout beneath it (style-only composite). Cheap: only a few rows.
    let band = (app.elapsed() * 0.9) as u16;
    let y = (band % (rows.max(1))) as f32;
    Node::stack()
        .percent_width(100.0)
        .percent_height(100.0)
        .child(root)
        .child(
            Node::col()
                .percent_width(100.0)
                .percent_height(100.0)
                .padding_top(y)
                .child(Node::dim().percent_width(100.0).height(1.0))
                .child(Node::dim().percent_width(100.0).height(2.0)),
        )
}

fn banner(app: &App, cols: u16, wide: bool) -> Node {
    let fx = &app.fx;
    let mut line = shimmer(
        "▚▚▚ GIBSON MAINFRAME ▞▞▞",
        fx,
        (app.elapsed() * 26.0) as usize,
    );
    line = line.span(Span::styled("   ", fx.st.text));
    line = line.span(Span::styled(" DEFCON 1 ", Style::new().fg(fx.red).bold()));
    line = line.span(Span::styled("  ● LINK ", fx.st.success));
    if wide {
        line = line
            .span(Span::styled("  baud ", fx.st.muted))
            .span(Span::styled("28.8k", fx.st.code))
            .span(Span::styled("  ·  route ", fx.st.muted))
            .span(Span::styled("7 hops / 42ms", fx.st.text))
            .span(Span::styled("  ·  da-vinci ", fx.st.muted))
            .span(Span::styled("SCANNING", fx.st.warning));
    } else if cols > 50 {
        line = line.span(Span::styled("  28.8k · 7 hops", fx.st.muted));
    }
    Node::line(line).height(1.0)
}

fn shimmer(text: &str, fx: &Fx, offset: usize) -> Line {
    if !fx.color {
        return Line::styled(text, Style::new().bold());
    }
    use unicode_segmentation::UnicodeSegmentation as _;
    let gs: Vec<&str> = text.graphemes(true).collect();
    let n = gs.len().max(1);
    let mut spans = Vec::with_capacity(n);
    for (i, g) in gs.iter().enumerate() {
        let t = i as f32 / (n - 1).max(1) as f32;
        let base = fx.green.lerp(fx.cyan, t);
        let d = ((i + offset) % 20) as f32;
        let boost = (1.0 - (d.min(20.0 - d) / 10.0)).clamp(0.0, 1.0) * 0.6;
        spans.push(Span::styled(
            *g,
            Style::new()
                .fg(base.lerp(Color::Rgb(255, 255, 255), boost))
                .bold(),
        ));
    }
    Line::from_spans(spans)
}

fn panel_mainframe(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(4) as usize;
    let bar_w = inner.saturating_sub(12).clamp(8, 26);
    let pulse = (app.elapsed() * 2.0).sin() * 0.5 + 0.5;
    let cpu = (0.55 + 0.35 * pulse).clamp(0.0, 1.0);
    let mem = (0.42 + 0.30 * ((app.elapsed() * 1.3).sin() * 0.5 + 0.5)).clamp(0.0, 1.0);

    let mut rt = RichText::new();
    for (label, value) in [("CPU", cpu), ("MEM", mem)] {
        let mut line = Line::new().span(Span::styled(format!("{label} "), fx.st.muted));
        for s in fx.bar(value, bar_w, fx.green, fx.cyan).spans {
            line = line.span(s);
        }
        line = line.span(Span::styled(
            format!(" {:>3}%", (value * 100.0) as usize),
            fx.st.text,
        ));
        rt = rt.line(line);
    }
    rt = rt.line(
        Line::new()
            .span(Span::styled("BANKS ", fx.st.muted))
            .span(Span::styled("04/07 ONLINE", fx.st.success))
            .span(Span::styled("   ENTROPY ", fx.st.muted))
            .span(Span::styled(
                format!("{:.3}", 0.99 + 0.009 * pulse),
                fx.st.code,
            )),
    );
    rt = rt.line(
        Line::new()
            .span(Span::styled("BREACH ", fx.st.muted))
            .span(Span::styled(
                gibson::node::SPINNER_BRAILLE
                    [(app.elapsed() * 12.0) as usize % gibson::node::SPINNER_BRAILLE.len()],
                fx.st.accent,
            ))
            .span(Span::styled(
                format!(" {:>3}%", (app.breach * 100.0) as usize),
                fx.st.warning,
            )),
    );
    tpanel("MAINFRAME", fx.st.border)
        .percent_width(100.0)
        .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
}

fn panel_garbage(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(4) as usize;
    let cols_hex = (inner / 3).clamp(4, 18);
    let mut rt = RichText::new();
    let rows = 8u64;
    for i in 0..rows {
        rt = rt.line(show::hex_dump_line(
            app.hex_seed.wrapping_add(i * 2_654_435_761),
            cols_hex,
            fx.dim,
            fx.green,
        ));
    }
    tpanel("garbage.bin", fx.st.border)
        .percent_width(100.0)
        .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
}

fn panel_davinci(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(4) as usize;
    let scan_cols = inner.clamp(6, 20) as u16;

    // Sub-cell Braille instrument: a moving worm plus a sweeping scanner beam.
    let rows = 4u16;
    let mut worm = gibson::BrailleCanvas::new(scan_cols, rows);
    let mut beam = gibson::BrailleCanvas::new(scan_cols, rows);
    let pw = worm.pixel_width() as f32;
    let ph = worm.pixel_height() as f32;
    let t = app.elapsed();
    let mut prev: Option<(i32, i32)> = None;
    let mut x = 0i32;
    while x < worm.pixel_width() as i32 {
        let fx1 = x as f32 / pw;
        let y = (ph * 0.5
            + (fx1 * 9.0 + t * 2.2).sin() * (ph * 0.32)
            + (fx1 * 23.0 + t * 1.1).sin() * (ph * 0.08))
            .clamp(0.0, ph - 1.0);
        let yi = y.round() as i32;
        if let Some((px, py)) = prev {
            worm.line(px, py, x, yi);
        } else {
            worm.set(x, yi);
        }
        prev = Some((x, yi));
        x += 1;
    }
    // Scanner beam sweeps across the instrument.
    let beam_x = ((app.scan / 100.0) * (worm.pixel_width().saturating_sub(1) as f32)) as i32;
    beam.line(beam_x, 0, beam_x, worm.pixel_height() as i32 - 1);

    let mut rt = RichText::new().line(
        Line::new()
            .span(Span::styled("SCAN ", fx.st.muted))
            .span(Span::styled(
                gibson::node::SPINNER_BRAILLE
                    [(app.scan as usize) % gibson::node::SPINNER_BRAILLE.len()],
                fx.st.warning,
            ))
            .span(Span::styled("  sweep ", fx.st.muted))
            .span(Span::styled(format!("{:>3.0}%", app.scan), fx.st.text)),
    );

    for cy in 0..rows {
        let mut line = Line::new();
        for cx in 0..scan_cols {
            let b = beam.glyph_at(cx, cy);
            let w = worm.glyph_at(cx, cy);
            if let Some(ch) = b {
                let style = if fx.color {
                    Style::new().fg(fx.magenta)
                } else {
                    Style::default()
                };
                line = line.span(Span::styled(ch.to_string(), style));
            } else if let Some(ch) = w {
                let style = if fx.color {
                    Style::new().fg(fx.green)
                } else {
                    Style::default()
                };
                line = line.span(Span::styled(ch.to_string(), style));
            } else {
                line = line.span(Span::raw(" "));
            }
        }
        rt = rt.line(line);
    }
    rt = rt.line(
        Line::new()
            .span(Span::styled("SIG ", fx.st.muted))
            .span(Span::styled("da-vinci", fx.st.warning))
            .span(Span::styled("  financial worm ", fx.st.muted))
            .span(Span::styled("QUARANTINED", fx.st.success)),
    );
    tpanel("DA VINCI SCAN", fx.st.border)
        .percent_width(100.0)
        .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
}

fn panel_trace(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(4) as usize;
    let mut rt = RichText::new();
    if app.events.is_empty() {
        rt = rt.line(Line::new().span(Span::styled("no trace yet…", fx.st.faint)));
    } else {
        for e in app.events.iter().rev().take(10).rev() {
            let mut line = Line::new();
            for s in &e.spans {
                line = line.span(Span::styled(truncate(s.text.as_str(), inner), s.style));
            }
            rt = rt.line(line);
        }
    }
    tpanel("EVENT TRACE", fx.st.border)
        .percent_width(100.0)
        .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
}

fn panel_transfer(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(4) as usize;
    let bar_w = inner.saturating_sub(20).clamp(10, 48);
    let pct = app.transfer.clamp(0.0, 1.0);
    let mb = (pct * 256.0) as usize;
    let mut line = Line::new().span(Span::styled("garbage.bin ", fx.st.code));
    for s in fx.bar(pct, bar_w, fx.green, fx.magenta).spans {
        line = line.span(s);
    }
    line = line.span(Span::styled(
        format!(" {:>3}%  {} / 256 MB", (pct * 100.0) as usize, mb),
        fx.st.text,
    ));
    let rt = RichText::new()
        .line(line)
        .line(
            Line::new()
                .span(Span::styled("throughput ", fx.st.muted))
                .span(Span::styled(
                    format!("{:.1} MB/s", 18.0 + 22.0 * pct),
                    fx.st.success,
                ))
                .span(Span::styled("  │  checksum ", fx.st.muted))
                .span(Span::styled("sha256:e3b0…b855", fx.st.code)),
        )
        .line(fx.spark(&app.throughput, bar_w.min(40)));
    tpanel("TRANSFER", fx.st.border)
        .percent_width(100.0)
        .height(6.0)
        .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
}

fn bottom_panel(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(4) as usize;
    match app.phase {
        Phase::Tactical => {
            let mut rt = RichText::new().line(
                Line::new()
                    .span(Span::styled("SELECT PAYLOAD ", fx.st.accent))
                    .span(Span::styled(
                        "↑/↓ or j/k · Enter to fire · 1-4",
                        fx.st.muted,
                    )),
            );
            for (i, opt) in TACTICAL.iter().enumerate() {
                let sel = i == app.selected;
                rt = rt.line(
                    Line::new()
                        .span(Span::styled(
                            if sel { "▶ " } else { "  " },
                            if sel { fx.st.warning } else { fx.st.faint },
                        ))
                        .span(Span::styled(format!("[{}] ", i + 1), fx.st.muted))
                        .span(Span::styled(
                            truncate(opt, inner.saturating_sub(8)),
                            if sel { Style::new().bold() } else { fx.st.text },
                        )),
                );
            }
            tpanel("TACTICAL PAYLOAD", fx.st.warning)
                .percent_width(100.0)
                .height(7.0)
                .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
        }
        Phase::Shell | Phase::Done => {
            let mut rt = RichText::new();
            for line in app.shell_log.iter().rev().take(4).rev() {
                rt = rt.line(line.clone());
            }
            let caret = if ((app.elapsed() * 2.0) as usize).is_multiple_of(2) {
                "▌"
            } else {
                " "
            };
            rt = rt.line(
                Line::new()
                    .span(Span::styled("root@gibson:~# ", fx.st.success))
                    .span(Span::styled(app.input.text.clone(), fx.st.text))
                    .span(Span::styled(caret, fx.st.accent)),
            );
            tpanel("ROOT SHELL", fx.st.success)
                .percent_width(100.0)
                .height(7.0)
                .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
        }
        _ => {
            let mut rt = RichText::new().line(
                Line::new()
                    .span(Span::styled("breaching Gibson ", fx.st.accent))
                    .span(Span::styled(
                        gibson::node::SPINNER_BRAILLE
                            [(app.elapsed() * 12.0) as usize % gibson::node::SPINNER_BRAILLE.len()],
                        fx.st.warning,
                    ))
                    .span(Span::styled("  hold tight…", fx.st.muted)),
            );
            rt = rt.line(Line::new().span(Span::styled(
                truncate(
                    &format!(
                        "phase {:?}  ·  breach {:>3}%  ·  transfer {:>3}%",
                        app.phase,
                        (app.breach * 100.0) as usize,
                        (app.transfer * 100.0) as usize
                    ),
                    inner,
                ),
                fx.st.text,
            )));
            tpanel("STATUS", fx.st.border)
                .percent_width(100.0)
                .height(4.0)
                .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
        }
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let deterministic = args.iter().any(|a| a == "--deterministic");
    let freeze_at: Option<usize> = args
        .iter()
        .find_map(|a| a.strip_prefix("--freeze-at=").and_then(|v| v.parse().ok()));
    let auto = deterministic
        || args
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
    ctx.set_animation_interval(Duration::from_millis(if auto { 8 } else { 45 }));

    let mut app = App::new(fx, deterministic);

    let mut iterations = 0usize;
    let cap = if auto { 8000 } else { usize::MAX };
    while !app.done && iterations < cap {
        iterations += 1;
        let frozen = deterministic && freeze_at.is_some_and(|n| iterations > n);
        if !frozen {
            app.time.advance();
            app.sample(&mut ctx);
            app.animate();
            if auto {
                app.auto_step();
            }
        }

        ctx.set_root(build_root(&app, &ctx));

        if auto {
            ctx.request_render();
            ctx.run_once(ctx.animation_interval())?;
        } else if let Some(event) = ctx.run_once(Duration::from_millis(50))? {
            if app.handle(&event) {
                ctx.request_render();
            }
        }
        if frozen {
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    // Flush one final frame so terminal/exit state is visible before leaving
    // the alternate screen.
    ctx.request_render();
    let _ = ctx.run_once(ctx.animation_interval());
    ctx.restore()?;

    let stats = ctx.stats();
    println!(
        "✔ Gibson connection closed. {} frames · {} B frames · {} B inserts · {} anchor resyncs",
        stats.frames, stats.frame_bytes, stats.insertion_bytes, stats.anchor_resyncs
    );
    Ok(())
}
