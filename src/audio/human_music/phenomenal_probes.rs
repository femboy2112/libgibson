//! Round XI: ontology falsifiers, actual-content mutations and fixed-band controls.
use super::backbone::{ChartRoot, HarmonicGesture};
use super::composer::{chart_space, Composer};
use super::contract::CompositionGrammar;
use super::functor::perform;
use super::meaning::{Level, MeaningKind, MeaningPlan};
use super::performance::PerformanceOptions;
use super::phenomenal::{PhenomenalRegime as Regime, PhenomenalTarget, PhenomenalTrajectory};
use super::semantic::{calm_loop, deflected_lift_trace};
use super::song::{SongMap, SongMapConformance};
use super::world::MusicWorld;

fn stable(beats: f64, seed: u64) -> SongMap {
    SongMap::compose(
        &deflected_lift_trace(beats),
        seed,
        None,
        Composer::StablePropulsion,
    )
}

#[test]
fn low_pressure_should_not_require_high_preparation() {
    let trace = calm_loop(120.0);
    let song = SongMap::build(&trace, 2112, Some(CompositionGrammar::DeflectedLift));
    let legacy = MeaningPlan::target(&trace, &song.plan);
    assert_eq!(legacy.arc, Level::Low);
    // The frozen R10 control still exhibits the original failing witness.
    assert!(legacy
        .events
        .iter()
        .any(|e| e.kind == MeaningKind::Prepare(Level::High)));
    let target = PhenomenalTarget::from_trace(&trace, Regime::StablePropulsion);
    assert_eq!(target.preparation(), Level::Low);
    assert_eq!(target.grammar(), CompositionGrammar::PropulsiveReturn);
    let actual = PhenomenalTrajectory::observe(&stable(120.0, 2112));
    assert!(actual
        .slots
        .iter()
        .all(|s| s.state.expectation == Level::Low));
    assert!(
        actual.divergences(&target).is_empty(),
        "{}",
        actual.report()
    );
}

#[test]
fn grammar_should_admit_fulfilled_arrival() {
    assert!(chart_space().iter().all(|c| c.deflect != c.expected));
    let song = stable(120.0, 2112);
    assert!(song
        .plan
        .backbone
        .as_ref()
        .unwrap()
        .slots
        .iter()
        .all(|s| !matches!(s.gesture, HarmonicGesture::Lift | HarmonicGesture::Deflect)));
    let actual = PhenomenalTrajectory::observe(&song);
    assert!(actual.confirms >= 2, "{}", actual.report());
    assert_eq!(actual.withheld, 0);
    assert_eq!(actual.regime(), Some(Regime::StablePropulsion));
    assert!(actual
        .divergences(song.phenomenal.as_ref().unwrap())
        .is_empty());
}

#[test]
fn propulsion_can_increase_without_harmonic_debt() {
    let mut moving = stable(120.0, 2112);
    // Hold exactly the same tonic at every chord site, eliminating harmonic motion entirely.
    moving.harmonic.as_mut().unwrap().cell.open = ChartRoot::Degree(0);
    moving.harmonic.as_mut().unwrap().cell.satellites = [ChartRoot::Degree(0); 3];
    let mut held = moving.clone();
    held.thematic.bank.identity.degrees = vec![0, 7];
    held.thematic.bank.identity.rhythm = vec![4.0, 4.0];
    for site in &mut held.thematic.sites {
        site.motif = held.thematic.bank.identity.clone();
    }
    let a = PhenomenalTrajectory::observe(&held);
    let b = PhenomenalTrajectory::observe(&moving);
    assert_eq!(a.summary.stability, Level::High);
    assert_eq!(a.summary.stability, b.summary.stability);
    assert_eq!((a.unresolved_debt, b.unresolved_debt), (0, 0));
    assert_eq!(
        (a.summary.expectation, b.summary.expectation),
        (Level::Low, Level::Low)
    );
    assert!(a.summary.propulsion < b.summary.propulsion);
    assert_eq!(b.summary.propulsion, Level::High);
}

#[test]
fn suspended_deflection_is_a_distinct_measured_control() {
    let mut song = stable(120.0, 2112);
    let cell = &mut song.harmonic.as_mut().unwrap().cell;
    cell.lift = ChartRoot::Degree(4);
    cell.lift_alt = ChartRoot::Degree(4);
    cell.deflect = ChartRoot::Degree(5);
    cell.satellites[0] = ChartRoot::Degree(5);
    for (i, slot) in song
        .plan
        .backbone
        .as_mut()
        .unwrap()
        .slots
        .iter_mut()
        .enumerate()
    {
        slot.gesture = if i % 2 == 0 {
            HarmonicGesture::Lift
        } else {
            HarmonicGesture::Deflect
        };
    }
    let observed = PhenomenalTrajectory::observe(&song);
    assert_eq!(
        observed.regime(),
        Some(Regime::SuspendedDeflection),
        "{}",
        observed.report()
    );
    let target = PhenomenalTarget::from_trace(&song.trace, Regime::SuspendedDeflection);
    assert!(
        observed.divergences(&target).is_empty(),
        "{:?}",
        observed.divergences(&target)
    );
    assert!(!observed
        .divergences(song.phenomenal.as_ref().unwrap())
        .is_empty());
}

#[test]
fn actual_content_can_falsify_the_target_and_labels_cannot_rescue_it() {
    let song = stable(120.0, 2112);
    let base = PhenomenalTrajectory::observe(&song);
    let target = song.phenomenal.clone().unwrap();
    let mut relabeled = song.clone();
    relabeled.phenomenal = None;
    relabeled.plan.contract.grammar = CompositionGrammar::DeflectedLift;
    assert_eq!(base, PhenomenalTrajectory::observe(&relabeled));
    let mut no_home = song.clone();
    no_home.harmonic.as_mut().unwrap().cell.reset = ChartRoot::Degree(5);
    no_home.harmonic.as_mut().unwrap().cell.satellites[2] = ChartRoot::Degree(5);
    let broken = PhenomenalTrajectory::observe(&no_home);
    assert_eq!(broken.confirms, 0);
    assert!(broken.divergences(&target).contains(&"stability"));
    // Learn IV→I once, then substitute vi at its next promised landing. The observer must
    // detect denial from actual content even though the slot still has a stable target.
    let mut denied = song.clone();
    denied.harmonic.as_mut().unwrap().cell.deflect = ChartRoot::Degree(5);
    denied.harmonic.as_mut().unwrap().cell.satellites[0] = ChartRoot::Degree(5);
    denied.plan.backbone.as_mut().unwrap().slots[7].gesture = HarmonicGesture::Deflect;
    let denied = PhenomenalTrajectory::observe(&denied);
    assert!(denied.withheld > 0);
    assert!(denied.divergences(&target).contains(&"surprise"));
    let mut no_recurrence = song.clone();
    for (i, site) in no_recurrence.thematic.sites.iter_mut().enumerate() {
        if i > 0 {
            site.motif = site.motif.transpose(i as i32);
        }
    }
    assert!(PhenomenalTrajectory::observe(&no_recurrence)
        .divergences(&target)
        .contains(&"familiarity"));
    let mut slow = song.clone();
    for site in &mut slow.thematic.sites {
        site.motif = site.motif.scale_rhythm(4.0);
    }
    assert!(PhenomenalTrajectory::observe(&slow)
        .divergences(&target)
        .contains(&"propulsion"));
}

#[test]
fn same_band_preserves_both_song_maps() {
    let trace = deflected_lift_trace(120.0);
    let a = SongMap::compose(
        &trace,
        2112,
        Some(CompositionGrammar::DeflectedLift),
        Composer::MeaningDirected,
    );
    let b = stable(120.0, 2112);
    let opts = PerformanceOptions::default();
    assert_ne!(
        PhenomenalTrajectory::observe(&a).summary,
        PhenomenalTrajectory::observe(&b).summary
    );
    for song in [&a, &b] {
        for world in [
            MusicWorld::swiss_signal(),
            MusicWorld::black_ice(),
            MusicWorld::vapor95(),
        ] {
            let p = perform(song, &world, opts);
            let law = SongMapConformance::check(song, &p.perf, &p.score);
            assert!(law.sites_checked > 0 && law.landmarks_checked > 0 && law.changes_checked > 0);
            assert!(law.passes(), "{}: {}", world.name, law.report());
        }
    }
}

#[test]
fn fresh_lengths_and_seeds_keep_the_distinction_or_report_short_form_limits() {
    for beats in [64.0, 97.5, 160.0] {
        for seed in [7, 991] {
            let song = stable(beats, seed);
            let observed = PhenomenalTrajectory::observe(&song);
            assert!(
                observed
                    .divergences(song.phenomenal.as_ref().unwrap())
                    .is_empty(),
                "beats {beats} seed {seed}: {}",
                observed.report()
            );
        }
    }
    let short = stable(9.0, 7);
    let observed = PhenomenalTrajectory::observe(&short);
    assert!(!observed
        .divergences(short.phenomenal.as_ref().unwrap())
        .is_empty());
}
