//! Source phrase planning, independently of downstream integration.
use gibson::audio::human_music::{
    composer::Composer,
    expression::{annotate, observe, project, ConnectiveViability},
    functor::perform_coherent,
    performance::PerformanceOptions,
    phrase_expression::{realize, PhraseTransform},
    score::Role,
    semantic::deflected_lift_trace,
    MusicWorld, SongMap,
};

fn source(w: &MusicWorld) -> gibson::audio::human_music::functor::Composition {
    let s = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    perform_coherent(&s, w, PerformanceOptions::default())
}

#[test]
fn phrase_plan_compresses_existing_optional_notes_into_real_fragments() {
    let w = MusicWorld::black_ice();
    let c = source(&w);
    let ns: Vec<_> = c.score.role_notes(Role::Lead).copied().collect();
    let line = annotate(&c.perf, &ns);
    let skeleton = project(&line);
    let a = realize(&c.perf, &w, line.clone(), &[], &[]);
    let b = realize(&c.perf, &w, line, &[], &[]);
    assert_eq!(skeleton, project(&a.events));
    assert_eq!(
        a.events
            .iter()
            .map(|e| format!("{:?}", e.note))
            .collect::<Vec<_>>(),
        b.events
            .iter()
            .map(|e| format!("{:?}", e.note))
            .collect::<Vec<_>>()
    );
    let fragments: Vec<_> = a
        .plans
        .iter()
        .filter(|p| p.transform == PhraseTransform::CompressedFragment)
        .collect();
    println!(
        "fragments={:?}",
        fragments
            .iter()
            .map(|p| (&p.source, p.grid, p.destination.start_beat))
            .collect::<Vec<_>>()
    );
    assert!(
        fragments.len() >= 3,
        "must perform multi-note content, not quantize seventeen single graces"
    );
    for plan in fragments {
        assert!(plan.source.len() >= 2 && plan.performed.len() == plan.source.len());
        assert_eq!(
            plan.source.iter().map(|n| n.pitch).collect::<Vec<_>>(),
            plan.performed.iter().map(|n| n.pitch).collect::<Vec<_>>()
        );
        for (old, new) in plan.source.iter().zip(&plan.performed) {
            assert_eq!(old.prov, new.prov);
            assert!(new.start_beat >= old.start_beat);
        }
        assert!(a
            .events
            .iter()
            .any(|e| format!("{:?}", e.note) == format!("{:?}", plan.destination)));
    }
    for (i, e) in a
        .events
        .iter()
        .enumerate()
        .filter(|(_, e)| gibson::audio::human_music::expression::connective(e.note.function))
    {
        assert_eq!(
            observe(
                &c.perf,
                &w,
                e,
                i.checked_sub(1).map(|j| &a.events[j].note),
                a.events.get(i + 1).map(|n| &n.note),
                &[]
            )
            .verdict,
            ConnectiveViability::AsWritten
        );
    }
}

#[test]
fn swiss_accepted_source_is_exactly_unchanged_by_phrase_planning() {
    let w = MusicWorld::swiss_signal();
    let c = source(&w);
    let ns: Vec<_> = c.score.role_notes(Role::Lead).copied().collect();
    let a = realize(&c.perf, &w, annotate(&c.perf, &ns), &[], &[]);
    assert_eq!(
        ns.iter().map(|n| format!("{:?}", n)).collect::<Vec<_>>(),
        a.events
            .iter()
            .map(|e| format!("{:?}", e.note))
            .collect::<Vec<_>>()
    );
    assert!(a.decisions.is_empty());
    assert!(a
        .plans
        .iter()
        .all(|p| p.transform == PhraseTransform::AsWritten));
}

#[test]
fn bass_uses_committed_lead_fragment_space_without_losing_metric_pushes() {
    let w = MusicWorld::black_ice();
    let c = source(&w);
    let lead: Vec<_> = c.score.role_notes(Role::Lead).copied().collect();
    let l = realize(&c.perf, &w, annotate(&c.perf, &lead), &[], &[]);
    let bass: Vec<_> = c.score.role_notes(Role::Bass).copied().collect();
    let b = realize(
        &c.perf,
        &w,
        annotate(&c.perf, &bass),
        &l.events.iter().map(|e| e.note).collect::<Vec<_>>(),
        &l.plans,
    );
    assert!(b
        .plans
        .iter()
        .any(|p| p.transform == PhraseTransform::CoordinatedSpace));
    for beat in [15.5, 79.5] {
        let n = b
            .events
            .iter()
            .find(|e| (e.note.start_beat - beat).abs() < 1e-6)
            .expect("Push must keep its exact attack onset");
        assert!(!n.note.prov.actions.is_empty());
    }
    for p in b
        .plans
        .iter()
        .filter(|p| p.transform == PhraseTransform::CoordinatedSpace)
    {
        assert!(p.source.iter().all(|n| n.prov.actions.is_empty()));
        assert!(l
            .plans
            .iter()
            .any(|l| l.transform == PhraseTransform::CompressedFragment
                && l.destination.start_beat == p.destination.start_beat));
        assert!(b
            .events
            .iter()
            .any(|e| format!("{:?}", e.note) == format!("{:?}", p.destination)));
    }
}

#[test]
fn empty_and_single_optional_sources_preserve_fixed_endpoints() {
    use gibson::audio::human_music::expression::ExpressionEvent;
    let w = MusicWorld::black_ice();
    let c = source(&w);
    assert!(realize(&c.perf, &w, Vec::new(), &[], &[]).events.is_empty());
    let notes: Vec<_> = c
        .score
        .role_notes(Role::Lead)
        .filter(|n| [24.0, 24.5, 25.5].contains(&n.start_beat))
        .copied()
        .collect();
    assert_eq!(notes.len(), 3);
    let line: Vec<_> = notes
        .into_iter()
        .map(|note| ExpressionEvent {
            structural: note.start_beat != 24.5,
            note,
        })
        .collect();
    let skeleton = project(&line);
    let a = realize(&c.perf, &w, line, &[], &[]);
    assert_eq!(skeleton, project(&a.events));
    assert_eq!(a.events.len(), 3);
    assert!(a.plans.iter().all(|p| p.grid
        != Some(gibson::audio::human_music::phrase_expression::PhraseGrid::EighthTriplet)));
}

#[test]
fn low_kinetic_space_is_legal_and_distinct_from_driving_fragments() {
    let w = MusicWorld::black_ice();
    let mut c = source(&w);
    let ns: Vec<_> = c.score.role_notes(Role::Lead).copied().collect();
    let line = annotate(&c.perf, &ns);
    let skeleton = project(&line);
    for b in &mut c.perf.ensemble {
        b.kinetic = 0.0;
    }
    let quiet = realize(&c.perf, &w, line.clone(), &[], &[]);
    for b in &mut c.perf.ensemble {
        b.kinetic = 0.95;
    }
    let driving = realize(&c.perf, &w, line, &[], &[]);
    assert_eq!(skeleton, project(&quiet.events));
    assert_eq!(skeleton, project(&driving.events));
    assert!(quiet.events.len() < driving.events.len());
    assert!(quiet
        .decisions
        .iter()
        .any(|d| d.after.is_none() && d.before.optional));
    assert!(driving
        .plans
        .iter()
        .any(|p| p.transform == PhraseTransform::CompressedFragment));
}

#[test]
fn harmonic_and_action_boundaries_are_not_sacrificed_for_fragments() {
    let w = MusicWorld::black_ice();
    let c = source(&w);
    let ns: Vec<_> = c.score.role_notes(Role::Lead).copied().collect();
    let a = realize(&c.perf, &w, annotate(&c.perf, &ns), &[], &[]);
    for d in &a.decisions {
        if let Some(after) = d.after {
            assert_eq!(
                c.perf
                    .context_at(d.before.note.start_beat)
                    .map(|h| h.start_beat),
                c.perf.context_at(after.start_beat).map(|h| h.start_beat)
            );
            assert_eq!(d.before.note.prov, after.prov);
        }
    }
    // These stable precursors lie before a harmony boundary and cannot be pulled across it.
    for at in [19.5, 83.5, 107.5] {
        let old = ns.iter().find(|n| n.start_beat == at).unwrap();
        assert!(a
            .events
            .iter()
            .any(|e| format!("{:?}", e.note) == format!("{:?}", old)));
    }
}

#[test]
fn swing_transport_changes_only_declared_offbeats() {
    use gibson::audio::human_music::phrase_expression::groove_position;
    let straight = MusicWorld::black_ice();
    let mut swung = straight.clone();
    swung.swing = 0.16;
    assert_eq!(groove_position(20.5, &straight), 20.5);
    assert!(groove_position(20.5, &swung) > 20.5);
    assert_eq!(groove_position(20.25, &swung), 20.25);
    assert_eq!(groove_position(20.0, &swung), 20.0);
}
