//! RXIV Gate D1–D3 — capture writer/reader closure, aggregate budget, strict files, atomic commit.
//!
//! RED recorded against the RXIII capture (5/5): the writer emitted a zero- and a NaN-cell-width
//! frame the reader then rejected (`DegenerateGeometry`); the default limits admitted
//! `65536 × 2^20 × 8` = 512 GiB of sample storage; trailing garbage read back as a valid capture;
//! and the `File::create` + write pattern destroyed a previous good capture when a write failed
//! (leaving a 117-byte truncated file).
//!
//! Every budget here is TINY so each refusal is proven without allocating large payloads.

#[path = "../examples/wire_instrument/capture.rs"]
mod capture;

use capture::{
    atomic_write, content_digest, read_capture, read_capture_with, validate_capture, write_capture,
    write_capture_file, write_capture_with, CaptureError, CaptureLimits, CapturedFrame,
};
use std::io;
use std::path::PathBuf;

fn frame(cols: u32, rows: u32, masked: bool) -> CapturedFrame {
    let n = (cols * rows) as usize;
    CapturedFrame {
        source: 1,
        epoch: 1,
        sequence: 1,
        model_tick: 1,
        cols,
        rows,
        origin_x: -2.5,
        origin_y: 4.0,
        cell_w: 0.5,
        cell_h: 1.25,
        data: (0..n).map(|i| i as f64 * 0.25).collect(),
        mask: masked.then(|| (0..n).map(|i| i % 3 != 0).collect()),
    }
}

fn tight(max_frames: usize, max_field_cells: usize, max_total_bytes: usize) -> CaptureLimits {
    CaptureLimits {
        max_frames,
        max_field_cells,
        max_total_bytes,
    }
}

/// Bitwise equality (NaN-safe) via the content checksum plus the identity/geometry fields.
fn same(a: &[CapturedFrame], b: &[CapturedFrame]) -> bool {
    content_digest(a) == content_digest(b) && a.len() == b.len()
}

// ---------------------------------------------------------------------------------------------
// D1 — write Ok ⇒ read Ok (same limits), and a refused write emits NOTHING
// ---------------------------------------------------------------------------------------------

#[test]
fn the_writer_refuses_every_geometry_the_reader_rejects_and_emits_nothing() {
    let cases: Vec<(&str, CapturedFrame)> = vec![
        (
            "zero cell width",
            CapturedFrame {
                cell_w: 0.0,
                ..frame(2, 2, false)
            },
        ),
        (
            "NaN cell width",
            CapturedFrame {
                cell_w: f64::NAN,
                ..frame(2, 2, false)
            },
        ),
        (
            "negative cell height",
            CapturedFrame {
                cell_h: -1.0,
                ..frame(2, 2, false)
            },
        ),
        (
            "infinite origin",
            CapturedFrame {
                origin_x: f64::INFINITY,
                ..frame(2, 2, false)
            },
        ),
        (
            "zero columns",
            CapturedFrame {
                cols: 0,
                data: vec![],
                ..frame(2, 2, false)
            },
        ),
    ];
    for (name, f) in cases {
        let mut buf = Vec::new();
        let w = write_capture(&mut buf, std::slice::from_ref(&f));
        assert!(
            matches!(w, Err(CaptureError::DegenerateGeometry)),
            "{name}: writer must refuse, got {w:?}"
        );
        assert!(
            buf.is_empty(),
            "{name}: a refused capture emits no bytes ({} written)",
            buf.len()
        );
    }
}

#[test]
fn the_writer_enforces_the_same_budgets_the_reader_does() {
    let l = tight(2, 4, 1000);
    let mut buf = Vec::new();
    assert!(matches!(
        write_capture_with(&mut buf, &[frame(3, 2, false)], &l),
        Err(CaptureError::CellsExceedBudget {
            declared: 6,
            cap: 4
        })
    ));
    assert!(matches!(
        write_capture_with(&mut buf, &vec![frame(1, 1, false); 3], &l),
        Err(CaptureError::FramesExceedBudget {
            declared: 3,
            cap: 2
        })
    ));
    let l = tight(8, 100, 100);
    assert!(matches!(
        write_capture_with(&mut buf, &[frame(2, 2, false), frame(3, 3, false)], &l),
        Err(CaptureError::AggregateExceedsBudget {
            declared: 104,
            cap: 100
        })
    ));
    assert!(buf.is_empty());
}

/// The closure law over a seeded sweep: every combination of hostile geometry values, dimensions
/// and masks. Whenever the writer accepts, the reader (same limits) decodes the identical content;
/// whenever it refuses, nothing was written.
#[test]
fn closure_law_write_ok_implies_read_ok_over_a_seeded_sweep() {
    let reals = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        1e-300,
        1e300,
        f64::MIN_POSITIVE,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    let sane = [0.5, 1.0, 2.0, 1e-3, -3.25];
    let dims = [(0u32, 3u32), (1, 1), (2, 3), (4, 4), (5, 1)];
    let limits = tight(4, 12, 1 << 12);
    let mut seed: u64 = 0x5eed_2026_1105;
    let mut next = |n: usize| {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) as usize % n
    };
    let (mut accepted, mut refused) = (0, 0);
    for _ in 0..4000 {
        let nframes = 1 + next(3);
        let frames: Vec<CapturedFrame> = (0..nframes)
            .map(|_| {
                let (cols, rows) = dims[next(dims.len())];
                let mut f = frame(cols, rows, next(2) == 0);
                // Each geometry value is hostile one time in four, otherwise ordinary.
                let pick = |n: &mut dyn FnMut(usize) -> usize| {
                    if n(4) == 0 {
                        reals[n(reals.len())]
                    } else {
                        sane[n(sane.len())]
                    }
                };
                f.origin_x = pick(&mut next);
                f.origin_y = pick(&mut next);
                f.cell_w = pick(&mut next);
                f.cell_h = pick(&mut next);
                if next(4) == 0 && !f.data.is_empty() {
                    f.data[0] = reals[next(reals.len())]; // non-finite SAMPLES are legal content
                }
                f
            })
            .collect();
        let mut buf = Vec::new();
        match write_capture_with(&mut buf, &frames, &limits) {
            Ok(()) => {
                accepted += 1;
                let back = read_capture_with(&mut buf.as_slice(), &limits)
                    .unwrap_or_else(|e| panic!("writer accepted, reader rejected: {e}"));
                assert!(same(&back, &frames));
                let mut again = Vec::new();
                write_capture_with(&mut again, &back, &limits).unwrap();
                assert_eq!(
                    again, buf,
                    "a strictly-read file is byte-identical on rewrite"
                );
            }
            Err(_) => {
                refused += 1;
                assert!(buf.is_empty(), "a refused capture emitted bytes");
                assert!(validate_capture(&frames, &limits).is_err());
            }
        }
    }
    eprintln!("closure sweep: accepted {accepted}, refused {refused}");
    assert!(
        accepted > 300 && refused > 300,
        "the sweep exercised both arms (accepted {accepted}, refused {refused})"
    );
}

// ---------------------------------------------------------------------------------------------
// D2 — the aggregate budget, charged BEFORE allocation, with checked arithmetic
// ---------------------------------------------------------------------------------------------

#[test]
fn individually_legal_frames_whose_aggregate_exceeds_the_budget_are_refused() {
    // Each 10-cell frame is within the per-frame cap; three of them (240 data bytes) are not within
    // a 200-byte session budget.
    let frames = vec![frame(5, 2, false); 3];
    let mut buf = Vec::new();
    write_capture_with(&mut buf, &frames, &tight(8, 10, 1 << 20)).unwrap();
    assert!(matches!(
        read_capture_with(&mut buf.as_slice(), &tight(8, 10, 200)),
        Err(CaptureError::AggregateExceedsBudget {
            declared: 240,
            cap: 200
        })
    ));
    // Mask bytes count too: two masked 10-cell frames = 2 × (80 + 10) = 180 > 170.
    let masked = vec![frame(5, 2, true); 2];
    let mut buf = Vec::new();
    write_capture_with(&mut buf, &masked, &tight(8, 10, 1 << 20)).unwrap();
    assert!(matches!(
        read_capture_with(&mut buf.as_slice(), &tight(8, 10, 170)),
        Err(CaptureError::AggregateExceedsBudget {
            declared: 180,
            cap: 170
        })
    ));
    assert_eq!(
        read_capture_with(&mut buf.as_slice(), &tight(8, 10, 180))
            .unwrap()
            .len(),
        2,
        "exactly at the budget is admitted"
    );
}

#[test]
fn the_aggregate_is_charged_before_the_payload_is_read_or_allocated() {
    // Truncate the stream right after frame 2 declares its data length: if the budget were charged
    // after reading, this would be `Truncated`; it is the budget refusal, so the check came first.
    let frames = vec![frame(5, 2, false); 2];
    let mut buf = Vec::new();
    write_capture_with(&mut buf, &frames, &tight(8, 10, 1 << 20)).unwrap();
    let one_frame = (buf.len() - 12) / 2;
    let header_of_frame2 = 12 + one_frame + 8 + 4 + 8 + 8 + 4 + 4 + 32 + 4; // through its dlen
    let cut = &buf[..header_of_frame2];
    assert!(matches!(
        read_capture_with(&mut &cut[..], &tight(8, 10, 100)),
        Err(CaptureError::AggregateExceedsBudget {
            declared: 160,
            cap: 100
        })
    ));
}

#[test]
fn the_default_aggregate_bounds_what_the_per_item_caps_alone_could_not() {
    let l = CaptureLimits::default();
    let per_item_worst = (l.max_frames as u128) * (l.max_field_cells as u128) * 9;
    assert!(
        per_item_worst > 512 * (1u128 << 30),
        "per-item caps alone: {per_item_worst} bytes"
    );
    assert_eq!(
        l.max_total_bytes,
        256 << 20,
        "the aggregate ceiling is 256 MiB"
    );
}

// ---------------------------------------------------------------------------------------------
// D3 — strict files, versioning, atomic commit
// ---------------------------------------------------------------------------------------------

fn scratch_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rxiv-d3-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn dir_entries(d: &PathBuf) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(d)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn a_refused_capture_file_never_touches_the_existing_one() {
    let d = scratch_dir("refused");
    let path = d.join("session.cap");
    let good = vec![frame(2, 2, true)];
    write_capture_file(&path, &good, &CaptureLimits::default()).unwrap();
    let before = std::fs::read(&path).unwrap();

    let bad = vec![
        frame(2, 2, false),
        CapturedFrame {
            data: vec![0.0],
            ..frame(2, 2, false)
        },
    ];
    assert!(write_capture_file(&path, &bad, &CaptureLimits::default()).is_err());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "the previous capture survives byte-for-byte"
    );
    assert_eq!(
        dir_entries(&d),
        vec!["session.cap"],
        "no temporary left behind"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn an_io_failure_mid_write_leaves_the_target_untouched_and_no_temporary() {
    let d = scratch_dir("midwrite");
    let path = d.join("trace.bin");
    std::fs::write(&path, b"PREVIOUS").unwrap();
    let r = atomic_write(&path, |w| {
        w.write_all(b"half of a new fi")?;
        Err(io::Error::other("disk full (scripted)"))
    });
    assert!(r.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"PREVIOUS");
    assert_eq!(dir_entries(&d), vec!["trace.bin"]);

    // And a failed FIRST write leaves no file at all where none existed.
    let fresh = d.join("fresh.cap");
    let r = atomic_write(&fresh, |_| Err(io::Error::other("refused before writing")));
    assert!(r.is_err());
    assert!(!fresh.exists(), "no apparently-valid partial file appears");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_successful_atomic_write_replaces_the_file_with_an_exactly_readable_capture() {
    let d = scratch_dir("ok");
    let path = d.join("session.cap");
    std::fs::write(&path, b"OLD").unwrap();
    let frames = vec![frame(3, 2, true), frame(1, 4, false)];
    write_capture_file(&path, &frames, &CaptureLimits::default()).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let back = read_capture(&mut bytes.as_slice()).unwrap();
    assert!(same(&back, &frames));
    let mut rewritten = Vec::new();
    write_capture(&mut rewritten, &back).unwrap();
    assert_eq!(rewritten, bytes, "strict read ⇒ byte-identical file");
    assert_eq!(dir_entries(&d), vec!["session.cap"]);
    let _ = std::fs::remove_dir_all(&d);
}
