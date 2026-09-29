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

use super::backbone::HarmonicGesture;
use super::contract::CompositionGrammar;
use super::functor::{compose_full, perform, Composition};
use super::language::MusicalLanguage;
use super::performance::PerformanceOptions;
use super::score::Role;
use super::semantic::deflected_lift_trace;
use super::song::{SongMap, ThemeSite};
use super::theory::Scale;
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

/// The 0-based scale degree of pitch class `pc` in `region`, or `None` if it is not diatonic.
fn degree_of(pc: i32, region: &Scale) -> Option<i32> {
    (0..7).find(|&d| region.degree_pitch(d, 4).rem_euclid(12) == pc.rem_euclid(12))
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

/// **Witness 2 (defect): the room picks the chart.** The plan carries only the abstract gestures
/// (Lift/Deflect/Open/Reset); `backbone::realize(timeline, world, ..)` searches the chord cell in
/// the world's own mode, so rooms of different modes discover different journeys. Measured as
/// scale degrees in each room (so a mere transposition would compare equal): the Open lands on a
/// different degree in the Aeolian room than in the Ionian rooms.
#[test]
fn witness_the_world_picks_the_chart() {
    let fusion = MusicalLanguage::fusion_conversation();
    let bi = flagship(&MusicWorld::black_ice(), fusion);
    let tl = bi
        .song
        .plan
        .backbone
        .clone()
        .expect("DeflectedLift has a backbone");
    let journey = |world: &MusicWorld| -> [Option<i32>; 4] {
        let region = Scale::new(world.tonic_pc, world.mode);
        let cell = super::backbone::realize(&tl, world, &fusion, SEED).cell;
        [cell.lift, cell.deflect, cell.open, cell.reset].map(|c| degree_of(c.root_pc, &region))
    };
    let [a, b, c] = MusicWorld::all().map(|w| journey(&w));
    eprintln!(
        "degree journey [lift, deflect, open, reset]: BLACK_ICE {a:?}  VAPOR95 {b:?}  SWISS {c:?}"
    );
    assert_ne!(
        a, b,
        "defect: the same plan realizes a different degree journey per room"
    );
    assert_eq!(
        b, c,
        "rooms of one mode agree (the search is transposition-invariant)"
    );
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
    let onsets = |lang: &MusicalLanguage| -> Vec<f64> {
        super::backbone::realize(&tl, &world, lang, SEED)
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
        super::backbone::realize(&tl, &world, lang, SEED)
            .spans
            .iter()
            .filter(|sp| sp.start_beat >= lift.start_beat() && sp.start_beat < lift.end_beat())
            .map(|sp| sp.start_beat)
            .fold(f64::NEG_INFINITY, f64::max)
    };
    assert_ne!(pointer_onset(&simple), pointer_onset(&fusion));
}
