//! Round XIII witnesses and controls: pitch justification must depend on temporal exposure.
//!
//! Commit 1 recorded these four witnesses failing against the Round XII observer, which returns
//! identical verdicts for events that differ only in time
//! (`docs/fixtures/humanmusic-r13/baseline-failures.txt`). They now assert the contract on the
//! [`MassDiagnostics`] overlay; `round_xii_observer_is_time_blind` keeps the original blindness
//! pinned, so the overlay's reason to exist stays visible.
use super::composer::Composer;
use super::context::analyze;
use super::form::SectionKind;
use super::functor::perform_temporal;
use super::harmony::ChordSpan;
use super::mass::{
    Accent, ExposureClass as X, MassDiagnostics, MassVerdict as V, TemporalMass, ASSERTED_MIN_SECS,
};
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

/// The mass overlay of `notes` over `chords` at `bpm` with SWISS_SIGNAL's patches.
fn overlay(chords: &[(f64, Quality, i32)], bpm: f32, notes: Vec<Note>) -> MassDiagnostics {
    overlay_in(&MusicWorld::swiss_signal(), chords, bpm, notes)
}

/// The mass overlay with `world`'s patches.
fn overlay_in(
    world: &MusicWorld,
    chords: &[(f64, Quality, i32)],
    bpm: f32,
    notes: Vec<Note>,
) -> MassDiagnostics {
    let (p, mut s) = fixture(chords, bpm);
    s.notes = notes;
    MassDiagnostics::measure(&p, &s, world)
}

/// What a listener-facing Round XII verdict could differ in: path verdict and stated reasons.
fn r12_verdict(r: &PitchTrajectoryProof) -> (String, bool, Vec<String>) {
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

fn stable_song() -> SongMap {
    SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    )
}

/// SWISS witnesses A/B: the keys D5 struck at beats 32 and 88 (the maintainer's 15–17 s and
/// 43–45 s) are asserted, unowned, and heavier than the offbeat D5 comp stab at 30.75. They are
/// percussive pings: their written lengths differ two-fold, their audible lengths do not.
#[test]
fn r13_swiss_witnesses_outweigh_the_offbeat_stab() {
    let world = MusicWorld::swiss_signal();
    let c = perform_temporal(&stable_song(), &world, PerformanceOptions::default());
    let d = MassDiagnostics::measure(&c.perf, &c.score, &world);
    let row = |beat: f64| {
        let i = c
            .score
            .notes
            .iter()
            .position(|n| n.role == Role::Keys && n.pitch == 74 && n.start_beat == beat)
            .expect("frozen Round XII keys D5");
        d.rows[i].clone()
    };
    let stab = row(30.75);
    assert!(
        stab.mass.class < X::Asserted && !stab.mass.fused,
        "{}",
        stab.detail
    );
    assert!(matches!(stab.verdict, V::Carried | V::SoftOrphan));
    let (a, b) = (row(32.0), row(88.0));
    for w in [&a, &b] {
        assert!(w.mass.fused && w.mass.class == X::Asserted, "{}", w.detail);
        assert_eq!(w.verdict, V::UnownedAssertedColor, "{}", w.detail);
        assert!(
            w.mass.mass >= 2.0 * stab.mass.mass,
            "{}{}",
            w.detail,
            stab.detail
        );
    }
    assert!(b.mass.written_secs > 1.9 * a.mass.written_secs);
    assert!(
        (a.mass.audible_secs - b.mass.audible_secs).abs() < 1e-9,
        "a ping is a ping"
    );
}

/// BLACK_ICE's clock exposes the same 3.92-beat support colours for longer than SWISS's.
#[test]
fn r13_black_ice_holds_expose_more_real_time_than_swiss() {
    let song = stable_song();
    let orphan_pad_holds = |world: MusicWorld| {
        let c = perform_temporal(&song, &world, PerformanceOptions::default());
        let d = MassDiagnostics::measure(&c.perf, &c.score, &world);
        d.rows
            .into_iter()
            .filter(|r| {
                let n = &c.score.notes[r.note_index];
                n.role == Role::Pad && (n.dur_beats - 3.92).abs() < 1e-3 && r.is_color
            })
            .map(|r| r.mass)
            .collect::<Vec<TemporalMass>>()
    };
    let swiss = orphan_pad_holds(MusicWorld::swiss_signal());
    let black = orphan_pad_holds(MusicWorld::black_ice());
    assert!(!swiss.is_empty() && !black.is_empty());
    let max = |v: &[TemporalMass], f: fn(&TemporalMass) -> f64| v.iter().map(f).fold(0.0, f64::max);
    let min = |v: &[TemporalMass], f: fn(&TemporalMass) -> f64| {
        v.iter().map(f).fold(f64::INFINITY, f64::min)
    };
    assert!(min(&black, |m| m.written_secs) > max(&swiss, |m| m.written_secs));
    assert!(min(&black, |m| m.audible_secs) > max(&swiss, |m| m.audible_secs));
}

/// Identical beat geometry at 160/118/88 BPM: exposure rises as the tempo falls, and two
/// marginal events change class across the range (real time is load-bearing).
#[test]
fn r13_tempo_dilation_changes_the_proof() {
    // (onset, length) with a following chord-tone attack at the written end masking the tail.
    let event = |bpm: f32, at: f64, len: f32| {
        let d = overlay(
            &[(0.0, Quality::Maj, 0)],
            bpm,
            vec![
                note(at, len, 52, Role::Bass, F::ChordTone),
                note(at + f64::from(len), 0.5, 48, Role::Bass, F::ChordTone),
            ],
        );
        d.rows[0].mass
    };
    let tempos = [160.0, 118.0, 88.0];
    for (at, len) in [(0.5, 0.5), (2.0, 1.0), (1.0, 1.0)] {
        let m: Vec<TemporalMass> = tempos.iter().map(|&t| event(t, at, len)).collect();
        assert!(m[0].mass < m[1].mass && m[1].mass < m[2].mass, "{m:?}");
        assert!(m[0].written_secs < m[1].written_secs && m[1].written_secs < m[2].written_secs);
        println!(
            "tempo-proof onset {at} len {len}: {}",
            m.iter()
                .zip(tempos)
                .map(|(m, t)| format!(
                    "{t} BPM {:.3} s -> {:.3} {:?}",
                    m.audible_secs, m.mass, m.class
                ))
                .collect::<Vec<_>>()
                .join(" | ")
        );
    }
    let classes = |at, len| tempos.map(|t| event(t, at, len).class);
    assert_eq!(classes(0.5, 0.5), [X::Fleeting, X::Gestural, X::Gestural]);
    assert_eq!(classes(2.0, 1.0), [X::Gestural, X::Gestural, X::Asserted]);
}

/// A percussive ping's exposure is its envelope, not its written length or the tempo.
#[test]
fn r13_percussive_ping_does_not_dilate() {
    let ping = |bpm: f32, len: f32| {
        overlay(
            &[(0.0, Quality::Maj6, 0)],
            bpm,
            vec![note(1.5, len, 74, Role::Keys, F::LicensedExtension)],
        )
        .rows[0]
            .mass
    };
    let a = ping(160.0, 0.45);
    for m in [ping(118.0, 0.45), ping(88.0, 0.45), ping(118.0, 3.9)] {
        assert!(
            (m.audible_secs - a.audible_secs).abs() < 1e-9,
            "{m:?} {a:?}"
        );
    }
    assert!(ping(88.0, 3.9).written_secs > 10.0 * a.written_secs);
}

/// The flagship: the same chromatic pitches. Fast, every intermediate inherits the gesture.
/// Slow, the C# (aimed at a non-structural D) is overdrawn, the D loses the slide and is only a
/// soft orphan colour, and the D# earns its place by resolving into the written E.
#[test]
fn r13_slow_staircase_owes_its_own_justification() {
    let run = |slow: bool, bpm: f32| overlay(&[(0.0, Quality::Maj, 0)], bpm, chromatic_line(slow));
    let fast = run(false, 118.0);
    for r in &fast.rows[1..4] {
        assert_eq!(r.mass.class, X::Fleeting, "{}", r.detail);
        assert_eq!(r.verdict, V::Carried, "{}", r.detail);
    }
    let slow = run(true, 118.0);
    let v: Vec<V> = slow.rows[1..4].iter().map(|r| r.verdict).collect();
    assert_eq!(
        v,
        [V::OverdrawnGesture, V::SoftOrphan, V::Carried],
        "{}",
        slow.report()
    );
    let slower = run(true, 88.0);
    let v: Vec<V> = slower.rows[1..4].iter().map(|r| r.verdict).collect();
    assert_eq!(
        v,
        [V::OverdrawnGesture, V::UnownedAssertedColor, V::Carried]
    );
}

/// Short weak orphan ninth: legal colour. The same D held (fused at the arrival, or sustained
/// on a pad from a weak beat) is an accidental reharmonization. Written in the harmony
/// (`Cadd9`), the same long D is authored and carried.
#[test]
fn r13_long_orphan_ninth_is_not_a_short_stab() {
    let one = |quality: Quality, n: Note| {
        overlay(
            &[(0.0, quality, 0), (8.0, Quality::Maj7, 0)],
            118.0,
            vec![n],
        )
        .rows[0]
            .clone()
    };
    let short = one(
        Quality::Maj6,
        note(1.75, 0.2, 74, Role::Keys, F::LicensedExtension),
    );
    assert_eq!(short.mass.class, X::Fleeting, "{}", short.detail);
    assert_eq!(short.verdict, V::Carried);
    for n in [
        note(0.0, 3.9, 74, Role::Keys, F::LicensedExtension),
        note(0.0, 3.9, 74, Role::Pad, F::LicensedExtension),
        note(1.0, 2.9, 74, Role::Pad, F::LicensedExtension),
    ] {
        let long = one(Quality::Maj6, n);
        assert_eq!(long.mass.class, X::Asserted, "{}", long.detail);
        assert_eq!(long.verdict, V::UnownedAssertedColor, "{}", long.detail);
        let mut written = n;
        written.function = Some(F::ChordTone);
        let authored = one(Quality::Add9, written);
        assert_eq!(authored.mass.class, X::Asserted);
        assert_eq!(authored.verdict, V::Carried, "{}", authored.detail);
    }
    // The dwell route alone, off the arrival: not fused, asserted by its own sustained mass.
    let dwelt = one(
        Quality::Maj6,
        note(1.0, 2.9, 74, Role::Pad, F::LicensedExtension),
    );
    assert!(!dwelt.mass.fused && dwelt.mass.mass >= ASSERTED_MIN_SECS);
}

/// High mass is not wrongness: a prepared, physically held, stepwise-resolved suspension on
/// BLACK_ICE's sustaining lead (SWISS's lead is a percussive ping and could not hold one).
#[test]
fn r13_long_suspension_is_carried() {
    let d = overlay_in(
        &MusicWorld::black_ice(),
        &[(0.0, Quality::Dom7, 7), (4.0, Quality::Maj, 0)],
        88.0,
        vec![
            note(2.0, 3.0, 62, Role::Lead, F::Suspension),
            note(5.0, 1.0, 60, Role::Lead, F::ChordTone),
        ],
    );
    let r = &d.rows[0];
    assert_eq!(r.mass.class, X::Asserted, "{}", r.detail);
    assert_eq!(r.verdict, V::Carried, "{}", r.detail);
    assert_eq!(d.base.false_suspensions, 0);
}

/// Four short attacks of one unowned colour carry materially more exposure than one.
#[test]
fn r13_repetition_accumulates_exposure() {
    let pings = |k: usize| {
        let notes = (0..k)
            .map(|i| {
                note(
                    1.25 + 0.5 * i as f64,
                    0.2,
                    74,
                    Role::Keys,
                    F::LicensedExtension,
                )
            })
            .collect();
        let d = overlay(&[(0.0, Quality::Maj6, 0)], 118.0, notes);
        d.rows[k - 1].mass
    };
    let (one, four) = (pings(1), pings(4));
    assert!(four.mass >= 3.5 * one.mass, "{one:?} {four:?}");
    assert!(four.class > one.class);
}

/// Monotonicity: holding everything else constant, more audible time, more recurrence, a
/// stronger accent or onset fusion never reduce mass or class; and more written time never
/// reduces audible time on a sustaining patch.
#[test]
fn r13_mass_is_monotone_in_its_evidence() {
    let accents = [Accent::Weak, Accent::MidBar, Accent::Downbeat];
    let m = |a: f64, r: f64, acc: Accent, fused: bool| {
        TemporalMass::from_evidence(Role::Keys, 1.0, 0.5, a, r, acc, fused, 0.5)
    };
    let grid = [0.0, 0.05, 0.15, 0.25, 0.5, 0.8, 1.6];
    for &a in &grid {
        for &r in &grid {
            for (k, &acc) in accents.iter().enumerate() {
                for fused in [false, true] {
                    let x = m(a, r, acc, fused);
                    let bigger = [
                        m(a + 0.1, r, acc, fused),
                        m(a, r + 0.1, acc, fused),
                        m(a, r, accents[(k + 1).min(2)], fused),
                        m(a, r, acc, true),
                    ];
                    for y in bigger {
                        assert!(y.mass >= x.mass && y.class >= x.class, "{x:?} -> {y:?}");
                    }
                }
            }
        }
    }
    assert!(Accent::Downbeat.salience() > Accent::MidBar.salience());
    assert!(Accent::MidBar.salience() > Accent::Weak.salience());
    let pad = |len: f32| {
        overlay(
            &[(0.0, Quality::Maj6, 0)],
            88.0,
            vec![note(1.0, len, 74, Role::Pad, F::LicensedExtension)],
        )
        .rows[0]
            .mass
    };
    let lens = [0.25, 0.5, 1.0, 2.0, 3.0];
    for w in lens.windows(2) {
        let (x, y) = (pad(w[0]), pad(w[1]));
        assert!(y.audible_secs >= x.audible_secs && y.mass >= x.mass);
    }
}

/// The Round XII observer remains exactly as time-blind as commit 1 recorded (frozen).
#[test]
fn round_xii_observer_is_time_blind() {
    let at = |bpm: f32| {
        let (p, mut s) = fixture(&[(0.0, Quality::Maj6, 0), (8.0, Quality::Maj7, 0)], bpm);
        s.notes = vec![note(1.0, 1.0, 74, Role::Pad, F::LicensedExtension)];
        r12_verdict(&Audit::measure(&p, &s).rows[0])
    };
    assert_eq!(at(160.0), at(88.0));
    let run = |slow: bool| {
        let (p, mut s) = fixture(&[(0.0, Quality::Maj, 0)], 118.0);
        s.notes = chromatic_line(slow);
        r12_verdict(&Audit::measure(&p, &s).rows[1])
    };
    assert_eq!(run(false), run(true));
}

/// A voicing note of `role` tagged as the realizer tags it (`hold`, `comp`, `pad`, `answer`).
fn voiced(beat: f64, dur: f32, pitch: i32, role: Role, tag: &'static str) -> Note {
    let mut n = note(beat, dur, pitch, role, F::ChordTone);
    n.prov.role_note = tag;
    n
}

/// The realizer's gate pays exactly the rent owed, by the smallest intervention: an asserted
/// unwritten colour is shortened when that alone ends the assertion, omitted once per voicing
/// while the voicing keeps a guide tone, or moved to the nearest written chord tone. Written
/// extensions, lines and gestural colours are left alone.
#[test]
fn r13_gate_pays_rent_by_the_smallest_intervention() {
    use super::comp::{gate_support_mass, SupportMassAction as A};
    let world = MusicWorld::swiss_signal();
    let gate = |chords: &[(f64, Quality, i32)], mut pad: Vec<Note>, mut keys: Vec<Note>| {
        let (p, _) = fixture(chords, world.tempo_bpm);
        let edits = gate_support_mass(&p, &world, &mut pad, &mut keys);
        let pitches = |v: &[Note]| v.iter().map(|n| (n.pitch, n.dur_beats)).collect::<Vec<_>>();
        (
            edits
                .iter()
                .map(|e| (e.note.pitch, e.action))
                .collect::<Vec<_>>(),
            pitches(&pad),
            pitches(&keys),
        )
    };
    let c6 = [(0.0, Quality::Maj6, 0), (8.0, Quality::Maj7, 0)];
    // The SWISS witness shape: a keys shell struck with the C6 arrival sheds its unwritten D5.
    let hold = [64, 69, 74]
        .map(|p| voiced(0.0, 1.94, p, Role::Keys, "hold"))
        .to_vec();
    let (e, _, keys) = gate(&c6, vec![], hold);
    assert_eq!(e, [(74, A::Omitted)]);
    assert_eq!(keys, [(64, 1.94), (69, 1.94)]);
    // Two unwritten colours in one pad voicing over Dm7: one omitted, the next moved (the bed
    // keeps its density).
    let dm7 = [(0.0, Quality::Min7, 2), (8.0, Quality::Min7, 9)];
    let pad = [65, 72, 76, 79]
        .map(|p| voiced(0.0, 3.92, p, Role::Pad, "pad"))
        .to_vec();
    let (e, pad, _) = gate(&dm7, pad, vec![]);
    assert_eq!(e, [(76, A::Omitted), (79, A::Moved(77))]);
    assert_eq!(pad.len(), 3);
    // Off the arrival and not fused: releasing the sustained colour early is enough.
    let (e, pad, _) = gate(&c6, vec![voiced(1.0, 2.9, 74, Role::Pad, "pad")], vec![]);
    assert_eq!(e, [(74, A::Shortened(0.75))]);
    assert_eq!(pad, [(74, 0.75)]);
    // Authored: the same held D over Cadd9 is written harmony.
    let add9 = [(0.0, Quality::Add9, 0), (8.0, Quality::Maj7, 0)];
    let hold = [64, 67, 74]
        .map(|p| voiced(0.0, 3.9, p, Role::Keys, "hold"))
        .to_vec();
    assert!(gate(&add9, vec![], hold).0.is_empty());
    // A weak offbeat stab and a material line at the arrival are not voicing assertions.
    let stab = [64, 69, 74]
        .map(|p| voiced(1.5, 0.45, p, Role::Keys, "comp"))
        .to_vec();
    assert!(gate(&c6, vec![], stab).0.is_empty());
    let line = vec![voiced(0.0, 1.0, 74, Role::Keys, "answer")];
    assert!(gate(&c6, vec![], line).0.is_empty());
}

/// The matched A/B: Round XII `perform_temporal` stays frozen; `perform_mass` removes every
/// accidental reharmonization while the song, the plan, the lead, the bass, the drums and every
/// written chord tone stay exactly where they were.
#[test]
fn r13_matched_scores_pay_rent_without_moving_the_song() {
    use super::contract::CompositionGrammar;
    use super::functor::perform_mass;
    use super::song::SongMapConformance;
    for (composer, grammar, frozen, budget) in [
        (
            Composer::StablePropulsion,
            None,
            [0x8ab3cd9fa10d751c, 0x06446ebcbbd14038],
            [0.02, 0.05],
        ),
        (
            Composer::MeaningDirected,
            Some(CompositionGrammar::DeflectedLift),
            [0x8b2b4cb318d64c52, 0x716f7ec67508ea4b],
            [0.05, 0.07],
        ),
    ] {
        let song = SongMap::compose(&deflected_lift_trace(120.0), 2112, grammar, composer);
        for (wi, world) in [MusicWorld::swiss_signal(), MusicWorld::black_ice()]
            .iter()
            .enumerate()
        {
            let a = perform_temporal(&song, world, PerformanceOptions::default());
            let b = perform_mass(&song, world, PerformanceOptions::default());
            assert_eq!(a.score.fingerprint(), frozen[wi], "Round XII control drift");
            assert_eq!(a.perf.fingerprint(), b.perf.fingerprint());
            assert!(SongMapConformance::check(&song, &b.perf, &b.score).passes());
            for (x, y) in [
                (
                    format!("{:?}", a.score.drums),
                    format!("{:?}", b.score.drums),
                ),
                (format!("{:?}", a.score.sfx), format!("{:?}", b.score.sfx)),
                (
                    format!("{:?}", a.score.chords),
                    format!("{:?}", b.score.chords),
                ),
            ] {
                assert_eq!(x, y);
            }
            let line = |s: &Score, r: Role| {
                s.notes
                    .iter()
                    .filter(|n| n.role == r)
                    .map(|n| format!("{n:?}"))
                    .collect::<Vec<_>>()
            };
            for r in [Role::Lead, Role::Bass] {
                assert_eq!(line(&a.score, r), line(&b.score, r), "{r:?} moved");
            }
            let present = |n: &Note| {
                b.score
                    .notes
                    .iter()
                    .any(|m| m.role == n.role && m.start_beat == n.start_beat && m.pitch == n.pitch)
            };
            let mut moved = 0;
            for n in &a.score.notes {
                let written = a
                    .perf
                    .context_at(n.start_beat)
                    .is_some_and(|c| c.chord.contains_pc(super::theory::pitch_class(n.pitch)));
                if written || !matches!(n.role, Role::Pad | Role::Keys) {
                    assert!(present(n), "a written or non-support note moved: {n:?}");
                }
                moved += usize::from(!present(n));
            }
            assert!(b.score.notes.len() <= a.score.notes.len());
            assert!(
                moved > 0 && (moved as f64) < budget[wi] * a.score.notes.len() as f64,
                "unbounded perturbation {moved}"
            );
            let (da, db) = (
                MassDiagnostics::measure(&a.perf, &a.score, world),
                MassDiagnostics::measure(&b.perf, &b.score, world),
            );
            assert!(da.accidental_reharmonizations > 0);
            assert_eq!(db.accidental_reharmonizations, 0, "{}", db.report());
            assert_eq!(
                db.overdrawn_gestures, da.overdrawn_gestures,
                "the lead is untouched"
            );
            let r12 = &db.base;
            assert_eq!(
                r12.false_function_claims
                    + r12.false_suspensions
                    + r12.broken_anticipations
                    + r12.unresolved_tendencies
                    + r12.bad_arrivals,
                0,
                "{}",
                r12.report()
            );
        }
    }
}
