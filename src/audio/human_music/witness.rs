//! **Audible witnesses** — does each planned action actually happen in the notes, *because of the
//! action*?
//!
//! Round VII's audit was a set of temporal proxies: a Resolve passed if ANY chord tone sounded
//! near the resolution, a Hit if any two players happened to onset together, a Fragment if the
//! PLAN contained a fragmentary statement — so unrelated events in the same window could witness
//! an action by accident, and the audit only ever ran inside its own unit test.
//!
//! Round VIIb makes the receipt causal. Every realizer stamps the exact [`ActionId`]s an event
//! performs into its [`super::score::Provenance`]; the audit's PRIMARY evidence is the set of
//! events carrying the action's id, and only then is the event shape held to the action's musical
//! contract (a push by two stamped players off the target downbeat; a resolve arriving on the
//! intended harmony; a fragment that is actually fragmentary; an answer by the right responder
//! carrying the response's own material). Three kinds are not events but the ABSENCE of events
//! (a break, a thinning) or a change of the harmony itself (a tonicization, a recolouring); those
//! are checked by shape against the plan's own window and say so in [`WitnessClass`].
//!
//! Measurements, not a quality score.

use super::action::{ActionKind, Agent, MusicalAction};
use super::ids::ActionId;
use super::interaction::Transform;
use super::material::{relation, Heard};
use super::performance::{AccentGrid, PerformancePlan, STEPS};
use super::score::{DrumVoice, Provenance, Role, Score};
use super::theory::pitch_class;

/// Onset tolerance: within half a sixteenth.
const TOL: f64 = 0.125 + 1e-6;

/// What kind of evidence witnesses an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WitnessClass {
    /// Score events carrying the action's id, whose shape satisfies the action's contract.
    Stamped,
    /// The absence the action plans (a break, a thinning), checked against the preceding window.
    Absence,
    /// The harmony itself changed (an edit, a prepared miss) and a sounding note proves it.
    Harmonic,
}

/// The audit result for one action.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Witness {
    pub action: ActionId,
    pub kind: ActionKind,
    pub witnessed: bool,
    pub class: WitnessClass,
    /// How many Score events carry the action's id.
    pub stamped: usize,
    /// What the audit looked for.
    pub evidence: &'static str,
}

/// The whole audit.
#[derive(Debug, Clone, PartialEq)]
pub struct WitnessReport {
    pub rows: Vec<Witness>,
}

impl WitnessReport {
    /// How many actions were audited.
    pub fn total(&self) -> usize {
        self.rows.len()
    }
    /// How many the notes witnessed.
    pub fn witnessed(&self) -> usize {
        self.rows.iter().filter(|w| w.witnessed).count()
    }
    /// How many were witnessed by stamped events (the causal kind of receipt).
    pub fn witnessed_stamped(&self) -> usize {
        self.rows
            .iter()
            .filter(|w| w.witnessed && w.class == WitnessClass::Stamped)
            .count()
    }
    /// The unwitnessed actions.
    pub fn missing(&self) -> impl Iterator<Item = &Witness> {
        self.rows.iter().filter(|w| !w.witnessed)
    }
    /// A compact report: totals, the per-kind counts and every miss.
    pub fn report(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "causal witnesses (events stamped with the action's id, held to its contract — NOT a quality score):"
        );
        let by = |c: WitnessClass| self.rows.iter().filter(|w| w.class == c).count();
        let _ = writeln!(
            s,
            "  witnessed {}/{} actions  (stamped {}/{}, absence {}, harmonic {})",
            self.witnessed(),
            self.total(),
            self.witnessed_stamped(),
            by(WitnessClass::Stamped),
            by(WitnessClass::Absence),
            by(WitnessClass::Harmonic),
        );
        let mut kinds: Vec<ActionKind> = self.rows.iter().map(|w| w.kind).collect();
        kinds.sort();
        kinds.dedup();
        let mut line = String::from("  ");
        for k in kinds {
            let all = self.rows.iter().filter(|w| w.kind == k).count();
            let ok = self
                .rows
                .iter()
                .filter(|w| w.kind == k && w.witnessed)
                .count();
            let _ = write!(line, "{}={ok}/{all} ", k.label());
        }
        let _ = writeln!(s, "{}", line.trim_end());
        for w in self.missing() {
            let _ = writeln!(
                s,
                "  UNWITNESSED {} {} ({} stamped events) — expected {}",
                w.action,
                w.kind.label(),
                w.stamped,
                w.evidence
            );
        }
        s
    }
}

/// The player of a pitched role.
fn agent_of(role: Role) -> Agent {
    match role {
        Role::Lead => Agent::Lead,
        Role::Keys => Agent::Keys,
        Role::Bass => Agent::Bass,
        Role::Pad => Agent::Pad,
    }
}

/// One stamped event: who, when, pitch (None for a drum stroke), its provenance.
#[derive(Debug, Clone, Copy)]
struct Ev {
    agent: Agent,
    beat: f64,
    dur: f64,
    pitch: Option<i32>,
    prov: Provenance,
}

/// Every Score event, as audit events.
fn events(score: &Score) -> Vec<Ev> {
    let mut v: Vec<Ev> = score
        .notes
        .iter()
        .map(|n| Ev {
            agent: agent_of(n.role),
            beat: n.start_beat,
            dur: n.dur_beats as f64,
            pitch: Some(n.pitch),
            prov: n.prov,
        })
        .collect();
    v.extend(score.drums.iter().map(|d| Ev {
        agent: Agent::Drums,
        beat: d.start_beat,
        dur: 0.1,
        pitch: None,
        prov: d.prov,
    }));
    v
}

/// A performer's onsets (any provenance).
fn onsets(evs: &[Ev], agent: Agent) -> impl Iterator<Item = f64> + '_ {
    evs.iter().filter(move |e| e.agent == agent).map(|e| e.beat)
}

fn onsets_in(evs: &[Ev], agent: Agent, a: f64, b: f64) -> usize {
    onsets(evs, agent)
        .filter(|&t| t >= a - TOL && t < b - 1e-6)
        .count()
}

fn audit_one(
    perf: &PerformancePlan,
    evs: &[Ev],
    a: &MusicalAction,
) -> (bool, WitnessClass, usize, &'static str) {
    use WitnessClass::*;
    let (s, e) = (a.start_beat, a.end_beat());
    let mine: Vec<&Ev> = evs.iter().filter(|x| x.prov.actions.has(a.id)).collect();
    let n = mine.len();
    let by_initiator = |lo: f64, hi: f64| {
        mine.iter()
            .filter(|x| x.agent == a.initiator && x.beat >= lo - TOL && x.beat < hi - 1e-6)
            .count()
    };
    let players_at = |t: f64| {
        let mut ps: Vec<Agent> = mine
            .iter()
            .filter(|x| (x.beat - t).abs() <= TOL)
            .map(|x| x.agent)
            .collect();
        ps.sort();
        ps.dedup();
        ps.len()
    };
    match a.kind {
        ActionKind::Pickup => {
            let target = a.target_beat.unwrap_or(e);
            (
                by_initiator(s, target - TOL) > 0,
                Stamped,
                n,
                "a stamped onset by the initiator strictly before the target downbeat",
            )
        }
        ActionKind::Push => {
            let target = a.target_beat.unwrap_or(e);
            let need = if a.initiator == Agent::Ensemble { 2 } else { 1 };
            (
                players_at(s) >= need
                    && s < target - 0.2
                    && (need == 2 || by_initiator(s, s + TOL) > 0),
                Stamped,
                n,
                "stamped players on the push step (two for an ensemble push), ahead of the target",
            )
        }
        ActionKind::Hit => {
            let need = if a.initiator == Agent::Ensemble { 2 } else { 1 };
            (
                players_at(s) >= need && (need == 2 || by_initiator(s, s + TOL) > 0),
                Stamped,
                n,
                "stamped players striking the hit together (two for an ensemble hit)",
            )
        }
        ActionKind::Break => {
            // The band stops: nobody but the break's listed responders starts a note inside it,
            // and the kit and keys really were playing just before (a stop, not a rest).
            let busy = [Agent::Keys, Agent::Drums, Agent::Pad]
                .iter()
                .filter(|p| !a.responders.contains(p))
                .map(|&p| onsets_in(evs, p, s + TOL, e - TOL))
                .sum::<usize>();
            let before =
                onsets_in(evs, Agent::Drums, s - 2.0, s) + onsets_in(evs, Agent::Keys, s - 2.0, s);
            (
                busy == 0 && before > 0,
                Absence,
                n,
                "silence from non-responders inside the break, after sound",
            )
        }
        ActionKind::ReEntry => {
            let stamped = by_initiator(s, s + 1.5);
            let absent_before = onsets_in(evs, a.initiator, s - 2.0, s - TOL) == 0;
            let staged = {
                let mut t: Vec<i64> = mine
                    .iter()
                    .filter(|x| x.agent == a.initiator)
                    .map(|x| (x.beat * 4.0).round() as i64)
                    .collect();
                t.sort_unstable();
                t.dedup();
                t.len() >= 2
            };
            (
                stamped > 0 && (absent_before || staged),
                Stamped,
                n,
                "stamped entry notes by the player, after silence or as a staged entry",
            )
        }
        ActionKind::Hold => (
            mine.iter().any(|x| {
                matches!(x.agent, Agent::Keys | Agent::Pad)
                    && x.beat >= s - TOL
                    && x.beat < e
                    && x.dur >= 1.4
            }),
            Stamped,
            n,
            "a stamped keys/pad voicing sustained across the hold",
        ),
        ActionKind::Reharmonize | ActionKind::Tonicize => {
            // The edit changed the chord, and a note sounding in the edited span carries a pitch
            // class the new chord has and the old did not.
            let ok = perf.edits.iter().filter(|x| x.action == a.id).any(|x| {
                let span = perf
                    .chords
                    .iter()
                    .find(|c| (c.start_beat - x.at_beat).abs() < 1e-6);
                let (lo, hi) = span
                    .map(|c| (c.start_beat, c.start_beat + c.dur_beats as f64))
                    .unwrap_or((x.at_beat, x.at_beat + 1.0));
                let new_pcs: Vec<i32> = x
                    .after
                    .pitch_classes()
                    .into_iter()
                    .filter(|pc| !x.before.contains_pc(*pc))
                    .collect();
                evs.iter().any(|v| {
                    v.pitch.is_some_and(|p| new_pcs.contains(&pitch_class(p)))
                        && v.beat < hi - 1e-6
                        && v.beat + v.dur > lo + 1e-6
                })
            });
            (
                ok,
                Harmonic,
                n,
                "an edit by this action whose new pitch class actually sounds in the edited span",
            )
        }
        ActionKind::Deflect => {
            let ok = perf.deflects.iter().any(|w| {
                (w.at_beat - s).abs() < 1e-6
                    && w.prepared
                    && w.actual.root_pc != w.expected.root_pc
                    && mine.iter().any(|x| {
                        x.agent == Agent::Bass
                            && (x.beat - s).abs() <= TOL
                            && x.pitch.map(pitch_class) == Some(w.actual.root_pc)
                    })
            });
            (
                ok,
                Harmonic,
                n,
                "a prepared miss at the slot start AND the bass stamped on the actual (not the expected) root",
            )
        }
        ActionKind::Resolve => {
            let target = a.target_beat.unwrap_or(s);
            let ok = perf.context_at(target).is_some_and(|ctx| {
                let prev = perf
                    .contexts
                    .iter()
                    .rev()
                    .find(|c| c.start_beat < ctx.start_beat - 1e-6);
                // Intended = the arrival the previous harmony pointed at, home, a tonic-family
                // arrival (Arrival / Prolong), or the backbone's own release (an Open or Reset slot
                // beginning here — the idiom's resolution after a miss).
                let release = perf.contexts.iter().any(|c| {
                    (c.start_beat - ctx.start_beat).abs() < 1e-6
                        && perf.bar_at(c.start_beat).is_some_and(|b| {
                            matches!(
                                b.gesture,
                                Some(super::backbone::HarmonicGesture::Open)
                                    | Some(super::backbone::HarmonicGesture::Reset)
                            )
                        })
                });
                let intended = prev.and_then(|p| p.expects) == Some(ctx.chord.root_pc)
                    || ctx.chord.root_pc == ctx.region.tonic_pc
                    || matches!(
                        ctx.relation,
                        super::context::HarmonicRelation::Arrival
                            | super::context::HarmonicRelation::Prolong
                    )
                    || release;
                intended
                    && mine.iter().any(|x| {
                        x.beat >= target - TOL
                            && x.beat < target + 1.0
                            && x.pitch
                                .is_some_and(|p| ctx.chord.contains_pc(pitch_class(p)))
                    })
            });
            (
                ok,
                Stamped,
                n,
                "a stamped chord-tone arrival on the intended harmony (the expected target or home)",
            )
        }
        ActionKind::Displace => {
            let off = mine
                .iter()
                .filter(|x| {
                    x.beat >= s - TOL && x.beat < e && AccentGrid::step_of(x.beat).1 % 4 != 0
                })
                .count();
            (
                off >= 2,
                Stamped,
                n,
                "at least two stamped off-beat onsets inside the displacement window",
            )
        }
        ActionKind::Pullback | ActionKind::Accelerate => {
            let rate = |lo: f64, hi: f64| {
                if hi <= lo {
                    0.0
                } else {
                    onsets_in(evs, Agent::Drums, lo, hi) as f64 / (hi - lo)
                }
            };
            let (now, before) = (rate(s, e), rate((s - (e - s)).max(0.0), s));
            let shape = if a.kind == ActionKind::Pullback {
                now < before - 1e-6
            } else {
                now > before + 1e-6
            };
            (
                n > 0 && shape,
                Stamped,
                n,
                "stamped half-/double-time strokes and a drum rate lower/higher than before",
            )
        }
        ActionKind::Fragment | ActionKind::Sequence => {
            // Stamped notes that are genuinely fragmentary: fewer events than the identity motif,
            // counted per material the action's notes project.
            let ident = perf.bank.identity.len();
            let mut mats: Vec<Option<super::ids::MaterialId>> =
                mine.iter().map(|x| x.prov.material).collect();
            mats.sort();
            mats.dedup();
            let fragmentary = mats.iter().any(|m| {
                let k = mine.iter().filter(|x| x.prov.material == *m).count();
                k > 0 && k < ident
            });
            (
                fragmentary,
                Stamped,
                n,
                "stamped notes forming a fragment (fewer events than the identity motif)",
            )
        }
        ActionKind::Thicken => {
            let voices = |lo: f64, hi: f64| {
                let mut best = 0;
                let mut t = lo;
                while t < hi - 1e-6 {
                    let c = evs
                        .iter()
                        .filter(|v| matches!(v.agent, Agent::Pad | Agent::Keys))
                        .filter(|v| v.beat <= t + 1e-6 && v.beat + v.dur > t + 1e-6)
                        .count();
                    best = best.max(c);
                    t += 0.25;
                }
                best
            };
            let high = mine
                .iter()
                .any(|x| x.agent == Agent::Pad && x.pitch.is_some_and(|p| p >= 79));
            (
                n > 0 && (high || voices(s, e) > voices((s - 8.0).max(0.0), s)),
                Stamped,
                n,
                "stamped pad notes adding an upper layer or more simultaneous voices",
            )
        }
        ActionKind::Thin => {
            let now = onsets_in(evs, a.initiator, s, e);
            let before = onsets_in(evs, a.initiator, (s - (e - s).max(2.0)).max(0.0), s);
            (
                now < before,
                Absence,
                n,
                "fewer onsets from the thinned player than in the preceding window",
            )
        }
        ActionKind::Fill => (
            by_initiator(s, e + TOL) >= 2,
            Stamped,
            n,
            "at least two stamped fill events by the fill's own initiator",
        ),
        ActionKind::Call => (
            by_initiator(s, e) > 0,
            Stamped,
            n,
            "the caller's stamped statement notes",
        ),
        ActionKind::Answer => {
            // By the RIGHT responder, and (for a planned material response) carrying that
            // response's own material.
            let mat = perf
                .interactions
                .iter()
                .filter_map(|i| i.response)
                .find(|r| r.action == Some(a.id))
                .and_then(|r| r.material);
            let ok = mine.iter().any(|x| {
                x.agent == a.initiator
                    && x.beat >= s - TOL
                    && x.beat < e + TOL
                    && (mat.is_none() || x.prov.material == mat)
            });
            (
                ok,
                Stamped,
                n,
                "the responder's stamped answer notes, carrying the response's material",
            )
        }
        ActionKind::Unison => {
            let keys: Vec<&&Ev> = mine.iter().filter(|x| x.agent == Agent::Keys).collect();
            let together = keys.iter().any(|k| {
                mine.iter().any(|b| {
                    b.agent == Agent::Bass
                        && (b.beat - k.beat).abs() <= TOL
                        && b.pitch.map(pitch_class) == k.pitch.map(pitch_class)
                })
            });
            (
                together,
                Stamped,
                n,
                "stamped keys and bass on the same figure, same onsets, same pitch classes",
            )
        }
    }
}

/// Audit every action of `perf` against `score`.
pub fn audit(perf: &PerformancePlan, score: &Score) -> WitnessReport {
    let evs = events(score);
    let rows = perf
        .actions
        .chronological()
        .into_iter()
        .map(|a| {
            let (witnessed, class, stamped, evidence) = audit_one(perf, &evs, a);
            Witness {
                action: a.id,
                kind: a.kind,
                witnessed,
                class,
                stamped,
                evidence,
            }
        })
        .collect();
    WitnessReport { rows }
}

/// One realized interaction receipt: is the answer materially related to ITS caller, more than to
/// whatever else was sounding (the coincident lead line a Round-VII responder used to copy)?
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InteractionReceipt {
    pub interaction: super::ids::InteractionId,
    pub initiator: Agent,
    pub responder: Agent,
    pub transform: Transform,
    /// Onsets of the caller's realized call notes / the responder's realized answer notes.
    pub call_events: usize,
    pub answer_events: usize,
    /// `relation(caller's realized call, answer)`.
    pub to_caller: f32,
    /// The strongest `relation` of the answer to any OTHER realized line sounding in the call's
    /// window (for a non-lead call, the lead's coincident notes), or 0 when there is none.
    pub to_other: f32,
}

impl InteractionReceipt {
    /// `to_caller - to_other`: positive = the answer is about the caller.
    pub fn margin(&self) -> f32 {
        self.to_caller - self.to_other
    }

    /// Whether the answer carries enough events to be evidence at all (a two-stroke echo matches
    /// almost anything with two onsets the same distance apart).
    pub fn informative(&self) -> bool {
        self.answer_events >= 3
    }
}

/// Measure every answered interaction whose responder realized notes (drums and pitched).
pub fn interaction_receipts(perf: &PerformancePlan, score: &Score) -> Vec<InteractionReceipt> {
    let evs = events(score);
    let heard = |xs: Vec<&Ev>| -> Vec<Heard> {
        let mut v: Vec<Heard> = xs
            .into_iter()
            .map(|x| Heard {
                beat: x.beat,
                pitch: x.pitch,
            })
            .collect();
        v.sort_by(|a, b| a.beat.total_cmp(&b.beat));
        v.dedup_by(|a, b| (a.beat - b.beat).abs() < 1e-3);
        v
    };
    let mut out = Vec::new();
    for i in &perf.interactions {
        let Some(r) = i.response else { continue };
        let (Some(ra), Some(_)) = (r.action, r.material) else {
            continue;
        };
        let call_notes = heard(
            evs.iter()
                .filter(|x| x.agent == i.call.initiator && x.prov.material == Some(i.call.material))
                .collect(),
        );
        let answer = heard(
            evs.iter()
                .filter(|x| x.agent == r.responder && x.prov.actions.has(ra))
                .collect(),
        );
        if call_notes.is_empty() || answer.is_empty() {
            continue;
        }
        let to_caller = relation(&call_notes, &answer, r.transform);
        // Every other player's line in the call window (not the caller, not the responder).
        let mut to_other = 0.0f32;
        for other in [Agent::Lead, Agent::Keys, Agent::Bass] {
            if other == i.call.initiator || other == r.responder {
                continue;
            }
            let line = heard(
                evs.iter()
                    .filter(|x| {
                        x.agent == other
                            && x.beat >= i.call.start_beat - 1e-6
                            && x.beat < i.call.end_beat - 1e-6
                    })
                    .collect(),
            );
            if line.len() >= 2 {
                to_other = to_other.max(relation(&line, &answer, r.transform));
            }
        }
        out.push(InteractionReceipt {
            interaction: i.id,
            initiator: i.call.initiator,
            responder: r.responder,
            transform: r.transform,
            call_events: call_notes.len(),
            answer_events: answer.len(),
            to_caller,
            to_other,
        });
    }
    out
}

/// Fraction of bars whose kick/snare pattern (16-step bitmask) equals an earlier bar's — a cheap
/// placement-rigidity reading used by the audit's tests (the diagnostics module has the full set).
pub fn drum_pattern_recurrence(score: &Score, total_bars: u32) -> f32 {
    let mut seen: Vec<u32> = Vec::new();
    let mut repeats = 0;
    let mut nonempty = 0;
    for bar in 0..total_bars {
        let mut v = 0u32;
        for d in &score.drums {
            if !matches!(d.voice, DrumVoice::Kick | DrumVoice::Snare) || d.velocity < 0.3 {
                continue;
            }
            let (b, s) = AccentGrid::step_of(d.start_beat);
            if b == bar {
                v |= 1 << s.min(STEPS - 1);
                if d.voice == DrumVoice::Snare {
                    v |= 1 << (16 + s.min(STEPS - 1)).min(31);
                }
            }
        }
        if v == 0 {
            continue;
        }
        nonempty += 1;
        if seen.contains(&v) {
            repeats += 1;
        } else {
            seen.push(v);
        }
    }
    if nonempty == 0 {
        0.0
    } else {
        repeats as f32 / nonempty as f32
    }
}

/// The realized kinetic curve: onsets per beat per bar across every player (drums included) —
/// forward motion read from the notes, independent of loudness.
pub fn kinetic_curve(score: &Score) -> Vec<f32> {
    let bars = (score.total_beats / score.beats_per_bar).round() as usize;
    let mut v = vec![0f32; bars];
    let mut add = |t: f64| {
        let b = (t / score.beats_per_bar).floor() as usize;
        if b < bars {
            v[b] += 1.0;
        }
    };
    for n in &score.notes {
        add(n.start_beat);
    }
    for d in &score.drums {
        add(d.start_beat);
    }
    v.iter().map(|c| c / score.beats_per_bar as f32).collect()
}

#[cfg(test)]
mod tests {
    use super::super::contract::CompositionGrammar;
    use super::super::functor::compose_full;
    use super::super::performance::PerformanceOptions;
    use super::super::semantic::deflected_lift_trace;
    use super::super::world::MusicWorld;
    use super::*;

    #[test]
    fn the_flagship_witnesses_every_action_in_the_notes() {
        for world in MusicWorld::all() {
            let c = compose_full(
                &deflected_lift_trace(120.0),
                &world,
                2112,
                Some(CompositionGrammar::DeflectedLift),
                PerformanceOptions::default(),
            );
            let r = audit(&c.perf, &c.score);
            eprintln!("{}: {}", world.name, r.report());
            assert!(r.total() >= 30, "{}: too few actions", world.name);
            // The compressed second cycle is more urgent than the statement cycle, without being
            // louder: more onsets per beat (the kinetic arc).
            let k = kinetic_curve(&c.score);
            let mean = |a: usize, b: usize| k[a..b].iter().sum::<f32>() / (b - a) as f32;
            assert!(
                mean(20, 28) > mean(4, 12),
                "{}: no kinetic build ({:.2} vs {:.2})",
                world.name,
                mean(20, 28),
                mean(4, 12)
            );
            assert!(
                r.witnessed() == r.total(),
                "{}: only {}/{} actions audibly witnessed\n{}",
                world.name,
                r.witnessed(),
                r.total(),
                r.report()
            );
        }
    }
}
