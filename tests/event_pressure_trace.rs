//! Presentation must not convert a partial trace into a completed experiment.
#![cfg(unix)]
#![allow(dead_code)]
#[path = "../examples/event_pressure_lab/child.rs"]
mod child;
#[path = "../examples/event_pressure_lab/host.rs"]
mod host;
#[path = "../examples/event_pressure_lab/probe.rs"]
mod probe;
#[path = "../examples/event_pressure_lab/trace.rs"]
mod trace;
#[path = "../examples/event_pressure_lab/visual.rs"]
mod visual;
use trace::Record;
fn r(us: u64, source: &str, kind: &str, value: &str) -> Record {
    Record {
        seq: us,
        us,
        source: source.into(),
        kind: kind.into(),
        value: value.into(),
    }
}
#[test]
fn replay_prefix_cannot_see_future_verdict_or_override_recorded_configuration() {
    let records = [
        r(0, "PTY", "Config", "raw,coincident,false,150,700"),
        r(10, "PTY", "Write", "114"),
        r(20, "APP", "Key", "114"),
        r(30, "PTY", "VerdictWindow", "received"),
    ];
    let prefix = host::view(&probe::Config::default(), &records, 0, 25, false);
    assert_eq!(prefix.mode, "raw");
    assert_eq!(prefix.scenario, "coincident");
    assert_eq!(prefix.pause_ms, 150);
    assert_eq!(prefix.result, "measuring; deadline pending");
    assert_eq!(
        host::view(&probe::Config::default(), &records, 0, 30, false).result,
        "DELIVERED"
    );
    let before = host::view(&probe::Config::default(), &records, 0, 15, false);
    assert!(before.pulses[0].observed_us[5].is_none());
    assert!(before.latencies_us.is_empty());
}
#[test]
fn malformed_key_and_extreme_clock_are_not_hidden_by_analysis() {
    let records = [
        r(0, "PTY", "Write", "114"),
        r(20, "APP", "Key", "114"),
        r(30, "APP", "Key", "not-a-codepoint"),
    ];
    let report = probe::analyze(&records, 0, true);
    assert!(!report.passed());
    assert_eq!(report.verdict, "INVALID KEY RECEIPT");
    assert!(report.latencies_us.is_empty());
    let records = [
        r(u64::MAX, "PTY", "Write", "114"),
        r(u64::MAX, "TTY", "Readable", "1:true"),
    ];
    assert!(!probe::analyze(&records, 0, true).passed());
    let backwards = [r(20, "PTY", "Write", "114"), r(10, "APP", "Key", "114")];
    assert_eq!(
        probe::analyze(&backwards, 0, true).verdict,
        "INVALID CAUSAL TIMESTAMPS"
    );
}
