//! Voice leading: turn a bare [`Chord`] symbol into explicit sounding [`Voicing`]s that
//! move smoothly from chord to chord. A chord symbol is not enough — the *movement*
//! between voicings is what makes a progression sound composed rather than stamped.
//!
//! Two engines live here:
//!
//! - [`VoiceLeader`] — the **legacy control**. It builds one close voicing from a fixed register
//!   window and offers it at −12/0/+12. In practice the shifted placements never win, so its
//!   memory of the previous voicing never changes the outcome: it is a register-windowed stamp
//!   that reads only the chord's pitch classes (never the palette), and a 3-voice "shell" of it
//!   can miss a guide tone. Kept verbatim so the new engine can be measured against it.
//! - The **voice path** (Round VIIb) — a bounded Viterbi over explicit per-harmony candidates
//!   ([`candidates`]: close inversions, drop-2, 3–7/7–3 shells, rootless A/B, guide tones +
//!   extensions, licensed upper-structure triads, plus the legacy voicing itself). Hard filters
//!   keep every pitch a chord tone or a licensed tension and every candidate carrying its guide
//!   tones; [`voice_path`] then minimizes an inspectable [`VoicePathCost`] vector across the whole
//!   progression under a lexicographic [`PathKey`]: the backbone's gesture targets first (Lift
//!   rises, Open widens, Reset contracts), then guide-tone continuity, then the weighted rest
//!   (motion, retention — doubled on a Deflect — spacing, register, span, doubling, melody).
//!   [`greedy_path`] is the same candidates and cost chosen one step at a time — the control.
//!
//! [`VoicingDiagnostics`] measures what the pad and keys actually sounded, from the Score.

use std::collections::BTreeMap;

use super::backbone::HarmonicGesture;
use super::context::{context_at, HarmonicContext};
use super::form::BEATS_PER_BAR;
use super::performance::{KeysMode, PadMode, PerformancePlan};
use super::score::{Note, Role, Score};
use super::theory::{pitch_class, voice_motion, Chord, Midi};

/// A sounding chord voicing: pitches in ascending order.
#[derive(Debug, Clone, PartialEq)]
pub struct Voicing {
    pub voices: Vec<Midi>,
}

impl Voicing {
    fn sorted(mut v: Vec<Midi>) -> Voicing {
        v.sort_unstable();
        Voicing { voices: v }
    }

    /// Total semitone motion to another voicing (both are compared position-by-position).
    pub fn motion_to(&self, other: &Voicing) -> i32 {
        voice_motion(&self.voices, &other.voices)
    }
}

/// A stateful voice-leader: remembers the previous voicing and leads into the next chord.
///
/// The **legacy control** (see the module docs): the realizers now voice through [`voice_path`];
/// this leader survives as the source of the [`VoicingShape::Legacy`] candidate and as the
/// baseline the path is proven never to lose to.
#[derive(Debug, Clone)]
pub struct VoiceLeader {
    prev: Option<Voicing>,
    low: Midi,
    high: Midi,
    spread: f32,
}

impl VoiceLeader {
    /// A leader confined to `[low, high]`, spacing per `spread` (0 tight, 1 open).
    pub fn new(low: Midi, high: Midi, spread: f32) -> VoiceLeader {
        VoiceLeader {
            prev: None,
            low,
            high: high.max(low + 12),
            spread: spread.clamp(0.0, 1.0),
        }
    }

    /// Forget history (e.g. after a seek / section boundary).
    pub fn reset(&mut self) {
        self.prev = None;
    }

    /// Lead into `chord` with `n` upper voices centered near `center`, returning the chosen
    /// voicing.
    pub fn lead(&mut self, chord: &Chord, n: usize, center: Midi) -> Voicing {
        let n = n.clamp(2, 6);
        let base = self.close_voicing(chord, n, center);
        // Candidate octave placements of the whole voicing.
        let candidates = [shift(&base, -12), base.clone(), shift(&base, 12)];
        let chosen = match &self.prev {
            None => {
                // No history: choose the placement whose mean pitch is nearest `center`.
                candidates
                    .into_iter()
                    .min_by_key(|c| (mean(c) - center).abs())
                    .unwrap()
            }
            Some(prev) => {
                // Choose the placement minimizing total motion from the previous voicing.
                candidates
                    .into_iter()
                    .map(|c| {
                        let padded = pad_to(&prev.voices, c.len());
                        (voice_motion(&padded, &c), c)
                    })
                    .min_by_key(|(m, _)| *m)
                    .map(|(_, c)| c)
                    .unwrap()
            }
        };
        let clamped = self.clamp_register(chosen);
        let v = Voicing::sorted(clamped);
        self.prev = Some(v.clone());
        v
    }

    /// A close-position (then optionally opened) voicing of `n` tones from `center`: the
    /// `n` lowest chord tones at or above the anchor, ascending (so every voice is a real
    /// chord tone and all pitch classes are covered before any is doubled).
    fn close_voicing(&self, chord: &Chord, n: usize, center: Midi) -> Vec<Midi> {
        let tones = chord.pitch_classes();
        let start = center - 6;
        let mut out = Vec::with_capacity(n);
        let mut p = start;
        while out.len() < n {
            if tones.contains(&p.rem_euclid(12)) {
                out.push(p);
            }
            p += 1;
            // Safety: bounded scan (12 semitones always yields a chord tone).
            if p > start + 128 {
                break;
            }
        }
        // Open the spacing for high-spread worlds: drop an inner voice an octave.
        if self.spread > 0.6 && out.len() >= 3 {
            let idx = 1;
            if out[idx] - 12 >= self.low {
                out[idx] -= 12;
            }
        }
        out
    }

    fn clamp_register(&self, mut v: Vec<Midi>) -> Vec<Midi> {
        for p in v.iter_mut() {
            while *p < self.low {
                *p += 12;
            }
            while *p > self.high {
                *p -= 12;
            }
        }
        v
    }
}

fn shift(v: &[Midi], by: Midi) -> Vec<Midi> {
    v.iter().map(|&p| p + by).collect()
}

fn mean(v: &[Midi]) -> Midi {
    if v.is_empty() {
        0
    } else {
        v.iter().sum::<Midi>() / v.len() as Midi
    }
}

/// Pad or trim `prev` to `len` for a positional comparison (duplicating the top voice when
/// the new voicing has more voices).
fn pad_to(prev: &[Midi], len: usize) -> Vec<Midi> {
    let mut v = prev.to_vec();
    if v.is_empty() {
        return vec![60; len];
    }
    while v.len() < len {
        let last = *v.last().unwrap();
        v.push(last);
    }
    v.truncate(len);
    v
}

// =================================================================================================
// Round VIIb — the voice path: bounded candidates, an inspectable cost vector, a Viterbi.
// =================================================================================================

/// At most this many candidate voicings per harmony (the DP is `O(steps · K²)`).
pub const MAX_CANDIDATES: usize = 24;

/// What a candidate voicing IS — for per-mode filtering and for the diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VoicingShape {
    /// The legacy [`VoiceLeader`] close voicing (the control, kept as a candidate when admissible).
    Legacy,
    /// A close-position inversion of the chord's core tones.
    Close,
    /// A four-voice close voicing with its second voice from the top dropped an octave.
    Drop2,
    /// A guide-tone shell: 3–7 or 7–3, optionally plus one licensed extension (over a chord with a
    /// single guide tone: the 3rd + 9th + 5th).
    Shell,
    /// A rootless A/B voicing: 3–5–7–9 or 7–9–3–5.
    Rootless,
    /// Both guide tones plus two licensed extensions (3–7–9–13 and its rotations).
    GuideExtensions,
    /// A triad of licensed tones (at least one a tension) stacked over the guide tone(s).
    UpperStructure,
    /// Round VIII: a variant of another candidate that COMPLEMENTS the band rather than respelling
    /// the whole chord — one voice moved an octave, dropped (another player supplies it), or a guide
    /// tone swapped for a non-guide neighbour. Offered only by the joint support solve, which
    /// requires the BAND (not each player) to hold every identity tone.
    Complement,
}

impl VoicingShape {
    /// Candidate families in round-robin order (after the legacy control).
    const ORDER: [VoicingShape; 6] = [
        VoicingShape::Close,
        VoicingShape::Drop2,
        VoicingShape::Shell,
        VoicingShape::Rootless,
        VoicingShape::GuideExtensions,
        VoicingShape::UpperStructure,
    ];

    /// A short label for dumps and diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            VoicingShape::Legacy => "legacy",
            VoicingShape::Close => "close",
            VoicingShape::Drop2 => "drop2",
            VoicingShape::Shell => "shell",
            VoicingShape::Rootless => "rootless",
            VoicingShape::GuideExtensions => "guide+ext",
            VoicingShape::UpperStructure => "upper-structure",
            VoicingShape::Complement => "complement",
        }
    }
}

/// One admissible voicing of one harmony.
#[derive(Debug, Clone, PartialEq)]
pub struct VoicingCandidate {
    /// Ascending, distinct pitches.
    pub voices: Vec<Midi>,
    pub shape: VoicingShape,
}

impl VoicingCandidate {
    /// As a [`Voicing`].
    pub fn voicing(&self) -> Voicing {
        Voicing::sorted(self.voices.clone())
    }
}

/// Which shapes a step allows (the pad's per-bar mode picks it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeFamily {
    /// Every shape (full voicings, the legacy control included).
    Full,
    /// Guide-tone shells only.
    Shell,
}

/// A role's register window, voice count and spacing target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceRange {
    pub low: Midi,
    pub high: Midi,
    /// The register centre the `register` cost pulls toward.
    pub center: Midi,
    pub min_voices: usize,
    pub max_voices: usize,
    /// The voice count the legacy control is asked for.
    pub legacy_voices: usize,
    /// The span (semitones, bottom to top) the `span` cost pulls toward — the world's spread,
    /// used continuously (no threshold).
    pub target_span: f32,
    /// A voice at or above this pitch is required (the upper layer's audible witness), if set.
    pub min_top: Option<Midi>,
}

impl VoiceRange {
    /// The pad's full four-voice voicing.
    pub fn pad(spread: f32) -> VoiceRange {
        VoiceRange {
            low: 52,
            high: 79,
            center: 67,
            min_voices: 4,
            max_voices: 4,
            legacy_voices: 4,
            target_span: 12.0 + 12.0 * spread.clamp(0.0, 1.0),
            min_top: None,
        }
    }

    /// The pad's guide-tone shell (2–3 voices).
    pub fn pad_shell(spread: f32) -> VoiceRange {
        VoiceRange {
            min_voices: 2,
            max_voices: 3,
            target_span: 7.0 + 6.0 * spread.clamp(0.0, 1.0),
            ..VoiceRange::pad(spread)
        }
    }

    /// The pad's wide upper layer: four voices reaching MIDI 79 or above.
    pub fn pad_upper(spread: f32) -> VoiceRange {
        VoiceRange {
            low: 62,
            high: 91,
            center: 78,
            target_span: 14.0 + 8.0 * spread.clamp(0.0, 1.0),
            min_top: Some(79),
            ..VoiceRange::pad(spread)
        }
    }

    /// The keys: exactly `n` voices (so every stab keeps today's note count).
    pub fn keys(n: usize, spread: f32) -> VoiceRange {
        let n = n.clamp(2, 6);
        let s = spread.clamp(0.0, 1.0);
        VoiceRange {
            low: 58,
            high: 84,
            center: 72,
            min_voices: n,
            max_voices: n,
            legacy_voices: n,
            target_span: if n <= 3 {
                7.0 + 6.0 * s
            } else {
                10.0 + 8.0 * s
            },
            min_top: None,
        }
    }
}

/// Whether `pc` may sound in a support voicing over `ctx` (a chord tone or a licensed tension).
pub fn allowed(ctx: &HarmonicContext, pc: i32) -> bool {
    let pc = pc.rem_euclid(12);
    ctx.chord.contains_pc(pc) || ctx.palette.tensions.contains(&pc)
}

/// The chord's fifth (perfect, diminished or augmented), if it has one.
fn fifth_pc(chord: &Chord) -> Option<i32> {
    chord
        .quality
        .intervals()
        .iter()
        .find(|&&i| matches!(i, 6..=8))
        .map(|&i| (chord.root_pc + i).rem_euclid(12))
}

/// The pitch classes every candidate over `ctx` must carry, most defining first: both guide tones
/// (3rd, 7th/6th) — or, over a chord with a single guide tone (add9, a triad, a sus), the 3rd +
/// 9th (when licensed) + 5th, with the root added if fewer than three remain.
pub fn required_pcs(ctx: &HarmonicContext) -> Vec<i32> {
    let g = ctx.palette.guide_tones.clone();
    if g.len() >= 2 {
        return g;
    }
    let root = ctx.chord.root_pc;
    let mut req = g;
    let ninth = (root + 2).rem_euclid(12);
    if allowed(ctx, ninth) && !req.contains(&ninth) {
        req.push(ninth);
    }
    if let Some(f) = fifth_pc(&ctx.chord) {
        if !req.contains(&f) {
            req.push(f);
        }
    }
    if req.len() < 3 && !req.contains(&root) {
        req.push(root);
    }
    req
}

/// Whether `pc` is an extension over `ctx` (a licensed tension, or a written 9th that is not one
/// of the required tones).
fn is_extension(ctx: &HarmonicContext, pc: i32) -> bool {
    let pc = pc.rem_euclid(12);
    ctx.palette.tensions.contains(&pc)
        || (ctx.chord.contains_pc(pc) && (pc - ctx.chord.root_pc).rem_euclid(12) == 2)
}

/// Licensed extensions over `ctx` beyond the required tones, most idiomatic first (9, 13, 11, ♭13,
/// then the rest).
pub fn extensions(ctx: &HarmonicContext) -> Vec<i32> {
    let root = ctx.chord.root_pc;
    let req = required_pcs(ctx);
    let fifth = fifth_pc(&ctx.chord);
    let mut ext: Vec<i32> = (0..12)
        .filter(|&pc| is_extension(ctx, pc))
        .filter(|pc| !req.contains(pc) && Some(*pc) != fifth && *pc != root)
        .collect();
    ext.sort_by_key(|&pc| {
        let iv = (pc - root).rem_euclid(12);
        let rank = match iv {
            2 => 0,
            9 => 1,
            5 => 2,
            8 => 3,
            _ => 4,
        };
        (rank, iv)
    });
    ext
}

fn rotations(seq: &[i32]) -> Vec<Vec<i32>> {
    (0..seq.len())
        .map(|k| seq[k..].iter().chain(&seq[..k]).copied().collect())
        .collect()
}

/// The pitch-class stacks (bottom to top) worth placing over `ctx`.
fn shapes(
    ctx: &HarmonicContext,
    range: &VoiceRange,
    family: ShapeFamily,
) -> Vec<(VoicingShape, Vec<i32>)> {
    let root = ctx.chord.root_pc;
    let iv_of = |pc: i32| (pc - root).rem_euclid(12);
    let req = required_pcs(ctx);
    let ext = extensions(ctx);
    let fifth = fifth_pc(&ctx.chord);
    let guide = &ctx.palette.guide_tones;
    let two_guides = guide.len() >= 2;
    let mut out: Vec<(VoicingShape, Vec<i32>)> = Vec::new();

    // Shells: 3–7 / 7–3, alone or with one licensed extension (or the 5th/root when none is).
    if two_guides {
        let (g3, g7) = (guide[0], guide[1]);
        out.push((VoicingShape::Shell, vec![g3, g7]));
        out.push((VoicingShape::Shell, vec![g7, g3]));
        let thirds: Vec<i32> = if ext.is_empty() {
            [fifth, Some(root)].into_iter().flatten().collect()
        } else {
            ext.iter().take(2).copied().collect()
        };
        for e in thirds {
            out.push((VoicingShape::Shell, vec![g3, g7, e]));
            out.push((VoicingShape::Shell, vec![g7, g3, e]));
            out.push((VoicingShape::Shell, vec![g7, e, g3]));
        }
    } else {
        let mut cyc = req.clone();
        cyc.sort_by_key(|&pc| iv_of(pc));
        for r in rotations(&cyc) {
            out.push((VoicingShape::Shell, r));
        }
    }
    if family == ShapeFamily::Shell {
        return out;
    }

    // Close inversions of the core: the required tones plus the chord tones, trimmed to the voice
    // count (the 5th goes first, then the root) and filled with extensions.
    let mut core: Vec<i32> = Vec::new();
    for &pc in req.iter().chain(ctx.chord.pitch_classes().iter()) {
        if !core.contains(&pc) {
            core.push(pc);
        }
    }
    while core.len() > range.max_voices {
        let drop = [fifth, Some(root)]
            .into_iter()
            .flatten()
            .find(|pc| core.contains(pc) && !req.contains(pc))
            .or_else(|| {
                core.iter()
                    .copied()
                    .filter(|pc| !req.contains(pc))
                    .max_by_key(|&pc| iv_of(pc))
            });
        match drop {
            Some(d) => core.retain(|&p| p != d),
            None => break,
        }
    }
    for &e in &ext {
        if core.len() >= range.min_voices {
            break;
        }
        if !core.contains(&e) {
            core.push(e);
        }
    }
    core.sort_by_key(|&pc| iv_of(pc));
    for r in rotations(&core) {
        out.push((VoicingShape::Close, r));
    }

    if two_guides {
        let (g3, g7) = (guide[0], guide[1]);
        // Rootless A/B: 3–5–7–9 and 7–9–3–5 (the 13th standing in for the 9th as a variant).
        if let Some(f) = fifth {
            for &e in ext.iter().take(2) {
                out.push((VoicingShape::Rootless, vec![g3, f, g7, e]));
                out.push((VoicingShape::Rootless, vec![g7, e, g3, f]));
            }
        }
        // Guide tones plus two extensions: 3–7–9–13, 7–3–9–13, 3–13–7–9, 7–9–3–13.
        if ext.len() >= 2 {
            let (e0, e1) = (ext[0], ext[1]);
            out.push((VoicingShape::GuideExtensions, vec![g3, g7, e0, e1]));
            out.push((VoicingShape::GuideExtensions, vec![g7, g3, e0, e1]));
            out.push((VoicingShape::GuideExtensions, vec![g3, e1, g7, e0]));
            out.push((VoicingShape::GuideExtensions, vec![g7, e0, g3, e1]));
        }
    }

    // Upper-structure triads: a triad of the chord-scale whose tones are all licensed and at least
    // one a tension, stacked over whichever required tones it does not already carry.
    let scale = ctx.palette.scale;
    let sp: Vec<i32> = (0..7)
        .map(|d| pitch_class(scale.degree_pitch(d, 4)))
        .collect();
    let mut found = 0;
    for d in 0..7 {
        let tri = [sp[d], sp[(d + 2) % 7], sp[(d + 4) % 7]];
        if !tri.iter().all(|&pc| allowed(ctx, pc))
            || !tri.iter().any(|pc| ctx.palette.tensions.contains(pc))
        {
            continue;
        }
        let base: Vec<i32> = req.iter().copied().filter(|pc| !tri.contains(pc)).collect();
        let total = base.len() + 3;
        if total < range.min_voices || total > range.max_voices {
            continue;
        }
        let bases: Vec<Vec<i32>> = if base.len() == 2 {
            vec![base.clone(), vec![base[1], base[0]]]
        } else {
            vec![base]
        };
        for b in &bases {
            for r in rotations(&tri) {
                out.push((
                    VoicingShape::UpperStructure,
                    b.iter().chain(&r).copied().collect(),
                ));
            }
        }
        found += 1;
        if found >= 3 {
            break;
        }
    }
    out
}

/// Every placement of the pitch-class stack `seq` inside `[low, high]`: each tone the first pitch
/// of its class above the one below.
fn place(seq: &[i32], low: Midi, high: Midi) -> Vec<Vec<Midi>> {
    let mut out = Vec::new();
    if seq.is_empty() {
        return out;
    }
    for start in low..=high {
        if pitch_class(start) != seq[0].rem_euclid(12) {
            continue;
        }
        let mut v = vec![start];
        for &pc in &seq[1..] {
            let mut p = v[v.len() - 1] + 1;
            while pitch_class(p) != pc.rem_euclid(12) {
                p += 1;
            }
            v.push(p);
        }
        if v[v.len() - 1] <= high {
            out.push(v);
        }
    }
    out
}

/// The low-interval limits: a pair whose lower note is below MIDI 48 must be at least a fifth
/// apart, below MIDI 55 at least a minor third (no mud in the bottom of the voicing).
fn low_intervals_ok(v: &[Midi]) -> bool {
    v.windows(2).all(|w| {
        let d = w[1] - w[0];
        if w[0] < 48 {
            d >= 7
        } else if w[0] < 55 {
            d >= 3
        } else {
            true
        }
    })
}

/// The hard filters. `strict = false` relaxes only the low-interval limits and the upper-layer
/// floor (a last resort before the legacy fallback).
fn admissible(
    ctx: &HarmonicContext,
    v: &[Midi],
    range: &VoiceRange,
    req: &[i32],
    strict: bool,
) -> bool {
    let n = v.len();
    n >= range.min_voices
        && n <= range.max_voices
        && v.windows(2).all(|w| w[0] < w[1])
        && v[0] >= range.low
        && v[n - 1] <= range.high
        && (!strict || range.min_top.is_none_or(|t| v[n - 1] >= t))
        && v.iter().all(|&p| allowed(ctx, pitch_class(p)))
        && req
            .iter()
            .all(|&pc| v.iter().any(|&p| pitch_class(p) == pc))
        && (!strict || low_intervals_ok(v))
}

fn centroid(v: &[Midi]) -> f32 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<Midi>() as f32 / v.len() as f32
    }
}

fn span(v: &[Midi]) -> i32 {
    match (v.first(), v.last()) {
        (Some(a), Some(b)) => b - a,
        _ => 0,
    }
}

fn build_candidates(
    ctx: &HarmonicContext,
    range: &VoiceRange,
    family: ShapeFamily,
    legacy: Option<&Voicing>,
    strict: bool,
) -> Vec<VoicingCandidate> {
    let req = required_pcs(ctx);
    let mut seen: Vec<Vec<Midi>> = Vec::new();
    let mut out: Vec<VoicingCandidate> = Vec::new();
    if family == ShapeFamily::Full {
        if let Some(l) = legacy {
            if !l.voices.is_empty() && admissible(ctx, &l.voices, range, &req, strict) {
                seen.push(l.voices.clone());
                out.push(VoicingCandidate {
                    voices: l.voices.clone(),
                    shape: VoicingShape::Legacy,
                });
            }
        }
    }
    let mut groups: BTreeMap<VoicingShape, Vec<Vec<Midi>>> = BTreeMap::new();
    let mut offer = |shape: VoicingShape, v: Vec<Midi>, seen: &mut Vec<Vec<Midi>>| {
        if admissible(ctx, &v, range, &req, strict) && !seen.contains(&v) {
            seen.push(v.clone());
            groups.entry(shape).or_default().push(v);
        }
    };
    for (shape, seq) in shapes(ctx, range, family) {
        for v in place(&seq, range.low, range.high) {
            if shape == VoicingShape::Close && v.len() == 4 {
                let mut d = v.clone();
                d[2] -= 12;
                d.sort_unstable();
                offer(VoicingShape::Drop2, d, &mut seen);
            }
            offer(shape, v, &mut seen);
        }
    }
    // Nearest the register centre first within each family; then round-robin across families so
    // the bounded set keeps its variety.
    let c = range.center as f32;
    for g in groups.values_mut() {
        g.sort_by(|a, b| (centroid(a) - c).abs().total_cmp(&(centroid(b) - c).abs()));
    }
    let mut depth = 0;
    loop {
        let mut any = false;
        for shape in VoicingShape::ORDER {
            if out.len() >= MAX_CANDIDATES {
                return out;
            }
            if let Some(v) = groups.get(&shape).and_then(|g| g.get(depth)) {
                out.push(VoicingCandidate {
                    voices: v.clone(),
                    shape,
                });
                any = true;
            }
        }
        if !any {
            return out;
        }
        depth += 1;
    }
}

/// The bounded candidate voicings of `ctx` for `range` (at most [`MAX_CANDIDATES`]; never empty).
///
/// Hard filters: every pitch class is a chord tone or a licensed tension (so a realizer's
/// `function_over` is total), the [`required_pcs`] are all present, every pitch is inside the
/// window, the voice count is in range, and the low-interval limits hold. The `legacy` voicing, if
/// given and admissible, is candidate 0 (so the path can never lose to the control). If nothing is
/// admissible the low-interval limits are relaxed; failing even that, the legacy close voicing
/// (chord tones only — always justified) is the sole candidate.
pub fn candidates(
    ctx: &HarmonicContext,
    range: &VoiceRange,
    family: ShapeFamily,
    legacy: Option<&Voicing>,
) -> Vec<VoicingCandidate> {
    let strict = build_candidates(ctx, range, family, legacy, true);
    if !strict.is_empty() {
        return strict;
    }
    let relaxed = build_candidates(ctx, range, family, legacy, false);
    if !relaxed.is_empty() {
        return relaxed;
    }
    let v = legacy.cloned().unwrap_or_else(|| {
        VoiceLeader::new(range.low, range.high, 0.0).lead(
            &ctx.chord,
            range.legacy_voices,
            range.center,
        )
    });
    vec![VoicingCandidate {
        voices: v.voices,
        shape: VoicingShape::Legacy,
    }]
}

/// One step of a voice path: a harmony, its candidates, and what the backbone asks of it.
#[derive(Debug, Clone)]
pub struct PathStep<'a> {
    pub ctx: &'a HarmonicContext,
    pub range: VoiceRange,
    pub candidates: Vec<VoicingCandidate>,
    /// The backbone gesture of the bar the harmony starts in.
    pub gesture: Option<HarmonicGesture>,
    /// The backbone slot (a run of bars with one gesture in one cycle); gesture targets compare
    /// consecutive steps of the SAME slot only.
    pub slot: Option<u32>,
    /// The role re-enters after silence here: no motion, retention or continuity is owed.
    pub re_entry: bool,
    /// Lead pitches sounding over this harmony (the keys listen to them).
    pub melody: Vec<Midi>,
}

/// The inspectable cost of one step (a transition into a voicing plus its own static terms).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VoicePathCost {
    /// Symmetric nearest-neighbour motion (half the sum of each voice's distance to the nearest
    /// voice on the other side — voice counts may differ).
    pub motion: f32,
    /// Exact pitches retained from the previous voicing (a reward).
    pub common_tones: f32,
    /// Previous guide-tone voices with no guide tone of the new harmony within a whole step.
    /// A lexicographic tier of the [`PathKey`] (prior to every weighted term).
    pub guide_breaks: u32,
    /// Positional voice overlaps (a voice moving past its neighbour's old pitch).
    pub crossing: f32,
    /// Adjacent semitones (doubly at the top) and holes over an octave between upper voices.
    pub spacing: f32,
    /// Distance of the centroid from the role's register centre.
    pub register: f32,
    /// Distance of the span from the world's spread target.
    pub span: f32,
    /// Doubled pitch classes (a doubled guide tone counts double).
    pub doubling: f32,
    /// Voices a semitone against a sounding lead pitch, plus lead notes under the top voice.
    pub melody_clash: f32,
    /// Gesture violations inside one slot: a Lift's centroid falling, an Open's span shrinking, a
    /// Reset's span widening (semitones). The first lexicographic tier of the [`PathKey`].
    pub gesture: f32,
}

impl VoicePathCost {
    fn add(&mut self, o: &VoicePathCost) {
        self.motion += o.motion;
        self.common_tones += o.common_tones;
        self.guide_breaks += o.guide_breaks;
        self.crossing += o.crossing;
        self.spacing += o.spacing;
        self.register += o.register;
        self.span += o.span;
        self.doubling += o.doubling;
        self.melody_clash += o.melody_clash;
        self.gesture += o.gesture;
    }
}

/// Weights of the non-lexicographic cost terms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathWeights {
    pub motion: f32,
    /// Per retained pitch (doubled on a Deflect bar: the miss holds what it shares).
    pub common_tone: f32,
    pub crossing: f32,
    pub spacing: f32,
    pub register: f32,
    pub span: f32,
    pub doubling: f32,
    pub melody: f32,
    /// Scales the gesture tier of the [`PathKey`] (0 switches the backbone's targets off).
    pub gesture: f32,
}

impl Default for PathWeights {
    fn default() -> Self {
        PathWeights {
            motion: 1.0,
            common_tone: 1.0,
            crossing: 2.0,
            spacing: 3.0,
            register: 0.35,
            span: 0.2,
            doubling: 1.5,
            melody: 2.0,
            gesture: 4.0,
        }
    }
}

/// Symmetric nearest-neighbour motion between two voicings (voice counts may differ).
pub fn nn_motion(a: &[Midi], b: &[Midi]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let near = |x: Midi, set: &[Midi]| set.iter().map(|&y| (x - y).abs()).min().unwrap_or(0);
    let ab: i32 = a.iter().map(|&x| near(x, b)).sum();
    let ba: i32 = b.iter().map(|&y| near(y, a)).sum();
    (ab + ba) as f32 / 2.0
}

/// Guide-tone voices of `a` (classes in `ga`) with no guide tone of the next harmony (classes in
/// `gb`) within a whole step in `b` — a broken guide-tone line.
pub fn guide_breaks(a: &[Midi], ga: &[i32], b: &[Midi], gb: &[i32]) -> u32 {
    a.iter()
        .filter(|&&x| ga.contains(&pitch_class(x)))
        .filter(|&&x| {
            !b.iter()
                .any(|&y| (x - y).abs() <= 2 && gb.contains(&pitch_class(y)))
        })
        .count() as u32
}

fn overlaps(a: &[Midi], b: &[Midi]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let n = a.len();
    (0..n)
        .filter(|&i| (i + 1 < n && b[i] > a[i + 1]) || (i >= 1 && b[i] < a[i - 1]))
        .count() as f32
}

fn spacing_penalty(v: &[Midi]) -> f32 {
    let n = v.len();
    let mut pen = 0.0;
    for i in 0..n.saturating_sub(1) {
        let d = v[i + 1] - v[i];
        if d == 1 {
            pen += if i + 2 == n { 3.0 } else { 1.0 };
        }
        if i >= 1 && d > 12 {
            pen += 1.0;
        }
    }
    pen
}

fn doubling_penalty(v: &[Midi], ctx: &HarmonicContext) -> f32 {
    let mut pen = 0.0;
    for (i, &p) in v.iter().enumerate() {
        let pc = pitch_class(p);
        if v[..i].iter().any(|&q| pitch_class(q) == pc) {
            pen += if ctx.palette.guide_tones.contains(&pc) {
                2.0
            } else {
                1.0
            };
        }
    }
    pen
}

fn melody_clash(v: &[Midi], melody: &[Midi]) -> f32 {
    let top = v.last().copied().unwrap_or(Midi::MIN);
    melody
        .iter()
        .map(|&m| {
            let semitone = v.iter().any(|&p| {
                let d = (p - m).rem_euclid(12);
                d == 1 || d == 11
            });
            (semitone as u8 + (top > m) as u8) as f32
        })
        .sum()
}

/// The cost of voicing `step` with `v` after `prev` (the previous step's voicing and step).
pub fn step_cost(prev: Option<(&[Midi], &PathStep)>, v: &[Midi], step: &PathStep) -> VoicePathCost {
    let mut c = VoicePathCost {
        spacing: spacing_penalty(v),
        register: (centroid(v) - step.range.center as f32).abs(),
        span: (span(v) as f32 - step.range.target_span).abs(),
        doubling: doubling_penalty(v, step.ctx),
        melody_clash: melody_clash(v, &step.melody),
        ..VoicePathCost::default()
    };
    if let Some((a, ps)) = prev {
        if !step.re_entry && !a.is_empty() {
            c.motion = nn_motion(a, v);
            c.common_tones = a.iter().filter(|p| v.contains(p)).count() as f32;
            c.guide_breaks = guide_breaks(
                a,
                &ps.ctx.palette.guide_tones,
                v,
                &step.ctx.palette.guide_tones,
            );
            c.crossing = overlaps(a, v);
            if step.slot.is_some() && step.slot == ps.slot {
                c.gesture = match step.gesture {
                    Some(HarmonicGesture::Lift) => (centroid(a) - centroid(v)).max(0.0),
                    Some(HarmonicGesture::Open) => (span(a) - span(v)).max(0) as f32,
                    Some(HarmonicGesture::Reset) => (span(v) - span(a)).max(0) as f32,
                    _ => 0.0,
                };
            }
        }
    }
    c
}

/// The weighted scalar of a step's non-lexicographic terms (everything but the guide-tone breaks
/// and the gesture violations, which are tiers of their own).
pub fn weigh(c: &VoicePathCost, step: &PathStep, w: &PathWeights) -> f64 {
    let ct = if step.gesture == Some(HarmonicGesture::Deflect) {
        2.0 * w.common_tone
    } else {
        w.common_tone
    };
    (w.motion * c.motion - ct * c.common_tones
        + w.crossing * c.crossing
        + w.spacing * c.spacing
        + w.register * c.register
        + w.span * c.span
        + w.doubling * c.doubling
        + w.melody * c.melody_clash) as f64
}

/// The gesture tier of a step: its weighted gesture violation in twelfths of a semitone (an exact
/// integer for 2–4 voice centroids, so the lexicographic sum never rounds).
pub fn gesture_tier(c: &VoicePathCost, w: &PathWeights) -> i64 {
    (12.0 * w.gesture as f64 * c.gesture as f64).round() as i64
}

/// A path's lexicographic key: the backbone's gesture violations first (a Lift that sinks, an Open
/// that closes, a Reset that widens — satisfied whenever the candidates allow it), then guide-tone
/// breaks, then the weighted rest. Guide-tone continuity is therefore strictly prior to motion,
/// retention and spacing: no amount of smoothness buys a broken 3rd/7th line. Every tier is summed
/// along the path; the Viterbi is exact for this order because each tier's addition is monotone
/// (the first two are integers).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathKey {
    pub guide_breaks: u32,
    pub gesture: i64,
    pub weighted: f64,
}

impl PathKey {
    const ZERO: PathKey = PathKey {
        guide_breaks: 0,
        gesture: 0,
        weighted: 0.0,
    };

    fn plus(self, o: PathKey) -> PathKey {
        PathKey {
            guide_breaks: self.guide_breaks + o.guide_breaks,
            gesture: self.gesture + o.gesture,
            weighted: self.weighted + o.weighted,
        }
    }

    /// The integer tiers in priority order.
    fn tiers(&self) -> (i64, u32) {
        (self.gesture, self.guide_breaks)
    }

    /// Strictly better (lexicographically smaller).
    pub fn lt(&self, o: &PathKey) -> bool {
        self.tiers() < o.tiers() || (self.tiers() == o.tiers() && self.weighted < o.weighted)
    }

    /// No worse than `o`, allowing `eps` of float slack on the weighted term.
    pub fn le(&self, o: &PathKey, eps: f64) -> bool {
        self.tiers() < o.tiers() || (self.tiers() == o.tiers() && self.weighted <= o.weighted + eps)
    }
}

fn key_of(
    prev: Option<(&[Midi], &PathStep)>,
    v: &[Midi],
    step: &PathStep,
    w: &PathWeights,
) -> PathKey {
    let c = step_cost(prev, v, step);
    PathKey {
        guide_breaks: c.guide_breaks,
        gesture: gesture_tier(&c, w),
        weighted: weigh(&c, step, w),
    }
}

/// A chosen voicing per step, with its inspectable costs.
#[derive(Debug, Clone, PartialEq)]
pub struct VoicePath {
    /// The candidate index chosen at each step.
    pub choice: Vec<usize>,
    pub voicings: Vec<VoicingCandidate>,
    /// Per-step cost vectors.
    pub costs: Vec<VoicePathCost>,
    /// Their sum.
    pub total: VoicePathCost,
    /// The lexicographic key of the whole path.
    pub key: PathKey,
}

/// Evaluate a given choice of candidates (one index per step) under the path cost.
pub fn evaluate_path(steps: &[PathStep], choice: &[usize], w: &PathWeights) -> VoicePath {
    let mut out = VoicePath {
        choice: choice.to_vec(),
        voicings: Vec::with_capacity(steps.len()),
        costs: Vec::with_capacity(steps.len()),
        total: VoicePathCost::default(),
        key: PathKey::ZERO,
    };
    for (t, (step, &j)) in steps.iter().zip(choice).enumerate() {
        let v = &step.candidates[j];
        let prev = (t > 0).then(|| {
            (
                steps[t - 1].candidates[choice[t - 1]].voices.as_slice(),
                &steps[t - 1],
            )
        });
        let c = step_cost(prev, &v.voices, step);
        out.key = out.key.plus(PathKey {
            guide_breaks: c.guide_breaks,
            gesture: gesture_tier(&c, w),
            weighted: weigh(&c, step, w),
        });
        out.total.add(&c);
        out.costs.push(c);
        out.voicings.push(v.clone());
    }
    out
}

/// The globally best path: a Viterbi over the steps' candidates, minimizing the lexicographic
/// [`PathKey`] (gesture violations, then guide-tone breaks, then the weighted terms).
/// Deterministic: exact ties go to the lower candidate index.
pub fn voice_path(steps: &[PathStep], w: &PathWeights) -> VoicePath {
    if steps.is_empty() || steps.iter().any(|s| s.candidates.is_empty()) {
        return evaluate_path(&[], &[], w);
    }
    let mut dp: Vec<Vec<PathKey>> = Vec::with_capacity(steps.len());
    let mut back: Vec<Vec<usize>> = Vec::with_capacity(steps.len());
    dp.push(
        steps[0]
            .candidates
            .iter()
            .map(|c| key_of(None, &c.voices, &steps[0], w))
            .collect(),
    );
    back.push(vec![0; steps[0].candidates.len()]);
    for t in 1..steps.len() {
        let mut row = Vec::with_capacity(steps[t].candidates.len());
        let mut brow = Vec::with_capacity(steps[t].candidates.len());
        for cb in &steps[t].candidates {
            let mut best: Option<(PathKey, usize)> = None;
            for (i, ca) in steps[t - 1].candidates.iter().enumerate() {
                let k = dp[t - 1][i].plus(key_of(
                    Some((&ca.voices, &steps[t - 1])),
                    &cb.voices,
                    &steps[t],
                    w,
                ));
                if best.is_none_or(|(b, _)| k.lt(&b)) {
                    best = Some((k, i));
                }
            }
            let (k, i) = best.expect("non-empty candidates");
            row.push(k);
            brow.push(i);
        }
        dp.push(row);
        back.push(brow);
    }
    let last = dp.len() - 1;
    let mut j = 0;
    for (i, k) in dp[last].iter().enumerate() {
        if k.lt(&dp[last][j]) {
            j = i;
        }
    }
    let mut choice = vec![0; steps.len()];
    for t in (0..steps.len()).rev() {
        choice[t] = j;
        j = back[t][j];
    }
    evaluate_path(steps, &choice, w)
}

/// The control: the SAME candidates and cost, chosen one step at a time (each step's best given
/// the previous choice). Ties go to the lower candidate index.
pub fn greedy_path(steps: &[PathStep], w: &PathWeights) -> VoicePath {
    let mut choice: Vec<usize> = Vec::with_capacity(steps.len());
    for (t, step) in steps.iter().enumerate() {
        let prev = (t > 0).then(|| {
            (
                steps[t - 1].candidates[choice[t - 1]].voices.as_slice(),
                &steps[t - 1],
            )
        });
        let mut j = 0;
        let mut best: Option<PathKey> = None;
        for (i, c) in step.candidates.iter().enumerate() {
            let k = key_of(prev, &c.voices, step, w);
            if best.is_none_or(|b| k.lt(&b)) {
                best = Some(k);
                j = i;
            }
        }
        if step.candidates.is_empty() {
            return evaluate_path(&[], &[], w);
        }
        choice.push(j);
    }
    evaluate_path(steps, &choice, w)
}

/// Backbone slot ids per bar: a new id whenever the (gesture, cycle) pair changes.
fn slot_ids(perf: &PerformancePlan) -> Vec<Option<u32>> {
    let mut out = Vec::with_capacity(perf.ensemble.len());
    let mut id = 0u32;
    let mut last: Option<(HarmonicGesture, u32)> = None;
    for eb in &perf.ensemble {
        match eb.gesture {
            None => {
                last = None;
                out.push(None);
            }
            Some(g) => {
                if last != Some((g, eb.cycle)) {
                    id += 1;
                    last = Some((g, eb.cycle));
                }
                out.push(Some(id));
            }
        }
    }
    out
}

/// Lead pitches sounding anywhere in `[a, b)`.
fn sounding(lead: &[Note], a: f64, b: f64) -> Vec<Midi> {
    lead.iter()
        .filter(|n| n.start_beat < b - 1e-6 && n.start_beat + n.dur_beats as f64 > a + 1e-6)
        .map(|n| n.pitch)
        .collect()
}

/// The pad's steps over `perf`: one per sounding harmony, its candidate family and window picked by
/// the bar's [`PadMode`] (Shell → shells; UpperStructure → the upper window, a voice ≥ MIDI 79;
/// otherwise full). Returns the context index of each step alongside.
pub fn pad_steps(perf: &PerformancePlan, spread: f32) -> (Vec<usize>, Vec<PathStep<'_>>) {
    let slots = slot_ids(perf);
    // The legacy control, called exactly as the old pad called it (every bar-mapped harmony).
    let mut vl = VoiceLeader::new(52, 79, spread);
    let mut ix = Vec::new();
    let mut steps = Vec::new();
    let mut last: Option<usize> = None;
    for (ci, ctx) in perf.contexts.iter().enumerate() {
        let Some(eb) = perf.bar_at(ctx.start_beat) else {
            continue;
        };
        let legacy = vl.lead(&ctx.chord, 4, 67);
        let (range, family) = match eb.pad {
            PadMode::Silent => continue,
            PadMode::Shell => (VoiceRange::pad_shell(spread), ShapeFamily::Shell),
            PadMode::UpperStructure => (VoiceRange::pad_upper(spread), ShapeFamily::Full),
            PadMode::Sustain | PadMode::CommonToneCarry | PadMode::Swell => {
                (VoiceRange::pad(spread), ShapeFamily::Full)
            }
        };
        steps.push(PathStep {
            ctx,
            range,
            candidates: candidates(ctx, &range, family, Some(&legacy)),
            gesture: eb.gesture,
            slot: slots.get(eb.bar as usize).copied().flatten(),
            re_entry: last.is_none_or(|l| l + 1 != ci),
            melody: Vec::new(),
        });
        ix.push(ci);
        last = Some(ci);
    }
    (ix, steps)
}

/// The keys' steps over `perf`: one per harmony the keys play in (any bar it touches not
/// [`KeysMode::Space`]), exactly `n` voices, listening to the `lead` pitches sounding over it.
pub fn keys_steps<'a>(
    perf: &'a PerformancePlan,
    spread: f32,
    lead: &[Note],
    n: usize,
) -> (Vec<usize>, Vec<PathStep<'a>>) {
    let range = VoiceRange::keys(n, spread);
    let slots = slot_ids(perf);
    let mut vl = VoiceLeader::new(58, 84, spread);
    let mut ix = Vec::new();
    let mut steps = Vec::new();
    let mut last: Option<usize> = None;
    for (ci, ctx) in perf.contexts.iter().enumerate() {
        let end = ctx.start_beat + ctx.dur_beats as f64;
        let first_bar = (ctx.start_beat / BEATS_PER_BAR).floor().max(0.0) as u32;
        let last_bar = ((end - 1e-6) / BEATS_PER_BAR).floor().max(0.0) as u32;
        let plays = (first_bar..=last_bar.max(first_bar))
            .any(|b| perf.bar(b).is_some_and(|eb| eb.keys != KeysMode::Space));
        if !plays {
            continue;
        }
        let eb = perf.bar_at(ctx.start_beat);
        let legacy = vl.lead(&ctx.chord, range.legacy_voices, 72);
        steps.push(PathStep {
            ctx,
            range,
            candidates: candidates(ctx, &range, ShapeFamily::Full, Some(&legacy)),
            gesture: eb.and_then(|e| e.gesture),
            slot: eb.and_then(|e| slots.get(e.bar as usize).copied().flatten()),
            re_entry: last.is_none_or(|l| l + 1 != ci),
            melody: sounding(lead, ctx.start_beat, end),
        });
        ix.push(ci);
        last = Some(ci);
    }
    (ix, steps)
}

/// A role's solved voice path, addressable by harmony (context index).
#[derive(Debug, Clone)]
pub struct RolePath {
    /// The context index of each step (ascending).
    pub context_ix: Vec<usize>,
    /// Whether each step re-enters after silence.
    pub re_entry: Vec<bool>,
    pub path: VoicePath,
    range: VoiceRange,
}

impl RolePath {
    /// A role path from an already-chosen path over `steps` (context index per step) — the joint
    /// support solve ([`super::support`]) builds the pad's and the keys' paths this way.
    pub fn from_steps(
        ix: Vec<usize>,
        steps: &[PathStep],
        path: VoicePath,
        range: VoiceRange,
    ) -> RolePath {
        RolePath {
            re_entry: steps.iter().map(|s| s.re_entry).collect(),
            path,
            context_ix: ix,
            range,
        }
    }

    fn solve(ix: Vec<usize>, steps: &[PathStep], range: VoiceRange) -> RolePath {
        RolePath {
            re_entry: steps.iter().map(|s| s.re_entry).collect(),
            path: voice_path(steps, &PathWeights::default()),
            context_ix: ix,
            range,
        }
    }

    fn step_of(&self, ci: usize) -> Option<usize> {
        self.context_ix
            .binary_search(&ci)
            .ok()
            .filter(|&t| t < self.path.voicings.len())
    }

    /// The path's voicing for harmony `ci`, if the role sounds there.
    pub fn at(&self, ci: usize) -> Option<Voicing> {
        self.step_of(ci).map(|t| self.path.voicings[t].voicing())
    }

    /// The previous step's voicing when it sounded in the immediately preceding harmony (what a
    /// common-tone carry can hold at the exact same pitch).
    pub fn previous(&self, ci: usize) -> Option<Voicing> {
        let t = self.step_of(ci)?;
        (t > 0 && !self.re_entry[t] && self.context_ix[t - 1] + 1 == ci)
            .then(|| self.path.voicings[t - 1].voicing())
    }

    /// The voicing for harmony `ci` — the path's, or (off the path) the best standalone candidate.
    pub fn voicing(&self, ci: usize, ctx: &HarmonicContext) -> Voicing {
        self.at(ci)
            .unwrap_or_else(|| candidates(ctx, &self.range, ShapeFamily::Full, None)[0].voicing())
    }

    /// The voicing for the harmony sounding at `beat`.
    pub fn voicing_at(&self, perf: &PerformancePlan, beat: f64) -> Voicing {
        match perf
            .contexts
            .iter()
            .rposition(|c| c.start_beat <= beat + 1e-6)
        {
            Some(ci) => self.voicing(ci, &perf.contexts[ci]),
            None => Voicing { voices: Vec::new() },
        }
    }
}

/// The pad's voice path over `perf` (see [`pad_steps`]).
pub fn pad_path(perf: &PerformancePlan, spread: f32) -> RolePath {
    let (ix, steps) = pad_steps(perf, spread);
    RolePath::solve(ix, &steps, VoiceRange::pad(spread))
}

/// The keys' voice path over `perf` (see [`keys_steps`]).
pub fn keys_path(perf: &PerformancePlan, spread: f32, lead: &[Note], n: usize) -> RolePath {
    let (ix, steps) = keys_steps(perf, spread, lead, n);
    RolePath::solve(ix, &steps, VoiceRange::keys(n, spread))
}

/// The `k` voices of `v` a stab sounds, top first: `k = 2` → the guide-tone shell (3rd + 7th; over
/// a single-guide chord 3rd + 9th); `k = 3` → the shell plus the highest licensed tension (else the
/// highest remaining voice). `k ≥ len` sounds the whole voicing.
pub fn stab_voices(v: &Voicing, ctx: &HarmonicContext, k: usize) -> Vec<Midi> {
    if k >= v.voices.len() {
        return v.voices.iter().rev().copied().collect();
    }
    let req = required_pcs(ctx);
    let mut pick: Vec<Midi> = Vec::with_capacity(k);
    for pc in req.iter().take(2.min(k)) {
        if let Some(&p) = v
            .voices
            .iter()
            .rev()
            .find(|&&p| pitch_class(p) == *pc && !pick.contains(&p))
        {
            pick.push(p);
        }
    }
    if pick.len() < k {
        if let Some(&p) = v
            .voices
            .iter()
            .rev()
            .find(|&&p| !pick.contains(&p) && is_extension(ctx, pitch_class(p)))
        {
            pick.push(p);
        }
    }
    for &p in v.voices.iter().rev() {
        if pick.len() >= k {
            break;
        }
        if !pick.contains(&p) {
            pick.push(p);
        }
    }
    pick.sort_unstable_by(|a, b| b.cmp(a));
    pick
}

// -------------------------------------------------------------------------------------------------
// Measurement: what the pad and keys actually sounded.
// -------------------------------------------------------------------------------------------------

/// Voicing statistics for one role, measured from the Score's notes grouped by onset.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RoleVoicings {
    /// Onsets sounding two or more of the role's chordal notes.
    pub voicings: usize,
    /// Mean symmetric nearest-neighbour motion between consecutive voicings.
    pub mean_motion: f32,
    /// Guide-tone voices whose line breaks into the next voicing (no guide tone within a step).
    pub guide_breaks: usize,
    /// Voicings missing at least one of their harmony's guide tones.
    pub missing_guide: usize,
    /// Voicings with any adjacent-semitone pair.
    pub semitone_pairs: usize,
    /// Voicings whose top two voices are a semitone apart.
    pub top_semitone_pairs: usize,
    /// How many voicings of each shape (classified from the pitches).
    pub shapes: BTreeMap<&'static str, usize>,
}

/// Pad and keys voicing diagnostics — measured from the realized Score, not from the planner.
#[derive(Debug, Clone, PartialEq)]
pub struct VoicingDiagnostics {
    pub pad: RoleVoicings,
    pub keys: RoleVoicings,
}

fn classify(v: &[Midi], ctx: &HarmonicContext) -> &'static str {
    let pcs: Vec<i32> = v.iter().map(|&p| pitch_class(p)).collect();
    let req = required_pcs(ctx);
    let has_guides = ctx.palette.guide_tones.iter().all(|g| pcs.contains(g));
    let non_req = pcs.iter().filter(|pc| !req.contains(pc)).count();
    let tensions = pcs
        .iter()
        .filter(|pc| ctx.palette.tensions.contains(pc))
        .count();
    if v.len() <= 3 && has_guides && non_req <= 1 {
        "shell"
    } else if v.len() <= 3 && !has_guides {
        "partial"
    } else if tensions >= 2 {
        "upper-structure"
    } else if !pcs.contains(&ctx.chord.root_pc) {
        "rootless"
    } else if span(v) <= 12 {
        "close"
    } else {
        "open"
    }
}

fn measure_role(
    score: &Score,
    contexts: &[HarmonicContext],
    keep: impl Fn(&Note) -> bool,
) -> RoleVoicings {
    // Group by onset: (pitches, earliest end) per onset.
    let mut by_onset: BTreeMap<i64, (Vec<Midi>, f64)> = BTreeMap::new();
    for n in score.notes.iter().filter(|n| keep(n)) {
        let e = by_onset
            .entry((n.start_beat * 1000.0).round() as i64)
            .or_insert((Vec::new(), f64::INFINITY));
        e.0.push(n.pitch);
        e.1 = e.1.min(n.start_beat + n.dur_beats as f64);
    }
    // A staged entry (a swell's later voices) — an onset in the same harmony while every note of
    // the previous onset still sounds — joins that voicing rather than counting as its own.
    let mut groups: Vec<(f64, Vec<Midi>, f64)> = Vec::new();
    for (t, (pitches, end)) in by_onset {
        let beat = t as f64 / 1000.0;
        if let Some(last) = groups.last_mut() {
            let same_ctx = context_at(contexts, beat).map(|c| c.start_beat)
                == context_at(contexts, last.0).map(|c| c.start_beat);
            if same_ctx && last.2 > beat + 1e-6 {
                last.1.extend(pitches);
                last.2 = last.2.min(end);
                continue;
            }
        }
        groups.push((beat, pitches, end));
    }
    let mut out = RoleVoicings::default();
    let mut prev: Option<(Vec<Midi>, &HarmonicContext)> = None;
    let mut motion = 0.0f32;
    let mut pairs = 0usize;
    for (beat, mut v, _) in groups {
        v.sort_unstable();
        v.dedup();
        if v.len() < 2 {
            continue;
        }
        let Some(ctx) = context_at(contexts, beat) else {
            continue;
        };
        out.voicings += 1;
        let g = &ctx.palette.guide_tones;
        if !g.iter().all(|pc| v.iter().any(|&p| pitch_class(p) == *pc)) {
            out.missing_guide += 1;
        }
        if v.windows(2).any(|w| w[1] - w[0] == 1) {
            out.semitone_pairs += 1;
        }
        if v[v.len() - 1] - v[v.len() - 2] == 1 {
            out.top_semitone_pairs += 1;
        }
        *out.shapes.entry(classify(&v, ctx)).or_default() += 1;
        if let Some((a, actx)) = &prev {
            motion += nn_motion(a, &v);
            pairs += 1;
            out.guide_breaks += guide_breaks(a, &actx.palette.guide_tones, &v, g) as usize;
        }
        prev = Some((v, ctx));
    }
    out.mean_motion = if pairs == 0 {
        0.0
    } else {
        motion / pairs as f32
    };
    out
}

impl VoicingDiagnostics {
    /// Measure the pad (`"pad"` notes) and keys (`"hold"` / `"comp"` notes) voicings of `score`
    /// against the harmonies in `contexts`, grouping each role's notes by onset (a staged entry —
    /// a later onset in the same harmony while the earlier notes all still sound — joins its
    /// voicing). Single-note onsets are not voicings and are skipped.
    pub fn measure(score: &Score, contexts: &[HarmonicContext]) -> VoicingDiagnostics {
        VoicingDiagnostics {
            pad: measure_role(score, contexts, |n| {
                n.role == Role::Pad && n.prov.role_note == "pad"
            }),
            keys: measure_role(score, contexts, |n| {
                n.role == Role::Keys && matches!(n.prov.role_note, "hold" | "comp")
            }),
        }
    }

    /// A compact, human-readable report. Measurements of the realized score, not verdicts.
    pub fn report(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "voicing diagnostics (pad/keys voicings grouped by onset — the actual score):"
        );
        for (name, r) in [("pad", &self.pad), ("keys", &self.keys)] {
            let shapes = r
                .shapes
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(" ");
            let _ = writeln!(
                s,
                "  {name}: voicings={} mean_motion={:.2} guide_breaks={} missing_guide={} semitone_pairs={} top_semitone_pairs={}  shapes: {shapes}",
                r.voicings,
                r.mean_motion,
                r.guide_breaks,
                r.missing_guide,
                r.semitone_pairs,
                r.top_semitone_pairs,
            );
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::super::theory::Quality;
    use super::*;

    #[test]
    fn common_tones_reduce_motion_between_related_chords() {
        // C major -> A minor share C and E; motion should be small.
        let mut vl = VoiceLeader::new(48, 84, 0.3);
        let c = vl.lead(&Chord::new(0, Quality::Maj), 4, 67);
        let a = vl.lead(&Chord::new(9, Quality::Min), 4, 67);
        let motion = c.motion_to(&a);
        assert!(
            motion <= 6,
            "Cmaj->Amin motion {motion} too large for related chords"
        );
    }

    #[test]
    fn voices_stay_in_register() {
        let mut vl = VoiceLeader::new(55, 79, 0.4);
        for root in [0, 5, 7, 2, 9] {
            let v = vl.lead(&Chord::new(root, Quality::Maj7), 4, 67);
            for &p in &v.voices {
                assert!((55..=79 + 12).contains(&p), "voice {p} out of register");
            }
            assert!(v.voices.windows(2).all(|w| w[0] <= w[1]), "not sorted");
        }
    }

    #[test]
    fn voicing_contains_the_chord_tones() {
        let mut vl = VoiceLeader::new(48, 84, 0.3);
        let v = vl.lead(&Chord::new(0, Quality::Maj7), 4, 67);
        let pcs: std::collections::HashSet<i32> =
            v.voices.iter().map(|p| p.rem_euclid(12)).collect();
        // Root, third, seventh must all be present in a 4-voice maj7 voicing.
        for tone in [0, 4, 11] {
            assert!(
                pcs.contains(&tone),
                "maj7 voicing missing pc {tone}: {:?}",
                v.voices
            );
        }
    }

    #[test]
    fn open_spread_is_wider_than_close() {
        let mut tight = VoiceLeader::new(48, 84, 0.2);
        let mut open = VoiceLeader::new(48, 84, 0.9);
        let vt = tight.lead(&Chord::new(0, Quality::Maj), 4, 67);
        let vo = open.lead(&Chord::new(0, Quality::Maj), 4, 67);
        let span = |v: &Voicing| v.voices.last().unwrap() - v.voices.first().unwrap();
        assert!(
            span(&vo) >= span(&vt),
            "open span {} !>= tight span {}",
            span(&vo),
            span(&vt)
        );
    }

    // --- Round VIIb: the voice path ------------------------------------------------------------

    use super::super::context::analyze;
    use super::super::contract::CompositionGrammar;
    use super::super::functor::compose_full;
    use super::super::harmony::ChordSpan;
    use super::super::performance::PerformanceOptions;
    use super::super::semantic::deflected_lift_trace;
    use super::super::theory::{Function, Mode, Scale};
    use super::super::world::MusicWorld;

    const BLACK_ICE: &str = "Bm7b5 E7 Fmaj7 Cadd9 Am6 Bm7b5 Gmaj7 B7 E7 Fmaj7 Dm7 Fmaj7 Dm7 Cadd9 Em9 Cadd9 Em9 Am6 Dm6 Am6 Am6 Bm7b5 E7 Fmaj7 Dm7 Cadd9 Em9 Cmaj9 Em9 Am6 Dm6";

    fn parse_chord(s: &str) -> Chord {
        let b = s.as_bytes();
        let letter = match b[0] {
            b'C' => 0,
            b'D' => 2,
            b'E' => 4,
            b'F' => 5,
            b'G' => 7,
            b'A' => 9,
            b'B' => 11,
            _ => panic!("bad chord {s}"),
        };
        let (root, rest) = match b.get(1) {
            Some(b'#') => (letter + 1, &s[2..]),
            Some(b'b') => (letter - 1, &s[2..]),
            _ => (letter, &s[1..]),
        };
        let q = match rest {
            "" => Quality::Maj,
            "m" => Quality::Min,
            "7" => Quality::Dom7,
            "maj7" => Quality::Maj7,
            "m7" => Quality::Min7,
            "m7b5" => Quality::Min7b5,
            "dim" => Quality::Dim,
            "dim7" => Quality::Dim7,
            "add9" => Quality::Add9,
            "6" => Quality::Maj6,
            "m6" => Quality::Min6,
            "maj9" => Quality::Maj9,
            "m9" => Quality::Min9,
            "9" => Quality::Dom9,
            other => panic!("bad quality {other}"),
        };
        Chord::new(root, q)
    }

    fn contexts_of(chords: &[Chord], region: Scale) -> Vec<HarmonicContext> {
        let spans: Vec<ChordSpan> = chords
            .iter()
            .enumerate()
            .map(|(i, &chord)| ChordSpan {
                start_beat: i as f64 * 4.0,
                dur_beats: 4.0,
                chord,
                function: Function::Tonic,
                degree: -1,
                note: "",
            })
            .collect();
        analyze(&spans, &region)
    }

    /// ii–V–I–vi ×4 in all twelve keys, and the BLACK_ICE spine in A Aeolian.
    fn sequences() -> Vec<(String, Vec<HarmonicContext>)> {
        let mut out = Vec::new();
        for key in 0..12 {
            let cell = [
                Chord::new(key + 2, Quality::Min7),
                Chord::new(key + 7, Quality::Dom7),
                Chord::new(key, Quality::Maj7),
                Chord::new(key + 9, Quality::Min7),
            ];
            let chords: Vec<Chord> = cell.iter().cycle().take(16).copied().collect();
            out.push((
                format!("ii-V-I-vi in {key}"),
                contexts_of(&chords, Scale::new(key, Mode::Ionian)),
            ));
        }
        let bi: Vec<Chord> = BLACK_ICE.split_whitespace().map(parse_chord).collect();
        out.push((
            "BLACK_ICE".into(),
            contexts_of(&bi, Scale::new(9, Mode::Aeolian)),
        ));
        out
    }

    /// Plain steps (no gestures, no melody) with the legacy control threaded through them.
    fn plain_steps(ctxs: &[HarmonicContext], range: VoiceRange, spread: f32) -> Vec<PathStep<'_>> {
        let mut vl = VoiceLeader::new(range.low, range.high, spread);
        ctxs.iter()
            .enumerate()
            .map(|(i, ctx)| {
                let legacy = vl.lead(&ctx.chord, range.legacy_voices, range.center);
                PathStep {
                    ctx,
                    range,
                    candidates: candidates(ctx, &range, ShapeFamily::Full, Some(&legacy)),
                    gesture: None,
                    slot: None,
                    re_entry: i == 0,
                    melody: Vec::new(),
                }
            })
            .collect()
    }

    fn flagship(world: &MusicWorld) -> super::super::functor::Composition {
        compose_full(
            &deflected_lift_trace(120.0),
            world,
            2112,
            Some(CompositionGrammar::DeflectedLift),
            PerformanceOptions::default(),
        )
    }

    fn has_guides(v: &[Midi], ctx: &HarmonicContext) -> bool {
        ctx.palette
            .guide_tones
            .iter()
            .all(|g| v.iter().any(|&p| pitch_class(p) == *g))
    }

    #[test]
    fn dp_never_worse_than_greedy_or_legacy() {
        // A theorem, not a tuning: the Viterbi is exact for the lexicographic key, the greedy
        // control picks from the same candidates under the same cost, and the legacy voicing is
        // candidate 0 whenever admissible — any failure here is a bug.
        let w = PathWeights::default();
        let mut legacy_feasible = 0;
        let mut compared = 0;
        for (name, ctxs) in sequences() {
            for (label, range) in [
                ("pad", VoiceRange::pad(0.35)),
                ("keys3", VoiceRange::keys(3, 0.35)),
                ("keys4", VoiceRange::keys(4, 0.35)),
            ] {
                let steps = plain_steps(&ctxs, range, 0.35);
                for s in &steps {
                    assert!(!s.candidates.is_empty() && s.candidates.len() <= MAX_CANDIDATES);
                }
                let dp = voice_path(&steps, &w);
                let gr = greedy_path(&steps, &w);
                assert_eq!(dp, evaluate_path(&steps, &dp.choice, &w), "{name}/{label}");
                assert!(
                    dp.key.le(&gr.key, 1e-6),
                    "{name}/{label}: dp {:?} worse than greedy {:?}",
                    dp.key,
                    gr.key
                );
                assert!(dp.total.guide_breaks <= gr.total.guide_breaks);
                compared += 1;
                if steps
                    .iter()
                    .all(|s| s.candidates[0].shape == VoicingShape::Legacy)
                {
                    legacy_feasible += 1;
                    let lp = evaluate_path(&steps, &vec![0; steps.len()], &w);
                    assert!(
                        dp.key.le(&lp.key, 1e-6),
                        "{name}/{label}: dp {:?} worse than legacy {:?}",
                        dp.key,
                        lp.key
                    );
                    if name == "ii-V-I-vi in 0" || name == "BLACK_ICE" {
                        eprintln!(
                            "{name}/{label}: legacy {:?} greedy {:?} dp {:?}",
                            lp.key, gr.key, dp.key
                        );
                    }
                } else if name == "BLACK_ICE" {
                    eprintln!(
                        "{name}/{label}: legacy infeasible; greedy {:?} dp {:?}",
                        gr.key, dp.key
                    );
                }
            }
        }
        assert_eq!(compared, 13 * 3);
        assert!(
            legacy_feasible > 0,
            "the legacy control was never admissible — the comparison is vacuous"
        );
    }

    #[test]
    fn keys_shells_carry_both_guide_tones() {
        let bi: Vec<Chord> = BLACK_ICE.split_whitespace().map(parse_chord).collect();
        let ctxs = contexts_of(&bi, Scale::new(9, Mode::Aeolian));
        for n in [3, 4] {
            let steps = plain_steps(&ctxs, VoiceRange::keys(n, 0.35), 0.35);
            let dp = voice_path(&steps, &PathWeights::default());
            for (step, v) in steps.iter().zip(&dp.voicings) {
                let v = v.voicing();
                for k in [2, 3] {
                    let sub = stab_voices(&v, step.ctx, k);
                    assert_eq!(sub.len(), k.min(v.voices.len()));
                    assert!(
                        has_guides(&sub, step.ctx),
                        "n={n} k={k} {}: stab {:?} misses a guide tone of {:?}",
                        step.ctx.chord.label(),
                        sub,
                        step.ctx.palette.guide_tones
                    );
                }
            }
        }
        // Negative control: the legacy leader's top-3 / top-2 "shells" miss guide tones here.
        let mut vl = VoiceLeader::new(58, 84, 0.35);
        let mut misses = 0;
        for ctx in &ctxs {
            let v = vl.lead(&ctx.chord, 3, 72);
            for take in [3, 2] {
                let sub: Vec<Midi> = v.voices.iter().rev().take(take).copied().collect();
                if !has_guides(&sub, ctx) {
                    misses += 1;
                }
            }
        }
        eprintln!("legacy keys stabs missing a guide tone on BLACK_ICE: {misses}/62");
        assert!(
            misses > 0,
            "the negative control no longer fails — it controls nothing"
        );
    }

    #[test]
    fn path_pcs_are_chord_or_licensed() {
        for world in MusicWorld::all() {
            let c = flagship(&world);
            let lead: Vec<Note> = c
                .score
                .notes
                .iter()
                .filter(|n| n.role == Role::Lead)
                .copied()
                .collect();
            let n = if c.perf.language.shell_voicings { 3 } else { 4 };
            let (_, pad) = pad_steps(&c.perf, world.voicing_spread);
            let (_, keys) = keys_steps(&c.perf, world.voicing_spread, &lead, n);
            for (role, steps) in [("pad", &pad), ("keys", &keys)] {
                assert!(!steps.is_empty(), "{}: no {role} steps", world.name);
                let path = voice_path(steps, &PathWeights::default());
                for (step, v) in steps.iter().zip(&path.voicings) {
                    for &p in &v.voices {
                        assert!(
                            allowed(step.ctx, pitch_class(p)),
                            "{} {role}: {p} over {} is neither chord tone nor licensed",
                            world.name,
                            step.ctx.chord.label()
                        );
                    }
                    if let Some(t) = step.range.min_top {
                        assert!(v.voices.last().is_some_and(|&top| top >= t));
                    }
                }
            }
            for n in c.score.notes.iter().filter(|n| {
                matches!(n.role, Role::Pad | Role::Keys)
                    && matches!(n.prov.role_note, "pad" | "hold" | "comp")
            }) {
                assert!(
                    n.function.is_some(),
                    "{}: unjustified {:?} note {} at {}",
                    world.name,
                    n.role,
                    n.pitch,
                    n.start_beat
                );
            }
        }
    }

    /// Consecutive same-slot Lift pairs whose centroid falls, and how many pairs were checked.
    fn lift_falls(steps: &[PathStep], path: &VoicePath) -> (usize, usize) {
        let mut falls = 0;
        let mut checked = 0;
        for t in 1..steps.len() {
            let (a, b) = (&steps[t - 1], &steps[t]);
            if b.re_entry || b.gesture != Some(HarmonicGesture::Lift) || b.slot.is_none() {
                continue;
            }
            if a.slot != b.slot {
                continue;
            }
            checked += 1;
            if centroid(&path.voicings[t].voices) < centroid(&path.voicings[t - 1].voices) - 1e-6 {
                falls += 1;
            }
        }
        (falls, checked)
    }

    #[test]
    fn lift_centroid_never_falls_within_a_slot() {
        let w = PathWeights::default();
        let off = PathWeights {
            gesture: 0.0,
            ..PathWeights::default()
        };
        let mut checked = 0;
        let mut falls_off = 0;
        // Synthetic: every ii–V–I–vi cell is one Lift slot.
        for (name, ctxs) in sequences().into_iter().take(12) {
            let mut steps = plain_steps(&ctxs, VoiceRange::pad(0.35), 0.35);
            for (i, s) in steps.iter_mut().enumerate() {
                s.gesture = Some(HarmonicGesture::Lift);
                s.slot = Some(i as u32 / 4);
            }
            let (f, c) = lift_falls(&steps, &voice_path(&steps, &w));
            assert_eq!(f, 0, "{name}: a Lift centroid fell {f}/{c} times");
            checked += c;
            falls_off += lift_falls(&steps, &voice_path(&steps, &off)).0;
        }
        // The flagships: pad and keys paths over the real backbone.
        for world in MusicWorld::all() {
            let c = flagship(&world);
            let lead: Vec<Note> = c
                .score
                .notes
                .iter()
                .filter(|n| n.role == Role::Lead)
                .copied()
                .collect();
            let n = if c.perf.language.shell_voicings { 3 } else { 4 };
            let (_, pad) = pad_steps(&c.perf, world.voicing_spread);
            let (_, keys) = keys_steps(&c.perf, world.voicing_spread, &lead, n);
            for (role, steps) in [("pad", &pad), ("keys", &keys)] {
                let (f, cnt) = lift_falls(steps, &voice_path(steps, &w));
                assert_eq!(
                    f, 0,
                    "{} {role}: a Lift centroid fell {f}/{cnt}",
                    world.name
                );
                checked += cnt;
                falls_off += lift_falls(steps, &voice_path(steps, &off)).0;
            }
        }
        eprintln!("lift pairs checked={checked}; falls with gesture weight 0: {falls_off}");
        assert!(checked > 0, "no Lift pairs checked");
        assert!(
            falls_off > 0,
            "with the gesture term off no Lift ever falls — the term is not what holds it"
        );
    }

    #[test]
    fn flagship_voicings_keep_the_score_honest() {
        use super::super::comp::release_at_harmony_change;
        use super::super::diagnostics::RealizationDiagnostics;
        use super::super::score::Provenance;
        use super::super::witness::audit;
        for world in MusicWorld::all() {
            let c = flagship(&world);
            let r = RealizationDiagnostics::measure(&c.plan, &c.score);
            for (role, count) in &r.unjustified_by_role {
                if matches!(role, Role::Pad | Role::Keys) {
                    assert_eq!(*count, 0, "{}: unjustified {:?} notes", world.name, role);
                }
            }
            assert_eq!(r.cross_boundary_dissonances, 0, "{}", world.name);
            let wr = audit(&c.perf, &c.score);
            eprintln!(
                "{}: witnessed {}/{}\n{}",
                world.name,
                wr.witnessed(),
                wr.total(),
                VoicingDiagnostics::measure(&c.score, &c.perf.contexts).report()
            );
            assert_eq!(
                wr.witnessed(),
                wr.total(),
                "{}\n{}",
                world.name,
                wr.report()
            );

            // The keys' held voicings still sustain ≥ 1.4 beats wherever the legacy ones did
            // (reconstructed: the legacy leader's voicing, released at the next harmony change).
            let n = if c.perf.language.shell_voicings { 3 } else { 4 };
            let mut checked = 0;
            for eb in c
                .perf
                .ensemble
                .iter()
                .filter(|e| e.keys == KeysMode::Sustain)
            {
                let at = eb.bar as f64 * BEATS_PER_BAR;
                let Some(ctx) = c.perf.context_at(at) else {
                    continue;
                };
                let held: Vec<f32> = c
                    .score
                    .notes
                    .iter()
                    .filter(|n| {
                        n.role == Role::Keys
                            && n.prov.role_note == "hold"
                            && (n.start_beat - at).abs() < 1e-6
                    })
                    .map(|n| n.dur_beats)
                    .collect();
                if held.is_empty() {
                    continue; // the arrangement silenced the keys in this phrase
                }
                let v = VoiceLeader::new(58, 84, world.voicing_spread).lead(&ctx.chord, n, 72);
                let mut legacy: Vec<Note> = v
                    .voices
                    .iter()
                    .map(|&p| {
                        Note::new(
                            at,
                            3.9,
                            p,
                            0.3,
                            Role::Keys,
                            Provenance::new(super::super::form::SectionKind::A),
                        )
                    })
                    .collect();
                release_at_harmony_change(&mut legacy, &c.perf.chords);
                let legacy_long = legacy.iter().any(|n| n.dur_beats >= 1.4);
                let now_long = held.iter().any(|&d| d >= 1.4);
                assert!(
                    !legacy_long || now_long,
                    "{} bar {}: the hold no longer sustains ({held:?})",
                    world.name,
                    eb.bar
                );
                checked += 1;
            }
            eprintln!("{}: keys holds checked={checked}", world.name);
        }
    }
}
