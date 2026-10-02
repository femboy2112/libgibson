//! The archival **VAPOR95 v1** world: the parameter set `MusicWorld::vapor95()` returned from
//! Round I through the closeout source `abc3f9f`, kept verbatim so historical VAPOR95 audio and
//! receipts stay reproducible after the product world was deliberately revised.
//!
//! This is a fixture, not a product world: it is NOT a `WorldId`, it is not exported by the
//! library, and nothing outside tests and lab examples may depend on it. Examples include it with
//! `#[path = "../tests/common/vapor95_v1.rs"]`.

// Each including crate uses a different subset of this shared helper.
#![allow(dead_code)]

use gibson::audio::dsp::drums::{KickParams, SnareParams};
use gibson::audio::dsp::osc::Wave;
use gibson::audio::human_music::{
    instrument::{OscKind, Patch},
    theory::Mode,
    world::{Space, WorldId, WorldProduction},
    MusicWorld,
};

/// The VAPOR95 v1 world, byte for byte the historical `MusicWorld::vapor95()`.
pub fn vapor95_v1() -> MusicWorld {
    MusicWorld {
        id: WorldId::Vapor95,
        name: "VAPOR95",
        tempo_bpm: 71.0,
        tonic_pc: 5, // F
        mode: Mode::Ionian,
        swing: 0.16,
        subdiv: 2,
        use_sevenths: true,
        allow_chromatic_mediant: true,
        allow_modal_mixture: true,
        allow_secondary_dominant: true,
        voicing_spread: 0.6,
        pad: Patch {
            osc: OscKind::Shape(Wave::Saw),
            sub: false,
            unison: 3,
            detune_cents: 16.0,
            cutoff_hz: 2200.0,
            cutoff_env: 700.0,
            resonance: 0.15,
            adsr: (0.4, 0.8, 0.7, 1.2),
            gain: 0.6,
            pan: 0.0,
        },
        bass: Patch {
            osc: OscKind::Shape(Wave::Triangle),
            sub: true,
            unison: 1,
            detune_cents: 0.0,
            cutoff_hz: 900.0,
            cutoff_env: 300.0,
            resonance: 0.15,
            adsr: (0.01, 0.2, 0.7, 0.25),
            gain: 0.8,
            pan: 0.0,
        },
        lead: Patch {
            osc: OscKind::Fm {
                ratio: 1.0,
                index: 1.4,
            },
            sub: false,
            unison: 1,
            detune_cents: 0.0,
            cutoff_hz: 3500.0,
            cutoff_env: 800.0,
            resonance: 0.12,
            adsr: (0.01, 0.6, 0.35, 0.6),
            gain: 0.6,
            pan: -0.1,
        },
        keys: Patch {
            osc: OscKind::Fm {
                ratio: 2.0,
                index: 1.0,
            },
            sub: false,
            unison: 1,
            detune_cents: 0.0,
            cutoff_hz: 4000.0,
            cutoff_env: 500.0,
            resonance: 0.1,
            adsr: (0.005, 0.5, 0.25, 0.5),
            gain: 0.58,
            pan: 0.15,
        },
        kick: KickParams {
            base_hz: 52.0,
            pitch_sweep_hz: 140.0,
            pitch_tau: 0.04,
            amp_tau: 0.18,
            click: 0.15,
        },
        snare: SnareParams {
            noise_hp_hz: 1200.0,
            noise_tau: 0.14,
            tone_tau: 0.1,
            tone_lo_hz: 170.0,
            tone_hi_hz: 300.0,
            noise_mix: 0.6,
        },
        hat_cutoff: 6500.0,
        ghost_amount: 0.2,
        drum_density: 0.5,
        production: WorldProduction {
            saturation: 2.2,
            space: Space {
                size: 1.3,
                damp: 0.25,
                mix: 0.34,
                low_cut_hz: None,
            },
            chorus: None,
            echo: None,
        },
        master_ceiling: 0.95,
        base_dynamic: 0.7,
        pad_mix: 0.8,
        lead_mix: 0.62,
        keys_mix: 0.9,
        bass_mix: 0.82,
    }
}
