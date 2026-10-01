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
        world: MusicWorld::vapor95,
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
