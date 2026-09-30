//! Round XVII independent score/lattice witnesses and finite fresh-case invariants.
use gibson::audio::human_music::{
    composer::Composer,
    expression::{annotate, connective, valid_function},
    functor::{
        orchestration_violations, perform_coherent, perform_expressive, perform_phrased,
        perform_pocket_experiment, perform_pocketed, Composition,
    },
    identity::IdentityDiagnostics,
    performance::PerformanceOptions,
    pocket::PocketOptions,
    score::{PitchFunction, Role},
    semantic::{deflected_lift_trace, demo_trace},
    song::SongMapConformance,
    temporal::TemporalPitchDiagnostics,
    witness, MusicWorld, SongMap,
};

fn check(a: &Composition, b: &Composition, world: &MusicWorld, label: &str) {
    assert_eq!(a.song.fingerprint(), b.song.fingerprint(), "{label}");
    assert_eq!(a.perf.fingerprint(), b.perf.fingerprint(), "{label}");
    assert_eq!(
        SongMapConformance::check(&a.song, &a.perf, &a.score).report(),
        SongMapConformance::check(&b.song, &b.perf, &b.score).report(),
        "{label}"
    );
    b.score.validate().unwrap();
    assert!(
        gibson::audio::human_music::voice::continuity_violations(
            &b.score.notes,
            &b.score.voice_continuity
        )
        .is_empty(),
        "continuity {label}"
    );
    assert!(
        orchestration_violations(&b.perf, &b.score).is_empty(),
        "{label}"
    );
    assert!(b.score.stale_hearings().is_empty(), "{label}");
    for plan in &b.score.phrase_plans {
        assert!(
            b.score
                .role_notes(plan.role)
                .any(|n| n.start_beat == plan.destination.start_beat
                    && n.pitch == plan.destination.pitch),
            "local destination moved {label}: {plan:?}"
        );
    }
    for role in [Role::Lead, Role::Bass] {
        let mut notes: Vec<_> = b.score.role_notes(role).collect();
        notes.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
        for (i, n) in notes.iter().enumerate() {
            if b.score.expression_decisions.iter().any(|d| {
                d.after.is_some_and(|a| {
                    a.role == role && a.start_beat == n.start_beat && a.pitch == n.pitch
                })
            }) && connective(n.function)
            {
                assert!(
                    valid_function(
                        &b.perf,
                        i.checked_sub(1).map(|j| notes[j]),
                        n,
                        notes.get(i + 1).copied()
                    ),
                    "transformed path unsupported {label}: {n:?}"
                );
            }
        }
    }
    let r14 = perform_coherent(&a.song, world, PerformanceOptions::default());
    let source: Vec<_> = r14.score.role_notes(Role::Lead).copied().collect();
    for e in annotate(&a.perf, &source).iter().filter(|e| e.structural) {
        assert!(
            b.score
                .role_notes(Role::Lead)
                .any(|n| n.start_beat == e.note.start_beat
                    && n.pitch == e.note.pitch
                    && n.prov == e.note.prov
                    && n.dur_beats == e.note.dur_beats
                    && n.velocity == e.note.velocity
                    && n.function == e.note.function),
            "structural destination {label}: {:?}",
            e.note
        );
    }
    assert!(
        !b.score
            .notes
            .iter()
            .any(|n| n.function == Some(PitchFunction::SlidePath)),
        "{label}"
    );
    let old = witness::audit(&a.perf, &a.score);
    let new = witness::audit(&b.perf, &b.score);
    for row in old.rows.iter().filter(|r| r.witnessed) {
        assert!(
            new.rows
                .iter()
                .any(|r| r.action == row.action && r.witnessed),
            "lost action {label}: {row:?}"
        );
    }
    let ia = IdentityDiagnostics::measure_score(&a.score, &a.perf.contexts, world);
    let ib = IdentityDiagnostics::measure_score(&b.score, &b.perf.contexts, world);
    for r in ib.flips() {
        assert!(
            ia.flips()
                .any(|old| old.start_beat <= r.start_beat + 1e-6
                    && old.end_beat >= r.end_beat - 1e-6),
            "new held identity flip {label}: {r:?}"
        );
    }
    let ta = TemporalPitchDiagnostics::measure(&a.perf, &a.score);
    let tb = TemporalPitchDiagnostics::measure(&b.perf, &b.score);
    assert!(
        tb.false_function_claims <= ta.false_function_claims,
        "new temporal claim {label}: {} -> {}",
        ta.false_function_claims,
        tb.false_function_claims
    );
}

fn original_reservations(c: &Composition, world: &MusicWorld, label: &str) {
    use gibson::audio::human_music::{bass, melody, occupancy::AuthoredOccupancy};
    let source = melody::realize_lead_temporal(&c.perf, &c.song.plan);
    let lead = AuthoredOccupancy::from_lead(&c.perf, &source.notes);
    let bass_source =
        bass::realize_bass_temporal_owned(&c.perf, &c.song.plan, world, &source.notes, Some(&lead));
    let bass = AuthoredOccupancy::from_bass(&c.perf, &bass_source, &[]);
    assert_eq!(
        c.score.occupancy,
        vec![lead, bass],
        "authored-source reservations {label}"
    );
}

fn fixture() -> SongMap {
    SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    )
}

// Independent enumeration, without importing the production candidate generator.
fn on_lattice(beat: f64, world: &MusicWorld, surface: u32) -> bool {
    [
        world.subdiv,
        if world.subdiv == 3 || surface == 3 {
            3
        } else {
            world.subdiv
        },
    ]
    .into_iter()
    .any(|subdiv| {
        let nearest = (beat * f64::from(subdiv)).round() as i64;
        (nearest - 2..=nearest + 2).any(|slot| {
            let mut at = slot as f64 / f64::from(subdiv);
            let eighth = (at * 2.0).round() as i64;
            if world.swing > 0.0
                && (at * 2.0 - eighth as f64).abs() < 1e-6
                && eighth.rem_euclid(2) == 1
            {
                at += f64::from(world.swing) * 0.25;
            }
            (beat - at).abs() < 1e-6
        })
    })
}

fn pocket_witnesses(c: &Composition, world: &MusicWorld, label: &str) {
    for n in &c.score.notes {
        assert!(
            on_lattice(n.start_beat, world, c.perf.language.surface_subdivision),
            "off-lattice {label}: {n:?}"
        );
    }
    for role in [Role::Lead, Role::Bass] {
        let mut notes: Vec<_> = c.score.role_notes(role).collect();
        notes.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
        for pair in notes.windows(2) {
            let (n, target) = (pair[0], pair[1]);
            let retimed_optional = c.score.expression_decisions.iter().any(|d| {
                d.before.optional
                    && connective(d.before.note.function)
                    && d.after.is_some_and(|a| {
                        a.role == role
                            && a.pitch == n.pitch
                            && a.start_beat == n.start_beat
                            && (d.before.note.start_beat - n.start_beat).abs() > 1e-6
                    })
            });
            if retimed_optional {
                let silence = (target.start_beat - n.start_beat - f64::from(n.dur_beats)).max(0.0)
                    * 60.0
                    / f64::from(world.tempo_bpm);
                assert!(
                    silence <= 0.040 + 1e-6,
                    "connective blip {label}: silence={silence} {n:?} -> {target:?}"
                );
            }
        }
    }
}

fn source_onset(c: &Composition, n: &gibson::audio::human_music::score::Note) -> f64 {
    c.score
        .expression_decisions
        .iter()
        .find_map(|d| {
            let after = d.after?;
            ((after.start_beat - n.start_beat).abs() < 1e-6
                && ((after.role == n.role && after.pitch == n.pitch)
                    || (n.prov.role_note == "unison"
                        && after.role == Role::Lead
                        && (after.pitch - n.pitch).rem_euclid(12) == 0)))
                .then_some(d.before.note.start_beat)
        })
        .unwrap_or(n.start_beat)
}

fn matched_pitches_unchanged(a: &Composition, b: &Composition, label: &str) {
    let mut used = vec![false; a.score.notes.len()];
    for n in b.score.notes.iter().filter(|n| n.role != Role::Pad) {
        let candidates: Vec<_> = a
            .score
            .notes
            .iter()
            .enumerate()
            .filter(|(i, m)| {
                !used[*i]
                    && m.role == n.role
                    && m.prov == n.prov
                    && (source_onset(a, m) - source_onset(b, n)).abs() < 1e-6
            })
            .collect();
        if !candidates.is_empty() {
            let matched = candidates.iter().find(|(_, old)| old.pitch == n.pitch);
            assert!(
                matched.is_some(),
                "accepted pitch changed {label}: prior={candidates:?} new={n:?}"
            );
            used[matched.unwrap().0] = true;
        }
    }
}

fn no_pitch_edits(c: &Composition, label: &str) {
    for d in &c.score.expression_decisions {
        if let Some(after) = d.after {
            assert_eq!(
                d.before.note.pitch, after.pitch,
                "pitch change {label}: {d:?}"
            );
        }
    }
}

#[test]
fn frozen_historical_controls_all_off_and_pocket_flagships() {
    let song = fixture();
    for (world, old14, old15, old16) in [
        (
            MusicWorld::black_ice(),
            0x562462b4c3c7e728,
            0x32cecf0800d51ce5,
            0x71d6b6423a560637,
        ),
        (
            MusicWorld::swiss_signal(),
            0xbeb08c057ed87d94,
            0xbeb08c057ed87d94,
            0x03817af045ff24d6,
        ),
    ] {
        let r14 = perform_coherent(&song, &world, PerformanceOptions::default());
        let r15 = perform_expressive(&song, &world, PerformanceOptions::default());
        let r16 = perform_phrased(&song, &world, PerformanceOptions::default());
        assert_eq!(r14.score.fingerprint(), old14);
        assert_eq!(r15.score.fingerprint(), old15);
        assert_eq!(r16.score.fingerprint(), old16);
        let off = perform_pocket_experiment(
            &song,
            &world,
            PerformanceOptions::default(),
            PocketOptions::NONE,
        );
        assert_eq!(off.score.fingerprint(), old16);
        assert!(!off.score.mono_voice);
        assert_eq!(
            format!("{:?}", off.score.notes),
            format!("{:?}", r16.score.notes)
        );
        assert_eq!(
            format!("{:?}", off.score.drums),
            format!("{:?}", r16.score.drums)
        );
        let candidate = perform_pocketed(&song, &world, PerformanceOptions::default());
        check(&r16, &candidate, &world, world.name);
        check(&r15, &candidate, &world, world.name);
        pocket_witnesses(&candidate, &world, world.name);
        original_reservations(&candidate, &world, world.name);
        no_pitch_edits(&candidate, world.name);
        matched_pitches_unchanged(&r16, &candidate, world.name);
        let actions = witness::audit(&candidate.perf, &candidate.score);
        assert_eq!(actions.rows.len(), 45);
        assert_eq!(actions.rows.iter().filter(|a| a.witnessed).count(), 45);
    }
}

#[test]
fn swiss_support_only_keeps_accepted_other_roles_and_drums_exact() {
    let song = fixture();
    let world = MusicWorld::swiss_signal();
    let old = perform_phrased(&song, &world, PerformanceOptions::default());
    let candidate = perform_pocket_experiment(
        &song,
        &world,
        PerformanceOptions::default(),
        PocketOptions {
            support_top_voice: true,
            ..PocketOptions::NONE
        },
    );
    for role in [Role::Lead, Role::Bass, Role::Keys] {
        assert_eq!(
            format!("{:?}", old.score.role_notes(role).collect::<Vec<_>>()),
            format!("{:?}", candidate.score.role_notes(role).collect::<Vec<_>>())
        );
    }
    assert_eq!(
        format!("{:?}", old.score.drums),
        format!("{:?}", candidate.score.drums)
    );
    assert!(!candidate.score.mono_voice);
    check(&old, &candidate, &world, "swiss support only");
}

#[test]
fn primary_factorial_keeps_source_and_consumer_contracts() {
    let song = fixture();
    for world in [MusicWorld::black_ice(), MusicWorld::swiss_signal()] {
        let r16 = perform_phrased(&song, &world, PerformanceOptions::default());
        for bits in 0..16 {
            let factors = PocketOptions {
                lattice_positions: bits & 1 != 0,
                legato_connectives: bits & 2 != 0,
                mono_voice: bits & 4 != 0,
                support_top_voice: bits & 8 != 0,
                stable_precursors: false,
            };
            let c =
                perform_pocket_experiment(&song, &world, PerformanceOptions::default(), factors);
            let label = format!("{} factors={bits:04b}", world.name);
            check(&r16, &c, &world, &label);
            if bits & 7 != 0 {
                original_reservations(&c, &world, &label);
            } else {
                assert_eq!(
                    c.score.occupancy, r16.score.occupancy,
                    "historical source {label}"
                );
            }
            no_pitch_edits(&c, &label);
            matched_pitches_unchanged(&r16, &c, &label);
            assert_eq!(c.score.mono_voice, factors.mono_voice);
            if factors.lattice_positions {
                for n in &c.score.notes {
                    assert!(
                        on_lattice(n.start_beat, &world, c.perf.language.surface_subdivision),
                        "{label}: {n:?}"
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "120-case fresh sweep; VAPOR95 withheld until flagship implementation settled"]
fn r17_fresh_world_seed_tempo_sweep() {
    let mut count = 0;
    let mut failures = Vec::new();
    for seed in [3, 7, 19, 43, 101] {
        for story in [false, true] {
            for composer in [Composer::StablePropulsion, Composer::MeaningDirected] {
                let trace = if story {
                    deflected_lift_trace(120.0)
                } else {
                    demo_trace(96.0)
                };
                let song = SongMap::compose(&trace, seed, None, composer);
                for mut world in [
                    MusicWorld::swiss_signal(),
                    MusicWorld::black_ice(),
                    MusicWorld::vapor95(),
                ] {
                    for delta in [0.0, -12.0] {
                        let original = world.tempo_bpm;
                        world.tempo_bpm += delta;
                        let label = format!(
                            "seed={seed} story={story} composer={composer:?} world={} tempo={}",
                            world.name, world.tempo_bpm
                        );
                        let result = std::panic::catch_unwind(|| {
                            let r15 =
                                perform_expressive(&song, &world, PerformanceOptions::default());
                            let r16 = perform_phrased(&song, &world, PerformanceOptions::default());
                            let c = perform_pocketed(&song, &world, PerformanceOptions::default());
                            check(&r15, &c, &world, &label);
                            check(&r16, &c, &world, &label);
                            pocket_witnesses(&c, &world, &label);
                            original_reservations(&c, &world, &label);
                            no_pitch_edits(&c, &label);
                            matched_pitches_unchanged(&r16, &c, &label);
                            println!(
                                "PASS {label} notes={} decisions={}",
                                c.score.notes.len(),
                                c.score.expression_decisions.len()
                            );
                        });
                        if result.is_err() {
                            println!("FAIL {label}");
                            failures.push(label);
                        }
                        count += 1;
                        world.tempo_bpm = original;
                    }
                }
            }
        }
    }
    assert_eq!(count, 120);
    println!(
        "R17 sweep attempted={count} failed={} cases={failures:?}",
        failures.len()
    );
    assert!(failures.is_empty(), "fresh sweep failures remain failures");
    println!("R17 fresh sweep: {count} finite performances; source projection, action receipts, identity, orchestration, lattice and connective blip gates pass");
}

#[test]
fn linked_lifetime_consumers_preserve_an_unrelated_same_role_overlap() {
    use gibson::audio::human_music::{
        expression::patch,
        mass::TemporalMass,
        score::Score,
        sonority::{audible_voices, AUDIBLE_FLOOR_DB},
        tension::heard_windows_score,
        voice::{effective_audible_end_at, VoiceContinuation},
    };
    let world = MusicWorld::black_ice();
    let c = perform_phrased(&fixture(), &world, PerformanceOptions::default());
    let mut source = *c.score.role_notes(Role::Lead).next().unwrap();
    source.start_beat = 4.0;
    source.dur_beats = 2.0;
    let mut destination = source;
    destination.start_beat = 4.25;
    destination.pitch += 2;
    let mut independent = source;
    independent.start_beat = 4.125;
    independent.pitch -= 7;
    let mut score = Score::new(world.tempo_bpm, 4.0, 8.0);
    score.notes = vec![source, independent, destination];
    score.mono_voice = true;
    score.voice_continuity = vec![VoiceContinuation::new(&source, &destination).unwrap()];
    let windows = heard_windows_score(&score, &world);
    let masses = TemporalMass::of_score(&score, &c.perf.contexts, &world);
    let vertical = audible_voices(&score, &c.perf.contexts, &world, AUDIBLE_FLOOR_DB);
    for (i, n) in score.notes.iter().enumerate() {
        let expected = effective_audible_end_at(
            n,
            patch(&world, n.role),
            score.tempo_bpm,
            AUDIBLE_FLOOR_DB,
            &score.voice_continuity,
        );
        assert_eq!(windows[i].1, expected);
        assert!(
            (masses[i].audible_secs
                - (expected - n.start_beat) * 60.0 / f64::from(score.tempo_bpm))
            .abs()
                < 1e-9
        );
        let v = vertical
            .iter()
            .find(|v| v.pitch == n.pitch && v.start == n.start_beat)
            .unwrap();
        assert_eq!(v.end, expected);
    }
    assert!(
        windows[1].1 > 6.0,
        "independent voice must not be choked by destination"
    );
    assert!(
        windows[0].1 < 4.3,
        "explicit source string must end on destination"
    );
    let mut links_without_flag = score.clone();
    links_without_flag.mono_voice = false;
    assert_eq!(heard_windows_score(&links_without_flag, &world), windows);
    assert_eq!(
        TemporalMass::of_score(&links_without_flag, &c.perf.contexts, &world),
        masses
    );
    let score_id = IdentityDiagnostics::measure_score(&score, &c.perf.contexts, &world);
    let independent_ix = 1;
    assert!(score_id
        .slices
        .iter()
        .any(|s| s.end_beat > 4.3 && s.heard.contains(&independent_ix)));
}

#[test]
fn frozen_unison_predecessor_reservation_exposes_final_lead_leak() {
    let world = MusicWorld::black_ice();
    let song = fixture();
    let r16 = perform_phrased(&song, &world, PerformanceOptions::default());
    let r17 = perform_pocketed(&song, &world, PerformanceOptions::default());
    original_reservations(&r17, &world, "unison source");
    let end = |c: &Composition| {
        c.score
            .occupancy
            .iter()
            .find(|o| o.role == Role::Bass)
            .unwrap()
            .rhythm
            .iter()
            .find(|r| r.beat == 84.0)
            .unwrap()
            .end_beat
    };
    assert!((end(&r16) - 84.59375).abs() < 1e-6, "frozen R16 defect");
    assert!(
        (end(&r17) - 84.475).abs() < 1e-6,
        "original predecessor gate"
    );
}
