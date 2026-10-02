//! Round XVII: direct-voice contract, distinct from downstream reverb and human acceptance.
use gibson::audio::{
    human_music::{
        composer::Composer,
        expression::{observe, observe_with_voice_contract, ExpressionEvent},
        form::SectionKind,
        functor::perform_coherent,
        performance::PerformanceOptions,
        score::{Note, Provenance, Role, Score},
        semantic::deflected_lift_trace,
        synth::HumanMusicSynth,
        voice::{continuity_violations, effective_audible_end, VoiceContinuation, MONO_CHOKE_SECS},
        MusicWorld, SongMap,
    },
    render::OfflineRenderer,
    time::SampleRate,
    StereoBlock,
};

fn fixture() -> (Score, MusicWorld) {
    let world = MusicWorld::black_ice();
    let mut score = Score::new(world.tempo_bpm, 4.0, 2.0);
    for role in Role::ALL {
        for (beat, pitch) in [(0.0, 60), (0.25, 64), (0.5, 67)] {
            score.notes.push(Note::new(
                beat,
                1.0,
                pitch,
                0.75,
                role,
                Provenance::new(SectionKind::A),
            ));
        }
    }
    (score, world)
}
fn render(score: &Score, world: &MusicWorld, block: usize) -> StereoBlock {
    let sr = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::new(score, world, sr);
    OfflineRenderer::new(sr, block)
        .render(&mut synth, 48_000)
        .audio
}
fn fingerprint(audio: &StereoBlock) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for x in audio.left.iter().chain(&audio.right) {
        for b in x.to_bits().to_le_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x100000001b3);
        }
    }
    h
}

#[test]
fn mono_disabled_matches_pre_edit_pcm_and_enabled_is_block_independent() {
    let (mut score, world) = fixture();
    assert!(!score.mono_voice);
    let historical = render(&score, &world, 257);
    // Captured from c9242ff's built library before production edits using Rust 1.98.1.
    assert_eq!(fingerprint(&historical), 0xe6027cf399d5cc71);
    score.mono_voice = true;
    for role in [Role::Lead, Role::Bass] {
        let notes: Vec<_> = score.role_notes(role).copied().collect();
        for pair in notes.windows(2) {
            score
                .voice_continuity
                .push(VoiceContinuation::new(&pair[0], &pair[1]).unwrap());
        }
    }
    let mono = render(&score, &world, 257);
    assert_ne!(mono.left, historical.left);
    for block in [1, 128, 511, 4096] {
        let again = render(&score, &world, block);
        assert_eq!(mono.left, again.left);
        assert_eq!(mono.right, again.right);
    }
}

#[test]
fn observer_caps_same_role_direct_voice_and_preserves_historical_ruler() {
    let world = MusicWorld::black_ice();
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let c = perform_coherent(&song, &world, PerformanceOptions::default());
    let mut n = *c
        .score
        .role_notes(Role::Lead)
        .find(|n| n.start_beat == 24.5)
        .unwrap();
    n.start_beat = 24.75;
    n.dur_beats = 0.25;
    let mut target = n;
    target.start_beat = 25.0;
    target.pitch -= 1;
    let links = [VoiceContinuation::new(&n, &target).unwrap()];
    let event = ExpressionEvent {
        note: n,
        structural: false,
    };
    let old = observe(&c.perf, &world, &event, None, Some(&target), &[]);
    let disabled =
        observe_with_voice_contract(&c.perf, &world, &event, None, Some(&target), &[], &[]);
    assert_eq!(format!("{old:?}"), format!("{disabled:?}"));
    let mono =
        observe_with_voice_contract(&c.perf, &world, &event, None, Some(&target), &[], &links);
    let expected = 0.25 * 60.0 / f64::from(world.tempo_bpm) + MONO_CHOKE_SECS;
    assert!((mono.audible_duration_secs - expected).abs() < 1e-12);
    assert!(mono.audible_duration_secs < old.audible_duration_secs);
    let mut other = target;
    other.role = Role::Bass;
    assert!(VoiceContinuation::new(&n, &other).is_none());
    let cross = observe_with_voice_contract(&c.perf, &world, &event, None, Some(&other), &[], &[]);
    assert_eq!(cross.audible_duration_secs, old.audible_duration_secs);
    let absent = observe_with_voice_contract(&c.perf, &world, &event, None, None, &[], &[]);
    assert_eq!(absent.audible_duration_secs, old.audible_duration_secs);
}

#[test]
fn continuity_is_explicit_unique_and_stable_under_arrangement_stamps() {
    let (mut score, world) = fixture();
    let from = score.notes[9];
    let to = score.notes[10];
    let link = VoiceContinuation::new(&from, &to).unwrap();
    assert!(continuity_violations(&score.notes, &[link]).is_empty());
    score.notes[9].prov.phrase = Some(99);
    score.notes[9].prov.family = Some("derived arrangement label");
    score.notes[9].dur_beats *= 0.95;
    assert!(continuity_violations(&score.notes, &[link]).is_empty());
    score.notes.push(score.notes[9]);
    assert!(continuity_violations(&score.notes, &[link])
        .iter()
        .any(|s| s.contains("2 matching events")));
    assert!(continuity_violations(&[from], &[link])
        .iter()
        .any(|s| s.contains("0 matching events")));
    let mut independent = from;
    independent.pitch += 7;
    assert_eq!(
        effective_audible_end(&independent, &world, &[link]),
        effective_audible_end(&independent, &world, &[]),
    );
}

#[test]
fn mono_factor_without_explicit_links_does_not_choke_by_role() {
    let (mut score, world) = fixture();
    let normal = render(&score, &world, 257);
    score.mono_voice = true;
    let unlinked = render(&score, &world, 257);
    assert_eq!(normal.left, unlinked.left);
    assert_eq!(normal.right, unlinked.right);
}
