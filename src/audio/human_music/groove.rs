//! The groove engine: an interlocking percussion pattern with a meter, an accent
//! hierarchy, swing, deterministic microtiming/humanization, ghost notes, syncopation and
//! phrase-end fills. It also exports where the kick lands so the bass engine can lock to
//! it (kick/bass interlock).
//!
//! Density and energy come from the form, so the pattern thins in the intro and thickens
//! into the climax without a new pattern being invented per bar.

use super::discourse::DiscourseRole;
use super::form::BEATS_PER_BAR;
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

    /// Generate the percussion track from the composition plan's [`super::plan::PhraseTarget`]s.
    ///
    /// A base groove CELL — kick on 1 & 3, snare backbeat on 2 & 4, subdivided hats — is realized
    /// every bar; the variations (extra kick, ghosts, open-hat lift, fill) are a bounded, repeating
    /// function of a 2-bar cell position and the phrase's energy/density targets, and a fill lands
    /// because a *phrase* is ending — not because a die rolled. Energy, density and the section all
    /// come from the phrase target holding each bar (the single authority), never a parallel form.
    /// Only the micro-timing humanization stays stochastic — a tasteful, seeded pocket.
    pub fn generate(&mut self, targets: &[super::plan::PhraseTarget]) -> Groove {
        let bpb = BEATS_PER_BAR;
        let mut hits = Vec::new();
        let mut kick_beats = Vec::new();
        if targets.is_empty() {
            return Groove { hits, kick_beats };
        }
        let total_bars = targets.last().map(|t| t.phrase.end_bar()).unwrap_or(0);

        for bar in 0..total_bars {
            let bar_start = bar as f64 * bpb;
            // One authority: energy/density and the section come from the phrase target holding
            // this bar — the plan decides a bar's job, not a parallel form curve.
            let Some(pt) = targets
                .iter()
                .find(|t| bar >= t.phrase.start_bar && bar < t.phrase.end_bar())
            else {
                continue;
            };
            let energy = pt.goal.energy_target;
            let density = pt.goal.density_target;
            let prov = Provenance::new(pt.phrase.family.to_section_kind());

            // Very low energy phrases may run drumless — silence is a valid event.
            if energy < 0.24 {
                continue;
            }

            // 2-bar cell: bar 0 is the plain statement, bar 1 carries the variation. A
            // recognizable groove that repeats with bounded mutation instead of re-rolling.
            let varied = bar % 2 == 1;
            // A fill lands because the next bar begins a new phrase.
            let is_fill_bar = targets.iter().any(|t| t.phrase.end_bar() == bar + 1) && energy > 0.4;
            // Discourse departure/re-entry: a Withhold/Question phrase strips the kit back to a
            // stark kick+snare backbone (no hats, ghosts or syncopated pushes), and the groove
            // re-enters at the Answer/Return — so the pocket has a story, not just a pattern.
            let stripped = matches!(
                pt.goal.role,
                DiscourseRole::Withhold | DiscourseRole::Question
            );

            // --- Kick: downbeat + beat 3; syncopations only on the cell's varied, un-stripped bar. ---
            self.emit_kick(&mut hits, &mut kick_beats, bar_start, 0.0, 0.95, prov);
            self.emit_kick(&mut hits, &mut kick_beats, bar_start, 2.0, 0.8, prov);
            if energy > 0.55 && varied && !stripped {
                // Push kick to the "and of 3" — a syncopated anticipation.
                self.emit_kick(&mut hits, &mut kick_beats, bar_start, 2.5, 0.6, prov);
            }
            if energy > 0.78 && varied && !stripped {
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
            //     with a real ghost character, at a fixed low velocity (dropped when stripped). ---
            if self.ghost_amount > 0.15 && energy > 0.45 && varied && !stripped {
                for &b in &[1.75f64, 3.5] {
                    let v = (0.22 * self.dyn_scale(energy)).min(0.35);
                    hits.push(self.hit(DrumVoice::Snare, bar_start + self.swung(b), v, prov));
                }
            }

            // --- Hats: subdivisions with accent hierarchy + swing, thinned DETERMINISTICALLY
            //     at low density (drop the same weak off-16ths every bar, not random ones). A
            //     stripped (Withhold/Question) bar drops the hats entirely — the departure. ---
            if !stripped {
                let steps = (self.subdiv as f64 * bpb) as u32; // subdivisions per bar
                for s in 0..steps {
                    let frac = s as f64 / self.subdiv as f64; // beat position within the bar
                    let on_beat = (frac.fract()).abs() < 1e-6;
                    if !on_beat && density < 0.5 && s % 2 == 1 {
                        continue; // thin the "e"/"a" off-subdivisions, consistently
                    }
                    let accent = if on_beat { 0.7 } else { 0.42 };
                    let v = accent * self.dyn_scale(energy);
                    let open = !on_beat && (frac - (bpb - 0.5)).abs() < 1e-6 && varied; // "& of 4"
                    let voice = if open {
                        DrumVoice::OpenHat
                    } else {
                        DrumVoice::ClosedHat
                    };
                    hits.push(self.hit(voice, bar_start + self.swung(frac), v, prov));
                }
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

/// Realize the drums as one projection of the [`super::performance::PerformancePlan`].
///
/// The pocket keeps its identity (kick on the downbeat, the backbeat on 2 and 4, subdivided hats
/// with an accent contour) but everything around it is read from the shared plan: the kick
/// interlocks with the bass's own syncopations (it doubles some of them and leaves the rest),
/// ghost notes fall on the grid's weaker off-beats, open hats mark pickups and pushes, the planned
/// ensemble hits land with everyone, planned breaks are left empty, fills fire because a Reset is
/// handing over to the next attempt (an action), half-time/double-time surfaces follow Pullback /
/// Accelerate, and when the drummer is the chosen responder it echoes the call's rhythm.
pub fn realize_drums(
    perf: &super::performance::PerformancePlan,
    plan: &super::plan::CompositionPlan,
    world: &MusicWorld,
    seed: u64,
    bass: &[super::score::Note],
    lead: &[super::score::Note],
) -> Vec<DrumHit> {
    use super::action::{ActionKind, Agent};
    use super::ids::{ActionId, ActionStamp};
    use super::performance::{AccentGrid, DrumsMode, STEPS, STEP_BEATS};
    let mut rng = Rng::new(seed ^ 0x6300_0E00_0000_0001);
    let subdiv_steps = match perf.language.surface_subdivision.max(world.subdiv) {
        s if s >= 4 => 1usize,
        2 | 3 => 2,
        _ => 4,
    };
    let use_clap = matches!(world.id, super::world::WorldId::Vapor95);
    let dyn_scale = |energy: f32| {
        (world.base_dynamic * (0.55 + 0.45 * energy) * (0.6 + 0.4 * world.drum_density))
            .clamp(0.0, 1.0)
    };
    let swung = |frac: f64| -> f64 {
        if world.swing <= 0.0 {
            return frac;
        }
        let idx = (frac / 0.5).round() as i64;
        if (frac * 2.0 - idx as f64).abs() < 1e-6 && idx % 2 != 0 {
            frac + world.swing as f64 * 0.25
        } else {
            frac
        }
    };
    let mut hits: Vec<DrumHit> = Vec::new();
    // Every stroke carries the exact actions it realizes; the level comes from the stage (the
    // single orchestration authority), so nothing has to be deleted or rescaled afterwards.
    let hit = |hits: &mut Vec<DrumHit>,
               voice: DrumVoice,
               at: f64,
               vel: f32,
               tag: &'static str,
               stamp: ActionStamp,
               rng: &mut Rng| {
        if !perf.on_stage(Agent::Drums, at) {
            return;
        }
        let level = perf.level(Agent::Drums, at);
        hits.push(DrumHit {
            start_beat: (at + rng.range_f32(-0.008, 0.008) as f64).max(0.0),
            voice,
            velocity: (vel * level).clamp(0.02, 1.0),
            prov: super::comp::stamped(
                Provenance {
                    groove_variation: Some(tag),
                    ..Provenance::new(super::form::SectionKind::A)
                },
                stamp,
            ),
        });
    };
    let bass_steps = |bar: u32| -> Vec<usize> {
        let bs = AccentGrid::beat_of(bar, 0);
        bass.iter()
            .filter(|n| n.start_beat >= bs - 1e-6 && n.start_beat < bs + 4.0 - 1e-6)
            .map(|n| AccentGrid::step_of(n.start_beat).1)
            .collect()
    };
    // The drummer's own figures (a drum-led fill is a rhythm-only call the band can answer).
    let drum_figures: Vec<(ActionId, ActionKind, Vec<f64>, Vec<f32>)> = perf
        .figures_for(Agent::Drums)
        .filter_map(|m| match m.source {
            super::material::MaterialSource::Figure { action, kind } => Some((
                action,
                kind,
                m.events.iter().map(|e| m.start_beat + e.onset).collect(),
                m.events.iter().map(|e| e.accent).collect(),
            )),
            _ => None,
        })
        .collect();
    let in_drum_figure = |b: f64| {
        drum_figures.iter().any(|(_, k, on, _)| {
            *k == ActionKind::Fill
                && on.first().is_some_and(|&f| b >= f - 1e-6)
                && on.last().is_some_and(|&l| b < l + 0.5)
        })
    };

    for eb in &perf.ensemble {
        let bar = eb.bar;
        let bs = AccentGrid::beat_of(bar, 0);
        // The stage decides: a bar the drums sit out is not played (and later deleted).
        if !perf.on_stage(Agent::Drums, bs) {
            continue;
        }
        let pt = plan.form.phrase_at(bs);
        let energy = plan
            .discourse
            .goal(pt.ix as usize)
            .energy_target
            .max(0.3 * eb.kinetic);
        if energy < 0.24 && eb.drums != DrumsMode::Fill {
            continue; // very low energy may run drumless — silence is an event
        }
        let d = dyn_scale(energy.max(eb.kinetic * 0.8));
        let in_hole = |s: usize| perf.accent.at(bar, s).hole >= 0.5;
        let bsteps = bass_steps(bar);
        // The bar's surface verbs (a half-time pullback, a double-time acceleration) are realized
        // by every stroke of the bar: stamp them.
        let surface = perf
            .actions_covering(
                &[ActionKind::Pullback, ActionKind::Accelerate],
                bs + 0.5,
                Some(Agent::Drums),
            )
            .fold(ActionStamp::NONE, ActionStamp::with);
        // The shared budget: the kit's free allowance this bar. The pocket (kick, backbeat,
        // accents) is the floor; ghosts spend what is left, and a tight bar thins the hats.
        let free = perf.free_allowance(Agent::Drums, bar);
        let mut spent = 0.0f32;
        let tight = free < 4.0;

        for s in 0..STEPS {
            let w = perf.accent.at(bar, s);
            let at = AccentGrid::beat_of(bar, s);
            if in_hole(s) && w.hit < 0.9 {
                continue;
            }
            let in_fill = in_drum_figure(at);
            let accents0 = perf
                .actions_starting(
                    &[ActionKind::Push, ActionKind::Hit],
                    at,
                    STEP_BEATS * 0.5,
                    None,
                )
                .fold(ActionStamp::NONE, ActionStamp::with);
            // How hard the planned accent on this step lands: its semantic size (Round VIIb).
            let force = perf.force_of(accents0).unwrap_or(0.5);
            let accents = accents0.iter().fold(surface, ActionStamp::with);
            // --- Kick ---
            let kick = match eb.drums {
                DrumsMode::HalfTime => s == 0 || (s == 10 && eb.kinetic > 0.5),
                DrumsMode::Break => w.hit >= 0.9,
                _ => {
                    s == 0
                        || (s == 8 && !bsteps.iter().any(|&b| b == 6 || b == 10))
                        || w.push >= 0.9
                        || w.hit >= 0.9
                        // Interlock: double the bass on its strong eighth syncopations, alternate bars.
                        || (s % 2 == 0 && s % 4 != 0 && bsteps.contains(&s) && (bar + s as u32 / 2) % 2 == 0)
                }
            };
            if kick {
                hit(
                    &mut hits,
                    DrumVoice::Kick,
                    at,
                    (0.95 - 0.1 * (s != 0) as u8 as f32)
                        * world.base_dynamic
                        * if accents0.is_empty() {
                            1.0
                        } else {
                            0.85 + 0.3 * force
                        },
                    "kick",
                    accents,
                    &mut rng,
                );
            }
            // --- Snare / backbeat ---
            let back = match eb.drums {
                DrumsMode::HalfTime => s == 8,
                DrumsMode::Break => false,
                _ => w.backbeat >= 0.9,
            };
            if back && !in_fill {
                hit(
                    &mut hits,
                    DrumVoice::Snare,
                    at,
                    0.85 * d,
                    "backbeat",
                    surface,
                    &mut rng,
                );
                if use_clap {
                    hit(
                        &mut hits,
                        DrumVoice::Clap,
                        at,
                        0.6 * d,
                        "backbeat",
                        surface,
                        &mut rng,
                    );
                }
            }
            if w.hit >= 0.9 && !back {
                hit(
                    &mut hits,
                    DrumVoice::Snare,
                    at,
                    (0.6 + 0.4 * force) * d,
                    "hit",
                    accents,
                    &mut rng,
                );
                // A big, compact arrival gets the whole kit: a clap on top.
                if force > 0.7 {
                    hit(
                        &mut hits,
                        DrumVoice::Clap,
                        at,
                        (0.5 + 0.3 * force) * d,
                        "hit",
                        accents,
                        &mut rng,
                    );
                }
            }
            // Ghosts on the weaker off-beats of the grid.
            if world.ghost_amount > 0.15
                && !back
                && !in_fill
                && eb.drums != DrumsMode::Break
                && s % 2 == 1
                && (0.12..0.5).contains(&w.syncopation)
                && rng.chance(0.35 + 0.4 * eb.kinetic)
                && spent + 0.2 <= free
            {
                spent += 0.2;
                hit(
                    &mut hits,
                    DrumVoice::Snare,
                    at,
                    (0.22 * d).min(0.35),
                    "ghost",
                    surface,
                    &mut rng,
                );
            }

            // --- Hats ---
            let hat_every = match eb.drums {
                DrumsMode::HalfTime => 4,
                DrumsMode::DoubleTime => 1,
                DrumsMode::Break => 99,
                _ if tight => subdiv_steps.max(2),
                _ => subdiv_steps,
            };
            if kick {
                spent += 0.4;
            }
            if back {
                spent += 0.4;
            }
            if s % hat_every == 0 && !in_fill {
                let open =
                    (w.pickup >= 0.8 && s % 2 == 0 && s >= 12) || w.push >= 0.9 || w.hit >= 0.9;
                let contour = if s % 4 == 0 {
                    0.7
                } else if s % 2 == 0 {
                    0.5
                } else {
                    0.34
                };
                let acc = contour + 0.2 * w.syncopation;
                let voice = if open {
                    DrumVoice::OpenHat
                } else {
                    DrumVoice::ClosedHat
                };
                let frac = s as f64 * STEP_BEATS;
                let st = if open { accents } else { surface };
                hit(
                    &mut hits,
                    voice,
                    bs + swung(frac),
                    acc * d,
                    "hat",
                    st,
                    &mut rng,
                );
            }
        }
    }
    // The drummer's figures — fills (an action, not a coin) and drum-led pickups — from their
    // plan material: a rhythm the band can hear and answer on the landing.
    for (id, kind, onsets, accents) in &drum_figures {
        let base = match kind {
            ActionKind::Fill => 0.4,
            _ => 0.5,
        };
        let mut landed: Vec<f64> = Vec::new();
        for (k, (&at0, &acc)) in onsets.iter().zip(accents).enumerate() {
            // The stroke also performs any same-player figure verb it covers (a drum pickup
            // inside the drum fill), and lands — once — on a planned push/hit step.
            let (at, accent_ids) = super::comp::land_once(perf, at0, &mut landed);
            let stamp = perf
                .actions_covering(
                    &[
                        ActionKind::Pickup,
                        ActionKind::Fill,
                        ActionKind::Fragment,
                        ActionKind::ReEntry,
                    ],
                    at,
                    Some(Agent::Drums),
                )
                .fold(accent_ids.with(*id), ActionStamp::with);
            let ph = plan.form.phrase_at(at);
            let energy = plan.discourse.goal(ph.ix as usize).energy_target.max(0.3);
            let d = dyn_scale(energy);
            hit(
                &mut hits,
                if *kind == ActionKind::Fill && k + 1 == onsets.len() && onsets.len() > 3 {
                    DrumVoice::Kick
                } else {
                    DrumVoice::Snare
                },
                at,
                (base + 0.5 * acc) * d,
                "fill",
                stamp,
                &mut rng,
            );
        }
    }
    // The ensemble unison: the kit accents the figure's onsets with everyone.
    for (id, line) in super::comp::unison_lines(perf, lead) {
        for (i, &(at, _, _, _)) in line.iter().enumerate() {
            let v = if i == 0 { 0.8 } else { 0.6 } * world.base_dynamic;
            let st = ActionStamp::of(id);
            hit(&mut hits, DrumVoice::Snare, at, v, "unison", st, &mut rng);
            if i == 0 || i + 1 == line.len() {
                hit(&mut hits, DrumVoice::Kick, at, v, "unison", st, &mut rng);
            }
        }
    }
    // The drummer's answers: the RESPONSE's material (the call's rhythm, derived in the plan —
    // whoever called), on snare, briefly.
    for (call, r) in perf.responses_for(Agent::Drums) {
        let Some(mid) = r.material else {
            continue;
        };
        let m = perf.material(mid);
        let interaction = perf
            .interactions
            .iter()
            .find(|i| i.call.action == call.action)
            .map(|i| i.id);
        for e in &m.events {
            let at = r.start_beat + e.onset;
            if at >= r.start_beat + r.dur_beats - 1e-6 {
                break;
            }
            let before = hits.len();
            hit(
                &mut hits,
                DrumVoice::Snare,
                at,
                (0.45 + 0.15 * e.accent) * world.base_dynamic,
                "answer",
                ActionStamp::NONE.with_opt(r.action),
                &mut rng,
            );
            for h in &mut hits[before..] {
                h.prov.interaction = interaction;
                h.prov.material = Some(mid);
            }
        }
    }
    hits.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    hits
}

#[cfg(test)]
mod tests {
    use super::super::plan::{CompositionPlan, PhraseTarget};
    use super::super::semantic::demo_trace;
    use super::super::timeline::IntentTimeline;
    use super::*;

    fn demo_targets() -> Vec<PhraseTarget> {
        let tl = IntentTimeline::walk(&demo_trace(120.0));
        CompositionPlan::build(&tl, 30).targets()
    }

    #[test]
    fn groove_interlocks_kick_and_backbeat() {
        let mut g = GrooveEngine::new(&MusicWorld::black_ice(), 3);
        let gr = g.generate(&demo_targets());
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
        let targets = demo_targets();
        let mut a = GrooveEngine::new(&MusicWorld::vapor95(), 5);
        let mut b = GrooveEngine::new(&MusicWorld::vapor95(), 5);
        let ga = a.generate(&targets);
        let gb = b.generate(&targets);
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
        // More phrase boundaries -> more fills. Six 4-bar phrases vs one 24-bar phrase, same
        // constant energy so only the number of phrase ends differs (fills follow structure).
        let many: Vec<PhraseTarget> = (0..6)
            .map(|i| PhraseTarget::test_flat(i * 4, 4, 0.6, 0.6))
            .collect();
        let one = [PhraseTarget::test_flat(0, 24, 0.6, 0.6)];
        let snares = |g: &Groove| {
            g.hits
                .iter()
                .filter(|h| h.voice == DrumVoice::Snare)
                .count()
        };
        let g_many = GrooveEngine::new(&MusicWorld::black_ice(), 9).generate(&many);
        let g_one = GrooveEngine::new(&MusicWorld::black_ice(), 9).generate(&one);
        assert!(
            snares(&g_many) > snares(&g_one),
            "phrase-end fills added no snares: {} vs {}",
            snares(&g_many),
            snares(&g_one)
        );
        // Deterministic given the same targets and seed.
        let g_again = GrooveEngine::new(&MusicWorld::black_ice(), 9).generate(&many);
        assert_eq!(g_many.hits.len(), g_again.hits.len());
    }
}
