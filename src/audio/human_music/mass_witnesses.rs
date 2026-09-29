//! Round XIII pre-intervention falsifiers: pitch justification must depend on temporal exposure.
//!
//! Each witness asserts a contract the Round XII observer cannot satisfy, because its every
//! window is written in beats and no row carries time. They are ignored until the exposure
//! overlay lands; `docs/fixtures/humanmusic-r13/baseline-failures.txt` records the failing run.
use super::composer::Composer;
use super::context::analyze;
use super::form::SectionKind;
use super::functor::perform_temporal;
use super::harmony::ChordSpan;
use super::performance::{PerformanceOptions, PerformancePlan};
use super::score::{Note, PitchFunction as F, Provenance, Role, Score};
use super::semantic::deflected_lift_trace;
use super::song::SongMap;
use super::temporal::{PitchTrajectoryProof, TemporalPitchDiagnostics as Audit};
use super::theory::{Chord, Mode, Quality, Scale};
use super::world::MusicWorld;

/// A C-major fixture at `bpm` whose harmonies are exactly `chords` (beat, quality, root).
fn fixture(chords: &[(f64, Quality, i32)], bpm: f32) -> (PerformancePlan, Score) {
    let song = SongMap::compose(
        &deflected_lift_trace(24.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let mut perf = PerformancePlan::from_song(
        &song,
        &MusicWorld::swiss_signal(),
        PerformanceOptions::default(),
    );
    perf.chords = chords
        .iter()
        .enumerate()
        .map(|(i, &(beat, quality, root))| {
            ChordSpan::test(
                beat,
                (chords.get(i + 1).map_or(24.0, |x| x.0) - beat) as f32,
                Chord::new(root, quality),
            )
        })
        .collect();
    perf.contexts = analyze(&perf.chords, &Scale::new(0, Mode::Ionian));
    let mut score = Score::new(bpm, 4.0, 24.0);
    score.chords = perf.chords.clone();
    (perf, score)
}

fn note(beat: f64, dur: f32, pitch: i32, role: Role, function: F) -> Note {
    let mut n = Note::new(beat, dur, pitch, 0.8, role, Provenance::new(SectionKind::A));
    n.function = Some(function);
    n
}

/// What a listener-facing verdict could differ in: the path verdict and its stated reasons.
fn verdict(r: &PitchTrajectoryProof) -> (String, bool, Vec<String>) {
    (
        format!("{:?} {:?}", r.status, r.extension),
        r.suspect,
        r.reasons.clone(),
    )
}

/// The same chromatic line C–C#–D–D#–E into a written E, as a fast slide or a slow staircase.
fn chromatic_line(slow: bool) -> Vec<Note> {
    let (start, step, dur) = if slow {
        (0.0, 1.0, 0.95)
    } else {
        (3.0, 0.25, 0.2)
    };
    [
        (48, F::ChordTone),
        (49, F::ChromaticApproach),
        (50, F::LicensedExtension),
        (51, F::ChromaticApproach),
    ]
    .iter()
    .enumerate()
    .map(|(i, &(p, f))| note(start + i as f64 * step, dur, p, Role::Bass, f))
    .chain(std::iter::once(note(
        4.0,
        1.0,
        52,
        Role::Bass,
        F::ChordTone,
    )))
    .collect()
}

/// SWISS witnesses A/B: the keys D5 struck at beats 32 and 88 (the maintainer's 15–17 s and
/// 43–45 s) must be judged heavier than the offbeat D5 comp stab at 30.75 over the same harmony.
#[test]
#[ignore = "R13 witness: fails until temporal exposure is measured"]
fn r13_swiss_witnesses_outweigh_the_offbeat_stab() {
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let c = perform_temporal(
        &song,
        &MusicWorld::swiss_signal(),
        PerformanceOptions::default(),
    );
    let audit = Audit::measure(&c.perf, &c.score);
    let row = |beat: f64| {
        let i = c
            .score
            .notes
            .iter()
            .position(|n| n.role == Role::Keys && n.pitch == 74 && n.start_beat == beat)
            .expect("frozen Round XII keys D5");
        verdict(&audit.rows[i])
    };
    let stab = row(30.75);
    for witness in [32.0, 88.0] {
        assert_ne!(
            row(witness),
            stab,
            "beat {witness} is indistinguishable from the stab"
        );
    }
}

/// Identical beat geometry at 160/118/88 BPM must not receive identical temporal semantics.
#[test]
#[ignore = "R13 witness: fails until temporal exposure is measured"]
fn r13_tempo_dilation_changes_the_proof() {
    let at = |bpm: f32| {
        let (p, mut s) = fixture(&[(0.0, Quality::Maj6, 0), (8.0, Quality::Maj7, 0)], bpm);
        s.notes = vec![note(1.0, 1.0, 74, Role::Pad, F::LicensedExtension)];
        verdict(&Audit::measure(&p, &s).rows[0])
    };
    assert_ne!(at(160.0), at(88.0), "tempo left the proof unchanged");
}

/// The flagship: the same chromatic pitches as a fast slide inherit the gesture; as a slow
/// staircase the C# is heard on its own and owes its own justification. Round XII gives both
/// forms one verdict (its orphan-colour suspicion of the D reaches the C# at any speed).
#[test]
#[ignore = "R13 witness: fails until temporal exposure is measured"]
fn r13_slow_staircase_owes_its_own_justification() {
    let run = |slow: bool| {
        let (p, mut s) = fixture(&[(0.0, Quality::Maj, 0)], 118.0);
        s.notes = chromatic_line(slow);
        verdict(&Audit::measure(&p, &s).rows[1])
    };
    assert_ne!(
        run(false),
        run(true),
        "fast slide and slow staircase judged alike"
    );
}

/// A short weak D stab over C6 and the same D held at the harmony's arrival must differ; the
/// same held D over a harmony that writes the ninth must remain clean.
#[test]
#[ignore = "R13 witness: fails until temporal exposure is measured"]
fn r13_long_orphan_ninth_is_not_a_short_stab() {
    let one = |quality: Quality, n: Note| {
        let (p, mut s) = fixture(&[(0.0, quality, 0), (8.0, Quality::Maj7, 0)], 118.0);
        s.notes = vec![n];
        Audit::measure(&p, &s).rows[0].clone()
    };
    let short = one(
        Quality::Maj6,
        note(1.75, 0.2, 74, Role::Keys, F::LicensedExtension),
    );
    for role in [Role::Keys, Role::Pad] {
        let long = one(
            Quality::Maj6,
            note(0.0, 3.9, 74, role, F::LicensedExtension),
        );
        assert_ne!(
            verdict(&long),
            verdict(&short),
            "{role:?} hold equals a stab"
        );
        let authored = one(Quality::Add9, note(0.0, 3.9, 74, role, F::ChordTone));
        assert!(!authored.suspect, "{}", authored.detail);
    }
}
