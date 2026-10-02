//! Closeout round: a relational pitch function is claimed only where its destination is realized
//! inside the finite performance and holds the relation the function names.
//!
//! An appoggiatura is not "an accented non-chord tone". It is a relation: an accented tone, leapt
//! into, that resolves by step onto a core tone of the harmony sounding where it resolves. A fresh
//! exploratory BAND search (seeds `9680xxxx`, outside every declared sweep and holdout) rejected 60
//! of 7,200 performances in the family `deflected_lift_trace(33.25) + StructuralR9 + DeflectedLift +
//! FusionConversation + BLACK_ICE` with one false temporal function claim each.
//!
//! The mechanism (Observed, seed 96_800_004): the restated motif at beat 24.5 plays A5 -> F#6
//! (25.0, accented, leapt into) -> E6 (25.5) over Dm9. The source line engine labelled F#6
//! `Appoggiatura` because its destination E is a SPELLED member of Dm9 — its 9th, a colour tone. The
//! destination was not truncated, omitted or displaced: it sounds one step away, mid-form, inside
//! the domain. Truncation is only the trigger (a 1-2 beat final bar moves the cadential Dm9 under
//! the motif). A 9th is not a resolution: the claimed relation never holds.
//!
//! The oracle below restates the relation independently of `temporal::TemporalPitchDiagnostics`
//! (which stays unchanged and strict).
use gibson::audio::human_music::{
    composer::Composer,
    contract::CompositionGrammar,
    functor::{perform_checked, perform_with_profile, Composition},
    language::MusicalLanguage,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    score::{Note, PitchFunction, Role, Score},
    semantic::{deflected_lift_trace, demo_trace},
    temporal::TemporalPitchDiagnostics,
    MusicWorld, SongMap,
};

const EPS: f64 = 1e-6;
/// How long after an accented tone ends its resolution may arrive and still be heard as one.
const RESOLVES_WITHIN: f64 = 1.0;

/// Whether `pitch` is a core tone (root, 3rd, 5th, 7th/6th — every interval below the 9th) of the
/// chord the score sounds at `beat`.
fn core_at(score: &Score, beat: f64, pitch: i32) -> bool {
    score
        .chords
        .iter()
        .find(|s| beat >= s.start_beat - EPS && beat < s.start_beat + f64::from(s.dur_beats) - EPS)
        .is_some_and(|s| {
            s.chord
                .quality
                .intervals()
                .iter()
                .any(|&i| i < 12 && (s.chord.root_pc + i).rem_euclid(12) == pitch.rem_euclid(12))
        })
}

/// The appoggiatura relation, restated: the claimed lead note is leapt into, and its next lead
/// note sounds inside the piece, within the resolution window, a step away, on a core tone of the
/// harmony at its own onset. Returns every lead note that claims the function without the relation.
fn false_appoggiaturas(score: &Score) -> Vec<(f64, i32)> {
    let lead: Vec<&Note> = score.role_notes(Role::Lead).collect();
    let mut out = Vec::new();
    for (i, n) in lead.iter().enumerate() {
        if n.function != Some(PitchFunction::Appoggiatura) {
            continue;
        }
        let leapt = i
            .checked_sub(1)
            .map(|j| lead[j])
            .is_some_and(|p| (n.pitch - p.pitch).abs() > 2);
        let resolves = lead.get(i + 1).is_some_and(|q| {
            let end = n.start_beat + f64::from(n.dur_beats);
            q.start_beat > n.start_beat + EPS
                && q.start_beat <= end + RESOLVES_WITHIN + EPS
                && q.start_beat < score.total_beats - EPS
                && (1..=2).contains(&(q.pitch - n.pitch).abs())
                && core_at(score, q.start_beat, q.pitch)
        });
        if !(leapt && resolves) {
            out.push((n.start_beat, n.pitch));
        }
    }
    out
}

fn fusion() -> PerformanceOptions {
    PerformanceOptions {
        language: MusicalLanguage::fusion_conversation(),
        ..PerformanceOptions::default()
    }
}

/// The fresh falsifiers: one song shape, three seeds.
const FALSIFIER_SEEDS: [u64; 3] = [96_800_004, 96_800_005, 96_800_009];

fn falsifier(seed: u64) -> SongMap {
    SongMap::compose(
        &deflected_lift_trace(33.25),
        seed,
        Some(CompositionGrammar::DeflectedLift),
        Composer::StructuralR9,
    )
}

#[test]
fn band_claims_an_appoggiatura_only_where_it_resolves_onto_a_core_tone() {
    for seed in FALSIFIER_SEEDS {
        let take = perform_with_profile(
            &falsifier(seed),
            &MusicWorld::black_ice(),
            fusion(),
            PerformanceProfile::BAND,
        )
        .expect("lawful candidate");
        let false_claims = false_appoggiaturas(&take.score);
        assert!(
            false_claims.is_empty(),
            "seed {seed}: appoggiaturas without their resolution: {false_claims:?}"
        );
    }
}

#[test]
fn band_admits_the_relational_function_falsifiers() {
    for seed in FALSIFIER_SEEDS {
        if let Err(e) = perform_checked(
            &falsifier(seed),
            &MusicWorld::black_ice(),
            fusion(),
            PerformanceProfile::BAND,
        ) {
            panic!("seed {seed}: {e}");
        }
    }
}

/// A genuine appoggiatura (fresh seed): C5 at 17.0, leapt into, resolving down a semitone onto B4
/// (a core tone) at 17.5.
fn genuine() -> Composition {
    let song = SongMap::compose(
        &demo_trace(33.25),
        96_850_000,
        Some(CompositionGrammar::DeflectedLift),
        Composer::StructuralR9,
    );
    perform_with_profile(
        &song,
        &MusicWorld::black_ice(),
        fusion(),
        PerformanceProfile::BAND,
    )
    .expect("lawful candidate")
}

fn genuine_appoggiatura(c: &Composition) -> (usize, usize) {
    let lead: Vec<(usize, &Note)> = c
        .score
        .notes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.role == Role::Lead)
        .collect();
    let k = lead
        .iter()
        .position(|(_, n)| {
            n.function == Some(PitchFunction::Appoggiatura) && (n.start_beat - 17.0).abs() < EPS
        })
        .expect("the genuine appoggiatura at 17.0 is still claimed");
    (lead[k].0, lead[k + 1].0)
}

/// Whether the (unchanged) temporal observer reconstructs no support for note `note`'s declared
/// function in `score`.
fn observer_refutes(c: &Composition, score: &Score, note: usize) -> bool {
    let declared = score.notes[note].function;
    TemporalPitchDiagnostics::measure(&c.perf, score)
        .rows
        .iter()
        .find(|r| r.note_index == note)
        .is_some_and(|r| declared.is_some_and(|f| !r.supported.contains(&f)))
}

/// Control: a genuine resolved appoggiatura keeps its claim, and both the oracle and the observer
/// accept it.
#[test]
fn a_genuine_resolved_appoggiatura_still_stands() {
    let c = genuine();
    let (n, _) = genuine_appoggiatura(&c);
    assert!(false_appoggiaturas(&c.score).is_empty());
    assert!(!observer_refutes(&c, &c.score, n));
}

/// Control: forging the claim without its resolution stays red — delete the destination.
#[test]
fn a_forged_unresolved_appoggiatura_stays_red() {
    let c = genuine();
    let (n, q) = genuine_appoggiatura(&c);
    let mut forged = c.score.clone();
    forged.notes.remove(q);
    let n_at = if q < n { n - 1 } else { n };
    assert!(!false_appoggiaturas(&forged).is_empty());
    assert!(
        observer_refutes(&c, &forged, n_at),
        "the observer must still refuse an appoggiatura with no resolution"
    );
}

/// A score cut at `at`: every event at or after it is gone, every earlier event ends by it.
fn cut(score: &Score, at: f64) -> Score {
    let mut s = score.clone();
    s.total_beats = at;
    s.notes.retain(|n| n.start_beat < at - EPS);
    for n in &mut s.notes {
        let end = n.start_beat + f64::from(n.dur_beats);
        if end > at {
            n.dur_beats = (at - n.start_beat) as f32;
        }
    }
    s
}

/// Where note `note` of `from` sits in `into` (the same role, onset and pitch).
fn locate(from: &Score, into: &Score, note: usize) -> usize {
    let n = &from.notes[note];
    into.notes
        .iter()
        .position(|x| {
            x.role == n.role && x.pitch == n.pitch && (x.start_beat - n.start_beat).abs() < EPS
        })
        .expect("the accented tone precedes every cut")
}

/// Control: a cut just AFTER the resolution's onset leaves the relation intact.
#[test]
fn truncation_after_the_resolution_keeps_the_claim() {
    let c = genuine();
    let (n, q) = genuine_appoggiatura(&c);
    let after = cut(&c.score, c.score.notes[q].start_beat + 0.01);
    assert!(false_appoggiaturas(&after).is_empty());
    assert!(
        !observer_refutes(&c, &after, locate(&c.score, &after, n)),
        "the resolution sounds inside the cut piece"
    );
}

/// Control: a cut just BEFORE the resolution's onset removes the destination; the claim is then
/// false for both the oracle and the observer.
#[test]
fn truncation_before_the_resolution_breaks_the_claim() {
    let c = genuine();
    let (n, q) = genuine_appoggiatura(&c);
    let before = cut(&c.score, c.score.notes[q].start_beat - 0.01);
    assert!(!false_appoggiaturas(&before).is_empty());
    assert!(
        observer_refutes(&c, &before, locate(&c.score, &before, n)),
        "a destination outside the piece never resolves the claim"
    );
}
