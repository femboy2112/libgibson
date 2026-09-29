//! **The surgical pass** (Round VIIIb) — the R7b band, perturbed as little as possible.
//!
//! Round VIII answered "the band lost harmonic coherence" by rebuilding the harmonic bed: one joint
//! pad+keys solve that ranked every counted collision above all voice-leading. It revoiced three
//! quarters of the pad (motion 4.7 -> 7.6 semitones, common tones 1.3 -> 0.9) and the listen called
//! it trash. The collisions it chased were real but LOCAL: a keys stab ringing across the change,
//! one pad voice a minor 9th above its own major 7th, a bass that octave-copied the lead's 11th.
//!
//! This pass keeps the R7b realization as the prior and touches only what is actually wrong. It
//! cuts the finished Score at its real temporal boundaries — every note to its AUDIBLE end at the
//! masking floor ([`MASKING_FLOOR_DB`]), so a 0.45-beat stab is judged only against what sounds
//! while it sounds, never against the whole harmony it sits in — and asks the audit's own ruler
//! ([`slices`]) for the HARD defects: an unowned minor 2nd / minor 9th that lasts
//! ([`MIN_OVERLAP_BEATS`]), a floor naming a tension nothing owns, a held identity flip, a linear
//! note that never resolves. Everything else the ruler reports (a doubled colour, a crowded slice, a
//! missing guide tone in a transient) stays a measurement, not a rewrite command.
//!
//! Each defect is repaired by the smallest local edit of ONE offending note, in ladder order:
//! release it before the collision (articulation before pitch); leave it out when the harmony is
//! carried without it; move it an octave; swap it for a nearby tone of the same sonority (a chord
//! tone, or a colour the band already sounds); for the bass, re-pitch it onto the harmony's root or
//! fifth. The lead is never touched, and a support player takes the edit before the bass does. An
//! edit is taken only if the hard defects around it strictly drop and no soft measure (a guide tone
//! lost, an identity flip) worsens by more than a transient; among those: fewest hard defects, then
//! an edit that keeps every receipt, then the earliest rung, then the smallest displacement. A
//! receipt is never kept by keeping the bad note: when every clean repair costs an action its
//! witness, the repair happens and the action is reported deferred ([`VerticalRepair::deferred`]).

use std::collections::BTreeMap;

use super::context::{context_at, HarmonicContext};
use super::harmonic_state::voice_of;
use super::ids::ActionId;
use super::instrument::Patch;
use super::score::{Note, PitchFunction, Role, Score};
use super::sonority::{
    audible_end_at, audible_voices, is_linear, is_suspension, settle_resolutions, slices,
    BassFunction, Clash, ColorPolicy, Problem, Voice, LINEAR_MAX_BEATS, MASKING_FLOOR_DB,
    MIN_OVERLAP_BEATS,
};
use super::theory::{note_name, pitch_class, Midi};
use super::world::MusicWorld;

/// The receipt oracle: every planned action and whether `score` witnesses it (in plan order).
pub type Receipts<'a> = &'a dyn Fn(&Score) -> Vec<(ActionId, bool)>;

/// Detection passes over the whole score (a repair can expose a defect its note was masking).
const PASSES: usize = 3;
/// Beats judged on each side of an edited note (covers a linear predecessor's resolution window).
const JUDGE_MARGIN: f64 = 2.5;
/// Extra beats of voices loaded around the judged window (root memory, resolutions at its edge).
const VOICE_MARGIN: f64 = 3.0;
/// The shortest note a release may leave.
const MIN_KEEP_BEATS: f64 = 0.25;
/// How much a repair may worsen the soft measures (missing guide tones, unheld identity flips).
const SOFT_SLACK_BEATS: f64 = 0.25;
/// How far a same-sonority swap may move a support voice.
const SAME_SONORITY_REACH: Midi = 4;
/// How far a floor repair may move the bass.
const FLOOR_REACH: Midi = 7;

/// What a repair did to one note.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RepairEdit {
    /// Released earlier: the written duration went from `from` to `to` beats.
    Shortened { from: f32, to: f32 },
    /// Left out.
    Removed,
    /// Re-pitched to `to`.
    Repitched { to: Midi },
}

/// The rung of the repair ladder an edit came from (earliest = least perturbation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RepairStrategy {
    /// Released before the collision: articulation, not pitch.
    Shorten,
    /// Left out: space beats a revoicing.
    Omit,
    /// The same pitch class an octave away.
    Octave,
    /// Another tone of the same sonority (a chord tone, or a colour the band already sounds).
    SameSonority,
    /// The bass re-pitched onto a valid floor (the harmony's root or fifth).
    Floor,
}

impl RepairStrategy {
    /// Where this rung sits in `role`'s ladder. The support players leave a note out before they
    /// move it (space beats a revoicing); the bass re-places a line note before it drops one — an
    /// omitted approach punches a hole in the walking line's rhythm, a re-pitched one keeps it.
    fn rung(self, role: Role) -> u8 {
        match (role, self) {
            (_, RepairStrategy::Shorten) => 0,
            (Role::Bass, RepairStrategy::Octave) => 1,
            (Role::Bass, RepairStrategy::Floor) => 2,
            (Role::Bass, RepairStrategy::Omit) => 3,
            (_, RepairStrategy::Omit) => 1,
            (_, RepairStrategy::Octave) => 2,
            (_, RepairStrategy::SameSonority) | (_, RepairStrategy::Floor) => 3,
        }
    }

    /// A short label.
    pub fn label(self) -> &'static str {
        match self {
            RepairStrategy::Shorten => "shorten",
            RepairStrategy::Omit => "omit",
            RepairStrategy::Octave => "octave",
            RepairStrategy::SameSonority => "same-sonority",
            RepairStrategy::Floor => "floor",
        }
    }
}

/// One edit of the surgical pass — nothing is mutated silently.
#[derive(Debug, Clone, PartialEq)]
pub struct VerticalRepair {
    /// The edited note's onset.
    pub beat: f64,
    pub role: Role,
    /// The note's pitch before the edit.
    pub original: Midi,
    pub edit: RepairEdit,
    pub strategy: RepairStrategy,
    /// The hard defect it removed, naming the notes.
    pub defect: String,
    /// Actions whose witness this edit surrendered to the harmony (empty: every receipt kept).
    pub deferred: Vec<ActionId>,
}

impl VerticalRepair {
    /// One ledger line.
    pub fn line(&self) -> String {
        let edit = match self.edit {
            RepairEdit::Shortened { from, to } => format!(
                "{} shortened {from:.2} -> {to:.2} b",
                note_name(self.original)
            ),
            RepairEdit::Removed => format!("{} removed", note_name(self.original)),
            RepairEdit::Repitched { to } => {
                format!("{} -> {}", note_name(self.original), note_name(to))
            }
        };
        let deferred = if self.deferred.is_empty() {
            String::new()
        } else {
            let ids: Vec<String> = self.deferred.iter().map(|a| a.to_string()).collect();
            format!("  [receipt deferred: {}]", ids.join(" "))
        };
        format!(
            "{:>7.2} {:5} {:13} {:30} because {}{deferred}",
            self.beat,
            self.role.label(),
            self.strategy.label(),
            edit,
            self.defect
        )
    }
}

/// The repair ledger, one line per edit.
pub fn ledger(repairs: &[VerticalRepair]) -> String {
    let mut s = format!(
        "surgical repairs (R7b -> surgical, every edit): {}\n",
        repairs.len()
    );
    for r in repairs {
        s.push_str("  ");
        s.push_str(&r.line());
        s.push('\n');
    }
    s
}

/// The kind of a hard defect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Clash,
    Floor,
    Flip,
    Unresolved,
}

/// One hard defect: its kind, the notes it names (Score indices; `None` = a pitched SFX), where it
/// starts sounding, and a description.
#[derive(Debug, Clone, PartialEq)]
struct Defect {
    kind: Kind,
    notes: Vec<Option<usize>>,
    start: f64,
    text: String,
    /// A clash against the melody (the lead is fixed: the pitch space is the melody's).
    melody: bool,
}

impl Defect {
    fn same(&self, o: &Defect) -> bool {
        self.kind == o.kind && self.notes == o.notes
    }
}

/// What a window of the score costs: unique hard defects, the beats during which one sounds, and the
/// soft guard (beats missing a guide tone, beats of an unheld identity flip).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Cost {
    hard: usize,
    beats: f64,
    soft: f64,
}

/// A candidate edit of one note.
#[derive(Debug, Clone, Copy)]
struct Candidate {
    strategy: RepairStrategy,
    /// The replacement note (`None` = removed).
    note: Option<Note>,
    displacement: i32,
}

fn patch(world: &MusicWorld, role: Role) -> &Patch {
    match role {
        Role::Pad => &world.pad,
        Role::Keys => &world.keys,
        Role::Bass => &world.bass,
        Role::Lead => &world.lead,
    }
}

/// The idiomatic register a repair may move a role's note within: the realizers' own windows (the
/// pad's full and upper layers 52–91, the keys 58–84, the bass's floor), a few semitones of slack
/// below for a voice stepping out from under the melody.
fn range(role: Role) -> (Midi, Midi) {
    match role {
        Role::Pad => (48, 91),
        Role::Keys => (55, 84),
        Role::Bass => (28, 55),
        Role::Lead => (0, 127),
    }
}

fn order(a: &Voice, b: &Voice) -> std::cmp::Ordering {
    a.start
        .total_cmp(&b.start)
        .then(a.role.label().cmp(b.role.label()))
        .then(a.pitch.cmp(&b.pitch))
}

/// The score being repaired: its notes (edited in place, removals flagged so indices stay stable),
/// the pitched stings (fixed), and what the ruler needs.
struct Work<'a> {
    notes: Vec<Note>,
    gone: Vec<bool>,
    edited: Vec<bool>,
    sfx: Vec<Voice>,
    contexts: &'a [HarmonicContext],
    world: &'a MusicWorld,
    policy: &'a ColorPolicy,
    tempo: f32,
}

impl Work<'_> {
    /// A note as the ear meets it: sounding to its audible end at the masking floor.
    fn voice(&self, n: &Note) -> Voice {
        let mut v = voice_of(n);
        v.end = audible_end_at(
            v.start,
            n.dur_beats as f64,
            patch(self.world, n.role),
            self.tempo,
            MASKING_FLOOR_DB,
        );
        v
    }

    /// Every voice sounding in `[lo, hi)` with its origin, `edit` applied to one note (`Some(n)`
    /// replaces it, `None` removes it); resolutions settled over the list.
    fn voices(
        &self,
        lo: f64,
        hi: f64,
        edit: Option<(usize, Option<&Note>)>,
    ) -> (Vec<Voice>, Vec<Option<usize>>) {
        let mut pairs: Vec<(Voice, Option<usize>)> = Vec::new();
        for (i, n) in self.notes.iter().enumerate() {
            if self.gone[i] {
                continue;
            }
            let n = match edit {
                Some((j, rep)) if j == i => match rep {
                    Some(r) => r,
                    None => continue,
                },
                _ => n,
            };
            let v = self.voice(n);
            if v.start < hi && v.end > lo {
                pairs.push((v, Some(i)));
            }
        }
        for v in &self.sfx {
            if v.start < hi && v.end > lo {
                pairs.push((*v, None));
            }
        }
        pairs.sort_by(|a, b| order(&a.0, &b.0));
        let (mut v, o): (Vec<Voice>, Vec<Option<usize>>) = pairs.into_iter().unzip();
        settle_resolutions(&mut v, self.contexts);
        (v, o)
    }

    /// The hard defects sounding in `[lo, hi)` and the window's cost, `edit` applied.
    fn assess(
        &self,
        lo: f64,
        hi: f64,
        edit: Option<(usize, Option<&Note>)>,
    ) -> (Vec<Defect>, Cost) {
        let (voices, origin) = self.voices(lo - VOICE_MARGIN, hi + VOICE_MARGIN, edit);
        let chord_at = |beat: f64| {
            context_at(self.contexts, beat)
                .map(|c| c.chord.label())
                .unwrap_or_default()
        };
        let who = |i: usize| {
            let v = &voices[i];
            let role = if v.sfx { "sfx" } else { v.role.label() };
            format!("{role} {}", note_name(v.pitch))
        };
        let mut found: BTreeMap<(Kind, Vec<Option<usize>>), Defect> = BTreeMap::new();
        let mut cost = Cost::default();
        for s in slices(&voices, self.contexts, self.policy, &[]) {
            let (a, b) = (s.start.max(lo), s.end.min(hi));
            if b <= a + 1e-9 {
                continue;
            }
            let mut hard_here = false;
            for p in &s.problems {
                let (kind, idx, start, text) = match *p {
                    Problem::Unowned { clash, a: x, b: y } => {
                        let (vx, vy) = (&voices[x], &voices[y]);
                        if vx.end.min(vy.end) - vx.start.max(vy.start) < MIN_OVERLAP_BEATS - 1e-9 {
                            continue;
                        }
                        // Where the ear first hears it as a clash: the first slice that flags it (a
                        // legal cluster's own release tail only clashes under the NEXT harmony).
                        let start = s.start;
                        let label = match clash {
                            Clash::MinorSecond => "m2",
                            Clash::MinorNinth => "m9",
                        };
                        (
                            Kind::Clash,
                            vec![x, y],
                            start,
                            format!(
                                "{} against {} = unowned {label} over {} @{start:.2}",
                                who(x),
                                who(y),
                                chord_at(start)
                            ),
                        )
                    }
                    Problem::BassFunction { bass, function } => (
                        Kind::Floor,
                        vec![bass],
                        voices[bass].start,
                        format!(
                            "{} as the floor of {} = {}",
                            who(bass),
                            chord_at(voices[bass].start),
                            function.label()
                        ),
                    ),
                    Problem::IdentityFlip { bass, held: true } => (
                        Kind::Flip,
                        vec![bass],
                        s.start,
                        format!(
                            "{} floor held under a rootless {} (identity flip)",
                            who(bass),
                            chord_at(s.start)
                        ),
                    ),
                    Problem::Unresolved { voice } => (
                        Kind::Unresolved,
                        vec![voice],
                        voices[voice].start,
                        format!("{} never resolves", who(voice)),
                    ),
                    _ => continue,
                };
                hard_here = true;
                let notes: Vec<Option<usize>> = idx.iter().map(|&i| origin[i]).collect();
                let melody = kind == Kind::Clash
                    && idx
                        .iter()
                        .any(|&i| voices[i].role == Role::Lead && !voices[i].sfx);
                found.entry((kind, notes.clone())).or_insert(Defect {
                    kind,
                    notes,
                    start,
                    text,
                    melody,
                });
            }
            for p in &s.problems {
                match p {
                    Problem::MissingCore { .. } if s.end - s.start >= 0.25 - 1e-9 => {
                        cost.soft += b - a
                    }
                    Problem::IdentityFlip { held: false, .. } => cost.soft += b - a,
                    _ => {}
                }
            }
            if hard_here {
                cost.beats += b - a;
            }
        }
        // A linear or suspended note that never reaches its resolution (the audit's own rule).
        for (i, v) in voices.iter().enumerate() {
            let needs = v.function.is_some_and(|f| is_linear(f) || is_suspension(f));
            if needs && !v.resolves && v.start >= lo - 1e-9 && v.start < hi {
                found
                    .entry((Kind::Unresolved, vec![origin[i]]))
                    .or_insert(Defect {
                        kind: Kind::Unresolved,
                        notes: vec![origin[i]],
                        start: v.start,
                        melody: false,
                        text: format!(
                            "{} ({}) never resolves",
                            who(i),
                            v.function.map(|f| f.label()).unwrap_or("?")
                        ),
                    });
            }
        }
        cost.hard = found.len();
        let mut out: Vec<Defect> = found.into_values().collect();
        out.sort_by(|a, b| a.start.total_cmp(&b.start));
        (out, cost)
    }

    /// The longest written duration of `n` whose audible end lands by `t` (0 if even an instant
    /// rings past it).
    fn dur_ending_by(&self, n: &Note, t: f64) -> f64 {
        let end = |d: f64| {
            audible_end_at(
                n.start_beat,
                d,
                patch(self.world, n.role),
                self.tempo,
                MASKING_FLOOR_DB,
            )
        };
        if end(0.0) > t + 1e-9 {
            return 0.0;
        }
        let (mut lo, mut hi) = (0.0f64, n.dur_beats as f64);
        for _ in 0..40 {
            let mid = 0.5 * (lo + hi);
            if end(mid) <= t + 1e-9 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// The notes this defect lets the pass edit: the support players (pad, keys) and the bass —
    /// never the lead or a sting. The ranking keeps the bass the last resort: a bass edit wins only
    /// by clearing strictly more (a 0.9-beat approach trimmed to a real passing note owns the brush
    /// it made with the pad AND stops being a floor).
    fn parties(&self, d: &Defect) -> Vec<usize> {
        let live: Vec<usize> = d
            .notes
            .iter()
            .flatten()
            .copied()
            .filter(|&i| !self.gone[i] && !self.edited[i])
            .collect();
        let role = |i: usize| self.notes[i].role;
        match d.kind {
            Kind::Clash => live
                .into_iter()
                .filter(|&i| matches!(role(i), Role::Pad | Role::Keys | Role::Bass))
                .collect(),
            Kind::Floor | Kind::Flip => live
                .into_iter()
                .filter(|&i| role(i) == Role::Bass)
                .collect(),
            Kind::Unresolved => live
                .into_iter()
                .filter(|&i| role(i) != Role::Lead)
                .collect(),
        }
    }

    /// Every candidate edit of note `i` for defect `d`, in ladder order.
    fn candidates(&self, i: usize, d: &Defect) -> Vec<Candidate> {
        let n = self.notes[i];
        let mut out = Vec::new();
        let linear = n.function.is_some_and(is_linear) || n.prov.role_note == "approach";
        let dur = n.dur_beats as f64;
        // 1. Release it before the collision (or, for a line note, down to a linear length).
        let mut durs: Vec<f64> = Vec::new();
        if d.kind == Kind::Clash && d.start > n.start_beat + 1e-6 {
            durs.push(self.dur_ending_by(&n, d.start));
        }
        if linear {
            durs.push(LINEAR_MAX_BEATS - 0.05);
        }
        for dd in durs {
            let dd = (dd * 32.0).floor() / 32.0;
            let enough = dd >= MIN_KEEP_BEATS - 1e-9 && (linear || dd >= 0.5 * dur - 1e-9);
            if dd < dur - 1e-6 && enough {
                out.push(Candidate {
                    strategy: RepairStrategy::Shorten,
                    note: Some(Note {
                        dur_beats: dd as f32,
                        ..n
                    }),
                    displacement: 0,
                });
            }
        }
        // 2. Leave it out (a bass note only when it is a line note, never a floor it states).
        if n.role != Role::Bass || linear {
            out.push(Candidate {
                strategy: RepairStrategy::Omit,
                note: None,
                displacement: 0,
            });
        }
        let (low, high) = range(n.role);
        // No new unison inside one player: the same role already sounding that pitch meanwhile.
        let doubled = |p: Midi| {
            self.notes.iter().enumerate().any(|(j, m)| {
                j != i
                    && !self.gone[j]
                    && m.role == n.role
                    && m.pitch == p
                    && m.start_beat < n.start_beat + dur - 1e-6
                    && m.start_beat + m.dur_beats as f64 > n.start_beat + 1e-6
            })
        };
        // A support voice stays under the melody: no repair lifts it past a lead note it sounds with.
        let crosses = |p: Midi| {
            n.role != Role::Bass
                && self.notes.iter().enumerate().any(|(j, m)| {
                    !self.gone[j]
                        && m.role == Role::Lead
                        && n.pitch <= m.pitch
                        && p > m.pitch
                        && m.start_beat < n.start_beat + dur - 1e-6
                        && m.start_beat + m.dur_beats as f64 > n.start_beat + 1e-6
                })
        };
        let repitch = |p: Midi, f: Option<PitchFunction>, strategy| Candidate {
            strategy,
            note: Some(Note {
                pitch: p,
                function: f,
                ..n
            }),
            displacement: (p - n.pitch).abs(),
        };
        // 3. The same pitch class an octave away.
        for p in [n.pitch - 12, n.pitch + 12] {
            if (low..=high).contains(&p) && !doubled(p) && !crosses(p) {
                out.push(repitch(p, n.function, RepairStrategy::Octave));
            }
        }
        let Some(ctx) = context_at(self.contexts, n.start_beat) else {
            return out;
        };
        if n.role == Role::Bass {
            // 4b. The floor: the harmony's root or fifth, nearest first.
            for p in (n.pitch - FLOOR_REACH)..=(n.pitch + FLOOR_REACH) {
                if p != n.pitch
                    && (low..=high).contains(&p)
                    && BassFunction::of(&ctx.chord, p).is_foundation()
                {
                    out.push(repitch(
                        p,
                        Some(PitchFunction::ChordTone),
                        RepairStrategy::Floor,
                    ));
                }
            }
            return out;
        }
        // 4. Another tone of the same sonority: a chord tone, or a colour the band already sounds
        //    while this note does (never a new extension).
        let end = n.start_beat + dur;
        let colours: Vec<i32> = self
            .notes
            .iter()
            .enumerate()
            .filter(|&(j, m)| {
                j != i
                    && !self.gone[j]
                    && m.start_beat < end
                    && m.start_beat + m.dur_beats as f64 > n.start_beat
            })
            .map(|(_, m)| pitch_class(m.pitch))
            .filter(|pc| ctx.palette.tensions.contains(pc) && !ctx.chord.contains_pc(*pc))
            .collect();
        for p in (n.pitch - SAME_SONORITY_REACH)..=(n.pitch + SAME_SONORITY_REACH) {
            if p == n.pitch || !(low..=high).contains(&p) || doubled(p) || crosses(p) {
                continue;
            }
            let pc = pitch_class(p);
            let f = if ctx.chord.contains_pc(pc) {
                PitchFunction::ChordTone
            } else if colours.contains(&pc) {
                PitchFunction::LicensedExtension
            } else {
                continue;
            };
            out.push(repitch(p, Some(f), RepairStrategy::SameSonority));
        }
        out
    }

    /// The score as it stands (edits applied), `edit` on top — what the receipt oracle audits.
    fn materialize(&self, template: &Score, edit: Option<(usize, Option<&Note>)>) -> Score {
        let mut s = template.clone();
        s.notes = self
            .notes
            .iter()
            .enumerate()
            .filter(|&(i, _)| !self.gone[i])
            .filter_map(|(i, n)| match edit {
                Some((j, rep)) if j == i => rep.copied(),
                _ => Some(*n),
            })
            .collect();
        s
    }

    /// Repair `d` with the best candidate edit of one of its parties, if any strictly helps.
    fn fix(
        &mut self,
        d: &Defect,
        oracle: Option<(Receipts, &Score)>,
        now: &mut Vec<(ActionId, bool)>,
    ) -> Option<VerticalRepair> {
        struct Ranked {
            i: usize,
            c: Candidate,
            after: Cost,
            /// It costs a guide tone / identity beyond the slack (allowed only as the melody's yield).
            soft_worse: bool,
        }
        let parties = self.parties(d);
        // ONE window for every party, so their candidates are compared on the same ledger.
        let lo = parties
            .iter()
            .map(|&i| self.notes[i].start_beat)
            .fold(d.start, f64::min)
            - JUDGE_MARGIN;
        let hi = parties
            .iter()
            .map(|&i| self.voice(&self.notes[i]).end)
            .fold(d.start, f64::max)
            + JUDGE_MARGIN;
        let (before, cost) = self.assess(lo, hi, None);
        if parties.is_empty() || !before.iter().any(|x| x.same(d)) {
            return None; // nobody to move, or an earlier repair already removed it
        }
        let mut ranked: Vec<Ranked> = Vec::new();
        for &i in &parties {
            for c in self.candidates(i, d) {
                let (after, a) = self.assess(lo, hi, Some((i, c.note.as_ref())));
                let gone = !after.iter().any(|x| x.same(d));
                // The soft guard (a guide tone lost, an identity flip) — waived for leaving a note out
                // under the melody: the lead owns that pitch space (Fmaj7 under a melody on F drops
                // its 7th and sounds F6/9 — the arranger's move, not a loss).
                let soft_worse = a.soft > cost.soft + SOFT_SLACK_BEATS;
                let yields = d.melody && c.note.is_none();
                if gone && a.hard < cost.hard && (!soft_worse || yields) {
                    ranked.push(Ranked {
                        i,
                        c,
                        after: a,
                        soft_worse,
                    });
                }
            }
        }
        // Fewest hard defects, then an edit that keeps the harmony's identity (the melody's yield is
        // the fallback), a support player before the bass, the earliest rung, the smallest
        // displacement, the shortest remaining collision, the least soft cost.
        let bass = |r: &Ranked| self.notes[r.i].role == Role::Bass;
        ranked.sort_by(|x, y| {
            x.after
                .hard
                .cmp(&y.after.hard)
                .then(x.soft_worse.cmp(&y.soft_worse))
                .then(bass(x).cmp(&bass(y)))
                .then(
                    x.c.strategy
                        .rung(self.notes[x.i].role)
                        .cmp(&y.c.strategy.rung(self.notes[y.i].role)),
                )
                .then(x.c.displacement.cmp(&y.c.displacement))
                .then(x.after.beats.total_cmp(&y.after.beats))
                .then(x.after.soft.total_cmp(&y.after.soft))
        });
        // A receipt is a constraint only while it stays realizable: within the best tier of hard
        // defects, an edit that keeps every witness wins; if none does, the best edit happens and
        // the witness it costs is reported deferred.
        let mut chosen: Option<(usize, Vec<ActionId>)> = None;
        if let Some(best) = ranked
            .first()
            .map(|r| (r.after.hard, r.soft_worse, bass(r)))
        {
            for (k, r) in ranked
                .iter()
                .enumerate()
                .take_while(|(_, r)| (r.after.hard, r.soft_worse, bass(r)) == best)
            {
                let lost: Vec<ActionId> = match oracle {
                    Some((f, template)) => {
                        let after = f(&self.materialize(template, Some((r.i, r.c.note.as_ref()))));
                        now.iter()
                            .zip(&after)
                            .filter(|((_, w0), (_, w1))| *w0 && !*w1)
                            .map(|((a, _), _)| *a)
                            .collect()
                    }
                    None => Vec::new(),
                };
                if lost.is_empty() {
                    chosen = Some((k, lost));
                    break;
                }
                if chosen.is_none() {
                    chosen = Some((k, lost));
                }
            }
        }
        let (k, deferred) = chosen?;
        let r = &ranked[k];
        let n = self.notes[r.i];
        let edit = match r.c.note {
            None => {
                self.gone[r.i] = true;
                RepairEdit::Removed
            }
            Some(m) => {
                self.notes[r.i] = m;
                if m.pitch != n.pitch {
                    RepairEdit::Repitched { to: m.pitch }
                } else {
                    RepairEdit::Shortened {
                        from: n.dur_beats,
                        to: m.dur_beats,
                    }
                }
            }
        };
        self.edited[r.i] = true;
        if !deferred.is_empty() {
            if let Some((f, template)) = oracle {
                *now = f(&self.materialize(template, None));
            }
        }
        Some(VerticalRepair {
            beat: n.start_beat,
            role: n.role,
            original: n.pitch,
            edit,
            strategy: r.c.strategy,
            defect: d.text.clone(),
            deferred,
        })
    }
}

/// Repair the REAL hard vertical defects of a finished `score` in place — the smallest local edit
/// of one offending note per defect (see the module docs) — and return the ledger of every edit.
/// `receipts`, when given, is the action audit: an edit that would cost a witness loses to one that
/// does not, and one that must is reported in [`VerticalRepair::deferred`].
pub fn repair(
    score: &mut Score,
    contexts: &[HarmonicContext],
    world: &MusicWorld,
    policy: &ColorPolicy,
    receipts: Option<Receipts>,
) -> Vec<VerticalRepair> {
    let sfx: Vec<Voice> = audible_voices(score, contexts, world, MASKING_FLOOR_DB)
        .into_iter()
        .filter(|v| v.sfx)
        .collect();
    let n = score.notes.len();
    let mut w = Work {
        notes: score.notes.clone(),
        gone: vec![false; n],
        edited: vec![false; n],
        sfx,
        contexts,
        world,
        policy,
        tempo: score.tempo_bpm,
    };
    let template = receipts.map(|_| score.clone());
    let oracle = receipts.zip(template.as_ref());
    let mut now = receipts.map(|f| f(score)).unwrap_or_default();
    let mut out = Vec::new();
    for _ in 0..PASSES {
        let (defects, _) = w.assess(f64::NEG_INFINITY, f64::INFINITY, None);
        let mut progressed = false;
        for d in &defects {
            if let Some(r) = w.fix(d, oracle, &mut now) {
                out.push(r);
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    score.notes = w
        .notes
        .iter()
        .zip(&w.gone)
        .filter(|(_, &g)| !g)
        .map(|(n, _)| *n)
        .collect();
    out
}

/// The hard vertical defects `score` still sounds (the pass's own detector over the whole score):
/// what the ladder could not take — a clash between the lead and a sting, or a note no edit could
/// clean without a worse one — one description each, in time order.
pub fn residual(
    score: &Score,
    contexts: &[HarmonicContext],
    world: &MusicWorld,
    policy: &ColorPolicy,
) -> Vec<String> {
    let n = score.notes.len();
    let w = Work {
        notes: score.notes.clone(),
        gone: vec![false; n],
        edited: vec![false; n],
        sfx: audible_voices(score, contexts, world, MASKING_FLOOR_DB)
            .into_iter()
            .filter(|v| v.sfx)
            .collect(),
        contexts,
        world,
        policy,
        tempo: score.tempo_bpm,
    };
    w.assess(f64::NEG_INFINITY, f64::INFINITY, None)
        .0
        .into_iter()
        .map(|d| d.text)
        .collect()
}

/// How far a repaired realization moved from its reference, measured by DIFFING the two Scores (not
/// by trusting the repair ledger): notes are matched within (role, onset, velocity, tag) groups by
/// the least total pitch change, so an edit is counted once, as what it audibly is.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Perturbation {
    /// Notes in the reference.
    pub notes: usize,
    /// Reference notes that differ in the repaired score (removed + shortened + re-pitched).
    pub changed: usize,
    pub removed: usize,
    /// Only the duration changed.
    pub shortened: usize,
    pub repitched: usize,
    /// Total and per-role semitone displacement of the re-pitched notes.
    pub displacement: i32,
    /// Changed notes per role.
    pub by_role: BTreeMap<&'static str, usize>,
    /// Notes the repaired score has that the reference does not (a repair never adds one: 0).
    pub added: usize,
}

impl Perturbation {
    /// Diff `repaired` against `reference`.
    pub fn measure(reference: &Score, repaired: &Score) -> Perturbation {
        type Key = (&'static str, i64, u32, &'static str);
        let key = |n: &Note| -> Key {
            (
                n.role.label(),
                (n.start_beat * 1e6).round() as i64,
                n.velocity.to_bits(),
                n.prov.role_note,
            )
        };
        let group = |s: &Score| {
            let mut g: BTreeMap<Key, Vec<Note>> = BTreeMap::new();
            for n in &s.notes {
                g.entry(key(n)).or_default().push(*n);
            }
            g
        };
        let (r, q) = (group(reference), group(repaired));
        let mut p = Perturbation {
            notes: reference.notes.len(),
            ..Perturbation::default()
        };
        for (k, rs) in &r {
            let qs = q.get(k).map(Vec::as_slice).unwrap_or(&[]);
            let (pairs, extra) = best_matching(rs, qs);
            p.added += extra;
            let mut matched = vec![false; rs.len()];
            for (ri, qi) in pairs {
                matched[ri] = true;
                let (a, b) = (&rs[ri], &qs[qi]);
                if a.pitch != b.pitch {
                    p.repitched += 1;
                    p.displacement += (a.pitch - b.pitch).abs();
                    *p.by_role.entry(k.0).or_default() += 1;
                } else if a.dur_beats != b.dur_beats {
                    p.shortened += 1;
                    *p.by_role.entry(k.0).or_default() += 1;
                }
            }
            let removed = matched.iter().filter(|m| !**m).count();
            p.removed += removed;
            if removed > 0 {
                *p.by_role.entry(k.0).or_default() += removed;
            }
        }
        p.added += q
            .iter()
            .filter(|(k, _)| !r.contains_key(*k))
            .map(|(_, v)| v.len())
            .sum::<usize>();
        p.changed = p.removed + p.shortened + p.repitched;
        p
    }

    /// The share of the reference's notes that changed.
    pub fn edit_fraction(&self) -> f64 {
        if self.notes == 0 {
            0.0
        } else {
            self.changed as f64 / self.notes as f64
        }
    }

    /// A compact report (with a red warning past a tenth of the notes: a surgical pass that edits
    /// that much has stopped being surgical).
    pub fn report(&self) -> String {
        let roles: Vec<String> = ["pad", "keys", "bass", "lead"]
            .iter()
            .map(|r| format!("{r}={}", self.by_role.get(r).copied().unwrap_or(0)))
            .collect();
        let mean = if self.repitched == 0 {
            0.0
        } else {
            self.displacement as f64 / self.repitched as f64
        };
        let warn = if self.edit_fraction() > 0.10 {
            "  !! RED: more than 10% of the notes changed — investigate before accepting"
        } else {
            ""
        };
        format!(
            "perturbation (R7b -> surgical, diffed): notes={} changed={} ({:.1}%) removed={} shortened_only={} repitched={} displacement={} st (mean {:.2}) added={} | by role: {}{warn}\n",
            self.notes,
            self.changed,
            100.0 * self.edit_fraction(),
            self.removed,
            self.shortened,
            self.repitched,
            self.displacement,
            mean,
            self.added,
            roles.join(" ")
        )
    }
}

/// The injective matching of `qs` into `rs` with the least total change (pitch first, then a
/// duration difference), as `(r index, q index)` pairs, plus how many of `qs` could not be matched.
/// Groups are a chord's worth of notes, so an exhaustive search is cheap.
fn best_matching(rs: &[Note], qs: &[Note]) -> (Vec<(usize, usize)>, usize) {
    let extra = qs.len().saturating_sub(rs.len());
    let qs = &qs[..qs.len() - extra];
    let cost = |a: &Note, b: &Note| {
        4 * (a.pitch - b.pitch).unsigned_abs() + u32::from(a.dur_beats != b.dur_beats)
    };
    let mut best: (u32, Vec<(usize, usize)>) = (u32::MAX, Vec::new());
    let mut used = vec![false; rs.len()];
    let mut cur: Vec<(usize, usize)> = Vec::new();
    #[allow(clippy::too_many_arguments)]
    fn go(
        qi: usize,
        rs: &[Note],
        qs: &[Note],
        used: &mut [bool],
        cur: &mut Vec<(usize, usize)>,
        acc: u32,
        best: &mut (u32, Vec<(usize, usize)>),
        cost: &dyn Fn(&Note, &Note) -> u32,
    ) {
        if acc >= best.0 {
            return;
        }
        if qi == qs.len() {
            *best = (acc, cur.clone());
            return;
        }
        for ri in 0..rs.len() {
            if !used[ri] {
                used[ri] = true;
                cur.push((ri, qi));
                go(
                    qi + 1,
                    rs,
                    qs,
                    used,
                    cur,
                    acc + cost(&rs[ri], &qs[qi]),
                    best,
                    cost,
                );
                cur.pop();
                used[ri] = false;
            }
        }
    }
    go(0, rs, qs, &mut used, &mut cur, 0, &mut best, &cost);
    (best.1, extra)
}

#[cfg(test)]
mod tests {
    use super::super::context::analyze;
    use super::super::form::SectionKind;
    use super::super::harmony::ChordSpan;
    use super::super::ids::ActionStamp;
    use super::super::score::Provenance;
    use super::super::sonority::{EnsembleSonorityDiagnostics, VerticalClass};
    use super::super::theory::{Chord, Mode, Quality, Scale};
    use super::*;

    /// A world whose envelopes make the written length the audible one (sustain 0.8, a release of
    /// 50 ms): the controls test the pass, not a patch's decay.
    fn world() -> MusicWorld {
        let mut w = MusicWorld::black_ice();
        for p in [&mut w.pad, &mut w.keys, &mut w.bass, &mut w.lead] {
            p.adsr = (0.005, 0.1, 0.8, 0.05);
        }
        w
    }

    fn ctxs(chords: &[(f64, f32, Chord)]) -> Vec<HarmonicContext> {
        let spans: Vec<ChordSpan> = chords
            .iter()
            .map(|&(s, d, c)| ChordSpan::test(s, d, c))
            .collect();
        analyze(&spans, &Scale::new(0, Mode::Ionian))
    }

    fn note(
        role: Role,
        pitch: Midi,
        start: f64,
        dur: f32,
        f: PitchFunction,
        tag: &'static str,
    ) -> Note {
        let mut n = Note::new(
            start,
            dur,
            pitch,
            0.8,
            role,
            Provenance::new(SectionKind::A),
        );
        n.function = Some(f);
        n.prov.role_note = tag;
        n
    }

    fn score(notes: Vec<Note>) -> Score {
        let mut s = Score::new(120.0, 4.0, 16.0);
        s.notes = notes;
        s
    }

    const CT: PitchFunction = PitchFunction::ChordTone;

    fn audible_unowned(s: &Score, c: &[HarmonicContext]) -> usize {
        let d = EnsembleSonorityDiagnostics::measure_audible_at(
            s,
            c,
            &world(),
            &ColorPolicy::lenient(),
            &[],
            MASKING_FLOOR_DB,
        );
        d.unowned_m2 + d.unowned_m9
    }

    /// Lead C5 on beat 1, a keys stab B4–E5–G5 on beat 3 of the same Cmaj7: a minor 2nd IF they
    /// sounded together — they never do. The audit and the surgical pass hear nothing; Round VIII's
    /// joint scorer, which stretches the stab over the whole harmony window, counts a collision.
    #[test]
    fn a_stab_is_judged_only_while_it_sounds() {
        let c = ctxs(&[(0.0, 4.0, Chord::new(0, Quality::Maj7))]);
        let lead = note(Role::Lead, 72, 0.0, 1.0, CT, "melody");
        let stab: Vec<Note> = [59, 64, 67]
            .iter()
            .map(|&p| note(Role::Keys, p + 12, 2.0, 0.45, CT, "comp"))
            .collect();
        let mut s = score([vec![lead], stab.clone()].concat());
        let before = s.notes.clone();
        assert_eq!(audible_unowned(&s, &c), 0, "they never overlap");
        let r = repair(&mut s, &c, &world(), &ColorPolicy::lenient(), None);
        assert!(r.is_empty(), "nothing to repair: {r:?}");
        assert_eq!(format!("{:?}", s.notes), format!("{before:?}"));
        // The negative control: R8's union cost models the keys candidate over [0, 4).
        use super::super::sonority::tension_specs;
        use super::super::support::{union_cost, Carriers};
        let lead_voice = voice_of(&lead);
        let keys: Vec<Midi> = stab.iter().map(|n| n.pitch).collect();
        let u = union_cost(
            &c[0],
            0.0,
            4.0,
            &[],
            &keys,
            &[lead_voice],
            None,
            &ColorPolicy::lenient(),
            &tension_specs(&c[0]),
            Carriers {
                pad: false,
                keys: true,
                pad_on: false,
                keys_on: true,
                pad_all: false,
            },
            &[],
        );
        assert!(
            u.unowned >= 1,
            "the whole-window model invents the collision R8 revoiced around: {u:?}"
        );
    }

    /// One keys note makes one real hard clash (E5 struck with the melody's F5 over Fmaj7): that
    /// note, and only that note, changes; every neighbouring voicing is the R7b one.
    #[test]
    fn one_clash_one_edit() {
        let c = ctxs(&[
            (0.0, 4.0, Chord::new(0, Quality::Maj7)),
            (4.0, 4.0, Chord::new(5, Quality::Maj7)),
            (8.0, 4.0, Chord::new(7, Quality::Dom7)),
        ]);
        let mut notes = Vec::new();
        for (t, pad, root) in [
            (0.0, [60, 64, 67, 71], 36),
            (4.0, [60, 65, 69, 72], 41),
            (8.0, [59, 62, 65, 67], 43),
        ] {
            notes.extend(pad.iter().map(|&p| note(Role::Pad, p, t, 3.9, CT, "pad")));
            notes.push(note(Role::Bass, root, t, 3.9, CT, "root"));
        }
        notes.push(note(Role::Lead, 79, 0.0, 3.9, CT, "melody"));
        notes.push(note(Role::Lead, 77, 4.0, 3.9, CT, "melody"));
        notes.push(note(Role::Lead, 79, 8.0, 3.9, CT, "melody"));
        // The keys' Fmaj7 stab: A4 C5 E5 — the E5 a semitone under the melody's F5.
        for p in [69, 72, 76] {
            notes.push(note(Role::Keys, p, 4.0, 1.5, CT, "comp"));
        }
        let mut s = score(notes);
        let before = s.notes.clone();
        let r = repair(&mut s, &c, &world(), &ColorPolicy::lenient(), None);
        assert_eq!(r.len(), 1, "{r:#?}");
        assert_eq!((r[0].role, r[0].original, r[0].beat), (Role::Keys, 76, 4.0));
        let p = Perturbation::measure(&score(before.clone()), &s);
        assert_eq!(p.changed, 1, "{}", p.report());
        // Everything else is note-for-note the R7b score.
        let kept: Vec<String> = before
            .iter()
            .filter(|n| !(n.role == Role::Keys && n.pitch == 76))
            .map(|n| format!("{n:?}"))
            .collect();
        for k in &kept {
            assert!(
                s.notes.iter().any(|n| &format!("{n:?}") == k),
                "{k} changed"
            );
        }
        assert_eq!(audible_unowned(&s, &c), 0);
    }

    /// The keys' C5 sits a semitone above the melody's B4 (and a minor 9th above the pad's B3), in
    /// a legal B4–C5–E5 cluster. Every replacement leaves a hard defect (C4 grinds against the pad's
    /// B3, C6 is a minor 9th over the melody, the nearby Cmaj7 tones are already in the stab);
    /// leaving it out leaves none — so it is left out.
    #[test]
    fn silence_beats_a_worse_replacement() {
        let c = ctxs(&[(0.0, 4.0, Chord::new(0, Quality::Maj7))]);
        let mut notes = vec![
            note(Role::Lead, 71, 0.0, 3.9, CT, "melody"),
            note(Role::Bass, 36, 0.0, 3.9, CT, "root"),
            note(Role::Pad, 59, 0.0, 3.9, CT, "pad"),
            note(Role::Pad, 64, 0.0, 3.9, CT, "pad"),
        ];
        const C5: usize = 5;
        for p in [71, 72, 76] {
            notes.push(note(Role::Keys, p, 0.0, 2.0, CT, "comp"));
        }
        let mut s = score(notes);
        let n = s.notes.len();
        let w = Work {
            notes: s.notes.clone(),
            gone: vec![false; n],
            edited: vec![false; n],
            sfx: Vec::new(),
            contexts: &c,
            world: &world(),
            policy: &ColorPolicy::lenient(),
            tempo: 120.0,
        };
        let (defects, before) = w.assess(-10.0, 20.0, None);
        assert_eq!(s.notes[C5].pitch, 72);
        let d = defects
            .iter()
            .find(|d| d.kind == Kind::Clash && d.notes.contains(&Some(C5)))
            .expect("the keys C5 clashes with the melody");
        let (_, silent) = w.assess(-10.0, 20.0, Some((C5, None)));
        assert_eq!((before.hard, silent.hard), (2, 0), "silence clears both");
        let replacements: Vec<Candidate> = w
            .candidates(C5, d)
            .into_iter()
            .filter(|x| x.note.is_some())
            .collect();
        assert!(!replacements.is_empty());
        for cand in &replacements {
            let (after, a) = w.assess(-10.0, 20.0, Some((C5, cand.note.as_ref())));
            assert!(
                after.iter().any(|x| x.same(d)) || a.hard > silent.hard,
                "{:?} is as clean as silence",
                cand.note.map(|m| note_name(m.pitch))
            );
        }
        let r = repair(&mut s, &c, &world(), &ColorPolicy::lenient(), None);
        assert_eq!(r.len(), 1, "{r:#?}");
        assert_eq!(r[0].edit, RepairEdit::Removed);
        assert_eq!(r[0].strategy, RepairStrategy::Omit);
        assert_eq!(audible_unowned(&s, &c), 0);
    }

    /// An action-stamped keys note IS the defect (C5 a semitone above the melody's B4) and the
    /// witness wants that very pitch class: the bad note is not kept to keep the paperwork green —
    /// the repair happens and the action is reported deferred. When a clean edit keeps the receipt
    /// (the witness only wants the stamped note to sound), that edit wins even from a later rung.
    #[test]
    fn a_receipt_never_keeps_a_bad_note() {
        let c = ctxs(&[(0.0, 4.0, Chord::new(0, Quality::Maj7))]);
        let mut notes = vec![
            note(Role::Lead, 71, 0.0, 3.9, CT, "melody"),
            note(Role::Bass, 36, 0.0, 3.9, CT, "root"),
            note(Role::Pad, 59, 0.0, 3.9, CT, "pad"),
            note(Role::Pad, 64, 0.0, 3.9, CT, "pad"),
            note(Role::Keys, 72, 0.0, 2.0, CT, "comp"),
        ];
        notes[4].prov.actions = ActionStamp::of(ActionId(7));
        let stamped = |s: &Score| {
            s.notes
                .iter()
                .filter(|n| n.prov.actions.has(ActionId(7)))
                .copied()
                .collect::<Vec<_>>()
        };
        // (a) The witness needs pitch class C on the stamped note.
        let pc_witness = |s: &Score| {
            vec![(
                ActionId(7),
                stamped(s).iter().any(|n| pitch_class(n.pitch) == 0),
            )]
        };
        let mut s = score(notes.clone());
        let r = repair(
            &mut s,
            &c,
            &world(),
            &ColorPolicy::lenient(),
            Some(&pc_witness),
        );
        assert_eq!(r.len(), 1, "{r:#?}");
        assert_eq!(
            r[0].deferred,
            vec![ActionId(7)],
            "the surrendered receipt is reported"
        );
        assert!(
            !s.notes
                .iter()
                .any(|n| n.role == Role::Keys && n.pitch == 72),
            "the C5 above the melody does not survive"
        );
        assert_eq!(audible_unowned(&s, &c), 0);
        // (b) The witness only needs the stamped note to sound: a re-pitch keeps it and beats omission.
        let any_witness = |s: &Score| vec![(ActionId(7), !stamped(s).is_empty())];
        let mut s = score(notes);
        let r = repair(
            &mut s,
            &c,
            &world(),
            &ColorPolicy::lenient(),
            Some(&any_witness),
        );
        assert_eq!(r.len(), 1, "{r:#?}");
        assert!(r[0].deferred.is_empty(), "{r:#?}");
        assert!(matches!(r[0].edit, RepairEdit::Repitched { .. }), "{r:#?}");
    }

    /// A four-chord bed with ONE local collision (the third harmony's pad E5 under the melody's F5):
    /// the fix touches that voicing only; the neighbouring pad voicings are the R7b ones.
    #[test]
    fn one_collision_does_not_move_the_bed() {
        let c = ctxs(&[
            (0.0, 4.0, Chord::new(0, Quality::Maj7)),
            (4.0, 4.0, Chord::new(9, Quality::Min7)),
            (8.0, 4.0, Chord::new(5, Quality::Maj7)),
            (12.0, 4.0, Chord::new(7, Quality::Dom7)),
        ]);
        let pads: [(f64, [Midi; 4]); 4] = [
            (0.0, [64, 67, 71, 74]),
            (4.0, [64, 67, 72, 76]),
            (8.0, [60, 65, 69, 76]),
            (12.0, [65, 67, 71, 74]),
        ];
        let mut notes = Vec::new();
        for (t, v) in pads {
            notes.extend(v.iter().map(|&p| note(Role::Pad, p, t, 3.9, CT, "pad")));
        }
        for (t, p, b) in [(0.0, 79, 36), (4.0, 79, 45), (8.0, 77, 41), (12.0, 77, 43)] {
            notes.push(note(Role::Lead, p, t, 3.9, CT, "melody"));
            notes.push(note(Role::Bass, b, t, 3.9, CT, "root"));
        }
        let mut s = score(notes);
        let before = s.notes.clone();
        let r = repair(&mut s, &c, &world(), &ColorPolicy::lenient(), None);
        assert_eq!(r.len(), 1, "{r:#?}");
        assert_eq!((r[0].role, r[0].beat, r[0].original), (Role::Pad, 8.0, 76));
        let pad_at = |notes: &[Note], t: f64| -> Vec<Midi> {
            let mut v: Vec<Midi> = notes
                .iter()
                .filter(|n| n.role == Role::Pad && (n.start_beat - t).abs() < 1e-9)
                .map(|n| n.pitch)
                .collect();
            v.sort_unstable();
            v
        };
        for t in [0.0, 4.0, 12.0] {
            assert_eq!(
                pad_at(&s.notes, t),
                pad_at(&before, t),
                "the bed at {t} moved"
            );
        }
        let changed: Vec<Midi> = pad_at(&before, 8.0)
            .into_iter()
            .filter(|p| !pad_at(&s.notes, 8.0).contains(p))
            .collect();
        assert_eq!(changed, vec![76], "one voice of the local voicing");
        assert_eq!(audible_unowned(&s, &c), 0);
    }

    /// The lead sings the 9th (D over Cmaj7) in an ensemble unison line and the bass copies it an
    /// octave down — a lead extension as the floor. The bass becomes a valid floor again; the lead
    /// and the rest of the line are untouched.
    #[test]
    fn a_unison_bass_never_floors_the_leads_ninth() {
        let c = ctxs(&[(0.0, 4.0, Chord::new(0, Quality::Maj7))]);
        let mut notes = vec![
            note(Role::Pad, 64, 0.0, 3.9, CT, "pad"),
            note(Role::Pad, 67, 0.0, 3.9, CT, "pad"),
            note(Role::Bass, 36, 0.0, 1.0, CT, "root"),
        ];
        for (t, p, f) in [
            (1.0, 74, PitchFunction::LicensedExtension),
            (2.0, 72, CT),
            (3.0, 67, CT),
        ] {
            notes.push(note(Role::Lead, p, t, 0.95, f, "melody"));
            let mut b = note(Role::Bass, p - 36, t, 0.95, f, "unison");
            b.prov.motif_xform = Some("unison");
            notes.push(b);
        }
        let mut s = score(notes);
        let before = s.notes.clone();
        let d0 = EnsembleSonorityDiagnostics::measure(&s, &c, &ColorPolicy::lenient(), &[]);
        assert_eq!(
            d0.bass_function_violations, 1,
            "the control: a 9th as the floor"
        );
        let r = repair(&mut s, &c, &world(), &ColorPolicy::lenient(), None);
        assert_eq!(r.len(), 1, "{r:#?}");
        assert_eq!((r[0].role, r[0].original), (Role::Bass, 38));
        let RepairEdit::Repitched { to } = r[0].edit else {
            panic!("the floor is re-pitched, not dropped: {r:#?}");
        };
        assert!(
            BassFunction::of(&c[0].chord, to).is_foundation(),
            "{}",
            note_name(to)
        );
        let d = EnsembleSonorityDiagnostics::measure(&s, &c, &ColorPolicy::lenient(), &[]);
        assert_eq!(d.bass_function_violations, 0);
        let leads = |v: &[Note]| {
            format!(
                "{:?}",
                v.iter()
                    .filter(|n| n.role == Role::Lead)
                    .collect::<Vec<_>>()
            )
        };
        assert_eq!(
            leads(&s.notes),
            leads(&before),
            "the melody is never touched"
        );
    }

    /// Owned dissonance is music, not damage: a b9–8 suspension over the bass that resolves, and a
    /// chromatic passing tone in the lead brushing a pad tone, both stay exactly as written.
    #[test]
    fn owned_tension_is_not_sterilized() {
        let c = ctxs(&[(0.0, 4.0, Chord::new(0, Quality::Dom7))]);
        let notes = vec![
            note(Role::Bass, 48, 0.0, 3.9, CT, "root"),
            note(Role::Pad, 61, 0.0, 1.0, PitchFunction::Suspension, "pad"),
            note(Role::Pad, 60, 1.0, 2.9, CT, "pad"),
            note(Role::Pad, 76, 0.0, 3.9, CT, "pad"),
            note(Role::Lead, 79, 0.0, 2.0, CT, "melody"),
            note(
                Role::Lead,
                75,
                2.0,
                0.25,
                PitchFunction::ChromaticPassing,
                "melody",
            ),
            note(Role::Lead, 76, 2.25, 1.5, CT, "melody"),
        ];
        let mut s = score(notes);
        let before = format!("{:?}", s.notes);
        let d = EnsembleSonorityDiagnostics::measure(&s, &c, &ColorPolicy::lenient(), &[]);
        assert!(
            d.owned.contains_key(VerticalClass::Suspension.label()),
            "{:?}",
            d.owned
        );
        assert!(
            d.owned.contains_key(VerticalClass::LinearCollision.label()),
            "{:?}",
            d.owned
        );
        let r = repair(&mut s, &c, &world(), &ColorPolicy::lenient(), None);
        assert!(r.is_empty(), "{r:#?}");
        assert_eq!(format!("{:?}", s.notes), before);
    }

    /// The perturbation report is a DIFF, not the ledger: re-pitch, release and removal in one chord
    /// are each counted once, as what they audibly are.
    #[test]
    fn the_perturbation_report_diffs_the_scores() {
        let a: Vec<Note> = [60, 64, 67, 71]
            .iter()
            .map(|&p| note(Role::Pad, p, 0.0, 3.9, CT, "pad"))
            .collect();
        let mut b = a.clone();
        b.remove(1); // E4 left out
        b[0].pitch = 48; // C4 -> C3
        b[2].dur_beats = 3.0; // B4 released early
        let p = Perturbation::measure(&score(a), &score(b));
        assert_eq!(
            (
                p.notes,
                p.changed,
                p.removed,
                p.repitched,
                p.shortened,
                p.displacement,
                p.added
            ),
            (4, 3, 1, 1, 1, 12, 0)
        );
    }
}
