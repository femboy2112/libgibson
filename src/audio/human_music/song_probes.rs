#![cfg(test)]
//! **Round IX song probes** — the claim under test: *the song is one object upstream of every
//! performance*. Same trace, same seed, same grammar (the flagship: [`deflected_lift_trace`]`(120.0)`,
//! forced DeflectedLift, seed 2112); what varies is only the room ([`MusicWorld`]) and the idiom
//! ([`MusicalLanguage`]).
//!
//! The round started from three witnesses, each asserting a defect AS IT STOOD on the R8b tip
//! (0b4483d): the composition plan was identical across worlds, yet the room picked the theme and
//! the chart, and the idiom rewrote the chart's rhythm through a field no song coordinate owned.
//! Each is replaced by the law it witnessed when that law lands (the git history keeps the flip).

use super::backbone::{ChartRoot, HarmonicGesture};
use super::contract::CompositionGrammar;
use super::functor::{compose_full, perform, Composition};
use super::language::MusicalLanguage;
use super::performance::PerformanceOptions;
use super::score::Role;
use super::semantic::deflected_lift_trace;
use super::song::{SongMap, ThemeSite};
use super::world::MusicWorld;

const SEED: u64 = 2112;

fn flagship(world: &MusicWorld, language: MusicalLanguage) -> Composition {
    compose_full(
        &deflected_lift_trace(120.0),
        world,
        SEED,
        Some(CompositionGrammar::DeflectedLift),
        PerformanceOptions {
            language,
            ..PerformanceOptions::default()
        },
    )
}

/// The four acceptance performances of ONE song: three rooms speaking the flagship idiom, and the
/// Aeolian room speaking the plain one.
fn acceptance(song: &SongMap) -> [(&'static str, Composition); 4] {
    let fusion = PerformanceOptions::default();
    let simple = PerformanceOptions {
        language: MusicalLanguage::simple(),
        ..fusion
    };
    [
        (
            "BLACK_ICE/fusion",
            perform(song, &MusicWorld::black_ice(), fusion),
        ),
        (
            "VAPOR95/fusion",
            perform(song, &MusicWorld::vapor95(), fusion),
        ),
        (
            "SWISS_SIGNAL/fusion",
            perform(song, &MusicWorld::swiss_signal(), fusion),
        ),
        (
            "BLACK_ICE/simple",
            perform(song, &MusicWorld::black_ice(), simple),
        ),
    ]
}

fn flagship_song() -> SongMap {
    SongMap::build(
        &deflected_lift_trace(120.0),
        SEED,
        Some(CompositionGrammar::DeflectedLift),
    )
}

/// **Law 1 (was witness 1): the theme is the song's.** Every performance states the SongMap's own
/// bank, and at every identity site (the thesis coming home, the hook) states exactly what the song
/// states there — and the lead is heard stating it. On 0b4483d the Aeolian room stated germ id 1
/// `[0,3,6,4,3,1]` while the Ionian rooms stated id 0 `[0,4,3,5,2]`.
#[test]
fn the_theme_is_the_songs() {
    let song = flagship_song();
    let identity: Vec<&ThemeSite> = song
        .thematic
        .sites
        .iter()
        .filter(|s| s.is_identity())
        .collect();
    assert!(
        identity.len() >= 3,
        "the flagship states its thesis and hook"
    );
    for (label, c) in acceptance(&song) {
        assert_eq!(
            c.perf.bank, song.thematic.bank,
            "{label}: the bank is the song's"
        );
        for site in &identity {
            let st = c
                .perf
                .statements
                .iter()
                .find(|st| st.phrase == site.phrase)
                .unwrap_or_else(|| panic!("{label}: phrase {} states nothing", site.phrase));
            assert_eq!(
                st.motif, site.motif,
                "{label}: phrase {} restated",
                site.phrase
            );
            let heard = c
                .score
                .notes
                .iter()
                .filter(|n| n.role == Role::Lead && n.prov.material == Some(st.material))
                .count();
            assert!(heard > 0, "{label}: phrase {} is silent", site.phrase);
        }
        eprintln!(
            "{label}: germ {:?} at {} identity sites",
            c.perf.bank.identity.degrees,
            identity.len()
        );
    }
}

/// The chart landmarks a performance must sound: every slot's entry anchor at its downbeat and the
/// pointer as the last harmony of every Lift, each as `(beat, what, chart root)`.
fn landmarks(song: &SongMap) -> Vec<(f64, &'static str, ChartRoot)> {
    let tl = song
        .plan
        .backbone
        .as_ref()
        .expect("DeflectedLift has a backbone");
    let cell = song.harmonic.expect("DeflectedLift has a chart").cell;
    let mut v = Vec::new();
    for sl in &tl.slots {
        let root = match sl.gesture {
            HarmonicGesture::Lift => cell.lift,
            HarmonicGesture::Deflect => cell.deflect,
            HarmonicGesture::Open => cell.open,
            HarmonicGesture::Reset => cell.reset,
        };
        v.push((sl.start_beat(), sl.gesture.label(), root));
        if sl.gesture == HarmonicGesture::Lift {
            let end = sl.end_beat().min(song.plan.form.total_beats);
            v.push((end - 1e-3, "pointer", cell.pointer));
        }
    }
    v
}

/// **Law 2 (was witness 2): the chart is the song's.** Every room sounds the SAME relational
/// journey: at every slot's downbeat the chart's anchor, and the pointer closing every Lift — each
/// root read in the region in force there. On 0b4483d the Aeolian room searched its own cell and
/// opened on degree 2 (bIII) where the Ionian rooms opened on degree 3 (IV).
#[test]
fn the_chart_is_the_songs() {
    let song = flagship_song();
    let marks = landmarks(&song);
    let kinds: std::collections::BTreeSet<&str> = marks.iter().map(|m| m.1).collect();
    assert_eq!(kinds.len(), 5, "every landmark kind is charted: {kinds:?}");
    for (label, c) in acceptance(&song) {
        let mut wrong = Vec::new();
        for &(beat, what, root) in &marks {
            let heard = c
                .score
                .chords
                .iter()
                .find(|sp| {
                    sp.start_beat <= beat + 1e-6 && beat < sp.start_beat + sp.dur_beats as f64
                })
                .map(|sp| sp.chord.root_pc);
            let region = c.perf.region_at(beat);
            if heard != Some(root.root_pc(&region)) {
                wrong.push((beat, what, root, heard));
            }
        }
        eprintln!("{label}: {} landmarks, wrong {:?}", marks.len(), wrong);
        assert!(wrong.is_empty(), "{label}: the room played another journey");
    }
}

/// **Witness 3 (defect): the idiom silently rewrites the chart's rhythm.** Nothing in the plan
/// says how often the chord changes; `MusicalLanguage::harmonic_rhythm_bars` (Simple 2, Fusion 1)
/// does, inside `slot_path`. Same plan, same room: the change points differ, and the only
/// authority for them is a language field.
#[test]
fn witness_the_language_rewrites_the_chart_rhythm() {
    let world = MusicWorld::black_ice();
    let simple = MusicalLanguage::simple();
    let fusion = MusicalLanguage::fusion_conversation();
    let s = flagship(&world, simple);
    let f = flagship(&world, fusion);
    assert_eq!(
        s.song.plan.dump(),
        f.song.plan.dump(),
        "the plan is language-free"
    );
    let tl = s
        .song
        .plan
        .backbone
        .clone()
        .expect("DeflectedLift has a backbone");
    let chart = s.song.harmonic.expect("DeflectedLift has a chart").cell;
    let onsets = |lang: &MusicalLanguage| -> Vec<f64> {
        super::backbone::realize(&tl, &chart, &world, lang)
            .spans
            .iter()
            .map(|sp| sp.start_beat)
            .collect()
    };
    let (os, of) = (onsets(&simple), onsets(&fusion));
    eprintln!(
        "chord changes: simple {} / fusion {} (harmonic_rhythm_bars {} / {})",
        os.len(),
        of.len(),
        simple.harmonic_rhythm_bars,
        fusion.harmonic_rhythm_bars
    );
    assert_ne!(simple.harmonic_rhythm_bars, fusion.harmonic_rhythm_bars);
    assert_ne!(
        os, of,
        "defect: the language moves the chart's change points"
    );
    // ...and the Lift of a multi-bar slot reaches the pointer at a different beat.
    let lift = tl
        .slots
        .iter()
        .find(|sl| sl.gesture == HarmonicGesture::Lift && sl.bars >= 4)
        .expect("the flagship has a statement-length Lift");
    let pointer_onset = |lang: &MusicalLanguage| -> f64 {
        super::backbone::realize(&tl, &chart, &world, lang)
            .spans
            .iter()
            .filter(|sp| sp.start_beat >= lift.start_beat() && sp.start_beat < lift.end_beat())
            .map(|sp| sp.start_beat)
            .fold(f64::NEG_INFINITY, f64::max)
    };
    assert_ne!(pointer_onset(&simple), pointer_onset(&fusion));
}
