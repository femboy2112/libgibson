//! Independent mutations of the cover/reference boundary; no seed sweep or listening claim.
use gibson::audio::human_music::{
    contract::CompositionGrammar,
    cover::{
        cover_candidate, cover_skeleton, CoverAxis, CoverMap, CoverPhrase, CoverPipelineReceipt,
        CoverSpec, CoverTarget, OrderedChart, SkeletonSchedule,
    },
    performance::PerformanceOptions,
    plan::SectionFamily,
    policy::PerformanceProfile,
    reference_song::ReferenceSong,
    MusicWorld,
};

fn reference() -> ReferenceSong {
    ReferenceSong::from_tsv(
        "meter 4 4\nkey 0 major\ntempo 100\nlength 4/1\nnote melody 0/1 1/2 60\nnote melody 2/1 1/2 64\n",
        "melody",
    ).unwrap()
}
fn target(world: &MusicWorld) -> CoverTarget<'_> {
    CoverTarget {
        world,
        seed: 9,
        grammar: CompositionGrammar::HookArc,
        options: PerformanceOptions::default(),
        profile: PerformanceProfile::POCKET,
    }
}

#[test]
fn source_record_order_is_not_musical_identity() {
    let ordered = reference();
    let reversed = ReferenceSong::from_tsv(
        "meter 4 4\nkey 0 major\ntempo 100\nlength 4/1\nnote melody 2/1 1/2 64\nnote melody 0/1 1/2 60\n",
        "melody",
    ).unwrap();
    let spec = CoverSpec::new([CoverAxis::Motif]);
    assert_eq!(
        ordered.extract(spec.clone()).unwrap(),
        reversed.extract(spec).unwrap()
    );
}

#[test]
fn invalid_form_extent_returns_error_without_arithmetic_panic() {
    let mut map = reference().extract(CoverSpec::none()).unwrap();
    map.spec = CoverSpec::new([CoverAxis::Form]);
    map.unknown_axes.retain(|a| *a != CoverAxis::Form);
    map.form = Some(vec![
        CoverPhrase {
            start_bar: 0,
            bars: u32::MAX,
            family: SectionFamily::A,
        },
        CoverPhrase {
            start_bar: u32::MAX,
            bars: 1,
            family: SectionFamily::A,
        },
    ]);
    let outcome = std::panic::catch_unwind(|| map.validate());
    assert!(
        outcome.is_ok(),
        "invalid form metadata escaped the Result boundary as a panic"
    );
    assert!(outcome.unwrap().is_err());
}

#[test]
fn invalid_skeleton_schedule_returns_error_without_arithmetic_panic() {
    let chart = OrderedChart::from_tsv("section verse 0:maj 7:maj\n").unwrap();
    let map = CoverMap::from_ordered_chart(chart).unwrap();
    let world = MusicWorld::swiss_signal();
    let outcome = std::panic::catch_unwind(|| {
        cover_skeleton(
            &map,
            target(&world),
            SkeletonSchedule {
                bars_per_chord: u32::MAX,
            },
        )
    });
    assert!(
        outcome.is_ok(),
        "invalid target schedule escaped the Result boundary as a panic"
    );
    assert!(outcome.unwrap().is_err());
}

#[test]
fn mutated_reference_metadata_is_revalidated_at_extraction() {
    let mut source = reference();
    source.tonic = i32::MIN;
    let outcome = std::panic::catch_unwind(|| source.extract(CoverSpec::new([CoverAxis::Motif])));
    assert!(
        outcome.is_ok(),
        "invalid source tonic escaped the Result boundary as a panic"
    );
    assert!(outcome.unwrap().is_err());
}

#[test]
fn cover_score_receipt_rejects_out_of_domain_pitched_event() {
    let map = reference()
        .extract(CoverSpec::new([CoverAxis::Motif]))
        .unwrap();
    let world = MusicWorld::swiss_signal();
    let mut candidate = cover_candidate(&map, target(&world)).unwrap();
    assert!(candidate.score.validate().is_ok());
    candidate.score.notes[0].pitch = 128;
    let receipt = CoverPipelineReceipt::measure(&candidate, &world);
    assert!(
        receipt.score_validation.is_some(),
        "pitch128 is outside the declared MIDI event domain"
    );
}

#[test]
fn note_domain_accepts_midi_endpoints_and_rejects_outside_them() {
    use gibson::audio::human_music::{
        form::SectionKind,
        score::{Note, Provenance, Role, Score},
    };
    let mut score = Score::new(100.0, 4.0, 4.0);
    score.notes.push(Note::new(
        0.0,
        1.0,
        0,
        0.7,
        Role::Lead,
        Provenance::new(SectionKind::A),
    ));
    for pitch in [0, 127] {
        score.notes[0].pitch = pitch;
        assert!(score.validate().is_ok(), "MIDI endpoint {pitch}");
    }
    for pitch in [-1, 128] {
        score.notes[0].pitch = pitch;
        assert!(score.validate().is_err(), "outside MIDI domain {pitch}");
    }
}

#[test]
fn accepted_maps_require_canonical_set_and_stroke_order() {
    use gibson::audio::human_music::{
        cover::{CoverStroke, GrooveVoice},
        fingerprint::CanonicalFingerprint,
        rhythm::MetricPosition,
    };
    let map = reference().extract(CoverSpec::none()).unwrap();
    assert!(map.validate().is_ok());
    let mut reversed = map.clone();
    reversed.unknown_axes.reverse();
    assert!(reversed.validate().is_err());
    let mut duplicate = map.clone();
    duplicate.unknown_axes.push(duplicate.unknown_axes[0]);
    assert!(duplicate.validate().is_err());

    let mut groove = map;
    groove.spec = CoverSpec::new([CoverAxis::Groove]);
    groove.unknown_axes.retain(|a| *a != CoverAxis::Groove);
    let kick = CoverStroke {
        at: MetricPosition::new(0, 1).unwrap(),
        voice: GrooveVoice::Kick,
    };
    let snare = CoverStroke {
        at: MetricPosition::new(1, 1).unwrap(),
        voice: GrooveVoice::Snare,
    };
    groove.groove = Some(vec![kick.clone(), snare.clone()]);
    assert!(groove.validate().is_ok());
    groove.groove = Some(vec![snare, kick.clone()]);
    assert!(groove.validate().is_err());
    groove.groove = Some(vec![kick.clone()]);
    let one_stroke = groove.canonical_fingerprint();
    groove.groove = Some(vec![kick.clone(), kick]);
    // Groove is an ordered multiset: multiplicity is retained semantic information.
    assert!(groove.validate().is_ok());
    assert_ne!(one_stroke, groove.canonical_fingerprint());
}

#[test]
fn groove_transport_must_preserve_distinct_metric_attacks() {
    use gibson::audio::human_music::{
        cover::{CoverStroke, GrooveVoice},
        rhythm::MetricPosition,
    };
    let mut map = reference().extract(CoverSpec::none()).unwrap();
    map.spec = CoverSpec::new([CoverAxis::Groove]);
    map.unknown_axes.retain(|a| *a != CoverAxis::Groove);
    map.groove = Some(vec![
        CoverStroke {
            at: MetricPosition::new(1, 2).unwrap(),
            voice: GrooveVoice::Kick,
        },
        CoverStroke {
            at: MetricPosition::new(5, 8).unwrap(),
            voice: GrooveVoice::Kick,
        },
    ]);
    assert!(map.validate().is_ok());
    let mut world = MusicWorld::vapor95();
    world.swing = 0.5;
    assert!(
        cover_candidate(&map, target(&world)).is_err(),
        "distinct metric attacks cannot collapse before drum source generation"
    );
}
