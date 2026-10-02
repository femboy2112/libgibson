//! Closeout round: a declared bass figure keeps the room to be stated.
//!
//! A fresh exploratory BAND search (seeds `9681xxxx`/`9683xxxx`, outside every declared sweep and
//! holdout) exposed a third internal-red family the brief did not name: very short forms (one
//! bar, 2.5-5.5 beats) under the fusion language whose contract declares `BassFigure`, rejected
//! with `BassFigure: the bass states no figure of its own` (282 of 38,880 short-form performances).
//!
//! The mechanism (Observed): a bass-initiated discourse verb gets a borrowed figure — a fragment of
//! the lead's motif — and in a one-bar form that call starts on the bar's only downbeat. The bass's
//! own figure has exactly one guaranteed onset per bar (the downbeat, in Foundation/Counter/Quote
//! modes), and the bass never plays its line inside its own quote, so its declared identity never
//! sounds. Under `Planned` admission a lone root sounded only as part of an ensemble Push that no
//! one else performed; the rehearsal rightly strikes that verb, and the bass's last identity note
//! goes with it.
//!
//! The law: where the song declares the bass figure as identity, the bass's borrowed material never
//! takes the last bar downbeat its own figure could sound on. The oracle below restates the
//! identity projection's bass law independently: a bass note carries the figure iff it was voiced
//! as part of the bass's own line (root / fifth / pedal / walk / counter / octave), never a quote or
//! an answer.
//!
//! The VAPOR95 falsifier was derived under the archival VAPOR95 v1 world and is performed under it.
#[path = "common/vapor95_v1.rs"]
mod vapor95_v1;

use gibson::audio::human_music::{
    composer::Composer,
    contract::{CoherenceAnchor, CompositionGrammar},
    functor::{perform_checked, perform_with_profile},
    language::MusicalLanguage,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    score::Role,
    semantic::deflected_lift_trace,
    MusicWorld, SongMap,
};

/// The bass's own line, as the bass realizer voices it (independent of `projection`).
const OWN_LINE: [&str; 6] = ["root", "fifth", "pedal", "walk", "counter", "octave"];

struct Case {
    beats: f64,
    seed: u64,
    grammar: CompositionGrammar,
    composer: Composer,
    world: fn() -> MusicWorld,
}

/// Fresh falsifiers: one per grammar/length/world shape the search found.
const FALSIFIERS: [Case; 3] = [
    Case {
        beats: 2.5,
        seed: 96_810_002,
        grammar: CompositionGrammar::RiffDrive,
        composer: Composer::StructuralR9,
        world: MusicWorld::black_ice,
    },
    Case {
        beats: 4.0,
        seed: 96_810_002,
        grammar: CompositionGrammar::WorldSwitch,
        composer: Composer::StructuralR9,
        world: vapor95_v1::vapor95_v1,
    },
    Case {
        beats: 2.5,
        seed: 96_810_007,
        grammar: CompositionGrammar::WorldSwitch,
        composer: Composer::MeaningDirected,
        world: MusicWorld::swiss_signal,
    },
];

fn fusion() -> PerformanceOptions {
    PerformanceOptions {
        language: MusicalLanguage::fusion_conversation(),
        ..PerformanceOptions::default()
    }
}

fn song(c: &Case) -> SongMap {
    SongMap::compose(
        &deflected_lift_trace(c.beats),
        c.seed,
        Some(c.grammar),
        c.composer,
    )
}

#[test]
fn a_declared_bass_figure_is_stated_by_the_bass_itself() {
    for c in &FALSIFIERS {
        let song = song(c);
        assert!(
            song.plan
                .contract
                .anchors
                .contains(&CoherenceAnchor::BassFigure),
            "seed {}: the falsifier must declare the bass figure",
            c.seed
        );
        let take = perform_with_profile(&song, &(c.world)(), fusion(), PerformanceProfile::BAND)
            .expect("lawful candidate");
        let own: Vec<_> = take
            .score
            .role_notes(Role::Bass)
            .filter(|n| OWN_LINE.contains(&n.prov.role_note))
            .map(|n| (n.start_beat, n.prov.role_note))
            .collect();
        assert!(
            !own.is_empty(),
            "seed {} {:?} {} beats: the song declares the bass figure but the bass only sounds \
             borrowed material: {:?}",
            c.seed,
            c.grammar,
            c.beats,
            take.score
                .role_notes(Role::Bass)
                .map(|n| (n.start_beat, n.prov.role_note))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn band_admits_the_bass_figure_room_falsifiers() {
    for c in &FALSIFIERS {
        if let Err(e) = perform_checked(&song(c), &(c.world)(), fusion(), PerformanceProfile::BAND)
        {
            panic!("seed {} {:?} {} beats: {e}", c.seed, c.grammar, c.beats);
        }
    }
}

fn bass_figure_windows(song: &SongMap, world: MusicWorld) -> Vec<(f64, f64)> {
    use gibson::audio::human_music::action::Agent;
    let take = perform_with_profile(song, &world, fusion(), PerformanceProfile::BAND)
        .expect("lawful candidate");
    take.perf
        .figures_for(Agent::Bass)
        .map(|m| (m.start_beat, m.start_beat + m.length()))
        .collect()
}

/// Minimal intervention: only the call that would take the last downbeat yields; the bass's
/// later borrowed call in the same one-bar form still sounds.
#[test]
fn only_the_call_on_the_last_downbeat_yields() {
    let windows = bass_figure_windows(&song(&FALSIFIERS[0]), MusicWorld::black_ice());
    assert!(
        windows.iter().all(|&(s, _)| s > 1e-6) && !windows.is_empty(),
        "the bass keeps its later borrowed call and gives up only the downbeat: {windows:?}"
    );
}

/// Control: a song that does not declare the bass figure keeps its borrowed call on the downbeat.
#[test]
fn borrowed_bass_material_stands_where_no_bass_figure_is_declared() {
    let control = SongMap::compose(
        &deflected_lift_trace(2.5),
        96_840_000,
        Some(CompositionGrammar::HookArc),
        Composer::StructuralR9,
    );
    assert!(!control
        .plan
        .contract
        .anchors
        .contains(&CoherenceAnchor::BassFigure));
    let windows = bass_figure_windows(&control, MusicWorld::black_ice());
    assert!(
        windows.iter().any(|&(s, _)| s.abs() < 1e-6),
        "no declared bass figure: the borrowed call keeps the downbeat: {windows:?}"
    );
}

/// Control: a declaring song with another downbeat left for its own line keeps the call that
/// covers the first downbeat, and its own figure sounds on the free one.
#[test]
fn a_free_downbeat_leaves_the_borrowed_call_in_place() {
    let control = SongMap::compose(
        &deflected_lift_trace(7.25),
        96_840_000,
        Some(CompositionGrammar::RiffDrive),
        Composer::StructuralR9,
    );
    assert!(control
        .plan
        .contract
        .anchors
        .contains(&CoherenceAnchor::BassFigure));
    let windows = bass_figure_windows(&control, MusicWorld::black_ice());
    assert!(
        windows.iter().any(|&(s, _)| s.abs() < 1e-6),
        "a free downbeat remains (4.0): the call on 0.0 stays: {windows:?}"
    );
    let take = perform_with_profile(
        &control,
        &MusicWorld::black_ice(),
        fusion(),
        PerformanceProfile::BAND,
    )
    .expect("lawful candidate");
    assert!(take
        .score
        .role_notes(Role::Bass)
        .any(|n| OWN_LINE.contains(&n.prov.role_note)));
}
