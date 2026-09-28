//! Integration tests for `gibson::audio::human_music` (HumanMusic) — the procedural
//! music + SFX engine — plus the offline renderer, WAV writer and dialogue-ducking
//! sidechain it rides on. Public API only; no src/ edits, because that's not my job
//! today, it's just making sure the thing that's already there actually behaves.
//!
//! Coverage: cross-world validity/safety, determinism, the natural-transformation
//! invariant (same form skeleton across worlds, different dialects), the category laws
//! on `MusicIntent`/`IntentMorphism` (identity, composition, holonomy), `walk_intent`
//! resolution, a WAV round-trip, and the dialogue ducking sidechain.

use std::io::Read;

use gibson::audio::human_music::functor::{event_to_morphisms, walk_intent};
use gibson::audio::human_music::intent::{self, CostWeights, IntentMorphism, MusicIntent};
use gibson::audio::human_music::score::Role;
use gibson::audio::human_music::synth::HumanMusicSynth;
use gibson::audio::human_music::theory::Function;
use gibson::audio::human_music::world::WorldId;
use gibson::audio::human_music::{compose, demo_trace, render, MusicWorld};
use gibson::audio::media::{ClipAudio, DialogueTrack};
use gibson::audio::render::OfflineRenderer;
use gibson::audio::wav::write_wav_i16;
use gibson::audio::{SampleRate, StereoBlock};

// Silence an unused-import complaint on the event_to_morphisms re-export check below —
// it's actually used, but keep the import list tidy either way.
#[allow(unused_imports)]
use gibson::audio::human_music::semantic::EventKind;

fn tmp_path(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "libgibson_humanmusic_test_{}_{}.wav",
        std::process::id(),
        name
    ));
    p
}

// ---------------------------------------------------------------------------------
// 1. Cross-world render is valid & safe.
// ---------------------------------------------------------------------------------

#[test]
fn cross_world_render_is_valid_and_safe() {
    let trace = demo_trace(120.0);
    for world in MusicWorld::all() {
        let (score, out) = render(&trace, &world, 2112, SampleRate::STUDIO, 512);

        score
            .validate()
            .unwrap_or_else(|e| panic!("{}: score invalid: {e}", world.name));
        assert!(
            !out.had_nonfinite,
            "{}: render produced non-finite samples",
            world.name
        );
        assert!(
            out.peak <= 1.0,
            "{}: peak {} exceeds ceiling (limiter should hold this)",
            world.name,
            out.peak
        );
        assert!(out.frames() > 0, "{}: zero frames rendered", world.name);

        for role in [Role::Pad, Role::Bass, Role::Lead, Role::Keys] {
            let n = score.role_notes(role).count();
            assert!(n > 0, "{}: role {:?} has no notes", world.name, role);
        }
        assert!(!score.drums.is_empty(), "{}: no drum hits", world.name);
        assert!(!score.sfx.is_empty(), "{}: no sfx events", world.name);
    }
}

// ---------------------------------------------------------------------------------
// 2. Determinism: same (trace, world, seed) -> byte-identical audio + identical score.
// ---------------------------------------------------------------------------------

#[test]
fn same_seed_is_byte_identical() {
    let trace = demo_trace(48.0); // short: this test renders audio twice, keep it snappy
    let world = MusicWorld::from_id(WorldId::BlackIce);

    let (score_a, out_a) = render(&trace, &world, 2112, SampleRate::STUDIO, 512);
    let (score_b, out_b) = render(&trace, &world, 2112, SampleRate::STUDIO, 512);

    assert_eq!(
        out_a.audio.left, out_b.audio.left,
        "left channel diverged across identical renders"
    );
    assert_eq!(
        out_a.audio.right, out_b.audio.right,
        "right channel diverged across identical renders"
    );

    assert_eq!(score_a.notes.len(), score_b.notes.len());
    for (na, nb) in score_a.notes.iter().zip(score_b.notes.iter()) {
        assert_eq!(na.pitch, nb.pitch);
        assert!((na.start_beat - nb.start_beat).abs() < 1e-12);
    }
}

// ---------------------------------------------------------------------------------
// 3. Natural-transformation invariant: same trace -> same section skeleton across
//    worlds, but the dialects (tempos) genuinely differ.
// ---------------------------------------------------------------------------------

#[test]
fn same_trace_same_skeleton_different_dialect() {
    let trace = demo_trace(120.0);
    let scores: Vec<_> = MusicWorld::all()
        .into_iter()
        .map(|w| compose(&trace, &w, 1))
        .collect();

    let kinds = |s: &gibson::audio::human_music::Score| {
        s.sections
            .iter()
            .map(|sec| sec.kind.label())
            .collect::<Vec<_>>()
    };
    let skeleton0 = kinds(&scores[0]);
    for s in &scores[1..] {
        assert_eq!(
            kinds(s),
            skeleton0,
            "section-kind skeleton differs across worlds"
        );
    }

    // Different worlds must sound like different dialects: tempos differ pairwise.
    let tempos: Vec<f32> = scores.iter().map(|s| s.tempo_bpm).collect();
    assert_ne!(tempos[0], tempos[1]);
    assert_ne!(tempos[1], tempos[2]);
    assert_ne!(tempos[0], tempos[2]);
}

// ---------------------------------------------------------------------------------
// 4. Category laws via the public intent API.
// ---------------------------------------------------------------------------------

#[test]
fn identity_prolong_leaves_core_state_unchanged() {
    let a = MusicIntent::default();
    let (b, _cost) = IntentMorphism::Prolong.apply(a);
    assert_eq!(b.energy, a.energy);
    assert_eq!(b.tension, a.tension);
    assert_eq!(b.function, a.function);
}

#[test]
fn composition_matches_sequential_application_and_summed_cost() {
    let a = MusicIntent::default();
    let seq = [IntentMorphism::Prepare, IntentMorphism::Resolve];

    let (composed_state, composed_cost) = intent::compose(a, &seq);

    let (m1, c1) = IntentMorphism::Prepare.apply(a);
    let (m2, c2) = IntentMorphism::Resolve.apply(m1);

    assert_eq!(
        composed_state, m2,
        "compose() state diverges from manual sequential apply"
    );
    assert_eq!(
        composed_cost,
        c1.combine(c2),
        "compose() cost diverges from manually combined costs"
    );

    // The weighted scalar should also agree, since it's a pure function of the cost.
    let w = CostWeights::default();
    assert_eq!(composed_cost.weighted(&w), c1.combine(c2).weighted(&w));
}

#[test]
fn modulate_round_trip_has_holonomy() {
    // A -> (Modulate) -> B -> (Modulate) -> C -> (Resolve) -> back to Function::Tonic,
    // but the accumulated path leaves marks: expectation/motif development don't reset
    // to the untouched default. Round trips in this category are not identity.
    let start = MusicIntent::default();
    let (end, _cost) = intent::compose(
        start,
        &[
            IntentMorphism::Modulate,
            IntentMorphism::Modulate,
            IntentMorphism::Resolve,
        ],
    );

    assert_eq!(
        end.function,
        Function::Tonic,
        "Resolve should land back on Tonic"
    );
    assert_ne!(
        end,
        MusicIntent::default(),
        "round trip erased its own history — no holonomy"
    );
    // Specifically: two Modulates should have developed the motif, and that isn't
    // undone by Resolve (Resolve only touches function/tension/expectation).
    assert!(
        end.motif.development >= 2,
        "motif development didn't persist through the round trip"
    );
}

// ---------------------------------------------------------------------------------
// 5. walk_intent resolves by the end of the demo arc.
// ---------------------------------------------------------------------------------

#[test]
fn walk_intent_resolves_and_develops_motif() {
    let intent = walk_intent(&demo_trace(120.0));
    assert!(
        intent.tension < 0.5,
        "demo arc should end resolved; tension = {}",
        intent.tension
    );
    assert!(
        intent.motif.development > 0,
        "demo arc should develop the motif along the way"
    );
}

// Sanity: event_to_morphisms is a real total function over the trace's event kinds (used
// above transitively via walk_intent; exercise it directly too since it's public API).
#[test]
fn event_to_morphisms_is_nonempty_for_every_demo_event() {
    let trace = demo_trace(48.0);
    assert!(
        !trace.events.is_empty(),
        "demo_trace produced no events to check"
    );
    for ev in &trace.events {
        let morphs = event_to_morphisms(ev.kind, ev.state.tone);
        assert!(
            !morphs.is_empty(),
            "event kind {:?} mapped to zero morphisms",
            ev.kind
        );
    }
}

// ---------------------------------------------------------------------------------
// 6. WAV round-trip.
// ---------------------------------------------------------------------------------

#[test]
fn wav_round_trip_has_correct_riff_header_and_data_length() {
    let trace = demo_trace(48.0);
    let world = MusicWorld::from_id(WorldId::SwissSignal);
    let (_score, out) = render(&trace, &world, 7, SampleRate::STUDIO, 512);
    assert!(out.frames() > 0);

    let path = tmp_path("roundtrip");
    write_wav_i16(&path, &out.audio, SampleRate::STUDIO).expect("write_wav_i16 failed");

    let mut bytes = Vec::new();
    std::fs::File::open(&path)
        .expect("could not reopen written wav")
        .read_to_end(&mut bytes)
        .expect("could not read written wav");

    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(&bytes[12..16], b"fmt ");
    assert_eq!(&bytes[36..40], b"data");

    let data_len = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]) as usize;
    let expected_len = out.frames() * 2 /* channels */ * 2 /* bytes per i16 sample */;
    assert_eq!(
        data_len, expected_len,
        "data chunk length doesn't match frames*2ch*2bytes"
    );
    assert_eq!(
        bytes.len(),
        44 + expected_len,
        "file length doesn't match header + declared data"
    );

    std::fs::remove_file(&path).ok();
}

// ---------------------------------------------------------------------------------
// 7. Media / ducking.
// ---------------------------------------------------------------------------------

fn sine_clip(sr: SampleRate, secs: f64, amp: f32) -> ClipAudio {
    let n = sr.samples_in(secs) as usize;
    let l: Vec<f32> = (0..n).map(|i| amp * (i as f32 * 0.05).sin()).collect();
    ClipAudio {
        left: l.clone(),
        right: l,
        sr,
    }
}

#[test]
fn dialogue_track_is_silent_before_placement_and_energetic_after() {
    let sr = SampleRate::STUDIO;
    let mut track = DialogueTrack::new();
    track.place(sine_clip(sr, 0.2, 0.8), 0.5, 1.0); // dialogue starts at 0.5s

    let total = sr.samples_in(1.0);
    let out = OfflineRenderer::new(sr, 512).render(&mut track, total);

    let before_end = sr.samples_in(0.5) as usize;
    let before = &out.audio.left[..before_end];
    assert!(
        before.iter().all(|&s| s.abs() < 1e-6),
        "audio before placement isn't silent"
    );

    let after_start = before_end;
    let after_end = (before_end + sr.samples_in(0.1) as usize).min(out.frames());
    let after = &out.audio.left[after_start..after_end];
    assert!(
        after.iter().any(|&s| s.abs() > 0.1),
        "audio after placement isn't energetic"
    );
}

#[test]
fn duck_curve_pulls_mid_dialogue_down_but_leaves_idle_alone() {
    let sr = SampleRate::STUDIO;
    let mut track = DialogueTrack::new();
    track.place(sine_clip(sr, 0.5, 0.9), 0.5, 1.0); // dialogue occupies 0.5..1.0s

    let total = sr.samples_in(1.5) as usize;
    let duck = track.duck_curve(total, sr, 0.3, 5.0, 120.0);

    let idle = duck[sr.samples_in(0.1) as usize]; // well before any dialogue
    let mid_dialogue = duck[sr.samples_in(0.75) as usize]; // deep in the dialogue window

    assert!(
        idle > 0.95,
        "idle music was ducked when nothing was speaking: {idle}"
    );
    assert!(
        mid_dialogue < 0.5,
        "music wasn't pulled down under dialogue: {mid_dialogue}"
    );
    assert!(
        duck.iter().all(|&g| (0.3..=1.0001).contains(&g)),
        "duck gain left [floor,1]"
    );
}

// ---------------------------------------------------------------------------------
// 8. Score summary is a real, non-empty diagnostic string.
// ---------------------------------------------------------------------------------

#[test]
fn score_summary_is_nonempty_and_reports_tempo() {
    let trace = demo_trace(48.0);
    let world = MusicWorld::from_id(WorldId::Vapor95);
    let score = compose(&trace, &world, 42);

    let summary = score.summary();
    assert!(!summary.is_empty(), "summary() returned nothing");
    assert!(
        summary.contains("tempo="),
        "summary() is missing the tempo= field: {summary}"
    );
}

// ---------------------------------------------------------------------------------
// Bonus: the synth + generic StereoBlock surface used by everything above actually
// agrees with itself (total_samples matches what OfflineRenderer.render() produces),
// since that's the load-bearing plumbing every other test relies on.
// ---------------------------------------------------------------------------------

#[test]
fn synth_total_samples_matches_render_frame_count() {
    let trace = demo_trace(48.0);
    let world = MusicWorld::from_id(WorldId::BlackIce);
    let score = compose(&trace, &world, 5);
    let sr = SampleRate::STUDIO;

    let mut synth = HumanMusicSynth::new(&score, &world, sr);
    let frames = synth.total_samples();
    assert!(frames > 0);

    let out: gibson::audio::render::RenderResult =
        OfflineRenderer::new(sr, 256).render(&mut synth, frames);
    assert_eq!(out.frames(), frames as usize);

    let block: &StereoBlock = &out.audio;
    assert!(!block.has_nonfinite());
    assert!(block.peak() <= 1.0);
    assert!(block.rms() >= 0.0);
}
