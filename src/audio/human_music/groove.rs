//! The groove engine: an interlocking percussion pattern with a meter, an accent
//! hierarchy, swing, deterministic microtiming/humanization, ghost notes, syncopation and
//! phrase-end fills. It also exports where the kick lands so the bass engine can lock to
//! it (kick/bass interlock).
//!
//! Density and energy come from the form, so the pattern thins in the intro and thickens
//! into the climax without a new pattern being invented per bar.

use super::form::{Form, BEATS_PER_BAR};
use super::rng::Rng;
use super::score::{DrumHit, DrumVoice, Provenance};
use super::world::MusicWorld;

/// Percussion generator for a world.
pub struct GrooveEngine {
    swing: f32,
    subdiv: u32,
    ghost_amount: f32,
    drum_density: f32,
    base_dynamic: f32,
    use_clap: bool,
    rng: Rng,
}

/// The generated groove: hits plus the kick onset beats (for bass interlock).
pub struct Groove {
    pub hits: Vec<DrumHit>,
    pub kick_beats: Vec<f64>,
}

impl GrooveEngine {
    /// A groove engine for `world`, seeded by `seed`.
    pub fn new(world: &MusicWorld, seed: u64) -> GrooveEngine {
        GrooveEngine {
            swing: world.swing,
            subdiv: world.subdiv.clamp(1, 4),
            ghost_amount: world.ghost_amount,
            drum_density: world.drum_density,
            base_dynamic: world.base_dynamic,
            use_clap: matches!(world.id, super::world::WorldId::Vapor95),
            rng: Rng::new(seed ^ 0x6300_0E00_0000_0001),
        }
    }

    /// Generate the percussion track over `form`.
    ///
    /// A base groove CELL — kick on 1 & 3, snare backbeat on 2 & 4, subdivided hats — is
    /// realized every bar. Round I decided each variation (extra kick, ghosts, open-hat lift,
    /// fill) with an INDEPENDENT per-bar coin flip, so nothing repeated and the boundary was
    /// unmarked 30% of the time. Round II makes the variations a bounded, repeating function
    /// of a 2-bar cell position and the energy curve: the groove is recognizable and mutates
    /// within bounds, and a fill lands because a *phrase* is ending (`phrase_end_bars`), not
    /// because a die rolled. Only the micro-timing humanization stays stochastic — a tasteful,
    /// seeded pocket, not structural noise.
    pub fn generate(&mut self, form: &Form, phrase_end_bars: &[u32]) -> Groove {
        let bpb = BEATS_PER_BAR;
        let mut hits = Vec::new();
        let mut kick_beats = Vec::new();

        for bar in 0..form.total_bars {
            let bar_start = bar as f64 * bpb;
            let sec = *form.section_at_bar(bar);
            let energy = form.energy_at(bar as f64 + 0.5);
            let density = form.density_at(bar as f64 + 0.5);
            let prov = Provenance::new(sec.kind);

            // The intro (very low energy) may run drumless — silence is a valid event.
            if energy < 0.24 {
                continue;
            }

            // 2-bar cell: bar 0 is the plain statement, bar 1 carries the variation. A
            // recognizable groove that repeats with bounded mutation instead of re-rolling.
            let varied = bar % 2 == 1;
            let is_fill_bar = phrase_end_bars.contains(&(bar + 1)) && energy > 0.4;

            // --- Kick: downbeat + beat 3; syncopations only on the cell's varied bar. ---
            self.emit_kick(&mut hits, &mut kick_beats, bar_start, 0.0, 0.95, prov);
            self.emit_kick(&mut hits, &mut kick_beats, bar_start, 2.0, 0.8, prov);
            if energy > 0.55 && varied {
                // Push kick to the "and of 3" — a syncopated anticipation.
                self.emit_kick(&mut hits, &mut kick_beats, bar_start, 2.5, 0.6, prov);
            }
            if energy > 0.78 && varied {
                self.emit_kick(&mut hits, &mut kick_beats, bar_start, 3.75, 0.55, prov);
            }

            // --- Backbeat: snare (and clap layer in some worlds) on 1 and 3. ---
            for &b in &[1.0f64, 3.0] {
                let v = 0.85 * self.dyn_scale(energy);
                hits.push(self.hit(DrumVoice::Snare, bar_start + b, v, prov));
                if self.use_clap {
                    hits.push(self.hit(DrumVoice::Clap, bar_start + b, v * 0.7, prov));
                }
            }

            // --- Ghost snares between backbeats (pocket) — placed on the varied bar in worlds
            //     with a real ghost character, at a fixed low velocity. ---
            if self.ghost_amount > 0.15 && energy > 0.45 && varied {
                for &b in &[1.75f64, 3.5] {
                    let v = (0.22 * self.dyn_scale(energy)).min(0.35);
                    hits.push(self.hit(DrumVoice::Snare, bar_start + self.swung(b), v, prov));
                }
            }

            // --- Hats: subdivisions with accent hierarchy + swing, thinned DETERMINISTICALLY
            //     at low density (drop the same weak off-16ths every bar, not random ones). ---
            let steps = (self.subdiv as f64 * bpb) as u32; // subdivisions per bar
            for s in 0..steps {
                let frac = s as f64 / self.subdiv as f64; // beat position within the bar
                let on_beat = (frac.fract()).abs() < 1e-6;
                if !on_beat && density < 0.5 && s % 2 == 1 {
                    continue; // thin the "e"/"a" off-subdivisions, consistently
                }
                let accent = if on_beat { 0.7 } else { 0.42 };
                let v = accent * self.dyn_scale(energy);
                let open = !on_beat && (frac - (bpb - 0.5)).abs() < 1e-6 && varied; // "& of 4" lift
                let voice = if open {
                    DrumVoice::OpenHat
                } else {
                    DrumVoice::ClosedHat
                };
                hits.push(self.hit(voice, bar_start + self.swung(frac), v, prov));
            }

            // --- Phrase-end fill: extra snares on the last half-bar's 16ths, because the
            //     phrase is ending — not because a 70% coin fired. ---
            if is_fill_bar {
                for k in 0..4 {
                    let b = 2.0 + k as f64 * 0.5;
                    let v = (0.4 + 0.12 * k as f32) * self.dyn_scale(energy);
                    hits.push(self.hit(DrumVoice::Snare, bar_start + b, v, prov));
                }
            }
        }

        // Sort by time for deterministic, in-order playback.
        hits.sort_by(|a, b| a.start_beat.partial_cmp(&b.start_beat).unwrap());
        kick_beats.sort_by(|a, b| a.partial_cmp(b).unwrap());
        Groove { hits, kick_beats }
    }

    fn dyn_scale(&self, energy: f32) -> f32 {
        (self.base_dynamic * (0.55 + 0.45 * energy) * (0.6 + 0.4 * self.drum_density))
            .clamp(0.0, 1.0)
    }

    /// Apply swing to an off-subdivision position (delays every odd subdivision — the
    /// "and" of an 8th feel, or the e/a of a 16th feel).
    fn swung(&self, frac: f64) -> f64 {
        if self.swing <= 0.0 {
            return frac;
        }
        let sub = 1.0 / self.subdiv as f64;
        let idx = (frac / sub).round() as i64;
        if idx % 2 != 0 {
            frac + self.swing as f64 * sub * 0.5
        } else {
            frac
        }
    }

    fn emit_kick(
        &mut self,
        hits: &mut Vec<DrumHit>,
        kicks: &mut Vec<f64>,
        bar_start: f64,
        b: f64,
        vel: f32,
        prov: Provenance,
    ) {
        let t = bar_start + b + self.micro();
        kicks.push(t);
        hits.push(DrumHit {
            start_beat: t,
            voice: DrumVoice::Kick,
            velocity: (vel * self.base_dynamic).clamp(0.05, 1.0),
            prov,
        });
    }

    fn hit(&mut self, voice: DrumVoice, at: f64, vel: f32, prov: Provenance) -> DrumHit {
        DrumHit {
            start_beat: at + self.micro(),
            voice,
            velocity: vel.clamp(0.02, 1.0),
            prov,
        }
    }

    /// Deterministic humanization: ±~6 ms of timing jitter in beats (tasteful, not drunk).
    fn micro(&mut self) -> f64 {
        self.rng.range_f32(-0.008, 0.008) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::super::semantic::demo_trace;
    use super::*;

    #[test]
    fn groove_interlocks_kick_and_backbeat() {
        let form = Form::from_trace(&demo_trace(120.0));
        let mut g = GrooveEngine::new(&MusicWorld::black_ice(), 3);
        let gr = g.generate(&form, &[]);
        assert!(!gr.hits.is_empty());
        assert!(!gr.kick_beats.is_empty());
        // Kicks recorded match kick hits.
        let kick_hits = gr
            .hits
            .iter()
            .filter(|h| h.voice == DrumVoice::Kick)
            .count();
        assert_eq!(kick_hits, gr.kick_beats.len());
        // There are snares on backbeats and hats subdividing.
        assert!(gr.hits.iter().any(|h| h.voice == DrumVoice::Snare));
        assert!(gr.hits.iter().any(|h| h.voice == DrumVoice::ClosedHat));
        // Sorted in time.
        for w in gr.hits.windows(2) {
            assert!(w[0].start_beat <= w[1].start_beat + 1e-9);
        }
    }

    #[test]
    fn velocities_are_bounded_and_deterministic() {
        let form = Form::from_trace(&demo_trace(120.0));
        let mut a = GrooveEngine::new(&MusicWorld::vapor95(), 5);
        let mut b = GrooveEngine::new(&MusicWorld::vapor95(), 5);
        let ga = a.generate(&form, &[]);
        let gb = b.generate(&form, &[]);
        assert_eq!(ga.hits.len(), gb.hits.len());
        for h in &ga.hits {
            assert!((0.0..=1.0).contains(&h.velocity));
        }
        for (x, y) in ga.hits.iter().zip(gb.hits.iter()) {
            assert!((x.start_beat - y.start_beat).abs() < 1e-9);
            assert_eq!(x.voice, y.voice);
        }
    }

    #[test]
    fn swing_delays_offbeats_in_vapor_not_black_ice() {
        let straight = GrooveEngine::new(&MusicWorld::black_ice(), 1);
        let swung = GrooveEngine::new(&MusicWorld::vapor95(), 1);
        // Black Ice is straight (swing 0): an offbeat position is unchanged.
        assert!((straight.swung(0.5) - 0.5).abs() < 1e-9);
        // Vapor swings: the "and" is pushed later.
        assert!(swung.swung(0.5) > 0.5);
    }

    #[test]
    fn fills_land_at_phrase_ends_not_by_coin_flip() {
        let form = Form::from_trace(&demo_trace(120.0));
        let all_ends: Vec<u32> = (1..=form.total_bars).collect();
        let g_with = GrooveEngine::new(&MusicWorld::black_ice(), 9).generate(&form, &all_ends);
        let g_without = GrooveEngine::new(&MusicWorld::black_ice(), 9).generate(&form, &[]);
        let snares = |g: &Groove| {
            g.hits
                .iter()
                .filter(|h| h.voice == DrumVoice::Snare)
                .count()
        };
        // Marking phrase ends adds fill snares; with none marked there are strictly fewer.
        // (Structural: fills follow the form, not a die.)
        assert!(
            snares(&g_with) > snares(&g_without),
            "phrase-end fills added no snares: {} vs {}",
            snares(&g_with),
            snares(&g_without)
        );
        // Deterministic given the same seed and phrase ends.
        let g_again = GrooveEngine::new(&MusicWorld::black_ice(), 9).generate(&form, &all_ends);
        assert_eq!(g_with.hits.len(), g_again.hits.len());
    }
}
