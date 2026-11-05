//! RXIII Checkpoint A / Gate B1 — the capture parser treated as adversarial.
//!
//! The pre-RXIII `read_capture` read a `u32` frame count and `Vec::with_capacity`'d it, read each
//! field's `dlen` and `with_capacity`'d THAT, and never compared the data count to `cols × rows`,
//! never validated the geometry, and accepted any nonzero mask tag — so a lying header could demand
//! an unbounded reservation and an internally-inconsistent frame was accepted as valid. These tests
//! pin the hardened contract: every declared length is checked against a [`CaptureLimits`] budget and
//! against the geometry BEFORE allocation, and a malformed stream is a typed [`CaptureError`], never
//! an `Ok`. Budgets are deliberately tiny — the refusals are proven without allocating anything big.

#[path = "../examples/wire_instrument/capture.rs"]
mod capture;

use capture::{
    read_capture, read_capture_with, write_capture, CaptureError, CaptureLimits, CapturedFrame,
};

// ---- raw-stream builders: hand-forge bytes write_capture would refuse to emit ------------------

fn hdr(n: u32) -> Vec<u8> {
    let mut v = b"WIRECAP1".to_vec();
    v.extend_from_slice(&n.to_le_bytes());
    v
}

/// A fully controllable on-disk frame, so a test can inject an inconsistent `dlen`, a bad mask tag,
/// degenerate geometry, etc. Field order matches `read_capture` exactly.
#[allow(clippy::too_many_arguments)]
fn raw_frame(
    cols: u32,
    rows: u32,
    cell_w: f64,
    cell_h: f64,
    origin: (f64, f64),
    dlen: u32,
    data: &[f64],
    mask: Option<(u32, Vec<u8>)>,
) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&1u64.to_le_bytes()); // source
    v.extend_from_slice(&1u32.to_le_bytes()); // epoch
    v.extend_from_slice(&1u64.to_le_bytes()); // sequence
    v.extend_from_slice(&1u64.to_le_bytes()); // model_tick
    v.extend_from_slice(&cols.to_le_bytes());
    v.extend_from_slice(&rows.to_le_bytes());
    v.extend_from_slice(&origin.0.to_bits().to_le_bytes());
    v.extend_from_slice(&origin.1.to_bits().to_le_bytes());
    v.extend_from_slice(&cell_w.to_bits().to_le_bytes());
    v.extend_from_slice(&cell_h.to_bits().to_le_bytes());
    v.extend_from_slice(&dlen.to_le_bytes());
    for &x in data {
        v.extend_from_slice(&x.to_bits().to_le_bytes());
    }
    match mask {
        None => v.push(0),
        Some((mlen, bytes)) => {
            v.push(1);
            v.extend_from_slice(&mlen.to_le_bytes());
            v.extend_from_slice(&bytes);
        }
    }
    v
}

/// A well-formed single-frame capture: 2×2, unit cells, four samples, no mask.
fn good_frame() -> CapturedFrame {
    CapturedFrame {
        source: 1,
        epoch: 1,
        sequence: 1,
        model_tick: 1,
        cols: 2,
        rows: 2,
        origin_x: 0.0,
        origin_y: 0.0,
        cell_w: 1.0,
        cell_h: 1.0,
        data: vec![0.0, 1.0, 2.0, 3.0],
        mask: None,
    }
}

const GENEROUS: CaptureLimits = CaptureLimits {
    max_frames: 16,
    max_field_cells: 1 << 20,
};

// ---- positive / negative controls -------------------------------------------------------------

/// Negative control: a well-formed capture is accepted under a TINY budget — the hardening rejects
/// hostile input, not legitimate input.
#[test]
fn a_valid_capture_round_trips_under_a_tiny_budget() {
    let frames = vec![good_frame()];
    let mut buf = Vec::new();
    write_capture(&mut buf, &frames).expect("write a valid capture");
    let tiny = CaptureLimits {
        max_frames: 4,
        max_field_cells: 16,
    };
    let back = read_capture_with(&mut buf.as_slice(), &tiny).expect("read within budget");
    assert_eq!(back, frames, "a legitimate capture survives a tight budget");
}

// ---- unbounded-allocation refusals ------------------------------------------------------------

/// A declared frame count beyond the budget is refused outright — before any per-frame allocation.
#[test]
fn a_frame_count_beyond_budget_is_refused_without_allocating() {
    let stream = hdr(1_000_000); // header only; the reader bails before it needs a single frame byte
    let tiny = CaptureLimits {
        max_frames: 4,
        max_field_cells: 16,
    };
    match read_capture_with(&mut stream.as_slice(), &tiny) {
        Err(CaptureError::FramesExceedBudget { declared, cap }) => {
            assert_eq!(declared, 1_000_000);
            assert_eq!(cap, 4);
        }
        other => panic!("expected FramesExceedBudget, got {other:?}"),
    }
}

/// A frame count within budget but a file too short to back it truncates fast — the capped
/// `with_capacity` cannot over-read; the per-frame loop hits EOF and reports it.
#[test]
fn a_within_budget_count_with_a_short_file_truncates_rather_than_over_reading() {
    let stream = hdr(10_000); // claims 10k frames, provides zero
    let limits = CaptureLimits {
        max_frames: 1 << 16,
        max_field_cells: 16,
    };
    assert!(matches!(
        read_capture_with(&mut stream.as_slice(), &limits),
        Err(CaptureError::Truncated)
    ));
}

/// A single field whose `cols × rows` exceeds the cell budget is refused before its payload is read.
#[test]
fn a_field_exceeding_the_cell_budget_is_refused() {
    let mut stream = hdr(1);
    stream.extend(raw_frame(100, 100, 1.0, 1.0, (0.0, 0.0), 0, &[], None)); // 10_000 cells
    let tiny = CaptureLimits {
        max_frames: 16,
        max_field_cells: 16,
    };
    match read_capture_with(&mut stream.as_slice(), &tiny) {
        Err(CaptureError::CellsExceedBudget { declared, cap }) => {
            assert_eq!(declared, 10_000);
            assert_eq!(cap, 16);
        }
        other => panic!("expected CellsExceedBudget, got {other:?}"),
    }
}

// ---- consistency refusals (budget generous, so the rejection is the inconsistency) -------------

/// The data count must equal `cols × rows`.
#[test]
fn a_data_count_disagreeing_with_the_geometry_is_rejected() {
    let mut stream = hdr(1);
    // 2×2 = 4 cells, but declare and supply only 3 samples.
    stream.extend(raw_frame(
        2,
        2,
        1.0,
        1.0,
        (0.0, 0.0),
        3,
        &[0.0, 1.0, 2.0],
        None,
    ));
    assert!(matches!(
        read_capture_with(&mut stream.as_slice(), &GENEROUS),
        Err(CaptureError::DataLengthMismatch { data: 3, cells: 4 })
    ));
}

/// A present mask must match the data count.
#[test]
fn a_mask_length_disagreeing_with_the_data_is_rejected() {
    let mut stream = hdr(1);
    stream.extend(raw_frame(
        2,
        2,
        1.0,
        1.0,
        (0.0, 0.0),
        4,
        &[0.0, 1.0, 2.0, 3.0],
        Some((3, vec![1, 0, 1])), // mask of 3 for data of 4
    ));
    assert!(matches!(
        read_capture_with(&mut stream.as_slice(), &GENEROUS),
        Err(CaptureError::MaskLengthMismatch { mask: 3, data: 4 })
    ));
}

/// The mask-presence tag is strict: neither 0 nor 1 is rejected.
#[test]
fn an_invalid_mask_tag_is_rejected() {
    let mut stream = hdr(1);
    let mut frame = raw_frame(2, 2, 1.0, 1.0, (0.0, 0.0), 4, &[0.0, 1.0, 2.0, 3.0], None);
    *frame.last_mut().unwrap() = 2; // flip the trailing mask tag from 0 to a bogus 2
    stream.extend(frame);
    assert!(matches!(
        read_capture_with(&mut stream.as_slice(), &GENEROUS),
        Err(CaptureError::InvalidMaskTag(2))
    ));
}

/// A mask byte that is neither 0 nor 1 is rejected (strict boolean encoding).
#[test]
fn an_invalid_boolean_byte_in_a_mask_is_rejected() {
    let mut stream = hdr(1);
    stream.extend(raw_frame(
        2,
        2,
        1.0,
        1.0,
        (0.0, 0.0),
        4,
        &[0.0, 1.0, 2.0, 3.0],
        Some((4, vec![0, 1, 2, 0])), // 2 is not a boolean
    ));
    assert!(matches!(
        read_capture_with(&mut stream.as_slice(), &GENEROUS),
        Err(CaptureError::InvalidBooleanByte(2))
    ));
}

// ---- geometry vs. sample non-finiteness (the load-bearing distinction) -------------------------

/// Degenerate geometry is rejected: a zero cell size and a non-finite origin both fail
/// `FieldExtent::new`, the same refusal the live wire applies.
#[test]
fn degenerate_or_non_finite_geometry_is_rejected() {
    // Zero cell width.
    let mut z = hdr(1);
    z.extend(raw_frame(2, 2, 0.0, 1.0, (0.0, 0.0), 4, &[0.0; 4], None));
    assert!(matches!(
        read_capture_with(&mut z.as_slice(), &GENEROUS),
        Err(CaptureError::DegenerateGeometry)
    ));

    // Non-finite origin.
    let mut n = hdr(1);
    n.extend(raw_frame(
        2,
        2,
        1.0,
        1.0,
        (f64::NAN, 0.0),
        4,
        &[0.0; 4],
        None,
    ));
    assert!(matches!(
        read_capture_with(&mut n.as_slice(), &GENEROUS),
        Err(CaptureError::DegenerateGeometry)
    ));

    // Non-finite cell size.
    let mut i = hdr(1);
    i.extend(raw_frame(
        2,
        2,
        f64::INFINITY,
        1.0,
        (0.0, 0.0),
        4,
        &[0.0; 4],
        None,
    ));
    assert!(matches!(
        read_capture_with(&mut i.as_slice(), &GENEROUS),
        Err(CaptureError::DegenerateGeometry)
    ));
}

/// Non-finite SAMPLES, by contrast, are legal content and are accepted verbatim (NaN/±inf bit
/// patterns survive the round-trip). The parser rejects non-finite GEOMETRY, never non-finite data.
#[test]
fn non_finite_samples_are_accepted_and_preserved() {
    let frame = CapturedFrame {
        data: vec![f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1.5],
        ..good_frame()
    };
    let mut buf = Vec::new();
    write_capture(&mut buf, std::slice::from_ref(&frame)).expect("non-finite samples are writable");
    let back = read_capture(&mut buf.as_slice()).expect("non-finite samples are readable");
    assert_eq!(back.len(), 1);
    assert!(back[0].data[0].is_nan());
    assert_eq!(back[0].data[1], f64::INFINITY);
    assert_eq!(back[0].data[2], f64::NEG_INFINITY);
    assert_eq!(back[0].data[3], 1.5);
}

// ---- framing edges ----------------------------------------------------------------------------

/// Bad magic (including a different version suffix) and a truncated header are both rejected.
#[test]
fn bad_magic_and_truncated_headers_are_rejected() {
    let mut wrong = b"WIRECAP2".to_vec();
    wrong.extend_from_slice(&1u32.to_le_bytes());
    assert!(matches!(
        read_capture(&mut wrong.as_slice()),
        Err(CaptureError::BadMagic)
    ));

    let short = b"WIR".to_vec();
    assert!(matches!(
        read_capture(&mut short.as_slice()),
        Err(CaptureError::Truncated)
    ));
}

/// The stream is self-delimiting by frame count: trailing bytes after the declared frames are not
/// consumed and do not corrupt the result (bounded by the count, not by EOF).
#[test]
fn trailing_garbage_after_the_declared_frames_is_ignored() {
    let frames = vec![good_frame()];
    let mut buf = Vec::new();
    write_capture(&mut buf, &frames).unwrap();
    buf.extend_from_slice(b"GARBAGE-NOT-A-FRAME");
    let back = read_capture(&mut buf.as_slice()).expect("reads exactly the declared frames");
    assert_eq!(back, frames, "trailing bytes are ignored, not misparsed");
}

// ---- writer refuses what it cannot represent faithfully ---------------------------------------

/// The writer validates at the boundary rather than emitting a capture it would later reject: a
/// frame whose data count disagrees with its geometry is refused, not silently written.
#[test]
fn the_writer_refuses_a_geometry_inconsistent_frame() {
    let bad = CapturedFrame {
        cols: 2,
        rows: 2,                   // 4 cells
        data: vec![0.0, 1.0, 2.0], // but only 3 samples
        ..good_frame()
    };
    let mut buf = Vec::new();
    assert!(matches!(
        write_capture(&mut buf, std::slice::from_ref(&bad)),
        Err(CaptureError::DataLengthMismatch { data: 3, cells: 4 })
    ));
    assert!(
        buf.len() <= 12,
        "nothing past the header should be emitted for a rejected frame"
    );
}
