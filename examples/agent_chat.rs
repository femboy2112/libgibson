use gibson::cell::{Color, Style};
use gibson::context::Context;
use gibson::input::{Event, KeyCode, TextInputState};
use gibson::node::{Node, WrapMode};
use gibson::BorderType;
use std::env;
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let auto_mode = args.iter().any(|a| a == "--auto" || a == "--headless");

    let mut ctx = Context::inline()?;
    ctx.set_max_fps(60);

    // =========================================================================
    // STEP 1: Print already committed immutable transcript
    // =========================================================================
    ctx.commit("\x1b[1;34m● LibGibson Engine\x1b[0m session started.")?;
    ctx.commit("\x1b[90m[system] Initialized live-region cell framebuffer with Taffy flexbox layout.\x1b[0m")?;
    ctx.commit("\x1b[1;37mUser:\x1b[0m Run test suite and check code quality.")?;

    if !auto_mode {
        sleep(Duration::from_millis(500));
    }

    // =========================================================================
    // STEP 2 & 3 & 4: Live spinner & streaming text into live assistant block
    // =========================================================================
    let tokens = [
        "Analyzing",
        " codebase",
        " architecture...",
        "\n",
        "Found",
        " 28 unit tests",
        " covering",
        " grapheme clusters,",
        " Taffy flex layout,",
        " differential ANSI compiler,",
        " and immutable-scrollback commit semantics.\n",
        "Ready to execute test runner.",
    ];

    let mut accumulated_text = String::new();
    let mut spinner_frame = 0;

    for token in tokens {
        accumulated_text.push_str(token);
        spinner_frame += 1;

        let root = Node::col()
            .gap(1.0)
            .child(
                Node::row().gap(1.0).child(
                    Node::spinner(
                        spinner_frame,
                        Style::new().fg(Color::BrightCyan),
                        Some("Synthesizing response..."),
                    )
                    .flex_grow(1.0),
                ),
            )
            .child(
                Node::border_box(BorderType::Rounded, Style::new().fg(Color::BrightBlack)).child(
                    Node::text_wrapped(
                        &accumulated_text,
                        Style::new().fg(Color::White),
                        WrapMode::WordWrap,
                    ),
                ),
            );

        ctx.set_root(root);
        ctx.render()?;

        if !auto_mode {
            sleep(Duration::from_millis(100));
        }
    }

    if !auto_mode {
        sleep(Duration::from_millis(300));
    }

    // =========================================================================
    // STEP 5: Commit the completed assistant response into immutable scrollback
    // =========================================================================
    let committed_response = format!("\x1b[1;32mAssistant:\x1b[0m\n{}", accumulated_text);
    ctx.commit(&committed_response)?;

    // =========================================================================
    // STEP 6 & 7: Permission selector in live region
    // =========================================================================
    let options = ["Yes, once", "Yes, for this session", "No"];
    let mut selected_idx: usize = 0;

    if auto_mode {
        // Render selector once in auto mode
        let root = build_selector_node(selected_idx, &options);
        ctx.set_root(root);
        ctx.render()?;
    } else {
        // Interactive keyboard loop for permission selector
        loop {
            let root = build_selector_node(selected_idx, &options);
            ctx.set_root(root);
            ctx.render()?;

            if let Some(Event::Key(k)) = ctx.poll_event(Duration::from_millis(50))? {
                match k.code {
                    KeyCode::Up => {
                        selected_idx = selected_idx.saturating_sub(1);
                    }
                    KeyCode::Down if selected_idx + 1 < options.len() => {
                        selected_idx += 1;
                    }
                    KeyCode::Enter | KeyCode::Char('q') | KeyCode::Esc => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    // =========================================================================
    // STEP 8 & 9: Commit the decision and transition cleanly
    // =========================================================================
    let decision_text = format!(
        "\x1b[90mPermission granted: {}\x1b[0m",
        options[selected_idx]
    );
    ctx.commit(&decision_text)?;

    // =========================================================================
    // STEP 10: Multiline / interactive prompt with TextInput
    // =========================================================================
    let mut input_state = TextInputState::new();

    if auto_mode {
        input_state.insert_str("cargo test --lib");
        let prompt_node = build_prompt_node(&input_state);
        ctx.set_root(prompt_node);
        ctx.render()?;
    } else {
        loop {
            let prompt_node = build_prompt_node(&input_state);
            ctx.set_root(prompt_node);
            ctx.render()?;

            if let Some(event) = ctx.poll_event(Duration::from_millis(50))? {
                match event {
                    Event::Key(k) => match k.code {
                        KeyCode::Enter => {
                            break;
                        }
                        KeyCode::Esc => {
                            break;
                        }
                        _ => {
                            input_state.handle_event(&event);
                        }
                    },
                    Event::Paste(_) => {
                        input_state.handle_event(&event);
                    }
                    _ => {}
                }
            }
        }
    }

    // Final commit of input
    let final_text = format!("\x1b[1;34m❯\x1b[0m {}", input_state.text);
    ctx.commit(&final_text)?;

    // Report renderer statistics
    let stats = ctx.stats();
    let stats_line = format!(
        "\x1b[90m[metrics] frames: {}, dirty_cells: {}, total_bytes_emitted: {}, repaints: {}\x1b[0m",
        stats.frames, stats.dirty_cells, stats.bytes_emitted, stats.full_repaints
    );
    ctx.commit(&stats_line)?;

    ctx.restore()?;
    Ok(())
}

fn build_selector_node(selected_idx: usize, options: &[&str]) -> Node {
    let mut col = Node::col()
        .gap(0.0)
        .child(Node::text(
            "Allow command?",
            Style::new().bold().fg(Color::Yellow),
        ))
        .child(Node::text(
            "  $ cargo test",
            Style::new().fg(Color::BrightCyan),
        ))
        .child(Node::text("", Style::default()));

    for (i, &opt) in options.iter().enumerate() {
        let (prefix, style) = if i == selected_idx {
            ("> ", Style::new().bold().fg(Color::Green))
        } else {
            ("  ", Style::new().fg(Color::White).dim())
        };

        col.add_child(Node::text(format!("{}{}", prefix, opt), style));
    }

    Node::border_box(BorderType::Rounded, Style::new().fg(Color::BrightBlack)).child(col)
}

fn build_prompt_node(input_state: &TextInputState) -> Node {
    Node::border_box(BorderType::Rounded, Style::new().fg(Color::Blue)).child(
        Node::row()
            .gap(1.0)
            .child(Node::text("❯", Style::new().bold().fg(Color::BrightBlue)))
            .child(
                Node::text_input(
                    &input_state.text,
                    input_state.cursor_grapheme,
                    Some("Type instructions or 'exit'..."),
                    Style::new().fg(Color::White),
                )
                .flex_grow(1.0),
            ),
    )
}
