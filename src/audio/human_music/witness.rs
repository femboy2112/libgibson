//! **Audible witnesses** — does each planned action actually happen in the notes?
//!
//! An [`super::action::ActionPlan`] entry is a promise; this module checks the promise against the
//! realized [`Score`]. A push must put at least two players on the push step; a break must leave
//! the window empty; a fill must be drummed; a hold must sustain; a tonicization must have changed
//! the harmony; an answer must be heard from its responder; a unison must sound in at least two
//! parts at once. An action that the notes do not witness is exactly the "decorative verb" this
//! round removes — the audit counts them, per kind, so a regression is visible.
//!
//! Measurements, not a quality score.

use super::action::{ActionKind, Agent, MusicalAction};
use super::performance::{AccentGrid, PerformancePlan, STEPS};
use super::score::{DrumVoice, Note, Role, Score};
use super::theory::pitch_class;

/// Onset tolerance: within half a sixteenth.
const TOL: f64 = 0.125 + 1e-6;

/// The audit result for one action.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Witness {
    pub action: super::ids::ActionId,
    pub kind: ActionKind,
    pub witnessed: bool,
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
    /// The unwitnessed actions.
    pub fn missing(&self) -> impl Iterator<Item = &Witness> {
        self.rows.iter().filter(|w| !w.witnessed)
    }
    /// A compact report: totals and the per-kind misses.
    pub fn report(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "audible witnesses (does each action happen in the notes? — NOT a quality score):"
        );
        let _ = writeln!(
            s,
            "  witnessed {}/{} actions",
            self.witnessed(),
            self.total()
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
                "  UNWITNESSED a{} {} — expected {}",
                w.action,
                w.kind.label(),
                w.evidence
            );
        }
        s
    }
}

/// A performer's onsets (pitched roles from notes, drums from hits).
fn onsets(score: &Score, agent: Agent) -> Vec<f64> {
    let role = match agent {
        Agent::Lead => Some(Role::Lead),
        Agent::Keys => Some(Role::Keys),
        Agent::Bass => Some(Role::Bass),
        Agent::Pad => Some(Role::Pad),
        Agent::Drums | Agent::Ensemble => None,
    };
    match role {
        Some(r) => score.role_notes(r).map(|n| n.start_beat).collect(),
        None if agent == Agent::Drums => score.drums.iter().map(|d| d.start_beat).collect(),
        None => Vec::new(),
    }
}

const PLAYERS: [Agent; 5] = [
    Agent::Lead,
    Agent::Keys,
    Agent::Bass,
    Agent::Drums,
    Agent::Pad,
];

fn players_at(score: &Score, beat: f64) -> usize {
    PLAYERS
        .iter()
        .filter(|&&a| onsets(score, a).iter().any(|&t| (t - beat).abs() <= TOL))
        .count()
}

fn onsets_in(score: &Score, agent: Agent, a: f64, b: f64) -> usize {
    onsets(score, agent)
        .iter()
        .filter(|&&t| t >= a - TOL && t < b - 1e-6)
        .count()
}

fn drum_rate(score: &Score, a: f64, b: f64) -> f64 {
    if b <= a {
        return 0.0;
    }
    onsets_in(score, Agent::Drums, a, b) as f64 / (b - a)
}

/// Max simultaneously sounding pitched notes in `[a, b)`, sampled every sixteenth.
fn max_voices(score: &Score, roles: &[Role], a: f64, b: f64) -> usize {
    let mut best = 0;
    let mut t = a;
    while t < b - 1e-6 {
        let n = score
            .notes
            .iter()
            .filter(|n| roles.contains(&n.role))
            .filter(|n| n.start_beat <= t + 1e-6 && n.start_beat + n.dur_beats as f64 > t + 1e-6)
            .count();
        best = best.max(n);
        t += 0.25;
    }
    best
}

fn notes_tagged<'a>(score: &'a Score, tag: &'a str) -> impl Iterator<Item = &'a Note> + 'a {
    score.notes.iter().filter(move |n| n.prov.role_note == tag)
}

fn audit_one(perf: &PerformancePlan, score: &Score, a: &MusicalAction) -> (bool, &'static str) {
    let (s, e) = (a.start_beat, a.end_beat());
    match a.kind {
        ActionKind::Pickup => (
            onsets_in(score, a.initiator, s, a.target_beat.unwrap_or(e) + TOL) > 0,
            "an onset by the initiator leading into the target",
        ),
        ActionKind::Push | ActionKind::Hit => (
            players_at(score, s) >= 2,
            "at least two players striking the push/hit step together",
        ),
        ActionKind::Break => {
            // The band stops: nobody but the break's listed responders starts a note inside it.
            let busy = [Agent::Keys, Agent::Drums, Agent::Pad]
                .iter()
                .filter(|p| !a.responders.contains(p))
                .map(|&p| onsets_in(score, p, s + TOL, e))
                .sum::<usize>();
            (busy == 0, "no onsets from non-responders inside the break")
        }
        ActionKind::ReEntry => (
            onsets_in(score, a.initiator, s, s + 1.0) > 0
                || (a.initiator == Agent::Pad && onsets_in(score, Agent::Pad, s, s + 1.5) > 0),
            "the re-entering player sounding at the window's start",
        ),
        ActionKind::Hold => (
            score.notes.iter().any(|n| {
                matches!(n.role, Role::Keys | Role::Pad)
                    && n.start_beat >= s - TOL
                    && n.start_beat < e
                    && n.dur_beats >= 1.4
            }),
            "a keys/pad voicing sustained across the hold",
        ),
        ActionKind::Reharmonize | ActionKind::Tonicize => (
            perf.edits.iter().any(|x| x.action == a.id),
            "a harmonic edit made by this action",
        ),
        ActionKind::Deflect => (
            perf.deflects.iter().any(|w| {
                (w.at_beat - s).abs() < 1e-6 && w.prepared && w.actual.root_pc != w.expected.root_pc
            }),
            "a prepared miss (pointer toward X, arrival on not-X) at the slot start",
        ),
        ActionKind::Resolve => {
            let ok = perf.context_at(s).is_some_and(|ctx| {
                score.notes.iter().any(|n| {
                    matches!(n.role, Role::Lead | Role::Bass | Role::Keys)
                        && n.start_beat >= s - TOL
                        && n.start_beat < s + 1.0
                        && ctx.chord.contains_pc(pitch_class(n.pitch))
                })
            });
            (ok, "a chord-tone arrival at the resolution")
        }
        ActionKind::Displace => {
            let mut off = 0usize;
            let mut all = 0usize;
            for p in [Agent::Keys, Agent::Bass, Agent::Drums] {
                for t in onsets(score, p).into_iter().filter(|&t| t >= s && t < e) {
                    all += 1;
                    let (_, st) = AccentGrid::step_of(t);
                    if st % 4 != 0 {
                        off += 1;
                    }
                }
            }
            (
                all > 0 && off as f64 / all as f64 >= 0.35,
                "at least 35% of keys/bass/drum onsets off the beat inside the window",
            )
        }
        ActionKind::Pullback => (
            drum_rate(score, s, e) < drum_rate(score, (s - (e - s)).max(0.0), s) - 1e-6,
            "a lower drum onset rate than the preceding window",
        ),
        ActionKind::Accelerate => (
            drum_rate(score, s, e) > drum_rate(score, (s - (e - s)).max(0.0), s) + 1e-6,
            "a higher drum onset rate than the preceding window",
        ),
        ActionKind::Fragment | ActionKind::Sequence => {
            let lead_frag = perf.statements.iter().any(|st| {
                st.start_beat >= s - 1e-6
                    && st.start_beat < e
                    && st.motif.len() < perf.bank.identity.len()
            });
            let figure = perf
                .interactions
                .iter()
                .any(|i| i.call.action == a.id && i.call.initiator != Agent::Lead);
            // Before the thesis is stated the band carries the verb: a compressed-fragment answer.
            let band = perf.interactions.iter().any(|i| {
                i.call.start_beat >= s - 1e-6
                    && i.call.start_beat < e
                    && i.response
                        .as_ref()
                        .is_some_and(|r| r.transform == super::performance::Transform::Compress)
            });
            (
                lead_frag || figure || band,
                "a fragmented lead statement, a figure by the initiator, or the band's compressed-fragment answer",
            )
        }
        ActionKind::Thicken | ActionKind::Thin => {
            let roles = [Role::Pad, Role::Keys];
            let now = max_voices(score, &roles, s, e);
            let before = max_voices(score, &roles, (s - 8.0).max(0.0), s);
            let ok = if a.kind == ActionKind::Thicken {
                now > before
                    || notes_tagged(score, "pad")
                        .any(|n| n.start_beat >= s && n.start_beat < e && n.pitch >= 79)
            } else {
                now < before
            };
            (
                ok,
                "a change in simultaneous pad/keys voices (or an added upper layer)",
            )
        }
        ActionKind::Fill => (
            score.drums.iter().any(|d| {
                d.prov.groove_variation == Some("fill")
                    && d.start_beat >= s - TOL
                    && d.start_beat < e
            }) || (a.initiator == Agent::Bass && onsets_in(score, Agent::Bass, s, e) >= 2),
            "drum fill strokes (or a bass fill) inside the window",
        ),
        ActionKind::Call => (
            onsets_in(score, Agent::Lead, s, e) > 0,
            "the lead sounding its statement",
        ),
        ActionKind::Answer => {
            let heard = match a.initiator {
                Agent::Drums => score.drums.iter().any(|d| {
                    d.prov.groove_variation == Some("answer")
                        && d.start_beat >= s - TOL
                        && d.start_beat < e
                }),
                Agent::Lead => onsets_in(score, Agent::Lead, s, e) > 0,
                p => score.notes.iter().any(|n| {
                    n.prov.role_note == "answer"
                        && n.start_beat >= s - TOL
                        && n.start_beat < e
                        && onsets(score, p).contains(&n.start_beat)
                }),
            };
            (
                heard,
                "the responder's answer notes inside the response window",
            )
        }
        ActionKind::Unison => {
            let keys: Vec<&Note> = notes_tagged(score, "unison")
                .filter(|n| n.role == Role::Keys)
                .collect();
            let bass: Vec<&Note> = notes_tagged(score, "unison")
                .filter(|n| n.role == Role::Bass)
                .collect();
            let together = keys
                .iter()
                .filter(|k| k.start_beat >= s - TOL && k.start_beat < e)
                .any(|k| {
                    bass.iter().any(|b| {
                        (b.start_beat - k.start_beat).abs() <= TOL
                            && pitch_class(b.pitch) == pitch_class(k.pitch)
                    })
                });
            (
                together,
                "keys and bass on the same figure, same onsets, same pitch classes",
            )
        }
    }
}

/// Audit every action of `perf` against `score`.
pub fn audit(perf: &PerformancePlan, score: &Score) -> WitnessReport {
    let rows = perf
        .actions
        .actions
        .iter()
        .map(|a| {
            let (witnessed, evidence) = audit_one(perf, score, a);
            Witness {
                action: a.id,
                kind: a.kind,
                witnessed,
                evidence,
            }
        })
        .collect();
    WitnessReport { rows }
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
