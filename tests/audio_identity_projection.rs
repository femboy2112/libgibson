//! Identity, not instruments: falsifiers for the cover/anchor identity projection (committed red
//! before the repair). A role is an instrument; a declared anchor names song identity. The relation
//! under test is `declared anchor → identity-bearing source material → realized evidence`, never
//! `declared anchor → every event of one instrument`.
//!
//! Seeds 78_305_0xx are fresh. Identity-bearing material is read from the song's own declaration
//! (its identity theme sites, ThemeSite::is_identity) and from the realizers' provenance tags, so
//! the tests do not call the projection they judge.
use gibson::audio::human_music::{
    composer::Composer,
    contract::{CoherenceAnchor, CompositionGrammar},
    cover::{cover_candidate, CoverAxis, CoverConformance, CoverMap, CoverSpec, CoverTarget},
    functor::{perform_with_profile, Composition},
    ids::MaterialId,
    language::MusicalLanguage,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    score::{Note, Provenance, Role},
    semantic::{deflected_lift_trace, demo_trace},
    song::AnchorReport,
    MusicWorld, SongMap,
};

fn options(language: MusicalLanguage) -> PerformanceOptions {
    PerformanceOptions {
        language,
        ..PerformanceOptions::default()
    }
}

fn copy(c: &Composition) -> Composition {
    Composition {
        score: c.score.clone(),
        song: c.song.clone(),
        perf: c.perf.clone(),
    }
}

fn band(song: &SongMap, world: &MusicWorld, language: MusicalLanguage) -> Composition {
    perform_with_profile(song, world, options(language), PerformanceProfile::BAND).expect("BAND")
}

/// The materials of the lead statements at the song's IDENTITY theme sites.
fn identity_materials(c: &Composition) -> Vec<MaterialId> {
    c.perf
        .statements
        .iter()
        .filter(|st| {
            c.song
                .thematic
                .sites
                .iter()
                .any(|s| s.phrase == st.phrase && s.is_identity())
        })
        .map(|st| st.material)
        .collect()
}

/// The bass planner's own structural line (its figure), by its provenance tags.
const BASS_FIGURE_TAGS: [&str; 6] = ["root", "fifth", "pedal", "walk", "counter", "octave"];

fn is_bass_figure(n: &Note) -> bool {
    n.role == Role::Bass && BASS_FIGURE_TAGS.contains(&n.prov.role_note)
}

fn established(c: &Composition) -> CoverSpec {
    CoverSpec::established(&AnchorReport::check(&c.song, &c.perf, &c.score))
}

/// A HookArc/DeflectedLift source whose lead states BOTH identity and non-identity material.
fn mixed_lead_source() -> (Composition, MusicWorld) {
    let world = MusicWorld::vapor95();
    for seed in 78_305_001u64..78_305_060 {
        for grammar in [
            CompositionGrammar::HookArc,
            CompositionGrammar::DeflectedLift,
        ] {
            let song = SongMap::compose(
                &deflected_lift_trace(48.0),
                seed,
                Some(grammar),
                Composer::StructuralR9,
            );
            let c = band(&song, &world, MusicalLanguage::fusion_conversation());
            let ids = identity_materials(&c);
            let lead: Vec<_> = c.score.role_notes(Role::Lead).collect();
            let identity = lead
                .iter()
                .filter(|n| n.prov.material.is_some_and(|m| ids.contains(&m)))
                .count();
            if identity >= 3 && identity < lead.len() {
                return (c, world);
            }
        }
    }
    panic!("fixture: no source mixes identity and non-identity lead material");
}

/// Two logical materials share the Lead at one onset (an identity note and a response quote):
/// the motif is still well defined — the identity line, unchanged. The simultaneity is not
/// resolved by forcing the lead monophonic.
#[test]
fn two_materials_on_one_lead_onset_leave_the_motif_well_defined() {
    let (c, world) = mixed_lead_source();
    let clean = CoverMap::extract(&c, &world, CoverSpec::new([CoverAxis::Motif]))
        .expect("the identity line extracts");
    let ids = identity_materials(&c);
    let anchor = *c
        .score
        .role_notes(Role::Lead)
        .find(|n| n.prov.material.is_some_and(|m| ids.contains(&m)))
        .unwrap();
    let foreign = c
        .perf
        .materials
        .iter()
        .find(|m| !ids.contains(&m.id))
        .expect("a non-identity material")
        .id;
    let mut hostile = copy(&c);
    hostile.score.notes.push(Note::new(
        anchor.start_beat,
        0.25,
        anchor.pitch + 5,
        0.5,
        Role::Lead,
        Provenance {
            material: Some(foreign),
            role_note: "quote",
            ..anchor.prov
        },
    ));
    assert_eq!(
        CoverMap::extract(&hostile, &world, CoverSpec::new([CoverAxis::Motif])),
        Ok(clean),
        "a simultaneous non-identity material changed (or broke) the motif"
    );
}

/// Notes of material that is not the song's identity never establish Motif: remove every
/// identity-statement note, keep the lead's other statements, and Motif is no longer established.
#[test]
fn unrelated_lead_notes_do_not_establish_motif() {
    let (c, _) = mixed_lead_source();
    assert!(
        established(&c).contains(CoverAxis::Motif),
        "fixture: Motif established"
    );
    let ids = identity_materials(&c);
    let mut m = copy(&c);
    m.score
        .notes
        .retain(|n| !(n.role == Role::Lead && n.prov.material.is_some_and(|x| ids.contains(&x))));
    assert!(
        m.score.role_notes(Role::Lead).next().is_some(),
        "other lead material remains"
    );
    assert!(
        !established(&m).contains(CoverAxis::Motif),
        "lead notes that are not the song's identity established Motif"
    );
}

fn riff_source() -> (Composition, MusicWorld) {
    let world = MusicWorld::black_ice();
    for seed in 78_305_100u64..78_305_160 {
        let song = SongMap::compose(
            &demo_trace(48.0),
            seed,
            Some(CompositionGrammar::RiffDrive),
            Composer::StructuralR9,
        );
        let c = band(&song, &world, MusicalLanguage::fusion_conversation());
        let bass: Vec<_> = c.score.role_notes(Role::Bass).collect();
        if bass.iter().any(|n| is_bass_figure(n)) && bass.iter().any(|n| !is_bass_figure(n)) {
            return (c, world);
        }
    }
    panic!("fixture: no RiffDrive source mixes bass figure and bass non-figure notes");
}

/// Bass notes that are not the bass's own figure (quotes, answers, unison figures, approaches)
/// never establish BassFigure.
#[test]
fn unrelated_bass_notes_do_not_establish_bass_figure() {
    let (c, _) = riff_source();
    assert!(
        c.song
            .plan
            .contract
            .anchors
            .contains(&CoherenceAnchor::BassFigure)
            && established(&c).contains(CoverAxis::BassFigure),
        "fixture: BassFigure declared and established"
    );
    let mut m = copy(&c);
    m.score.notes.retain(|n| !is_bass_figure(n));
    assert!(m.score.role_notes(Role::Bass).next().is_some());
    assert!(
        !established(&m).contains(CoverAxis::BassFigure),
        "bass notes outside the bass figure established BassFigure"
    );
}

/// Neither lane's unrelated events establish a Riff: with the lead's identity statements and the
/// bass figure removed, the remaining lead and bass material does not.
#[test]
fn unrelated_lead_and_bass_notes_do_not_establish_riff() {
    let (c, _) = riff_source();
    assert!(
        established(&c).contains(CoverAxis::Riff),
        "fixture: Riff established"
    );
    let ids = identity_materials(&c);
    let mut m = copy(&c);
    m.score.notes.retain(|n| {
        !(is_bass_figure(n)
            || (n.role == Role::Lead && n.prov.material.is_some_and(|x| ids.contains(&x))))
    });
    assert!(m
        .score
        .notes
        .iter()
        .any(|n| matches!(n.role, Role::Lead | Role::Bass)));
    assert!(
        !established(&m).contains(CoverAxis::Riff),
        "events outside both lanes' identity established Riff"
    );
}

/// Chords existing is not a harmonic identity: a performance whose chord spans follow another
/// trajectory than the song's does not establish its harmonic anchor.
#[test]
fn a_wrong_harmonic_trajectory_does_not_establish_the_harmonic_anchor() {
    let world = MusicWorld::black_ice();
    for (grammar, axis) in [
        (CompositionGrammar::HookArc, CoverAxis::HarmonicContour),
        (CompositionGrammar::DeflectedLift, CoverAxis::HarmonicLoop),
    ] {
        let song = SongMap::compose(
            &deflected_lift_trace(48.0),
            78_305_200,
            Some(grammar),
            Composer::StructuralR9,
        );
        let c = band(&song, &world, MusicalLanguage::fusion_conversation());
        assert!(
            established(&c).contains(axis),
            "{grammar:?}: fixture established"
        );
        let mut m = copy(&c);
        // Another trajectory: every span a tritone away from what the song charted.
        for s in &mut m.score.chords {
            s.chord.root_pc = (s.chord.root_pc + 6).rem_euclid(12);
        }
        for s in &mut m.perf.chords {
            s.chord.root_pc = (s.chord.root_pc + 6).rem_euclid(12);
        }
        assert!(
            !established(&m).contains(axis),
            "{grammar:?}: chord spans on another trajectory established {axis:?}"
        );
    }
}

/// Action and response material never becomes song identity: a BassFigure extraction pins the
/// bass figure only — not quotes, answers, unison figures or connective approaches (holdout v2
/// H23/H31: a bass quote on a triplet onset made the whole bass lane unprojectable).
#[test]
fn the_bass_figure_quotient_holds_only_the_bass_figure() {
    let (c, world) = riff_source();
    let map = CoverMap::extract(&c, &world, CoverSpec::new([CoverAxis::BassFigure]))
        .expect("the bass figure extracts");
    let figure = c.score.notes.iter().filter(|n| is_bass_figure(n)).count();
    assert_eq!(
        map.bass.as_ref().map(|l| l.notes.len()),
        Some(figure),
        "the bass quotient holds events outside the bass figure"
    );
}

/// Figure material is never groove identity: the Groove quotient of a source whose drummer plays
/// action figures holds none of the figure's strokes (holdout v2 H35: a triplet figure stroke made
/// the groove unprojectable).
#[test]
fn figure_strokes_are_not_groove_identity() {
    let world = MusicWorld::vapor95();
    let mut checked = 0;
    for seed in 78_305_300u64..78_305_330 {
        let song = SongMap::compose(
            &deflected_lift_trace(48.0),
            seed,
            Some(CompositionGrammar::DeflectedLift),
            Composer::StructuralR9,
        );
        let c = band(&song, &world, MusicalLanguage::fusion_conversation());
        let figure_strokes = c
            .score
            .drums
            .iter()
            .filter(|d| d.prov.material.is_some())
            .count();
        if figure_strokes == 0 {
            continue;
        }
        let map = CoverMap::extract(&c, &world, CoverSpec::new([CoverAxis::Groove]))
            .expect("the groove extracts");
        let pinned = map.groove.as_ref().map_or(0, Vec::len);
        let kick_snare_without_material = c
            .score
            .drums
            .iter()
            .filter(|d| {
                matches!(
                    d.voice,
                    gibson::audio::human_music::score::DrumVoice::Kick
                        | gibson::audio::human_music::score::DrumVoice::Snare
                ) && d.prov.material.is_none()
            })
            .count();
        assert!(
            pinned <= kick_snare_without_material,
            "seed {seed}: the groove quotient pinned figure strokes ({pinned} > {kick_snare_without_material})"
        );
        checked += 1;
    }
    assert!(checked > 0, "fixture: no source with drum figure strokes");
}

/// The verifier does not certify itself: a cover that drops one pinned identity event and plays an
/// unrelated lead note (a quote) at the same onset and pitch does not conform — the verifier asks
/// for the realized identity event, not for any lead note in that place.
#[test]
fn an_unrelated_note_in_a_pinned_place_does_not_conform() {
    let (c, world) = mixed_lead_source();
    let map = CoverMap::extract(&c, &world, CoverSpec::new([CoverAxis::Motif])).unwrap();
    let target_world = MusicWorld::black_ice();
    let cover = cover_candidate(
        &map,
        CoverTarget {
            world: &target_world,
            seed: 78_305_400,
            grammar: CompositionGrammar::HookArc,
            options: options(MusicalLanguage::fusion_conversation()),
            profile: PerformanceProfile::BAND,
        },
    )
    .expect("the motif lifts");
    assert!(CoverConformance::check(&map, &cover, &target_world).passes());
    let mut forged = copy(&cover);
    let ix = forged
        .score
        .notes
        .iter()
        .position(|n| n.role == Role::Lead && n.prov.role_note == "cover-identity")
        .expect("a realized pin");
    let pin = forged.score.notes[ix];
    forged.score.notes[ix] = Note::new(
        pin.start_beat,
        pin.dur_beats,
        pin.pitch,
        pin.velocity,
        Role::Lead,
        Provenance {
            material: None,
            role_note: "quote",
            ..Provenance::new(pin.prov.section)
        },
    );
    let law = CoverConformance::check(&map, &forged, &target_world);
    assert!(
        !law.passes(),
        "an unrelated note in a pinned place certified the motif:\n{}",
        law.report()
    );
}
