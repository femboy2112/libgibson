//! C137_ACCEPTED_V1 — CI-fast, red-first witness calibration for the acoustic regression lock.
//!
//! The ABSOLUTE freeze (per-level WAV + `score.tsv` SHA-256, rendered by the REAL `c137_song`
//! binary) lives in `docs/fixtures/humanmusic-c137-accepted-v1/SHA256SUMS` and is checked
//! out-of-band by `scripts/verify-c137-accepted-v1.sh` — the plan's deliberately-added acoustic
//! step (`cargo test` does not render audio).
//!
//! THIS test is the calibration that makes a green lock mean something: it proves, without
//! rendering, that the audible-content witness (`Score::fingerprint`) actually BITES on a musical
//! change (positive controls) and is correctly BLIND to render-irrelevant metadata (the negative
//! control) — which is precisely why the lock ALSO pins tempo in `MANIFEST.tsv` and freezes the
//! rendered WAV bytes, not just the Score fingerprint.

#[allow(dead_code)]
#[path = "../examples/c137_song/source.rs"]
mod source;

use gibson::audio::human_music::{kernel, score::Score, theory::Mode, MusicWorld};

/// The validated long-form world the `c137_song` example renders under: A-Aeolian, sevenths and
/// modal mixture, 144 BPM.
fn world() -> MusicWorld {
    let mut world = MusicWorld::black_ice();
    world.tempo_bpm = 144.0;
    world.tonic_pc = 9;
    world.mode = Mode::Aeolian;
    world.use_sevenths = true;
    world.allow_modal_mixture = true;
    world
}

/// An accepted long-form Score, committed through the real kernel path.
fn reference_score(seed: u64) -> Score {
    kernel::perform_song(&source::source(seed), &world())
        .expect("accepted longform source must commit")
        .score()
        .clone()
}

#[test]
fn the_reference_fingerprint_is_deterministic_and_seed_distinct() {
    // Determinism is the precondition for any pin: two builds of the same source agree.
    assert_eq!(
        reference_score(2112).fingerprint(),
        reference_score(2112).fingerprint()
    );
    // And the three accepted seeds are genuinely distinct music, not one tune thrice.
    let (a, b, c) = (
        reference_score(2112).fingerprint(),
        reference_score(7).fingerprint(),
        reference_score(99).fingerprint(),
    );
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_ne!(a, c);
}

// ---- Positive controls: a genuine musical change MUST move the audible fingerprint. ----

#[test]
fn transposing_a_note_moves_the_fingerprint() {
    let base = reference_score(2112);
    let mut m = base.clone();
    assert!(!m.notes.is_empty(), "reference must carry pitched notes");
    m.notes[0].pitch += 1;
    assert_ne!(
        base.fingerprint(),
        m.fingerprint(),
        "a transposed note must be audible to the witness"
    );
}

#[test]
fn shifting_an_onset_moves_the_fingerprint() {
    let base = reference_score(2112);
    let mut m = base.clone();
    m.notes[0].start_beat += 0.25;
    assert_ne!(
        base.fingerprint(),
        m.fingerprint(),
        "a displaced onset must be audible to the witness"
    );
}

#[test]
fn changing_a_dynamic_moves_the_fingerprint() {
    let base = reference_score(2112);
    let mut m = base.clone();
    m.notes[0].velocity += 0.1;
    assert_ne!(
        base.fingerprint(),
        m.fingerprint(),
        "a changed note dynamic must be audible to the witness"
    );
}

#[test]
fn dropping_a_drum_moves_the_fingerprint() {
    let base = reference_score(2112);
    let mut m = base.clone();
    assert!(!m.drums.is_empty(), "reference must carry drum hits");
    m.drums.remove(0);
    assert_ne!(
        base.fingerprint(),
        m.fingerprint(),
        "a dropped drum hit must be audible to the witness"
    );
}

#[test]
fn changing_the_harmony_moves_the_fingerprint() {
    let base = reference_score(2112);
    let mut m = base.clone();
    assert!(
        !m.chords.is_empty(),
        "reference must carry a harmonic chart"
    );
    m.chords.pop();
    assert_ne!(
        base.fingerprint(),
        m.fingerprint(),
        "a changed chord must be audible to the witness"
    );
}

// ---- Negative control: render-irrelevant metadata must NOT move the fingerprint. ----
// This is the "harmless metadata-only mutation passes its intentionally narrower witness" the lock
// plan calls for — and it documents the gap the WAV layer exists to close.

#[test]
fn tempo_and_sections_are_outside_the_audible_fingerprint() {
    let base = reference_score(2112);
    let fp = base.fingerprint();

    let mut t = base.clone();
    t.tempo_bpm += 12.0;
    assert_eq!(
        fp,
        t.fingerprint(),
        "Score::fingerprint is tempo-blind by construction"
    );

    let mut s = base.clone();
    s.sections.clear();
    assert_eq!(
        fp,
        s.fingerprint(),
        "Score::fingerprint does not cover sections"
    );

    // Corollary: a TEMPO regression is Score-fingerprint-silent. It can only be caught by the WAV
    // SHA-256 layer (scripts/verify-c137-accepted-v1.sh) plus the pinned tempo in MANIFEST.tsv.
}
