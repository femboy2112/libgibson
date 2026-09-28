//! The synthesizer: realize a [`Score`] to audio through the DSP layer. It implements
//! [`AudioSource`], so the deterministic offline renderer and (under `audio-cpal`) a live
//! device drive the *same* code — one synthesizer, not a test double.
//!
//! Events are pre-scheduled to sample-accurate positions; the render loop triggers them as
//! its monotonic playhead crosses them, drives per-role voice pools and synthesized drums,
//! applies world production (saturation → reverb → bus compression → limiter) and writes
//! the master into the output block. It is designed for sequential offline rendering start to
//! finish; [`HumanMusicSynth::rewind`] rewinds the transport, but a *bit-exact* re-render
//! should use a fresh synth (`rewind` does not zero DSP tails — see its docs).

use super::super::dsp::drums::{Clap, Hat, Kick, Snare};
use super::super::dsp::env::Adsr;
use super::super::dsp::filter::Svf;
use super::super::dsp::fx::{pan, soft_saturate, Compressor, Limiter, Reverb};
use super::super::dsp::osc::{FmOsc, Osc, Wave};
use super::super::render::{AudioSource, RenderCtx};
use super::super::time::{SampleRate, SampleTime, TempoMap};
use super::super::StereoBlock;
use super::instrument::{OscKind, Patch};
use super::score::{DrumVoice, Role, Score, SfxKind};
use super::theory::{midi_to_hz, Scale};
use super::world::MusicWorld;

/// A single polyphonic synth voice built from a [`Patch`].
struct SynthVoice {
    oscs: Vec<Osc>,
    detune_ratio: Vec<f32>,
    sub: Option<Osc>,
    fm: Option<FmOsc>,
    fm_ratio: f32,
    fm_index: f32,
    amp: Adsr,
    fenv: Adsr,
    filt: Svf,
    cutoff_base: f32,
    cutoff_env: f32,
    resonance: f32,
    gain: f32,
    pan: f32,
    velocity: f32,
    remaining: i64,
    gated_off: bool,
    tick: u32,
    age: u64,
}

impl SynthVoice {
    fn new(patch: &Patch, sr: f32) -> SynthVoice {
        let unison = patch.unison.max(1) as usize;
        let mut oscs = Vec::with_capacity(unison);
        let mut detune_ratio = Vec::with_capacity(unison);
        for i in 0..unison {
            let mut o = Osc::new(sr);
            if let OscKind::Shape(w) = patch.osc {
                o.set_shape(w);
            } else {
                o.set_shape(Wave::Sine); // FM handled separately
            }
            oscs.push(o);
            // Symmetric detune spread in cents -> ratio.
            let spread = if unison > 1 {
                (i as f32 / (unison - 1) as f32 - 0.5) * 2.0
            } else {
                0.0
            };
            detune_ratio.push(2f32.powf(spread * patch.detune_cents / 1200.0));
        }
        let sub = if patch.sub {
            let mut s = Osc::new(sr);
            s.set_shape(Wave::Sine);
            Some(s)
        } else {
            None
        };
        let (fm, fm_ratio, fm_index) = match patch.osc {
            OscKind::Fm { ratio, index } => (Some(FmOsc::new(sr)), ratio, index),
            _ => (None, 1.0, 0.0),
        };
        let mut amp = Adsr::new(sr);
        let (a, d, s, r) = patch.adsr;
        amp.set(a, d, s, r);
        let mut fenv = Adsr::new(sr);
        fenv.set(a.max(0.002), d, s * 0.6, r);
        SynthVoice {
            oscs,
            detune_ratio,
            sub,
            fm,
            fm_ratio,
            fm_index,
            amp,
            fenv,
            filt: Svf::new(sr),
            cutoff_base: patch.cutoff_hz,
            cutoff_env: patch.cutoff_env,
            resonance: patch.resonance,
            gain: patch.gain,
            pan: patch.pan,
            velocity: 0.0,
            remaining: 0,
            gated_off: true,
            tick: 0,
            age: 0,
        }
    }

    fn trigger(&mut self, freq: f32, velocity: f32, dur_samples: i64) {
        for (o, r) in self.oscs.iter_mut().zip(self.detune_ratio.iter()) {
            o.set_freq(freq * r);
        }
        if let Some(s) = &mut self.sub {
            s.set_freq(freq * 0.5);
        }
        if let Some(fm) = &mut self.fm {
            fm.set(freq, self.fm_ratio, self.fm_index);
        }
        self.amp.gate_on();
        self.fenv.gate_on();
        self.filt.reset();
        self.velocity = velocity;
        self.remaining = dur_samples.max(1);
        self.gated_off = false;
        self.age = 0;
    }

    fn active(&self) -> bool {
        self.amp.is_active()
    }

    /// One mono sample of this voice (unpanned).
    fn next(&mut self) -> f32 {
        if !self.amp.is_active() {
            return 0.0;
        }
        let mut raw = 0.0;
        if let Some(fm) = &mut self.fm {
            raw += fm.next();
        }
        for o in self.oscs.iter_mut() {
            raw += o.next();
        }
        if !self.oscs.is_empty() {
            raw /= self.oscs.len().max(1) as f32;
        }
        if let Some(s) = &mut self.sub {
            raw += s.next() * 0.6;
        }
        let fe = self.fenv.next();
        if self.tick % 16 == 0 {
            let cut = (self.cutoff_base + self.cutoff_env * fe).clamp(30.0, 20_000.0);
            self.filt.set(cut, self.resonance);
        }
        self.tick = self.tick.wrapping_add(1);
        let a = self.amp.next();
        let y = self.filt.process(raw * a) * self.gain * self.velocity;

        self.remaining -= 1;
        if self.remaining <= 0 && !self.gated_off {
            self.amp.gate_off();
            self.fenv.gate_off();
            self.gated_off = true;
        }
        self.age += 1;
        y
    }
}

/// A pre-scheduled note event (sample-accurate).
struct NoteEvent {
    at: u64,
    dur: i64,
    freq: f32,
    velocity: f32,
    role: Role,
}

struct DrumEvent {
    at: u64,
    voice: DrumVoice,
    velocity: f32,
}

struct SfxEventS {
    at: u64,
    kind: SfxKind,
    velocity: f32,
    freqs: [f32; 2],
}

/// A debug **stem/bus mask**: which voice families are routed into the render. This is an
/// experimental lab/debug surface (Rust-only, not part of the C ABI) for stem isolation — render
/// one bus at a time to answer "which voice is that bad tone in?" instead of guessing. A muted bus
/// still advances its voices (so the event timeline is identical); it is simply not summed into the
/// mix, so the full mask reproduces the normal render exactly.
#[derive(Debug, Clone, Copy)]
pub struct StemMask {
    pub pad: bool,
    pub keys: bool,
    pub bass: bool,
    pub lead: bool,
    pub drums: bool,
    pub sfx: bool,
}

impl Default for StemMask {
    fn default() -> StemMask {
        StemMask::full()
    }
}

impl StemMask {
    /// The bus names, in a fixed order — for `--stems` iteration and CLI parsing.
    pub const NAMES: [&'static str; 6] = ["pad", "keys", "bass", "lead", "drums", "sfx"];

    /// Every bus audible (the normal full mix).
    pub const fn full() -> StemMask {
        StemMask {
            pad: true,
            keys: true,
            bass: true,
            lead: true,
            drums: true,
            sfx: true,
        }
    }

    /// Every bus muted.
    pub const fn silent() -> StemMask {
        StemMask {
            pad: false,
            keys: false,
            bass: false,
            lead: false,
            drums: false,
            sfx: false,
        }
    }

    /// Solo exactly one named bus (all others muted). An unrecognized name solos nothing.
    pub fn solo(name: &str) -> StemMask {
        let mut m = StemMask::silent();
        match name {
            "pad" => m.pad = true,
            "keys" => m.keys = true,
            "bass" => m.bass = true,
            "lead" => m.lead = true,
            "drums" => m.drums = true,
            "sfx" => m.sfx = true,
            _ => {}
        }
        m
    }
}

/// One bus's realized level over a render: RMS (of the mono mixdown) and peak (max channel
/// magnitude). Both are the bus's *own* contribution, before the shared master chain.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BusLevel {
    pub rms: f32,
    pub peak: f32,
}

/// Per-bus level readout for the six voice families. A lab/diagnostic surface (Rust-only, not in
/// the C ABI): it answers "how loud does each bus *actually* sit?" so a mix can be balanced by
/// reading one render instead of soloing every stem and eyeballing six files. The whole point of
/// Round VI — decoupling semantic foreground from brute loudness — needs a truthful ruler, and a
/// ruler that the StemMask could silence would be no ruler at all: the meter is fed regardless of
/// the mute (see [`BusMeter::tap`]).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BusLevels {
    pub pad: BusLevel,
    pub keys: BusLevel,
    pub bass: BusLevel,
    pub lead: BusLevel,
    pub drums: BusLevel,
    pub sfx: BusLevel,
}

/// The per-bus RMS+peak accumulator, folded every sample. Indices match [`StemMask::NAMES`]:
/// 0 pad, 1 keys, 2 bass, 3 lead, 4 drums, 5 sfx. It is a pure observer — it never feeds back into
/// the audio, so it cannot perturb bit-exactness.
#[derive(Default)]
struct BusMeter {
    frames: u64,
    sumsq: [f64; 6],
    peak: [f32; 6],
}

impl BusMeter {
    /// Fold one sample's stereo contribution for `bus` into the meter (mono energy + channel peak).
    /// Called for every bus each sample, muted or not — the mute gate stops audio, never the meter.
    fn tap(&mut self, bus: usize, l: f32, r: f32) {
        let mono = 0.5 * (l + r);
        self.sumsq[bus] += (mono as f64) * (mono as f64);
        let p = l.abs().max(r.abs());
        if p > self.peak[bus] {
            self.peak[bus] = p;
        }
    }

    fn levels(&self) -> BusLevels {
        let n = self.frames.max(1) as f64;
        let mk = |i: usize| BusLevel {
            rms: (self.sumsq[i] / n).sqrt() as f32,
            peak: self.peak[i],
        };
        BusLevels {
            pad: mk(0),
            keys: mk(1),
            bass: mk(2),
            lead: mk(3),
            drums: mk(4),
            sfx: mk(5),
        }
    }
}

/// The HumanMusic synthesizer.
pub struct HumanMusicSynth {
    total_samples: u64,
    // Voice pools per role.
    pads: Vec<SynthVoice>,
    keys: Vec<SynthVoice>,
    bass: Vec<SynthVoice>,
    lead: Vec<SynthVoice>,
    // Drums (monophonic per voice; the hat chokes itself).
    kick: Kick,
    snare: Snare,
    hat: Hat,
    clap: Clap,
    // SFX voices (simple two-osc gestures).
    sfx_voices: Vec<SfxVoice>,
    // Production.
    reverb: Reverb,
    sat_drive: f32,
    comp: Compressor,
    limiter: Limiter,
    music_gain: f32,
    sfx_gain: f32,
    // Per-role bus mix (the world's *_mix knobs — how loud each voice family sits
    // relative to the others before the shared production chain; previously declared
    // per-world and then completely ignored, which is a bit rude to whoever tuned them).
    pad_mix: f32,
    keys_mix: f32,
    bass_mix: f32,
    lead_mix: f32,
    // Scheduled events + cursors.
    notes: Vec<NoteEvent>,
    drums: Vec<DrumEvent>,
    sfx: Vec<SfxEventS>,
    ncur: usize,
    dcur: usize,
    scur: usize,
    playhead: u64,
    // Optional per-sample music duck gain (set for reaction integration).
    duck: Option<Vec<f32>>,
    // Debug stem/bus mask (experimental lab surface); full by default = the normal mix.
    stems: StemMask,
    // Per-bus level meter (lab/diagnostic), accumulated every sample independent of `stems`.
    meter: BusMeter,
}

impl HumanMusicSynth {
    /// Build a synth for `score` under `world` at `sr`.
    pub fn new(score: &Score, world: &MusicWorld, sr: SampleRate) -> HumanMusicSynth {
        let srf = sr.as_f64() as f32;
        // Tempo is the SCORE's — the score IS the composition, the world is only the dialect.
        // compose() sets score.tempo_bpm from world.tempo_bpm so they agree today, but the
        // score is the single declared source of truth for realization (a score re-skinned
        // under a different world would otherwise silently clock at the wrong tempo).
        let tempo = TempoMap::new(sr, score.tempo_bpm as f64, 4);
        let scale = Scale::new(world.tonic_pc, world.mode);

        let mk_pool = |patch: &Patch, n: usize| -> Vec<SynthVoice> {
            (0..n).map(|_| SynthVoice::new(patch, srf)).collect()
        };

        // Schedule notes.
        let mut notes: Vec<NoteEvent> = score
            .notes
            .iter()
            .map(|n| NoteEvent {
                at: tempo.beat_to_sample(n.start_beat).0,
                dur: (n.dur_beats as f64 * tempo.samples_per_beat()).round() as i64,
                freq: midi_to_hz(n.pitch),
                velocity: n.velocity,
                role: n.role,
            })
            .collect();
        notes.sort_by_key(|e| e.at);

        let mut drums: Vec<DrumEvent> = score
            .drums
            .iter()
            .map(|d| DrumEvent {
                at: tempo.beat_to_sample(d.start_beat).0,
                voice: d.voice,
                velocity: d.velocity,
            })
            .collect();
        drums.sort_by_key(|e| e.at);

        let mut sfx: Vec<SfxEventS> = score
            .sfx
            .iter()
            .map(|s| SfxEventS {
                at: tempo.beat_to_sample(s.start_beat).0,
                kind: s.kind,
                velocity: s.velocity,
                freqs: sfx_freqs(s.kind, &scale),
            })
            .collect();
        sfx.sort_by_key(|e| e.at);

        let mut reverb = Reverb::new(srf);
        reverb.set(world.reverb_size, world.reverb_damp, world.reverb_mix);
        let mut comp = Compressor::new(srf);
        comp.set(-14.0, 2.5, 12.0, 140.0, 1.5);
        let mut limiter = Limiter::new(srf);
        limiter.set_ceiling(world.master_ceiling);

        let total_samples =
            (score.total_beats * tempo.samples_per_beat()).round() as u64 + (srf * 2.5) as u64; // tail for reverb/release

        HumanMusicSynth {
            total_samples,
            pads: mk_pool(&world.pad, 6),
            keys: mk_pool(&world.keys, 6),
            bass: mk_pool(&world.bass, 3),
            lead: mk_pool(&world.lead, 3),
            kick: Kick::with_params(srf, world.kick),
            snare: Snare::with_params(srf, world.snare),
            hat: Hat::with_cutoff(srf, world.hat_cutoff),
            clap: Clap::new(srf),
            sfx_voices: (0..4).map(|_| SfxVoice::new(srf)).collect(),
            reverb,
            sat_drive: world.saturation,
            comp,
            limiter,
            music_gain: world.base_dynamic.clamp(0.4, 1.0),
            sfx_gain: 0.8,
            pad_mix: world.pad_mix,
            keys_mix: world.keys_mix,
            bass_mix: world.bass_mix,
            lead_mix: world.lead_mix,
            notes,
            drums,
            sfx,
            ncur: 0,
            dcur: 0,
            scur: 0,
            playhead: 0,
            duck: None,
            stems: StemMask::full(),
            meter: BusMeter::default(),
        }
    }

    /// Total samples this score will render (including the reverb/release tail).
    pub fn total_samples(&self) -> u64 {
        self.total_samples
    }

    /// Rewind the scheduling cursors and playhead to the start of the score.
    ///
    /// This resets *scheduling* only — it does NOT zero the voice envelopes, the drum voices,
    /// or the DSP effect tails (reverb / bus compressor / limiter all carry state). A render
    /// resumed over a live tail would therefore not be bit-identical to one from a fresh
    /// synth, so for a clean re-render construct a new [`HumanMusicSynth`] rather than relying
    /// on this. Named honestly: it rewinds the transport, it is not a full DSP reset.
    pub fn rewind(&mut self) {
        self.ncur = 0;
        self.dcur = 0;
        self.scur = 0;
        self.playhead = 0;
        // The meter measures the transport, so it rewinds with it — otherwise a resumed render
        // would double-count. (A clean re-render still wants a fresh synth; see the doc above.)
        self.meter = BusMeter::default();
    }

    /// Provide a per-sample music-bus duck gain (used by the reaction integrator to duck
    /// the score under dialogue). Length should cover the render.
    pub fn set_duck(&mut self, duck: Vec<f32>) {
        self.duck = Some(duck);
    }

    /// Set the debug **stem mask** (experimental lab/debug surface). Muted buses still have their
    /// voices advanced — so the event timeline is unchanged — they are just not summed into the
    /// mix. [`StemMask::full`] (the default) reproduces the normal render exactly. Use
    /// [`StemMask::solo`] to isolate one bus for debugging a bad tone.
    pub fn set_stem_mask(&mut self, mask: StemMask) {
        self.stems = mask;
    }

    /// The realized per-bus levels (RMS + peak) accumulated over the render so far. This is the
    /// Round VI mix ruler: read it after driving the synth to see how loud each voice family
    /// actually sits, without soloing and re-rendering every stem. It is INDEPENDENT of the
    /// [`StemMask`] — a muted bus still reports its true level, so isolation never lies to the meter.
    pub fn bus_levels(&self) -> BusLevels {
        self.meter.levels()
    }

    fn trigger_note(&mut self, ev: &NoteEvent) {
        let pool = match ev.role {
            Role::Pad => &mut self.pads,
            Role::Keys => &mut self.keys,
            Role::Bass => &mut self.bass,
            Role::Lead => &mut self.lead,
        };
        // Find an inactive voice, else steal the oldest.
        let idx = pool.iter().position(|v| !v.active()).unwrap_or_else(|| {
            pool.iter()
                .enumerate()
                .max_by_key(|(_, v)| v.age)
                .map(|(i, _)| i)
                .unwrap_or(0)
        });
        pool[idx].trigger(ev.freq, ev.velocity, ev.dur);
    }
}

impl AudioSource for HumanMusicSynth {
    fn render(&mut self, out: &mut StereoBlock, ctx: &RenderCtx) {
        let frames = out.frames();
        for i in 0..frames {
            let abs = ctx.start.0 + i as u64;
            self.playhead = abs;

            // --- Trigger scheduled events at this sample. ---
            while self.ncur < self.notes.len() && self.notes[self.ncur].at <= abs {
                let ev = NoteEvent {
                    at: self.notes[self.ncur].at,
                    dur: self.notes[self.ncur].dur,
                    freq: self.notes[self.ncur].freq,
                    velocity: self.notes[self.ncur].velocity,
                    role: self.notes[self.ncur].role,
                };
                self.trigger_note(&ev);
                self.ncur += 1;
            }
            while self.dcur < self.drums.len() && self.drums[self.dcur].at <= abs {
                let v = self.drums[self.dcur].voice;
                let vel = self.drums[self.dcur].velocity;
                match v {
                    DrumVoice::Kick => self.kick.trigger(vel),
                    DrumVoice::Snare => self.snare.trigger(vel),
                    DrumVoice::ClosedHat => self.hat.trigger(vel, false),
                    DrumVoice::OpenHat => self.hat.trigger(vel, true),
                    DrumVoice::Clap => self.clap.trigger(vel),
                }
                self.dcur += 1;
            }
            while self.scur < self.sfx.len() && self.sfx[self.scur].at <= abs {
                let e = &self.sfx[self.scur];
                if let Some(v) = self.sfx_voices.iter_mut().find(|v| !v.active()) {
                    v.trigger(e.kind, e.freqs, e.velocity);
                }
                self.scur += 1;
            }

            // --- Sum the music bus (melodic voices + drums). ---
            let mut ml = 0.0f32;
            let mut mr = 0.0f32;
            // Each role pool gets the world's tuned *_mix before it joins the bus — this is
            // the knob BLACK_ICE turns up on bass and VAPOR95 eases off on, not just four
            // numbers that sat in the struct looking pretty.
            for (bus, (pool, mix, on)) in [
                (&mut self.pads, self.pad_mix, self.stems.pad),
                (&mut self.keys, self.keys_mix, self.stems.keys),
                (&mut self.bass, self.bass_mix, self.stems.bass),
                (&mut self.lead, self.lead_mix, self.stems.lead),
            ]
            .into_iter()
            .enumerate()
            {
                // Sum the bus's own post-mix stereo contribution first, then meter it and only
                // then decide whether it joins the master — so the meter sees the true level even
                // when the mask has this bus muted.
                let mut bl = 0.0f32;
                let mut br = 0.0f32;
                for v in pool.iter_mut() {
                    // A muted bus still advances its voice (identical timeline); it just is not summed.
                    if v.active() {
                        let s = v.next();
                        let (l, r) = pan(s, v.pan);
                        bl += l * mix;
                        br += r * mix;
                    }
                }
                self.meter.tap(bus, bl, br);
                if on {
                    ml += bl;
                    mr += br;
                }
            }
            // Drums (center-ish placement).
            let k = self.kick.next();
            let sn = self.snare.next();
            let ht = self.hat.next();
            let cl = self.clap.next();
            let dl = k + sn + ht * 0.85 + cl * 0.6;
            let dr = k + sn + ht * 1.0 + cl * 0.8;
            self.meter.tap(4, dl, dr);
            if self.stems.drums {
                ml += dl;
                mr += dr;
            }

            // Music production: saturation -> reverb send.
            ml = soft_saturate(ml * 0.6, self.sat_drive);
            mr = soft_saturate(mr * 0.6, self.sat_drive);
            let (ml, mr) = self.reverb.process_stereo(ml, mr);

            // Duck the music under dialogue if a curve is set.
            let duck = self
                .duck
                .as_ref()
                .and_then(|d| d.get(abs as usize).copied())
                .unwrap_or(1.0);

            // --- SFX bus. ---
            let mut sfx_l = 0.0f32;
            let mut sfx_r = 0.0f32;
            for v in self.sfx_voices.iter_mut() {
                if v.active() {
                    let (l, r) = v.next();
                    sfx_l += l;
                    sfx_r += r;
                }
            }
            self.meter.tap(5, sfx_l, sfx_r);
            // One frame folded into every bus's meter — count it once, here, not six times.
            self.meter.frames += 1;
            let (sl, sr) = if self.stems.sfx {
                (sfx_l, sfx_r)
            } else {
                (0.0, 0.0)
            };

            // --- Master mix + bus comp + limiter. ---
            let mut lx = ml * self.music_gain * duck + sl * self.sfx_gain;
            let mut rx = mr * self.music_gain * duck + sr * self.sfx_gain;
            let (cl2, cr2) = self.comp.process_stereo(lx, rx);
            lx = cl2;
            rx = cr2;
            let (fl, fr) = self.limiter.process_stereo(lx, rx);
            out.left[i] = fl;
            out.right[i] = fr;
        }
    }

    fn active_voices(&self) -> usize {
        let melodic: usize = [&self.pads, &self.keys, &self.bass, &self.lead]
            .iter()
            .map(|p| p.iter().filter(|v| v.active()).count())
            .sum();
        let drums = self.kick.is_active() as usize
            + self.snare.is_active() as usize
            + self.hat.is_active() as usize
            + self.clap.is_active() as usize;
        let sfx = self.sfx_voices.iter().filter(|v| v.active()).count();
        melodic + drums + sfx
    }

    fn is_finished(&self, at: SampleTime) -> bool {
        at.0 >= self.total_samples
    }
}

/// A short synthesized SFX gesture (two detuned FM/osc blips + noise, world-tuned pitches).
struct SfxVoice {
    osc_a: FmOsc,
    osc_b: Osc,
    amp: Adsr,
    filt: Svf,
    active: bool,
    pan: f32,
    vel: f32,
    // Bounded one-shot lifecycle. Previously an SFX gated its envelope ON and NOTHING ever
    // gated it off, so it parked in Stage::Sustain forever (a held tone that rang to the end of
    // the render, and a permanently-occupied voice slot). Mirrors the melodic SynthVoice: count
    // down to a note-off so attack+decay+hold+release is finite. `sr` lets trigger() turn the
    // kind's declared hold seconds into a sample count.
    remaining: i64,
    gated_off: bool,
    sr: f32,
}

impl SfxVoice {
    fn new(sr: f32) -> SfxVoice {
        let mut osc_b = Osc::new(sr);
        osc_b.set_shape(Wave::Triangle);
        SfxVoice {
            osc_a: FmOsc::new(sr),
            osc_b,
            amp: Adsr::new(sr),
            filt: Svf::new(sr),
            active: false,
            pan: 0.0,
            vel: 0.0,
            remaining: 0,
            gated_off: true,
            sr,
        }
    }

    fn active(&self) -> bool {
        self.active && self.amp.is_active()
    }

    fn trigger(&mut self, kind: SfxKind, freqs: [f32; 2], vel: f32) {
        self.osc_a.set(freqs[0], 2.0, 1.5);
        self.osc_b.set_freq(freqs[1]);
        // The envelope is the kind's own declared property (score.rs) — the synth no longer keeps
        // a private second copy that could drift from it.
        let (a, d, s, r) = kind.envelope();
        self.amp.set(a, d, s, r);
        self.amp.gate_on();
        self.filt.set(
            if matches!(kind, SfxKind::Danger | SfxKind::Impact) {
                1800.0
            } else {
                5000.0
            },
            0.3,
        );
        self.filt.reset();
        self.pan = match kind {
            SfxKind::Acquire => 0.2,
            SfxKind::Warning => -0.2,
            _ => 0.0,
        };
        self.active = true;
        self.vel = vel;
        // Arm the note-off: hold through attack + decay + the kind's declared hold, then gate off
        // so the release tail carries it to silence. This is what makes the gesture finite.
        self.remaining = (((a + d + kind.hold_secs()) * self.sr).round() as i64).max(1);
        self.gated_off = false;
    }

    fn next(&mut self) -> (f32, f32) {
        let a = self.amp.next();
        if !self.amp.is_active() {
            self.active = false;
        }
        let raw = self.osc_a.next() * 0.6 + self.osc_b.next() * 0.4;
        let y = self.filt.process(raw) * a * self.vel * 0.5;
        // Bounded lifecycle: when the hold expires, gate off exactly once; the ADSR release then
        // brings the voice to Idle and frees the slot. No SFX can sustain indefinitely.
        self.remaining -= 1;
        if self.remaining <= 0 && !self.gated_off {
            self.amp.gate_off();
            self.gated_off = true;
        }
        pan(y, self.pan)
    }
}

/// Two frequencies for an SFX gesture, drawn from the world scale so a sting belongs to the
/// score rather than sounding like a foreign notification.
fn sfx_freqs(kind: SfxKind, scale: &Scale) -> [f32; 2] {
    let d = |deg: i32, oct: i32| midi_to_hz(scale.degree_pitch(deg, oct));
    match kind {
        SfxKind::Acquire => [d(0, 6), d(4, 5)],
        SfxKind::Confirm => [d(4, 5), d(0, 6)], // rising to the tonic
        SfxKind::Warning => [d(0, 4), midi_to_hz(scale.degree_pitch(0, 4) + 6)], // tritone
        SfxKind::Danger => [d(0, 2), d(1, 2)],
        SfxKind::Transition => [d(2, 4), d(4, 4)],
        SfxKind::Impact => [midi_to_hz(scale.degree_pitch(0, 1)), d(0, 2)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // P0 REGRESSION (written to FAIL on the pre-fix code): a triggered SFX voice must reach a
    // bounded end. Pre-fix, SfxVoice::trigger() gates the ADSR on and NOTHING ever gates it off,
    // so the envelope parks in Stage::Sustain forever (env.rs) — a Transition (sustain 0.3) rings
    // to the end of the render (the maintainer's "background voice gets stuck on a chord and
    // hangs"). Three seconds is far past any legitimate SFX lifetime.
    #[test]
    fn repro_sustained_sfx_hangs_forever() {
        let sr = 48_000.0f32;
        let mut v = SfxVoice::new(sr);
        v.trigger(SfxKind::Transition, [440.0, 660.0], 1.0);
        for _ in 0..(sr as usize * 3) {
            v.next();
        }
        assert!(
            !v.active(),
            "Transition SFX voice still active after 3s — it hangs forever"
        );
    }

    #[test]
    fn every_sfx_kind_dies_within_its_declared_lifetime() {
        // No SFX may outlive its declared bound. Render each kind for exactly its
        // max_lifetime_secs and assert the voice is (a) inactive and (b) silent by then — this is
        // what makes the Warning *tritone* and the Transition tone SHORT gestures rather than
        // hanging chords. (The reverb bus may still be decaying a finite tail afterward; that is a
        // shared effect, not this oscillator — the point is the oscillator has stopped feeding it.)
        let sr = 48_000.0f32;
        for kind in SfxKind::ALL {
            let mut v = SfxVoice::new(sr);
            v.trigger(kind, [440.0, 660.0], 1.0);
            let bound = (kind.max_lifetime_secs() * sr).ceil() as usize;
            let tail_start = bound - bound / 10; // last 10% is well past the release
            let mut tail_peak = 0.0f32;
            for i in 0..bound {
                let (l, r) = v.next();
                if i >= tail_start {
                    tail_peak = tail_peak.max(l.abs()).max(r.abs());
                }
            }
            assert!(
                !v.active(),
                "{kind:?}: still active at its declared max lifetime ({:.2}s) — SFX must be bounded",
                kind.max_lifetime_secs()
            );
            assert!(
                tail_peak < 1e-3,
                "{kind:?}: still audible (peak {tail_peak}) at the end of its declared lifetime"
            );
        }
    }

    #[test]
    fn four_sfx_voices_service_ten_sequential_events() {
        // The pool is four voices. Ten SFX arriving 0.5s apart (longer than any kind's ~1.5s
        // lifetime lets more than three overlap) must ALL find a free voice: pre-fix, the first
        // four jammed active forever and events 5..10 were silently dropped. After the last event
        // drains, the pool must be fully free again.
        let sr = 48_000.0f32;
        let mut pool: Vec<SfxVoice> = (0..4).map(|_| SfxVoice::new(sr)).collect();
        let gap = (sr * 0.5) as usize;
        let mut serviced = 0;
        for i in 0..10usize {
            if let Some(v) = pool.iter_mut().find(|v| !v.active()) {
                v.trigger(SfxKind::ALL[i % SfxKind::ALL.len()], [440.0, 660.0], 1.0);
                serviced += 1;
            }
            for _ in 0..gap {
                for v in pool.iter_mut() {
                    if v.active() {
                        v.next();
                    }
                }
            }
        }
        assert_eq!(
            serviced, 10,
            "pool exhausted: only {serviced}/10 SFX events found a free voice"
        );
        // Drain past the longest possible release, then the pool must be completely idle.
        for _ in 0..(sr as usize * 2) {
            for v in pool.iter_mut() {
                if v.active() {
                    v.next();
                }
            }
        }
        assert_eq!(
            pool.iter().filter(|v| v.active()).count(),
            0,
            "an SFX voice is still active after draining — a voice leaked/hung"
        );
    }

    #[test]
    fn tempo_comes_from_the_score_not_the_world() {
        // A score clocked at 200 bpm, rendered against an 88 bpm world, must run at the
        // SCORE's tempo. At 200 bpm, 16 beats is ~230k samples plus a ~2.5s tail (~350k);
        // at the world's 88 bpm it would be ~523k + tail. The gap is unambiguous.
        let score = Score::new(200.0, 4.0, 16.0);
        let world = MusicWorld::black_ice(); // 88 bpm
        let synth = HumanMusicSynth::new(&score, &world, SampleRate::STUDIO);
        assert!(
            synth.total_samples() < 450_000,
            "total_samples {} implies the world tempo (88), not the score tempo (200)",
            synth.total_samples()
        );
    }

    #[test]
    fn bus_levels_are_nonzero_for_every_audible_bus_in_a_real_bounce() {
        // Round VI: the mix ruler must actually read the room. Render the flagship DeflectedLift
        // bounce and confirm every voice family that plays reports a level — a silent bus here
        // means a voice the arrangement scheduled never reached the meter.
        use super::super::functor::compose;
        use super::super::semantic::deflected_lift_trace;
        use crate::audio::render::OfflineRenderer;

        let world = MusicWorld::black_ice();
        let score = compose(&deflected_lift_trace(120.0), &world, 2112);
        let sr = SampleRate::STUDIO;
        let mut synth = HumanMusicSynth::new(&score, &world, sr);
        let frames = synth.total_samples();
        let _ = OfflineRenderer::new(sr, 512).render(&mut synth, frames);

        let bl = synth.bus_levels();
        for (name, lvl) in [
            ("pad", bl.pad),
            ("keys", bl.keys),
            ("bass", bl.bass),
            ("lead", bl.lead),
            ("drums", bl.drums),
        ] {
            assert!(
                lvl.rms > 0.0,
                "{name} bus reports zero rms in a real bounce render"
            );
            assert!(
                lvl.peak >= lvl.rms,
                "{name} bus peak {} is below its rms {} — meter is inconsistent",
                lvl.peak,
                lvl.rms
            );
        }
    }

    #[test]
    fn bus_meter_is_independent_of_the_stem_mask() {
        // The load-bearing property of the meter: muting a bus stops its AUDIO, never its METER.
        // A ruler the mask could silence would lie precisely when you solo a bus to read it. So a
        // fully-silent render must report the exact same per-bus levels as the full mix.
        use super::super::functor::compose;
        use super::super::semantic::deflected_lift_trace;
        use crate::audio::render::OfflineRenderer;

        let world = MusicWorld::black_ice();
        let score = compose(&deflected_lift_trace(120.0), &world, 2112);
        let sr = SampleRate::STUDIO;
        let render = |mask: StemMask| {
            let mut s = HumanMusicSynth::new(&score, &world, sr);
            s.set_stem_mask(mask);
            let frames = s.total_samples();
            let _ = OfflineRenderer::new(sr, 512).render(&mut s, frames);
            s.bus_levels()
        };

        let full = render(StemMask::full());
        let muted = render(StemMask::silent());
        assert_eq!(full, muted, "the stem mask leaked into the bus meter");
        assert!(
            muted.keys.rms > 0.0,
            "keys meter was zeroed by the mute gate"
        );
        assert!(
            muted.lead.rms > 0.0,
            "lead meter was zeroed by the mute gate"
        );
    }
}
