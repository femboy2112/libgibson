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
    /// A half-remembered euphoric 80s pop/R&B record replayed by a computer — glassy source,
    /// soft machine pocket, chorus, note-value echoes and haze.
    Vapor95,
    /// Sparse, clean, articulated, restrained — Swiss/Bauhaus information design.
    SwissSignal,
}

/// A tempo-relative echo time: a note value, never seconds. The world states the musical
/// duration; the synthesizer converts it with the Score's tempo at the DSP boundary, so the same
/// echo stays locked to the pulse at any tempo (musical identity first, physical time second).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EchoTime {
    /// Half a beat.
    Eighth,
    /// Three quarters of a beat.
    DottedEighth,
    /// One beat.
    Quarter,
    /// One and a half beats.
    DottedQuarter,
}

impl EchoTime {
    /// The duration in beats.
    pub fn beats(self) -> f64 {
        match self {
            EchoTime::Eighth => 0.5,
            EchoTime::DottedEighth => 0.75,
            EchoTime::Quarter => 1.0,
            EchoTime::DottedQuarter => 1.5,
        }
    }
}

/// The world's shared acoustic space (the FDN reverb).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Space {
    /// Room size `[0.3, 1.5]`.
    pub size: f32,
    /// High-frequency damping `[0, 1]` (higher = darker tail).
    pub damp: f32,
    /// Wet mix `[0, 1]`.
    pub mix: f32,
    /// Below this frequency nothing enters the space, so the bass and kick stay dry and mono while
    /// the room still answers everything above. `None` = the whole band enters (historical).
    pub low_cut_hz: Option<f32>,
}

/// Slow modulation of the **memory bus** — pad, keys and lead, never bass or drums: the
/// unstable-memory smear. Peak pitch deviation is about `depth · 2π · rate` (relative).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryChorus {
    /// LFO rate (Hz).
    pub rate_hz: f32,
    /// Modulation depth (ms).
    pub depth_ms: f32,
    /// Wet mix `[0, 1]`.
    pub mix: f32,
}

/// A tempo-synchronous stereo echo fed by the articulated foreground (keys and lead — a held pad
/// would only thicken the wash, and the bass and drums keep the pocket dry). Each side repeats at
/// its own note value, so the remembered phrase moves across the stereo field; every repeat passes
/// the tone filter again and comes back darker.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TempoEcho {
    /// Left repeat time.
    pub left: EchoTime,
    /// Right repeat time.
    pub right: EchoTime,
    /// Feedback per repeat `[0, 0.9]`.
    pub feedback: f32,
    /// Send level of the keys+lead signal into the echo `[0, 1]`.
    pub send: f32,
    /// Low-pass corner of the send and of each repeat (Hz).
    pub tone_hz: f32,
}

/// A world's **production law**: what the synthesizer does to the music bus after the voices.
/// The world declares the physics; the synth executes them. Order of the chain:
/// memory bus (pad, keys, lead) → [`MemoryChorus`] → joins bass and drums → `tanh` saturation →
/// [`TempoEcho`] returns join → [`Space`] (input low-cut) → bus compressor → limiter. A stage that
/// is `None` is not computed at all, so a world without it renders exactly as before it existed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldProduction {
    /// Music-bus `tanh` drive (`>= 1`).
    pub saturation: f32,
    /// The shared space.
    pub space: Space,
    /// Memory-bus chorus, if the world has one.
    pub chorus: Option<MemoryChorus>,
    /// Tempo echo, if the world has one.
    pub echo: Option<TempoEcho>,
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
    /// The world's production law: saturation, space, memory chorus and tempo echo.
    pub production: WorldProduction,
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
                gain: 0.6,
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
                gain: 0.72,
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
            production: WorldProduction {
                saturation: 1.6,
                space: Space {
                    size: 0.6,
                    damp: 0.5,
                    mix: 0.14,
                    low_cut_hz: None,
                },
                chorus: None,
                echo: None,
            },
            master_ceiling: 0.97,
            base_dynamic: 0.85,
            pad_mix: 0.62,
            lead_mix: 0.62,
            keys_mix: 1.3,
            bass_mix: 0.9,
        }
    }

    /// VAPOR95 — a half-remembered euphoric 80s pop/R&B record replayed by a computer: a glassy
    /// FM workstation keys voice as the source object, a soft drum-machine pocket behind it, and
    /// production as an instrument — memory-bus chorus, note-value echoes that come back darker,
    /// a damped room the low end stays out of. F major, lightly swung, slowed but bouncing.
    ///
    /// Revised from the archival v1 palette (Round I to `abc3f9f`; see
    /// `docs/HUMAN_MUSIC_VAPOR95.md`): the world's dialect changed, not what a song is.
    pub fn vapor95() -> MusicWorld {
        MusicWorld {
            id: WorldId::Vapor95,
            name: "VAPOR95",
            tempo_bpm: 84.0,
            tonic_pc: 5, // F
            mode: Mode::Ionian,
            swing: 0.16,
            subdiv: 2,
            use_sevenths: true,
            allow_chromatic_mediant: true,
            allow_modal_mixture: true,
            allow_secondary_dominant: true,
            voicing_spread: 0.6,
            // Supportive atmosphere: two lightly detuned saws, filtered low, shorter tail — the
            // chorus supplies the width the v1 supersaw got from detune.
            pad: Patch {
                osc: OscKind::Shape(Wave::Saw),
                sub: false,
                unison: 2,
                detune_cents: 7.0,
                cutoff_hz: 1400.0,
                cutoff_env: 250.0,
                resonance: 0.06,
                adsr: (0.4, 1.0, 0.65, 0.7),
                gain: 0.55,
                pan: 0.0,
            },
            // Warm and defined: a filtered saw at the written pitch (no sub-octave rumble), mono.
            bass: Patch {
                osc: OscKind::Shape(Wave::Saw),
                sub: false,
                unison: 1,
                detune_cents: 0.0,
                cutoff_hz: 560.0,
                cutoff_env: 360.0,
                resonance: 0.18,
                adsr: (0.006, 0.22, 0.72, 0.16),
                gain: 0.9,
                pan: 0.0,
            },
            // A smooth melodic source for the haze to ghost: soft odd-harmonic FM, no whistle.
            lead: Patch {
                osc: OscKind::Fm {
                    ratio: 2.0,
                    index: 0.15,
                },
                sub: false,
                unison: 1,
                detune_cents: 0.0,
                cutoff_hz: 2600.0,
                cutoff_env: 700.0,
                resonance: 0.08,
                adsr: (0.018, 0.5, 0.62, 0.5),
                gain: 0.56,
                pan: -0.1,
            },
            // The source object: a glassy digital EP — ratio-3 FM (harmonics 1 2 4 5 7 8 over the
            // fundamental) whose bright attack the filter envelope lets through, then closes.
            keys: Patch {
                osc: OscKind::Fm {
                    ratio: 3.0,
                    index: 0.25,
                },
                sub: false,
                unison: 1,
                detune_cents: 0.0,
                cutoff_hz: 2200.0,
                cutoff_env: 4200.0,
                resonance: 0.12,
                adsr: (0.003, 0.6, 0.22, 0.45),
                gain: 0.72,
                pan: 0.15,
            },
            // A soft machine pocket: tighter round kick, softer snare, airier hats.
            kick: KickParams {
                base_hz: 54.0,
                pitch_sweep_hz: 150.0,
                pitch_tau: 0.03,
                amp_tau: 0.13,
                click: 0.22,
            },
            snare: SnareParams {
                noise_hp_hz: 1700.0,
                noise_tau: 0.12,
                tone_tau: 0.08,
                tone_lo_hz: 190.0,
                tone_hi_hz: 320.0,
                noise_mix: 0.68,
            },
            hat_cutoff: 7800.0,
            ghost_amount: 0.2,
            drum_density: 0.5,
            production: WorldProduction {
                saturation: 1.4,
                space: Space {
                    size: 1.2,
                    damp: 0.55,
                    mix: 0.28,
                    low_cut_hz: Some(180.0),
                },
                chorus: Some(MemoryChorus {
                    rate_hz: 0.32,
                    depth_ms: 3.5,
                    mix: 0.4,
                }),
                echo: Some(TempoEcho {
                    left: EchoTime::DottedEighth,
                    right: EchoTime::Quarter,
                    feedback: 0.38,
                    send: 0.2,
                    tone_hz: 2600.0,
                }),
            },
            master_ceiling: 0.95,
            base_dynamic: 0.7,
            pad_mix: 1.4,
            lead_mix: 0.56,
            keys_mix: 2.1,
            bass_mix: 1.4,
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
                gain: 0.42,
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
                gain: 0.58,
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
            production: WorldProduction {
                saturation: 1.15,
                space: Space {
                    size: 0.5,
                    damp: 0.6,
                    mix: 0.1,
                    low_cut_hz: None,
                },
                chorus: None,
                echo: None,
            },
            master_ceiling: 0.98,
            base_dynamic: 0.75,
            pad_mix: 0.6,
            lead_mix: 0.62,
            keys_mix: 0.9,
            bass_mix: 0.85,
        }
    }
}

/// The archival **VAPOR95 v1** world (Round I to `abc3f9f`) for in-crate tests that pin
/// historical VAPOR95 behaviour. Test-only: not a `WorldId`, not public API. It is the same
/// literal as the integration fixture `tests/common/vapor95_v1.rs`; both are checked against one
/// committed dump (`docs/fixtures/humanmusic-vaporize/v1-baseline/world-v1.txt`).
#[cfg(test)]
pub(crate) fn vapor95_v1() -> MusicWorld {
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

/// `MusicWorld::all()` as it was before the VAPOR95 revision: BLACK_ICE, VAPOR95 v1, SWISS_SIGNAL.
#[cfg(test)]
pub(crate) fn archival_worlds() -> [MusicWorld; 3] {
    [
        MusicWorld::black_ice(),
        vapor95_v1(),
        MusicWorld::swiss_signal(),
    ]
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
    fn the_archival_v1_world_is_the_committed_dump() {
        assert_eq!(
            format!("{:#?}\n", vapor95_v1()),
            include_str!("../../../docs/fixtures/humanmusic-vaporize/v1-baseline/world-v1.txt")
        );
    }

    #[test]
    fn only_vapor95_declares_the_new_production_stages() {
        // BLACK_ICE and SWISS_SIGNAL keep their historical production exactly: no chorus, no
        // echo, a full-band room (their audio is byte-identical across the revision).
        for w in [
            MusicWorld::black_ice(),
            MusicWorld::swiss_signal(),
            vapor95_v1(),
        ] {
            assert_eq!(w.production.chorus, None, "{}", w.name);
            assert_eq!(w.production.echo, None, "{}", w.name);
            assert_eq!(w.production.space.low_cut_hz, None, "{}", w.name);
        }
        let v = MusicWorld::vapor95().production;
        assert!(v.chorus.is_some() && v.echo.is_some() && v.space.low_cut_hz.is_some());
    }

    #[test]
    fn from_id_round_trips() {
        for id in [WorldId::BlackIce, WorldId::Vapor95, WorldId::SwissSignal] {
            assert_eq!(MusicWorld::from_id(id).id, id);
        }
    }
}
