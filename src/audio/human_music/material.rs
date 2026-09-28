//! **Interaction material** — what a call actually *says*, as an instrument-independent object that
//! exists in the [`super::performance::PerformancePlan`] before anybody plays a note (Round VIIb).
//!
//! Through Round VII a response was built from whatever the LEAD happened to play inside the call's
//! window — even when the caller was the bass or the keys. The players are realized in listening
//! order (lead → keys → pad → bass → drums), so a keys answer to a bass figure could not hear the
//! bass (it did not exist yet), and a plan that said "keys answers bass" produced keys transforming
//! coincident lead material. That is not conversation; it is two people talking over a third.
//!
//! Here every call owns an [`InteractionMaterial`]: a rhythm (onsets, durations, accents), an
//! optional relative pitch contour in scale steps (absent for drums — rhythm alone is identity),
//! and its provenance ([`MaterialSource`]). A response owns a *derived* material
//! ([`transform_material`] applies the transform — quote, echo, invert, compress, complete — to the CALL's
//! material, not to anybody's notes). The initiator realizes a projection of the call material and
//! the responder a projection of the derived material, so there is no circular dependence and the
//! realization order stops mattering for causality. [`relation`] measures, on realized notes,
//! whether a response is materially related to its real caller.

use super::action::{ActionKind, Agent, MusicalAction};
use super::context::HarmonicContext;
use super::ids::{ActionId, MaterialId};
use super::interaction::Transform;
use super::motif::{Motif, MotifBank};
use super::performance::PerformancePlan;
use super::theory::{pitch_class, Midi, Scale};

/// One event of a piece of material.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialEvent {
    /// Beats from the material's start.
    pub onset: f64,
    /// Duration in beats.
    pub dur: f64,
    /// Accent in `[0, 1]`.
    pub accent: f32,
    /// Scale steps relative to the first pitched event; `None` = unpitched (rhythm only).
    pub step: Option<i32>,
}

/// Where a piece of material came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MaterialSource {
    /// A planned lead statement (index into the performance's statements) and its motif id.
    Statement { statement: usize, motif: u8 },
    /// A figure a player states for an action (a bass pickup, a keys re-entry, a drum fill).
    Figure { action: ActionId, kind: ActionKind },
    /// A transform of another material — a response.
    Derived {
        from: MaterialId,
        transform: Transform,
    },
}

/// A piece of interaction material.
#[derive(Debug, Clone, PartialEq)]
pub struct InteractionMaterial {
    pub id: MaterialId,
    /// Who states it.
    pub owner: Agent,
    pub source: MaterialSource,
    /// The beat it is stated at.
    pub start_beat: f64,
    pub events: Vec<MaterialEvent>,
}

impl InteractionMaterial {
    /// Whether the material carries a pitch contour.
    pub fn pitched(&self) -> bool {
        self.events.iter().any(|e| e.step.is_some())
    }

    /// Its length in beats (last onset + duration).
    pub fn length(&self) -> f64 {
        self.events
            .iter()
            .map(|e| e.onset + e.dur)
            .fold(0.0, f64::max)
    }

    /// The contour's final direction (+1 rising, -1 falling, 0 flat or unpitched).
    pub fn final_direction(&self) -> i32 {
        let steps: Vec<i32> = self.events.iter().filter_map(|e| e.step).collect();
        match steps.as_slice() {
            [.., a, b] if b != a => (b - a).signum(),
            [first, .., last] => (last - first).signum(),
            _ => 0,
        }
    }

    /// The material a lead statement's motif states (rests are gaps, not events).
    pub fn from_motif(
        id: MaterialId,
        owner: Agent,
        motif: &Motif,
        start_beat: f64,
        source: MaterialSource,
    ) -> InteractionMaterial {
        let first = motif.degrees.first().copied().unwrap_or(0);
        let mut at = 0.0;
        let mut events = Vec::with_capacity(motif.len());
        for (i, &deg) in motif.degrees.iter().enumerate() {
            let d = motif.rhythm.get(i).copied().unwrap_or(0.5) as f64;
            let on_beat = ((start_beat + at) % 1.0).abs() < 1e-6;
            events.push(MaterialEvent {
                onset: at,
                dur: d,
                accent: if i == 0 {
                    1.0
                } else if on_beat {
                    0.8
                } else {
                    0.6
                },
                step: Some(deg - first),
            });
            at += d;
        }
        InteractionMaterial {
            id,
            owner,
            source,
            start_beat,
            events,
        }
    }

    /// The figure a player states for action `a`, generated centrally from the piece's motif bank
    /// (so every figure is the piece's own speech) — or `None` when the kind has no figure.
    /// Drum figures carry rhythm only.
    pub fn figure(
        id: MaterialId,
        a: &MusicalAction,
        bank: &MotifBank,
        strength: f32,
    ) -> Option<InteractionMaterial> {
        let d = a.dur_beats.max(0.5);
        let src = MaterialSource::Figure {
            action: a.id,
            kind: a.kind,
        };
        let even = |n: usize, step_of: &dyn Fn(usize) -> Option<i32>| -> Vec<MaterialEvent> {
            let gap = d / n as f64;
            (0..n)
                .map(|i| MaterialEvent {
                    onset: i as f64 * gap,
                    dur: gap * 0.9,
                    accent: (0.55 + 0.45 * i as f32 / n.max(2) as f32 - 0.05).min(1.0),
                    step: step_of(i),
                })
                .collect()
        };
        let events: Vec<MaterialEvent> = match (a.kind, a.initiator) {
            (ActionKind::Fill, Agent::Drums) => {
                // Eighths into the target (sixteenths when the move is strong): a rhythm-only call.
                let per_beat = if strength > 0.7 { 4.0 } else { 2.0 };
                let n = ((d * per_beat).round() as usize).max(2);
                even(n, &|_| None)
            }
            (ActionKind::Fill, _) => {
                // A pitched run into the next attempt: rising scale steps on eighths.
                let n = ((d * 2.0).round() as usize).max(2);
                even(n, &|i| Some(i as i32))
            }
            (ActionKind::Pickup, _) => {
                // A stepwise lead-in that points at the target downbeat.
                let n = if d >= 1.0 - 1e-6 { 3 } else { 2 };
                even(n, &|i| Some(i as i32))
            }
            (ActionKind::Fragment, _) | (ActionKind::ReEntry, _) => {
                // The identity's head (a fragment) or the piece's rhythmic cell (a re-entry),
                // fitted into the action's window.
                let m = if a.kind == ActionKind::Fragment {
                    bank.identity.fragment(3.min(bank.identity.len()))
                } else {
                    bank.rhythmic_cell.fragment(3.min(bank.rhythmic_cell.len()))
                };
                let total = m.total_beats() as f64;
                let scale = if total > d { d / total } else { 1.0 };
                let first = m.degrees.first().copied().unwrap_or(0);
                let mut at = 0.0;
                let mut ev = Vec::new();
                for (i, &deg) in m.degrees.iter().enumerate() {
                    let dur = m.rhythm.get(i).copied().unwrap_or(0.5) as f64 * scale;
                    ev.push(MaterialEvent {
                        onset: at,
                        dur: dur * 0.9,
                        accent: if i == 0 { 1.0 } else { 0.7 },
                        step: Some(deg - first),
                    });
                    at += dur;
                }
                ev
            }
            _ => return None,
        };
        Some(InteractionMaterial {
            id,
            owner: a.initiator,
            source: src,
            start_beat: a.start_beat,
            events,
        })
    }
}

/// Apply `transform` to the CALL's material (never to anybody's realized notes), producing the
/// response's own material, fitted into `room` beats. `None` for a silence or empty material.
pub fn transform_material(
    id: MaterialId,
    src: &InteractionMaterial,
    transform: Transform,
    owner: Agent,
    start_beat: f64,
    room: f64,
) -> Option<InteractionMaterial> {
    if src.events.is_empty() || room <= 0.0 {
        return None;
    }
    let n = src.events.len();
    let tail: Vec<MaterialEvent> = src.events[n.saturating_sub(4)..].to_vec();
    let head: Vec<MaterialEvent> = src.events[..n.min(3)].to_vec();
    let rebase = |ev: &[MaterialEvent], time: f64| -> Vec<MaterialEvent> {
        let t0 = ev[0].onset;
        let s0 = ev.iter().find_map(|e| e.step).unwrap_or(0);
        ev.iter()
            .map(|e| MaterialEvent {
                onset: (e.onset - t0) * time,
                dur: e.dur * time,
                accent: e.accent,
                step: e.step.map(|s| s - s0),
            })
            .collect()
    };
    let dir = match src.final_direction() {
        0 => 1,
        d => d,
    };
    let mut events = match transform {
        Transform::Silence => return None,
        Transform::Quote => rebase(&tail, 1.0),
        Transform::Echo => rebase(&tail, 1.0)
            .into_iter()
            .map(|e| MaterialEvent { step: None, ..e })
            .collect(),
        Transform::Invert => rebase(&tail, 1.0)
            .into_iter()
            .map(|e| MaterialEvent {
                step: e.step.map(|s| -s),
                ..e
            })
            .collect(),
        Transform::Compress => rebase(&head, 0.5),
        Transform::Complete => {
            // Continue past the call's end in its final direction, on the call's tail rhythm.
            let last = rebase(&tail, 1.0)
                .iter()
                .rev()
                .find_map(|e| e.step)
                .unwrap_or(0);
            rebase(&tail, 1.0)
                .into_iter()
                .enumerate()
                .map(|(i, e)| MaterialEvent {
                    step: e.step.map(|_| last + dir * (i as i32 + 1)),
                    ..e
                })
                .collect()
        }
    };
    // An unpitched call (a drum figure) answered by a pitched player is echoed: the rhythm is the
    // identity; the responder chooses its own pitches.
    events.retain(|e| e.onset < room - 1e-6);
    for e in &mut events {
        e.dur = e.dur.min(room - e.onset).max(0.1);
    }
    if events.is_empty() {
        return None;
    }
    Some(InteractionMaterial {
        id,
        owner,
        source: MaterialSource::Derived {
            from: src.id,
            transform,
        },
        start_beat,
        events,
    })
}

/// `steps` scale steps from `from` (snapped into `scale` first).
pub fn scale_step(scale: &Scale, from: Midi, steps: i32) -> Midi {
    let mut p = scale.nearest_scale_pitch(from);
    let dir = steps.signum();
    for _ in 0..steps.abs() {
        let mut q = p + dir;
        while !scale.contains_pc(pitch_class(q)) {
            q += dir;
        }
        p = q;
    }
    p
}

/// The stable pitch nearest `target` that keeps the contour's direction from `prev` (up stays up,
/// down stays down, a repeat stays put when it can), falling back to the plain nearest.
fn snap_directional(
    ctx: &HarmonicContext,
    target: Midi,
    prev: Option<Midi>,
    dir: i32,
    stable: &dyn Fn(&HarmonicContext, i32) -> bool,
) -> Midi {
    let ok_dir = |p: Midi| match (prev, dir) {
        (Some(q), d) if d > 0 => p > q,
        (Some(q), d) if d < 0 => p < q,
        _ => true,
    };
    if dir == 0 {
        if let Some(q) = prev {
            if stable(ctx, pitch_class(q)) {
                return q;
            }
        }
    }
    let cands = (0..=7).flat_map(|d| [target - d, target + d]);
    cands
        .clone()
        .find(|&p| stable(ctx, pitch_class(p)) && ok_dir(p))
        .or_else(|| {
            (0..=12)
                .flat_map(|d| [target - d, target + d])
                .find(|&p| stable(ctx, pitch_class(p)))
        })
        .unwrap_or(target)
}

/// One projected event: `(beat, dur, pitch)`; `pitch` is `None` for an unpitched projection.
pub type Projected = (f64, f64, Option<Midi>, f32);

/// Project `m` onto the timeline at `at` (clipped at `until`) for a pitched player whose line starts
/// near `anchor`: each step is walked in the local chord-scale, then placed on the nearest pitch the
/// player may hold over that harmony (`stable`) that keeps the contour's direction. Unpitched
/// material is echoed on the context's guide tones near `anchor`.
pub fn project_pitched(
    m: &InteractionMaterial,
    at: f64,
    until: f64,
    anchor: Midi,
    perf: &PerformancePlan,
    stable: &dyn Fn(&HarmonicContext, i32) -> bool,
) -> Vec<Projected> {
    let mut out: Vec<Projected> = Vec::new();
    let mut prev: Option<Midi> = None;
    let mut prev_step: Option<i32> = None;
    for (i, e) in m.events.iter().enumerate() {
        let beat = at + e.onset;
        if beat >= until - 1e-6 {
            break;
        }
        let Some(ctx) = perf.context_at(beat) else {
            continue;
        };
        let p = match e.step {
            Some(s) => {
                let base = match prev {
                    Some(q) => scale_step(&ctx.palette.scale, q, s - prev_step.unwrap_or(0)),
                    None => scale_step(&ctx.palette.scale, anchor, s),
                };
                let dir = prev_step.map(|ps| (s - ps).signum()).unwrap_or(0);
                prev_step = Some(s);
                snap_directional(
                    ctx,
                    base,
                    prev,
                    if prev.is_some() { dir } else { 0 },
                    stable,
                )
            }
            None => {
                let g = &ctx.palette.guide_tones;
                let pc = g
                    .get(i % g.len().max(1))
                    .copied()
                    .unwrap_or(ctx.chord.root_pc);
                let base = (anchor / 12) * 12 + pc;
                if base > anchor + 6 {
                    base - 12
                } else {
                    base
                }
            }
        };
        prev = Some(p);
        let dur = e.dur.min(until - beat).max(0.1);
        out.push((beat, dur, Some(p), e.accent));
    }
    out
}

/// Where a player's projection of material starts: the lead near its middle register, keys near
/// the top of the comp, bass near E2 (bass figures sit on chord tones only).
pub fn home_register(agent: Agent) -> Midi {
    match agent {
        Agent::Lead => 72,
        Agent::Keys => 72,
        Agent::Pad => 67,
        Agent::Bass => 40,
        Agent::Drums | Agent::Ensemble => 60,
    }
}

/// Whether `agent` may hold pitch class `pc` over `ctx` in a figure or answer: the bass takes chord
/// tones only; everybody else chord tones or licensed tensions.
pub fn stable_for(agent: Agent, ctx: &HarmonicContext, pc: i32) -> bool {
    match agent {
        Agent::Bass => ctx.chord.contains_pc(pc),
        _ => ctx.palette.is_stable(pc),
    }
}

/// The concrete line `m` becomes when its owner plays it at `at` (clipped at `until`) — the ONE
/// projection both the owner and anybody who listens to it (a lead answering a bass figure) use.
/// The line starts on the owner's stable pitch nearest its home register over the harmony at `at`.
/// Unpitched material (a drum figure) projects onsets only.
pub fn line_of(
    m: &InteractionMaterial,
    owner: Agent,
    at: f64,
    until: f64,
    perf: &PerformancePlan,
) -> Vec<Projected> {
    if !m.pitched() || owner == Agent::Drums {
        return m
            .events
            .iter()
            .map(|e| (at + e.onset, e.dur, None, e.accent))
            .take_while(|(b, ..)| *b < until - 1e-6)
            .collect();
    }
    let center = home_register(owner);
    let anchor = perf
        .context_at(at)
        .map(|ctx| {
            (0..=12)
                .flat_map(|d| [center - d, center + d])
                .find(|&p| stable_for(owner, ctx, pitch_class(p)))
                .unwrap_or(center)
        })
        .unwrap_or(center);
    project_pitched(m, at, until, anchor, perf, &|c, pc| {
        stable_for(owner, c, pc)
    })
}

/// A heard event, for [`relation`]: an onset and (for a pitched player) its pitch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Heard {
    pub beat: f64,
    pub pitch: Option<Midi>,
}

/// How strongly a realized response `resp` is materially related to a realized call `call` under
/// `transform`, in `[0, 1]` (NOT a quality score): onset-pattern agreement on the sixteenth grid
/// (±1 step counts half) blended with interval-contour agreement (the call's contour negated for an
/// inversion, its final direction continued for a completion). The call's tail is the reference
/// (its head, time-compressed ×0.5, for a compression). Rhythm alone when either side is unpitched.
pub fn relation(call: &[Heard], resp: &[Heard], transform: Transform) -> f32 {
    if call.is_empty() || resp.is_empty() || transform == Transform::Silence {
        return 0.0;
    }
    let n = call.len();
    let (mut reference, time): (Vec<Heard>, f64) = match transform {
        Transform::Compress => (call[..n.min(3)].to_vec(), 0.5),
        _ => (call[n.saturating_sub(4)..].to_vec(), 1.0),
    };
    // A response is fitted into its room from the START of the transformed material, so it can
    // only be held to the part of the reference that fits: compare against the same-length prefix.
    reference.truncate(resp.len().max(2).min(reference.len()));
    let q = |xs: &[Heard], t: f64| -> Vec<i64> {
        let t0 = xs[0].beat;
        xs.iter()
            .map(|h| (((h.beat - t0) * t) / 0.25).round() as i64)
            .collect()
    };
    let rq = q(&reference, time);
    let sq = q(resp, 1.0);
    let hits: f32 = sq
        .iter()
        .map(|s| {
            if rq.contains(s) {
                1.0
            } else if rq.contains(&(s - 1)) || rq.contains(&(s + 1)) {
                0.5
            } else {
                0.0
            }
        })
        .sum();
    let rhythm = hits / rq.len().max(sq.len()) as f32;
    let signs = |xs: &[Heard]| -> Option<Vec<i32>> {
        let ps: Vec<Midi> = xs.iter().map(|h| h.pitch).collect::<Option<Vec<_>>>()?;
        Some(ps.windows(2).map(|w| (w[1] - w[0]).signum()).collect())
    };
    let contour = match (signs(&reference), signs(resp)) {
        (Some(rs), Some(ss)) if !rs.is_empty() && !ss.is_empty() => {
            let want: Vec<i32> = match transform {
                Transform::Invert => rs.iter().map(|s| -s).collect(),
                Transform::Complete => {
                    let dir = rs.iter().rev().find(|&&s| s != 0).copied().unwrap_or(1);
                    vec![dir; ss.len()]
                }
                _ => rs.clone(),
            };
            let agree = ss.iter().zip(&want).filter(|(a, b)| a == b).count();
            Some(agree as f32 / want.len().max(ss.len()) as f32)
        }
        _ => None,
    };
    match (contour, transform) {
        (Some(c), t) if t != Transform::Echo => 0.6 * rhythm + 0.4 * c,
        _ => rhythm,
    }
}

#[cfg(test)]
mod tests {
    use super::super::theory::Mode;
    use super::*;

    fn mat(steps: &[Option<i32>], onsets: &[f64]) -> InteractionMaterial {
        InteractionMaterial {
            id: MaterialId(0),
            owner: Agent::Bass,
            source: MaterialSource::Figure {
                action: ActionId(0),
                kind: ActionKind::Pickup,
            },
            start_beat: 0.0,
            events: steps
                .iter()
                .zip(onsets)
                .map(|(&s, &o)| MaterialEvent {
                    onset: o,
                    dur: 0.4,
                    accent: 0.7,
                    step: s,
                })
                .collect(),
        }
    }

    #[test]
    fn the_transform_algebra_holds_at_the_material_level() {
        let m = mat(&[Some(0), Some(2), Some(1), Some(3)], &[0.0, 0.5, 1.0, 1.5]);
        let inv = transform_material(MaterialId(1), &m, Transform::Invert, Agent::Keys, 4.0, 8.0)
            .unwrap();
        let back = transform_material(
            MaterialId(2),
            &inv,
            Transform::Invert,
            Agent::Keys,
            4.0,
            8.0,
        )
        .unwrap();
        let steps = |x: &InteractionMaterial| x.events.iter().map(|e| e.step).collect::<Vec<_>>();
        assert_eq!(steps(&back), steps(&m), "invert∘invert = id");
        let c = transform_material(
            MaterialId(3),
            &m,
            Transform::Compress,
            Agent::Keys,
            4.0,
            8.0,
        )
        .unwrap();
        assert_eq!(c.events.len(), 3);
        assert!(
            (c.events[1].onset - 0.25).abs() < 1e-9,
            "compress halves onsets"
        );
        let e =
            transform_material(MaterialId(4), &m, Transform::Echo, Agent::Keys, 4.0, 8.0).unwrap();
        assert!(
            e.events.iter().all(|x| x.step.is_none()),
            "echo keeps rhythm only"
        );
        let k = transform_material(
            MaterialId(5),
            &m,
            Transform::Complete,
            Agent::Keys,
            4.0,
            8.0,
        )
        .unwrap();
        let ks: Vec<i32> = k.events.iter().filter_map(|x| x.step).collect();
        assert!(
            ks.windows(2).all(|w| w[1] > w[0]),
            "complete continues upward: {ks:?}"
        );
        assert!(
            transform_material(MaterialId(6), &m, Transform::Silence, Agent::Keys, 4.0, 8.0)
                .is_none()
        );
        let short =
            transform_material(MaterialId(7), &m, Transform::Quote, Agent::Keys, 4.0, 1.0).unwrap();
        assert!(
            short.events.iter().all(|x| x.onset < 1.0),
            "fitted into the room"
        );
    }

    #[test]
    fn scale_steps_walk_the_scale() {
        let c = Scale::new(0, Mode::Ionian);
        assert_eq!(scale_step(&c, 60, 1), 62);
        assert_eq!(scale_step(&c, 60, 2), 64);
        assert_eq!(scale_step(&c, 64, 1), 65);
        assert_eq!(scale_step(&c, 60, -1), 59);
    }

    #[test]
    fn relation_prefers_the_real_caller_over_unrelated_material() {
        let h = |v: &[(f64, i32)]| -> Vec<Heard> {
            v.iter()
                .map(|&(b, p)| Heard {
                    beat: b,
                    pitch: Some(p),
                })
                .collect()
        };
        // Call A: rising eighths. Unrelated B: a falling dotted figure.
        let a = h(&[(0.0, 40), (0.5, 43), (1.0, 45), (1.5, 47)]);
        let b = h(&[(0.0, 72), (0.75, 69), (1.25, 67), (2.5, 64)]);
        // A quote of A in another register, later.
        let resp = h(&[(4.0, 64), (4.5, 67), (5.0, 69), (5.5, 71)]);
        let ra = relation(&a, &resp, Transform::Quote);
        let rb = relation(&b, &resp, Transform::Quote);
        assert!(ra > 0.95, "a quote of the caller relates to it: {ra}");
        assert!(
            ra - rb > 0.4,
            "…and not to the unrelated phrase: {ra} vs {rb}"
        );
        // An inversion relates under Invert, not under Quote.
        let inv = h(&[(4.0, 64), (4.5, 61), (5.0, 59), (5.5, 57)]);
        assert!(relation(&a, &inv, Transform::Invert) > relation(&a, &inv, Transform::Quote));
    }
}
