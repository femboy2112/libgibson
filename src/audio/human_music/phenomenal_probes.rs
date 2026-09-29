//! Round XI: executable falsifiers of the universal DeflectedLift ontology.
use super::composer::chart_space;
use super::contract::CompositionGrammar;
use super::meaning::{Level, MeaningKind, MeaningPlan};
use super::semantic::calm_loop;
use super::song::SongMap;

#[test]
fn low_pressure_should_not_require_high_preparation() {
    let trace = calm_loop(120.0);
    let song = SongMap::build(&trace, 2112, Some(CompositionGrammar::DeflectedLift));
    let target = MeaningPlan::target(&trace, &song.plan);
    assert_eq!(target.arc, Level::Low);
    assert!(
        !target
            .events
            .iter()
            .any(|e| e.kind == MeaningKind::Prepare(Level::High)),
        "low-pressure target still mandates Prepare(High)"
    );
}

#[test]
fn grammar_should_admit_fulfilled_arrival() {
    assert!(
        chart_space().iter().any(|c| c.deflect == c.expected),
        "every available chart denies its expected arrival"
    );
}
