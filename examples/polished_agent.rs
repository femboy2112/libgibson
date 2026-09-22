//! Polished Agent — the restrained flagship LibGibson showcase.
//!
//! A coherent mini agent session that demonstrates the engine's product-facing
//! qualities: hierarchy, sparse chrome, semantic color, stable live geometry,
//! precise animation, structured streaming text and excellent keyboard ergonomics.
//!
//! Visual doctrine
//! ---------------
//! * native terminal background (never paints a canvas)
//! * semantic `Theme` *style roles* rather than hardcoded colors
//! * no fixed composition width; responsive at 40 / 60 / 80 / 120 / 160 columns
//! * a stable-height live region so animation never jitters the layout
//!
//! Modes
//! -----
//! * `--auto` / `--scripted` — deterministic timeline for CI and snapshots
//! * `--no-color`, `--light`, `--dark` — theme proofs
//! * interactive by default, including Ctrl-C cancellation

use gibson::cell::{Line, RichText, Span, Theme};
use gibson::context::Context;
use gibson::input::{Event, KeyCode, KeyModifiers, TextInputState};
use gibson::node::{Node, WrapMode};
use gibson::painter::PaintContext;
use std::env;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Theme selection
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tool model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Queued,
    Running,
    Success,
    Warning,
    Error,
}

fn phase_marker(phase: Phase, st: &gibson::ThemeStyles) -> Span {
    match phase {
        Phase::Queued => Span::styled("· queued  ", st.faint),
        Phase::Running => Span::styled("◐ running ", st.accent),
        Phase::Success => Span::styled("✔ ok      ", st.success),
        Phase::Warning => Span::styled("▲ warn    ", st.warning),
        Phase::Error => Span::styled("✖ error   ", st.error),
    }
}

struct Tool {
    verb: &'static str,
    target: &'static str,
    phase: Phase,
    note: &'static str,
}

const TOOLS: &[Tool] = &[
    Tool {
        verb: "read",
        target: "src/renderer.rs",
        phase: Phase::Queued,
        note: "842 lines",
    },
    Tool {
        verb: "search",
        target: "insert_before_live",
        phase: Phase::Queued,
        note: "14 hits",
    },
    Tool {
        verb: "build",
        target: "cargo test",
        phase: Phase::Queued,
        note: "104 passed",
    },
    Tool {
        verb: "patch",
        target: "src/ansi.rs",
        phase: Phase::Queued,
        note: "+18 −2",
    },
];

const STREAM: &[&str] = &[
    "I audited the differential pipeline and the scrollback handoff. ",
    "The live region now tracks an explicit physical anchor, so a resize ",
    "forces a full re-anchor instead of trusting stale coordinates. ",
    "Insert-before-live is proven to leave the live framebuffer untouched ",
    "on the fast path, with a correct repaint fallback otherwise.",
];

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let auto = args
        .iter()
        .any(|a| a == "--auto" || a == "--scripted" || a == "--headless");
    let light = args.iter().any(|a| a == "--light");
    let dark = args.iter().any(|a| a == "--dark");
    let no_color = args.iter().any(|a| a == "--no-color");

    let theme = select_theme(light, dark, no_color);
    let st = theme.styles();

    let mut ctx = Context::inline()?;
    ctx.set_max_fps(if auto { 240 } else { 60 });
    ctx.set_animation_interval(Duration::from_millis(if auto { 8 } else { 90 }));

    let started = Instant::now();

    session_header(&mut ctx, &st)?;
    user_instruction(&mut ctx, &st)?;
    let cancelled = run_plan(&mut ctx, &st, auto, started)?;
    if !cancelled {
        let approved = run_permission(&mut ctx, &st, auto)?;
        patch_summary(&mut ctx, &st, approved)?;
        run_prompt(&mut ctx, &st, auto)?;
    } else {
        ctx.commit_rich_text(
            &RichText::new().line(Line::new().span(Span::styled("✖ ", st.error)).span(
                Span::styled("Operation cancelled by operator (Ctrl-C).", st.muted),
            )),
        )?;
    }
    telemetry(&mut ctx, &st, started)?;

    ctx.restore()?;
    Ok(())
}

fn session_header(
    ctx: &mut Context,
    st: &gibson::ThemeStyles,
) -> Result<(), Box<dyn std::error::Error>> {
    let (cols, _) = ctx.session.terminal_size();
    let wide = cols >= 120;

    let mut info = RichText::new().line(
        Line::new()
            .span(Span::styled("session ", st.muted))
            .span(Span::styled("libgibson-hardening", st.text))
            .span(Span::styled("  │  mode ", st.muted))
            .span(Span::styled("differential", st.success)),
    );
    if wide {
        info = info.line(
            Line::new()
                .span(Span::styled("workspace ", st.muted))
                .span(Span::styled("/home/leah/LibGibson", st.code))
                .span(Span::styled("  │  branch ", st.muted))
                .span(Span::styled("deepseek/next-level-terminal-ui", st.accent)),
        );
    }

    let mut header = Node::col()
        .percent_width(100.0)
        .child(Node::rule(Some("LibGibson Agent".to_string()), st.accent).percent_width(100.0))
        .child(Node::rail(st.rail).child(Node::rich_text(info)));
    ctx.commit_node(&mut header)?;
    Ok(())
}

fn user_instruction(
    ctx: &mut Context,
    st: &gibson::ThemeStyles,
) -> Result<(), Box<dyn std::error::Error>> {
    ctx.commit_rich_text(
        &RichText::new().line(
            Line::new()
                .span(Span::styled("❯ ", st.accent))
                .span(Span::styled(
                    "Harden the renderer invariants and make the demos exceptional 日本語 🚀",
                    st.text,
                )),
        ),
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Live planning + streaming
// ---------------------------------------------------------------------------

fn run_plan(
    ctx: &mut Context,
    st: &gibson::ThemeStyles,
    auto: bool,
    started: Instant,
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut tools: Vec<Tool> = TOOLS
        .iter()
        .map(|t| Tool {
            verb: t.verb,
            target: t.target,
            phase: t.phase,
            note: t.note,
        })
        .collect();

    let total_steps = tools.len();
    let step_frames = if auto { 3 } else { 12 };
    let mut stream_len = 0usize;
    let full_stream: String = STREAM.concat();

    for step in 0..total_steps {
        tools[step].phase = Phase::Running;
        // Earlier tool settles.
        if step > 0 {
            tools[step - 1].phase = match step - 1 {
                2 => Phase::Warning,
                _ => Phase::Success,
            };
        }

        for _ in 0..step_frames {
            let frame = animation_frame(started, ctx.animation_interval());
            stream_len = (stream_len + 6).min(full_stream.chars().count());
            let shown: String = full_stream.chars().take(stream_len).collect();
            let root = plan_root(ctx, &tools, step, frame, &shown, st);
            ctx.set_root(root);

            if let Some(event) = ctx.run_once(ctx.animation_interval())? {
                if is_cancel(&event) {
                    ctx.clear_live_region()?;
                    return Ok(true);
                }
            }
        }

        // Mid-run asynchronous event above the live region.
        if step == 1 {
            ctx.insert_rich_text_before_live(
                &RichText::new().line(
                    Line::new()
                        .span(Span::styled("[background] ", st.muted))
                        .span(Span::styled(
                            "diagnostics refreshed: 0 new warnings on this branch",
                            st.success,
                        )),
                ),
            )?;
        }
    }

    tools[total_steps - 1].phase = Phase::Error; // deliberately demonstrate an error state
                                                 // Render the terminal state once for the record.
    let frame = animation_frame(started, ctx.animation_interval());
    let shown: String = full_stream.chars().collect();
    let root = plan_root(ctx, &tools, total_steps - 1, frame, &shown, st);
    ctx.set_root(root);
    ctx.request_render();
    ctx.run_once(ctx.animation_interval())?;

    // Commit the tool summary + streamed assistant text into scrollback.
    ctx.clear_live_region()?;
    let mut committed = RichText::new();
    for t in &tools {
        committed = committed.line(
            Line::new()
                .span(phase_marker(t.phase, st))
                .span(Span::styled(format!("{} ", t.verb), st.code))
                .span(Span::styled(t.target, st.text))
                .span(Span::styled(format!("  ({})", t.note), st.muted)),
        );
    }
    committed = committed.line(
        Line::new()
            .span(Span::styled("  ", st.faint))
            .span(Span::styled(
                "note: the patch tool reports a conflict and needs review.",
                st.warning,
            )),
    );
    ctx.commit_rich_text(&committed)?;

    ctx.commit_rich_text(
        &RichText::new().line(Line::new().span(Span::styled(full_stream, st.muted))),
    )?;
    Ok(false)
}

fn plan_root(
    ctx: &Context,
    tools: &[Tool],
    current: usize,
    frame: usize,
    streamed: &str,
    st: &gibson::ThemeStyles,
) -> Node {
    let (cols, _) = ctx.session.terminal_size();
    let compact = cols < 60;

    let mut list = RichText::new();
    for t in tools.iter() {
        let line = if compact {
            Line::new()
                .span(phase_marker(t.phase, st))
                .span(Span::styled(t.verb, st.code))
        } else {
            Line::new()
                .span(phase_marker(t.phase, st))
                .span(Span::styled(format!("{} ", t.verb), st.code))
                .span(Span::styled(t.target, st.text))
                .span(Span::styled(format!("  {}", t.note), st.muted))
        };
        list = list.line(line);
    }

    let header = Node::row()
        .gap(1.0)
        .child(
            Node::spinner(
                frame,
                st.accent,
                Some(if compact { "working" } else { "Executing plan" }),
            )
            .flex_grow(1.0),
        )
        .child(Node::text(
            format!("{}/{}", current + 1, tools.len()),
            st.muted,
        ));

    // Fixed-height streaming area keeps the live geometry stable.
    let stream_height = if compact { 2.0 } else { 3.0 };

    Node::col()
        .percent_width(100.0)
        .child(header)
        .child(Node::rail(st.rail).child(Node::rich_text(list)))
        .child(
            Node::rail(st.border).child(
                Node::rich_text_wrapped(RichText::raw(streamed), WrapMode::WordWrap)
                    .height(stream_height),
            ),
        )
        .child(Node::text(
            if compact {
                ""
            } else {
                "Esc / Ctrl-C to cancel"
            },
            st.faint,
        ))
        .height(if compact { 7.0 } else { 8.0 })
}

// ---------------------------------------------------------------------------
// Permission selector
// ---------------------------------------------------------------------------

const OPTIONS: &[(&str, &str)] = &[
    ("Approve once", "apply this patch for this step"),
    ("Approve for session", "trust this class of change"),
    ("Reject", "abort without touching disk"),
];

fn run_permission(
    ctx: &mut Context,
    st: &gibson::ThemeStyles,
    auto: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut selected = 0usize;
    if auto {
        let root = permission_root(ctx, selected, st);
        ctx.set_root(root);
        ctx.request_render();
        ctx.run_once(ctx.animation_interval())?;
        return Ok(true);
    }

    loop {
        let root = permission_root(ctx, selected, st);
        ctx.set_root(root);
        if let Some(Event::Key(k)) = ctx.run_once(Duration::from_millis(100))? {
            match k.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    selected = if selected == 0 {
                        OPTIONS.len() - 1
                    } else {
                        selected - 1
                    };
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1) % OPTIONS.len();
                }
                KeyCode::Char(c @ '1'..='9') => {
                    let idx = (c as usize) - ('1' as usize);
                    if idx < OPTIONS.len() {
                        selected = idx;
                    }
                }
                KeyCode::Char('a') => {
                    break Ok(true);
                }
                KeyCode::Enter => break Ok(selected != OPTIONS.len() - 1),
                KeyCode::Esc => {
                    break Ok(false);
                }
                _ => {}
            }
        }
    }
}

fn permission_root(ctx: &Context, selected: usize, st: &gibson::ThemeStyles) -> Node {
    let (cols, _) = ctx.session.terminal_size();
    let compact = cols < 60;

    let mut body = RichText::new().line(
        Line::new()
            .span(Span::styled("Permission requested: ", st.warning))
            .span(Span::styled("apply patch to src/ansi.rs?", st.text)),
    );
    for (i, (label, detail)) in OPTIONS.iter().enumerate() {
        let (cursor, style) = if i == selected {
            ("❯ ", st.accent)
        } else {
            ("  ", st.faint)
        };
        let mut line = Line::new()
            .span(Span::styled(cursor, st.accent))
            .span(Span::styled(format!("[{}] ", i + 1), st.muted))
            .span(Span::styled(*label, style));
        if !compact {
            line = line.span(Span::styled(format!("  — {detail}"), st.muted));
        }
        body = body.line(line);
    }
    if !compact {
        body = body.line(Line::new().span(Span::styled(
            "↑/↓ or j/k • 1-3 • Enter • a = approve all • Esc = reject",
            st.faint,
        )));
    }

    Node::col()
        .percent_width(100.0)
        .child(
            Node::rule(Some("Operator confirmation".to_string()), st.warning).percent_width(100.0),
        )
        .child(Node::rail(st.rail).child(Node::rich_text(body)))
}

fn patch_summary(
    ctx: &mut Context,
    st: &gibson::ThemeStyles,
    approved: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut text = RichText::new().line(
        Line::new()
            .span(Span::styled(
                if approved { "✔ " } else { "✖ " },
                if approved { st.success } else { st.error },
            ))
            .span(Span::styled(
                if approved {
                    "Patch applied: src/ansi.rs (+18 −2)"
                } else {
                    "Patch rejected"
                },
                st.text,
            )),
    );
    if approved {
        text = text
            .line(
                Line::new()
                    .span(Span::styled("  + ", st.success))
                    .span(Span::styled("anchor invalidation on resize", st.text)),
            )
            .line(
                Line::new()
                    .span(Span::styled("  + ", st.success))
                    .span(Span::styled(
                        "InsertLineFastPath / RepaintFallback split",
                        st.text,
                    )),
            )
            .line(
                Line::new()
                    .span(Span::styled("  - ", st.error))
                    .span(Span::styled("implicit relative-cursor trust", st.text)),
            );
    }
    ctx.commit_rich_text(&text)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Persistent prompt: long mixed Unicode, background event while typing, Ctrl-C
// ---------------------------------------------------------------------------

fn run_prompt(
    ctx: &mut Context,
    st: &gibson::ThemeStyles,
    auto: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut input = TextInputState::new();
    let sample = "review the fast-path invariant 🦀 你好世界 e\u{0301} 🇺🇸 before merge";
    let mut bg_sent = false;
    let mut frames = 0usize;

    if auto {
        for ch in sample.chars() {
            input.insert_char(ch);
        }
        let root = prompt_root(ctx, &input, st);
        ctx.set_root(root);
        ctx.request_render();
        let _paint: PaintContext = ctx.render()?;
        ctx.insert_rich_text_before_live(
            &RichText::new().line(
                Line::new()
                    .span(Span::styled("[background] ", st.muted))
                    .span(Span::styled(
                        "index finished while you were typing",
                        st.success,
                    )),
            ),
        )?;
        ctx.commit_rich_text(
            &RichText::new().line(
                Line::new()
                    .span(Span::styled("❯ ", st.accent))
                    .span(Span::styled(sample, st.text)),
            ),
        )?;
        return Ok(());
    }

    loop {
        let root = prompt_root(ctx, &input, st);
        ctx.set_root(root);
        if let Some(event) = ctx.run_once(Duration::from_millis(60))? {
            if is_cancel(&event) {
                ctx.clear_live_region()?;
                ctx.commit_rich_text(
                    &RichText::new().line(
                        Line::new()
                            .span(Span::styled("✖ ", st.error))
                            .span(Span::styled("Input cancelled (Ctrl-C).", st.muted)),
                    ),
                )?;
                return Ok(());
            }
            match event {
                Event::Key(k) => match k.code {
                    KeyCode::Enter => {
                        let text = input.text.clone();
                        ctx.commit_rich_text(
                            &RichText::new().line(
                                Line::new()
                                    .span(Span::styled("❯ ", st.accent))
                                    .span(Span::styled(text, st.text)),
                            ),
                        )?;
                        return Ok(());
                    }
                    KeyCode::Esc => {
                        ctx.clear_live_region()?;
                        return Ok(());
                    }
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

        frames += 1;
        // A background event arrives WHILE the user is typing; the prompt stays stable.
        if !bg_sent && frames > 25 {
            ctx.insert_rich_text_before_live(
                &RichText::new().line(
                    Line::new()
                        .span(Span::styled("[background] ", st.muted))
                        .span(Span::styled(
                            "index finished while you were typing",
                            st.success,
                        )),
                ),
            )?;
            bg_sent = true;
        }
    }
}

fn prompt_root(_ctx: &Context, input: &TextInputState, st: &gibson::ThemeStyles) -> Node {
    Node::col().percent_width(100.0).child(
        Node::row()
            .gap(1.0)
            .child(Node::text("❯", st.accent))
            .child(
                Node::text_input(
                    &input.text,
                    input.cursor_grapheme,
                    Some("type a follow-up (Enter to send)…"),
                    st.text,
                )
                .scroll_offset(input.scroll_offset)
                .flex_grow(1.0),
            ),
    )
}

// ---------------------------------------------------------------------------
// Telemetry
// ---------------------------------------------------------------------------

fn telemetry(
    ctx: &mut Context,
    st: &gibson::ThemeStyles,
    started: Instant,
) -> Result<(), Box<dyn std::error::Error>> {
    let s = ctx.stats();
    let elapsed = started.elapsed();
    ctx.commit_rich_text(
        &RichText::new()
            .line(
                Line::new()
                    .span(Span::styled("session telemetry ", st.accent))
                    .span(Span::styled("(measured, not asserted)", st.muted)),
            )
            .line(
                Line::new()
                    .span(Span::styled("frames ", st.muted))
                    .span(Span::styled(s.frames.to_string(), st.text))
                    .span(Span::styled(" │ full repaints ", st.muted))
                    .span(Span::styled(s.full_repaints.to_string(), st.text))
                    .span(Span::styled(" │ anchor resyncs ", st.muted))
                    .span(Span::styled(s.anchor_resyncs.to_string(), st.text))
                    .span(Span::styled(" │ elapsed ", st.muted))
                    .span(Span::styled(format!("{:.0?}", elapsed), st.text)),
            )
            .line(
                Line::new()
                    .span(Span::styled("frame_bytes ", st.muted))
                    .span(Span::styled(s.frame_bytes.to_string(), st.text))
                    .span(Span::styled(" │ commit_bytes ", st.muted))
                    .span(Span::styled(s.commit_bytes.to_string(), st.text))
                    .span(Span::styled(" │ insertion ", st.muted))
                    .span(Span::styled(
                        format!(
                            "{} (fast {} / fallback {})",
                            s.insertion_bytes, s.fast_insertions, s.insertion_repaints
                        ),
                        st.text,
                    ))
                    .span(Span::styled(" │ total ", st.muted))
                    .span(Span::styled(
                        s.total_terminal_bytes().to_string(),
                        st.accent,
                    )),
            ),
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn is_cancel(event: &Event) -> bool {
    matches!(
        event,
        Event::Key(k) if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL)
    )
}

fn animation_frame(started: Instant, interval: Duration) -> usize {
    let ms = interval.as_millis().max(1);
    (started.elapsed().as_millis() / ms) as usize
}
