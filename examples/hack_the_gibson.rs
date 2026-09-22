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
    // ACT I: The Acoustic Coupler & Mainframe Handshake (Immutable Scrollback)
    // =========================================================================
    ctx.commit(
        "\x1b[1;36m╔══════════════════════════════════════════════════════════════════════╗\x1b[0m",
    )?;
    ctx.commit(
        "\x1b[1;36m║  LIBGIBSON // ELLINGSon MINERAL CORP // GIBSON MAINFRAME TERMINAL    ║\x1b[0m",
    )?;
    ctx.commit(
        "\x1b[1;36m╚══════════════════════════════════════════════════════════════════════╝\x1b[0m",
    )?;
    ctx.commit("\x1b[90m[modem] 28.8k baud acoustic coupler locked on (212) 555-0199...\x1b[0m")?;
    ctx.commit(
        "\x1b[1;33m\"Mess with the best, die like the rest.\"\x1b[0m \x1b[90m— Zero Cool\x1b[0m",
    )?;
    ctx.commit(
        "\x1b[32m✔ Connected to Gibson Supercomputer (OS: UNIX System V / LibGibson)\x1b[0m",
    )?;

    if !auto_mode {
        sleep(Duration::from_millis(600));
    }

    // =========================================================================
    // ACT II: Live Animated Mainframe Breach (Differential Framebuffer)
    // =========================================================================
    let hack_steps = [
        ("Bypassing Eugene 'The Plague' Belford's firewall...", 15),
        ("Searching memory banks for Olympic pool on the roof...", 35),
        (
            "Zero Cool & Acid Burn signatures detected in kernel space...",
            60,
        ),
        (
            "Isolating Da Vinci virus financial worm in /usr/spool/garbage...",
            85,
        ),
        ("Downloading garbage file to 3.5\" neon floppy disk...", 100),
    ];

    let mut current_frame = 0usize;
    for (message, progress) in hack_steps {
        let iterations = if auto_mode { 2 } else { 8 };
        for _ in 0..iterations {
            current_frame += 1;

            let root = build_hacking_dashboard(current_frame, message, progress);
            ctx.set_root(root);
            ctx.render()?;

            if !auto_mode {
                sleep(Duration::from_millis(50));
            }
        }
    }

    // =========================================================================
    // ACT III: Commit Breach Success into Immutable Scrollback
    // =========================================================================
    ctx.commit(
        "\x1b[1;32m★ ACCESS GRANTED: Root privileges obtained on the Gibson mainframe!\x1b[0m",
    )?;
    ctx.commit("\x1b[90m[telemetry] Garbage file hash: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\x1b[0m")?;

    if !auto_mode {
        sleep(Duration::from_millis(400));
    }

    // =========================================================================
    // ACT IV: Interactive Tactical Selector (Keyboard Driven)
    // =========================================================================
    let tactical_options = [
        "🌊 Override Olympic-sized swimming pool on roof",
        "🐛 Neutralize Da Vinci virus before tanker fleet capsizes",
        "🛹 Summon Acid Burn, Cereal Killer & Lord Nikon",
        "🕶  Rollerblade away before Agent Richard Gill arrives",
    ];

    let mut selected_idx: usize = 0;

    if auto_mode {
        let root = build_tactical_selector(selected_idx, &tactical_options);
        ctx.set_root(root);
        ctx.render()?;
    } else {
        loop {
            let root = build_tactical_selector(selected_idx, &tactical_options);
            ctx.set_root(root);
            ctx.render()?;

            if let Some(Event::Key(k)) = ctx.poll_event(Duration::from_millis(50))? {
                match k.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        selected_idx = selected_idx.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j')
                        if selected_idx + 1 < tactical_options.len() =>
                    {
                        selected_idx += 1;
                    }
                    KeyCode::Enter => {
                        break;
                    }
                    KeyCode::Esc | KeyCode::Char('q') => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    // Commit tactical decision to immutable scrollback
    let decision = format!(
        "\x1b[1;35m[TACTICAL DIRECTIVE EXECUTED]\x1b[0m {}\n\x1b[90m\"HACK THE PLANET! HACK THE PLANET!\"\x1b[0m",
        tactical_options[selected_idx]
    );
    ctx.commit(&decision)?;

    if !auto_mode {
        sleep(Duration::from_millis(400));
    }

    // =========================================================================
    // ACT V: Interactive Live Gibson Shell / REPL (Grapheme-aware TextInput)
    // =========================================================================
    ctx.commit(
        "\x1b[1;36mType commands below. Try: 'pool', 'status', 'matrix', 'help', or 'exit'.\x1b[0m",
    )?;

    let mut input_state = TextInputState::new();

    if auto_mode {
        input_state.insert_str("status");
        let shell_node = build_shell_prompt(&input_state);
        ctx.set_root(shell_node);
        ctx.render()?;

        let stats = ctx.stats();
        let report = format!(
            "\x1b[90m[engine-telemetry] Frames: {}, Dirty cells patched: {}, Total ANSI bytes: {}\x1b[0m",
            stats.frames, stats.dirty_cells, stats.bytes_emitted
        );
        ctx.commit(&report)?;
    } else {
        loop {
            let shell_node = build_shell_prompt(&input_state);
            ctx.set_root(shell_node);
            ctx.render()?;

            if let Some(event) = ctx.poll_event(Duration::from_millis(50))? {
                match event {
                    Event::Key(k) => match k.code {
                        KeyCode::Enter => {
                            let cmd = input_state.text.trim().to_lowercase();
                            let cmd_echo =
                                format!("\x1b[1;32mroot@gibson:~#\x1b[0m {}", input_state.text);
                            ctx.commit(&cmd_echo)?;

                            match cmd.as_str() {
                                "exit" | "quit" => {
                                    ctx.commit("\x1b[1;31mConnection severed by foreign host. Skate fast.\x1b[0m")?;
                                    break;
                                }
                                "pool" => {
                                    ctx.commit("\x1b[1;36m🌊 \"There is no pool on the roof of Ellingson Mineral!\"\x1b[0m\n\x1b[90m... Wait. Sprinkler override initiated! Water pressure critical! 💦\x1b[0m")?;
                                }
                                "matrix" => {
                                    ctx.commit("\x1b[32m01001000 01000001 01000011 01001011 00100000 01010100 01001000 01000101 00100000 01010000 01001100 01000001 01001110 01000101 01010100\x1b[0m")?;
                                }
                                "status" => {
                                    let s = ctx.stats();
                                    let stats_box = format!(
                                        "\x1b[1;33m[LibGibson Telemetry]\x1b[0m\n  • Frames rendered:   {}\n  • Coalesced frames:  {}\n  • Dirty cells diffed:{}\n  • Total bytes sent:  {} bytes\n  • Last frame latency:{} µs",
                                        s.frames, s.skipped_frames, s.dirty_cells, s.bytes_emitted, s.last_render_duration_micros
                                    );
                                    ctx.commit(&stats_box)?;
                                }
                                "help" => {
                                    ctx.commit("\x1b[90mAvailable directives: 'pool', 'matrix', 'status', 'help', 'exit'\x1b[0m")?;
                                }
                                "" => {}
                                other => {
                                    ctx.commit(&format!("\x1b[31mbash: {}: command not found in /usr/local/bin\x1b[0m", other))?;
                                }
                            }

                            input_state = TextInputState::new();
                        }
                        KeyCode::Esc => {
                            ctx.commit("\x1b[90mEscape signal received. Closing session.\x1b[0m")?;
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

    // Final clean restoration
    ctx.restore()?;
    println!("\x1b[1;32m✔ LibGibson session closed cleanly. Terminal restored.\x1b[0m");
    Ok(())
}

fn build_hacking_dashboard(frame: usize, message: &str, progress: usize) -> Node {
    let bar_width = 24;
    let filled = (progress * bar_width) / 100;
    let empty = bar_width.saturating_sub(filled);
    let meter = format!(
        "[{}{}] {:>3}%",
        "█".repeat(filled),
        "░".repeat(empty),
        progress
    );

    let header_row = Node::row()
        .gap(2.0)
        .child(
            Node::spinner(
                frame,
                Style::new().fg(Color::BrightCyan).bold(),
                Some("BREACH IN PROGRESS"),
            )
            .flex_grow(1.0),
        )
        .child(Node::text(
            "DEFCON 1",
            Style::new().fg(Color::BrightRed).bold().reverse(),
        ));

    let card = Node::border_box(BorderType::Rounded, Style::new().fg(Color::BrightCyan))
        .gap(1.0)
        .child(header_row)
        .child(Node::text_wrapped(
            message,
            Style::new().fg(Color::White).bold(),
            WrapMode::WordWrap,
        ))
        .child(Node::row().gap(1.0).child(Node::text(
            meter,
            Style::new().fg(Color::BrightGreen).bold(),
        )));

    Node::col().width(74.0).child(card)
}

fn build_tactical_selector(selected_idx: usize, options: &[&str]) -> Node {
    let mut list = Node::col().gap(0.0);

    for (i, &opt) in options.iter().enumerate() {
        let is_sel = i == selected_idx;
        let prefix = if is_sel { "▶ " } else { "  " };
        let style = if is_sel {
            Style::new().fg(Color::BrightYellow).bold()
        } else {
            Style::new().fg(Color::White).dim()
        };

        list.add_child(Node::text(format!("{}{}", prefix, opt), style));
    }

    Node::border_box(BorderType::Thick, Style::new().fg(Color::BrightMagenta))
        .gap(1.0)
        .child(Node::text(
            "SELECT TACTICAL PAYLOAD (Use ↑/↓ or j/k, Enter to confirm):",
            Style::new().fg(Color::BrightCyan).bold(),
        ))
        .child(list)
}

fn build_shell_prompt(input_state: &TextInputState) -> Node {
    Node::border_box(BorderType::Rounded, Style::new().fg(Color::BrightGreen)).child(
        Node::row()
            .gap(1.0)
            .child(Node::text(
                "root@gibson:~#",
                Style::new().fg(Color::BrightGreen).bold(),
            ))
            .child(
                Node::text_input(
                    &input_state.text,
                    input_state.cursor_grapheme,
                    Some("Type command (try 'pool', 'matrix', 'status', 'exit')..."),
                    Style::new().fg(Color::White),
                )
                .flex_grow(1.0),
            ),
    )
}
