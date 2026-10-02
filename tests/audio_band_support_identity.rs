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
//!
//! Every VAPOR95 case here was derived under the archival VAPOR95 v1 world (before the palette
//! revision) and is performed under it, so each falsifier keeps exercising its mechanism.
#[path = "common/vapor95_v1.rs"]
mod vapor95_v1;

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
        let close = |run: &mut Option<(i32, bool, f64)>, out: &mut Vec<_>| {
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
            &vapor95_v1::vapor95_v1(),
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
                &vapor95_v1::vapor95_v1(),
                fusion(),
                PerformanceProfile::BAND,
            )
            .err()
            .map(|e| format!("seed {} ({}): {e}", c.seed, c.what))
        })
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The second-wave falsifier the fresh full grid found after the pad's law alone: Dm7 at 48, the
/// pad silent (a Thin bar), the keys holding C-F-G and the pad's E5 tail from the bar before
/// completing C major. The keys answer: G gives way to the root.
fn keys_answer() -> gibson::audio::human_music::functor::Composition {
    let song = SongMap::compose(
        &deflected_lift_trace(64.0),
        96_860_001,
        Some(CompositionGrammar::DeflectedLift),
        Composer::StructuralR9,
    );
    perform_checked(
        &song,
        &vapor95_v1::vapor95_v1(),
        fusion(),
        PerformanceProfile::BAND,
    )
    .expect("admitted")
}

/// Control: rootless voicings are not banned. The keys answer for the one harmony the band would
/// hear as another chord and leave every other rootless voicing as it was.
#[test]
fn a_rootless_voicing_nobody_hears_as_another_chord_stays_rootless() {
    let c = keys_answer();
    let edits = &c.score.keys_voicing_edits;
    assert_eq!(edits.len(), 1, "{edits:?}");
    assert!((edits[0].start_beat - 48.0).abs() < EPS);
    assert_eq!(
        edits[0].before.len(),
        edits[0].after.len(),
        "one voice replaced, none added: {edits:?}"
    );
    let rootless_elsewhere = c
        .score
        .chords
        .iter()
        .filter(|s| (s.start_beat - 48.0).abs() > EPS)
        .filter(|s| {
            let (a, b) = (s.start_beat, s.start_beat + f64::from(s.dur_beats));
            let keys: Vec<i32> = c
                .score
                .role_notes(Role::Keys)
                .filter(|n| n.start_beat >= a - EPS && n.start_beat < b - EPS)
                .map(|n| n.pitch.rem_euclid(12))
                .collect();
            !keys.is_empty() && !keys.contains(&s.chord.root_pc.rem_euclid(12))
        })
        .count();
    assert!(
        rootless_elsewhere >= 3,
        "rootless keys voicings elsewhere stay rootless (found {rootless_elsewhere})"
    );
}

/// Control: a forged rival is still detected — put the keys' G back in place of the root they
/// sounded, and both the oracle and the receipt hear C major again.
#[test]
fn a_forged_rival_identity_is_still_detected() {
    use gibson::audio::human_music::receipt::PerformanceReceipt;
    let mut c = keys_answer();
    let edit = c.score.keys_voicing_edits[0].clone();
    let (root, replaced) = match edit.reason {
        gibson::audio::human_music::comp::PadVoicingReason::Rooting { root, replaced, .. } => {
            (root, replaced.expect("a voice was replaced"))
        }
        other => panic!("unexpected edit {other:?}"),
    };
    let mut forged = 0;
    for n in c.score.notes.iter_mut().filter(|n| {
        n.role == Role::Keys && n.pitch == root && (n.start_beat - edit.start_beat).abs() < EPS
    }) {
        n.pitch = replaced;
        forged += 1;
    }
    assert!(forged > 0, "the keys' root sounds at the edit");
    // This rival is completed by the pad's E5 release TAIL from the bar before, which the
    // written-duration oracle never hears by construction; the receipt hears tails.
    let receipt =
        PerformanceReceipt::measure_under(&c, &vapor95_v1::vapor95_v1(), PerformanceProfile::BAND);
    assert!(
        receipt.held_identity_flips > 0,
        "the receipt still rejects a forged rival: {:?}",
        receipt.failures()
    );
}

/// The third wave, from the broad fresh search after the support law (16 of 195,264
/// performances, three songs): the responsible support's answer was there but could not land.
///
/// - 96_900_010 (BLACK_ICE, Am9 at 40): the pad's root removes E minor, but its tail joins the
///   NEXT harmony's flip, which the same sweep answers one step later — a single sweep vetoes it.
/// - 96_910_000 (VAPOR95 at 66 BPM, Fmaj7 at 4): every root the pad can add rings its tail into the
///   next harmony and completes a new F major there — a flip traded for a flip.
/// - 96_910_002 (VAPOR95 at 96 BPM, a 1.25-beat final F6): the pad is silent, but its consonant
///   Am7 tails from the harmony before complete A minor over a bass that leaves the root.
struct Wave {
    beats: f64,
    seed: u64,
    grammar: CompositionGrammar,
    composer: Composer,
    world: fn() -> MusicWorld,
    tempo: Option<f32>,
}

const THIRD_WAVE: [Wave; 3] = [
    Wave {
        beats: 64.0,
        seed: 96_900_010,
        grammar: CompositionGrammar::DeflectedLift,
        composer: Composer::MeaningDirected,
        world: MusicWorld::black_ice,
        tempo: None,
    },
    Wave {
        beats: 64.0,
        seed: 96_910_000,
        grammar: CompositionGrammar::DeflectedLift,
        composer: Composer::MeaningDirected,
        world: vapor95_v1::vapor95_v1,
        tempo: Some(66.0),
    },
    Wave {
        beats: 33.25,
        seed: 96_910_002,
        grammar: CompositionGrammar::PropulsiveReturn,
        composer: Composer::StructuralR9,
        world: vapor95_v1::vapor95_v1,
        tempo: Some(96.0),
    },
];

#[test]
fn band_admits_the_third_wave_of_support_identity() {
    let failures: Vec<String> = THIRD_WAVE
        .iter()
        .filter_map(|w| {
            let song = SongMap::compose(
                &deflected_lift_trace(w.beats),
                w.seed,
                Some(w.grammar),
                w.composer,
            );
            let mut world = (w.world)();
            if let Some(t) = w.tempo {
                world.tempo_bpm = t;
            }
            perform_checked(&song, &world, fusion(), PerformanceProfile::BAND)
                .err()
                .map(|e| format!("seed {}: {e}", w.seed))
        })
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Regression found by the closeout listening diff, after holdout v5's contact: the pad's tail
/// law looked up "the next harmony" with a lookup that has NO harmony past the piece's end, so
/// the final harmony's consonant tail counted as excluded and was cut before the end — in 360 of
/// 450 BAND performances (0 of 450 before). No law states that. The final harmony's pad, when
/// nothing released it, keeps its written gate and rings out as before.
#[test]
fn the_final_harmony_rings_out_unless_a_law_releases_it() {
    use gibson::audio::human_music::comp::PadVoicingReason;
    let mut shortened = Vec::new();
    for seed in 96_950_000u64..96_950_006 {
        for (beats, world) in [
            (64.0, vapor95_v1::vapor95_v1()),
            (16.0, MusicWorld::black_ice()),
        ] {
            let song = SongMap::compose(
                &demo_trace(beats),
                seed,
                Some(CompositionGrammar::HookArc),
                Composer::StructuralR9,
            );
            let take = perform_with_profile(&song, &world, fusion(), PerformanceProfile::BAND)
                .expect("lawful candidate");
            let last = take.perf.contexts.last().expect("a harmony");
            let released = take
                .score
                .pad_voicing_edits
                .iter()
                .any(|e| matches!(e.reason, PadVoicingReason::Release { .. }));
            let nominal = 0.98 * f64::from(last.dur_beats);
            for n in take
                .score
                .role_notes(Role::Pad)
                .filter(|n| n.start_beat >= last.start_beat - EPS)
            {
                if !released && f64::from(n.dur_beats) < nominal - 0.05 {
                    shortened.push((seed, beats, n.start_beat, n.pitch, n.dur_beats));
                }
            }
        }
    }
    assert!(
        shortened.is_empty(),
        "final pad notes cut short with no release: {shortened:?}"
    );
}
