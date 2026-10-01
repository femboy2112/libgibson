//! Final pre-main round: a landed material note is judged where it actually sounds.
//!
//! A fresh-seed sweep (seed `90_000_026`, outside every declared sweep) reproduced a second BAND
//! red: a keys answer projected from material chose its pitch and function against the harmony at
//! its authored onset, then `land_once` moved the onset onto a planned accent one step later — into
//! the next structural chord (Fmaj7 → F6). The pitch stayed legal there, but the *declared*
//! function did not: it claimed `ChordTone` of a chord that no longer contains it. The receipt's
//! temporal law names this a false function claim, so the BAND take is rejected.
//!
//! The bass already carries this law (`bass::reland`: "a projected onset that `land_once` moved
//! onto an accent may now sit in another harmony than the one its pitch was chosen for. Judge it
//! where it actually sounds."). This file falsifies the same law for the keys' material lines:
//! after any landing, a note's declared function must hold in the harmony it actually sounds in,
//! not in the harmony its pitch was chosen for.
//!
//! The restated invariant (independent of the library's full temporal reconstruction): every note
//! declared `ChordTone` must sound a member of the chord sounding at its actual onset, and every
//! note declared `LicensedExtension` must sound a palette tension there.
use gibson::audio::human_music::{
    composer::Composer,
    contract::CompositionGrammar,
    functor::{perform_checked, perform_with_profile},
    language::MusicalLanguage,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    score::PitchFunction,
    semantic::deflected_lift_trace,
    theory::pitch_class,
    MusicWorld, SongMap,
};

/// The exact fresh case that failed at the final Claude source.
fn falsifier() -> (SongMap, MusicWorld, PerformanceOptions) {
    let song = SongMap::compose(
        &deflected_lift_trace(33.25),
        90_000_026,
        Some(CompositionGrammar::PropulsiveReturn),
        Composer::StablePropulsion,
    );
    let world = MusicWorld::vapor95();
    let opts = PerformanceOptions {
        language: MusicalLanguage::fusion_conversation(),
        ..PerformanceOptions::default()
    };
    (song, world, opts)
}

#[test]
fn band_admits_the_fresh_landed_function_falsifier() {
    let (song, world, opts) = falsifier();
    match perform_checked(&song, &world, opts, PerformanceProfile::BAND) {
        Ok(_) => {}
        Err(e) => panic!("the fresh BAND falsifier is not admitted: {e}"),
    }
}

#[test]
fn a_landed_material_note_declares_a_function_it_sounds() {
    let (song, world, opts) = falsifier();
    let c = perform_with_profile(&song, &world, opts, PerformanceProfile::BAND)
        .expect("the falsifier is a lawful performance");
    let mut bad = Vec::new();
    for n in &c.score.notes {
        if !matches!(n.prov.role_note, "answer" | "figure" | "quote") {
            continue;
        }
        let Some(ctx) = c.perf.context_at(n.start_beat) else {
            continue;
        };
        let pc = pitch_class(n.pitch);
        let ok = match n.function {
            Some(PitchFunction::ChordTone) => ctx.chord.contains_pc(pc),
            Some(PitchFunction::LicensedExtension) => ctx.palette.tensions.contains(&pc),
            None => false,
            _ => true,
        };
        if !ok {
            bad.push(format!(
                "{:?} {} at {} declares {:?} but sounds {:?} over {}",
                n.role,
                pc,
                n.start_beat,
                n.function,
                ctx.palette,
                ctx.chord.label()
            ));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// A second fresh case (seed `90_500_000`): the *bass*'s material answer landed into another
/// harmony at a partial final bar and was left unclassified, so the receipt rejected the BAND take
/// ("1 unclassified notes"). The bass's `reland` law ran only in the coupled path; the independent
/// BAND path must judge a landed material note where it sounds too.
fn bass_falsifier() -> (SongMap, MusicWorld, PerformanceOptions) {
    let song = SongMap::compose(
        &deflected_lift_trace(13.25),
        90_500_000,
        Some(CompositionGrammar::HookArc),
        Composer::StructuralR9,
    );
    let world = MusicWorld::vapor95();
    let opts = PerformanceOptions {
        language: MusicalLanguage::fusion_conversation(),
        ..PerformanceOptions::default()
    };
    (song, world, opts)
}

#[test]
fn band_admits_the_fresh_bass_landed_function_falsifier() {
    let (song, world, opts) = bass_falsifier();
    match perform_checked(&song, &world, opts, PerformanceProfile::BAND) {
        Ok(_) => {}
        Err(e) => panic!("the fresh BAND bass falsifier is not admitted: {e}"),
    }
}

#[test]
fn no_material_line_note_is_left_unclassified() {
    let (song, world, opts) = bass_falsifier();
    let c = perform_with_profile(&song, &world, opts, PerformanceProfile::BAND)
        .expect("the falsifier is a lawful performance");
    let unclassified: Vec<_> = c
        .score
        .notes
        .iter()
        .filter(|n| {
            matches!(n.prov.role_note, "answer" | "figure" | "quote") && n.function.is_none()
        })
        .map(|n| format!("{:?} {} at {}", n.role, n.pitch, n.start_beat))
        .collect();
    assert!(unclassified.is_empty(), "{unclassified:?}");
}
