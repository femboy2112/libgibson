//! Hermetic tests for the video-ingest cache key and the zero-frame guard. No
//! ffmpeg, no clip, no terminal: we exercise the PURE `cache_fingerprint` (which
//! takes already-extracted metadata) and `KeyedFilm::from_raw` directly, and never
//! touch `run_ffmpeg`/`bake`. All three modules are pulled in at the crate root so
//! the modules' `super::` paths (film -> key, bake -> film) resolve.

#[path = "../examples/temporal_video_compositor/bake.rs"]
mod bake;
#[path = "../examples/temporal_video_compositor/film.rs"]
mod film;
#[path = "../examples/temporal_video_compositor/key.rs"]
mod key;

use bake::cache_fingerprint;
use film::KeyedFilm;

// A fixed baseline; each test perturbs exactly one input and demands the digest move.
const PATH: &str = "/clips/subject.mp4";
const LEN: u64 = 123_456;
const MTIME: u64 = 1_700_000_000_000_000_000;
const START: f32 = 2.0;
const DUR: f32 = 6.0;
const W: u32 = 96;
const H: u32 = 54;
const FPS: f32 = 29.0;

fn baseline() -> String {
    cache_fingerprint(PATH, LEN, MTIME, START, DUR, W, H, FPS)
}

#[test]
fn fingerprint_is_deterministic() {
    // Same anatomy in, same suture out — every time, or the cache is a coin flip.
    assert_eq!(baseline(), baseline());
    assert_eq!(
        cache_fingerprint(PATH, LEN, MTIME, START, DUR, W, H, FPS),
        cache_fingerprint(PATH, LEN, MTIME, START, DUR, W, H, FPS)
    );
}

#[test]
fn source_identity_changes_the_fingerprint() {
    let base = baseline();
    // A different file behind the same name must not reuse a stale bake.
    assert_ne!(
        base,
        cache_fingerprint("/clips/other.mp4", LEN, MTIME, START, DUR, W, H, FPS),
        "path must be keyed"
    );
    assert_ne!(
        base,
        cache_fingerprint(PATH, LEN + 1, MTIME, START, DUR, W, H, FPS),
        "byte length must be keyed"
    );
    assert_ne!(
        base,
        cache_fingerprint(PATH, LEN, MTIME + 1, START, DUR, W, H, FPS),
        "mtime must be keyed"
    );
}

#[test]
fn every_spec_field_changes_the_fingerprint() {
    let base = baseline();
    assert_ne!(
        base,
        cache_fingerprint(PATH, LEN, MTIME, START + 1.0, DUR, W, H, FPS),
        "start_s must be keyed"
    );
    assert_ne!(
        base,
        cache_fingerprint(PATH, LEN, MTIME, START, DUR + 1.0, W, H, FPS),
        "dur_s must be keyed"
    );
    assert_ne!(
        base,
        cache_fingerprint(PATH, LEN, MTIME, START, DUR, W + 1, H, FPS),
        "film_w must be keyed"
    );
    assert_ne!(
        base,
        cache_fingerprint(PATH, LEN, MTIME, START, DUR, W, H + 1, FPS),
        "film_h must be keyed"
    );
    assert_ne!(
        base,
        cache_fingerprint(PATH, LEN, MTIME, START, DUR, W, H, FPS + 1.0),
        "fps must be keyed"
    );
}

#[test]
fn nearby_cuts_do_not_collide() {
    // The load-bearing assertion: the old `{:.2}`/`{:.0}` formatting truncated
    // 2.001 and 2.004 to the same "2.00" and served one clip's frames for another.
    // Hashing `f32::to_bits` keeps distinct floats distinct.
    let a = cache_fingerprint(PATH, LEN, MTIME, 2.001, 6.0, W, H, FPS);
    let b = cache_fingerprint(PATH, LEN, MTIME, 2.004, 6.0, W, H, FPS);
    assert_ne!(a, b, "2.001 vs 2.004 start must not collide");

    let c = cache_fingerprint(PATH, LEN, MTIME, 2.0, 6.0, W, H, FPS);
    let d = cache_fingerprint(PATH, LEN, MTIME, 2.0, 6.0001, W, H, FPS);
    assert_ne!(c, d, "6.0 vs 6.0001 dur must not collide");
}

#[test]
fn empty_input_is_rejected() {
    // Zero bytes used to slip through as a valid 0-frame film that panics in
    // frame(0). It must be a clean None instead.
    assert!(KeyedFilm::from_raw(vec![], 4, 4, 29.0).is_none());
}

#[test]
fn one_whole_frame_is_accepted() {
    let f = KeyedFilm::from_raw(vec![0u8; 4 * 4 * 3], 4, 4, 29.0)
        .expect("a single whole frame is a valid film");
    assert_eq!(f.nframes, 1);
}

#[test]
fn non_frame_multiple_length_is_rejected() {
    // One byte short of a whole frame: a truncated bake, still refused.
    assert!(KeyedFilm::from_raw(vec![0u8; 4 * 4 * 3 - 1], 4, 4, 29.0).is_none());
    assert!(KeyedFilm::from_raw(vec![0u8; 4 * 4 * 3 + 5], 4, 4, 29.0).is_none());
}
