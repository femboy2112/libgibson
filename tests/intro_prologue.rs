//! First-contact prologue: composition witnesses + a real PTY restoration test.
//!
//! The prologue must (a) present a credible boot log and a live UI that matches
//! the film's Harness framing, and (b) hand terminal ownership to the fullscreen
//! film cleanly — inline context released before the fullscreen context is
//! acquired, terminal fully restored on exit. The pure-composition assertions run
//! everywhere; the PTY test is Unix-only.
//!
//! Set DUMP_PROLOGUE=1 to print the boot log and live-UI frames as text:
//!   DUMP_PROLOGUE=1 cargo test --test intro_prologue dump_prologue -- --nocapture

#[allow(dead_code)]
#[path = "../examples/libgibson_intro.rs"]
mod intro;

use intro::prologue;

/// Renders a live-region Node to plain text via the real layout + paint engine.
fn node_to_text(mut node: gibson::Node, width: u16, height: u16) -> String {
    let mut surface = gibson::Surface::new(width, height);
    if gibson::layout::compute_layout(&mut node, width, height).is_ok() {
        gibson::painter::paint(&node, &mut surface);
    }
    let mut out = String::new();
    for y in 0..height {
        for x in 0..width {
            out.push_str(
                surface
                    .get(x, y)
                    .map(|c| c.glyph.grapheme.as_str())
                    .unwrap_or(" "),
            );
        }
        out.push('\n');
    }
    out
}

/// The live UI at t=0 of the film-handoff must show the harness framing: title,
/// objective, all four canonical agents, ARCHITECT active and the rest queued.
#[test]
fn live_ui_matches_harness_framing_at_handoff() {
    let text = node_to_text(prologue::live_ui(prologue::prelude_seconds(), 96), 96, 22);
    for needle in [
        "LIBGIBSON",
        "AGENT OPERATIONS",
        "Build a resilient transit planner.",
        "ARCHITECT",
        "SCOUT",
        "BUILDER",
        "VERIFY",
        "PLAN → MAP → BUILD → PROVE",
        "orchestrator",
    ] {
        assert!(text.contains(needle), "live UI missing {needle:?}:\n{text}");
    }
    // ARCHITECT is the only active worker at handoff (matches film t=0).
    assert!(text.contains("active"), "ARCHITECT should be active");
    assert_eq!(
        text.matches("active").count(),
        1,
        "only ARCHITECT is active:\n{text}"
    );
    assert!(
        text.contains("queued"),
        "the other three workers should be queued"
    );
}

/// The boot log is credible and names the four canonical agents plus the key
/// harness milestones.
#[test]
fn boot_log_is_credible_and_names_the_agents() {
    let lines = prologue::boot_lines_plain();
    let joined = lines.join("\n");
    for needle in [
        "libgibson-harness",
        "workspace",
        "repository",
        "runtime",
        "tool registry",
        "acceptance",
        "coordinator",
        "ARCHITECT",
        "SCOUT",
        "BUILDER",
        "VERIFY",
        "online",
        "executing",
    ] {
        assert!(
            joined.contains(needle),
            "boot log missing {needle:?}:\n{joined}"
        );
    }
}

/// Visual inspection aid (opt-in). Prints the boot log and three live-UI frames.
#[test]
fn dump_prologue() {
    if std::env::var("DUMP_PROLOGUE").as_deref() != Ok("1") {
        return;
    }
    println!("\n===== PROLOGUE BOOT LOG (scrollback) =====");
    for l in prologue::boot_lines_plain() {
        println!("{l}");
    }
    let end = prologue::prelude_seconds();
    for (label, t) in [
        ("live UI · attach", 4.5_f32),
        ("live UI · mid", end * 0.7),
        ("live UI · handoff", end),
    ] {
        println!("\n===== {label} (t={t:.2}s) =====");
        print!("{}", node_to_text(prologue::live_ui(t, 96), 96, 22));
    }
}

#[cfg(unix)]
#[path = "common/pty_capture.rs"]
mod pty_capture;

/// The default experience: prologue plays, hands off to the fullscreen film, and
/// the terminal is fully restored on exit. Proves the inline->fullscreen lease
/// handoff (a leaked lease would make the film's fullscreen() fail and the
/// process exit non-zero).
#[cfg(unix)]
#[test]
fn prologue_boots_hands_off_to_film_and_restores() {
    use portable_pty::CommandBuilder;
    use std::time::{Duration, Instant};

    let mut cmd = CommandBuilder::new(pty_capture::example_path("libgibson_intro"));
    // --deterministic makes the prelude clock fast; no --at/--stage/--seconds so
    // the prologue actually runs. TERM is graphical so glyphs stay Braille.
    cmd.args(["--deterministic", "--color=truecolor"]);
    cmd.env("TERM", "xterm-256color");
    let mut capture = pty_capture::Capture::spawn(cmd, 120, 32, Duration::from_secs(30));
    let mut parser = vt100::Parser::new(32, 120, 0);
    let mut processed = 0usize;

    let mut pump = |capture: &mut pty_capture::Capture, parser: &mut vt100::Parser, ms: u64| {
        capture
            .collect_for(Duration::from_millis(ms))
            .expect("drain prologue PTY");
        parser.process(&capture.raw()[processed..]);
        processed = capture.raw().len();
    };

    // 1) The boot log appears in the primary buffer (inline, before alt-screen).
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        pump(&mut capture, &mut parser, 15);
        if parser.screen().contents().contains("ARCHITECT") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "prologue boot log never showed ARCHITECT:\n{}",
            parser.screen().contents()
        );
    }
    // The boot log is inline: the alternate screen has not been entered yet.
    assert!(
        !String::from_utf8_lossy(capture.raw()).contains("\x1b[?1049h"),
        "prologue entered the alternate screen before handoff"
    );

    // 2) Handoff into the fullscreen film: a film-only label ("SEALED") appears.
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        pump(&mut capture, &mut parser, 15);
        if parser.screen().contents().contains("SEALED") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "film never took over after the prologue:\n{}",
            parser.screen().contents()
        );
    }
    assert!(
        String::from_utf8_lossy(capture.raw()).contains("\x1b[?1049h"),
        "film did not enter the alternate screen (lease handoff likely failed)"
    );
    assert!(
        parser.screen().alternate_screen(),
        "not in alt-screen during film"
    );

    // 3) Esc exits; the terminal is fully restored.
    capture.write(b"\x1b").expect("send Esc");
    let deadline = Instant::now() + Duration::from_secs(6);
    let status = loop {
        pump(&mut capture, &mut parser, 15);
        if let Some(s) = capture.try_wait().expect("poll intro") {
            break s;
        }
        assert!(Instant::now() < deadline, "intro did not exit after Esc");
    };
    assert!(status.success(), "intro exited unsuccessfully: {status:?}");
    pump(&mut capture, &mut parser, 20);
    let raw = String::from_utf8_lossy(capture.raw());
    assert!(raw.contains("\x1b[?1049h"), "fullscreen not entered");
    assert!(raw.contains("\x1b[?1049l"), "fullscreen not restored");
    assert!(!parser.screen().alternate_screen(), "left in alt-screen");
    assert!(!parser.screen().hide_cursor(), "cursor left hidden");
    assert_eq!(
        capture.termios(),
        capture.initial_termios,
        "terminal modes not restored after prologue+film"
    );
}
