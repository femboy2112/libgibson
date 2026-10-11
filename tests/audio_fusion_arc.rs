//! OPERATION HUMANMUSIC — the global-argument arc witness (R0 reproduction → R1 acceptance).
//!
//! These tests read the REAL `MusicalArgument::fusion` + `compile` objects for the actual
//! `examples/fusion_gates.rs` default construction (`deflected_lift_trace(beats)`, seed 2112,
//! `DeflectedLift`/`MeaningDirected`). They are NOT a transcription of the
//! `research/c137_semantic_density` Python model (PR #99) — they execute the Rust.
//!
//! R0 reproduced the false-finale defect from live objects: the `fusion` argument declared
//! `ArgumentEnding::Resolved` yet ended on a bridge `Depart` (128: Return@80 then Depart@96/104/112/
//! 120), and the 8-bar `BRIDGE_PROG` restarted inside each short tail phrase so only two of its
//! eight chords sounded. R1 repaired it with a narrative ARC over the form (`MusicalArgument::arc`):
//! a `Resolved` program now closes on its Return, the bridge sits in the middle, and its harmonic
//! route advances continuously through all eight chords — ending on the open home-dominant that
//! pulls into the return. These assertions are the corrected (green) acceptance witness.
use gibson::audio::human_music::{
    argument::{ArgumentEnding, ArgumentRelation, CompiledArgument, MusicalArgument},
    composer::Composer,
    contract::CompositionGrammar,
    semantic::deflected_lift_trace,
    theory::Quality,
    SongMap,
};

const SEED: u64 = 2112;

/// The intended 8-bar bridge departure (mirrors `argument.rs::BRIDGE_PROG`): the tonic turns into
/// its own dominant, travels the relative-minor region, and ends OPEN on the home dominant (V7)
/// that wants the return.
const BRIDGE_PROG: [(i32, Quality); 8] = [
    (0, Quality::Dom7),
    (5, Quality::Min),
    (3, Quality::Maj),
    (8, Quality::Maj),
    (5, Quality::Min),
    (0, Quality::Dom7),
    (5, Quality::Min),
    (7, Quality::Dom7),
];

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

fn ends_on_return(c: &CompiledArgument) -> bool {
    matches!(
        c.placements.last().map(|p| p.relation),
        Some(ArgumentRelation::Return { .. })
    )
}

fn has_depart(c: &CompiledArgument) -> bool {
    c.placements
        .iter()
        .any(|p| matches!(p.relation, ArgumentRelation::Depart { .. }))
}

/// A `Depart` whose onset is strictly after the last `Return` — the audible false finale R0 found.
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

/// The route bars the (first) `Depart` section spans, in order.
fn bridge_route(c: &CompiledArgument) -> Vec<(i32, Quality)> {
    let dep = c
        .placements
        .iter()
        .find(|p| matches!(p.relation, ArgumentRelation::Depart { .. }))
        .expect("a bridge section");
    let first_bar = (dep.start_beat / 4.0).round() as usize;
    let bars = (dep.span_beats / 4.0).round() as usize;
    c.route[first_bar..first_bar + bars].to_vec()
}

#[test]
fn f0_the_128_default_is_a_closed_verse_hook_depart_return_arc() {
    let (argument, c) = compiled_fusion(128.0);
    print_arc(128.0, &c);

    // The program promises resolution...
    assert_eq!(argument.ending, ArgumentEnding::Resolved);
    assert!(
        argument.arc.is_some(),
        "the arc path is taken for the default"
    );

    // ...and now the realized order is a closed verse/hook, a middle departure, then a return.
    assert_eq!(
        relations(&c),
        vec![
            "Establish(verse)",
            "Consequent(hook)",
            "Establish(verse)",
            "Consequent(hook)",
            "Depart(bridge)", // beats 64..96 — the departure is in the MIDDLE
            "Return(hook)",   // beats 96..128 — the last word is the return
        ],
    );
    assert!(
        ends_on_return(&c),
        "a Resolved program closes on its Return, not a Depart"
    );
    assert!(
        !return_precedes_departure(&c),
        "no departure falls after the return"
    );
}

#[test]
fn f1_the_bridge_travels_its_full_progression_and_ends_on_the_home_dominant() {
    let (_argument, c) = compiled_fusion(128.0);
    let bridge = bridge_route(&c);
    println!("\n128-beat bridge route: {bridge:?}");

    // The 128 bridge spans 32 beats = 8 bars, so it plays the whole BRIDGE_PROG exactly once —
    // no restart. The route advances continuously (section-local bar indexing).
    assert_eq!(
        bridge.len(),
        8,
        "the 128 bridge is eight bars (two coalesced 16-beat phrases)"
    );
    for (k, chord) in bridge.iter().enumerate() {
        assert_eq!(
            *chord,
            BRIDGE_PROG[k % BRIDGE_PROG.len()],
            "bridge bar {k} must follow the continuous progression"
        );
    }
    // The chord that makes the return feel earned — the open home dominant — now sounds, last.
    assert_eq!(
        *bridge.last().unwrap(),
        (7, Quality::Dom7),
        "the bridge ends open on the home dominant that pulls into the return"
    );
}

#[test]
fn positive_control_the_160_fixture_still_ends_in_a_return() {
    let (_argument, c) = compiled_fusion(160.0);
    print_arc(160.0, &c);
    assert!(ends_on_return(&c));
    assert!(!return_precedes_departure(&c));
}

#[test]
fn every_tested_length_closes_on_a_return_after_a_middle_departure() {
    for &beats in &[96.0_f64, 112.0, 120.0, 128.0, 144.0, 160.0] {
        let (argument, c) = compiled_fusion(beats);
        print_arc(beats, &c);
        assert_eq!(argument.ending, ArgumentEnding::Resolved);
        assert!(
            ends_on_return(&c),
            "@{beats}: a Resolved program ends on its Return"
        );
        assert!(
            has_depart(&c),
            "@{beats}: the arc departs (a bridge) before it returns"
        );
        assert!(
            !return_precedes_departure(&c),
            "@{beats}: the return is the final gesture"
        );
        // Whatever its length, the bridge advances continuously through BRIDGE_PROG.
        let bridge = bridge_route(&c);
        for (k, chord) in bridge.iter().enumerate() {
            assert_eq!(*chord, BRIDGE_PROG[k % BRIDGE_PROG.len()]);
        }
    }
}
