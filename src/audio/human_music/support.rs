//! **The joint support path** — the pad and the keys voiced as ONE harmonic decision (Round VIII).
//!
//! R7b solved the pad's voice path and the keys' voice path as two separate Viterbis over the same
//! candidate families and overlapping registers (pad 52–79, keys 58–84): the keys heard the lead
//! (softly, by pitch class), the pad heard nobody, and neither heard the other. Measured on the
//! flagship, the pad↔keys pair carried 56–67 % of every unowned minor 2nd / minor 9th the band
//! sounded. Here the two paths are ONE Viterbi over (pad voicing, keys voicing) pairs per harmony:
//! each pair is scored on the UNION it sounds — against itself, against the other player, and against
//! everything the ledger already holds (the lead, the bass, the keys' material lines) — with the same
//! [`classify_clash`] the audit applies to the finished Score. The key is lexicographic, never one
//! "quality" scalar: unowned collisions and missing identity tones first (the hard tier), then the
//! backbone's gesture targets and the guide-tone line (the per-role tiers the independent paths
//! already honour), then the weighted rest (motion, retention, spacing, … plus the union's soft
//! costs: a colour owned twice, colour over the policy's budget, a detuned pad doubling a keys pitch
//! exactly, crowding, low-register mud, a tension below its register floor).
//!
//! The candidate sets are the voicing engine's own (every pitch a chord tone or licensed tension,
//! every candidate carrying its guide tones); the joint search only chooses among them. Pairs are
//! pruned to [`JOINT_BEAM`] per harmony by their static cost, and the pair the INDEPENDENT paths
//! would have chosen is always kept, so the joint path is never worse than the control on its own
//! lexicographic key. The per-role [`super::voicing::voice_path`] stays as that control.

use super::action::{ActionKind, Agent};
use super::context::HarmonicContext;
use super::form::BEATS_PER_BAR;
use super::harmonic_state::HarmonicEnsembleState;
use super::performance::{KeysMode, PadMode, PerformancePlan};
use super::score::{Note, PitchFunction, Role};
use super::sonority::{
    classify_clash, core_pcs, identity_pcs, is_selected_color, is_suspension, low_interval_limit,
    tension_specs, Clash, ColorPolicy, SonorityPlan, TensionSpec, VerticalClass, Voice,
    MIN_OVERLAP_BEATS,
};
use super::theory::{pitch_class, Midi};
use super::voicing::{
    allowed, evaluate_path, gesture_tier, keys_steps, keys_steps_with, pad_steps, pad_steps_with,
    step_cost, voice_path, weigh, PathStep, PathWeights, RolePath, VoiceRange, VoicingCandidate,
    VoicingShape,
};

/// The share of a harmony's window a committed voice must cover to carry a pitch class.
pub const CARRY_COVERAGE: f64 = 0.9;

/// Complement variants kept per role per harmony (after pre-scoring against what already sounds).
pub const COMPLEMENT_KEEP: usize = 16;
/// How far above its window a complement may lift one voice (an octave move that clears the lead).
const LIFT_HEADROOM: Midi = 7;

/// Candidate pairs kept per harmony after static pruning (the DP is `O(steps · BEAM²)`).
pub const JOINT_BEAM: usize = 48;

/// The union a (pad, keys) pair sounds over one harmony, as an inspectable cost vector.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UnionCost {
    /// Unowned minor 2nds / 9ths: inside either voicing, pad↔keys, and against what already sounds.
    pub unowned: u32,
    /// Identity tones of the harmony (guide tones, an altered 5th) the whole band lacks.
    pub identity_missing: u32,
    /// A harmonic edit here (a recolour, a substitution, an applied dominant) whose NEW pitch class
    /// nobody sounds — its witness would fail: unstamped carriers are load-bearing.
    pub witness_missing: u32,
    /// Pairs under their low-interval limit (across roles too).
    pub low_register: f32,
    /// Colour pitch classes owned by more than one player.
    pub duplicate_color: f32,
    /// Distinct colour tones beyond the policy's budget.
    pub over_color: f32,
    /// Exact MIDI pitches the pad and the keys both sound (a detuned pad beating against a pure one).
    pub exact_unison: f32,
    /// Sounding voices beyond the policy's density limit.
    pub crowd: f32,
    /// Colour tones below their tension's register floor.
    pub tension_low: f32,
    /// The bass leaves the root in this harmony (never states it here) and no carrier holds it: the
    /// rootless band over that floor spells another chord.
    pub root_missing: f32,
}

/// Weights of the union's soft terms (they join the per-role weighted tier).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnionWeights {
    pub low_register: f32,
    pub duplicate_color: f32,
    pub over_color: f32,
    pub exact_unison: f32,
    pub crowd: f32,
    pub tension_low: f32,
    pub root_missing: f32,
}

impl Default for UnionWeights {
    fn default() -> Self {
        UnionWeights {
            low_register: 2.0,
            duplicate_color: 1.5,
            over_color: 2.0,
            exact_unison: 1.0,
            crowd: 1.0,
            tension_low: 1.0,
            root_missing: 2.0,
        }
    }
}

impl UnionCost {
    /// The hard tier: unowned collisions plus missing identity tones.
    pub fn hard(&self) -> u32 {
        self.unowned + self.identity_missing
    }

    /// The weighted soft terms.
    pub fn soft(&self, w: &UnionWeights) -> f64 {
        (w.low_register * self.low_register
            + w.duplicate_color * self.duplicate_color
            + w.over_color * self.over_color
            + w.exact_unison * self.exact_unison
            + w.crowd * self.crowd
            + w.tension_low * self.tension_low
            + w.root_missing * self.root_missing) as f64
    }

    fn add(&mut self, o: &UnionCost) {
        self.unowned += o.unowned;
        self.identity_missing += o.identity_missing;
        self.witness_missing += o.witness_missing;
        self.low_register += o.low_register;
        self.duplicate_color += o.duplicate_color;
        self.over_color += o.over_color;
        self.exact_unison += o.exact_unison;
        self.crowd += o.crowd;
        self.tension_low += o.tension_low;
        self.root_missing += o.root_missing;
    }
}

/// The joint key: a planned harmonic edit's witness first (the plan is law — a recolour nobody
/// sounds did not happen), then the union's hard tier, then the backbone gestures and the guide-tone
/// line (summed over both roles), then everything weighted. Each tier is additive along the path, so
/// the Viterbi is exact for this order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointKey {
    pub witness: u32,
    pub vertical: u32,
    pub gesture: i64,
    pub guide_breaks: u32,
    pub weighted: f64,
}

impl JointKey {
    const ZERO: JointKey = JointKey {
        witness: 0,
        vertical: 0,
        gesture: 0,
        guide_breaks: 0,
        weighted: 0.0,
    };

    fn plus(self, o: JointKey) -> JointKey {
        JointKey {
            witness: self.witness + o.witness,
            vertical: self.vertical + o.vertical,
            gesture: self.gesture + o.gesture,
            guide_breaks: self.guide_breaks + o.guide_breaks,
            weighted: self.weighted + o.weighted,
        }
    }

    fn tiers(&self) -> (u32, u32, i64, u32) {
        (self.witness, self.vertical, self.gesture, self.guide_breaks)
    }

    /// Strictly better (lexicographically smaller).
    pub fn lt(&self, o: &JointKey) -> bool {
        self.tiers() < o.tiers() || (self.tiers() == o.tiers() && self.weighted < o.weighted)
    }
}

/// What the joint solve did, for the lab and the tests.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct JointReport {
    /// Harmonies where the pad, the keys, or both sound.
    pub steps: usize,
    /// Harmonies where both sound.
    pub both: usize,
    /// Candidate pairs scored (before pruning).
    pub pairs_scored: usize,
    /// The union cost summed along the INDEPENDENT paths' choice (the control) and the joint choice.
    pub independent: UnionCost,
    pub joint: UnionCost,
    /// Harmonies where the joint choice differs from the independent one.
    pub changed: usize,
    /// The joint path's summed key and the independent choice's key under the same joint cost.
    pub joint_key: Option<JointKey>,
    pub independent_key: Option<JointKey>,
}

impl JointReport {
    /// A compact human-readable report.
    pub fn report(&self) -> String {
        let u = |c: &UnionCost| {
            format!(
                "unowned={} identity_missing={} witness_missing={} low={} dup_colour={} over_colour={} exact_unison={} crowd={} tension_low={} root_missing={}",
                c.unowned,
                c.identity_missing,
                c.witness_missing,
                c.low_register,
                c.duplicate_color,
                c.over_color,
                c.exact_unison,
                c.crowd,
                c.tension_low,
                c.root_missing
            )
        };
        format!(
            "joint support path: steps={} both={} pairs_scored={} changed={}\n  independent pair union: {}\n  joint pair union:       {}\n",
            self.steps,
            self.both,
            self.pairs_scored,
            self.changed,
            u(&self.independent),
            u(&self.joint)
        )
    }
}

/// The pad's and the keys' paths from ONE joint solve, plus its report.
#[derive(Debug, Clone)]
pub struct SupportPaths {
    pub pad: RolePath,
    pub keys: RolePath,
    pub report: JointReport,
}

fn function_over(ctx: &HarmonicContext, p: Midi) -> Option<PitchFunction> {
    let pc = pitch_class(p);
    if ctx.chord.contains_pc(pc) {
        Some(PitchFunction::ChordTone)
    } else if ctx.palette.tensions.contains(&pc) {
        Some(PitchFunction::LicensedExtension)
    } else {
        None
    }
}

fn support_voice(
    ctx: &HarmonicContext,
    role: Role,
    p: Midi,
    a: f64,
    b: f64,
    tag: &'static str,
) -> Voice {
    Voice {
        role,
        pitch: p,
        start: a,
        end: b,
        function: function_over(ctx, p),
        tag,
        resolves: true,
        resolves_to: None,
        unison: false,
        written_end: b,
        sfx: false,
        owned: false,
    }
}

/// Who CARRIES the harmony through its window — the voices whose pitch classes count toward the
/// band's identity tones. A stab is a punctuation, not a bed: the keys' comping counts only when the
/// keys sustain there (a Sustain bar, a Hold) or when the pad is not sounding at all; a player the
/// Stage has out carries nothing; a committed voice counts for a pitch class when it covers
/// [`CARRY_COVERAGE`] of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Carriers {
    pub pad: bool,
    pub keys: bool,
    /// Whether the pad / the keys sound here at all (the Stage has them on).
    pub pad_on: bool,
    pub keys_on: bool,
    /// Whether the pad sounds its WHOLE voicing here (a common-tone carry sounds only what it keeps
    /// and its guide tones).
    pub pad_all: bool,
}

/// The union cost of the pad sounding `pad` and the keys sounding `keys` over the harmony `ctx`
/// (window `[a, b)`) against `others` (what already sounds there). The ONE vertical theory
/// ([`classify_clash`], [`low_interval_limit`], the identity tones, the colour policy) — the same
/// questions the audit asks of the finished Score. `carriers` says whose pitches hold the harmony
/// through the window for the identity check; `must` lists the new pitch classes of a harmonic edit
/// here, one of which somebody must sound (the edit's witness).
#[allow(clippy::too_many_arguments)]
pub fn union_cost(
    ctx: &HarmonicContext,
    a: f64,
    b: f64,
    pad: &[Midi],
    keys: &[Midi],
    others: &[Voice],
    plan: Option<&SonorityPlan>,
    policy: &ColorPolicy,
    specs: &[TensionSpec],
    carriers: Carriers,
    must: &[i32],
) -> UnionCost {
    let mut c = UnionCost::default();
    let cand: Vec<Voice> = pad
        .iter()
        .map(|&p| support_voice(ctx, Role::Pad, p, a, b, "pad"))
        .chain(
            keys.iter()
                .map(|&p| support_voice(ctx, Role::Keys, p, a, b, "comp")),
        )
        .collect();
    let judge = |x: &Voice, y: &Voice, c: &mut UnionCost| {
        if x.pitch == y.pitch {
            return;
        }
        if Clash::of(x.pitch, y.pitch).is_some()
            && classify_clash(ctx, x, y, plan) == VerticalClass::UnownedCollision
        {
            c.unowned += 1;
        }
        let (lo, hi) = if x.pitch < y.pitch {
            (x.pitch, y.pitch)
        } else {
            (y.pitch, x.pitch)
        };
        if let Some(limit) = low_interval_limit(hi - lo) {
            if lo < limit && !x.is_linear() && !y.is_linear() {
                c.low_register += 1.0;
            }
        }
    };
    for (i, x) in cand.iter().enumerate() {
        for y in &cand[i + 1..] {
            judge(x, y, &mut c);
        }
        for y in others {
            let overlap = x.end.min(y.end) - x.start.max(y.start);
            if overlap >= MIN_OVERLAP_BEATS - 1e-9 {
                judge(x, y, &mut c);
            }
        }
    }
    // What the band, taken whole, rests on.
    let resting_others: Vec<&Voice> = others
        .iter()
        .filter(|v| !v.sfx && !v.is_linear() && !v.function.is_some_and(is_suspension))
        .collect();
    let mut pcs: Vec<(i32, Role)> = cand
        .iter()
        .map(|v| (pitch_class(v.pitch), v.role))
        .chain(
            resting_others
                .iter()
                .map(|v| (pitch_class(v.pitch), v.role)),
        )
        .collect();
    pcs.sort_by_key(|x| (x.0, x.1.label()));
    pcs.dedup();
    // Identity is carried, not glimpsed: only the carriers' pitch classes, and a committed voice's
    // where it covers half the window.
    let window = (b - a).max(1e-6);
    let carried = |g: i32| -> bool {
        (carriers.pad && pad.iter().any(|&p| pitch_class(p) == g))
            || (carriers.keys && keys.iter().any(|&p| pitch_class(p) == g))
            || resting_others
                .iter()
                .filter(|v| pitch_class(v.pitch) == g)
                .map(|v| (v.end.min(b) - v.start.max(a)).max(0.0))
                .sum::<f64>()
                >= CARRY_COVERAGE * window
    };
    c.identity_missing = identity_pcs(&ctx.chord)
        .iter()
        .filter(|&&g| !carried(g))
        .count() as u32;
    // A harmonic edit's witness: any sounding note (a player the Stage has on) carrying a new pitch
    // class of the edited chord.
    if !must.is_empty() {
        let sounds = |g: i32| {
            (carriers.pad_on
                && (carriers.pad_all || ctx.palette.guide_tones.contains(&g))
                && pad.iter().any(|&p| pitch_class(p) == g))
                || (carriers.keys && keys.iter().any(|&p| pitch_class(p) == g))
                || others.iter().any(|v| pitch_class(v.pitch) == g)
        };
        if !must.iter().any(|&g| sounds(g)) {
            c.witness_missing = 1;
        }
    }
    // The floor: a bass that never states the root here, under a band that does not carry it.
    let root = ctx.chord.root_pc;
    let bass: Vec<&&Voice> = resting_others
        .iter()
        .filter(|v| v.role == Role::Bass)
        .collect();
    let bass_leaves_root = !bass.is_empty()
        && !bass.iter().any(|v| pitch_class(v.pitch) == root)
        && !bass
            .iter()
            .any(|v| v.function == Some(PitchFunction::PedalTone) || v.tag == "pedal");
    if bass_leaves_root && !carried(root) {
        c.root_missing = 1.0;
    }
    let core = core_pcs(&ctx.chord);
    let mut colors: Vec<i32> = pcs
        .iter()
        .map(|x| x.0)
        .filter(|pc| !core.contains(pc))
        .collect();
    colors.dedup();
    for &pc in &colors {
        let owners = pcs.iter().filter(|x| x.0 == pc).count();
        if owners > 1 {
            c.duplicate_color += (owners - 1) as f32;
        }
    }
    c.over_color = colors
        .iter()
        .filter(|&&pc| is_selected_color(&ctx.chord, pc))
        .count()
        .saturating_sub(policy.color_budget) as f32;
    c.exact_unison = pad.iter().filter(|p| keys.contains(p)).count() as f32;
    let busiest = others
        .iter()
        .map(|v| {
            others
                .iter()
                .filter(|w| w.start <= v.start + 1e-6 && w.end > v.start + 1e-6)
                .count()
        })
        .max()
        .unwrap_or(0);
    c.crowd = (pad.len() + keys.len() + busiest).saturating_sub(policy.max_voices) as f32;
    c.tension_low = cand
        .iter()
        .filter(|v| {
            let pc = pitch_class(v.pitch);
            specs.iter().any(|t| t.pc == pc && v.pitch < t.min_register)
        })
        .count() as f32;
    c
}

/// The complement variants of `base` over `ctx` for a role whose window is `range`: each candidate
/// with one voice moved an octave (up to [`LIFT_HEADROOM`] above the window, never below it), one
/// voice dropped (down to two voices), one guide-tone voice swapped for the nearest non-guide
/// allowed pitch within a minor 3rd, or — where the candidate lacks an identity tone (the ♭5 a
/// m7♭5 shell never carries) — one non-identity voice swapped for that tone at its nearest place. Pre-scored alone against `others` (the ledger's voices here) —
/// unowned collisions plus, for a player that CARRIES the harmony, the identity tones it would shed
/// that no committed voice carries; then distance from the base's register — and the best
/// [`COMPLEMENT_KEEP`] returned. The joint key still demands the BAND hold every identity tone.
#[allow(clippy::too_many_arguments)]
fn complements(
    ctx: &HarmonicContext,
    role: Role,
    base: &[VoicingCandidate],
    range: &VoiceRange,
    a: f64,
    b: f64,
    others: &[Voice],
    plan: Option<&SonorityPlan>,
    carrier: bool,
    must: &[i32],
) -> Vec<VoicingCandidate> {
    let guides = &ctx.palette.guide_tones;
    let window = (b - a).max(1e-6);
    let carried_elsewhere = |g: i32| {
        others
            .iter()
            .filter(|v| {
                !v.sfx
                    && !v.is_linear()
                    && !v.function.is_some_and(is_suspension)
                    && pitch_class(v.pitch) == g
            })
            .map(|v| (v.end.min(b) - v.start.max(a)).max(0.0))
            .sum::<f64>()
            >= CARRY_COVERAGE * window
    };
    let identity = identity_pcs(&ctx.chord);
    let mut seen: Vec<Vec<Midi>> = base.iter().map(|c| c.voices.clone()).collect();
    let mut out: Vec<(u32, i32, Vec<Midi>)> = Vec::new();
    let tag = if role == Role::Pad { "pad" } else { "comp" };
    let mut offer = |mut v: Vec<Midi>, origin: &[Midi], out: &mut Vec<(u32, i32, Vec<Midi>)>| {
        v.sort_unstable();
        v.dedup();
        // The role's own floor holds for a complement too: its voice count, and the upper layer's
        // top (the pad's `pad_upper` range is the audible witness of a Thicken).
        if v.len() < range.min_voices.max(2)
            || v[0] < range.low
            || v[v.len() - 1] > range.high + LIFT_HEADROOM
            || range.min_top.is_some_and(|t| v[v.len() - 1] < t)
            || seen.contains(&v)
        {
            return;
        }
        seen.push(v.clone());
        let voices: Vec<Voice> = v
            .iter()
            .map(|&p| support_voice(ctx, role, p, a, b, tag))
            .collect();
        let mut clash = 0u32;
        for (i, x) in voices.iter().enumerate() {
            for y in voices[i + 1..].iter().chain(others.iter()) {
                let overlap = x.end.min(y.end) - x.start.max(y.start);
                if x.pitch != y.pitch
                    && overlap >= MIN_OVERLAP_BEATS - 1e-9
                    && Clash::of(x.pitch, y.pitch).is_some()
                    && classify_clash(ctx, x, y, plan) == VerticalClass::UnownedCollision
                {
                    clash += 1;
                }
            }
        }
        // A harmonic edit's new pitch class, shed with nobody else sounding it, costs its witness.
        clash += must
            .iter()
            .filter(|&&g| {
                origin.iter().any(|&p| pitch_class(p) == g)
                    && !v.iter().any(|&p| pitch_class(p) == g)
                    && !others.iter().any(|o| pitch_class(o.pitch) == g)
            })
            .count() as u32;
        if carrier {
            clash += identity
                .iter()
                .filter(|&&g| {
                    origin.iter().any(|&p| pitch_class(p) == g)
                        && !v.iter().any(|&p| pitch_class(p) == g)
                        && !carried_elsewhere(g)
                })
                .count() as u32;
        }
        let centre = |v: &[Midi]| v.iter().sum::<Midi>() / v.len().max(1) as Midi;
        out.push((clash, (centre(&v) - centre(origin)).abs(), v));
    };
    for c in base {
        let v = &c.voices;
        for i in 0..v.len() {
            for shift in [12, -12] {
                let mut w = v.clone();
                w[i] += shift;
                offer(w, v, &mut out);
            }
            if v.len() > 2 {
                let mut w = v.clone();
                w.remove(i);
                offer(w, v, &mut out);
            }
            // Complete the identity: a non-identity voice becomes a missing identity tone.
            if !identity.contains(&pitch_class(v[i])) {
                for &g in identity
                    .iter()
                    .filter(|&&g| !v.iter().any(|&p| pitch_class(p) == g))
                {
                    if let Some(q) = (0..=6)
                        .flat_map(|d| [v[i] - d, v[i] + d])
                        .find(|&q| pitch_class(q) == g && !v.contains(&q))
                    {
                        let mut w = v.clone();
                        w[i] = q;
                        offer(w, v, &mut out);
                    }
                }
            }
            if guides.contains(&pitch_class(v[i])) {
                if let Some(q) = (1..=3).flat_map(|d| [v[i] - d, v[i] + d]).find(|&q| {
                    let pc = pitch_class(q);
                    allowed(ctx, pc) && !guides.contains(&pc) && !v.contains(&q)
                }) {
                    let mut w = v.clone();
                    w[i] = q;
                    offer(w, v, &mut out);
                }
            }
        }
    }
    out.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));
    out.into_iter()
        .take(COMPLEMENT_KEEP)
        .map(|(_, _, voices)| VoicingCandidate {
            voices,
            shape: VoicingShape::Complement,
        })
        .collect()
}

/// The new pitch classes of the harmonic edits applied at the harmony starting at `a` — its witness
/// needs one of them sounding in the span.
fn edit_pcs(perf: &PerformancePlan, a: f64) -> Vec<i32> {
    perf.edits
        .iter()
        .filter(|x| (x.at_beat - a).abs() < 1e-6)
        .flat_map(|x| {
            x.after
                .pitch_classes()
                .into_iter()
                .filter(|pc| !x.before.contains_pc(*pc))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// One harmony of the joint solve.
struct Joint {
    /// The harmony (index into the contexts).
    ci: usize,
    /// Index into the pad's / the keys' own step list, when that player sounds this harmony.
    pad: Option<usize>,
    keys: Option<usize>,
    /// Surviving candidate pairs `(pad candidate, keys candidate)` with their union cost.
    pairs: Vec<(Option<usize>, Option<usize>, UnionCost)>,
    /// The keys STRUCK in the previous harmony (a Hold, a Sustain downbeat) ring into this one:
    /// their common tones with this chord sound here, whatever this step chooses.
    ring_in: bool,
    /// The committed voices over this harmony (what a ringing keys tone meets besides the bed).
    others: Vec<Voice>,
}

/// Where the keys STRIKE a sustained voicing in `[a, b)`: a keys Hold's start, or a Sustain bar's
/// downbeat (the voicing of the harmony sounding there), with how long the struck notes are
/// written to last and whether a Hold stamps them (mirrors `comp::keys_comp`).
fn keys_strikes(perf: &PerformancePlan, a: f64, b: f64) -> Vec<(f64, f64, bool)> {
    let holds: Vec<(f64, f64)> = perf
        .actions
        .of_kind(ActionKind::Hold)
        .filter(|h| h.initiator == Agent::Keys)
        .map(|h| (h.start_beat, h.end_beat()))
        .collect();
    let mut out = Vec::new();
    let first_bar = (a / BEATS_PER_BAR).ceil().max(0.0) as u32;
    for (bar, s) in (first_bar..)
        .map(|bar| (bar, bar as f64 * BEATS_PER_BAR))
        .take_while(|&(_, s)| s < b - 1e-6)
    {
        if s >= a - 1e-6 && perf.bar(bar).is_some_and(|eb| eb.keys == KeysMode::Sustain) {
            let stamped = holds.iter().any(|&(hs, he)| hs < s + 3.9 && he > s + 1e-6);
            out.push((s, 3.9, stamped));
        }
    }
    for &(hs, he) in &holds {
        let in_sustain_bar = perf
            .bar_at(hs)
            .is_some_and(|eb| eb.keys == KeysMode::Sustain);
        if hs >= a - 1e-6 && hs < b - 1e-6 && !in_sustain_bar && perf.on_stage(Agent::Keys, hs) {
            out.push((hs, ((he - hs) * 0.97).max(1.5), true));
        }
    }
    out
}

fn role_key(
    prev: Option<(&[Midi], &PathStep)>,
    v: &[Midi],
    step: &PathStep,
    w: &PathWeights,
) -> JointKey {
    let prev = if step.re_entry { None } else { prev };
    let c = step_cost(prev, v, step);
    JointKey {
        witness: 0,
        vertical: 0,
        gesture: gesture_tier(&c, w),
        guide_breaks: c.guide_breaks,
        weighted: weigh(&c, step, w),
    }
}

/// Solve the pad's and the keys' voice paths JOINTLY against the ledger `state` (the lead, the bass
/// and the keys' material lines already committed). `spread` is the world's voicing spread, `lead`
/// the lead line (the keys' step candidates still carry the soft pitch-class melody term), `n` the
/// keys' voice count.
pub fn joint_support_paths(
    perf: &PerformancePlan,
    spread: f32,
    lead: &[Note],
    n: usize,
    state: &HarmonicEnsembleState,
) -> SupportPaths {
    let w = PathWeights::default();
    let uw = UnionWeights::default();
    // The control: each player's own R7b path, on the voicing engine's R7b candidates — exactly
    // the independent band, so "no worse than the control" (the witness tier) means no worse than
    // what the band played before it listened.
    let (pix, mut psteps) = pad_steps(perf, spread);
    let (kix, mut ksteps) = keys_steps(perf, spread, lead, n);
    let pind = voice_path(&psteps, &w);
    let kind = voice_path(&ksteps, &w);
    // The world/language decides whether a plain triad's support must carry an unwritten 9th
    // (R7b forced it everywhere): required means the harmony NEEDS it, not that it sounds jazzy.
    // Where it need not, the plainer candidates join the options (after the control's indices).
    if !state.policy().triad_color {
        let (_, pplain) = pad_steps_with(perf, spread, false);
        let (_, kplain) = keys_steps_with(perf, spread, lead, n, false);
        for (steps, plain) in [(&mut psteps, pplain), (&mut ksteps, kplain)] {
            for (st, pl) in steps.iter_mut().zip(plain) {
                for c in pl.candidates {
                    if !st.candidates.iter().any(|x| x.voices == c.voices) {
                        st.candidates.push(c);
                    }
                }
            }
        }
    }
    // The complements, appended AFTER the base candidates (the control's indices stay valid).
    for (steps, ix, role) in [
        (&mut psteps, &pix, Role::Pad),
        (&mut ksteps, &kix, Role::Keys),
    ] {
        for (t, &ci) in ix.iter().enumerate() {
            let ctx = &perf.contexts[ci];
            let (a, b) = (ctx.start_beat, ctx.start_beat + ctx.dur_beats as f64);
            let others: Vec<Voice> = state.sounding(a, b).copied().collect();
            let must = edit_pcs(perf, a);
            let extra = complements(
                ctx,
                role,
                &steps[t].candidates,
                &steps[t].range,
                a,
                b,
                &others,
                state.plan_at(a),
                role == Role::Pad,
                &must,
            );
            steps[t].candidates.extend(extra);
        }
    }
    let mut cis: Vec<usize> = pix.iter().chain(kix.iter()).copied().collect();
    cis.sort_unstable();
    cis.dedup();
    let policy = *state.policy();
    let mut report = JointReport::default();
    let mut joints: Vec<Joint> = Vec::with_capacity(cis.len());
    for &ci in &cis {
        let ctx = &perf.contexts[ci];
        let (a, b) = (ctx.start_beat, ctx.start_beat + ctx.dur_beats as f64);
        let pad = pix.binary_search(&ci).ok();
        let keys = kix.binary_search(&ci).ok();
        let others: Vec<Voice> = state.sounding(a, b).copied().collect();
        let plan = state.plan_at(a);
        let specs = tension_specs(ctx);
        // The keys carry THIS harmony only where they strike its voicing and hold it: a Sustain bar
        // strikes the voicing of the harmony at its downbeat (a later harmony in that bar hears
        // only held common tones, never its own path voicing); a Hold strikes at its start.
        let first_bar = (a / BEATS_PER_BAR).ceil().max(0.0) as u32;
        let sustained = (first_bar..)
            .map(|bar| (bar, bar as f64 * BEATS_PER_BAR))
            .take_while(|&(_, s)| s < b - 1e-6)
            .any(|(bar, s)| {
                s >= a - 1e-6
                    && perf.bar(bar).is_some_and(|eb| eb.keys == KeysMode::Sustain)
                    && (s + 3.9).min(b) - s >= 0.5 * (b - a)
            });
        let keys_hold = sustained
            || perf
                .actions
                .of_kind(ActionKind::Hold)
                .filter(|h| h.initiator == Agent::Keys)
                .any(|h| {
                    h.start_beat >= a - 1e-6
                        && h.start_beat < b - 1e-6
                        && h.end_beat().min(b) - h.start_beat >= 0.5 * (b - a)
                });
        let pad_on = pad.is_some() && perf.on_stage(Agent::Pad, a);
        let keys_on = keys.is_some() && perf.on_stage(Agent::Keys, a);
        let carriers = Carriers {
            pad: pad_on,
            keys: keys_on && (keys_hold || !pad_on),
            pad_on,
            keys_on,
            pad_all: perf
                .bar_at(a)
                .is_some_and(|eb| eb.pad != PadMode::CommonToneCarry),
        };
        let must = edit_pcs(perf, a);
        let pc_n = pad.map(|t| psteps[t].candidates.len());
        let kc_n = keys.map(|t| ksteps[t].candidates.len());
        let pad_opts: Vec<Option<usize>> = match pc_n {
            Some(n) => (0..n).map(Some).collect(),
            None => vec![None],
        };
        let keys_opts: Vec<Option<usize>> = match kc_n {
            Some(n) => (0..n).map(Some).collect(),
            None => vec![None],
        };
        let voices_of = |t: Option<usize>, j: Option<usize>, steps: &[PathStep]| -> Vec<Midi> {
            match (t, j) {
                (Some(t), Some(j)) => steps[t].candidates[j].voices.clone(),
                _ => Vec::new(),
            }
        };
        let ind = (pad.map(|t| pind.choice[t]), keys.map(|t| kind.choice[t]));
        // A Thicken over this harmony is witnessed by an upper layer (a pad pitch >= 79) or more
        // simultaneous pad+keys voices than before: the joint choice may not be thinner, or lower,
        // than the control pair that already witnesses it.
        let thickens: Vec<(f64, f64)> = perf
            .actions
            .of_kind(ActionKind::Thicken)
            .map(|x| (x.start_beat, x.end_beat()))
            .collect();
        let thicken = thickens.iter().any(|&(s, e)| s < b - 1e-6 && e > a + 1e-6);
        // ...and it is heard against the 8 beats before it: the joint may not thicken those either.
        let before_thicken = !thicken
            && thickens
                .iter()
                .any(|&(s, _)| (s - 8.0) < b - 1e-6 && s > a + 1e-6);
        let (cpv, ckv) = (
            voices_of(pad, ind.0, &psteps),
            voices_of(keys, ind.1, &ksteps),
        );
        let top = |v: &[Midi]| v.iter().copied().max().unwrap_or(0);
        let thinner = |pv: &[Midi], kv: &[Midi]| {
            pv.len() + kv.len() < cpv.len() + ckv.len() || (top(&cpv) >= 79 && top(pv) < 79)
        };
        let thicker = |pv: &[Midi], kv: &[Midi]| pv.len() + kv.len() > cpv.len() + ckv.len();
        // A stamped Hold struck here that the next change cuts short of its witness (1.4 beats)
        // survives only through a tone common to the next chord: keep one if the control did.
        let next_chord = perf
            .contexts
            .get(ci + 1)
            .filter(|n| (n.start_beat - b).abs() < 1e-6)
            .map(|n| n.chord);
        let hold_needs_common = next_chord.filter(|_| {
            keys_strikes(perf, a, b)
                .iter()
                .any(|&(s, d, stamped)| stamped && s + d > b + 0.02 && b - s < 1.4 / 0.97 + 1e-6)
        });
        let has_common = |kv: &[Midi]| {
            hold_needs_common.is_some_and(|c| kv.iter().any(|&p| c.contains_pc(pitch_class(p))))
        };
        let control_common = has_common(&ckv);
        let mut scored: Vec<(Option<usize>, Option<usize>, UnionCost, f64)> = Vec::new();
        for &pj in &pad_opts {
            for &kj in &keys_opts {
                let pv = voices_of(pad, pj, &psteps);
                let kv = voices_of(keys, kj, &ksteps);
                let mut u = union_cost(
                    ctx, a, b, &pv, &kv, &others, plan, &policy, &specs, carriers, &must,
                );
                if (thicken && thinner(&pv, &kv)) || (before_thicken && thicker(&pv, &kv)) {
                    u.witness_missing += 1;
                }
                if control_common && !has_common(&kv) {
                    u.witness_missing += 1;
                }
                // Static per-role terms, for pruning only.
                let stat = pad
                    .zip(pj)
                    .map(|(t, _)| role_key(None, &pv, &psteps[t], &w).weighted)
                    .unwrap_or(0.0)
                    + keys
                        .zip(kj)
                        .map(|(t, _)| role_key(None, &kv, &ksteps[t], &w).weighted)
                        .unwrap_or(0.0);
                scored.push((pj, kj, u, u.soft(&uw) + stat));
            }
        }
        report.pairs_scored += scored.len();
        scored.sort_by(|x, y| {
            (x.2.witness_missing, x.2.hard())
                .cmp(&(y.2.witness_missing, y.2.hard()))
                .then(x.3.total_cmp(&y.3))
                .then(x.0.cmp(&y.0))
                .then(x.1.cmp(&y.1))
        });
        let mut pairs: Vec<(Option<usize>, Option<usize>, UnionCost)> = scored
            .iter()
            .take(JOINT_BEAM)
            .map(|x| (x.0, x.1, x.2))
            .collect();
        if !pairs.iter().any(|x| (x.0, x.1) == ind) {
            if let Some(x) = scored.iter().find(|x| (x.0, x.1) == ind) {
                pairs.push((x.0, x.1, x.2));
            }
        }
        report.steps += 1;
        report.both += (pad.is_some() && keys.is_some()) as usize;
        // Keys struck in the previous harmony and written past this one's start ring into it.
        let ring_in = ci > 0 && {
            let p = &perf.contexts[ci - 1];
            keys_strikes(perf, p.start_beat, a)
                .iter()
                .any(|&(s, d, _)| s + d > a + 0.02)
        };
        joints.push(Joint {
            ci,
            pad,
            keys,
            pairs,
            ring_in,
            others,
        });
    }

    // The Viterbi over pairs. The per-role transition exists only between contiguous harmonies the
    // role sounds in both (its own re-entry flag says so).
    let pair_voices = |jt: &Joint, k: usize| -> (Vec<Midi>, Vec<Midi>) {
        let (pj, kj, _) = jt.pairs[k];
        let pv = jt
            .pad
            .zip(pj)
            .map(|(t, j)| psteps[t].candidates[j].voices.clone())
            .unwrap_or_default();
        let kv = jt
            .keys
            .zip(kj)
            .map(|(t, j)| ksteps[t].candidates[j].voices.clone())
            .unwrap_or_default();
        (pv, kv)
    };
    let step_key = |jt: &Joint, k: usize, prev: Option<(&Joint, usize)>| -> JointKey {
        let (pv, kv) = pair_voices(jt, k);
        let mut key = JointKey {
            witness: jt.pairs[k].2.witness_missing,
            vertical: jt.pairs[k].2.hard(),
            gesture: 0,
            guide_breaks: 0,
            weighted: jt.pairs[k].2.soft(&uw),
        };
        let prev_voices = prev.map(|(pj, pk)| pair_voices(pj, pk));
        // Keys struck in the previous harmony ring into this one on their common tones: this pad
        // (and the committed band) must not meet them a semitone away.
        if let Some(((pj, _), (_, prev_keys))) = prev.zip(prev_voices.as_ref()) {
            if jt.ring_in && pj.ci + 1 == jt.ci && pj.keys.is_some() {
                let ctx = &perf.contexts[jt.ci];
                let (a, b) = (ctx.start_beat, ctx.start_beat + ctx.dur_beats as f64);
                let held: Vec<Voice> = prev_keys
                    .iter()
                    .filter(|&&h| ctx.chord.contains_pc(pitch_class(h)))
                    .map(|&h| support_voice(ctx, Role::Keys, h, a, b, "hold"))
                    .collect();
                let now: Vec<Voice> = pv
                    .iter()
                    .map(|&p| support_voice(ctx, Role::Pad, p, a, b, "pad"))
                    .chain(jt.others.iter().copied())
                    .collect();
                let plan = state.plan_at(a);
                key.vertical += held
                    .iter()
                    .flat_map(|h| now.iter().map(move |x| (h, x)))
                    .filter(|(h, x)| {
                        h.pitch != x.pitch
                            && Clash::of(h.pitch, x.pitch).is_some()
                            && classify_clash(ctx, h, x, plan) == VerticalClass::UnownedCollision
                    })
                    .count() as u32;
            }
        }
        if let Some(t) = jt.pad {
            let pp = prev
                .zip(prev_voices.as_ref())
                .and_then(|((pj, _), pv)| pj.pad.map(|pt| (pv.0.as_slice(), &psteps[pt])));
            key = key.plus(role_key(pp, &pv, &psteps[t], &w));
        }
        if let Some(t) = jt.keys {
            let kp = prev
                .zip(prev_voices.as_ref())
                .and_then(|((pj, _), pv)| pj.keys.map(|kt| (pv.1.as_slice(), &ksteps[kt])));
            key = key.plus(role_key(kp, &kv, &ksteps[t], &w));
        }
        key
    };
    let mut dp: Vec<Vec<JointKey>> = Vec::with_capacity(joints.len());
    let mut back: Vec<Vec<usize>> = Vec::with_capacity(joints.len());
    for (t, jt) in joints.iter().enumerate() {
        let mut row = Vec::with_capacity(jt.pairs.len());
        let mut brow = Vec::with_capacity(jt.pairs.len());
        for k in 0..jt.pairs.len() {
            if t == 0 {
                row.push(JointKey::ZERO.plus(step_key(jt, k, None)));
                brow.push(0);
                continue;
            }
            let pj = &joints[t - 1];
            let mut best: Option<(JointKey, usize)> = None;
            for (i, prev_key) in dp[t - 1].iter().enumerate() {
                let key = prev_key.plus(step_key(jt, k, Some((pj, i))));
                if best.is_none_or(|(b, _)| key.lt(&b)) {
                    best = Some((key, i));
                }
            }
            let (key, i) = best.expect("every harmony keeps at least one pair");
            row.push(key);
            brow.push(i);
        }
        dp.push(row);
        back.push(brow);
    }
    let mut choice = vec![0usize; joints.len()];
    if let Some(last) = dp.last() {
        let mut j = 0;
        for (i, k) in last.iter().enumerate() {
            if k.lt(&last[j]) {
                j = i;
            }
        }
        report.joint_key = Some(last[j]);
        for t in (0..joints.len()).rev() {
            choice[t] = j;
            j = back[t][j];
        }
    }
    // The control's key under the same joint cost, for the report and the tests.
    let mut ind_key = JointKey::ZERO;
    let mut prev_ind: Option<usize> = None;
    for (t, jt) in joints.iter().enumerate() {
        let ind = (
            jt.pad.map(|s| pind.choice[s]),
            jt.keys.map(|s| kind.choice[s]),
        );
        let k = jt
            .pairs
            .iter()
            .position(|x| (x.0, x.1) == ind)
            .expect("the independent pair is always kept");
        ind_key = ind_key.plus(step_key(jt, k, prev_ind.map(|i| (&joints[t - 1], i))));
        prev_ind = Some(k);
        report.independent.add(&jt.pairs[k].2);
        report.joint.add(&jt.pairs[choice[t]].2);
        report.changed += (k != choice[t]) as usize;
    }
    report.independent_key = Some(ind_key);

    let mut pchoice = pind.choice.clone();
    let mut kchoice = kind.choice.clone();
    for (t, jt) in joints.iter().enumerate() {
        let (pj, kj, _) = jt.pairs[choice[t]];
        if let (Some(s), Some(j)) = (jt.pad, pj) {
            pchoice[s] = j;
        }
        if let (Some(s), Some(j)) = (jt.keys, kj) {
            kchoice[s] = j;
        }
    }
    let ppath = evaluate_path(&psteps, &pchoice, &w);
    let kpath = evaluate_path(&ksteps, &kchoice, &w);
    SupportPaths {
        pad: RolePath::from_steps(pix, &psteps, ppath, VoiceRange::pad(spread)),
        keys: RolePath::from_steps(kix, &ksteps, kpath, VoiceRange::keys(n, spread)),
        report,
    }
}
