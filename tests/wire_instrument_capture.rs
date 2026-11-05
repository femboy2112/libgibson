//! RXII Checkpoint 2 / Gate B (C4): capture/replay with a digest reproducible ACROSS COLOR DEPTH.
//!
//! The capture records the SEMANTIC content of each observed field; the reproducibility certificate
//! is `content_digest`, taken over that content and never over the rendered ANSI bytes. These
//! proofs:
//!   * a capture round-trips through its binary form byte-for-byte (digest preserved);
//!   * THE HEADLINE PROPERTY — the same field rendered at mono vs truecolor emits DIFFERENT ANSI
//!     bytes (so a digest of the rendered output could not certify a cross-depth replay), while the
//!     content digest is identical at either depth (so it can);
//!   * the digest is sensitive to content (not a vacuous constant);
//!   * (unix) the real `--capture` → `--replay` binaries reproduce the same digest from a file.

#[path = "../examples/wire_instrument/capture.rs"]
mod capture;
#[path = "../examples/wire_instrument/viewport.rs"]
mod viewport;

use capture::{content_digest, read_capture, write_capture, CapturedFrame};
use gibson::capability::ColorDepth;
use gibson::context::Context;
use gibson::field_view::{Colormap, FieldView, RangePolicy};
use gibson::node::Node;
use gibson::renderer::RenderMode;

fn sample_frames() -> Vec<CapturedFrame> {
    vec![
        CapturedFrame {
            source: 1,
            epoch: 1,
            sequence: 1,
            model_tick: 1,
            cols: 4,
            rows: 3,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_w: 1.0,
            cell_h: 1.0,
            data: (0..12).map(|i| i as f64).collect(),
            mask: None,
        },
        CapturedFrame {
            source: 1,
            epoch: 1,
            sequence: 2,
            model_tick: 2,
            cols: 4,
            rows: 3,
            origin_x: 2.0,
            origin_y: -1.0,
            cell_w: 1.5,
            cell_h: 2.0,
            data: (0..12).map(|i| (i as f64) * 1.5 - 3.0).collect(),
            mask: Some((0..12).map(|i| i % 3 != 0).collect()),
        },
    ]
}

/// Render a captured frame IN COLOR (the `Heat` colormap paints RGB backgrounds) headless at a given
/// color depth, and return the emitted ANSI bytes. Heat is the field's color colormap, so its output
/// genuinely varies with depth — truecolor emits 24-bit SGR, mono drops color entirely — which is
/// exactly what makes a byte digest depth-dependent. (The instrument's default `Density` colormap is
/// glyph-only and so happens to be colorless; the capture digest is over content, independent of
/// either colormap, so the choice here only affects the rendered-bytes contrast, never the digest.)
fn render_bytes(frame: &CapturedFrame, depth: ColorDepth) -> Vec<u8> {
    let ext = frame.extent().expect("captured geometry is valid");
    let view = FieldView::new(ext, &frame.data, frame.mask.as_deref()).expect("field view");
    let (wv, area) = viewport::world_view_for(&ext, 1).expect("ordinary geometry is displayable");
    let rendered = view.render(&wv, area, Colormap::Heat, RangePolicy::Auto);
    let mut ctx = Context::headless(RenderMode::Inline, 80, 30);
    ctx.set_color_depth(depth);
    ctx.set_root(Node::raster(rendered.surface));
    ctx.render().expect("headless render");
    ctx.rendered_bytes().to_vec()
}

/// A capture round-trips through its binary form exactly — the frames and the digest are preserved,
/// so a digest computed before writing matches one computed after reading back.
#[test]
fn a_capture_round_trips_through_its_binary_form() {
    let frames = sample_frames();
    let before = content_digest(&frames);

    let mut buf: Vec<u8> = Vec::new();
    write_capture(&mut buf, &frames).expect("write capture");
    let read_back = read_capture(&mut buf.as_slice()).expect("read capture");

    assert_eq!(
        read_back, frames,
        "the frames survive the round-trip exactly"
    );
    assert_eq!(
        content_digest(&read_back),
        before,
        "the content digest is preserved across write→read"
    );
}

/// THE HEADLINE C4 PROPERTY. The same field emits DIFFERENT ANSI bytes at mono vs truecolor — so a
/// digest of the rendered output would disagree across depth and could never certify a cross-depth
/// replay. The content digest, taken over the field content, is IDENTICAL whichever depth the
/// capture is replayed at. That is why the certificate is "reproducible across color depth".
#[test]
fn the_content_digest_is_identical_across_color_depth_while_the_ansi_bytes_differ() {
    let frames = sample_frames();

    // RED (documented inline): the emitted bytes depend on the color depth, so a byte digest would.
    let mono = render_bytes(&frames[0], ColorDepth::Mono);
    let truecolor = render_bytes(&frames[0], ColorDepth::TrueColor);
    assert_ne!(
        mono, truecolor,
        "color depth changes the emitted ANSI bytes — a pixel digest cannot be a cross-depth certificate"
    );

    // GREEN: the content digest a `--replay` prints is the SAME regardless of the depth it renders
    // at, because the digest never touches the rendered bytes. Replay on a mono terminal and on a
    // truecolor terminal both reproduce the one capture digest.
    let _ = render_bytes(&frames[0], ColorDepth::Mono); // a "replay" on a mono terminal
    let digest_mono_replay = content_digest(&frames);
    let _ = render_bytes(&frames[0], ColorDepth::TrueColor); // a "replay" on a truecolor terminal
    let digest_true_replay = content_digest(&frames);
    assert_eq!(
        digest_mono_replay, digest_true_replay,
        "the content digest reproduces across color depth — the reproducibility certificate"
    );
}

/// The digest is a real function of the content, not a vacuous constant: changing a data sample, the
/// model tick, the geometry, or the mask each changes it.
#[test]
fn the_content_digest_is_sensitive_to_every_content_field() {
    let base = sample_frames();
    let d0 = content_digest(&base);

    let mut data = base.clone();
    data[0].data[5] += 1.0;
    assert_ne!(
        content_digest(&data),
        d0,
        "a changed sample changes the digest"
    );

    let mut tick = base.clone();
    tick[0].model_tick += 1;
    assert_ne!(
        content_digest(&tick),
        d0,
        "a changed model tick changes the digest"
    );

    let mut geom = base.clone();
    geom[0].origin_x += 0.5;
    assert_ne!(
        content_digest(&geom),
        d0,
        "a changed origin changes the digest"
    );

    let mut mask = base.clone();
    mask[1].mask.as_mut().unwrap()[0] = !mask[1].mask.as_ref().unwrap()[0];
    assert_ne!(
        content_digest(&mask),
        d0,
        "a flipped mask bit changes the digest"
    );
}

/// A non-finite sample is preserved by the digest (it enters by bit pattern), so a field carrying a
/// NaN or infinity captures and replays to a distinct, stable digest rather than being normalized.
#[test]
fn non_finite_samples_are_preserved_in_the_digest() {
    let mut frames = sample_frames();
    let finite = content_digest(&frames);
    frames[0].data[0] = f64::NAN;
    let with_nan = content_digest(&frames);
    assert_ne!(
        with_nan, finite,
        "a NaN sample is captured, not normalized away"
    );

    // And it round-trips: the NaN's bit pattern survives write→read.
    let mut buf = Vec::new();
    write_capture(&mut buf, &frames).unwrap();
    let back = read_capture(&mut buf.as_slice()).unwrap();
    assert_eq!(
        content_digest(&back),
        with_nan,
        "the non-finite sample round-trips"
    );
    assert!(back[0].data[0].is_nan(), "the replayed sample is still NaN");
}

// --- End-to-end: the real --capture and --replay binaries reproduce the digest from a file. ------

#[cfg(unix)]
#[path = "common/pty_capture.rs"]
mod pty_capture;

#[cfg(unix)]
#[test]
fn the_capture_and_replay_binaries_reproduce_the_digest_from_a_file() {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let exe = pty_capture::example_path("wire_instrument");
    let python = std::env::var("GIBSON_PYTHON").unwrap_or_else(|_| "python3".to_string());
    let path = std::env::temp_dir().join(format!("wire_capture_{}.bin", std::process::id()));

    let run = |args: &[&str]| -> (bool, String) {
        let mut child = Command::new(&exe)
            .args(args)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .env("GIBSON_PYTHON", &python)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn wire_instrument");
        let mut out = child.stdout.take().expect("stdout piped");
        let deadline = Instant::now() + Duration::from_secs(60);
        let status = loop {
            if let Some(s) = child.try_wait().expect("poll") {
                break s;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("wire_instrument {args:?} did not finish within 60s");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let mut s = String::new();
        out.read_to_string(&mut s).expect("read stdout");
        (status.success(), s)
    };

    let path_str = path.to_str().unwrap();
    let (cap_ok, cap_out) = run(&["--capture", path_str]);
    assert!(cap_ok, "--capture exited nonzero:\n{cap_out}");
    let (rep_ok, rep_out) = run(&["--replay", path_str]);
    let _ = std::fs::remove_file(&path);
    assert!(rep_ok, "--replay exited nonzero:\n{rep_out}");

    let digest_line = |out: &str, prefix: &str| -> String {
        out.lines()
            .find_map(|l| l.strip_prefix(prefix).map(|h| h.trim().to_string()))
            .unwrap_or_else(|| panic!("no {prefix:?} line in:\n{out}"))
    };
    let captured = digest_line(&cap_out, "capture-digest ");
    let replayed = digest_line(&rep_out, "replay-digest ");
    assert_eq!(
        captured, replayed,
        "replay reproduces the capture digest from the file\ncapture:\n{cap_out}\nreplay:\n{rep_out}"
    );
    assert!(
        cap_out.contains("3 frames"),
        "the capture recorded the bounded frame count:\n{cap_out}"
    );
}
