//! G2 — tempo & sample-rate naturality for the locked pocket. The typed lock introduces no
//! tempo-dependent displacement (that is proven in `pocket_time`'s unit tests, with the exact
//! milliseconds); here we confirm the SHIPPING band renders cleanly and keeps the comp dead on the
//! downbeat at 80 / 104 / 140 BPM and at 44.1 / 48 kHz.

#[path = "../examples/c137_song/rehearsal.rs"]
#[allow(dead_code)]
mod rehearsal;

use gibson::audio::human_music::{
    kernel,
    score::{Role, Score},
    synth::{ProductionControl, StemMask},
    theory::Mode,
    HumanMusicSynth, MusicWorld,
};
use gibson::audio::{OfflineRenderer, SampleRate};

fn world(bpm: f32) -> MusicWorld {
    let mut w = MusicWorld::black_ice();
    w.tonic_pc = 9;
    w.mode = Mode::Aeolian;
    w.use_sevenths = true;
    w.allow_modal_mixture = true;
    w.tempo_bpm = bpm;
    w
}

fn performed(seed: u64, w: &MusicWorld) -> Score {
    let source = rehearsal::freeze(&rehearsal::rehearse(seed));
    kernel::perform_song(&source, w)
        .expect("opt-in rehearsal Score is lawful")
        .score()
        .clone()
}

#[test]
fn the_locked_band_renders_finite_across_tempo_and_sample_rate() {
    for &bpm in &[80.0f32, 104.0, 140.0] {
        let w = world(bpm);
        let score = performed(2112, &w);

        // The lock places the bar-2 downbeat comp dead on beat 4.0 at EVERY tempo — the comp beat
        // position is tempo-invariant (no early pull to scale with tempo).
        assert!(
            score
                .role_notes(Role::Keys)
                .any(|n| n.prov.role_note == "comp" && (n.start_beat - 4.0).abs() < 1e-6),
            "bpm {bpm}: the locked comp must sit on the bar-2 downbeat",
        );

        for &rate in &[SampleRate::CD, SampleRate::STUDIO] {
            let mut synth =
                HumanMusicSynth::with_production(&score, &w, rate, ProductionControl::NORMAL);
            synth.set_stem_mask(StemMask::full());
            // A few seconds is enough to catch a non-finite blow-up without a full-length render.
            let out = OfflineRenderer::new(rate, 256).render_seconds(&mut synth, 4.0);
            assert!(
                !out.had_nonfinite,
                "bpm {bpm} @ {} Hz produced a non-finite sample",
                rate.as_f64()
            );
            assert!(
                out.audio.frames() > 0,
                "bpm {bpm} @ {} Hz produced no audio",
                rate.as_f64()
            );
        }
    }
}
