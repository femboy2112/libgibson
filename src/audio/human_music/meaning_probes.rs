#![cfg(test)]
//! **Round X meaning probes** — the claim under test: *the song has identity but no listener
//! model*. Round IX made one song upstream of every performance; the listen said the band finally
//! plays one song, and that the song is written in a language the listener does not understand.
//!
//! The round starts from three witnesses, each asserting a defect AS IT STANDS on the Round IX tip
//! (d99ebff), on the Round IX composer (`SongMap::build`, kept unchanged as the control):
//! the composer's content is blind to what the story means, the listener hears the thesis
//! developed before it is ever stated, and the harmony raises its first expectation before home
//! has been heard.

use super::contract::CompositionGrammar;
use super::discourse::DiscourseRole;
use super::functor::perform;
use super::language::MusicalLanguage;
use super::performance::PerformanceOptions;
use super::semantic::{calm_loop, deflected_lift_trace, rise_unresolved, EventKind, SemanticTrace};
use super::song::SongMap;
use super::world::MusicWorld;

const SEED: u64 = 2112;

fn r9_song(trace: &SemanticTrace) -> SongMap {
    SongMap::build(trace, SEED, Some(CompositionGrammar::DeflectedLift))
}

/// The story's peak semantic pressure and whether a release (a confirmation or a resolved
/// section) follows that peak — the two things the stock traces most plainly disagree on.
fn peak_and_release(trace: &SemanticTrace) -> (f32, bool) {
    let (i, peak) = trace
        .events
        .iter()
        .enumerate()
        .map(|(i, e)| (i, e.state.pressure()))
        .fold((0, 0.0f32), |a, b| if b.1 > a.1 { b } else { a });
    let released = trace.events[i + 1..]
        .iter()
        .any(|e| matches!(e.kind, EventKind::Confirmation | EventKind::SectionResolved));
    (peak, released)
}

/// WITNESS 1 — the composer is meaning-blind. A calm loop (no salient event, pressure never above
/// Info) and a rise that ends on an unreleased danger impact are materially different stories; at
/// the same seed, grammar and frame the Round IX composer gives them the same germ and the same
/// chart. (The goal is not that every trace must differ in notes — it is that nothing the story
/// means reaches the content at all.)
#[test]
fn witness_the_composer_is_meaning_blind() {
    let (calm, rise) = (calm_loop(120.0), rise_unresolved(120.0));
    let (calm_peak, calm_released) = peak_and_release(&calm);
    let (rise_peak, rise_released) = peak_and_release(&rise);
    assert!(calm_peak < 0.5, "calm peaks at {calm_peak}");
    assert!(rise_peak > 0.85, "rise peaks at {rise_peak}");
    assert!(!rise_released, "rise must never release its peak");
    let _ = calm_released;

    let (a, b) = (r9_song(&calm), r9_song(&rise));
    assert_eq!(
        a.thematic.bank, b.thematic.bank,
        "the germ ignores the story"
    );
    assert_eq!(
        a.harmonic.map(|h| h.cell),
        b.harmonic.map(|h| h.cell),
        "the chart ignores the story"
    );
}

/// WITNESS 2 — development before exposition. The flagship's first lead statement is a
/// transposition of a thesis nobody has heard; the thesis itself is first stated at the last lead
/// site (phrase 7 of 9), and two stock stories never state it at all. The culminating "hook" is a
/// third object: neither the thesis nor the bank's own hook.
#[test]
fn witness_development_before_exposition() {
    let s = r9_song(&deflected_lift_trace(120.0));
    let thesis = &s.thematic.bank.identity;
    let first = &s.thematic.sites[0];
    assert_eq!(first.role, DiscourseRole::Depart);
    assert_eq!(first.motif.degrees, vec![2, 6, 5, 7, 4]);
    assert_eq!(
        first.motif,
        thesis.transpose(2),
        "a development of the unheard thesis"
    );
    let literal: Vec<u32> = s
        .thematic
        .sites
        .iter()
        .filter(|x| &x.motif == thesis)
        .map(|x| x.phrase)
        .collect();
    assert_eq!(literal, vec![7], "the thesis is stated once, last");
    assert_eq!(s.thematic.sites.len(), 6);

    for t in [calm_loop(120.0), rise_unresolved(120.0)] {
        let s = r9_song(&t);
        assert!(
            s.thematic
                .sites
                .iter()
                .all(|x| x.motif != s.thematic.bank.identity),
            "this story never states its thesis"
        );
    }

    let hook = s
        .thematic
        .sites
        .iter()
        .find(|x| x.role == DiscourseRole::Culminate)
        .unwrap();
    assert_eq!(hook.motif.degrees, vec![0, 4, 4, 6, 3]);
    assert_ne!(&hook.motif, thesis);
    assert_ne!(
        hook.motif, s.thematic.bank.hook,
        "the bank's hook is never stated"
    );
}

/// WITNESS 3 — expectation before home. The flagship's chart opens on its lift, so the pointer and
/// the first deflection (beat 4) sound before the tonic ever has (beat 12); the deflect and open
/// satellites collapse onto their anchors (16-beat pedals); and home is the LEAST-heard of the
/// three anchors the song sits on — the ear is asked to feel a miss of a home it was never given.
#[test]
fn witness_expectation_before_home() {
    let s = r9_song(&deflected_lift_trace(120.0));
    let cell = s.harmonic.unwrap().cell;
    assert_ne!(cell.lift, cell.reset, "the song opens away from home");
    assert_eq!(cell.satellites[0], cell.deflect, "deflect pedal");
    assert_eq!(cell.satellites[1], cell.open, "open pedal");

    // As charted, in C (SWISS_SIGNAL, the plain idiom — no rhythm transform, no colour).
    let simple = PerformanceOptions {
        language: MusicalLanguage::simple(),
        ..PerformanceOptions::default()
    };
    let c = perform(&s, &MusicWorld::swiss_signal(), simple);
    let first = |pc: i32| {
        c.score
            .chords
            .iter()
            .find(|sp| sp.chord.root_pc == pc)
            .map(|sp| sp.start_beat)
            .unwrap()
    };
    let (tonic, deflect) = (first(0), first(9));
    assert!(
        deflect < tonic,
        "the first miss (beat {deflect}) precedes home (beat {tonic})"
    );
    let heard = |pc: i32| -> f64 {
        c.score
            .chords
            .iter()
            .filter(|sp| sp.chord.root_pc == pc)
            .map(|sp| sp.dur_beats as f64)
            .sum()
    };
    let (i, vi, iv) = (heard(0), heard(9), heard(5));
    assert!(
        i < vi && i < iv,
        "home is the least-heard anchor: I {i} vi {vi} IV {iv} beats"
    );
}
