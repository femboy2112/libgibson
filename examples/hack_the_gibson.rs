//! HACK THE GIBSON — maximalist LibGibson showcase.
//!
//! The loud twin of `polished_agent`. Where that demo shows restraint, this one
//! pushes the engine hard while staying architecturally clean:
//!
//! * structured `RichText`/`Span`/`Line` and semantic `Theme` roles (no raw ANSI)
//! * responsive layout at 40 / 60 / 80 / 120 / 160 columns (no fixed width)
//! * differential live animation driven by the frame scheduler
//! * `insert_before_live` async events above a stable live viewport
//! * a persistent grapheme-aware shell `TextInput`
//! * deterministic `--auto` mode for CI and snapshotting
//!
//! `--light`, `--dark`, `--no-color` select the palette.

use gibson::cell::{Color, Line, RichText, Span, Style, Theme};
use gibson::context::Context;
use gibson::input::{Event, KeyCode, TextInputState};
use gibson::node::Node;
use std::env;
use std::time::{Duration, Instant};

fn hacker_theme(light: bool, no_color: bool) -> Theme {
    if no_color {
        return Theme::no_color();
    }
    if light {
        Theme {
            text: Color::Black,
            text_muted: Color::Black,
            accent: Color::Green,
            success: Color::Green,
            warning: Color::Magenta,
            error: Color::Red,
            border: Color::BrightBlack,
            rail: Color::Green,
            code: Color::Magenta,
            ..Theme::default()
        }
    } else {
        Theme {
            text: Color::Reset,
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

fn meter(progress: usize, width: usize) -> String {
    let width = width.max(6);
    let filled = (progress.min(100) * width) / 100;
    let empty = width.saturating_sub(filled);
    format!(
        "{}{} {:>3}%",
        "█".repeat(filled),
        "░".repeat(empty),
        progress.min(100)
    )
}

struct Act {
    title: &'static str,
    message: &'static str,
    progress: usize,
}

const BREACH_ACTS: &[Act] = &[
    Act {
        title: "ROUTING",
        message: "Bypassing Eugene 'The Plague' Belford's firewall...",
        progress: 18,
    },
    Act {
        title: "SCANNING",
        message: "Searching memory banks for the Olympic pool on the roof...",
        progress: 41,
    },
    Act {
        title: "TRACING",
        message: "Zero Cool & Acid Burn signatures detected in kernel space...",
        progress: 67,
    },
    Act {
        title: "QUARANTINE",
        message: "Isolating the Da Vinci virus financial worm in /usr/spool/garbage...",
        progress: 88,
    },
    Act {
        title: "EXFIL",
        message: "Downloading the garbage file to a 3.5\" neon floppy...",
        progress: 100,
    },
];

const TACTICAL_OPTIONS: &[&str] = &[
    "🌊 Override the Olympic-sized swimming pool on the roof",
    "🐛 Neutralize the Da Vinci virus before the tanker fleet capsizes",
    "🛹 Summon Acid Burn, Cereal Killer & Lord Nikon",
    "🕶  Rollerblade away before Agent Richard Gill arrives",
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let auto = args
        .iter()
        .any(|a| a == "--auto" || a == "--scripted" || a == "--headless");
    let light = args.iter().any(|a| a == "--light");
    let no_color = args.iter().any(|a| a == "--no-color");

    let theme = hacker_theme(light, no_color);
    let st = theme.styles();

    let mut ctx = Context::inline()?;
    ctx.set_max_fps(if auto { 240 } else { 60 });
    ctx.set_animation_interval(Duration::from_millis(if auto { 8 } else { 80 }));

    let started = Instant::now();

    act_i_handshake(&mut ctx, &theme, &st, auto, started)?;
    act_ii_breach(&mut ctx, &theme, &st, auto, started)?;
    act_iii_transfer(&mut ctx, &theme, &st, auto, started)?;
    act_iv_payload(&mut ctx, &theme, &st, auto)?;
    act_v_shell(&mut ctx, &theme, &st, auto)?;
    act_vi_telemetry(&mut ctx, &theme, &st)?;

    ctx.restore()?;
    println!(
        "{}",
        if no_color {
            "Gibson connection closed. Terminal restored."
        } else {
            "\u{1b}[1;32m✔ Gibson connection closed. Terminal restored.\u{1b}[0m"
        }
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// ACT I — MODEM / HANDSHAKE
// ---------------------------------------------------------------------------

fn act_i_handshake(
    ctx: &mut Context,
    theme: &Theme,
    st: &gibson::ThemeStyles,
    auto: bool,
    _started: Instant,
) -> Result<(), Box<dyn std::error::Error>> {
    let (cols, _) = ctx.session.terminal_size();
    let wide = cols >= 100;

    let mut handshake = RichText::new()
        .line(
            Line::new()
                .span(Span::styled("◤ LIBGIBSON ", st.accent))
                .span(Span::styled(
                    "// ELLINGSon MINERAL CORP // GIBSON MAINFRAME",
                    st.muted,
                )),
        )
        .line(
            Line::new()
                .span(Span::styled("[modem] ", st.muted))
                .span(Span::styled(
                    "28.8k acoustic coupler locked on (212) 555-0199",
                    st.text,
                )),
        );

    if wide {
        handshake = handshake.line(
            Line::new()
                .span(Span::styled("carrier ", st.muted))
                .span(Span::styled("detected", st.success))
                .span(Span::styled(" │ protocol ", st.muted))
                .span(Span::styled("VT100+ / DEC-2026", st.code)),
        );
    }

    ctx.commit_rich_text(&handshake)?;

    // Typewriter handshake in the live region.
    let quote = "Mess with the best, die like the rest.";
    let frames = if auto { quote.len().min(6) } else { 14 };
    for i in 0..frames.saturating_add(1) {
        let shown: String = quote.chars().take(i * 4).collect();
        let root = Node::col().percent_width(100.0).child(Node::rich_text(
            RichText::new().line(
                Line::new()
                    .span(Span::styled("\"", st.warning))
                    .span(Span::styled(shown, st.warning))
                    .span(Span::styled("\" — Zero Cool", st.muted)),
            ),
        ));
        ctx.set_root(root);
        tick(ctx, 1)?;
    }
    ctx.clear_live_region()?;

    ctx.commit_rich_text(
        &RichText::new().line(
            Line::new()
                .span(Span::styled("✔ ", st.success))
                .span(Span::styled(
                    "Connected to Gibson Supercomputer (OS: UNIX System V / LibGibson)",
                    st.text,
                )),
        ),
    )?;
    let _ = theme;
    Ok(())
}

// ---------------------------------------------------------------------------
// ACT II — GIBSON BREACH
// ---------------------------------------------------------------------------

fn act_ii_breach(
    ctx: &mut Context,
    _theme: &Theme,
    st: &gibson::ThemeStyles,
    auto: bool,
    started: Instant,
) -> Result<(), Box<dyn std::error::Error>> {
    let iterations = if auto { 3 } else { 10 };
    for (idx, act) in BREACH_ACTS.iter().enumerate() {
        for _ in 0..iterations {
            let frame = animation_frame(started, ctx.animation_interval());
            let root = build_breach_dashboard(ctx, frame, act, st);
            ctx.set_root(root);
            tick(ctx, 1)?;
        }
        // Fire an asynchronous trace event above the live viewport.
        let mut notice =
            RichText::new().line(Line::new().span(Span::styled("[trace] ", st.muted)).span(
                Span::styled(
                    format!(
                        "packet vector {} resolved through node 0x{:02X}",
                        idx + 1,
                        0xA0 + idx
                    ),
                    st.code,
                ),
            ));
        ctx.insert_rich_text_before_live(&notice)?;
        notice = RichText::new();
    }

    ctx.clear_live_region()?;
    ctx.commit_rich_text(
        &RichText::new().line(
            Line::new()
                .span(Span::styled("★ ", st.success))
                .span(Span::styled(
                    "ACCESS GRANTED: root privileges obtained on the Gibson mainframe!",
                    st.text,
                )),
        ),
    )?;
    Ok(())
}

fn build_breach_dashboard(
    ctx: &Context,
    frame: usize,
    act: &Act,
    st: &gibson::ThemeStyles,
) -> Node {
    let (cols, _) = ctx.session.terminal_size();
    let narrow = cols < 60;
    let wide = cols >= 100;

    let header = if narrow {
        Node::row()
            .gap(1.0)
            .child(Node::spinner(frame, st.accent, Some(act.title)).flex_grow(1.0))
    } else {
        Node::row()
            .gap(1.0)
            .child(Node::spinner(frame, st.accent, Some(act.title)).flex_grow(1.0))
            .child(Node::text("DEFCON 1", st.error))
            .child(Node::text("│", st.border))
            .child(Node::text(format!("{}%", act.progress), st.warning))
    };

    let meter_width = if cols < 40 {
        12
    } else {
        (cols as usize).saturating_sub(12).min(40)
    };
    let progress_line = Line::new()
        .span(Span::styled("breach ", st.muted))
        .span(Span::styled(meter(act.progress, meter_width), st.success));

    let mut body = RichText::new()
        .line(Line::new().span(Span::styled(act.message, st.text)))
        .line(progress_line);

    if wide {
        body = body
            .line(
                Line::new()
                    .span(Span::styled("banks ", st.muted))
                    .span(Span::styled("04/07 ONLINE", st.success))
                    .span(Span::styled(" │ route ", st.muted))
                    .span(Span::styled("7 hops / 42ms", st.code))
                    .span(Span::styled(" │ da-vinci ", st.muted))
                    .span(Span::styled("SCANNING", st.warning)),
            )
            .line(
                Line::new()
                    .span(Span::styled("entropy ", st.muted))
                    .span(Span::styled("0.998", st.text))
                    .span(Span::styled(" │ rollback ", st.muted))
                    .span(Span::styled("armed", st.error)),
            );
    } else if !narrow {
        body = body.line(
            Line::new()
                .span(Span::styled("banks ", st.muted))
                .span(Span::styled("04/07 ONLINE", st.success))
                .span(Span::styled(" │ da-vinci ", st.muted))
                .span(Span::styled("SCANNING", st.warning)),
        );
    }

    Node::col()
        .percent_width(100.0)
        .child(header)
        .child(Node::rail(st.rail).child(Node::rich_text(body)))
}

// ---------------------------------------------------------------------------
// ACT III — FILE TRANSFER
// ---------------------------------------------------------------------------

fn act_iii_transfer(
    ctx: &mut Context,
    _theme: &Theme,
    st: &gibson::ThemeStyles,
    auto: bool,
    started: Instant,
) -> Result<(), Box<dyn std::error::Error>> {
    let total_mb = 256usize;
    let steps = if auto { 5 } else { 20 };
    for i in 0..=steps {
        let pct = (i * 100) / steps;
        let mb = (pct * total_mb) / 100;
        let throughput = 28.8 + (i as f64) * 0.37;
        let frame = animation_frame(started, ctx.animation_interval());
        let (cols, _) = ctx.session.terminal_size();
        let meter_width = (cols as usize).saturating_sub(14).clamp(8, 48);

        let root = Node::col().percent_width(100.0).child(
            Node::rail(st.rail).child(Node::rich_text(
                RichText::new()
                    .line(
                        Line::new()
                            .span(Span::styled("garbage.bin ", st.code))
                            .span(Span::styled(format!("{} / {} MB", mb, total_mb), st.text))
                            .span(Span::styled("  ", st.text))
                            .span(Span::styled(meter(pct, meter_width), st.accent)),
                    )
                    .line(
                        Line::new()
                            .span(Span::styled("throughput ", st.muted))
                            .span(Span::styled(format!("{:.1} MB/s", throughput), st.success))
                            .span(Span::styled(" │ checksum ", st.muted))
                            .span(Span::styled("sha256:e3b0…b855", st.code)),
                    ),
            )),
        );
        ctx.set_root(root);
        let _ = frame;
        tick(ctx, 1)?;

        if i == steps / 2 {
            ctx.insert_rich_text_before_live(
                &RichText::new().line(Line::new().span(Span::styled("[net] ", st.muted)).span(
                    Span::styled("rerouting through a cut fiber, 3ms latency", st.warning),
                )),
            )?;
        }
    }

    ctx.clear_live_region()?;
    ctx.commit_rich_text(
        &RichText::new().line(
            Line::new()
                .span(Span::styled("✔ ", st.success))
                .span(Span::styled(
                    "garbage file secured — 256 MB, checksum verified",
                    st.text,
                )),
        ),
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// ACT IV — TACTICAL PAYLOAD SELECTOR
// ---------------------------------------------------------------------------

fn act_iv_payload(
    ctx: &mut Context,
    _theme: &Theme,
    st: &gibson::ThemeStyles,
    auto: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut selected = 0usize;

    if auto {
        selected = 3;
        let root = build_selector(selected, st);
        ctx.set_root(root);
        tick(ctx, 1)?;
    } else {
        loop {
            let root = build_selector(selected, st);
            ctx.set_root(root);
            if let Some(Event::Key(k)) = ctx.run_once(Duration::from_millis(100))? {
                match k.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        selected = selected.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        selected = (selected + 1).min(TACTICAL_OPTIONS.len() - 1);
                    }
                    KeyCode::Enter => break,
                    KeyCode::Esc | KeyCode::Char('q') => break,
                    _ => {}
                }
            }
        }
    }

    ctx.clear_live_region()?;
    ctx.commit_rich_text(
        &RichText::new()
            .line(
                Line::new()
                    .span(Span::styled("[TACTICAL DIRECTIVE] ", st.warning))
                    .span(Span::styled(TACTICAL_OPTIONS[selected], st.text)),
            )
            .line(Line::new().span(Span::styled(
                "\"HACK THE PLANET! HACK THE PLANET!\"",
                st.accent,
            ))),
    )?;
    Ok(())
}

fn build_selector(selected: usize, st: &gibson::ThemeStyles) -> Node {
    let mut body = RichText::new().line(
        Line::new()
            .span(Span::styled("SELECT PAYLOAD ", st.accent))
            .span(Span::styled(
                "(↑/↓ or j/k, Enter to fire, q to abort)",
                st.muted,
            )),
    );

    for (i, opt) in TACTICAL_OPTIONS.iter().enumerate() {
        let (cursor, style) = if i == selected {
            ("▶ ", st.warning)
        } else {
            ("  ", st.muted)
        };
        body = body.line(
            Line::new()
                .span(Span::styled(cursor, st.accent))
                .span(Span::styled(format!("[{:02}] ", i + 1), st.muted))
                .span(Span::styled(*opt, style)),
        );
    }

    Node::col()
        .percent_width(100.0)
        .child(Node::rule(Some("TACTICAL PAYLOAD".to_string()), Style::new()).percent_width(100.0))
        .child(Node::rail(st.rail).child(Node::rich_text(body)))
}

// ---------------------------------------------------------------------------
// ACT V — ROOT@GIBSON SHELL
// ---------------------------------------------------------------------------

fn act_v_shell(
    ctx: &mut Context,
    _theme: &Theme,
    st: &gibson::ThemeStyles,
    auto: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    ctx.commit_rich_text(&RichText::new().line(
        Line::new()
            .span(Span::styled("root@gibson", st.success))
            .span(Span::styled(":~# ", st.muted))
            .span(Span::styled(
                "shell online — try help, status, pool, garbage, da-vinci, trace, metrics, exit",
                st.muted,
            )),
    ))?;

    let mut input = TextInputState::new();

    if auto {
        for cmd in ["status", "trace", "pool", "da-vinci", "metrics", "exit"] {
            input = TextInputState::with_text(cmd);
            let root = build_shell(ctx, &input, st);
            ctx.set_root(root);
            tick(ctx, 1)?;
            run_command(ctx, cmd, &input, st)?;
        }
        return Ok(());
    }

    loop {
        let root = build_shell(ctx, &input, st);
        ctx.set_root(root);
        if let Some(event) = ctx.run_once(Duration::from_millis(60))? {
            match event {
                Event::Key(k) => match k.code {
                    KeyCode::Enter => {
                        let cmd = input.text.trim().to_lowercase();
                        ctx.commit_rich_text(
                            &RichText::new().line(
                                Line::new()
                                    .span(Span::styled("root@gibson", st.success))
                                    .span(Span::styled(":~# ", st.muted))
                                    .span(Span::styled(input.text.clone(), st.text)),
                            ),
                        )?;
                        if cmd == "exit" || cmd == "quit" {
                            break;
                        }
                        run_command(ctx, &cmd, &input, st)?;
                        input = TextInputState::new();
                    }
                    KeyCode::Esc => break,
                    _ => {
                        if input.handle_event(&event) {
                            ctx.request_render();
                        }
                    }
                },
                Event::Paste(_) => {
                    if input.handle_event(&event) {
                        ctx.request_render();
                    }
                }
                Event::Resize(_, _) => ctx.request_render(),
                Event::Tick => {}
            }
        }
    }
    Ok(())
}

fn build_shell(ctx: &Context, input: &TextInputState, st: &gibson::ThemeStyles) -> Node {
    let (cols, _) = ctx.session.terminal_size();
    let prompt = if cols < 30 { "❯" } else { "root@gibson:~#" };
    Node::col().percent_width(100.0).child(
        Node::row()
            .gap(1.0)
            .child(Node::text(prompt, st.accent))
            .child(
                Node::text_input(
                    &input.text,
                    input.cursor_grapheme,
                    Some("type a directive…"),
                    st.text,
                )
                .scroll_offset(input.scroll_offset)
                .flex_grow(1.0),
            ),
    )
}

fn run_command(
    ctx: &mut Context,
    cmd: &str,
    input: &TextInputState,
    st: &gibson::ThemeStyles,
) -> Result<(), Box<dyn std::error::Error>> {
    let _ = input;
    match cmd {
        "" => {}
        "exit" | "quit" => {
            ctx.commit_rich_text(&RichText::new().line(Line::new().span(Span::styled(
                "Connection severed by foreign host. Skate fast.",
                st.error,
            ))))?;
        }
        "help" => {
            ctx.commit_rich_text(&RichText::new().line(Line::new().span(Span::styled(
                "commands: help status pool garbage da-vinci trace metrics exit",
                st.muted,
            ))))?;
        }
        "pool" => {
            ctx.commit_rich_text(
                &RichText::new()
                    .line(Line::new().span(Span::styled(
                        "There is no pool on the roof of Ellingson Mineral!",
                        st.accent,
                    )))
                    .line(Line::new().span(Span::styled(
                        "…sprinkler override initiated. Water pressure critical.",
                        st.warning,
                    ))),
            )?;
        }
        "garbage" => {
            ctx.commit_rich_text(
                &RichText::new().line(
                    Line::new()
                        .span(Span::styled("garbage.bin ", st.code))
                        .span(Span::styled(
                            "located at /usr/spool/garbage (256 MB)",
                            st.text,
                        )),
                ),
            )?;
        }
        "da-vinci" => {
            ctx.commit_rich_text(
                &RichText::new().line(
                    Line::new()
                        .span(Span::styled("da-vinci ", st.warning))
                        .span(Span::styled(
                            "worm neutralized; $25,000,000 siphon halted",
                            st.text,
                        )),
                ),
            )?;
        }
        "trace" => {
            ctx.insert_rich_text_before_live(
                &RichText::new().line(
                    Line::new()
                        .span(Span::styled("[trace] ", st.muted))
                        .span(Span::styled("route 66 → gibson-core → zero-cool", st.code)),
                ),
            )?;
        }
        "status" => {
            let s = ctx.stats();
            ctx.commit_rich_text(
                &RichText::new()
                    .line(
                        Line::new()
                            .span(Span::styled("status ", st.accent))
                            .span(Span::styled("frames ", st.muted))
                            .span(Span::styled(s.frames.to_string(), st.text))
                            .span(Span::styled(" │ dirty ", st.muted))
                            .span(Span::styled(s.dirty_cells.to_string(), st.text))
                            .span(Span::styled(" │ anchor resyncs ", st.muted))
                            .span(Span::styled(s.anchor_resyncs.to_string(), st.text)),
                    )
                    .line(
                        Line::new()
                            .span(Span::styled("history insertions ", st.muted))
                            .span(Span::styled(s.history_insertions.to_string(), st.text))
                            .span(Span::styled(" (fast ", st.muted))
                            .span(Span::styled(s.fast_insertions.to_string(), st.success))
                            .span(Span::styled(" / fallback ", st.muted))
                            .span(Span::styled(s.insertion_repaints.to_string(), st.warning))
                            .span(Span::styled(")", st.muted)),
                    ),
            )?;
        }
        "metrics" => {
            let s = ctx.stats();
            ctx.commit_rich_text(
                &RichText::new()
                    .line(
                        Line::new()
                            .span(Span::styled("frame_bytes ", st.muted))
                            .span(Span::styled(s.frame_bytes.to_string(), st.text))
                            .span(Span::styled(" │ commit_bytes ", st.muted))
                            .span(Span::styled(s.commit_bytes.to_string(), st.text))
                            .span(Span::styled(" │ insertion_bytes ", st.muted))
                            .span(Span::styled(s.insertion_bytes.to_string(), st.text)),
                    )
                    .line(Line::new().span(Span::styled(
                        format!("total_terminal_bytes {}", s.total_terminal_bytes()),
                        st.accent,
                    ))),
            )?;
        }
        other => {
            ctx.commit_rich_text(&RichText::new().line(Line::new().span(Span::styled(
                format!("bash: {}: command not found in /usr/local/bin", other),
                st.error,
            ))))?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// ACT VI — EXIT / TELEMETRY
// ---------------------------------------------------------------------------

fn act_vi_telemetry(
    ctx: &mut Context,
    _theme: &Theme,
    st: &gibson::ThemeStyles,
) -> Result<(), Box<dyn std::error::Error>> {
    let s = ctx.stats();
    ctx.commit_rich_text(
        &RichText::new()
            .line(
                Line::new()
                    .span(Span::styled("╰─ ", st.muted))
                    .span(Span::styled("engine telemetry ", st.accent))
                    .span(Span::styled("(measured, not scripted)", st.muted)),
            )
            .line(
                Line::new()
                    .span(Span::styled("frames ", st.muted))
                    .span(Span::styled(s.frames.to_string(), st.text))
                    .span(Span::styled(" │ full repaints ", st.muted))
                    .span(Span::styled(s.full_repaints.to_string(), st.text))
                    .span(Span::styled(" │ anchor resyncs ", st.muted))
                    .span(Span::styled(s.anchor_resyncs.to_string(), st.text))
                    .span(Span::styled(" │ skipped ", st.muted))
                    .span(Span::styled(s.skipped_frames.to_string(), st.text)),
            )
            .line(
                Line::new()
                    .span(Span::styled("frame ", st.muted))
                    .span(Span::styled(format!("{} B", s.frame_bytes), st.text))
                    .span(Span::styled(" │ commit ", st.muted))
                    .span(Span::styled(format!("{} B", s.commit_bytes), st.text))
                    .span(Span::styled(" │ insertion ", st.muted))
                    .span(Span::styled(format!("{} B", s.insertion_bytes), st.text))
                    .span(Span::styled(" │ total ", st.muted))
                    .span(Span::styled(
                        format!("{} B", s.total_terminal_bytes()),
                        st.accent,
                    )),
            ),
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Drives `frames` scheduler-bounded animation steps.
///
/// Input always has priority: `run_once` waits at most until the frame deadline
/// and returns immediately when an event arrives.
fn tick(ctx: &mut Context, frames: usize) -> Result<(), Box<dyn std::error::Error>> {
    for _ in 0..frames {
        ctx.request_render();
        let interval = ctx.animation_interval();
        ctx.run_once(interval)?;
    }
    Ok(())
}

fn animation_frame(started: Instant, interval: Duration) -> usize {
    let ms = interval.as_millis().max(1);
    (started.elapsed().as_millis() / ms) as usize
}
