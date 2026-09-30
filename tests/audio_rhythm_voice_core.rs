//! Coordinate and direct-voice laws; cheap structural tests independent of rendering.
use gibson::audio::human_music::{
    fingerprint::CanonicalFingerprint,
    form::SectionKind,
    identity::IdentityDiagnostics,
    mass::TemporalMass,
    rhythm::{GrooveTransport, MetricPosition},
    score::{Note, Provenance, Role, Score, SfxEvent, SfxKind},
    sonority::audible_voices,
    voice::{
        audible_end, audible_end_at, effective_audible_end, patch, HeardWindows,
        ObservedLifetimePolicy, VoiceContinuation,
    },
    MusicWorld,
};

fn notes() -> Vec<Note> {
    // Unsorted, overlapping, duplicate-onset, distinct-role, and near-boundary events.
    Role::ALL
        .into_iter()
        .flat_map(|role| {
            [
                (1.0, 0.5),
                (0.0, 0.75),
                (0.75, 0.25),
                (0.75, 0.5),
                (0.7499995, 0.01),
            ]
            .into_iter()
            .enumerate()
            .map(move |(index, (at, duration))| {
                Note::new(
                    at,
                    duration,
                    60 + index as i32,
                    0.7,
                    role,
                    Provenance::new(SectionKind::A),
                )
            })
        })
        .collect()
}

#[test]
fn indexed_legacy_window_reconstruction_matches_quadratic_historical_definition() {
    let notes = notes();
    for world in [
        MusicWorld::black_ice(),
        MusicWorld::swiss_signal(),
        MusicWorld::vapor95(),
    ] {
        for tempo in [59.0, 118.0, 173.0] {
            let windows = HeardWindows::historical(&notes, &world, tempo);
            assert_eq!(windows.policy(), ObservedLifetimePolicy::LegacyRoleMasking);
            for (note, &(start, end)) in notes.iter().zip(windows.windows()) {
                let written_end = note.start_beat + f64::from(note.dur_beats);
                let envelope = audible_end(
                    note.start_beat,
                    f64::from(note.dur_beats),
                    patch(&world, note.role),
                    tempo,
                );
                let historical = notes
                    .iter()
                    .filter(|other| {
                        other.role == note.role
                            && other.start_beat >= written_end - 1e-6
                            && other.start_beat > note.start_beat + 1e-6
                    })
                    .map(|other| other.start_beat)
                    .fold(envelope, f64::min)
                    .max(note.start_beat);
                assert_eq!(start.to_bits(), note.start_beat.to_bits());
                assert_eq!(end.to_bits(), historical.to_bits());
            }
        }
    }
}

#[test]
fn one_borrowed_reconstruction_serves_mass_and_identity_without_rebuilding_lifetimes() {
    let world = MusicWorld::black_ice();
    let mut score = Score::new(world.tempo_bpm, 4.0, 4.0);
    score.notes = notes();
    for linked in [false, true] {
        score.mono_voice = linked;
        if linked {
            let lead: Vec<_> = score.role_notes(Role::Lead).copied().collect();
            score
                .voice_continuity
                .push(VoiceContinuation::new(&lead[1], &lead[2]).unwrap());
        }
        let heard = HeardWindows::of_score(&score, &world);
        assert_eq!(
            TemporalMass::of_score(&score, &[], &world),
            TemporalMass::of_heard_windows(&heard, &[], 4.0)
        );
        assert_eq!(
            IdentityDiagnostics::measure_score(&score, &[], &world),
            IdentityDiagnostics::measure_heard_windows(&heard, &[])
        );
        let voices = audible_voices(&score, &[], &world, 30.0);
        if linked {
            for note in &score.notes {
                let voice = voices
                    .iter()
                    .find(|v| v.role == note.role && v.pitch == note.pitch)
                    .unwrap();
                assert_eq!(
                    voice.end,
                    effective_audible_end(note, &world, &score.voice_continuity)
                );
            }
        }
    }
}

#[test]
fn only_explicit_edges_change_linked_lifetime_and_metric_fingerprints_are_canonical() {
    let world = MusicWorld::black_ice();
    let source = Note::new(
        0.0,
        1.0,
        60,
        0.7,
        Role::Lead,
        Provenance::new(SectionKind::A),
    );
    let mut destination = source;
    destination.start_beat = 0.25;
    destination.pitch = 62;
    let mut unrelated = source;
    unrelated.pitch = 65;
    let edge = VoiceContinuation::new(&source, &destination).unwrap();
    assert!(
        effective_audible_end(&source, &world, &[edge])
            < effective_audible_end(&source, &world, &[])
    );
    assert_eq!(
        effective_audible_end(&unrelated, &world, &[edge]),
        effective_audible_end(&unrelated, &world, &[])
    );
    let a = MetricPosition::new(6, 12).unwrap();
    let b = MetricPosition::new(1, 2).unwrap();
    assert_eq!(a.canonical_fingerprint(), b.canonical_fingerprint());
    assert_ne!(
        a.canonical_fingerprint(),
        MetricPosition::new(3, 4).unwrap().canonical_fingerprint()
    );
    let swing = GrooveTransport::eighth_swing(0.16).unwrap();
    assert_eq!(swing.transport(a).metric(), a);
    assert_ne!(swing.transport(a).beats(), a.beats());
}

#[test]
fn coincident_sfx_pitches_keep_their_own_source_envelope_after_projection() {
    let world = MusicWorld::black_ice();
    let mut score = Score::new(world.tempo_bpm, 4.0, 4.0);
    for kind in [SfxKind::Acquire, SfxKind::Transition] {
        score.sfx.push(SfxEvent {
            start_beat: 1.0,
            kind,
            velocity: 0.5,
            prov: Provenance::new(SectionKind::A),
            pitches: [60, 67],
            function: [None, None],
            owned_by: None,
            dissonance_beats: None,
        });
    }
    let voices = audible_voices(&score, &[], &world, 30.0);
    let roots: Vec<_> = voices.iter().filter(|voice| voice.pitch == 60).collect();
    assert_eq!(roots.len(), 2);
    for (voice, event) in roots.iter().zip(&score.sfx) {
        let own_patch = gibson::audio::human_music::instrument::Patch {
            adsr: event.kind.envelope(),
            ..world.lead
        };
        let (attack, decay, _, _) = event.kind.envelope();
        let written_end = event.start_beat
            + f64::from(attack + decay + event.kind.hold_secs())
                * (f64::from(score.tempo_bpm.max(1.0)) / 60.0);
        assert_eq!(
            voice.end,
            audible_end_at(
                event.start_beat,
                written_end - event.start_beat,
                &own_patch,
                score.tempo_bpm,
                30.0
            ),
        );
    }
    assert_ne!(roots[0].end, roots[1].end);
}
