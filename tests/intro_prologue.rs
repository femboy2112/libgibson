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
    let text = node_to_text(
        prologue::live_ui(prologue::prelude_seconds(), 96, 32),
        96,
        30,
    );
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
    // The other three are queued (three "queued" tokens).
    assert_eq!(
        text.matches("queued").count(),
        3,
        "exactly three workers queued:\n{text}"
    );
    // ARCHITECT's bar is IGNITED but has done no work yet (matches film t=0's
    // progress(0)): the head glyph is present, and there is no filled run.
    assert!(
        text.contains('╸'),
        "ARCHITECT bar head should be lit:\n{text}"
    );
    assert!(
        !text.contains('━'),
        "no worker should show completed progress at handoff:\n{text}"
    );
}

/// The transition is a PROGRESSIVE ASSEMBLY, not a snap: the structure itself
/// arrives over time. These witnesses pin the intended choreography — early on
/// only the shell exists; the workers arrive one at a time; the full operational
/// pose exists only at the end.
#[test]
fn assembly_reveals_structure_progressively() {
    let render = |t: f32| node_to_text(prologue::live_ui(t, 110, 34), 110, 32);
    let count = |hay: &str, workers: &[&str]| workers.iter().filter(|w| hay.contains(**w)).count();
    let workers = ["ARCHITECT", "SCOUT", "BUILDER", "VERIFY"];

    // EARLY (shell just up): header exists, but no workers and no footer yet.
    let early = render(prologue::agent_slot_time(0) - 0.2);
    assert!(
        early.contains("LIBGIBSON"),
        "early: header must exist:\n{early}"
    );
    assert_eq!(
        count(&early, &workers),
        0,
        "early: no worker slots yet:\n{early}"
    );
    assert!(
        !early.contains("orchestrator"),
        "early: footer must not exist yet:\n{early}"
    );

    // MIDDLE (second slot just arrived): objective + orchestration exist, and
    // ONLY the first two workers are present — the last two have not arrived.
    let middle = render(prologue::agent_slot_time(1) + 0.05);
    assert!(
        middle.contains("Build a resilient transit planner."),
        "middle: objective must exist:\n{middle}"
    );
    assert!(
        middle.contains("PLAN → MAP → BUILD → PROVE"),
        "middle: orchestration must exist:\n{middle}"
    );
    assert!(
        middle.contains("ARCHITECT"),
        "middle: ARCHITECT present:\n{middle}"
    );
    assert!(middle.contains("SCOUT"), "middle: SCOUT present:\n{middle}");
    assert!(
        !middle.contains("BUILDER") && !middle.contains("VERIFY"),
        "middle: last two workers must not have arrived yet:\n{middle}"
    );

    // LATER (all slots arrived, before the operational pose): four identities
    // present; the footer/legend have not settled in yet.
    let later = render(prologue::agent_slot_time(3) + 0.1);
    assert_eq!(
        count(&later, &workers),
        4,
        "later: all four worker identities present:\n{later}"
    );

    // HANDOFF: the complete pose, checked by the dedicated test above.
    let handoff = render(prologue::prelude_seconds());
    assert!(
        handoff.contains("orchestrator") && handoff.contains("⌂ PLAN"),
        "handoff: legend + footer present:\n{handoff}"
    );
}

/// No worker may present as ACTIVE before ARCHITECT ignites: while the slots are
/// still arriving, every worker is queued/linking. This guards the specific
/// "don't imply work already happened" requirement.
#[test]
fn no_worker_is_active_before_ignition() {
    // Just after the last slot appears but before the plan begins executing.
    let t = prologue::agent_slot_time(3) + 0.1;
    let text = node_to_text(prologue::live_ui(t, 110, 34), 110, 32);
    assert!(
        text.contains("ARCHITECT"),
        "sanity: ARCHITECT present:\n{text}"
    );
    assert_eq!(
        text.matches("active").count(),
        0,
        "no worker should be active before ignition:\n{text}"
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
        ("live UI · shell", prologue::agent_slot_time(0) - 0.2),
        ("live UI · two workers", prologue::agent_slot_time(1) + 0.05),
        ("live UI · all slots", prologue::agent_slot_time(3) + 0.1),
        ("live UI · handoff pose", end),
    ] {
        println!(
            "\n===== {label} (t={t:.2}s, stage={:?}) =====",
            prologue::stage(t)
        );
        print!("{}", node_to_text(prologue::live_ui(t, 96, 34), 96, 32));
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

    // 2) Handoff into the fullscreen film: a film-only label appears. "LIVE
    // WORKSPACE" is the film's workspace-column heading (present at t=0 on a
    // 120-wide terminal) and never appears in the prologue — unlike "SEALED",
    // which the prologue's match-cut objective row now also shows.
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        pump(&mut capture, &mut parser, 15);
        if parser.screen().contents().contains("LIVE WORKSPACE") {
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
