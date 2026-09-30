//! Pre-main semantic consolidation: each falsifier here was committed red before its repair.
//!
//! Holdout v2 is known evidence; nothing here replays its rows. The seeds (78_301_0xx) are
//! fresh. Where a law is checked, the test restates what the world or language DECLARES
//! (world.rs / language.rs documentation) instead of calling the library's own predicate, so a
//! wrong law cannot certify itself.
use gibson::audio::human_music::{
    composer::Composer,
    contract::CompositionGrammar,
    cover::{cover_candidate, CoverError, CoverMap, CoverSpec, CoverTarget},
    functor::{perform_with_profile, Composition},
    language::MusicalLanguage,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    semantic::{deflected_lift_trace, demo_trace},
    song::AnchorReport,
    theory::{Chord, Quality, Scale},
    MusicWorld, SongMap,
};

const GRAMMARS: [CompositionGrammar; 5] = [
    CompositionGrammar::HookArc,
    CompositionGrammar::LoopEvolution,
    CompositionGrammar::RiffDrive,
    CompositionGrammar::DeflectedLift,
    CompositionGrammar::PropulsiveReturn,
];

fn worlds() -> [MusicWorld; 3] {
    [
        MusicWorld::black_ice(),
        MusicWorld::swiss_signal(),
        MusicWorld::vapor95(),
    ]
}

fn languages() -> [(MusicalLanguage, &'static str); 2] {
    [
        (MusicalLanguage::simple(), "simple"),
        (MusicalLanguage::fusion_conversation(), "fusion"),
    ]
}

fn options(language: MusicalLanguage) -> PerformanceOptions {
    PerformanceOptions {
        language,
        ..PerformanceOptions::default()
    }
}

/// The harmonic vocabulary `world` and `language` declare, restated from their documentation:
/// a four-note (or larger) quality only where the world uses sevenths; a sixth or ninth colour
/// only where the language's colour depth is non-zero ("0 = triads/7ths only"); a tone outside
/// the world's home scale only where the world allows modal mixture.
fn declared(world: &MusicWorld, language: &MusicalLanguage, chord: Chord) -> bool {
    let colour = matches!(
        chord.quality,
        Quality::Maj6
            | Quality::Min6
            | Quality::Add9
            | Quality::Maj9
            | Quality::Min9
            | Quality::Dom9
    );
    let home = Scale::new(world.tonic_pc, world.mode);
    (world.use_sevenths || chord.quality.intervals().len() <= 3)
        && (language.color_depth > 0 || !colour)
        && (world.allow_modal_mixture
            || chord
                .quality
                .intervals()
                .iter()
                .all(|i| home.contains_pc(chord.root_pc + i)))
}

/// The refusal a room gives a song whose chart names a chord the room cannot admit.
const VOCABULARY_REFUSAL: &str = "outside the world's harmonic vocabulary";

/// H05 (holdout v2, reproduced on fresh seeds): a SWISS_SIGNAL PropulsiveReturn source sounded
/// `Maj6`/`Maj7` Reset chords although SWISS declares no sevenths. The sweep showed the leak is
/// general: the backbone's colours and the harmonic edits ignore the world's sevenths and the
/// language's colour depth in every world. Under the hardened general profile every chord a
/// performance sounds is admitted by the vocabulary its world and language declare, or the
/// performance is refused before anybody plays.
#[test]
fn h05_band_sources_sound_only_their_declared_vocabulary() {
    let mut leaks = Vec::new();
    let mut refusals = 0;
    for world in worlds() {
        for (language, lang) in languages() {
            for grammar in GRAMMARS {
                for (i, beats) in [17.25, 40.0].into_iter().enumerate() {
                    let seed = 78_301_010 + i as u64;
                    let composer = if grammar == CompositionGrammar::PropulsiveReturn {
                        Composer::StablePropulsion
                    } else {
                        Composer::StructuralR9
                    };
                    let song = SongMap::compose(&demo_trace(beats), seed, Some(grammar), composer);
                    let tag = format!("{} {lang} {grammar:?} {beats}", world.name);
                    match perform_with_profile(
                        &song,
                        &world,
                        options(language),
                        PerformanceProfile::BAND,
                    ) {
                        Ok(c) => {
                            for s in &c.perf.chords {
                                if !declared(&world, &language, s.chord) {
                                    leaks.push(format!(
                                        "{tag}: {:?} on {} at {} ({})",
                                        s.chord.quality, s.chord.root_pc, s.start_beat, s.note
                                    ));
                                }
                            }
                            assert_eq!(c.score.chords.len(), c.perf.chords.len(), "{tag}");
                        }
                        Err(e) => {
                            assert!(e.0.contains(VOCABULARY_REFUSAL), "{tag}: {e}");
                            refusals += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(
        leaks.is_empty(),
        "{} undeclared chords:\n{}",
        leaks.len(),
        leaks.join("\n")
    );
    // A StructuralR9/StablePropulsion chart is diatonic: none of these may be refused.
    assert_eq!(refusals, 0, "a diatonic chart was refused");
}

/// The H05 consequence, end to end: a SWISS source covered into SWISS keeps its own harmony. The
/// target lawfully refuses a pinned chord outside its vocabulary; the source must not have sounded one.
#[test]
fn h05_a_swiss_source_covers_into_swiss_without_a_vocabulary_refusal() {
    let world = MusicWorld::swiss_signal();
    for (language, lang) in languages() {
        for (seed, beats) in [(78_301_020, 17.25), (78_301_021, 33.0)] {
            let song = SongMap::compose(
                &demo_trace(beats),
                seed,
                Some(CompositionGrammar::PropulsiveReturn),
                Composer::StablePropulsion,
            );
            let source =
                perform_with_profile(&song, &world, options(language), PerformanceProfile::BAND)
                    .expect("a diatonic chart performs in SWISS");
            let spec = CoverSpec::established(&AnchorReport::check(
                &source.song,
                &source.perf,
                &source.score,
            ));
            let map = CoverMap::extract(&source, &world, spec).expect("extract");
            let lifted = cover_candidate(
                &map,
                CoverTarget {
                    world: &world,
                    seed: seed + 100,
                    grammar: CompositionGrammar::PropulsiveReturn,
                    options: options(language),
                    profile: PerformanceProfile::BAND,
                },
            );
            if let Err(CoverError::Invalid(why)) = &lifted {
                assert!(
                    !why.contains("outside target vocabulary"),
                    "{lang} {beats}: the SWISS source sounded a chord SWISS refuses: {why}"
                );
            }
        }
    }
}

/// A chart root the room cannot admit is not recoloured into something else (a room never picks
/// a root): the performance is refused, typed, before anybody plays. The same song performs in a
/// room that admits it. The chart is written by hand (a deflection to bVI, `Chromatic{8, Maj}`),
/// because the current composers rarely sound a borrowed chart root: the law must hold anyway.
#[test]
fn h05_a_chart_the_room_cannot_admit_is_refused_not_leaked() {
    use gibson::audio::human_music::backbone::ChartRoot;
    let mut song = SongMap::compose(
        &deflected_lift_trace(48.0),
        78_301_001,
        Some(CompositionGrammar::DeflectedLift),
        Composer::StructuralR9,
    );
    let bvi = ChartRoot::Chromatic {
        semitones: 8,
        quality: Quality::Maj,
    };
    let chart = song
        .harmonic
        .as_mut()
        .expect("DeflectedLift charts a backbone");
    chart.cell.deflect = bvi;
    chart.cell.satellites[0] = bvi;
    let language = MusicalLanguage::fusion_conversation();
    let swiss = MusicWorld::swiss_signal();
    // The archived arm shows the chart really does sound the borrowed chord in this room.
    let archived =
        perform_with_profile(&song, &swiss, options(language), PerformanceProfile::POCKET).unwrap();
    assert!(
        archived
            .perf
            .chords
            .iter()
            .any(|s| !Scale::new(0, swiss.mode).contains_pc(s.chord.root_pc)),
        "fixture: the chart must name a borrowed root that sounds within 48 beats"
    );
    match perform_with_profile(&song, &swiss, options(language), PerformanceProfile::BAND) {
        Err(e) => assert!(e.0.contains(VOCABULARY_REFUSAL), "{e}"),
        Ok(c) => panic!(
            "SWISS performed a chart it cannot admit: {:?}",
            c.perf
                .chords
                .iter()
                .filter(|s| !declared(&swiss, &language, s.chord))
                .map(|s| (s.start_beat, s.chord))
                .collect::<Vec<_>>()
        ),
    }
    let vapor = MusicWorld::vapor95();
    let c = perform_with_profile(&song, &vapor, options(language), PerformanceProfile::BAND)
        .expect("a room with modal mixture admits bVI");
    let leaks: Vec<_> = c
        .perf
        .chords
        .iter()
        .filter(|s| !declared(&vapor, &language, s.chord))
        .map(|s| (s.start_beat, s.chord))
        .collect();
    assert!(leaks.is_empty(), "{leaks:?}");
}

/// The historical arm is characterized, not repaired: POCKET keeps its archived colours
/// (byte-exact R17), so the boundary between the two laws is visible.
#[test]
fn h05_the_historical_pocket_arm_keeps_its_archived_colours() {
    let song = SongMap::compose(
        &demo_trace(17.25),
        78_301_020,
        Some(CompositionGrammar::PropulsiveReturn),
        Composer::StablePropulsion,
    );
    let swiss = MusicWorld::swiss_signal();
    let language = MusicalLanguage::fusion_conversation();
    let c: Composition =
        perform_with_profile(&song, &swiss, options(language), PerformanceProfile::POCKET).unwrap();
    assert!(
        c.perf.chords.iter().any(|s| !declared(&swiss, &language, s.chord)),
        "the archived SWISS Reset colours are characterized here; if this changes, the R17 arm moved"
    );
}
