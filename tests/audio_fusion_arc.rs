//! R0 (OPERATION HUMANMUSIC) — independent Rust reproduction of the false-finale defect.
//!
//! These tests read the REAL `MusicalArgument::fusion` + `compile` objects for the actual
//! `examples/fusion_gates.rs` default construction (`deflected_lift_trace(beats)`, seed 2112,
//! `DeflectedLift`/`MeaningDirected`, black_ice A-Aeolian). They are NOT a transcription of the
//! `research/c137_semantic_density` Python model — they execute the pinned Rust and assert what it
//! actually produces. The observed sequences match that model exactly (receipt: PR #99).
//!
//! The defect, confirmed from live objects:
//!   * **F0** — the fusion argument declares `ArgumentEnding::Resolved`, yet at every tested length
//!     except 160 it ENDS on a bridge `Depart` and (112/120/128/144) sounds its `Return` BEFORE the
//!     final departures. The last thing the "resolved" piece says is a departure, not a return.
//!   * **F1** — `compile` indexes the 8-bar `BRIDGE_PROG` by each phrase's LOCAL bar, so the four
//!     short `Depart` phrases of the 128-beat tail each restart at bar 0: only the first two bridge
//!     chords (`Dom7@0`, `Min@5`) ever sound. The open home-dominant that *wants the return*
//!     (`BRIDGE_PROG[7]` = `Dom7@7`) never plays. The bridge forgets its cumulative phase.
//!
//! These assertions document the CURRENT (defective) behavior so the macroform repair (R1) has a
//! red-to-green pivot: R1 inverts them (a Resolved fusion program must discharge its departure and
//! close on a Return/Coda; the bridge route must advance through all eight chords).
use gibson::audio::human_music::{
    argument::{ArgumentEnding, ArgumentRelation, CompiledArgument, MusicalArgument},
    composer::Composer,
    contract::CompositionGrammar,
    semantic::deflected_lift_trace,
    theory::Quality,
    SongMap,
};

const SEED: u64 = 2112;

fn compiled_fusion(beats: f64) -> (MusicalArgument, CompiledArgument) {
    let base = SongMap::compose(
        &deflected_lift_trace(beats),
        2112,
        Some(CompositionGrammar::DeflectedLift),
        Composer::MeaningDirected,
    );
    let argument = MusicalArgument::fusion(SEED, &base).expect("fusion argument");
    let compiled = argument.compile(&base).expect("compile");
    (argument, compiled)
}

fn label(r: ArgumentRelation) -> &'static str {
    match r {
        ArgumentRelation::Establish => "Establish(verse)",
        ArgumentRelation::Question { .. } => "Question",
        ArgumentRelation::Develop { .. } => "Develop",
        ArgumentRelation::Depart { .. } => "Depart(bridge)",
        ArgumentRelation::Denial { .. } => "Denial",
        ArgumentRelation::Consequent { .. } => "Consequent(hook)",
        ArgumentRelation::Answer { .. } => "Answer",
        ArgumentRelation::Return { .. } => "Return(hook)",
        ArgumentRelation::Reconcile { .. } => "Reconcile",
    }
}

fn relations(c: &CompiledArgument) -> Vec<&'static str> {
    c.placements.iter().map(|p| label(p.relation)).collect()
}

fn print_arc(beats: f64, c: &CompiledArgument) {
    println!(
        "\n=== fusion arc @ {beats} beats: {} placements ===",
        c.placements.len()
    );
    for p in &c.placements {
        println!(
            "  beat {:>5.1}  span {:>4.1}  {}",
            p.start_beat,
            p.span_beats,
            label(p.relation)
        );
    }
}

fn ends_departed(c: &CompiledArgument) -> bool {
    matches!(
        c.placements.last().map(|p| p.relation),
        Some(ArgumentRelation::Depart { .. })
    )
}

fn has_return(c: &CompiledArgument) -> bool {
    c.placements
        .iter()
        .any(|p| matches!(p.relation, ArgumentRelation::Return { .. }))
}

/// A `Depart` whose onset is strictly after the last `Return` — the audible false finale.
fn return_precedes_departure(c: &CompiledArgument) -> bool {
    let last_return = c
        .placements
        .iter()
        .filter(|p| matches!(p.relation, ArgumentRelation::Return { .. }))
        .map(|p| p.start_beat)
        .fold(f64::NEG_INFINITY, f64::max);
    last_return.is_finite()
        && c.placements.iter().any(|p| {
            matches!(p.relation, ArgumentRelation::Depart { .. })
                && p.start_beat > last_return + 1e-9
        })
}

#[test]
fn f0_the_128_default_returns_at_beat_80_then_ends_on_four_bridge_departures() {
    let (argument, c) = compiled_fusion(128.0);
    print_arc(128.0, &c);

    // The program PROMISES resolution...
    assert_eq!(argument.ending, ArgumentEnding::Resolved);

    // ...yet the realized order is verse/hook ×3, an early Return, then four departures.
    assert_eq!(
        relations(&c),
        vec![
            "Establish(verse)",
            "Consequent(hook)",
            "Establish(verse)",
            "Consequent(hook)",
            "Establish(verse)",
            "Return(hook)",   // beat 80
            "Depart(bridge)", // beat 96
            "Depart(bridge)", // beat 104
            "Depart(bridge)", // beat 112
            "Depart(bridge)", // beat 120 — last word is a departure
        ],
    );
    assert!(
        return_precedes_departure(&c),
        "F0: the Return (beat 80) is followed by bridge departures"
    );
    assert!(
        ends_departed(&c),
        "F0: a Resolved program ends on a Depart, not a Return/Coda"
    );
}

#[test]
fn f1_the_bridge_progression_restarts_each_short_phrase_so_only_two_of_eight_chords_sound() {
    let (_argument, c) = compiled_fusion(128.0);
    // Bars 24..32 (beats 96..128) are the four 8-beat Depart phrases.
    let bridge: Vec<(i32, Quality)> = c.route[24..32].to_vec();
    println!("\n128-beat bridge route (bars 24..32): {bridge:?}");

    // Intended: the full 8-bar BRIDGE_PROG, eight distinct steps ending on the home dominant.
    // Actual: each 2-bar phrase restarts at local bar 0, so it is `[Dom7@0, Min@5]` vamped 4×.
    let vamp: Vec<(i32, Quality)> = vec![(0, Quality::Dom7), (5, Quality::Min)];
    let expected: Vec<(i32, Quality)> = vamp.iter().cycle().take(8).copied().collect();
    assert_eq!(
        bridge, expected,
        "F1: the bridge is a 2-chord vamp, not an 8-bar journey"
    );

    let mut distinct = bridge.clone();
    distinct.sort_by_key(|&(d, _)| d);
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        2,
        "F1: only two distinct chords sound across the bridge"
    );

    // The chord that makes the return feel earned — the open home dominant — never plays.
    assert!(
        !bridge.contains(&(7, Quality::Dom7)),
        "F1: BRIDGE_PROG[7] (the open home dominant) is lost to the restart"
    );
}

#[test]
fn positive_control_the_160_fixture_ends_in_a_return() {
    let (_argument, c) = compiled_fusion(160.0);
    print_arc(160.0, &c);
    assert!(has_return(&c));
    assert!(
        !ends_departed(&c),
        "160-beat control: the final placement is a Return, not a Depart"
    );
    assert!(
        !return_precedes_departure(&c),
        "160-beat control: no departure falls after the last Return"
    );
    assert!(matches!(
        c.placements.last().map(|p| p.relation),
        Some(ArgumentRelation::Return { .. })
    ));
}

#[test]
fn the_false_finale_is_systemic_not_a_single_length_artifact() {
    for &beats in &[96.0_f64, 112.0, 120.0, 128.0, 144.0] {
        let (argument, c) = compiled_fusion(beats);
        print_arc(beats, &c);
        assert_eq!(argument.ending, ArgumentEnding::Resolved);
        assert!(
            ends_departed(&c),
            "@{beats}: a Resolved program ends on a Depart"
        );
    }
    // 96 is its own pathology: a Resolved program that never returns at all.
    let (_argument, c96) = compiled_fusion(96.0);
    assert!(
        !has_return(&c96),
        "@96: the Resolved program contains no Return whatsoever"
    );
    // 160 alone, by luck of the partition, closes correctly.
    let (_argument, c160) = compiled_fusion(160.0);
    assert!(!ends_departed(&c160));
}
