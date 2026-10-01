//! Closeout round: harmonic identity is the responsibility of the sounding support.
//!
//! A fresh exploratory BAND search (seeds `9680xxxx`, outside every declared sweep and holdout)
//! rejected 60 of 7,200 VAPOR95/fusion performances with one held-identity flip each, in two shapes
//! (Observed, decomposed by an independent adversarial pass):
//!
//! - **The pad voices the harmony and its rooting repair is vetoed** (seed 96_800_019,
//!   `demo_trace(7.25)`, Gm7 over beats 0-2). The pad holds a rootless F4 + A#4, the bass sounds G2
//!   for a quarter beat then D2: D-F-Bb (B-flat major) holds 1.3 s with no G. The pad's existing
//!   rooting law finds the right edit (add G4) — and its strict acceptance throws it away because
//!   the added root's tail leaves a 0.15-beat sub-hold `Passing` Gm at the next chord, an overlap
//!   the hold law itself permits. A held flip kept to avoid a lawful blip.
//! - **Nobody voices the harmony's identity** (seed 96_800_001, `deflected_lift_trace(33.25)`, Dm7
//!   over beats 8-12). A Thin verb silences the pad for the bar; the keys hold a rootless
//!   F4-C5-E5; the bass sounds D2 for under half a beat, then A2. A-C-E (A minor) holds 2.4 s with
//!   no D, and no player is responsible for the chart's chord.
//!
//! The oracle below restates the hold law independently of `identity::IdentityDiagnostics`, from
//! WRITTEN durations only (no release tails, no continuity links): within one chart chord, a span
//! where the chord's root does not sound, a support voice (pad or keys) does, and a complete major
//! or minor triad on another root — containing the lowest sounding pitch — sounds throughout, held
//! at least 0.5 s, is the band playing another chord. Written durations are the conservative
//! reading: seed 96_800_016 at 13.25 beats flips only through release tails, so there only the
//! receipt (which hears tails) names it; the other three are held rivals even without a tail.
use gibson::audio::human_music::{
    composer::Composer,
    contract::CompositionGrammar,
    functor::{perform_checked, perform_with_profile},
    language::MusicalLanguage,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    score::{Role, Score},
    semantic::{deflected_lift_trace, demo_trace},
    MusicWorld, SemanticTrace, SongMap,
};

const EPS: f64 = 1e-6;
/// How long a rival must hold to replace the chart's chord.
const HOLD_SECS: f64 = 0.5;

/// Every held rival in `score`: `(chord start, rival root pc, rival is minor, seconds)`.
fn held_rivals(score: &Score) -> Vec<(f64, i32, bool, f64)> {
    let pitched: Vec<_> = score
        .notes
        .iter()
        .filter(|n| matches!(n.role, Role::Pad | Role::Keys | Role::Bass | Role::Lead))
        .collect();
    let mut out = Vec::new();
    for span in &score.chords {
        let (a, b) = (span.start_beat, span.start_beat + f64::from(span.dur_beats));
        let root = span.chord.root_pc.rem_euclid(12);
        let mut cuts: Vec<f64> = vec![a, b];
        for n in &pitched {
            for t in [n.start_beat, n.start_beat + f64::from(n.dur_beats)] {
                if t > a + EPS && t < b - EPS {
                    cuts.push(t);
                }
            }
        }
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|x, y| (*x - *y).abs() < EPS);
        let mut run: Option<(i32, bool, f64)> = None;
        let mut close = |run: &mut Option<(i32, bool, f64)>, out: &mut Vec<_>| {
            if let Some((r, minor, beats)) = run.take() {
                let secs = beats * 60.0 / f64::from(score.tempo_bpm);
                if secs >= HOLD_SECS - EPS {
                    out.push((a, r, minor, secs));
                }
            }
        };
        for w in cuts.windows(2) {
            let mid = 0.5 * (w[0] + w[1]);
            let sounding: Vec<_> = pitched
                .iter()
                .filter(|n| n.start_beat <= mid && mid < n.start_beat + f64::from(n.dur_beats))
                .collect();
            let pcs: Vec<i32> = sounding.iter().map(|n| n.pitch.rem_euclid(12)).collect();
            let support = sounding
                .iter()
                .any(|n| matches!(n.role, Role::Pad | Role::Keys));
            let lowest = sounding.iter().map(|n| n.pitch).min();
            let rival = (support && !pcs.contains(&root))
                .then_some(lowest)
                .flatten()
                .and_then(|low| {
                    (0..12)
                        .filter(|&r| r != root)
                        .flat_map(|r| [(r, false), (r, true)])
                        .find(|&(r, minor)| {
                            let triad = [r, (r + if minor { 3 } else { 4 }) % 12, (r + 7) % 12];
                            triad.iter().all(|pc| pcs.contains(pc))
                                && triad.contains(&low.rem_euclid(12))
                        })
                });
            match (rival, run) {
                (Some((r, m)), Some((rr, rm, beats))) if r == rr && m == rm => {
                    run = Some((rr, rm, beats + (w[1] - w[0])));
                }
                (Some((r, m)), _) => {
                    close(&mut run, &mut out);
                    run = Some((r, m, w[1] - w[0]));
                }
                (None, _) => close(&mut run, &mut out),
            }
        }
        close(&mut run, &mut out);
    }
    out
}

struct Case {
    what: &'static str,
    trace: fn(f64) -> SemanticTrace,
    beats: f64,
    seed: u64,
}

/// The fresh falsifiers: one per shape, plus a second of each.
const FALSIFIERS: [Case; 4] = [
    Case {
        what: "pad rooting vetoed",
        trace: demo_trace,
        beats: 7.25,
        seed: 96_800_019,
    },
    Case {
        what: "pad rooting vetoed",
        trace: demo_trace,
        beats: 13.25,
        seed: 96_800_016,
    },
    Case {
        what: "no responsible voice",
        trace: deflected_lift_trace,
        beats: 33.25,
        seed: 96_800_001,
    },
    Case {
        what: "no responsible voice",
        trace: deflected_lift_trace,
        beats: 33.25,
        seed: 96_800_016,
    },
];

fn song(c: &Case) -> SongMap {
    SongMap::compose(
        &(c.trace)(c.beats),
        c.seed,
        Some(CompositionGrammar::DeflectedLift),
        Composer::StructuralR9,
    )
}

fn fusion() -> PerformanceOptions {
    PerformanceOptions {
        language: MusicalLanguage::fusion_conversation(),
        ..PerformanceOptions::default()
    }
}

#[test]
fn band_support_preserves_the_charted_identity() {
    let mut failures = Vec::new();
    for c in &FALSIFIERS {
        let take = perform_with_profile(
            &song(c),
            &MusicWorld::vapor95(),
            fusion(),
            PerformanceProfile::BAND,
        )
        .expect("lawful candidate");
        let rivals = held_rivals(&take.score);
        if !rivals.is_empty() {
            failures.push(format!("seed {} ({}): {rivals:?}", c.seed, c.what));
        }
    }
    assert!(
        failures.is_empty(),
        "the band holds another chord: {failures:#?}"
    );
}

#[test]
fn band_admits_the_support_identity_falsifiers() {
    let failures: Vec<String> = FALSIFIERS
        .iter()
        .filter_map(|c| {
            perform_checked(
                &song(c),
                &MusicWorld::vapor95(),
                fusion(),
                PerformanceProfile::BAND,
            )
            .err()
            .map(|e| format!("seed {} ({}): {e}", c.seed, c.what))
        })
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
}
