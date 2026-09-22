//! Polished Agent CLI Demo
//!
//! A clean-room, typography-driven agent CLI interface demonstrating:
//! - Typography hierarchy and restrained visual doctrine (native terminal background)
//! - Horizontal rule (`Node::rule`) and left-rail (`Node::rail`) chrome primitives
//! - Structured text layout (`RichText`, `Line`, `Span`, `Theme`)
//! - Live animated spinner in the mutable live region
//! - Live scrollback insertion (`insert_before_live`) without disrupting active prompt
//! - Interactive keyboard permission selector
//! - Wide-character and emoji display-width aware text input (`TextInputState`)
//! - Zero raw SGR escape sequences in user code
//! - Clean non-TTY redirection with zero escape sequences

use gibson::cell::{Color, Line, RichText, Span, Style, Theme};
use gibson::context::Context;
use gibson::input::{Event, KeyCode, KeyEvent, KeyModifiers, TextInputState};
use gibson::node::{Node, WrapMode};
use std::env;
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let auto_mode = args.iter().any(|a| a == "--auto" || a == "--headless");

    let theme = Theme::default();
    let mut ctx = Context::inline()?;
    ctx.set_max_fps(60);

    // =========================================================================
    // 1. Session Header (Committed to Immutable Scrollback)
    // =========================================================================
    let mut header = Node::col()
        .percent_width(100.0)
        .max_width(78.0)
        .gap(0.0)
        .child(
            Node::rule(
                Some("LibGibson Agent v0.2.0"),
                Style::new().bold().fg(theme.accent),
            )
            .percent_width(100.0),
        )
        .child(
            Node::rail(Style::new().fg(theme.rail)).child(Node::rich_text(
                RichText::new()
                    .line(
                        Line::new()
                            .span(Span::styled("Session: ", Style::new().fg(theme.text_muted)))
                            .span(Span::styled("agy-autonomous-slice-88", Style::new().bold()))
                            .span(Span::styled(
                                " │ Model: ",
                                Style::new().fg(theme.text_muted),
                            ))
                            .span(Span::styled(
                                "gibson-kernel-v2",
                                Style::new().fg(theme.accent),
                            ))
                            .span(Span::styled(" │ Mode: ", Style::new().fg(theme.text_muted)))
                            .span(Span::styled("differential", Style::new().fg(theme.success))),
                    )
                    .line(
                        Line::new()
                            .span(Span::styled(
                                "Workspace: ",
                                Style::new().fg(theme.text_muted),
                            ))
                            .span(Span::styled("/home/leah/LibGibson", Style::new().dim())),
                    ),
            )),
        );

    ctx.commit_node(&mut header)?;

    if !auto_mode {
        sleep(Duration::from_millis(400));
    }

    // =========================================================================
    // 2. User Prompt (Committed to Scrollback)
    // =========================================================================
    let user_prompt = RichText::new().line(
        Line::new()
            .span(Span::styled("❯ ", Style::new().bold().fg(theme.accent)))
            .span(Span::styled(
                "Audit terminal diff engine for atomic synchronized updates 🚀",
                Style::new().bold(),
            )),
    );
    ctx.commit_rich_text(&user_prompt)?;

    if !auto_mode {
        sleep(Duration::from_millis(300));
    }

    // =========================================================================
    // 3. Live Tool Execution with Animated Spinner & Scrollback Insertion
    // =========================================================================
    let analysis_steps = [
        "Analyzing terminal rendering pipeline...",
        "Scanning ANSI compiler for DEC private modes...",
        "Evaluating frame budget scheduler at 60 FPS...",
        "Synthesizing atomic differential patch...",
    ];

    let mut spinner_frame = 0usize;
    for (step_idx, step_desc) in analysis_steps.iter().enumerate() {
        let iterations = if auto_mode { 2 } else { 8 };
        for _ in 0..iterations {
            spinner_frame = spinner_frame.wrapping_add(1);

            let live_root = Node::col()
                .percent_width(100.0)
                .max_width(78.0)
                .gap(0.0)
                .child(
                    Node::row().child(
                        Node::spinner(
                            spinner_frame,
                            Style::new().bold().fg(theme.accent),
                            Some(step_desc),
                        )
                        .percent_width(100.0),
                    ),
                )
                .child(
                    Node::rail(Style::new().fg(theme.rail)).child(Node::text_wrapped(
                        "Inspecting src/ansi.rs: checking DECSM/DECRM 2026 support for flicker-free atomic sync.",
                        Style::new().fg(theme.text_muted),
                        WrapMode::WordWrap,
                    )),
                );

            ctx.set_root(live_root);
            ctx.render()?;

            if !auto_mode {
                sleep(Duration::from_millis(60));
            }
        }

        // Mid-execution: demonstrate insert_before_live!
        // An asynchronous background event arrives and is written to scrollback ABOVE the live spinner.
        if step_idx == 1 {
            let mut async_notice =
                Node::rail(Style::new().fg(theme.text_muted)).child(Node::rich_text(
                    RichText::new().line(
                        Line::new()
                            .span(Span::styled(
                                "[git-monitor] ",
                                Style::new().fg(theme.text_muted).dim(),
                            ))
                            .span(Span::styled("Tree is clean on branch ", Style::new().dim()))
                            .span(Span::styled(
                                "agy/next-level-terminal-ui",
                                Style::new().bold().dim(),
                            )),
                    ),
                ));
            ctx.insert_node_before_live(&mut async_notice)?;
        }
    }

    // =========================================================================
    // 4. Tool Result Committed to Scrollback
    // =========================================================================
    let mut tool_result = Node::col()
        .percent_width(100.0)
        .max_width(78.0)
        .gap(0.0)
        .child(
            Node::rule(Some("Tool: AST Code Search"), Style::new().fg(theme.rail))
                .percent_width(100.0),
        )
        .child(
            Node::rail(Style::new().fg(theme.rail)).child(Node::rich_text(
                RichText::new()
                    .line(
                        Line::new()
                            .span(Span::styled("● Query: ", Style::new().fg(theme.text_muted)))
                            .span(Span::styled(
                                "sync_update in src/ansi.rs",
                                Style::new().bold(),
                            )),
                    )
                    .line(
                        Line::new()
                            .span(Span::styled(
                                "● Result: ",
                                Style::new().fg(theme.text_muted),
                            ))
                            .span(Span::styled(
                                "Found DECSM 2026 atomic batching; 0 tear frames detected.",
                                Style::new().fg(theme.success),
                            )),
                    ),
            )),
        );

    ctx.commit_node(&mut tool_result)?;

    if !auto_mode {
        sleep(Duration::from_millis(300));
    }

    // =========================================================================
    // 5. Interactive Permission Selector in Live Region
    // =========================================================================
    let options = [
        (
            "Approve patch once",
            "Applies atomic sync patch to src/ansi.rs",
        ),
        (
            "Approve all for session",
            "Grants session-level write permission",
        ),
        ("Reject operation", "Aborts patch without modifying disk"),
    ];

    let mut selected_idx: usize = 0;
    let permission_granted = if auto_mode {
        // In auto/headless mode, render selector once and accept option 0
        let selector_node = build_permission_ui(&theme, &options, selected_idx);
        ctx.set_root(selector_node);
        ctx.render()?;
        true
    } else {
        // Interactive event loop
        loop {
            let selector_node = build_permission_ui(&theme, &options, selected_idx);
            ctx.set_root(selector_node);
            ctx.render()?;

            if let Some(Event::Key(key)) = ctx.poll_event(Duration::from_millis(100))? {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if selected_idx > 0 {
                            selected_idx -= 1;
                        } else {
                            selected_idx = options.len() - 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if selected_idx + 1 < options.len() {
                            selected_idx += 1;
                        } else {
                            selected_idx = 0;
                        }
                    }
                    KeyCode::Char('1') => {
                        selected_idx = 0;
                        break true;
                    }
                    KeyCode::Char('2') => {
                        selected_idx = 1;
                        break true;
                    }
                    KeyCode::Char('3') | KeyCode::Esc => {
                        selected_idx = 2;
                        break false;
                    }
                    KeyCode::Enter => {
                        break selected_idx < 2;
                    }
                    _ => {}
                }
            }
        }
    };

    // Clear live region and commit decision to scrollback
    ctx.clear_live_region()?;

    let decision_line = if permission_granted {
        RichText::new().line(
            Line::new()
                .span(Span::styled("✔ ", Style::new().bold().fg(theme.success)))
                .span(Span::styled(
                    format!(
                        "Action approved ({}): Applied patch to src/ansi.rs (+18, -2)",
                        options[selected_idx].0
                    ),
                    Style::new().bold(),
                )),
        )
    } else {
        RichText::new().line(
            Line::new()
                .span(Span::styled("✖ ", Style::new().bold().fg(theme.error)))
                .span(Span::styled(
                    "Action rejected by operator.",
                    Style::new().bold(),
                )),
        )
    };
    ctx.commit_rich_text(&decision_line)?;

    if !auto_mode {
        sleep(Duration::from_millis(400));
    }

    // =========================================================================
    // 6. Display-Width Aware Interactive Text Input Prompt
    // =========================================================================
    let mut input_state = TextInputState::new();

    if auto_mode {
        // Simulate typing wide characters, emoji, and navigating cursor
        let simulated_inputs = ["Ready for next command 🚀 [日本語]"];
        for text in simulated_inputs {
            for ch in text.chars() {
                input_state.handle_event(&Event::Key(KeyEvent::new(
                    KeyCode::Char(ch),
                    KeyModifiers::empty(),
                )));
            }
        }

        let input_node = build_prompt_ui(&theme, &input_state);
        ctx.set_root(input_node);
        ctx.render()?;

        // Commit entered text to scrollback
        let prompt_commit = RichText::new().line(
            Line::new()
                .span(Span::styled("❯ ", Style::new().bold().fg(theme.accent)))
                .span(Span::styled(&input_state.text, Style::new().bold())),
        );
        ctx.commit_rich_text(&prompt_commit)?;
    } else {
        // Interactive live prompt
        loop {
            let prompt_node = build_prompt_ui(&theme, &input_state);
            ctx.set_root(prompt_node);
            ctx.render()?;

            if let Some(event) = ctx.poll_event(Duration::from_millis(50))? {
                match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => {
                        let final_text = input_state.text.clone();
                        if !final_text.trim().is_empty() {
                            let prompt_commit = RichText::new().line(
                                Line::new()
                                    .span(Span::styled("❯ ", Style::new().bold().fg(theme.accent)))
                                    .span(Span::styled(&final_text, Style::new().bold())),
                            );
                            ctx.commit_rich_text(&prompt_commit)?;
                        }
                        break;
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Esc, ..
                    }) => {
                        ctx.clear_live_region()?;
                        break;
                    }
                    _ => {
                        input_state.handle_event(&event);
                    }
                }
            }
        }
    }

    // =========================================================================
    // 7. Engine Statistics Summary (Committed to Scrollback)
    // =========================================================================
    let stats = ctx.stats();
    let efficiency = if stats.total_cells > 0 {
        100.0 - ((stats.dirty_cells as f64 / stats.total_cells as f64) * 100.0)
    } else {
        100.0
    };

    let mut summary = Node::col()
        .percent_width(100.0)
        .max_width(78.0)
        .gap(0.0)
        .child(
            Node::rule(
                Some("LibGibson Engine Metrics"),
                Style::new().bold().fg(theme.success),
            )
            .percent_width(100.0),
        )
        .child(
            Node::rail(Style::new().fg(theme.rail)).child(Node::rich_text(
                RichText::new()
                    .line(
                        Line::new()
                            .span(Span::styled(
                                "Frames Rendered: ",
                                Style::new().fg(theme.text_muted),
                            ))
                            .span(Span::styled(
                                format!("{}", stats.frames),
                                Style::new().bold(),
                            ))
                            .span(Span::styled(
                                " │ Skipped: ",
                                Style::new().fg(theme.text_muted),
                            ))
                            .span(Span::styled(
                                format!("{}", stats.skipped_frames),
                                Style::new().dim(),
                            ))
                            .span(Span::styled(
                                " │ Full Repaints: ",
                                Style::new().fg(theme.text_muted),
                            ))
                            .span(Span::styled(
                                format!("{}", stats.full_repaints),
                                Style::new().dim(),
                            )),
                    )
                    .line(
                        Line::new()
                            .span(Span::styled(
                                "Diff Efficiency: ",
                                Style::new().fg(theme.text_muted),
                            ))
                            .span(Span::styled(
                                format!("{:.1}% saved cells", efficiency),
                                Style::new().bold().fg(theme.accent),
                            ))
                            .span(Span::styled(" (", Style::new().fg(theme.text_muted)))
                            .span(Span::styled(
                                format!(
                                    "{} dirty / {} total",
                                    stats.dirty_cells, stats.total_cells
                                ),
                                Style::new().dim(),
                            ))
                            .span(Span::styled(")", Style::new().fg(theme.text_muted))),
                    )
                    .line(
                        Line::new()
                            .span(Span::styled(
                                "Total Wire Output: ",
                                Style::new().fg(theme.text_muted),
                            ))
                            .span(Span::styled(
                                format!("{} bytes", stats.bytes_emitted),
                                Style::new().bold(),
                            )),
                    ),
            )),
        );

    ctx.commit_node(&mut summary)?;

    Ok(())
}

fn build_permission_ui(theme: &Theme, options: &[(&str, &str)], selected_idx: usize) -> Node {
    let mut col = Node::col()
        .percent_width(100.0)
        .max_width(78.0)
        .gap(0.0)
        .child(
            Node::rule(
                Some("Operator Confirmation"),
                Style::new().bold().fg(theme.warning),
            )
            .percent_width(100.0),
        );

    let mut rail_body = RichText::new().line(
        Line::new()
            .span(Span::styled(
                "Permission Request: ",
                Style::new().bold().fg(theme.warning),
            ))
            .span(Span::styled(
                "Apply atomic sync updates patch to compiler pipeline?",
                Style::new().bold(),
            )),
    );

    for (i, (label, detail)) in options.iter().enumerate() {
        let is_sel = i == selected_idx;
        let prefix = if is_sel { "  ❯ " } else { "    " };
        let mut line = Line::new().span(Span::styled(
            prefix,
            if is_sel {
                Style::new().bold().fg(theme.accent)
            } else {
                Style::new().dim()
            },
        ));

        line = line.span(Span::styled(
            format!("[{}] {}", i + 1, label),
            if is_sel {
                Style::new().bold().fg(Color::White)
            } else {
                Style::new().fg(theme.text_muted)
            },
        ));

        line = line.span(Span::styled(format!(" — {}", detail), Style::new().dim()));

        rail_body = rail_body.line(line);
    }

    col = col.child(Node::rail(Style::new().fg(theme.rail)).child(Node::rich_text(rail_body)));
    col
}

fn build_prompt_ui(theme: &Theme, input: &TextInputState) -> Node {
    Node::col()
        .percent_width(100.0)
        .max_width(78.0)
        .gap(0.0)
        .child(
            Node::row()
                .gap(1.0)
                .child(Node::text("❯", Style::new().bold().fg(theme.accent)))
                .child(
                    Node::text_input(
                        &input.text,
                        input.cursor_grapheme,
                        Some("Type instruction (Esc to exit)..."),
                        Style::new().fg(Color::White),
                    )
                    .percent_width(95.0),
                ),
        )
}
