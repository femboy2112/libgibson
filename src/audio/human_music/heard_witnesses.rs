//! Round XIV pre-intervention falsifiers: the object the listener hears must commute with the one
//! the plan says is there.
//!
//! The maintainer's Round XIIIb listen: SWISS's ~16 s is now a different chord, and the band is no
//! longer the system that generated itself. Round XIIIb took the pad's C5 out of the Cmaj7 at
//! beat 28 because the bass sounded C at the pad's onset, but the bass then walks E2 G2 B2 while
//! the pad holds E4 B4 G5, so 15.3–16.0 s is heard as Em over its own fifth. Each witness asserts
//! a contract the current machinery cannot satisfy; they are ignored until the heard-object
//! audits land. `docs/fixtures/humanmusic-r14/baseline-failures.txt` records the failing run.
use super::composer::Composer;
use super::context::{analyze, HarmonicContext};
use super::form::SectionKind;
use super::functor::{perform_temporal, perform_tension, Composition};
use super::harmony::ChordSpan;
use super::performance::{PerformanceOptions, PerformancePlan};
use super::score::{Note, PitchFunction as F, Provenance, Role, Score};
use super::semantic::deflected_lift_trace;
use super::song::SongMap;
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

/// The existing identity law must hear Round XIIIb's flip as a regression over Round XII. It
/// cannot: its root memory lasts two beats past the root's audible end and is clamped to the
/// harmony's start, so a downbeat root certifies the whole bar.
#[test]
#[ignore = "Round XIV falsifier: the Round VIII identity law cannot hear the R13b flip"]
fn r14_identity_law_hears_the_r13b_flip() {
    let song = stable_song();
    let world = MusicWorld::swiss_signal();
    let opts = PerformanceOptions::default();
    let (a, c) = (
        perform_temporal(&song, &world, opts),
        perform_tension(&song, &world, opts),
    );
    let (fa, fc) = (round8_flip_beats(&a, &world), round8_flip_beats(&c, &world));
    assert!(
        fc > fa,
        "R12 flip beats {fa}, R13b flip beats {fc}: the removed root is inaudible to the law"
    );
}

/// A note may be removed as "covered" only while its pitch class keeps sounding for as long as
/// the note would have. Round XIIIb's proof reads one instant, the note's onset.
#[test]
#[ignore = "Round XIV falsifier: R13b removes a root the bass covers only at the onset"]
fn r14_coverage_must_hold_for_as_long_as_the_note_is_needed() {
    let world = MusicWorld::swiss_signal();
    let ctx = contexts(&[(0.0, Quality::Maj7, 0)], 8.0);
    let before = walk_under_cmaj7();
    let mut notes = before.clone();
    let edits = gate_sounding_tension(&mut notes, &ctx, &world, 118.0, 4.0, None);
    let win = heard_windows(&before, &world, 118.0);
    for e in edits
        .iter()
        .filter(|e| e.action == TensionAction::OmittedCovered)
    {
        let i = before
            .iter()
            .position(|n| {
                n.role == e.note.role
                    && n.pitch == e.note.pitch
                    && n.start_beat == e.note.start_beat
            })
            .expect("the edited note");
        let (a, b) = win[i];
        assert!(
            sounded_throughout(&before, i, pitch_class(e.note.pitch), a, b),
            "{:?} {} removed as covered over [{a}, {b}), but its pitch class stops sounding",
            e.note.role,
            e.note.pitch
        );
    }
}

/// The same route C–C#–D–D#–E as five separately attacked notes is a discrete line: the synth
/// triggers a fresh voice per note, so no intermediate may be certified as a slide.
#[test]
#[ignore = "Round XIV falsifier: the Round XII observer certifies attacked notes as a slide"]
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

/// Round XIIIb re-times and shortens BLACK_ICE lead notes after the keys, the bass and the drums
/// have heard them: the final score is not the band that generated itself.
#[test]
#[ignore = "Round XIV falsifier: R13b edits the lead after its dependents heard it"]
fn r14_every_dependent_heard_the_final_lead() {
    let song = stable_song();
    let world = MusicWorld::black_ice();
    let x = perform_tension(&song, &world, PerformanceOptions::default());
    let heard = super::melody::realize_lead_temporal(&x.perf, &song.plan);
    let fin: Vec<Note> = x.score.role_notes(Role::Lead).copied().collect();
    let (h, f) = (lead_as_heard(&heard.notes), lead_as_heard(&fin));
    let stale: Vec<_> = h.iter().filter(|n| !f.contains(n)).collect();
    assert!(
        stale.is_empty(),
        "{} lead notes heard by keys/bass/drums are not in the final score: {stale:?}",
        stale.len()
    );
}
