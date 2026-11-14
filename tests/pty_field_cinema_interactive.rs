//! RXIV Gate C — the INTERACTIVE live instrument-film, driven by real keystrokes on a real PTY.
//!
//! Every case runs `field_cinema --live --interactive --step` (paused: frames advance only on the
//! space key) and synchronizes on what is actually painted (a vt100 screen), never on sleeps — so
//! the human path is exercised deterministically: a real source field appears, the viewer moves the
//! probe, picks a target, issues the command through the certified channel, sees the correlated
//! verdict, steps to the next authoritative field, and the film responds to the VERIFIED
//! difference. Cases: accepted (+ record → producer-free replay equality), a narrow monochrome
//! 40×15 terminal, a refused edit, an acknowledged-but-suppressed edit, a masked sample on the
//! non-Gaussian ring holdout, a producer disconnect, reduced motion, and the film killed mid-session.
#![cfg(unix)]

use portable_pty::CommandBuilder;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use vt100::Parser;

#[path = "common/pty_capture.rs"]
mod pty_capture;

struct Film {
    capture: pty_capture::Capture,
    parser: Parser,
    processed: usize,
}

impl Film {
    fn boot(cols: u16, rows: u16, mono: bool, extra: &[&str], env: &[(&str, &str)]) -> Self {
        let mut cmd = CommandBuilder::new(pty_capture::example_path("field_cinema"));
        cmd.cwd(env!("CARGO_MANIFEST_DIR"));
        cmd.env("TERM", "xterm-256color");
        if mono {
            cmd.env("NO_COLOR", "1");
        }
        for (k, v) in env {
            cmd.env(k, v);
        }
        for a in ["--live", "--interactive", "--step"] {
            cmd.arg(a);
        }
        for a in extra {
            cmd.arg(a);
        }
        Film {
            capture: pty_capture::Capture::spawn(cmd, cols, rows, Duration::from_secs(60)),
            parser: Parser::new(
                core::num::NonZeroU16::new(rows).expect("vt100 size is non-zero"),
                core::num::NonZeroU16::new(cols).expect("vt100 size is non-zero"),
                0,
            ),
            processed: 0,
        }
    }

    fn pump(&mut self, d: Duration) {
        self.capture.collect_for(d).expect("collect PTY output");
        let raw = self.capture.raw();
        self.parser.process(&raw[self.processed..]);
        self.processed = raw.len();
    }

    fn screen(&self) -> String {
        self.parser.screen().contents()
    }

    fn wait_text(&mut self, needle: &str) {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            self.pump(Duration::from_millis(40));
            if self.screen().contains(needle) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {needle:?}; screen:\n{}",
                self.screen()
            );
        }
    }

    fn key(&mut self, bytes: &[u8]) {
        self.capture.write(bytes).expect("write to PTY");
        self.pump(Duration::from_millis(60));
    }

    /// Quit and return (exit success, the full raw output as text).
    fn quit(mut self) -> (bool, String) {
        let _ = self.capture.write(b"q");
        let deadline = Instant::now() + Duration::from_secs(15);
        let status = loop {
            self.pump(Duration::from_millis(40));
            if let Ok(Some(s)) = self.capture.try_wait() {
                break s;
            }
            assert!(
                Instant::now() < deadline,
                "the film did not exit after q:\n{}",
                self.screen()
            );
        };
        for _ in 0..4 {
            self.pump(Duration::from_millis(30));
        }
        let raw = self.capture.raw().to_vec();
        let has = |n: &[u8]| raw.windows(n.len()).any(|w| w == n);
        assert!(has(b"\x1b[?1049l"), "left the alternate screen");
        assert!(has(b"\x1b[?25h"), "cursor restored");
        (status.success(), String::from_utf8_lossy(&raw).into_owned())
    }
}

/// The value after `key` on the line that carries it (the first summary line can share a line with
/// the terminal-restore escapes, so the key is searched within lines, not only at their start).
fn line_value<'a>(out: &'a str, key: &str) -> &'a str {
    out.lines()
        .find_map(|l| l.split_once(key).map(|(_, v)| v))
        .map(str::trim)
        .unwrap_or_else(|| panic!("no {key:?} line in output:\n{out}"))
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rxiv-c-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn assert_gone(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Path::new(&format!("/proc/{pid}")).exists() {
        assert!(
            Instant::now() < deadline,
            "producer {pid} outlived the film"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The full human path at 100×32 in colour: ESTABLISH → (probe) DISCOVER → (step) calibration →
/// (Enter) INTERVENE → correlated ACK → AWAIT → (step) TRANSFORM on the verified difference →
/// (r) RESOLVE. The film's record is then replayed with NO producer and must reproduce the exact
/// per-event beat sequence.
#[test]
fn interactive_accepted_edit_walks_the_arc_and_replays_exactly() {
    let dir = scratch("accepted");
    let d = dir.to_str().unwrap();
    let mut f = Film::boot(100, 32, false, &["--record", d], &[]);
    f.wait_text("The source speaks");
    f.wait_text("probe   (24,12)");
    f.key(b"\x1b[C");
    f.wait_text("probe   (25,12)");
    f.wait_text("Discovering:");
    f.key(b" ");
    f.wait_text("calib   2 obs");
    f.key(b"\r");
    f.wait_text("verdict Acknowledged (#1)");
    f.wait_text("Awaiting authoritative evidence");
    f.key(b" ");
    f.wait_text("evidence consistent with request");
    f.wait_text("the field now peaks at");
    f.key(b"r");
    f.wait_text("Resolved — 1 evidenced transformation");
    let (ok, out) = f.quit();
    assert!(ok, "a clean film exits 0:\n{out}");
    assert_eq!(
        line_value(&out, "film-path:"),
        "ESTABLISH>DISCOVER>INTERVENE>AWAIT>TRANSFORM>RESOLVE"
    );
    assert!(out.contains("transforms 1"), "{out}");
    assert!(
        out.contains("(graceful)"),
        "the producer exited on EOF:\n{out}"
    );

    // Exact semantic replay from the committed record, with no producer at all.
    let replay = std::process::Command::new(pty_capture::example_path("field_cinema"))
        .args(["--replay-film", d])
        .output()
        .expect("run --replay-film");
    let rout = String::from_utf8_lossy(&replay.stdout).into_owned();
    assert!(replay.status.success(), "{rout}");
    assert_eq!(
        line_value(&rout, "film-path:"),
        line_value(&out, "film-path:")
    );
    assert_eq!(
        line_value(&rout, "film-beats-digest:"),
        line_value(&out, "film-beats-digest:"),
        "the FULL per-event beat sequence replays identically"
    );
    assert!(rout.contains("transforms 1"), "{rout}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A narrow MONOCHROME 40×15 terminal: the compact layout keeps beat, probe, target/verdict and
/// evidence on screen, paints the field with the colour-free ramp, and emits no colour SGR.
#[test]
fn interactive_film_works_at_40x15_monochrome() {
    let mut f = Film::boot(40, 15, true, &[], &[]);
    f.wait_text("ESTABLISH · t1");
    f.key(b"\x1b[D");
    f.wait_text("DISCOVER");
    f.key(b" ");
    f.wait_text("t2");
    f.key(b"\r");
    f.wait_text("Acknowledged");
    f.key(b" ");
    f.wait_text("TRANSFORM");
    f.wait_text("evidence consistent with request");
    let (ok, out) = f.quit();
    assert!(ok, "{out}");
    assert!(
        !out.contains("\x1b[38;2") && !out.contains("\x1b[48;2") && !out.contains("\x1b[38;5"),
        "a monochrome terminal receives no colour SGR"
    );
    assert!(line_value(&out, "film-path:").contains("TRANSFORM"));
}

/// An out-of-range target (12.0 > the producer's 10.0) is REFUSED: the film returns to DISCOVER,
/// says so, and never transforms.
#[test]
fn interactive_refused_edit_never_transforms() {
    let mut f = Film::boot(100, 32, false, &[], &[]);
    f.wait_text("The source speaks");
    f.key(b" ");
    f.wait_text("calib   2 obs");
    for _ in 0..20 {
        f.key(b"]");
    }
    f.wait_text("amplitude 12.0");
    f.key(b"\r");
    f.wait_text("verdict Refused (#1)");
    f.wait_text("Refused by the producer");
    f.key(b" ");
    f.wait_text("calib   3 obs");
    let (ok, out) = f.quit();
    assert!(ok, "{out}");
    assert!(
        !line_value(&out, "film-path:").contains("TRANSFORM"),
        "{out}"
    );
    assert!(out.contains("transforms 0"), "{out}");
}

/// The acknowledged-but-SUPPRESSED edit (`--suppress`): the identical ACK, but the field never
/// changes — the evidence window closes UNEVIDENCED and the film never enters TRANSFORM.
#[test]
fn interactive_suppressed_edit_is_acknowledged_but_never_transforms() {
    let mut f = Film::boot(100, 32, false, &["--suppress"], &[]);
    f.wait_text("The source speaks");
    f.key(b" ");
    f.wait_text("calib   2 obs");
    f.key(b"\r");
    f.wait_text("verdict Acknowledged (#1)");
    for _ in 0..3 {
        f.key(b" ");
    }
    f.wait_text("no consistent change, so no transformation");
    let (ok, out) = f.quit();
    assert!(ok, "{out}");
    assert!(
        !line_value(&out, "film-path:").contains("TRANSFORM"),
        "{out}"
    );
    assert!(out.contains("Unevidenced"), "{out}");
}

/// The non-Gaussian RING holdout with a sparse mask: probing a masked cell reports "masked" —
/// never a fabricated value — and an accepted edit on this geometry still earns the transform.
#[test]
fn interactive_ring_holdout_reports_masked_samples_and_transforms() {
    let mut f = Film::boot(
        100,
        32,
        false,
        &["--shape", "ring", "--mask-every", "5"],
        &[],
    );
    f.wait_text("The source speaks");
    f.key(b"\x1b[D");
    f.key(b"\x1b[D");
    // (22,12): (7·22 + 3·12) % 5 == 0 → masked by the producer.
    f.wait_text("probe   (22,12) = masked");
    f.key(b" ");
    f.wait_text("calib   2 obs");
    f.key(b"\r");
    f.wait_text("verdict Acknowledged (#1)");
    f.key(b" ");
    f.wait_text("evidence consistent with request");
    let (ok, out) = f.quit();
    assert!(ok, "{out}");
    assert!(line_value(&out, "film-path:").contains("TRANSFORM"));
}

/// A producer that publishes one field and then exits: the film shows the TYPED failure, keeps the
/// final state visible until q, exits nonzero, and the producer is reaped.
#[test]
fn interactive_producer_disconnect_is_shown_typed_and_exits_nonzero() {
    use gibson::field_view::FieldExtent;
    use gibson::wire::{encode_field, SourceCursor, WireField, WireLimits};
    let dir = scratch("disconnect");
    std::fs::create_dir_all(&dir).unwrap();
    let extent = FieldExtent::new(gibson::Point::new(0.0, 0.0), 1.0, 1.0, 16, 8).unwrap();
    let field = WireField {
        extent,
        model_tick: 1,
        data: (0..128).map(|i| (i as f64 / 128.0).sin()).collect(),
        mask: None,
    };
    let bytes = encode_field(1, SourceCursor::new(1, 1), &field, WireLimits::default()).unwrap();
    let frame = dir.join("one.bin");
    std::fs::write(&frame, bytes).unwrap();
    let fake = dir.join("one-shot-producer");
    std::fs::write(
        &fake,
        format!(
            "#!/bin/sh\ntrap '' HUP\ncat '{}'\necho 'one-shot producer: done' >&2\nexit 0\n",
            frame.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut f = Film::boot(
        100,
        32,
        false,
        &[],
        &[("GIBSON_PYTHON", fake.to_str().unwrap())],
    );
    f.wait_text("PRODUCER FAILURE");
    f.wait_text("producer closed its stream");
    let pid: u32 = {
        let s = f.screen();
        let after = s.split("pid ").nth(1).expect("pid in title").to_string();
        after
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    let (ok, out) = f.quit();
    assert!(!ok, "a failed film exits nonzero:\n{out}");
    assert!(
        out.contains("film-failure: producer closed its stream"),
        "{out}"
    );
    assert!(
        out.contains("one-shot producer: done"),
        "stderr is kept:\n{out}"
    );
    assert_gone(pid);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Reduced motion: the film announces it and walks the same arc (the motion contract itself — no
/// camera tween, no caption slide — is pinned headlessly in `field_cinema_interactive_oracle.rs`).
#[test]
fn interactive_reduced_motion_walks_the_same_arc() {
    let mut f = Film::boot(100, 32, false, &["--reduced-motion"], &[]);
    f.wait_text("motion reduced");
    f.wait_text("The source speaks");
    f.key(b" ");
    f.wait_text("calib   2 obs");
    f.key(b"\r");
    f.wait_text("verdict Acknowledged (#1)");
    f.key(b" ");
    f.wait_text("evidence consistent with request");
    let (ok, out) = f.quit();
    assert!(ok, "{out}");
    assert!(line_value(&out, "film-path:").contains("TRANSFORM"));
}

/// The film KILLED mid-session (SIGKILL — no destructor runs): the reference producer is a child
/// that reads stdin, so the kernel closing the film's end of the pipe gives it EOF and it exits on
/// its own. This is the documented boundary: a producer that ignores stdin EOF cannot be reaped by
/// a SIGKILLed parent (that is the OS's job), which is why the supervised paths above never rely on
/// it.
#[test]
fn a_killed_film_still_ends_its_reference_producer_via_stdin_eof() {
    let mut f = Film::boot(100, 32, false, &[], &[]);
    f.wait_text("The source speaks");
    let pid: u32 = {
        let s = f.screen();
        let after = s.split("pid ").nth(1).expect("pid in title").to_string();
        after
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    assert!(Path::new(&format!("/proc/{pid}")).exists());
    let film_pid = f.capture.process_id();
    // SAFETY: signalling the PTY child this test spawned.
    assert_eq!(unsafe { libc::kill(film_pid as i32, libc::SIGKILL) }, 0);
    let deadline = Instant::now() + Duration::from_secs(10);
    while f.capture.try_wait().ok().flatten().is_none() {
        assert!(Instant::now() < deadline, "the film did not die");
        f.pump(Duration::from_millis(20));
    }
    assert_gone(pid);
}

/// RXIV review finding 6: a producer that never answers a command must not freeze the film. After
/// the verdict deadline the film reports a TYPED failure (the command's fate is unknown — never
/// treated as acknowledged or refused), keeps the final state visible, and exits nonzero on q.
#[test]
fn interactive_a_verdict_that_never_arrives_fails_typed_after_the_deadline() {
    use gibson::field_view::FieldExtent;
    use gibson::wire::{encode_field, SourceCursor, WireField, WireLimits};
    let dir = scratch("silent-verdict");
    std::fs::create_dir_all(&dir).unwrap();
    let extent = FieldExtent::new(gibson::Point::new(0.0, 0.0), 1.0, 1.0, 16, 8).unwrap();
    let field = WireField {
        extent,
        model_tick: 1,
        data: (0..128).map(|i| (i as f64 / 128.0).cos()).collect(),
        mask: None,
    };
    let bytes = encode_field(1, SourceCursor::new(1, 1), &field, WireLimits::default()).unwrap();
    let frame = dir.join("one.bin");
    std::fs::write(&frame, bytes).unwrap();
    let fake = dir.join("mute-producer");
    std::fs::write(
        &fake,
        format!(
            // Publishes one field, keeps its stdout OPEN, never reads or answers a command.
            "#!/bin/sh\ntrap '' HUP\ncat '{}'\nexec sleep 60\n",
            frame.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut f = Film::boot(
        100,
        32,
        false,
        &[],
        &[("GIBSON_PYTHON", fake.to_str().unwrap())],
    );
    f.wait_text("The source speaks");
    f.key(b"\r");
    f.wait_text("verdict awaiting #1");
    f.wait_text("PRODUCER FAILURE");
    f.wait_text("the command's ACK");
    let (ok, out) = f.quit();
    assert!(!ok, "{out}");
    assert!(out.contains("film-failure: timed out"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}
