use gibson::audio::human_music::{
    composer::Composer,
    fingerprint::{CanonicalFingerprint, FingerprintWriter},
    functor::perform_pocketed,
    performance::PerformanceOptions,
    semantic::deflected_lift_trace,
    song::SongMap,
    theory::Mode,
    MusicWorld,
};
use std::fmt;

struct Presented<'a, T>(&'a T, bool);
impl<T: fmt::Debug> fmt::Debug for Presented<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.1 {
            write!(f, "new display: {:#?}", self.0)
        } else {
            write!(f, "{:?}", self.0)
        }
    }
}
impl<T: CanonicalFingerprint> CanonicalFingerprint for Presented<'_, T> {
    fn encode(&self, w: &mut FingerprintWriter) {
        self.0.encode(w);
    }
}

fn song() -> SongMap {
    SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    )
}

#[test]
fn debug_is_not_semantic_identity() {
    let s = song();
    let a = Presented(&s, false);
    let b = Presented(&s, true);
    assert_ne!(format!("{a:?}"), format!("{b:?}"));
    assert_eq!(a.canonical_fingerprint(), b.canonical_fingerprint());
    assert_ne!(s.fingerprint(), s.canonical_fingerprint());
    assert_eq!(s.fingerprint(), 0xaf53_6044_2b9b_6729);
}

#[test]
fn canonical_primitives_have_unambiguous_boundaries_and_float_rules() {
    assert_eq!(
        0.0f64.canonical_fingerprint(),
        (-0.0f64).canonical_fingerprint()
    );
    assert_eq!(
        f32::NAN.canonical_fingerprint(),
        f32::from_bits(0xffc0_1234).canonical_fingerprint()
    );
    assert_ne!(
        vec!["ab", "c"].canonical_fingerprint(),
        vec!["a", "bc"].canonical_fingerprint()
    );
    assert_ne!(
        vec![1u32, 2].canonical_fingerprint(),
        vec![2u32, 1].canonical_fingerprint()
    );
    assert_ne!(
        None::<u32>.canonical_fingerprint(),
        Some(0u32).canonical_fingerprint()
    );
    assert_ne!(1u32.canonical_fingerprint(), 1i32.canonical_fingerprint());
    assert_eq!(3usize.canonical_fingerprint(), 3u64.canonical_fingerprint());
}

#[test]
fn song_coordinates_move_identity_but_free_seed_does_not() {
    let s = song();
    let original = s.canonical_fingerprint();
    let mutations: &[fn(&mut SongMap)] = &[
        |s| s.frame = Mode::Dorian,
        |s| s.timeline.initial.energy += 0.125,
        |s| s.plan.contract.recurrence_bars += 1,
        |s| s.plan.form.total_beats += 0.25,
        |s| s.plan.discourse.ledger.phrases += 1,
        |s| s.thematic.bank.identity.degrees[0] += 1,
        |s| s.thematic.bank.identity.rhythm[0] += 0.25,
        |s| s.thematic.sites[0].phrase += 1,
        |s| s.harmonic.as_mut().unwrap().bars_per_chord += 1,
        |s| s.phenomenal = None,
    ];
    for (i, mutate) in mutations.iter().enumerate() {
        let mut changed = s.clone();
        mutate(&mut changed);
        assert_ne!(original, changed.canonical_fingerprint(), "mutation {i}");
    }
    let mut free = s.clone();
    free.seed += 1;
    assert_eq!(original, free.canonical_fingerprint());
}

#[test]
fn performance_and_events_have_distinct_explicit_schemas() {
    let c = perform_pocketed(
        &song(),
        &MusicWorld::black_ice(),
        PerformanceOptions::default(),
    );
    let original = c.perf.canonical_fingerprint();
    println!(
        "v2 song={:016x} performance={original:016x} events={:016x}",
        c.song.canonical_fingerprint(),
        c.score.canonical_fingerprint()
    );
    let mut changed = c.perf.clone();
    changed.accent.bars[0][0].structural += 0.1;
    assert_ne!(original, changed.canonical_fingerprint());
    changed = c.perf.clone();
    changed.song_fingerprint ^= 1; // legacy implementation claim is excluded from v2 data
    assert_eq!(original, changed.canonical_fingerprint());
    let mutations: &[fn(&mut gibson::audio::human_music::Score)] = &[
        |s| s.notes[0].pitch += 1,
        |s| s.notes[0].start_beat += 0.25,
        |s| s.notes[0].dur_beats += 0.125,
        |s| s.notes[0].velocity *= 0.9,
        |s| s.notes[0].prov.motif_id = Some(255),
        |s| s.tempo_bpm += 1.0,
        |s| s.total_beats += 0.25,
        |s| s.beats_per_bar = 3.0,
        |s| {
            s.notes.pop();
        },
    ];
    for (i, mutate) in mutations.iter().enumerate() {
        let mut changed = c.score.clone();
        mutate(&mut changed);
        assert_ne!(
            c.score.canonical_fingerprint(),
            changed.canonical_fingerprint(),
            "mutation {i}"
        );
    }
}
