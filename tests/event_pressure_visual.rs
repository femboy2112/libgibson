//! Rendering contracts for the diagnostic viewer, not evidence of a runtime fix.
#[path = "../examples/event_pressure_lab/visual.rs"]
mod visual;

use gibson::{compute_diff, AnsiCompiler, ColorDepth, Surface};

fn text(surface: &Surface) -> String {
    surface
        .cells
        .iter()
        .map(|c| c.glyph.grapheme.as_str())
        .collect()
}

#[test]
fn diagnostic_percentiles_are_nearest_rank_and_retention_bounded() {
    assert_eq!(visual::latency_summary(&[]), None);
    let s = visual::latency_summary(&[50, 10, 30, 20, 40]).unwrap();
    assert_eq!((s.samples, s.p50_us, s.p95_us, s.max_us), (5, 30, 50, 50));
    let mut samples = vec![u64::MAX];
    samples.extend([1; 512]);
    let s = visual::latency_summary(&samples).unwrap();
    assert_eq!((s.samples, s.max_us), (512, 1));
}

#[test]
fn unprobed_layers_do_not_become_observations_from_a_later_receipt() {
    let mut view = visual::fixture();
    view.illustrative = false;
    let surface = visual::frame(&view, 120, 32, ColorDepth::TrueColor);
    // Wide pipeline starts at x14, spans 98 cells and has six checkpoints.
    let x = |stage: u16| 14 + 98 * stage / 5;
    for stage in [1, 2, 3] {
        assert_eq!(
            surface.get(x(stage), 7).unwrap().glyph.grapheme.as_str(),
            "?"
        );
    }
    for stage in [0, 4, 5] {
        assert_eq!(
            surface.get(x(stage), 7).unwrap().glyph.grapheme.as_str(),
            "◆"
        );
    }
    assert!(text(&surface).contains("not byte identity"));
    assert!(text(&surface).contains("not kernel queue depth"));
}

#[test]
fn a_pending_single_key_has_no_invented_decode_or_application_receipt() {
    let mut view = visual::fixture();
    view.illustrative = false;
    view.observed_stages = [true, true, false, true, false, true];
    view.pulses = vec![visual::Pulse {
        id: 9,
        key: "r".into(),
        observed_us: [Some(100), Some(200), None, None, None, None],
    }];
    let surface = visual::frame(&view, 120, 32, ColorDepth::Mono);
    assert_eq!(surface.get(14, 7).unwrap().glyph.grapheme.as_str(), "◆");
    assert_eq!(surface.get(33, 7).unwrap().glyph.grapheme.as_str(), "◆");
    assert_eq!(surface.get(53, 7).unwrap().glyph.grapheme.as_str(), "?");
    assert_eq!(surface.get(72, 7).unwrap().glyph.grapheme.as_str(), "○");
    assert_eq!(surface.get(112, 7).unwrap().glyph.grapheme.as_str(), "○");
    // Seeking before a recorded future receipt must not reveal it early.
    view.pulses[0].observed_us[5] = Some(view.micros + 1);
    assert_eq!(visual::frame(&view, 120, 32, ColorDepth::Mono), surface);
}

#[test]
fn supported_compositions_are_pure_wide_safe_and_capability_aware() {
    let mut view = visual::fixture();
    view.entries.last_mut().unwrap().value = "界e\u{301}\n\u{1b}[31m".into();
    let before = view.clone();
    for (width, height) in [
        (0, 0),
        (1, 1),
        (31, 17),
        (56, 24),
        (80, 24),
        (120, 32),
        (160, 40),
    ] {
        for depth in [
            ColorDepth::TrueColor,
            ColorDepth::Ansi256,
            ColorDepth::Ansi16,
            ColorDepth::Mono,
        ] {
            let a = visual::frame(&view, width, height, depth);
            let b = visual::frame(&view, width, height, depth);
            assert_eq!(view, before, "paint mutated measured data");
            assert_eq!(a, b);
            assert_eq!((a.width, a.height), (width, height));
            assert_eq!(a.cells.len(), usize::from(width) * usize::from(height));
            for y in 0..height {
                for x in 0..width {
                    let cell = a.get(x, y).unwrap();
                    assert!(!cell.glyph.grapheme.chars().any(char::is_control));
                    if cell.is_continuation {
                        assert!(x > 0);
                        assert_eq!(a.get(x - 1, y).unwrap().glyph.display_width, 2);
                    } else if cell.glyph.display_width == 2 {
                        assert!(x + 1 < width);
                        assert!(a.get(x + 1, y).unwrap().is_continuation);
                    }
                    if depth == ColorDepth::Mono {
                        assert!(cell.style.fg.is_none() && cell.style.bg.is_none());
                    }
                }
            }
            if width >= 56 {
                assert!(text(&a).contains("ILLUSTRATIVE"));
                assert!(text(&a).contains("Esc exit"));
            }
            let delta = compute_diff(Some(&a), &b);
            assert_eq!(delta.exact_changed_cell_count(), 0);
            assert_eq!(delta.affected_cell_count(), 0);
            assert!(AnsiCompiler::new().compile(&delta).is_empty());
        }
    }
}

#[test]
fn empty_measurements_never_fabricate_a_latency_or_pending_byte_count() {
    let view = visual::View {
        mode: "Context".into(),
        scenario: "normal".into(),
        bytes_drained: 123,
        controls: "Esc exit".into(),
        ..Default::default()
    };
    let text = text(&visual::frame(&view, 120, 32, ColorDepth::TrueColor));
    assert!(text.contains("Awaiting correlated receipts"));
    assert!(text.contains("0 B EST. unread"));
    assert!(!text.contains("p95 0"));
}

#[test]
fn pending_frame_is_unmeasured_until_a_commit_receipt_and_replay_cannot_see_the_future() {
    let mut view = visual::View {
        micros: 100,
        entries: vec![visual::Entry {
            micros: 90,
            source: "RENDER".into(),
            kind: "FrameBegin".into(),
            value: "0".into(),
        }],
        ..Default::default()
    };
    for (w, h) in [(56, 24), (120, 32)] {
        let before = visual::frame(&view, w, h, ColorDepth::Mono);
        assert!(text(&before).contains("FRAME PENDING / in-flight bytes unmeasured"));
        assert!(text(&before).contains("0 B EST. unread"));
        view.entries.push(visual::Entry {
            micros: 101,
            source: "RENDER".into(),
            kind: "FrameCommitted".into(),
            value: "0:1000".into(),
        });
        assert_eq!(before, visual::frame(&view, w, h, ColorDepth::Mono));
        view.micros = 101;
        assert!(!text(&visual::frame(&view, w, h, ColorDepth::Mono)).contains("FRAME PENDING"));
        view.micros = 100;
        view.entries.pop();
    }
}
