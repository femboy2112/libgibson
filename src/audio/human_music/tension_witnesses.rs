//! Round XIIIb witnesses and controls: a note that sounds wrong in context must be quickly
//! transient, or foreshadow what justifies it.
//!
//! The acceptance anchor is the maintainer's Round XIII listen: "the same shitty chord at 16 s"
//! in SWISS (the pad's Cmaj7 voiced E4 B4 C5 G5, 14.24–16.27 s) and the wrong notes still in
//! BLACK_ICE. The synthetic controls pin each clause of the law and each named constant.
use super::composer::Composer;
use super::context::{analyze, HarmonicContext};
use super::form::SectionKind;
use super::functor::{perform_mass, perform_tension};
use super::harmony::ChordSpan;
use super::performance::PerformanceOptions;
use super::score::{Note, Provenance, Role};
use super::semantic::deflected_lift_trace;
use super::song::SongMap;
use super::tension::{
    clashes, gate_sounding_tension, Clash, TensionAction as A, TensionDiagnostics,
    TensionVerdict as V,
};
use super::theory::{pitch_class, Chord, Mode, Quality, Scale};
use super::world::{MusicWorld, WorldId};

/// The contexts of `chords` (beat, quality, root) in C major, the last one lasting to `end`.
fn contexts(chords: &[(f64, Quality, i32)], end: f64) -> Vec<HarmonicContext> {
    let spans: Vec<ChordSpan> = chords
        .iter()
        .enumerate()
        .map(|(i, &(beat, q, root))| {
            ChordSpan::test(
                beat,
                (chords.get(i + 1).map_or(end, |x| x.0) - beat) as f32,
                Chord::new(root, q),
            )
        })
        .collect();
    analyze(&spans, &Scale::new(0, Mode::Ionian))
}

fn note(beat: f64, dur: f32, pitch: i32, role: Role, role_note: &'static str) -> Note {
    let mut p = Provenance::new(SectionKind::A);
    p.role_note = role_note;
    Note::new(beat, dur, pitch, 0.8, role, p)
}

fn pad(beat: f64, pitches: &[i32]) -> Vec<Note> {
    pitches
        .iter()
        .map(|&p| note(beat, 3.92, p, Role::Pad, "pad"))
        .collect()
}

fn audit(world: &MusicWorld, bpm: f32, ctx: &[HarmonicContext], notes: &[Note]) -> Vec<Clash> {
    clashes(notes, ctx, world, bpm, 4.0)
}

/// The clash whose responsible note is `(role, pitch, beat)`.
fn clash_of(notes: &[Note], cs: &[Clash], role: Role, pitch: i32, beat: f64) -> Clash {
    *cs.iter()
        .find(|c| {
            let t = &notes[c.tension];
            t.role == role && t.pitch == pitch && t.start_beat == beat
        })
        .unwrap_or_else(|| panic!("no clash owed by {role:?} {pitch} at {beat}: {cs:?}"))
}

fn unjustified(cs: &[Clash]) -> usize {
    cs.iter().filter(|c| c.verdict == V::Unjustified).count()
}

/// The chromatic neighbour C5–C#5–C5 of half-beat lead notes over a held A-minor pad C5.
fn neighbour(to: i32) -> Vec<Note> {
    let mut v = pad(0.0, &[64, 69, 72]);
    v.push(note(0.0, 3.92, 45, Role::Bass, "root"));
    v.push(note(0.0, 0.45, 72, Role::Lead, "melody"));
    v.push(note(0.5, 0.45, 73, Role::Lead, "melody"));
    v.push(note(1.0, 0.45, to, Role::Lead, "melody"));
    v
}

fn a_minor() -> Vec<HarmonicContext> {
    contexts(&[(0.0, Quality::Min, 9)], 4.0)
}

/// The Round XIII witness: the SWISS pad's Cmaj7 with its major seventh a semitone under the root,
/// both held a whole bar, is an Asserted clash going nowhere. The gate drops the root the bass
/// already sounds; the written seventh survives. The same chord voiced as the SWISS pad voices it
/// elsewhere (C4 G4 B4 E5) has no clash at all.
#[test]
fn r13b_swiss_cmaj7_cluster_loses_its_covered_root() {
    let world = MusicWorld::swiss_signal();
    let ctx = contexts(&[(0.0, Quality::Maj7, 0), (4.0, Quality::Maj6, 0)], 8.0);
    let mut notes = pad(0.0, &[64, 71, 72, 79]);
    notes.extend(pad(4.0, &[64, 69, 72, 79]));
    notes.push(note(0.0, 1.9, 36, Role::Bass, "root"));
    notes.push(note(4.0, 1.9, 36, Role::Bass, "root"));
    let cs = audit(&world, 118.0, &ctx, &notes);
    let c = clash_of(&notes, &cs, Role::Pad, 71, 0.0);
    assert_eq!(notes[c.partner].pitch, 72);
    assert_eq!(
        (c.class, c.verdict),
        (super::mass::ExposureClass::Asserted, V::Unjustified)
    );
    assert_eq!(unjustified(&cs), 1, "{cs:?}");
    let edits = gate_sounding_tension(&mut notes, &ctx, &world, 118.0, 4.0, None);
    assert_eq!(edits.len(), 1);
    assert_eq!(
        (edits[0].note.pitch, edits[0].action),
        (72, A::OmittedCovered)
    );
    assert!(notes
        .iter()
        .any(|n| n.role == Role::Pad && n.pitch == 71 && n.start_beat == 0.0));
    assert_eq!(unjustified(&audit(&world, 118.0, &ctx, &notes)), 0);

    let mut open = pad(0.0, &[60, 67, 71, 76]);
    open.push(note(0.0, 1.9, 36, Role::Bass, "root"));
    assert!(audit(&world, 118.0, &ctx, &open).is_empty());
}

/// An authored ninth is never deleted to end a clash it can leave by register: BLACK_ICE's Am9
/// pad G4 B4 C5 E5 keeps B, an octave up, and keeps its pitch-class set.
#[test]
fn r13b_authored_ninth_survives_by_octave() {
    let world = MusicWorld::black_ice();
    let ctx = contexts(&[(0.0, Quality::Min9, 9)], 4.0);
    let mut notes = pad(0.0, &[67, 71, 72, 76]);
    notes.push(note(0.0, 3.92, 33, Role::Bass, "root"));
    let pcs = |ns: &[Note]| {
        let mut v: Vec<i32> = ns.iter().map(|n| pitch_class(n.pitch)).collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    let before = pcs(&notes);
    assert_eq!(unjustified(&audit(&world, 88.0, &ctx, &notes)), 1);
    let edits = gate_sounding_tension(&mut notes, &ctx, &world, 88.0, 4.0, None);
    assert_eq!(edits.len(), 1);
    assert_eq!(
        (edits[0].note.pitch, edits[0].action),
        (71, A::Displaced(83))
    );
    assert_eq!(pcs(&notes), before);
    assert_eq!(unjustified(&audit(&world, 88.0, &ctx, &notes)), 0);
}

/// Tempo is load-bearing: the same half-beat chromatic neighbour over the pad's C5 is Transient
/// at 160 BPM (0.19 s) and Unjustified at 118 and 88 BPM, where it is heard long enough to be
/// evaluated and "resolves" onto the C that was sounding all along.
#[test]
fn r13b_tempo_dilation_crosses_the_clash_class() {
    let world = MusicWorld::black_ice();
    let notes = neighbour(72);
    let verdicts: Vec<(V, f64)> = [160.0, 118.0, 88.0]
        .iter()
        .map(|&bpm| {
            let c = clash_of(
                &notes,
                &audit(&world, bpm, &a_minor(), &notes),
                Role::Lead,
                73,
                0.5,
            );
            (c.verdict, c.exposure)
        })
        .collect();
    assert_eq!(
        verdicts.iter().map(|v| v.0).collect::<Vec<_>>(),
        [V::Transient, V::Unjustified, V::Unjustified]
    );
    assert!(verdicts[0].1 < verdicts[1].1 && verdicts[1].1 < verdicts[2].1);
}

/// The same C#5 that walks on to D5 foreshadows it: a resolution the ear has not already heard.
/// Only the continuation differs from the neighbour that falls back onto the sounding C5.
#[test]
fn r13b_passing_tone_foreshadows_where_the_neighbour_cannot() {
    let world = MusicWorld::black_ice();
    let passing = neighbour(74);
    let c = clash_of(
        &passing,
        &audit(&world, 88.0, &a_minor(), &passing),
        Role::Lead,
        73,
        0.5,
    );
    assert_eq!(c.verdict, V::Foreshadowing);
    assert_eq!(passing[c.continuation.unwrap()].pitch, 74);
}

/// A line note is made transient without leaving its place: re-timed as a grace inside its own
/// written slot, leaning into its voice's next note, Fleeting with its whole release tail.
#[test]
fn r13b_ornament_stays_in_its_slot_and_is_fleeting() {
    let world = MusicWorld::black_ice();
    let mut notes = neighbour(72);
    let edits = gate_sounding_tension(&mut notes, &a_minor(), &world, 88.0, 4.0, None);
    assert_eq!(edits.len(), 1, "{edits:?}");
    let A::Ornament {
        start_beat,
        dur_beats,
    } = edits[0].action
    else {
        panic!("not an ornament: {edits:?}");
    };
    assert!((0.5..0.95).contains(&start_beat) && start_beat + f64::from(dur_beats) <= 1.0 + 1e-9);
    let cs = audit(&world, 88.0, &a_minor(), &notes);
    assert_eq!(
        clash_of(&notes, &cs, Role::Lead, 73, start_beat).verdict,
        V::Transient
    );
    assert_eq!(unjustified(&cs), 0);
}

/// Mass is evidence, not a verdict: a prepared 4–3 suspension (F5 held from the F chord into C,
/// a ninth over the pad's E4, resolving to E5) is justified at Asserted exposure. The same F5
/// struck with the C chord is not.
#[test]
fn r13b_prepared_suspension_survives_its_mass() {
    let world = MusicWorld::black_ice();
    let ctx = contexts(&[(0.0, Quality::Maj, 5), (4.0, Quality::Maj, 0)], 8.0);
    let run = |onset: f64| {
        let mut v = pad(0.0, &[65, 69, 72]);
        v.extend(pad(4.0, &[64, 67, 72]));
        v.push(note(onset, (5.0 - onset) as f32, 77, Role::Lead, "melody"));
        v.push(note(5.0, 1.0, 76, Role::Lead, "melody"));
        let c = clash_of(&v, &audit(&world, 88.0, &ctx, &v), Role::Lead, 77, onset);
        (c.class, c.verdict)
    };
    use super::mass::ExposureClass::Asserted;
    assert_eq!(run(2.0), (Asserted, V::Suspension));
    assert_eq!(run(4.0), (Asserted, V::Unjustified));
}

/// A line tone of the next harmony, sounding into it, anticipates it.
#[test]
fn r13b_anticipation_is_justified() {
    let world = MusicWorld::black_ice();
    let ctx = contexts(&[(0.0, Quality::Maj, 0), (4.0, Quality::Maj, 7)], 8.0);
    let mut notes = pad(0.0, &[64, 67, 72]);
    notes.push(note(3.5, 0.75, 71, Role::Lead, "melody"));
    notes.push(note(5.0, 1.0, 67, Role::Lead, "melody"));
    let c = clash_of(
        &notes,
        &audit(&world, 88.0, &ctx, &notes),
        Role::Lead,
        71,
        3.5,
    );
    assert_eq!(c.verdict, V::Anticipation);
}

/// [`CLASH_SEMITONES`](super::tension::CLASH_SEMITONES) witness: the dialects' stock colours are
/// not clashes (C6's G–A major second, Am6's C–F# tritone), and one voice's legato tail into its
/// next attack (the pad's F4 → F#4) is not one either.
#[test]
fn r13b_stock_colour_and_legato_are_not_clashes() {
    let world = MusicWorld::black_ice();
    let six = contexts(&[(0.0, Quality::Maj6, 0)], 4.0);
    assert!(audit(&world, 88.0, &six, &pad(0.0, &[60, 67, 69, 76])).is_empty());
    let am6 = contexts(&[(0.0, Quality::Min6, 9)], 4.0);
    assert!(audit(&world, 88.0, &am6, &pad(0.0, &[57, 64, 66, 72])).is_empty());
    let ctx = contexts(&[(0.0, Quality::Min7, 2), (4.0, Quality::Min6, 9)], 8.0);
    let mut legato = pad(0.0, &[65]);
    legato.extend(pad(4.0, &[66]));
    assert!(audit(&world, 88.0, &ctx, &legato).is_empty());
}

/// Exposure is non-decreasing in the overlap and in the accent where the clash begins.
#[test]
fn r13b_exposure_is_monotone_in_overlap_and_accent() {
    let world = MusicWorld::black_ice();
    let ctx = a_minor();
    let exposure = |beat: f64, dur: f32| {
        let mut v = pad(0.0, &[72]);
        v.push(note(beat, dur, 73, Role::Lead, "melody"));
        clash_of(&v, &audit(&world, 88.0, &ctx, &v), Role::Lead, 73, beat).exposure
    };
    let lens: Vec<f64> = [0.25, 0.5, 1.0, 2.0]
        .iter()
        .map(|&d| exposure(1.0, d))
        .collect();
    assert!(lens.windows(2).all(|w| w[0] <= w[1]), "{lens:?}");
    // Weak (beat 1), mid-bar (beat 2), downbeat (beat 4 of a later bar — same overlap).
    let accents: Vec<f64> = [1.0, 2.0].iter().map(|&b| exposure(b, 0.5)).collect();
    assert!(accents[0] < accents[1], "{accents:?}");
}

fn stable_song() -> SongMap {
    SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    )
}

/// The acceptance scores. Round XIII (the arm the maintainer heard) leaves the 16 s chord's
/// B4/C5 clash unjustified in SWISS and 33 in BLACK_ICE; Round XIIIb leaves none, in SWISS by
/// dropping only the four bass-covered roots under those sevenths.
#[test]
fn r13b_acceptance_songs_leave_no_unjustified_tension() {
    let song = stable_song();
    for (world, before, edits) in [
        (MusicWorld::swiss_signal(), 4, 4),
        (MusicWorld::black_ice(), 33, 29),
    ] {
        let opts = PerformanceOptions::default();
        let (b, c) = (
            perform_mass(&song, &world, opts),
            perform_tension(&song, &world, opts),
        );
        let tb = TensionDiagnostics::measure(&b.score, &b.perf.contexts, &world);
        let tc = TensionDiagnostics::measure(&c.score, &c.perf.contexts, &world);
        assert_eq!(
            (tb.unjustified, tc.unjustified),
            (before, 0),
            "{}",
            tc.report(&c.score)
        );
        assert_eq!(c.score.tension_edits.len(), edits);
        assert_eq!(b.perf.fingerprint(), c.perf.fingerprint());
        assert_eq!(
            format!("{:?}", b.score.drums),
            format!("{:?}", c.score.drums)
        );
        if world.id == WorldId::SwissSignal {
            let heard_at_16s = tb
                .clashes
                .iter()
                .find(|x| x.verdict == V::Unjustified && x.onset_beat == 28.0)
                .expect("the 16 s chord");
            assert_eq!(
                (
                    b.score.notes[heard_at_16s.tension].pitch,
                    b.score.notes[heard_at_16s.partner].pitch
                ),
                (71, 72)
            );
            let dropped: Vec<(f64, i32, A)> = c
                .score
                .tension_edits
                .iter()
                .map(|e| (e.note.start_beat, e.note.pitch, e.action))
                .collect();
            assert_eq!(
                dropped,
                [28.0, 36.0, 44.0, 116.0].map(|b| (b, 72, A::OmittedCovered))
            );
        }
    }
}

/// The gate never adds a note and never moves one earlier (it may only drop, shorten, re-register
/// or delay inside the note's own slot), on every note of both acceptance scores.
#[test]
fn r13b_gate_only_subtracts_or_delays() {
    let song = stable_song();
    for world in [MusicWorld::swiss_signal(), MusicWorld::black_ice()] {
        let opts = PerformanceOptions::default();
        let (b, c) = (
            perform_mass(&song, &world, opts),
            perform_tension(&song, &world, opts),
        );
        assert!(c.score.notes.len() <= b.score.notes.len());
        for e in &c.score.tension_edits {
            let n = e.note;
            let end = n.start_beat + f64::from(n.dur_beats);
            match e.action {
                A::Shortened(d) => assert!(d < n.dur_beats),
                A::Ornament {
                    start_beat,
                    dur_beats,
                } => assert!(
                    start_beat >= n.start_beat && start_beat < end && dur_beats < n.dur_beats
                ),
                A::Displaced(q) => assert_eq!((q - n.pitch).rem_euclid(12), 0),
                A::OmittedCovered | A::Omitted | A::Moved(_) => {}
            }
        }
    }
}

/// The law must hold far beyond the acceptance song: two stories, two lengths, ten seeds, both
/// composers, every world (240 performances per arm). Run explicitly (`--ignored`, release): it
/// fails on any unjustified clash the gate leaves, any SongMap conformance, plan, drum or
/// interaction witness it moves, any Round XII false claim it introduces, and any edit that adds
/// a note or plays one earlier.
#[test]
#[ignore = "fuzz sweep: run with --release -- --ignored"]
fn fuzz_the_tension_gate_leaves_no_unjustified_clash() {
    use super::contract::CompositionGrammar;
    use super::semantic::demo_trace;
    use super::song::SongMapConformance;
    let (mut runs, mut before, mut edits, mut notes) = (0usize, 0usize, 0usize, 0usize);
    let mut gained = 0usize;
    let mut kinds = std::collections::BTreeMap::new();
    for story in ["bounce", "demo"] {
        for beats in [37.0, 120.0] {
            let trace = match story {
                "bounce" => deflected_lift_trace(beats),
                _ => demo_trace(beats),
            };
            for seed in 0..10u64 {
                for (grammar, composer) in [
                    (None, Composer::StablePropulsion),
                    (
                        Some(CompositionGrammar::DeflectedLift),
                        Composer::MeaningDirected,
                    ),
                ] {
                    let song = SongMap::compose(&trace, seed, grammar, composer);
                    for world in MusicWorld::all() {
                        let opts = PerformanceOptions::default();
                        let b = perform_mass(&song, &world, opts);
                        let c = perform_tension(&song, &world, opts);
                        let what = format!("{story}/{beats}/{seed}/{composer:?}/{}", world.name);
                        let tc = TensionDiagnostics::measure(&c.score, &c.perf.contexts, &world);
                        assert_eq!(tc.unjustified, 0, "{what}\n{}", tc.report(&c.score));
                        // Not an absolute pass: some inherited plans leave a song obligation
                        // unwitnessed (a property of the plan alone, identical in both arms).
                        let law = |x: &super::functor::Composition| {
                            SongMapConformance::check(&song, &x.perf, &x.score).report()
                        };
                        assert_eq!(law(&b), law(&c), "{what}");
                        assert_eq!(b.perf.fingerprint(), c.perf.fingerprint(), "{what}");
                        assert_eq!(
                            format!("{:?}", b.score.drums),
                            format!("{:?}", c.score.drums),
                            "{what}"
                        );
                        let (wb, wc) = (
                            super::witness::audit(&b.perf, &b.score),
                            super::witness::audit(&c.perf, &c.score),
                        );
                        assert_eq!(wb.total(), wc.total(), "{what}");
                        for (x, y) in wb.rows.iter().zip(&wc.rows) {
                            assert!(!x.witnessed || y.witnessed, "{what}: lost {:?}", x.action);
                            gained += usize::from(y.witnessed && !x.witnessed);
                        }
                        for e in &c.score.tension_edits {
                            let n = e.note;
                            let kind = format!("{}:{:?}", n.role.label(), e.action)
                                .split(['(', ' '])
                                .next()
                                .unwrap_or_default()
                                .to_owned();
                            *kinds.entry(kind).or_insert(0usize) += 1;
                            match e.action {
                                A::Shortened(d) => assert!(d < n.dur_beats, "{what}"),
                                A::Ornament { start_beat, .. } => {
                                    assert!(start_beat >= n.start_beat, "{what}")
                                }
                                _ => {}
                            }
                        }
                        assert!(c.score.notes.len() <= b.score.notes.len(), "{what}");
                        // The Round XII observer finds no false claim the control did not have.
                        let claims = |x: &super::functor::Composition| {
                            let d =
                                super::mass::MassDiagnostics::measure(&x.perf, &x.score, &world)
                                    .base;
                            d.false_function_claims
                                + d.false_suspensions
                                + d.broken_anticipations
                                + d.bad_arrivals
                                + d.unresolved_tendencies
                        };
                        assert!(
                            claims(&c) <= claims(&b),
                            "{what}: {} > {}",
                            claims(&c),
                            claims(&b)
                        );
                        runs += 1;
                        before += TensionDiagnostics::measure(&b.score, &b.perf.contexts, &world)
                            .unjustified;
                        edits += c.score.tension_edits.len();
                        notes += b.score.notes.len();
                    }
                }
            }
        }
    }
    println!(
        "tension sweep: performances={runs} unjustified before={before} after=0 edits={edits} ({:.2}% of {notes} pitched notes) receipts lost=0 gained={gained}",
        100.0 * edits as f64 / notes as f64
    );
    println!("tension sweep edits by role and action: {kinds:?}");
}
