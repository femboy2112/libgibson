//! Bounded runtime diagnosis. A test that detects the known upstream stall is
//! not a claim that delivery works; the delivery acceptance stays ignored/red.
#![cfg(unix)]

#[path = "../examples/event_pressure_lab/probe.rs"]
mod probe;
#[path = "common/pty_capture.rs"]
mod pty_capture;
#[path = "../examples/event_pressure_lab/trace.rs"]
mod trace;

use portable_pty::CommandBuilder;
use probe::{Config, Report};
use std::time::{Duration, Instant};
use trace::Record;

fn config(mode: &str, scenario: &str, graphics: bool) -> Config {
    Config {
        mode: mode.into(),
        scenario: scenario.into(),
        graphics,
        pause_ms: 60,
        deadline_ms: 700,
    }
}

fn trial(config: Config) -> Report {
    let exe = pty_capture::example_path("event_pressure_lab");
    probe::run(&exe, config).expect("bounded diagnostic trial")
}

fn receipts(report: &Report) -> String {
    // Capped failure evidence retains early causal actions and recent state.
    let first = report.records.iter().take(32);
    let last = report
        .records
        .iter()
        .skip(report.records.len().saturating_sub(96).max(32));
    format!(
        "{} sent={:?} received={:?} restored={} records={}\n{}",
        report.verdict,
        report.sent,
        report.received,
        report.restored,
        report.records.len(),
        first.chain(last).map(Record::line).collect::<String>()
    )
}

fn assert_lifecycle(report: &Report) {
    assert!(report.restored, "{}", receipts(report));
    assert!(report.records.len() < trace::MAX_RECORDS);
    assert!(report
        .records
        .windows(2)
        .all(|p| p[0].seq < p[1].seq && p[0].us <= p[1].us));
    assert!(report
        .records
        .iter()
        .any(|r| r.source == "PTY" && r.kind == "Exit" && r.value == "0"));
}

fn assert_delivered(report: &Report, keys: &str) {
    assert_lifecycle(report);
    assert!(report.passed(), "{}", receipts(report));
    assert_eq!(report.sent, keys, "{}", receipts(report));
    assert_eq!(report.received, keys, "{}", receipts(report));
    assert_eq!(report.latencies_us.len(), keys.len());
    assert_eq!(
        report
            .records
            .iter()
            .filter(|r| r.source == "PTY" && r.kind == "Write")
            .count(),
        keys.len(),
        "acceptance must not inject a rescue key"
    );
}

fn assert_known_stall(report: &Report) {
    assert_lifecycle(report);
    assert_eq!(report.sent, "r", "{}", receipts(report));
    assert_eq!(report.received, "", "{}", receipts(report));
    assert_eq!(report.verdict, "READABLE / NOT DELIVERED");
    assert!(report.latencies_us.is_empty(), "failure is censored");
    let written = report
        .records
        .iter()
        .find(|r| r.source == "PTY" && r.kind == "Write")
        .unwrap();
    assert!(report.records.iter().any(|r| {
        r.source == "TTY"
            && r.kind == "Readable"
            && r.value == "1:true"
            && r.us > written.us + 50_000
    }));
    assert!(report
        .records
        .iter()
        .any(|r| { matches!(r.source.as_str(), "CROSSTERM" | "CONTEXT") && r.kind == "Resize" }));
    assert!(report
        .records
        .iter()
        .any(|r| r.kind == "VerdictWindow" && r.value == "deadline"));
    assert_eq!(
        report
            .records
            .iter()
            .filter(|r| r.source == "PTY" && r.kind == "Write")
            .count(),
        1,
        "diagnostic must leave the missing key alone"
    );
}

#[test]
fn silent_raw_crossterm_and_context_deliver_exact_normal_order() {
    for mode in probe::MODES {
        let report = trial(config(mode, "normal", false));
        assert_delivered(&report, "abc");
        assert_eq!(report.frames, 0);
    }
}

#[test]
fn settled_resize_then_lone_key_never_needs_a_second_key() {
    for mode in probe::MODES {
        assert_delivered(&trial(config(mode, "resize-key", false)), "r");
    }
}

#[test]
fn controlled_collision_distinguishes_raw_delivery_from_known_backend_stall() {
    assert_delivered(&trial(config("raw", "coincident", false)), "r");
    for mode in ["crossterm", "context"] {
        // This pins the present diagnostic signature, not acceptable behavior.
        // When the supported dependency is repaired, replace this expectation
        // and enable the separate delivery acceptance below.
        assert_known_stall(&trial(config(mode, "coincident", false)));
    }
}

#[test]
fn graphical_drain_pauses_are_bounded_and_failures_are_classified_honestly() {
    for mode in probe::MODES {
        for pause_ms in [20, 60, 150] {
            let report = trial(Config {
                pause_ms,
                ..config(mode, "slow-drain", true)
            });
            assert_lifecycle(&report);
            assert_eq!(report.sent, "r");
            assert!(report.frames > 0, "{}", receipts(&report));
            assert!(report.drained > 0, "{}", receipts(&report));
            assert!(report.generated > 0, "{}", receipts(&report));
            assert!(report.records.iter().any(|r| r.kind == "DrainPaused"));
            assert!(report.records.iter().any(|r| r.kind == "DrainResumed"));
            if report.passed() {
                assert_delivered(&report, "r");
            } else {
                assert_ne!(mode, "raw", "raw control failed: {}", receipts(&report));
                assert_known_stall(&report);
            }
        }
    }
}

#[test]
#[ignore = "Known upstream failure: issue #15; run explicitly to test real delivery acceptance"]
fn context_coincident_lone_key_delivery_acceptance() {
    assert_delivered(&trial(config("context", "coincident", false)), "r");
}

#[test]
fn diagnostic_configuration_and_receipt_boundaries_reject_invalid_input() {
    assert!(Config::default().validate().is_ok());
    for bad in [
        Config {
            mode: "unknown".into(),
            ..Config::default()
        },
        Config {
            scenario: "unknown".into(),
            ..Config::default()
        },
        Config {
            pause_ms: 501,
            ..Config::default()
        },
        Config {
            deadline_ms: 99,
            ..Config::default()
        },
        Config {
            deadline_ms: 3001,
            ..Config::default()
        },
    ] {
        assert!(bad.validate().is_err());
    }
    let sample = record(9, "APP", "Key", "114");
    assert_eq!(Record::parse(sample.line().trim_end()), Some(sample));
    for bad in [
        "",
        "0\t1\tAPP\tKey",
        "x\t1\tAPP\tKey\t114",
        "0\tx\tAPP\tKey\t114",
        "0\t1\tAPP\tKey\t114\textra",
        "0\t1\tAPP\tKey\t\u{1b}[31m",
        "0\t1\tAPP\r\tKey\t114",
    ] {
        assert!(Record::parse(bad).is_none());
    }
    assert!(Record::parse(&format!("0\t1\tAPP\tKey\t{}", "x".repeat(513))).is_none());
    let path = std::env::temp_dir().join(format!(
        "gibson-pressure-recorder-test-{}-{}",
        std::process::id(),
        trace::clock_us()
    ));
    {
        let mut recorder = trace::Recorder::new(&path, trace::clock_us()).unwrap();
        assert!(trace::Recorder::new(&path, trace::clock_us()).is_err());
        assert!(recorder.emit("APP", "Key", "a\u{1b}[31m").is_err());
        assert!(recorder.emit("APP\twrong", "Key", "114").is_err());
        assert!(recorder.emit("APP", "Key\nwrong", "114").is_err());
        assert!(recorder.emit("APP", "Key", "x".repeat(161)).is_err());
        recorder.emit("APP", "Key", "114").unwrap();
    }
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let records: Vec<_> = content
        .lines()
        .map(|line| Record::parse(line).unwrap())
        .collect();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].seq, 0,
        "rejected records must not consume sequence"
    );
}

fn record(us: u64, source: &str, kind: &str, value: &str) -> Record {
    Record {
        seq: us,
        us,
        source: source.into(),
        kind: kind.into(),
        value: value.into(),
    }
}

#[test]
fn analysis_rejects_duplicate_reordered_and_missing_keys_without_fake_latency() {
    let sent = [
        record(10, "PTY", "Write", "97"),
        record(20, "PTY", "Write", "98"),
    ];
    for keys in [vec!["97"], vec!["97", "97", "98"], vec!["98", "97"]] {
        let mut records = sent.to_vec();
        records.extend(
            keys.iter()
                .enumerate()
                .map(|(i, key)| record(30 + i as u64, "APP", "Key", key)),
        );
        let report = probe::analyze(&records, 0, true);
        assert!(!report.passed());
        assert!(report.latencies_us.is_empty());
        assert_eq!(report.verdict, "DEADLINE / UNRESOLVED");
    }
    let mut records = sent.to_vec();
    records.extend([
        record(30, "APP", "Key", "97"),
        record(50, "APP", "Key", "98"),
    ]);
    let report = probe::analyze(&records, 12, true);
    assert!(report.passed());
    assert_eq!(report.latencies_us, [20, 30]);
    assert_eq!(report.drained, 12);
    assert!(!probe::analyze(&records, 12, false).passed());
    assert!(!probe::analyze(&[], 0, true).passed());
}

#[test]
fn matching_keys_received_after_verdict_deadline_do_not_retroactively_pass() {
    let records = [
        record(10, "PTY", "Write", "114"),
        record(110, "PTY", "VerdictWindow", "deadline"),
        record(120, "APP", "Key", "114"),
        record(150, "CHILD", "Restored", ""),
        record(151, "PTY", "Exit", "0"),
    ];
    let report = probe::analyze(&records, 0, true);
    assert_eq!(report.sent, report.received);
    assert!(!report.passed(), "late receipt changed the failed deadline");
    assert!(
        report.latencies_us.is_empty(),
        "late key is not a successful sample"
    );
}

struct Viewer {
    capture: pty_capture::Capture,
    parser: vt100::Parser,
    processed: usize,
}

fn visible_cell(cell: &vt100::Cell) -> (String, vt100::Color, vt100::Color, [bool; 7]) {
    // Erase operations leave an empty cell; an explicit space paints the same
    // visible blank. Preserve every exposed style and wide-glyph attribute.
    (
        if cell.contents().is_empty() {
            " "
        } else {
            cell.contents()
        }
        .into(),
        cell.fgcolor(),
        cell.bgcolor(),
        [
            cell.bold(),
            cell.dim(),
            cell.italic(),
            cell.underline(),
            cell.inverse(),
            cell.is_wide(),
            cell.is_wide_continuation(),
        ],
    )
}
impl Viewer {
    fn pump(&mut self, ms: u64) {
        self.capture.collect_for(Duration::from_millis(ms)).unwrap();
        self.parser.process(&self.capture.raw()[self.processed..]);
        self.processed = self.capture.raw().len();
    }
    fn wait_text(&mut self, text: &str) {
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            self.pump(10);
            if self.parser.screen().contents().contains(text) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "missing {text:?}:\n{}",
                self.parser.screen().contents()
            );
        }
    }
}

#[test]
fn real_viewer_controls_resize_and_escape_restore_both_terminal_modes() {
    let mut cmd = CommandBuilder::new(pty_capture::example_path("event_pressure_lab"));
    cmd.args(["--mode=raw", "--workload=silent", "--color=mono"]);
    cmd.env("TERM", "xterm-256color");
    let mut viewer = Viewer {
        capture: pty_capture::Capture::spawn(cmd, 120, 32, Duration::from_secs(15)),
        parser: vt100::Parser::new(32, 120, 0),
        processed: 0,
    };
    viewer.wait_text("EVENT PRESSURE LAB");
    viewer.wait_text("DELIVERED");
    viewer.capture.write(b"4").unwrap();
    viewer.wait_text("coincident");
    viewer.wait_text("DELIVERED");
    viewer.capture.write(b" ").unwrap();
    viewer.pump(100);
    let frozen = viewer.parser.screen().contents_formatted();
    let frozen_cells: Vec<_> = (0..32)
        .flat_map(|y| (0..120).map(move |x| (y, x)))
        .map(|(y, x)| (y, x, viewer.parser.screen().cell(y, x).unwrap().clone()))
        .collect();
    let bytes = viewer.capture.raw().len();
    viewer.pump(100);
    assert_eq!(viewer.parser.screen().contents_formatted(), frozen);
    assert_eq!(
        viewer.capture.raw().len(),
        bytes,
        "frozen viewer emitted bytes"
    );
    for (cols, rows) in [(56, 24), (160, 40), (120, 32)] {
        viewer.parser.screen_mut().set_size(rows, cols);
        viewer.capture.resize(cols, rows);
        // Drain during settling: this is UI reflow hygiene, not the collision
        // experiment. The deliberately colliding child has its own PTY above.
        viewer.pump(120);
        viewer.wait_text("EVENT");
    }
    // Hidden hardware cursor position can differ after a full resize repaint;
    // compare every actual cell, including its attributes and wide metadata.
    for (y, x, cell) in frozen_cells {
        assert_eq!(
            visible_cell(viewer.parser.screen().cell(y, x).unwrap()),
            visible_cell(&cell),
            "resize changed frozen cell {x},{y}"
        );
    }
    viewer.capture.write(b"\x1b").unwrap();
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        viewer.pump(10);
        if let Some(status) = viewer.capture.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline, "viewer exit deadline");
    }
    viewer.pump(20);
    let raw = String::from_utf8_lossy(viewer.capture.raw());
    assert!(raw.contains("\x1b[?1049h"));
    assert!(raw.contains("\x1b[?1049l"));
    assert!(raw.contains("\x1b[?2004l"));
    assert!(!viewer.parser.screen().alternate_screen());
    assert!(!viewer.parser.screen().hide_cursor());
    assert_eq!(viewer.capture.termios(), viewer.capture.initial_termios);
}
