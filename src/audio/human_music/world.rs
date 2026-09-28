//! [`MusicWorld`] — a Skin's *sonic world / local physics*. A world does not pick notes;
//! it **constrains** the engine: the harmonic vocabulary, groove family, voicing spread,
//! timbral palette (patches), drum character and production treatment. The same semantic
//! trace under three worlds must stay recognizably one piece of music (same form, same
//! motif identity, same resolutions) while sounding like three different dialects — the
//! skin change is a natural transformation between the functors, not a new composition.

use super::super::dsp::drums::{KickParams, SnareParams};
use super::super::dsp::osc::Wave;
use super::instrument::{OscKind, Patch};
use super::theory::Mode;

/// Which sonic world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldId {
    /// Dark, sub-heavy, crisp, chromatic — an expensive cybernetic instrument.
    BlackIce,
    /// Lush, swung, saturated, extension-rich — a vaporwave workstation.
    Vapor95,
    /// Sparse, clean, articulated, restrained — Swiss/Bauhaus information design.
    SwissSignal,
}

/// A sonic world.
#[derive(Debug, Clone)]
pub struct MusicWorld {
    pub id: WorldId,
    pub name: &'static str,
    // --- global musical frame ---
    pub tempo_bpm: f32,
    pub tonic_pc: i32,
    pub mode: Mode,
    pub swing: f32,
    /// Hat subdivisions per beat (2 = 8ths, 3 = triplets, 4 = 16ths).
    pub subdiv: u32,
    // --- harmonic vocabulary (constrains harmony.rs) ---
    pub use_sevenths: bool,
    pub allow_chromatic_mediant: bool,
    pub allow_modal_mixture: bool,
    pub allow_secondary_dominant: bool,
    /// Voice-leading spacing preference (0 = tight/close, 1 = open).
    pub voicing_spread: f32,
    // --- timbral palette ---
    pub pad: Patch,
    pub bass: Patch,
    pub lead: Patch,
    pub keys: Patch,
    // --- percussion character ---
    pub kick: KickParams,
    pub snare: SnareParams,
    pub hat_cutoff: f32,
    pub ghost_amount: f32,
    pub drum_density: f32,
    // --- production ---
    pub reverb_size: f32,
    pub reverb_damp: f32,
    pub reverb_mix: f32,
    pub saturation: f32,
    pub master_ceiling: f32,
    // --- arrangement mix ---
    pub base_dynamic: f32,
    pub pad_mix: f32,
    pub lead_mix: f32,
    pub keys_mix: f32,
    pub bass_mix: f32,
}

impl MusicWorld {
    /// Construct a world by id.
    pub fn from_id(id: WorldId) -> MusicWorld {
        match id {
            WorldId::BlackIce => MusicWorld::black_ice(),
            WorldId::Vapor95 => MusicWorld::vapor95(),
            WorldId::SwissSignal => MusicWorld::swiss_signal(),
        }
    }

    /// All three worlds.
    pub fn all() -> [MusicWorld; 3] {
        [
            MusicWorld::black_ice(),
            MusicWorld::vapor95(),
            MusicWorld::swiss_signal(),
        ]
    }

    /// BLACK_ICE — dark A-minor, straight and crisp, chromatic extensions, dry space.
    pub fn black_ice() -> MusicWorld {
        MusicWorld {
            id: WorldId::BlackIce,
            name: "BLACK_ICE",
            tempo_bpm: 88.0,
            tonic_pc: 9, // A
            mode: Mode::Aeolian,
            swing: 0.0,
            subdiv: 4,
            use_sevenths: true,
            allow_chromatic_mediant: true,
            allow_modal_mixture: true,
            allow_secondary_dominant: true,
            voicing_spread: 0.35,
            pad: Patch {
                osc: OscKind::Shape(Wave::Saw),
                sub: false,
                unison: 2,
                detune_cents: 8.0,
                cutoff_hz: 1400.0,
                cutoff_env: 900.0,
                resonance: 0.35,
                adsr: (0.06, 0.4, 0.55, 0.6),
                gain: 0.5,
                pan: 0.0,
            },
            bass: Patch {
                osc: OscKind::Shape(Wave::Saw),
                sub: true,
                unison: 1,
                detune_cents: 0.0,
                cutoff_hz: 700.0,
                cutoff_env: 500.0,
                resonance: 0.3,
                adsr: (0.004, 0.12, 0.6, 0.12),
                gain: 0.85,
                pan: 0.0,
            },
            lead: Patch {
                osc: OscKind::Fm {
                    ratio: 2.0,
                    index: 2.2,
                },
                sub: false,
                unison: 1,
                detune_cents: 0.0,
                cutoff_hz: 4200.0,
                cutoff_env: 1200.0,
                resonance: 0.2,
                adsr: (0.004, 0.18, 0.4, 0.18),
                gain: 0.62,
                pan: 0.12,
            },
            keys: Patch {
                osc: OscKind::Shape(Wave::Pulse(0.35)),
                sub: false,
                unison: 1,
                detune_cents: 0.0,
                cutoff_hz: 3000.0,
                cutoff_env: 1600.0,
                resonance: 0.3,
                adsr: (0.003, 0.14, 0.0, 0.1),
                gain: 0.5,
                pan: -0.15,
            },
            kick: KickParams {
                base_hz: 48.0,
                pitch_sweep_hz: 220.0,
                pitch_tau: 0.025,
                amp_tau: 0.12,
                click: 0.4,
            },
            snare: SnareParams {
                noise_hp_hz: 1800.0,
                noise_tau: 0.10,
                tone_tau: 0.06,
                tone_lo_hz: 190.0,
                tone_hi_hz: 340.0,
                noise_mix: 0.75,
            },
            hat_cutoff: 8500.0,
            ghost_amount: 0.35,
            drum_density: 0.8,
            reverb_size: 0.6,
            reverb_damp: 0.5,
            reverb_mix: 0.14,
            saturation: 1.6,
            master_ceiling: 0.97,
            base_dynamic: 0.85,
            pad_mix: 0.55,
            lead_mix: 0.7,
            keys_mix: 0.5,
            bass_mix: 0.9,
        }
    }

    /// VAPOR95 — lush F-major, swung, detuned pads + FM keys, big soft reverb.
    pub fn vapor95() -> MusicWorld {
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
                gain: 0.55,
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
                gain: 0.5,
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
            reverb_size: 1.3,
            reverb_damp: 0.25,
            reverb_mix: 0.34,
            saturation: 2.2,
            master_ceiling: 0.95,
            base_dynamic: 0.7,
            pad_mix: 0.8,
            lead_mix: 0.62,
            keys_mix: 0.7,
            bass_mix: 0.82,
        }
    }

    /// SWISS_SIGNAL — sparse clean C-major, precise, restrained vocabulary, open voicings.
    pub fn swiss_signal() -> MusicWorld {
        MusicWorld {
            id: WorldId::SwissSignal,
            name: "SWISS_SIGNAL",
            tempo_bpm: 118.0,
            tonic_pc: 0, // C
            mode: Mode::Ionian,
            swing: 0.0,
            subdiv: 4,
            use_sevenths: false,
            allow_chromatic_mediant: false,
            allow_modal_mixture: false,
            allow_secondary_dominant: true,
            voicing_spread: 0.85,
            pad: Patch {
                osc: OscKind::Shape(Wave::Triangle),
                sub: false,
                unison: 1,
                detune_cents: 0.0,
                cutoff_hz: 2600.0,
                cutoff_env: 200.0,
                resonance: 0.08,
                adsr: (0.08, 0.5, 0.5, 0.5),
                gain: 0.35,
                pan: 0.0,
            },
            bass: Patch {
                osc: OscKind::Shape(Wave::Triangle),
                sub: true,
                unison: 1,
                detune_cents: 0.0,
                cutoff_hz: 1100.0,
                cutoff_env: 200.0,
                resonance: 0.1,
                adsr: (0.005, 0.16, 0.5, 0.14),
                gain: 0.78,
                pan: 0.0,
            },
            lead: Patch {
                osc: OscKind::Shape(Wave::Pulse(0.5)),
                sub: false,
                unison: 1,
                detune_cents: 0.0,
                cutoff_hz: 5000.0,
                cutoff_env: 700.0,
                resonance: 0.12,
                adsr: (0.003, 0.12, 0.0, 0.08),
                gain: 0.55,
                pan: 0.1,
            },
            keys: Patch {
                osc: OscKind::Shape(Wave::Sine),
                sub: false,
                unison: 1,
                detune_cents: 0.0,
                cutoff_hz: 6000.0,
                cutoff_env: 0.0,
                resonance: 0.05,
                adsr: (0.002, 0.18, 0.0, 0.1),
                gain: 0.5,
                pan: -0.12,
            },
            kick: KickParams {
                base_hz: 55.0,
                pitch_sweep_hz: 160.0,
                pitch_tau: 0.02,
                amp_tau: 0.1,
                click: 0.3,
            },
            snare: SnareParams {
                noise_hp_hz: 2000.0,
                noise_tau: 0.08,
                tone_tau: 0.05,
                tone_lo_hz: 200.0,
                tone_hi_hz: 360.0,
                noise_mix: 0.8,
            },
            hat_cutoff: 9500.0,
            ghost_amount: 0.1,
            drum_density: 0.4,
            reverb_size: 0.5,
            reverb_damp: 0.6,
            reverb_mix: 0.1,
            saturation: 1.15,
            master_ceiling: 0.98,
            base_dynamic: 0.75,
            pad_mix: 0.4,
            lead_mix: 0.62,
            keys_mix: 0.6,
            bass_mix: 0.85,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_distinct_worlds() {
        let w = MusicWorld::all();
        // Distinct tempos, tonics and groove families — the dialect axis.
        assert_ne!(w[0].tempo_bpm, w[1].tempo_bpm);
        assert_ne!(w[1].tempo_bpm, w[2].tempo_bpm);
        assert_ne!(w[0].tonic_pc, w[2].tonic_pc);
        // Vapor swings; the others are straight.
        assert!(w[1].swing > 0.1);
        assert_eq!(w[0].swing, 0.0);
        assert!(w[2].voicing_spread > w[0].voicing_spread); // open vs tighter
    }

    #[test]
    fn from_id_round_trips() {
        for id in [WorldId::BlackIce, WorldId::Vapor95, WorldId::SwissSignal] {
            assert_eq!(MusicWorld::from_id(id).id, id);
        }
    }
}
