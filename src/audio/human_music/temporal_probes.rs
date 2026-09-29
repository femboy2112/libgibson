//! Discriminating controls for the independent temporal observer.
use super::composer::Composer;
use super::context::analyze;
use super::form::SectionKind;
use super::harmony::ChordSpan;
use super::ids::MaterialId;
use super::performance::{PerformanceOptions, PerformancePlan};
use super::score::{Note, PitchFunction as F, Provenance, Role, Score};
use super::semantic::deflected_lift_trace;
use super::song::SongMap;
use super::temporal::{ExtensionPath, PathStatus, TemporalPitchDiagnostics as Audit};
use super::theory::{Chord, Mode, Quality, Scale};
use super::world::MusicWorld;

fn fixture(chords: &[(f64, Quality, i32)]) -> (PerformancePlan, Score) {
    let song = SongMap::compose(
        &deflected_lift_trace(24.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let mut perf = PerformancePlan::from_song(
        &song,
        &MusicWorld::swiss_signal(),
        PerformanceOptions::default(),
    );
    perf.chords = chords
        .iter()
        .enumerate()
        .map(|(i, &(beat, quality, root))| {
            ChordSpan::test(
                beat,
                (chords.get(i + 1).map_or(24.0, |x| x.0) - beat) as f32,
                Chord::new(root, quality),
            )
        })
        .collect();
    perf.contexts = analyze(&perf.chords, &Scale::new(0, Mode::Ionian));
    let mut score = Score::new(120.0, 4.0, 24.0);
    score.chords = perf.chords.clone();
    (perf, score)
}
fn note(beat: f64, dur: f32, pitch: i32, function: F) -> Note {
    let mut n = Note::new(
        beat,
        dur,
        pitch,
        0.8,
        Role::Lead,
        Provenance::new(SectionKind::A),
    );
    n.function = Some(function);
    n
}

#[test]
fn locally_licensed_thirteenth_can_be_orphaned() {
    let (p, mut s) = fixture(&[
        (0.0, Quality::Maj7, 0),
        (4.0, Quality::Maj7, 5),
        (8.0, Quality::Maj7, 0),
    ]);
    s.notes = vec![
        note(2.0, 1.0, 69, F::LicensedExtension),
        note(3.0, 0.5, 60, F::ChordTone),
    ];
    let d = Audit::measure(&p, &s);
    assert!(d.rows[0].supported.contains(&F::LicensedExtension));
    assert_eq!(d.rows[0].extension, Some(ExtensionPath::OrphanColor));
    assert_eq!(d.rows[0].status, PathStatus::Orphaned);
    assert_eq!(d.orphan_structural_extensions, 1);
}

#[test]
fn actual_common_tone_and_forward_step_own_the_same_color() {
    let (p, mut s) = fixture(&[
        (0.0, Quality::Maj7, 0),
        (4.0, Quality::Maj7, 5),
        (8.0, Quality::Maj7, 0),
    ]);
    s.notes = vec![
        note(3.0, 0.9, 69, F::LicensedExtension),
        note(4.0, 1.0, 69, F::ChordTone),
    ];
    let d = Audit::measure(&p, &s);
    assert_eq!(d.rows[0].extension, Some(ExtensionPath::CommonTone));
    assert!(!d.rows[0].suspect);
    s.notes[1].pitch = 67; // F chord's ninth is available, but not a core resolution.
    assert_eq!(
        Audit::measure(&p, &s).rows[0].extension,
        Some(ExtensionPath::OrphanColor)
    );
    s.notes[0].pitch = 62;
    s.notes[1].pitch = 60;
    assert_eq!(
        Audit::measure(&p, &s).rows[0].extension,
        Some(ExtensionPath::ForwardLeading)
    );
}

#[test]
fn historical_membership_cannot_make_a_new_attack_a_suspension() {
    let (p, mut s) = fixture(&[(0.0, Quality::Dom7, 7), (4.0, Quality::Maj, 0)]);
    s.notes = vec![
        note(5.0, 0.9, 62, F::Suspension),
        note(6.0, 1.0, 60, F::ChordTone),
    ];
    let d = Audit::measure(&p, &s);
    assert_eq!(d.false_suspensions, 1);
    assert!(!d.rows[0].supported.contains(&F::Suspension));
}

#[test]
fn genuine_held_suspension_is_observed_across_boundary() {
    let (p, mut s) = fixture(&[(0.0, Quality::Dom7, 7), (4.0, Quality::Maj, 0)]);
    s.notes = vec![
        note(3.0, 2.0, 62, F::Suspension),
        note(5.0, 1.0, 60, F::ChordTone),
    ];
    let d = Audit::measure(&p, &s);
    assert_eq!(d.false_suspensions, 0);
    assert!(d.rows[0].supported.contains(&F::Suspension));
    assert_eq!(d.rows[0].status, PathStatus::Resolved);
}

#[test]
fn anticipation_requires_actual_arrival_and_continuation() {
    let (p, mut s) = fixture(&[(0.0, Quality::Maj, 0), (4.0, Quality::Maj, 5)]);
    s.notes = vec![
        note(3.5, 0.6, 65, F::Anticipation),
        note(4.1, 0.8, 65, F::ChordTone),
    ];
    assert_eq!(Audit::measure(&p, &s).broken_anticipations, 0);
    s.chords[1].chord = Chord::new(7, Quality::Maj);
    assert_eq!(Audit::measure(&p, &s).broken_anticipations, 1);
}

#[test]
fn endpoint_position_does_not_prove_closed_arrival() {
    let (p, mut s) = fixture(&[(0.0, Quality::Maj7, 0)]);
    let mut n = note(2.0, 1.0, 69, F::LicensedExtension);
    n.prov.material = Some(MaterialId(0));
    n.prov.closure = Some("strong");
    n.prov.role_note = "answer";
    s.notes = vec![n];
    assert_eq!(Audit::measure(&p, &s).bad_arrivals, 1);
    s.notes[0].prov.closure = Some("open");
    let d = Audit::measure_with_selected_colors(&p, &s, &[(MaterialId(0), 9)]);
    assert_eq!(d.bad_arrivals, 0);
    assert_eq!(d.rows[0].extension, Some(ExtensionPath::RestingColor));
}

#[test]
fn unrelated_legal_extensions_differ_from_smooth_i_iv_i_voice() {
    let (p, mut s) = fixture(&[
        (0.0, Quality::Maj7, 0),
        (4.0, Quality::Maj7, 5),
        (8.0, Quality::Maj7, 0),
    ]);
    s.notes = vec![
        note(3.0, 0.9, 69, F::LicensedExtension),
        note(4.0, 3.9, 79, F::LicensedExtension),
        note(8.0, 1.0, 62, F::LicensedExtension),
    ];
    let bad = Audit::measure(&p, &s);
    assert!(bad.path_breaks_at_harmony_changes >= 2);
    s.notes = vec![
        note(3.0, 0.9, 64, F::ChordTone),
        note(4.0, 3.9, 64, F::ChordTone),
        note(8.0, 1.0, 64, F::ChordTone),
    ];
    let good = Audit::measure(&p, &s);
    assert_eq!(good.path_breaks_at_harmony_changes, 0);
    assert_eq!(good.orphan_extensions, 0);
}

#[test]
fn ii_v_i_retains_and_resolves_guide_path() {
    let (p, mut s) = fixture(&[
        (0.0, Quality::Min7, 2),
        (4.0, Quality::Dom7, 7),
        (8.0, Quality::Maj7, 0),
    ]);
    s.notes = vec![
        note(3.0, 0.9, 65, F::ChordTone),
        note(4.0, 3.9, 65, F::ChordTone),
        note(8.0, 1.0, 64, F::ChordTone),
    ];
    let d = Audit::measure(&p, &s);
    assert!(d.rows.iter().all(|r| !r.suspect), "{}", d.report());
    assert_eq!(d.rows[0].destination, Some(2));
}

#[test]
fn chromatic_approach_and_passing_controls_keep_good_wrong_notes() {
    let (p, mut s) = fixture(&[(0.0, Quality::Maj, 0)]);
    s.notes = vec![
        note(0.0, 0.4, 60, F::ChordTone),
        note(0.5, 0.4, 62, F::DiatonicPassing),
        note(1.0, 0.4, 64, F::ChordTone),
        note(1.5, 0.4, 66, F::ChromaticApproach),
        note(2.0, 0.4, 67, F::ChordTone),
    ];
    let d = Audit::measure(&p, &s);
    assert_eq!(d.false_function_claims, 0, "{}", d.report());
    assert!(d.rows[1].supported.contains(&F::DiatonicPassing));
    assert!(d.rows[3].supported.contains(&F::ChromaticApproach));
}

#[test]
fn polyphonic_voice_cannot_borrow_convenient_nonadjacent_resolution() {
    let (p, mut s) = fixture(&[(0.0, Quality::Maj, 0)]);
    s.notes = vec![
        note(0.0, 0.5, 61, F::ChromaticApproach),
        note(0.0, 0.5, 67, F::ChordTone),
        note(0.5, 0.5, 48, F::ChordTone),
        note(0.5, 0.5, 62, F::LicensedExtension),
    ];
    for n in &mut s.notes {
        n.role = Role::Keys;
    }
    let d = Audit::measure(&p, &s);
    assert!(!d.rows[0].supported.contains(&F::ChromaticApproach));
    assert_eq!(d.rows[0].next_note, Some(2));
}

#[test]
fn approach_to_orphan_color_is_locally_true_but_globally_unexplained() {
    let (p, mut s) = fixture(&[(0.0, Quality::Min6, 9)]);
    s.notes = vec![
        note(0.0, 0.4, 72, F::ChordTone),
        note(0.5, 0.4, 73, F::ChromaticApproach),
        note(1.0, 0.5, 74, F::LicensedExtension),
        note(2.0, 0.4, 71, F::LicensedExtension),
        note(2.5, 0.4, 81, F::ChordTone),
    ];
    let d = Audit::measure(&p, &s);
    assert!(d.rows[1].supported.contains(&F::ChromaticApproach));
    assert_eq!(d.rows[1].status, PathStatus::Orphaned);
    assert_eq!(d.false_function_claims, 0, "{}", d.report());
}

#[test]
fn open_closure_annotation_alone_does_not_select_a_color() {
    let (p, mut s) = fixture(&[(0.0, Quality::Maj7, 0)]);
    let mut n = note(2.0, 1.0, 69, F::LicensedExtension);
    n.prov.material = Some(MaterialId(0));
    n.prov.closure = Some("open");
    s.notes = vec![n];
    assert_eq!(
        Audit::measure(&p, &s).rows[0].extension,
        Some(ExtensionPath::OrphanColor)
    );
}

#[test]
fn labels_cannot_make_a_nonmember_into_a_chord_tone() {
    let (p, mut s) = fixture(&[(0.0, Quality::Maj, 0)]);
    s.notes = vec![note(0.0, 1.0, 66, F::ChordTone)];
    let d = Audit::measure(&p, &s);
    assert_eq!(d.false_function_claims, 1);
    assert!(!d.rows[0].supported.contains(&F::ChordTone));
    s.notes[0].function = Some(F::Suspension);
    let relabeled = Audit::measure(&p, &s);
    assert_eq!(d.rows[0].supported, relabeled.rows[0].supported);
    assert_eq!(relabeled.false_suspensions, 1);
}

#[test]
fn approach_cannot_resolve_by_pitch_class_across_an_octave_jump() {
    let (p, mut s) = fixture(&[(0.0, Quality::Maj, 0)]);
    s.notes = vec![
        note(0.5, 0.4, 61, F::ChromaticApproach),
        note(1.0, 0.4, 48, F::ChordTone),
    ];
    assert_eq!(Audit::measure(&p, &s).false_function_claims, 1);
}

#[test]
fn a_monophonic_approach_can_resolve_into_the_next_material() {
    let (p, mut s) = fixture(&[(0.0, Quality::Maj, 0), (4.0, Quality::Dom7, 7)]);
    let mut a = note(3.0, 0.9, 42, F::ChromaticApproach);
    a.role = Role::Bass;
    let mut b = note(4.0, 0.5, 41, F::ChordTone);
    b.role = Role::Bass;
    b.prov.material = Some(MaterialId(0));
    s.notes = vec![a, b];
    let d = Audit::measure(&p, &s);
    assert_eq!(d.rows[0].next_note, Some(1));
    assert_eq!(d.false_function_claims, 0, "{}", d.report());
    s.notes[1].pitch += 12;
    assert_eq!(Audit::measure(&p, &s).false_function_claims, 1);
}

#[test]
fn repeated_attacks_are_not_a_physical_suspension() {
    let (p, mut s) = fixture(&[(0.0, Quality::Dom7, 7), (4.0, Quality::Maj, 0)]);
    s.notes = vec![
        note(3.0, 1.0, 62, F::ChordTone),
        note(4.0, 0.9, 62, F::Suspension),
        note(5.0, 0.5, 60, F::ChordTone),
    ];
    let d = Audit::measure(&p, &s);
    assert_eq!(d.false_suspensions, 1);
    assert!(!d.rows[1].supported.contains(&F::Suspension));
}

#[test]
fn r12_matched_scores_preserve_identity_and_reduce_path_defects() {
    use super::contract::CompositionGrammar;
    use super::diagnostics::RealizationDiagnostics;
    use super::functor::{perform, perform_temporal};
    use super::song::SongMapConformance;
    for (composer, grammar, fingerprints) in [
        (
            Composer::StablePropulsion,
            None,
            [0xdaa35c0fd38738a9, 0x493297b10781a271],
        ),
        (
            Composer::MeaningDirected,
            Some(CompositionGrammar::DeflectedLift),
            [0x81436232f8f637db, 0x9d335353c4458c48],
        ),
    ] {
        let song = SongMap::compose(&deflected_lift_trace(120.0), 2112, grammar, composer);
        for (wi, world) in [MusicWorld::swiss_signal(), MusicWorld::black_ice()]
            .iter()
            .enumerate()
        {
            let a = perform(&song, world, PerformanceOptions::default());
            let b = perform_temporal(&song, world, PerformanceOptions::default());
            assert_eq!(a.score.fingerprint(), fingerprints[wi], "R11 control drift");
            assert_eq!(a.perf.fingerprint(), b.perf.fingerprint());
            assert!(SongMapConformance::check(&song, &b.perf, &b.score).passes());
            assert_eq!(a.score.notes.len(), b.score.notes.len());
            let mut changes = 0;
            for (old, new) in a.score.notes.iter().zip(&b.score.notes) {
                assert_eq!(
                    (old.start_beat, old.dur_beats, old.role, old.velocity),
                    (new.start_beat, new.dur_beats, new.role, new.velocity)
                );
                assert_eq!(format!("{:?}", old.prov), format!("{:?}", new.prov));
                changes += usize::from(old.pitch != new.pitch);
            }
            assert!(
                changes > 0 && changes * 10 < a.score.notes.len(),
                "unbounded perturbation {changes}"
            );
            let da = Audit::measure(&a.perf, &a.score);
            let db = Audit::measure(&b.perf, &b.score);
            assert_eq!(
                db.false_function_claims
                    + db.false_suspensions
                    + db.broken_anticipations
                    + db.unresolved_tendencies
                    + db.bad_arrivals,
                0,
                "{}",
                db.report()
            );
            assert!(db.orphan_structural_extensions < da.orphan_structural_extensions);
            assert!(db.path_breaks_at_harmony_changes <= da.path_breaks_at_harmony_changes);
            assert_eq!(
                RealizationDiagnostics::measure(&song.plan, &b.score).unjustified_nonchord_notes,
                0
            );
            assert!(b
                .score
                .notes
                .iter()
                .any(|n| n.function == Some(F::ChromaticApproach)));
            assert!(db.rows.iter().any(|r| matches!(
                r.extension,
                Some(
                    ExtensionPath::OwnedTendency
                        | ExtensionPath::CommonTone
                        | ExtensionPath::ForwardLeading
                )
            )));
        }
    }
}
