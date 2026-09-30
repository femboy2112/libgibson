//! Round XIV witnesses: the object the listener hears must commute with the one the plan says is
//! there.
//!
//! The maintainer's Round XIIIb listen: SWISS's ~16 s is now a different chord, and the band is no
//! longer the system that generated itself. Round XIIIb took the pad's C5 out of the Cmaj7 at
//! beat 28 because the bass sounded C at the pad's onset, but the bass then walks E2 G2 B2 while
//! the pad holds E4 B4 G5, so 15.3–16.0 s is heard as Em over its own fifth. The falsifiers were
//! committed failing first (`docs/fixtures/humanmusic-r14/baseline-failures.txt`); each is restated
//! here against the audit that can see it ([`super::identity`], [`super::gesture`],
//! [`Score::stale_hearings`]).
use super::composer::Composer;
use super::context::{analyze, HarmonicContext};
use super::form::SectionKind;
use super::functor::{
    perform_coherent, perform_mass, perform_temporal, perform_tension, Composition,
};
use super::gesture::{classify, entry_of, Entry, Gesture, GestureDiagnostics, PathEvent};
use super::harmony::ChordSpan;
use super::identity::{
    keeps_identity, IdentityDiagnostics, IdentityRun, IdentityStatus, IDENTITY_HOLD_SECS,
};
use super::performance::{PerformanceOptions, PerformancePlan};
use super::score::{Note, PitchFunction as F, Provenance, Role, Score};
use super::semantic::deflected_lift_trace;
use super::song::{SongMap, SongMapConformance};
use super::sonority::{ColorPolicy, EnsembleSonorityDiagnostics};
use super::temporal::TemporalPitchDiagnostics as Audit;
use super::tension::{gate_sounding_tension, heard_windows, TensionAction};
use super::theory::{pitch_class, Chord, Mode, Quality, Scale};
use super::world::MusicWorld;

/// The acceptance song: StablePropulsion, seed 2112, the 120-beat deflected-lift trace.
fn stable_song() -> SongMap {
    SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    )
}

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

/// A C-major performance and empty score at `bpm` whose harmonies are exactly `chords`.
fn fixture(chords: &[(f64, Quality, i32)], bpm: f32, end: f64) -> (PerformancePlan, Score) {
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
    perf.contexts = contexts(chords, end);
    perf.chords = chords
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
    let mut score = Score::new(bpm, 4.0, end);
    score.chords = perf.chords.clone();
    (perf, score)
}

fn note(beat: f64, dur: f32, pitch: i32, role: Role, role_note: &'static str) -> Note {
    let mut p = Provenance::new(SectionKind::A);
    p.role_note = role_note;
    Note::new(beat, dur, pitch, 0.8, role, p)
}

/// The R13b shape in one bar of Cmaj7 at 118 BPM: the pad holds E4 B4 C5 G5; the bass states C2
/// on the downbeat for two beats, then walks E2 G2 B2 in eighths and lands on C3.
fn walk_under_cmaj7() -> Vec<Note> {
    let mut v: Vec<Note> = [64, 71, 72, 79]
        .iter()
        .map(|&p| note(0.0, 3.92, p, Role::Pad, "pad"))
        .collect();
    v.push(note(0.0, 1.9, 36, Role::Bass, "root"));
    for (i, p) in [40, 43, 47, 48].into_iter().enumerate() {
        v.push(note(2.0 + 0.5 * i as f64, 0.45, p, Role::Bass, "quote"));
    }
    v
}

/// Whether some note other than `except` sounds pitch class `pc` at every instant of `[a, b)`
/// beats, by the heard windows of `notes`.
fn sounded_throughout(notes: &[Note], except: usize, pc: i32, a: f64, b: f64) -> bool {
    let win = heard_windows(notes, &MusicWorld::swiss_signal(), 118.0);
    let mut spans: Vec<(f64, f64)> = (0..notes.len())
        .filter(|&j| j != except && pitch_class(notes[j].pitch) == pc)
        .map(|j| win[j])
        .collect();
    spans.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut reach = a;
    for (s, e) in spans {
        if s > reach + 1e-9 {
            break;
        }
        reach = reach.max(e);
    }
    reach >= b - 1e-9
}

/// Round VIII's identity law, audible lifetimes: beats of flipped identity over the whole song.
fn round8_flip_beats(x: &Composition, world: &MusicWorld) -> f64 {
    let policy = ColorPolicy::for_world(world.id, &x.perf.language);
    EnsembleSonorityDiagnostics::measure_audible(&x.score, &x.perf.contexts, world, &policy, &[])
        .identity_flip_beats
}

/// The regression as sounded: at 16.0 s Round XIIIb's band is bass B2 under pad E4 B4 G5, no C
/// anywhere, under a chart that says Cmaj7; Round XII's pad still sounds C5 there.
#[test]
fn r14_the_r13b_band_at_sixteen_seconds_sounds_no_c() {
    let song = stable_song();
    let world = MusicWorld::swiss_signal();
    let opts = PerformanceOptions::default();
    let heard_at = |x: &Composition, secs: f64| -> Vec<(Role, i32)> {
        let beat = secs * f64::from(x.score.tempo_bpm) / 60.0;
        let win = heard_windows(&x.score.notes, &world, x.score.tempo_bpm);
        let mut v: Vec<(Role, i32)> = (0..x.score.notes.len())
            .filter(|&i| win[i].0 <= beat && beat < win[i].1)
            .map(|i| (x.score.notes[i].role, x.score.notes[i].pitch))
            .collect();
        v.sort_by_key(|&(r, p)| (r.label(), p));
        v
    };
    let (a, c) = (
        perform_temporal(&song, &world, opts),
        perform_tension(&song, &world, opts),
    );
    let chart = |x: &Composition| {
        x.perf
            .context_at(16.0 * f64::from(x.score.tempo_bpm) / 60.0)
            .map(|k| k.chord)
    };
    assert_eq!(chart(&c), Some(Chord::new(0, Quality::Maj7)));
    assert_eq!(chart(&a), chart(&c));
    assert_eq!(
        heard_at(&c, 16.0),
        [
            (Role::Bass, 47),
            (Role::Pad, 64),
            (Role::Pad, 71),
            (Role::Pad, 79)
        ]
    );
    assert_eq!(
        heard_at(&a, 16.0),
        [
            (Role::Bass, 47),
            (Role::Pad, 64),
            (Role::Pad, 71),
            (Role::Pad, 72),
            (Role::Pad, 79)
        ]
    );
}

/// The flips of `x` as (start s, end s, chart, heard rival), to the hundredth of a second.
fn flips(x: &Composition, world: &MusicWorld) -> Vec<(String, String, String, String)> {
    let id =
        IdentityDiagnostics::measure(&x.score.notes, &x.perf.contexts, world, x.score.tempo_bpm);
    id.flips()
        .map(|r| {
            (
                format!("{:.2}", id.secs(r.start_beat)),
                format!("{:.2}", id.secs(r.end_beat)),
                r.chart.label(),
                r.rival.label(),
            )
        })
        .collect()
}

fn run(a: &str, b: &str, chart: &str, rival: &str) -> (String, String, String, String) {
    (a.into(), b.into(), chart.into(), rival.into())
}

/// Round XII sounds the chart's Cmaj7 all through 15–17 s and flips once: at 42.97 s, Fmaj9
/// voiced A4 C5 E5 G5 over a bass line on C# and C, with no F anywhere (inside the 43–45 s
/// defect of the Round XII listen). Round XIII is the same. Round XIIIb adds the two Cmaj7 → Em
/// flips it made by taking the root out. The Round VIII law reads the same flip beats in Round
/// XII and Round XIIIb: its root memory lasts two beats past the root's audible end, clamped to
/// the harmony's start, so a downbeat root certifies the whole bar.
#[test]
fn r14_identity_audit_hears_the_r13b_flip() {
    let song = stable_song();
    let world = MusicWorld::swiss_signal();
    let opts = PerformanceOptions::default();
    let (a, b, c) = (
        perform_temporal(&song, &world, opts),
        perform_mass(&song, &world, opts),
        perform_tension(&song, &world, opts),
    );
    assert_eq!(round8_flip_beats(&a, &world), round8_flip_beats(&c, &world));
    let original = run("42.97", "44.75", "Fmaj9", "A");
    assert_eq!(flips(&a, &world), std::slice::from_ref(&original));
    assert_eq!(flips(&b, &world), std::slice::from_ref(&original));
    assert_eq!(
        flips(&c, &world),
        [
            run("15.25", "16.02", "Cmaj7", "Em"),
            run("23.72", "24.41", "Cmaj7", "Em"),
            original
        ]
    );
    let at16 = |x: &Composition| {
        let id = IdentityDiagnostics::measure(
            &x.score.notes,
            &x.perf.contexts,
            &world,
            x.score.tempo_bpm,
        );
        let s = id
            .at_beat(16.0 * 118.0 / 60.0)
            .expect("sound at 16 s")
            .clone();
        (s.status, s.rival.map(|r| r.label()), s.root_heard)
    };
    assert_eq!(at16(&a), (IdentityStatus::Rooted, None, true));
    assert_eq!(
        at16(&c),
        (IdentityStatus::Flipped, Some("Em".to_string()), false)
    );
}

/// Round XIIIb's coverage proof reads one instant: it removes the pad's C5 while the bass sounds
/// C only until beat 2. The heard-identity contract rejects that removal. C stops sounding for
/// the 1.5 beats the bass walks E G B under E4 B4 G5, the band flips to Em, and the band after
/// the removal keeps the chart's identity worse than before.
#[test]
fn r14_onset_coverage_is_rejected_by_heard_identity() {
    let world = MusicWorld::swiss_signal();
    let ctx = contexts(&[(0.0, Quality::Maj7, 0)], 8.0);
    let before = walk_under_cmaj7();
    let mut after = before.clone();
    let edits = gate_sounding_tension(&mut after, &ctx, &world, 118.0, 4.0, None);
    let omitted: Vec<i32> = edits
        .iter()
        .filter(|e| e.action == TensionAction::OmittedCovered)
        .map(|e| e.note.pitch)
        .collect();
    assert_eq!(omitted, [72], "R13b's gate removes C5 as covered");
    let i = before.iter().position(|n| n.pitch == 72).unwrap();
    let (a, b) = heard_windows(&before, &world, 118.0)[i];
    assert!(!sounded_throughout(&before, i, 0, a, b));
    let ib = IdentityDiagnostics::measure(&before, &ctx, &world, 118.0);
    let ia = IdentityDiagnostics::measure(&after, &ctx, &world, 118.0);
    assert_eq!(ib.flips().count(), 0, "{}", ib.report(&before, &ctx));
    let f: Vec<&IdentityRun> = ia.flips().collect();
    assert_eq!(f.len(), 1, "{}", ia.report(&after, &ctx));
    assert_eq!(f[0].rival, Chord::new(4, Quality::Min));
    assert!(f[0].secs >= IDENTITY_HOLD_SECS);
    assert!(!keeps_identity(&ib, &ia, 0.0, 8.0));
    assert!(keeps_identity(&ib, &ib, 0.0, 8.0));
}

/// Every status of one bar of `notes` under `chord` at 118 BPM, and its flips.
fn heard_statuses(
    chord: (Quality, i32),
    notes: &[Note],
) -> (Vec<IdentityStatus>, Vec<IdentityRun>) {
    let ctx = contexts(&[(0.0, chord.0, chord.1)], 8.0);
    let id = IdentityDiagnostics::measure(notes, &ctx, &MusicWorld::swiss_signal(), 118.0);
    let mut st: Vec<IdentityStatus> = id.slices.iter().map(|s| s.status).collect();
    st.dedup();
    (st, id.flips().cloned().collect())
}

fn held(pitches: &[i32], role: Role, beat: f64, dur: f32) -> Vec<Note> {
    pitches
        .iter()
        .map(|&p| note(beat, dur, p, role, role.label()))
        .collect()
}

/// The negative and positive controls. Flipped: a Cmaj7 heard as Em over B with no C anywhere,
/// and the Round XIIIb root deletion under a bass walk. Kept: an inversion (Cmaj7/E, Cmaj7/G),
/// a root absent from the bass while an upper voice sounds it, a rootless voicing over the root,
/// a genuine pedal, and a walk through a rival for one eighth (a passing ambiguity, not a flip).
#[test]
fn r14_identity_controls() {
    use IdentityStatus as S;
    let cmaj7 = (Quality::Maj7, 0);
    // Fake Em/B: bass B2 and E4 G4 B4, no C at all.
    let mut v = held(&[64, 67, 71], Role::Pad, 0.0, 3.92);
    v.extend(held(&[47], Role::Bass, 0.0, 3.9));
    let (st, f) = heard_statuses(cmaj7, &v);
    assert_eq!(st, [S::Flipped]);
    assert_eq!(f[0].rival, Chord::new(4, Quality::Min));
    // Real Cmaj7/E and Cmaj7/G: the root is in the band.
    for bass in [40, 43] {
        let mut v = held(&[60, 67, 71], Role::Pad, 0.0, 3.92);
        v.extend(held(&[bass], Role::Bass, 0.0, 3.9));
        assert_eq!(heard_statuses(cmaj7, &v).0, [S::Rooted], "bass {bass}");
    }
    // The bass rests for a beat; the pad's C4 E4 B4 still sounds the root.
    let mut v = held(&[60, 64, 71], Role::Pad, 0.0, 3.92);
    v.extend(held(&[36], Role::Bass, 1.0, 2.9));
    assert_eq!(heard_statuses(cmaj7, &v).0, [S::Rooted]);
    // Rootless Cmaj9 (E4 G4 B4 D5) over the root in the bass: rooted while the bass sounds; the
    // pad's release tail, ringing alone after the bass has stopped, is a passing Em7, not a flip.
    let mut v = held(&[64, 67, 71, 74], Role::Pad, 0.0, 3.92);
    v.extend(held(&[36], Role::Bass, 0.0, 3.9));
    let (st, f) = heard_statuses(cmaj7, &v);
    assert_eq!(st, [S::Rooted, S::Passing]);
    assert!(f.is_empty());
    // A C pedal under Dm7 voiced D4 F4 A4: the chart's root D is in the pad.
    let mut v = held(&[62, 65, 69], Role::Pad, 0.0, 3.92);
    v.extend(held(&[36], Role::Bass, 0.0, 3.9));
    assert_eq!(heard_statuses((Quality::Min7, 2), &v).0, [S::Rooted]);
    // The walk passes through Em for one eighth (B1 for 0.25 s at 118 BPM), then C2 returns.
    let mut v = held(&[64, 67, 71, 74], Role::Pad, 0.0, 3.92);
    v.extend(held(&[64, 67, 71, 74], Role::Pad, 4.0, 3.92));
    v.push(note(0.0, 3.45, 36, Role::Bass, "root"));
    v.push(note(3.5, 0.45, 35, Role::Bass, "approach"));
    v.push(note(4.0, 3.9, 36, Role::Bass, "root"));
    let (st, f) = heard_statuses(cmaj7, &v);
    assert!(f.is_empty(), "{f:?}");
    assert!(st.contains(&S::Passing), "{st:?}");
    // Root deletion plus the bass walk: the Round XIIIb regression.
    let v: Vec<Note> = walk_under_cmaj7()
        .into_iter()
        .filter(|n| n.pitch != 72)
        .collect();
    let (_, f) = heard_statuses(cmaj7, &v);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].rival, Chord::new(4, Quality::Min));
}

/// The frozen arms, as heard: Round XII's original flips are Round XIII's (SWISS 1; BLACK_ICE
/// 10, ten rootless pad voicings over a bass on the 5th or 3rd). Round XIIIb adds SWISS's two
/// Cmaj7 → Em. No arm has a slide claim standing on the slide alone, a declared SlidePath, or a
/// chromatic staircase: BLACK_ICE's chromatic moves are single approach notes.
#[test]
fn r14_frozen_arms_as_heard() {
    let song = stable_song();
    let opts = PerformanceOptions::default();
    for (world, r12, r13b_extra) in [
        (MusicWorld::swiss_signal(), 1, 2),
        (MusicWorld::black_ice(), 10, 0),
    ] {
        let arms = [
            perform_temporal(&song, &world, opts),
            perform_mass(&song, &world, opts),
            perform_tension(&song, &world, opts),
        ];
        let f: Vec<usize> = arms.iter().map(|x| flips(x, &world).len()).collect();
        assert_eq!(f, [r12, r12, r12 + r13b_extra], "{}", world.name);
        for x in &arms {
            let g = GestureDiagnostics::measure(&x.perf, &x.score);
            assert_eq!(
                (
                    g.declared_slides,
                    g.slide_dependent,
                    g.fake_slides,
                    g.chromatic_staircases
                ),
                (0, 0, 0, 0),
                "{}\n{}",
                world.name,
                g.report(&x.score)
            );
            // Round XIV's slide law: the observer no longer certifies attacked notes as a slide
            // (before it, 49 SWISS and 52 BLACK_ICE notes carried the label, every one of them
            // with another function beside it: docs/fixtures/humanmusic-r14/frozen/).
            assert_eq!(g.slide_supported, 0, "{}", world.name);
            assert!(x
                .score
                .notes
                .iter()
                .all(|n| n.function != Some(F::SlidePath)));
        }
    }
}

/// The same route C–C#–D–D#–E as five separately attacked notes is a discrete line: the synth
/// triggers a fresh voice per note, so no note of it may be certified as a slide, however fast.
/// Committed failing (the Round XII observer supported SlidePath from the pitch geometry alone);
/// it passes since the observer's slide rule requires a physical glide.
#[test]
fn r14_attacked_notes_are_never_a_slide() {
    let (p, mut s) = fixture(&[(0.0, Quality::Maj, 0)], 118.0, 8.0);
    s.notes = [
        (48, F::ChordTone),
        (49, F::ChromaticApproach),
        (50, F::LicensedExtension),
        (51, F::ChromaticApproach),
    ]
    .iter()
    .enumerate()
    .map(|(i, &(pitch, f))| {
        let mut n = note(3.0 + 0.25 * i as f64, 0.2, pitch, Role::Bass, "walk");
        n.function = Some(f);
        n
    })
    .chain(std::iter::once({
        let mut n = note(4.0, 1.0, 52, Role::Bass, "root");
        n.function = Some(F::ChordTone);
        n
    }))
    .collect();
    let audit = Audit::measure(&p, &s);
    for r in &audit.rows {
        assert!(
            !r.supported.contains(&F::SlidePath),
            "attacked note {} certified as a slide: {}",
            s.notes[r.note_index].pitch,
            r.detail
        );
    }
}

/// The route C–C#–D–D#: one attack gliding through the rest is a slide; four attacked notes are a
/// discrete line, never a slide; a single attacked G#→A is a chromatic approach. Every Note the
/// synth renders is attacked, so a declared SlidePath on attacked notes is a fake slide.
#[test]
fn r14_a_slide_is_one_attack() {
    let route = |entries: [Entry; 4]| -> Vec<PathEvent> {
        [48, 49, 50, 51]
            .iter()
            .zip(entries)
            .enumerate()
            .map(|(i, (&pitch, entry))| PathEvent {
                pitch,
                start_beat: 0.25 * i as f64,
                dur_beats: 0.2,
                entry,
            })
            .collect()
    };
    use Entry::{Attacked as At, Glided as Gl};
    assert_eq!(classify(&route([At, Gl, Gl, Gl])), Some(Gesture::Slide));
    assert_eq!(classify(&route([At; 4])), Some(Gesture::DiscreteLine));
    assert_eq!(
        classify(&route([At, Gl, At, Gl])),
        Some(Gesture::DiscreteLine)
    );
    assert_eq!(
        classify(&route([At; 4])[..2]),
        Some(Gesture::ChromaticApproach)
    );
    let n = note(0.0, 1.0, 56, Role::Bass, "approach");
    assert_eq!(entry_of(&n), Entry::Attacked);
    let (p, mut s) = fixture(&[(0.0, Quality::Maj, 0)], 118.0, 8.0);
    let line = |declared: F| -> Vec<Note> {
        [48, 49, 50, 51, 52]
            .iter()
            .enumerate()
            .map(|(i, &pitch)| {
                let mut n = note(3.0 + 0.25 * i as f64, 0.2, pitch, Role::Bass, "walk");
                n.function = Some(if i == 0 || i == 4 {
                    F::ChordTone
                } else {
                    declared
                });
                n
            })
            .collect()
    };
    s.notes = line(F::SlidePath);
    let g = GestureDiagnostics::measure(&p, &s);
    assert_eq!(
        (g.declared_slides, g.fake_slides),
        (3, 3),
        "{}",
        g.report(&s)
    );
    s.notes = line(F::ChromaticPassing);
    let g = GestureDiagnostics::measure(&p, &s);
    assert_eq!(
        (g.declared_slides, g.fake_slides),
        (0, 0),
        "{}",
        g.report(&s)
    );
    assert!(g
        .rows
        .iter()
        .all(|r| r.gesture != Some(Gesture::Slide) && r.attacks == r.route.len()));
}

/// What the keys, bass and drums realizers consumed of the lead: onset, length, pitch, function.
fn lead_as_heard(notes: &[Note]) -> Vec<(f64, f32, i32, Option<F>)> {
    notes
        .iter()
        .map(|n| (n.start_beat, n.dur_beats, n.pitch, n.function))
        .collect()
}

/// The keys, the bass and the drums are realized hearing the lead; the lead in the final score
/// must be the lead they heard. Round XII's is.
#[test]
fn r14_round12_band_heard_its_own_lead() {
    let song = stable_song();
    for world in [MusicWorld::swiss_signal(), MusicWorld::black_ice()] {
        let x = perform_temporal(&song, &world, PerformanceOptions::default());
        let heard = super::melody::realize_lead_temporal(&x.perf, &song.plan);
        let fin: Vec<Note> = x.score.role_notes(Role::Lead).copied().collect();
        assert_eq!(
            lead_as_heard(&heard.notes),
            lead_as_heard(&fin),
            "{}",
            world.name
        );
    }
}

/// Every dependent player must have heard the final upstream realization, and the score says
/// so itself ([`Score::stale_hearings`]): the keys, the bass and the drums hear the lead, the
/// drums hear the bass, the Round XIV pad hears lead, keys and bass. Round XIIIb re-times and
/// shortens 18 BLACK_ICE lead notes after the keys, the bass and the drums consumed them, so its
/// score is not the band that generated itself. Round XII's and Round XIV's are. (SWISS's Round
/// XIIIb edits are to the pad, which nobody there hears: its failure is identity, not causality.)
/// Committed failing against the re-realized lead; it passes on the hearing ledger.
#[test]
fn r14_every_dependent_heard_the_final_lead() {
    let song = stable_song();
    let opts = PerformanceOptions::default();
    for world in [MusicWorld::swiss_signal(), MusicWorld::black_ice()] {
        for x in [
            perform_temporal(&song, &world, opts),
            perform_coherent(&song, &world, opts),
        ] {
            assert_eq!(x.score.stale_hearings(), [], "{}", world.name);
            assert!(!x.score.hearings.is_empty());
        }
    }
    let c = perform_tension(&song, &MusicWorld::swiss_signal(), opts);
    assert_eq!(c.score.stale_hearings(), []);
    let c = perform_tension(&song, &MusicWorld::black_ice(), opts);
    let stale: Vec<(&str, Role, usize, usize)> = c
        .score
        .stale_hearings()
        .iter()
        .map(|h| (h.listener, h.source, h.missing.len(), h.unheard.len()))
        .collect();
    assert_eq!(
        stale,
        [
            ("keys", Role::Lead, 18, 18),
            ("bass", Role::Lead, 18, 18),
            ("drums", Role::Lead, 18, 18)
        ]
    );
    let heard = super::melody::realize_lead_temporal(&c.perf, &song.plan);
    let fin: Vec<Note> = c.score.role_notes(Role::Lead).copied().collect();
    let (h, f) = (lead_as_heard(&heard.notes), lead_as_heard(&fin));
    assert_eq!(h.iter().filter(|n| !f.contains(n)).count(), 18);
}

/// A pad pitch at an onset: (beat, MIDI).
type PadPitch = (f64, i32);

/// Pad pitches that differ between two bands as multisets: what left, what came.
fn pad_moves(a: &Score, b: &Score) -> (Vec<PadPitch>, Vec<PadPitch>) {
    let bag = |s: &Score| -> Vec<(f64, i32, f32)> {
        let mut v: Vec<_> = s
            .role_notes(Role::Pad)
            .map(|n| (n.start_beat, n.pitch, n.dur_beats))
            .collect();
        v.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
        v
    };
    let (x, y) = (bag(a), bag(b));
    let gone = x
        .iter()
        .filter(|n| !y.contains(n))
        .map(|n| (n.0, n.1))
        .collect();
    let came = y
        .iter()
        .filter(|n| !x.contains(n))
        .map(|n| (n.0, n.1))
        .collect();
    (gone, came)
}

/// Round XIV's arm against Round XII, as heard: no chart chord flips, and the band keeps the
/// chart's identity at least as well as Round XII at every instant. Only pad pitches change;
/// lead, keys, bass, drums, SFX, the plan, the song conformance, the interaction receipts and the
/// Round XII pitch-path counts are Round XII's.
#[test]
fn r14_coherent_arm_keeps_every_chart_chord() {
    use super::comp::PadVoicingReason as R;
    let song = stable_song();
    let opts = PerformanceOptions::default();
    for (world, spacings, rootings, moved) in [
        (MusicWorld::swiss_signal(), 4, 1, 5),
        (MusicWorld::black_ice(), 14, 10, 24),
    ] {
        let a = perform_temporal(&song, &world, opts);
        let d = perform_coherent(&song, &world, opts);
        let what = world.name;
        let ida = IdentityDiagnostics::measure(
            &a.score.notes,
            &a.perf.contexts,
            &world,
            a.score.tempo_bpm,
        );
        let idd = IdentityDiagnostics::measure(
            &d.score.notes,
            &d.perf.contexts,
            &world,
            d.score.tempo_bpm,
        );
        assert_eq!(
            idd.flips().count(),
            0,
            "{what}\n{}",
            idd.report(&d.score.notes, &d.perf.contexts)
        );
        assert!(keeps_identity(&ida, &idd, 0.0, 1e9), "{what}");
        assert_eq!(a.perf.fingerprint(), d.perf.fingerprint());
        for role in [Role::Lead, Role::Keys, Role::Bass] {
            let x: Vec<&Note> = a.score.role_notes(role).collect();
            let y: Vec<&Note> = d.score.role_notes(role).collect();
            assert_eq!(format!("{x:?}"), format!("{y:?}"), "{what} {role:?}");
        }
        assert_eq!(
            format!("{:?}", a.score.drums),
            format!("{:?}", d.score.drums)
        );
        assert_eq!(format!("{:?}", a.score.sfx), format!("{:?}", d.score.sfx));
        assert_eq!(a.score.notes.len(), d.score.notes.len());
        let conform =
            |x: &Composition| SongMapConformance::check(&song, &x.perf, &x.score).report();
        assert_eq!(conform(&a), conform(&d), "{what}");
        let receipts = |x: &Composition| -> Vec<bool> {
            super::witness::audit(&x.perf, &x.score)
                .rows
                .iter()
                .map(|r| r.witnessed)
                .collect()
        };
        assert_eq!(receipts(&a), receipts(&d), "{what}");
        let (ta, td) = (
            Audit::measure(&a.perf, &a.score),
            Audit::measure(&d.perf, &d.score),
        );
        assert_eq!(
            (
                ta.false_function_claims,
                ta.false_suspensions,
                ta.broken_anticipations,
                ta.unresolved_tendencies,
                ta.bad_arrivals
            ),
            (
                td.false_function_claims,
                td.false_suspensions,
                td.broken_anticipations,
                td.unresolved_tendencies,
                td.bad_arrivals
            ),
            "{what}"
        );
        let e = &d.score.pad_voicing_edits;
        let n_space = e
            .iter()
            .filter(|x| matches!(x.reason, R::Spacing { .. }))
            .count();
        let n_root = e
            .iter()
            .filter(|x| matches!(x.reason, R::Rooting { .. }))
            .count();
        assert_eq!((n_space, n_root), (spacings, rootings), "{what}: {e:?}");
        // Every spacing keeps the voicing's pitch classes; every rooting adds the chart's root.
        for x in e {
            let pcs = |v: &[i32]| {
                let mut p: Vec<i32> = v.iter().map(|&m| pitch_class(m)).collect();
                p.sort_unstable();
                p
            };
            match x.reason {
                R::Spacing { from, to } => {
                    assert_eq!(pcs(&x.before), pcs(&x.after));
                    assert_eq!((to - from).abs() % 12, 0);
                }
                R::Rooting { root, .. } => {
                    let chart = d.perf.contexts[x.context].chord.root_pc;
                    assert_eq!(pitch_class(root), chart);
                    assert!(!x.before.iter().any(|&m| pitch_class(m) == chart));
                }
            }
        }
        let (gone, came) = pad_moves(&a.score, &d.score);
        assert_eq!(
            (gone.len(), came.len()),
            (moved, moved),
            "{what}: {gone:?} -> {came:?}"
        );
    }
}

/// SWISS's ~16 s under Round XIV: the chart's Cmaj7 at every probe second, the pad's B moved an
/// octave up out of the B4–C5 cluster, the C kept.
#[test]
fn r14_swiss_sixteen_seconds_is_still_cmaj7() {
    let song = stable_song();
    let world = MusicWorld::swiss_signal();
    let d = perform_coherent(&song, &world, PerformanceOptions::default());
    let id = IdentityDiagnostics::measure(&d.score.notes, &d.perf.contexts, &world, 118.0);
    for secs in [15.0, 15.5, 16.0, 16.5, 17.0] {
        let s = id.at_beat(secs * 118.0 / 60.0).expect("sound");
        assert_eq!(s.status, IdentityStatus::Rooted, "{secs} s");
    }
    let win = heard_windows(&d.score.notes, &world, 118.0);
    let beat = 16.0 * 118.0 / 60.0;
    let mut at16: Vec<(Role, i32)> = (0..d.score.notes.len())
        .filter(|&i| win[i].0 <= beat && beat < win[i].1)
        .map(|i| (d.score.notes[i].role, d.score.notes[i].pitch))
        .collect();
    at16.sort_by_key(|&(r, p)| (r.label(), p));
    assert_eq!(
        at16,
        [
            (Role::Bass, 47),
            (Role::Pad, 64),
            (Role::Pad, 72),
            (Role::Pad, 79),
            (Role::Pad, 83)
        ]
    );
}

/// The Round XIV arm across 2 stories × 2 lengths × 10 seeds × 2 composers × 3 worlds (240
/// performances), against Round XII: identity no worse at any instant; the plan, the song
/// conformance, lead, keys, bass, drums and SFX unchanged; no interaction receipt lost; no new
/// Round XII false claim; every edit a pad pitch class kept (spacing) or the chart's root added
/// (rooting). Prints what remains flipped: the flips the pad cannot reach (it is silent there, or
/// no admissible voicing helps).
#[test]
#[ignore = "sweep: run with --ignored --nocapture"]
fn fuzz_the_coherent_arm_keeps_every_chart_chord() {
    use super::comp::PadVoicingReason as R;
    use super::contract::CompositionGrammar;
    use super::semantic::demo_trace;
    let (mut runs, mut before, mut after, mut spacings, mut rootings) = (0, 0, 0, 0, 0);
    let (mut secs_before, mut secs_after) = (0.0, 0.0);
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
                        let a = perform_temporal(&song, &world, opts);
                        let d = perform_coherent(&song, &world, opts);
                        let what = format!("{story}/{beats}/{seed}/{composer:?}/{}", world.name);
                        runs += 1;
                        let id = |x: &Composition| {
                            IdentityDiagnostics::measure(
                                &x.score.notes,
                                &x.perf.contexts,
                                &world,
                                x.score.tempo_bpm,
                            )
                        };
                        let (ia, id_) = (id(&a), id(&d));
                        assert!(keeps_identity(&ia, &id_, 0.0, 1e9), "{what}");
                        before += ia.flips().count();
                        after += id_.flips().count();
                        secs_before += ia.flipped_secs();
                        secs_after += id_.flipped_secs();
                        assert_eq!(a.perf.fingerprint(), d.perf.fingerprint(), "{what}");
                        let law = |x: &Composition| {
                            SongMapConformance::check(&song, &x.perf, &x.score).report()
                        };
                        assert_eq!(law(&a), law(&d), "{what}");
                        for role in [Role::Lead, Role::Keys, Role::Bass] {
                            let x: Vec<&Note> = a.score.role_notes(role).collect();
                            let y: Vec<&Note> = d.score.role_notes(role).collect();
                            assert_eq!(format!("{x:?}"), format!("{y:?}"), "{what} {role:?}");
                        }
                        assert_eq!(
                            format!("{:?}{:?}", a.score.drums, a.score.sfx),
                            format!("{:?}{:?}", d.score.drums, d.score.sfx),
                            "{what}"
                        );
                        let (wa, wd) = (
                            super::witness::audit(&a.perf, &a.score),
                            super::witness::audit(&d.perf, &d.score),
                        );
                        for (x, y) in wa.rows.iter().zip(&wd.rows) {
                            assert!(!x.witnessed || y.witnessed, "{what}: lost {:?}", x.action);
                        }
                        let claims = |x: &Composition| {
                            let t = Audit::measure(&x.perf, &x.score);
                            t.false_function_claims
                                + t.false_suspensions
                                + t.broken_anticipations
                                + t.bad_arrivals
                                + t.unresolved_tendencies
                        };
                        assert!(claims(&d) <= claims(&a), "{what}");
                        assert_eq!(a.score.stale_hearings(), [], "{what}");
                        assert_eq!(d.score.stale_hearings(), [], "{what}");
                        assert!(
                            a.score
                                .notes
                                .iter()
                                .chain(&d.score.notes)
                                .all(|n| n.function != Some(F::SlidePath)),
                            "{what}: a realizer declared a slide on attacked notes"
                        );
                        for e in &d.score.pad_voicing_edits {
                            match e.reason {
                                R::Spacing { from, to } => {
                                    spacings += 1;
                                    assert_eq!(pitch_class(from), pitch_class(to), "{what}");
                                }
                                R::Rooting { root, .. } => {
                                    rootings += 1;
                                    assert_eq!(
                                        pitch_class(root),
                                        d.perf.contexts[e.context].chord.root_pc,
                                        "{what}"
                                    );
                                }
                            }
                        }
                        for r in id_.flips() {
                            println!(
                                "remaining {what}: {:.2}-{:.2} s {} heard as {} ({:.3} s)",
                                id_.secs(r.start_beat),
                                id_.secs(r.end_beat),
                                r.chart.label(),
                                r.rival.label(),
                                r.secs
                            );
                        }
                    }
                }
            }
        }
    }
    println!(
        "coherent sweep: {runs} performances; flips {before} ({secs_before:.1} s) -> {after} ({secs_after:.1} s); pad edits: {spacings} spacings, {rootings} rootings"
    );
    assert!(after < before);
}

/// Every bass chromatic approach in the acceptance songs is ONE attacked note leaning a semitone
/// into its target on the next onset, the target a chord tone of its own harmony, and the Round
/// XII observer independently supports the approach. Nothing is left for a slide to justify.
#[test]
fn r14_every_bass_approach_is_one_attacked_note_into_its_target() {
    let song = stable_song();
    for (world, count) in [
        (MusicWorld::swiss_signal(), 18),
        (MusicWorld::black_ice(), 18),
    ] {
        let d = perform_coherent(&song, &world, PerformanceOptions::default());
        let audit = Audit::measure(&d.perf, &d.score);
        let bass: Vec<usize> = (0..d.score.notes.len())
            .filter(|&i| d.score.notes[i].role == Role::Bass)
            .collect();
        let mut seen = 0;
        for (k, &i) in bass.iter().enumerate() {
            let n = &d.score.notes[i];
            if n.function != Some(F::ChromaticApproach) {
                continue;
            }
            seen += 1;
            let t = &d.score.notes[bass[k + 1]];
            let what = format!("{} bass {} at {}", world.name, n.pitch, n.start_beat);
            assert_eq!((t.pitch - n.pitch).abs(), 1, "{what}");
            assert!(t.start_beat - n.start_beat <= 0.5 + 1e-9, "{what}");
            assert!(
                n.start_beat + f64::from(n.dur_beats) <= t.start_beat + 1e-9,
                "{what}"
            );
            let ctx = d.perf.context_at(t.start_beat).unwrap();
            assert!(ctx.chord.contains_pc(pitch_class(t.pitch)), "{what}");
            let route = [PathEvent::of(n), PathEvent::of(t)];
            assert_eq!(classify(&route), Some(Gesture::ChromaticApproach), "{what}");
            let row = audit.rows.iter().find(|r| r.note_index == i).unwrap();
            assert!(
                row.supported.contains(&F::ChromaticApproach),
                "{what}: {}",
                row.detail
            );
        }
        assert_eq!(seen, count, "{}", world.name);
    }
}
