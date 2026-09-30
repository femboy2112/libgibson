//! The cover fidelity dial: exact relations per axis, compiled from a preset, bounded by what the
//! source observed. Fresh target seeds 77_700_0xx.
use gibson::audio::human_music::{
    composer::Composer,
    contract::{CoherenceContract, CompositionGrammar},
    cover::{
        cover_candidate, AxisEvidence, CoverAxis, CoverConformance, CoverError,
        CoverFidelityPreset as Preset, CoverFidelityProfile, CoverKnowledge, CoverMap, CoverSpec,
        CoverTarget, GrooveRelation, HarmonyRelation, LineRelation, OrderedChart,
    },
    fingerprint::CanonicalFingerprint,
    functor::{perform_with_profile, Composition},
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    reference_song::ReferenceSong,
    rhythm::MetricPosition,
    semantic::deflected_lift_trace,
    MusicWorld, SongMap,
};

const ODE: &str = include_str!("../docs/fixtures/humanmusic-cover/ode-import/reference.tsv");
const SWING: &str =
    include_str!("../docs/fixtures/humanmusic-cover/swing-partial/ordered-chart.tsv");

fn generated(seed: u64, grammar: CompositionGrammar) -> (SongMap, Composition) {
    generated_in(seed, grammar, &MusicWorld::black_ice())
}

fn generated_in(
    seed: u64,
    grammar: CompositionGrammar,
    world: &MusicWorld,
) -> (SongMap, Composition) {
    let song = SongMap::compose(
        &deflected_lift_trace(32.0),
        seed,
        Some(grammar),
        Composer::StructuralR9,
    );
    let c = perform_with_profile(
        &song,
        world,
        PerformanceOptions::default(),
        PerformanceProfile::POCKET,
    )
    .expect("source");
    (song, c)
}

fn target(world: &MusicWorld, seed: u64) -> CoverTarget<'_> {
    CoverTarget {
        world,
        seed,
        grammar: CompositionGrammar::HookArc,
        options: PerformanceOptions::default(),
        profile: PerformanceProfile::POCKET,
    }
}

fn ode() -> ReferenceSong {
    ReferenceSong::from_tsv(ODE, "sop").expect("ode")
}

fn window() -> MetricPosition {
    MetricPosition::new(2, 1).unwrap()
}

/// Compatibility: the binary spec is the v1 profile. A generated source extracted at
/// `from_spec(spec)` yields exactly the v1 map — value and canonical hash.
#[test]
fn the_binary_spec_is_the_v1_profile_byte_for_byte() {
    let world = MusicWorld::black_ice();
    let mut compared = 0;
    for (seed, grammar) in [
        (77_700_001, CompositionGrammar::HookArc),
        (77_700_002, CompositionGrammar::RiffDrive),
        (77_700_003, CompositionGrammar::LoopEvolution),
    ] {
        let (song, c) = generated(seed, grammar);
        for spec in [
            CoverSpec::from_contract(&song.plan.contract),
            CoverSpec::new([CoverAxis::Motif]),
            CoverSpec::new([CoverAxis::Groove, CoverAxis::HarmonicContour]),
            CoverSpec::new([CoverAxis::Riff, CoverAxis::BassFigure]),
            CoverSpec::from_contract(&CoherenceContract::for_grammar(CompositionGrammar::HookArc))
                .with(CoverAxis::Groove, false),
        ] {
            let Ok(v1) = CoverMap::extract(&c, &world, spec.clone()) else {
                continue;
            };
            let (map, report) = CoverMap::extract_fidelity(
                &c,
                &world,
                &CoverFidelityProfile::from_spec(&spec),
                None,
            )
            .expect("v1 profile extracts where v1 does");
            assert_eq!(map, v1, "{grammar:?} {spec:?}");
            assert_eq!(map.canonical_fingerprint(), v1.canonical_fingerprint());
            assert!(map.fidelity.is_none());
            assert_eq!(report.effective.spec(), spec);
            compared += 1;
        }
    }
    assert!(compared >= 9, "{compared}");
}

/// The dial never manufactures an unobserved axis: for the Ode, groove, form and seating stay
/// Unknown at every preset; harmony stays Unknown without a supplied derived analysis and, when
/// pinned, is labelled derived. Asking for more reports the ceiling.
#[test]
fn ode_dial_never_manufactures_unobserved_axes() {
    let r = ode();
    let derived = r.derive_harmony(window()).unwrap();
    for preset in Preset::ALL {
        for d in [None, Some(&derived)] {
            let (map, report) = r
                .extract_fidelity(&CoverFidelityProfile::preset(preset), Some(preset), d)
                .unwrap();
            for axis in [CoverAxis::Groove, CoverAxis::Form, CoverAxis::Orchestration] {
                assert_eq!(
                    map.knowledge(axis),
                    CoverKnowledge::Unknown,
                    "{preset:?} {axis:?}"
                );
            }
            if d.is_none() {
                assert_eq!(
                    map.knowledge(CoverAxis::HarmonicContour),
                    CoverKnowledge::Unknown
                );
                assert!(map.harmony.is_none());
            }
            if map.harmony.is_some() {
                assert!(report
                    .axes
                    .iter()
                    .any(|a| a.axis == CoverAxis::HarmonicContour
                        && matches!(a.evidence, AxisEvidence::DerivedAnalysis(_))));
            }
        }
    }
    let (_, report) = r
        .extract_fidelity(
            &CoverFidelityProfile::preset(Preset::Strict),
            Some(Preset::Strict),
            None,
        )
        .unwrap();
    assert!(!report.at_ceiling());
    let text = report.report();
    assert!(text.contains("observational ceiling"), "{text}");
    assert!(text.contains("groove kick-snare->unknown"), "{text}");
}

/// The dial is monotone on the Ode: each stronger preset pins a superset of axes.
#[test]
fn ode_presets_pin_monotonically() {
    let r = ode();
    let derived = r.derive_harmony(window()).unwrap();
    let pinned: Vec<Vec<CoverAxis>> = Preset::ALL
        .iter()
        .map(|&p| {
            r.extract_fidelity(&CoverFidelityProfile::preset(p), Some(p), Some(&derived))
                .unwrap()
                .0
                .spec
                .axes()
                .to_vec()
        })
        .collect();
    for w in pinned.windows(2) {
        assert!(w[0].iter().all(|a| w[1].contains(a)), "{pinned:?}");
    }
    assert_eq!(pinned[0], vec![CoverAxis::Motif]);
    assert!(pinned[3].contains(&CoverAxis::BassFigure));
    assert!(pinned[3].contains(&CoverAxis::HarmonicContour));
}

fn ode_with(edit: impl Fn(&str) -> String) -> ReferenceSong {
    let text: String = ODE.lines().map(|l| edit(l) + "\n").collect();
    ReferenceSong::from_tsv(&text, "sop").unwrap()
}

fn cover_hash(map: &CoverMap, seed: u64) -> u64 {
    let world = MusicWorld::black_ice();
    cover_candidate(map, target(&world, seed))
        .expect("cover")
        .score
        .canonical_fingerprint()
}

/// Noninterference per relation: two references that differ only in what a relation excludes
/// extract to the same map and yield identical covers; the next stronger relation sees the
/// difference (positive control).
#[test]
fn each_relation_ignores_exactly_what_it_excludes() {
    let profile = |p: Preset| CoverFidelityProfile::preset(p);
    let base = ode();
    // Theme: a pitch after the two-bar theme window.
    let late_pitch = ode_with(|l| {
        if l == "note\tsop\t10/1\t1/1\t69" {
            "note\tsop\t10/1\t1/1\t64".into()
        } else {
            l.into()
        }
    });
    let (a, _) = base
        .extract_fidelity(&profile(Preset::Loose), None, None)
        .unwrap();
    let (b, _) = late_pitch
        .extract_fidelity(&profile(Preset::Loose), None, None)
        .unwrap();
    assert_eq!(a, b);
    assert_eq!(cover_hash(&a, 77_700_101), cover_hash(&b, 77_700_101));
    let metric = CoverFidelityProfile {
        motif: LineRelation::Metric,
        ..CoverFidelityProfile::FREE
    };
    assert_ne!(
        base.extract_fidelity(&metric, None, None).unwrap().0,
        late_pitch.extract_fidelity(&metric, None, None).unwrap().0
    );
    // Metric: a note's length, onsets unchanged.
    let shorter = ode_with(|l| {
        if l == "note\tsop\t14/1\t2/1\t69" {
            "note\tsop\t14/1\t1/1\t69".into()
        } else {
            l.into()
        }
    });
    let (a, _) = base.extract_fidelity(&metric, None, None).unwrap();
    let (b, _) = shorter.extract_fidelity(&metric, None, None).unwrap();
    assert_eq!(a, b);
    assert_eq!(cover_hash(&a, 77_700_102), cover_hash(&b, 77_700_102));
    let faithful = CoverFidelityProfile {
        motif: LineRelation::Faithful,
        ..CoverFidelityProfile::FREE
    };
    assert_ne!(
        base.extract_fidelity(&faithful, None, None).unwrap().0,
        shorter.extract_fidelity(&faithful, None, None).unwrap().0
    );
}

/// Every Ode preset lifts to a cover that satisfies its own relations, or is an explicit lawful
/// refusal; the loose theme is really stated.
#[test]
fn ode_covers_conform_at_every_preset() {
    let r = ode();
    let derived = r.derive_harmony(window()).unwrap();
    let mut lifted = 0;
    for world in [
        MusicWorld::black_ice(),
        MusicWorld::vapor95(),
        MusicWorld::swiss_signal(),
    ] {
        for preset in Preset::ALL {
            let (map, _) = r
                .extract_fidelity(
                    &CoverFidelityProfile::preset(preset),
                    Some(preset),
                    Some(&derived),
                )
                .unwrap();
            match cover_candidate(&map, target(&world, 77_700_201)) {
                Ok(cover) => {
                    lifted += 1;
                    let conformance = CoverConformance::check(&map, &cover, &world);
                    assert!(
                        conformance.passes(),
                        "{} {preset:?}: {}",
                        world.name,
                        conformance.report()
                    );
                }
                Err(CoverError::Invalid(why)) => {
                    assert!(
                        why.contains("outside target vocabulary"),
                        "{preset:?}: {why}"
                    )
                }
                Err(e) => panic!("{} {preset:?}: {e:?}", world.name),
            }
        }
    }
    assert!(lifted >= 8, "{lifted}");
}

/// Quality family: a pinned seventh chord into a target without sevenths is refused at Exact
/// and admitted at QualityFamily, whose cover keeps every span, root and triad family. VAPOR95
/// (Ionian, sevenths) into SWISS_SIGNAL (Ionian, triads, no mixture); a mode change is not a
/// quality family and stays a lawful refusal at both.
#[test]
fn quality_family_admits_what_exact_lawfully_refuses() {
    let swiss = MusicWorld::swiss_signal();
    let exact = CoverFidelityProfile {
        harmony: HarmonyRelation::Exact,
        ..CoverFidelityProfile::FREE
    };
    let family = CoverFidelityProfile {
        harmony: HarmonyRelation::QualityFamily,
        ..CoverFidelityProfile::FREE
    };
    let mut discriminating = 0;
    for seed in 77_700_301..77_700_321u64 {
        let vapor = MusicWorld::vapor95();
        let (_, c) = generated_in(seed, CompositionGrammar::HookArc, &vapor);
        let (e, _) = CoverMap::extract_fidelity(&c, &vapor, &exact, None).unwrap();
        let (f, _) = CoverMap::extract_fidelity(&c, &vapor, &family, None).unwrap();
        let exact_result = cover_candidate(&e, target(&swiss, seed));
        let refused = matches!(
            exact_result,
            Err(CoverError::Invalid(
                "pinned harmony outside target vocabulary"
            ))
        );
        if let Ok(cover) = cover_candidate(&f, target(&swiss, seed)) {
            let conformance = CoverConformance::check(&f, &cover, &swiss);
            assert!(
                conformance.passes(),
                "seed {seed}: {}",
                conformance.report()
            );
            if refused {
                discriminating += 1;
            }
        }
    }
    assert!(
        discriminating > 0,
        "no seed where exact refuses and the family admits"
    );
}

/// Pocket skeleton: only the kick/snare strokes on the beat grid are the song's; the cover keeps
/// them exactly.
#[test]
fn pocket_skeleton_pins_the_beat_grid_only() {
    let world = MusicWorld::black_ice();
    let profile = CoverFidelityProfile {
        groove: GrooveRelation::PocketSkeleton,
        ..CoverFidelityProfile::FREE
    };
    let (_, c) = generated(77_700_401, CompositionGrammar::HookArc);
    let (map, _) = CoverMap::extract_fidelity(&c, &world, &profile, None).unwrap();
    let full = CoverMap::extract(&c, &world, CoverSpec::new([CoverAxis::Groove])).unwrap();
    let skeleton = map.groove.as_ref().unwrap();
    assert!(!skeleton.is_empty() && skeleton.len() < full.groove.as_ref().unwrap().len());
    assert!(skeleton
        .iter()
        .all(|s| (s.at.beats() - s.at.beats().round()).abs() < 1e-9));
    let cover = cover_candidate(&map, target(&world, 77_700_402)).unwrap();
    let conformance = CoverConformance::check(&map, &cover, &world);
    assert!(conformance.passes(), "{}", conformance.report());
}

/// The partial Swing & A Miss chart: the dial cannot raise an unobserved axis. Melody, bass and
/// groove stay Unknown at every preset; the chart (ordered harmony in its section topology) is
/// pinned only when both are requested, and then exactly as the v1 partial map.
#[test]
fn swing_partial_reports_its_ceiling_and_stays_the_v1_skeleton() {
    let v1 = CoverMap::from_ordered_chart(OrderedChart::from_tsv(SWING).unwrap()).unwrap();
    for preset in Preset::ALL {
        let (map, report) = CoverMap::from_ordered_chart_fidelity(
            OrderedChart::from_tsv(SWING).unwrap(),
            &CoverFidelityProfile::preset(preset),
            Some(preset),
        )
        .unwrap();
        for a in &report.axes {
            if matches!(
                a.axis,
                CoverAxis::Motif | CoverAxis::BassFigure | CoverAxis::Groove
            ) && a.requested != "free"
            {
                assert_eq!(a.effective, "unknown", "{preset:?} {a:?}");
            }
        }
        match preset {
            Preset::Loose | Preset::Interpretive => assert!(map.is_none(), "{preset:?}"),
            Preset::Faithful | Preset::Strict => {
                assert_eq!(map.as_ref(), Some(&v1));
                assert!(!report.at_ceiling());
                assert!(report.report().contains("harmonic-contour exact->ordered"));
            }
        }
    }
}
