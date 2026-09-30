//! **Harmonic edits and tonal regions** — the actions that change the harmony itself
//! (Round VII; split out of [`super::performance`] in Round VIIb, real regions in Round VIIb).
//!
//! Through Round VIIa the harmonic verbs were misnamed: `Modulate` inserted one applied dominant
//! and the piece stayed in ONE region (`HarmonicContext::region` was written and never read), and
//! `Reharmonize` only recoloured a chord over its own root. Each verb now does what it says:
//!
//! - **Tonicize** — a brief applied dominant into a non-tonic chord. The region is unchanged
//!   ([`EditKind::AppliedDominant`]).
//! - **Modulate** — a true region change, planned as a [`RegionTimeline`] span (see
//!   [`modulation_target`] and the return rule on [`HarmonicFrame::planned_return`]): entered
//!   through a pivot chord diatonic to both regions and the new key's V7 into its tonic, the SAME
//!   functional path transposed into the new region (every cost term of the backbone search is
//!   transposition invariant, so the transposed path is the path the search would pick there), and
//!   left through a return pivot (home V7 or a chord common to both). A span that does not
//!   establish the new key (its dominant and then its tonic on a downbeat) is refused and the
//!   action is relabelled `Tonicize`, with the reason recorded ([`Relabel`]).
//! - **Reharmonize** — a real substitution: a dominant resolving to its expected target becomes its
//!   tritone substitute ([`tritone_sub`]); otherwise a diatonic third substitute with the same
//!   contextual function ([`third_sub`]). When neither is lawful the action falls back to a
//!   same-root colour change and is LABELLED `Recolor`.
//! - **Recolor** — the same-root colour change ([`recolor`]).

use super::action::{ActionCause, ActionKind, ActionPlan, EffectVector, MusicalAction};
use super::backbone::HarmonicGesture;
use super::context::{common_tones, contextual_function, expected_target, pc_motion, PullEvidence};
use super::form::BEATS_PER_BAR;
use super::harmony::{diatonic_chord, ChordSpan};
use super::ids::ActionId;
use super::intent::IntentMorphism;
use super::plan::CompositionPlan;
use super::theory::{Chord, Function, Mode, Quality, Scale};
use super::timeline::IntentTimeline;

const EPS: f64 = 1e-6;

/// What a [`HarmonicEdit`] did to the chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    /// Same root, new colour (a Recolor).
    Recolor,
    /// A dominant replaced by its tritone substitute (a Reharmonize).
    TritoneSub,
    /// A diatonic third substitute with the same contextual function (a Reharmonize).
    ThirdSub,
    /// An applied dominant inserted before a non-tonic chord (a Tonicize).
    AppliedDominant,
    /// The way into a modulation: the pivot chord, the new key's dominant, its tonic.
    PivotIn,
    /// The way back home: the return pivot (home V7 or a common chord).
    PivotOut,
}

impl EditKind {
    /// A short lowercase label for dumps.
    pub fn label(self) -> &'static str {
        match self {
            EditKind::Recolor => "recolor",
            EditKind::TritoneSub => "tritone-sub",
            EditKind::ThirdSub => "third-sub",
            EditKind::AppliedDominant => "applied-dominant",
            EditKind::PivotIn => "pivot-in",
            EditKind::PivotOut => "pivot-out",
        }
    }
}

/// A harmonic change applied by an action.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HarmonicEdit {
    pub action: ActionId,
    pub at_beat: f64,
    pub before: Chord,
    pub after: Chord,
    pub kind: EditKind,
}

/// How a region span came about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionKind {
    /// The piece's home region.
    Home,
    /// A modulated region, left from `from`.
    Modulated { from: Scale },
    /// The pivot into a modulation: a chord diatonic to both `from` and `to`. It is still heard in
    /// `from` (the span's scale); the region changes when the new key's dominant sounds.
    Pivot { from: Scale, to: Scale },
    /// The return pivot back to `to` (home): a home V7 or a chord common to both regions.
    Return { to: Scale },
}

impl RegionKind {
    /// A short lowercase label for dumps.
    pub fn label(self) -> &'static str {
        match self {
            RegionKind::Home => "home",
            RegionKind::Modulated { .. } => "modulated",
            RegionKind::Pivot { .. } => "pivot",
            RegionKind::Return { .. } => "return",
        }
    }
}

/// One span of the tonal-region timeline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionSpan {
    pub start_beat: f64,
    pub end_beat: f64,
    /// The region in force (tonic + mode).
    pub scale: Scale,
    pub kind: RegionKind,
    /// The action that caused it (`None` for home).
    pub cause: Option<ActionId>,
    /// Whether the span established its region (home always does; a planned modulation only
    /// commits when it sounds its dominant and then its tonic on a downbeat).
    pub established: bool,
}

/// A harmonic verb renamed to what it actually did, with the reason.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Relabel {
    pub action: ActionId,
    pub from: ActionKind,
    pub to: ActionKind,
    pub reason: &'static str,
}

/// The tonal-region timeline: contiguous spans tiling the piece, plus the harmonic verbs that were
/// relabelled on the way.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionTimeline {
    /// The home region (the world's tonic + mode).
    pub home: Scale,
    /// Contiguous spans in time order.
    pub spans: Vec<RegionSpan>,
    pub relabels: Vec<Relabel>,
}

impl RegionTimeline {
    /// One home span over `[0, total_beats)`.
    pub fn home(scale: Scale, total_beats: f64) -> RegionTimeline {
        RegionTimeline {
            home: scale,
            spans: vec![RegionSpan {
                start_beat: 0.0,
                end_beat: total_beats,
                scale,
                kind: RegionKind::Home,
                cause: None,
                established: true,
            }],
            relabels: Vec::new(),
        }
    }

    /// The span in force at `beat` (the latest one starting at or before it).
    pub fn span_at(&self, beat: f64) -> Option<&RegionSpan> {
        self.spans.iter().rev().find(|s| s.start_beat <= beat + EPS)
    }

    /// The region in force at `beat` (home before the first span).
    pub fn region_at(&self, beat: f64) -> Scale {
        self.span_at(beat).map(|s| s.scale).unwrap_or(self.home)
    }

    /// TRUE region changes: consecutive spans whose region differs (a modulation and its return
    /// are two; a pivot is heard in the old region, so it is not one).
    pub fn transitions(&self) -> usize {
        self.spans
            .windows(2)
            .filter(|w| w[0].scale != w[1].scale)
            .count()
    }

    /// The modulated spans.
    pub fn modulations(&self) -> impl Iterator<Item = &RegionSpan> {
        self.spans
            .iter()
            .filter(|s| matches!(s.kind, RegionKind::Modulated { .. }))
    }

    /// The return spans.
    pub fn returns(&self) -> impl Iterator<Item = &RegionSpan> {
        self.spans
            .iter()
            .filter(|s| matches!(s.kind, RegionKind::Return { .. }))
    }

    /// Replace the part of the home span holding `parts` (contiguous, in order) with them.
    fn splice(&mut self, parts: &[RegionSpan]) {
        let (Some(first), Some(last)) = (parts.first(), parts.last()) else {
            return;
        };
        let Some(ix) = self.spans.iter().position(|s| {
            s.start_beat <= first.start_beat + EPS && s.end_beat >= last.end_beat - EPS
        }) else {
            return;
        };
        let host = self.spans[ix];
        let mut repl = Vec::with_capacity(parts.len() + 2);
        if first.start_beat > host.start_beat + EPS {
            repl.push(RegionSpan {
                end_beat: first.start_beat,
                ..host
            });
        }
        repl.extend(
            parts
                .iter()
                .copied()
                .filter(|p| p.end_beat > p.start_beat + EPS),
        );
        if last.end_beat < host.end_beat - EPS {
            repl.push(RegionSpan {
                start_beat: last.end_beat,
                ..host
            });
        }
        self.spans.splice(ix..=ix, repl);
    }

    /// A compact one-line dump of the spans.
    pub fn dump(&self) -> String {
        dump_spans(&self.spans)
    }
}

/// A compact one-line dump of region spans: `start-end region kind (cause)` joined by ` | `.
pub fn dump_spans(spans: &[RegionSpan]) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    for r in spans {
        let _ = write!(
            s,
            "{:.0}-{:.0} {} {}",
            r.start_beat,
            r.end_beat,
            scale_label(&r.scale),
            r.kind.label()
        );
        if let RegionKind::Pivot { to, .. } = r.kind {
            let _ = write!(s, "->{}", scale_label(&to));
        }
        if let Some(c) = r.cause {
            let _ = write!(
                s,
                " ({c}{})",
                if r.established { "" } else { ", unestablished" }
            );
        }
        s.push_str(" | ");
    }
    s.trim_end_matches(" | ").to_string()
}

/// A readable region name, e.g. `A-aeolian`.
pub fn scale_label(s: &Scale) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let mode = match s.mode {
        Mode::Ionian => "ionian",
        Mode::Dorian => "dorian",
        Mode::Phrygian => "phrygian",
        Mode::Lydian => "lydian",
        Mode::Mixolydian => "mixolydian",
        Mode::Aeolian => "aeolian",
        Mode::Locrian => "locrian",
        Mode::HarmonicMinor => "harmonic-minor",
    };
    format!("{}-{mode}", NAMES[s.tonic_pc.rem_euclid(12) as usize])
}

/// The pitch collection of a region.
fn collection(s: &Scale) -> Vec<i32> {
    (0..12).filter(|&p| s.contains_pc(p)).collect()
}

/// Whether every tone of `c` belongs to `s`.
fn diatonic_to(c: &Chord, s: &Scale) -> bool {
    c.pitch_classes().iter().all(|&p| s.contains_pc(p))
}

/// Whether `c` carries a seventh.
fn has_seventh(c: &Chord) -> bool {
    c.quality.intervals().iter().any(|&i| i == 10 || i == 11)
}

/// The 0-based degree of `root` in `s`, or `-1` when it is not a scale tone.
fn degree_in(s: &Scale, root: i32) -> i32 {
    (0..7)
        .find(|&d| s.degree_pitch(d, 4).rem_euclid(12) == root.rem_euclid(12))
        .unwrap_or(-1)
}

/// **The modulation rule.** A Modulate goes to a closely related region whose pitch collection
/// really differs from home: a WHOLE-STEP LIFT (same mode, tonic +2: two pitch classes change —
/// the idiomatic fusion/pop lift) when the modulating event raises the pressure, else the DOMINANT
/// KEY (same mode, tonic +7: one pitch class changes). Never the relative key — that is the same
/// seven pitch classes relabelled, not a modulation ([`lawful_modulation`] refuses it).
pub fn modulation_target(home: &Scale, rising: bool) -> Scale {
    Scale::new(home.tonic_pc + if rising { 2 } else { 7 }, home.mode)
}

/// Whether moving from `from` to `to` is a lawful modulation: at least one pitch class changes (the
/// relative key does not count) and at most two do (closely related).
pub fn lawful_modulation(from: &Scale, to: &Scale) -> Result<(), &'static str> {
    let a = collection(from);
    let differ = collection(to).iter().filter(|p| !a.contains(p)).count();
    if differ == 0 {
        Err("the target has home's own pitch collection (a relative-key relabel), not a new region")
    } else if differ > 2 {
        Err("the target is not closely related: more than two pitch classes differ from home")
    } else {
        Ok(())
    }
}

/// The chord diatonic to BOTH regions nearest `near` (pitch-class motion, plus the motion on into
/// `into` when given), never on `to`'s tonic (a pivot must not pre-empt the arrival) nor on
/// `into`'s root. Sevenths when `near` has one, else triads (either family as a fallback).
fn common_chord(from: &Scale, to: &Scale, near: &Chord, into: Option<&Chord>) -> Option<Chord> {
    let pick = |seventh: bool| {
        (0..7)
            .map(|d| diatonic_chord(from, d, seventh))
            .filter(|c| diatonic_to(c, to) && c.root_pc != to.tonic_pc)
            .filter(|c| into.is_none_or(|n| c.root_pc != n.root_pc))
            .min_by_key(|c| pc_motion(near, c) + into.map(|n| pc_motion(c, n)).unwrap_or(0))
    };
    let s7 = has_seventh(near);
    pick(s7).or_else(|| pick(!s7))
}

/// The tritone substitute of `chord` when it is a dominant resolving to its expected target
/// (`next`): the dominant seventh a tritone away, which keeps the guide tritone (≥ 2 common tones)
/// and still pulls concretely onto `next` (a semitone above it). `None` otherwise — including when
/// the substitute would share fewer than two tones (a bare triad's "substitute" shares one).
pub fn tritone_sub(chord: &Chord, next: Option<&Chord>, region: &Scale) -> Option<Chord> {
    let n = next?;
    let resolves = PullEvidence::of(chord, n.root_pc).is_dominant()
        && expected_target(chord, region) == Some(n.root_pc);
    if !resolves {
        return None;
    }
    let sub = Chord::new(chord.root_pc + 6, Quality::Dom7);
    (common_tones(chord, &sub) >= 2 && PullEvidence::of(&sub, n.root_pc).is_dominant())
        .then_some(sub)
}

/// A diatonic third substitute for `chord` in `region`: the diatonic chord a third above or below
/// sharing ≥ 2 tones, with the same contextual function and the same expectation (it must not
/// create or cancel a pull), not on either neighbour's root (a substitute still moves), never the
/// tonic, and never replacing the tonic.
pub fn third_sub(
    chord: &Chord,
    prev: Option<&Chord>,
    next: Option<&Chord>,
    region: &Scale,
) -> Option<Chord> {
    if chord.root_pc == region.tonic_pc {
        return None;
    }
    let d = degree_in(region, chord.root_pc);
    if d < 0 {
        return None;
    }
    let func = contextual_function(chord, region);
    let expects = expected_target(chord, region);
    [d - 2, d + 2]
        .into_iter()
        .map(|dd| diatonic_chord(region, dd.rem_euclid(7), has_seventh(chord)))
        .filter(|c| {
            c.root_pc != region.tonic_pc
                && common_tones(chord, c) >= 2
                && contextual_function(c, region) == func
                && expected_target(c, region) == expects
                && prev.is_none_or(|p| p.root_pc != c.root_pc)
                && next.is_none_or(|n| n.root_pc != c.root_pc)
        })
        .max_by_key(|c| common_tones(chord, c))
}

/// The same-root colour change (Round VII's "reharmonize", now honestly named).
pub fn recolor(chord: &Chord) -> Option<Chord> {
    let q = match chord.quality {
        Quality::Maj | Quality::Maj6 => Quality::Maj7,
        Quality::Maj7 | Quality::Add9 => Quality::Maj9,
        Quality::Maj9 => Quality::Maj6,
        Quality::Min | Quality::Min6 => Quality::Min7,
        Quality::Min7 => Quality::Min9,
        Quality::Min9 => Quality::Min6,
        Quality::Dom7 => Quality::Dom9,
        Quality::Dom9 => Quality::Dom7,
        other => other,
    };
    (q != chord.quality).then(|| Chord::new(chord.root_pc, q))
}

/// The plan-level facts the harmonic verbs need: the phrase grid, the backbone's gesture slots and
/// pointers, and how the running intent moved at each semantic transition.
#[derive(Debug, Clone, PartialEq)]
pub struct HarmonicFrame {
    pub home: Scale,
    pub total_beats: f64,
    /// Phrase `(start, end)` beats, in order.
    pub phrases: Vec<(f64, f64)>,
    /// Every backbone gesture slot's `(start beat, gesture)`.
    pub slots: Vec<(f64, HarmonicGesture)>,
    /// The end beat of every Lift slot (the chord ending there is the pointer).
    pub lift_ends: Vec<f64>,
    /// Per intent transition: `(next.energy + next.tension) - (prev.energy + prev.tension)`.
    pub pressure_rise: Vec<f32>,
    /// The harmonic-vocabulary source law every edit must stay inside (`None`: archived edits).
    pub vocabulary: Option<super::vocabulary::HarmonicVocabulary>,
}

impl HarmonicFrame {
    /// A frame without plan facts (the phrase engine, unit tests).
    pub fn bare(home: Scale, total_beats: f64) -> HarmonicFrame {
        HarmonicFrame {
            home,
            total_beats,
            phrases: Vec::new(),
            slots: Vec::new(),
            lift_ends: Vec::new(),
            pressure_rise: Vec::new(),
            vocabulary: None,
        }
    }

    /// This frame with every harmonic edit held inside `vocabulary`: an edit whose chord the
    /// room cannot sound is not a lawful edit (the verb is refused or recast, never leaked).
    pub fn with_vocabulary(
        mut self,
        vocabulary: Option<super::vocabulary::HarmonicVocabulary>,
    ) -> HarmonicFrame {
        self.vocabulary = vocabulary;
        self
    }

    /// Whether the vocabulary law (if any) admits `chord`.
    fn lawful(&self, chord: &Chord) -> bool {
        self.vocabulary.is_none_or(|v| v.admits(*chord))
    }

    /// The frame of `plan` walked from `timeline`.
    pub fn from_plan(
        plan: &CompositionPlan,
        timeline: &IntentTimeline,
        home: Scale,
        total_beats: f64,
    ) -> HarmonicFrame {
        let (mut slots, mut lift_ends) = (Vec::new(), Vec::new());
        if let Some(bb) = &plan.backbone {
            for s in &bb.slots {
                slots.push((s.start_beat(), s.gesture));
                if s.gesture == HarmonicGesture::Lift {
                    lift_ends.push(s.end_beat());
                }
            }
        }
        HarmonicFrame {
            home,
            total_beats,
            phrases: plan
                .form
                .phrases
                .iter()
                .map(|p| (p.start_beat(), p.end_beat()))
                .collect(),
            slots,
            lift_ends,
            pressure_rise: timeline
                .transitions
                .iter()
                .map(|t| (t.next.energy + t.next.tension) - (t.prev.energy + t.prev.tension))
                .collect(),
            vocabulary: None,
        }
    }

    /// Whether the modulating action raises the pressure: its semantic effect vector when the
    /// planner sized it, else the running intent's energy + tension across its transition.
    fn rising(&self, a: &MusicalAction) -> bool {
        if a.effect != EffectVector::NEUTRAL {
            return a.effect.d_pressure > 1e-6;
        }
        match a.cause {
            ActionCause::Morphism { transition, .. } => self
                .pressure_rise
                .get(transition)
                .is_some_and(|&d| d > 1e-6),
            _ => false,
        }
    }

    /// Whether a backbone gesture slot starts at `beat` (its downbeat chord is the chart's anchor).
    fn is_anchor_start(&self, beat: f64) -> bool {
        self.slots.iter().any(|&(b, _)| (b - beat).abs() < EPS)
    }

    /// Whether a span ending at `end` closes a Lift (its harmony is the chart's pointer).
    fn closes_lift(&self, end: f64) -> bool {
        self.lift_ends.iter().any(|&e| (e - end).abs() < EPS)
    }

    /// Whether `c` is a backbone gesture anchor (the chord on a slot's downbeat — including every
    /// Deflect slot start) or a pointer (the chord that ends a Lift). These are the song's chart
    /// landmarks (Round IX: `SongMap::landmarks`): no harmonic action may replace their root.
    fn protected(&self, c: &ChordSpan) -> bool {
        self.is_anchor_start(c.start_beat) || self.closes_lift(c.start_beat + c.dur_beats as f64)
    }

    /// **The return rule.** A modulated region persists from its pivot until the FIRST of: the next
    /// resolution or cadence toward home after it (`cadences`), the next backbone Reset slot start,
    /// or the end of the phrase AFTER the one it starts in (a bounded span of at most two phrases)
    /// — or the end of the piece.
    pub fn planned_return(&self, start: f64, cadences: &[f64]) -> f64 {
        let mut ret = self.total_beats;
        let mut take = |b: f64| {
            if b > start + EPS && b < ret {
                ret = b;
            }
        };
        cadences.iter().copied().for_each(&mut take);
        self.slots
            .iter()
            .filter(|&&(_, g)| g == HarmonicGesture::Reset)
            .for_each(|&(b, _)| take(b));
        if let Some(p) = self
            .phrases
            .iter()
            .position(|&(s, e)| s <= start + EPS && start < e - EPS)
        {
            take(self.phrases.get(p + 1).unwrap_or(&self.phrases[p]).1);
        }
        ret
    }
}

/// A span with `chord` over `[start, start + dur)` in `region`, keeping `like`'s provenance note.
fn span(like: &ChordSpan, start: f64, dur: f32, chord: Chord, region: &Scale) -> ChordSpan {
    ChordSpan {
        start_beat: start,
        dur_beats: dur,
        chord,
        function: contextual_function(&chord, region),
        degree: degree_in(region, chord.root_pc),
        note: like.note,
    }
}

/// A planned modulation, not yet committed.
struct Modulation {
    chords: Vec<ChordSpan>,
    spans: Vec<RegionSpan>,
    edits: Vec<HarmonicEdit>,
    locked: Vec<(f64, f64)>,
}

fn overlaps(locked: &[(f64, f64)], c: &ChordSpan) -> bool {
    let end = c.start_beat + c.dur_beats as f64;
    locked
        .iter()
        .any(|&(s, e)| c.start_beat < e - EPS && end > s + EPS)
}

/// Plan the modulation of action `a` (to `to`, or by [`modulation_target`]). `Err` carries the
/// reason it is refused (the caller relabels the action `Tonicize`).
fn plan_modulation(
    chords: &[ChordSpan],
    a: &MusicalAction,
    frame: &HarmonicFrame,
    regions: &RegionTimeline,
    locked: &[(f64, f64)],
    cadences: &[f64],
    to: Option<Scale>,
) -> Result<Modulation, &'static str> {
    let host = regions
        .span_at(a.start_beat)
        .copied()
        .filter(|s| s.kind == RegionKind::Home)
        .ok_or("already away from home: a modulation inside a modulated span is not planned")?;
    let from = host.scale;
    let to = to.unwrap_or_else(|| modulation_target(&from, frame.rising(a)));
    lawful_modulation(&from, &to)?;

    // The span opens on the first harmony at or after the action.
    let k = chords
        .iter()
        .position(|c| c.start_beat >= a.start_beat - EPS)
        .ok_or("no harmony starts at or after the action")?;
    if k == 0 {
        return Err("the action lands on the first chord: there is no harmony to pivot from");
    }
    let s = chords[k].start_beat;
    let p = chords[k - 1];
    if p.dur_beats < 2.0 - 1e-6 || overlaps(locked, &p) {
        return Err("no room before the span for a pivot and the new key's dominant");
    }
    // The planned return, snapped to the latest chord start at or before it.
    let ret = frame.planned_return(s, cadences).min(frame.total_beats);
    let r = if ret >= frame.total_beats - EPS {
        chords.len()
    } else {
        chords
            .iter()
            .rposition(|c| c.start_beat <= ret + EPS)
            .filter(|&i| i > k)
            .ok_or("the planned return comes before the new region can sound")?
    };
    let r_beat = chords
        .get(r)
        .map(|c| c.start_beat)
        .unwrap_or(frame.total_beats);
    if host.start_beat > p.start_beat + EPS || host.end_beat < r_beat - EPS {
        return Err("the span would overlap another region change");
    }

    let mut out: Vec<ChordSpan> = chords[..k - 1].to_vec();
    let mut edits = Vec::new();
    let mut locked_new = Vec::new();
    let edit = |at: f64, before: Chord, after: Chord, kind: EditKind| HarmonicEdit {
        action: a.id,
        at_beat: at,
        before,
        after,
        kind,
    };

    // 1. The pivot (diatonic to both) and the new key's V7, sharing the harmony before the span.
    let half = (p.dur_beats / 2.0).max(1.0);
    let mid = p.start_beat + (p.dur_beats - half) as f64;
    let pivot = if diatonic_to(&p.chord, &from) && diatonic_to(&p.chord, &to) {
        p.chord
    } else {
        common_chord(&from, &to, &p.chord, None).ok_or("no chord is diatonic to both regions")?
    };
    if pivot != p.chord && frame.is_anchor_start(p.start_beat) {
        return Err("the pivot would replace a chart anchor");
    }
    out.push(span(&p, p.start_beat, p.dur_beats - half, pivot, &from));
    if pivot != p.chord {
        edits.push(edit(p.start_beat, p.chord, pivot, EditKind::PivotIn));
    }
    let dom = Chord::new(to.tonic_pc + 7, Quality::Dom7);
    out.push(ChordSpan {
        function: Function::Dominant,
        degree: -1,
        ..span(&p, mid, half, dom, &to)
    });
    edits.push(edit(mid, p.chord, dom, EditKind::PivotIn));
    locked_new.push((p.start_beat, s));

    // 2. The same functional path, transposed into the new region. The dominant resolves into the
    //    tonic: when the path's first chord is not the new tonic, its first half becomes it —
    //    except on a chart anchor (a slot's downbeat): the song's journey, transposed, stays the
    //    song's (a Deflect's planned miss stays a miss, a Lift stays a reach).
    let iv = (to.tonic_pc - from.tonic_pc).rem_euclid(12);
    for (j, c) in chords[k..r].iter().enumerate() {
        let t = Chord::new(c.chord.root_pc + iv, c.chord.quality);
        let moved = ChordSpan { chord: t, ..*c };
        if j == 0 && t.root_pc != to.tonic_pc && !frame.is_anchor_start(c.start_beat) {
            let tonic = diatonic_chord(&to, 0, has_seventh(&t));
            let head = if c.dur_beats >= 2.0 - 1e-6 {
                (c.dur_beats / 2.0).max(1.0)
            } else {
                c.dur_beats
            };
            out.push(span(c, c.start_beat, head, tonic, &to));
            if head < c.dur_beats - 1e-6 {
                out.push(ChordSpan {
                    start_beat: c.start_beat + head as f64,
                    dur_beats: c.dur_beats - head,
                    ..moved
                });
            }
            edits.push(edit(c.start_beat, c.chord, tonic, EditKind::PivotIn));
            locked_new.push((c.start_beat, c.start_beat + head as f64));
        } else {
            out.push(moved);
        }
    }

    // 3. The return pivot: home V7 into a home-tonic arrival, else the last chord itself when it
    //    is already common to both regions, else the common chord nearest it and the arrival.
    let ret_start = if let Some(arrival) = chords.get(r).map(|c| c.chord) {
        let q = out.len() - 1;
        let last = out[q];
        if overlaps(&locked_new, &last) && last.dur_beats < 2.0 - 1e-6 {
            return Err("the span is too short to leave through a return pivot");
        }
        let replace = if arrival.root_pc == from.tonic_pc {
            Some(Chord::new(
                diatonic_chord(&from, 4, false).root_pc,
                Quality::Dom7,
            ))
        } else if diatonic_to(&last.chord, &from) && diatonic_to(&last.chord, &to) {
            None
        } else {
            Some(
                common_chord(&from, &to, &last.chord, Some(&arrival))
                    .ok_or("no chord is diatonic to both regions")?,
            )
        };
        match replace {
            None => last.start_beat,
            Some(back) => {
                let tail = if last.dur_beats >= 2.0 - 1e-6 {
                    (last.dur_beats / 2.0).max(1.0)
                } else {
                    last.dur_beats
                };
                let at = last.start_beat + (last.dur_beats - tail) as f64;
                if frame.is_anchor_start(at) {
                    return Err("the return pivot would replace a chart anchor");
                }
                if overlaps(
                    &locked_new,
                    &ChordSpan {
                        start_beat: at,
                        dur_beats: tail,
                        ..last
                    },
                ) {
                    return Err("the span is too short to leave through a return pivot");
                }
                out.pop();
                if tail < last.dur_beats - 1e-6 {
                    out.push(ChordSpan {
                        dur_beats: last.dur_beats - tail,
                        ..last
                    });
                }
                let before = chords
                    .iter()
                    .rev()
                    .find(|c| c.start_beat <= at + EPS)
                    .map(|c| c.chord)
                    .unwrap_or(last.chord);
                out.push(span(&last, at, tail, back, &from));
                edits.push(edit(at, before, back, EditKind::PivotOut));
                locked_new.push((at, r_beat));
                at
            }
        }
    } else {
        frame.total_beats
    };
    out.extend_from_slice(&chords[r..]);

    // 4. Established: the new region sounds its dominant and then its tonic on a downbeat, and the
    //    span lasts at least a bar past its arrival.
    let inside: Vec<&ChordSpan> = out
        .iter()
        .filter(|c| c.start_beat >= mid - EPS && c.start_beat < ret_start - EPS)
        .collect();
    let dom_at = inside
        .iter()
        .find(|c| PullEvidence::of(&c.chord, to.tonic_pc).is_dominant())
        .map(|c| c.start_beat);
    let tonic_after = dom_at.is_some_and(|d| {
        inside.iter().any(|c| {
            c.chord.root_pc == to.tonic_pc
                && c.start_beat > d + EPS
                && (c.start_beat / BEATS_PER_BAR).fract().abs() < EPS
        })
    });
    if !tonic_after {
        return Err(
            "the new region never sounds its dominant and then its tonic on a downbeat (a deceptive \
             entry tonicizes; it does not modulate)",
        );
    }
    if ret_start - s < BEATS_PER_BAR - EPS {
        return Err("the span is shorter than a bar before its planned return");
    }

    let mut spans = vec![
        RegionSpan {
            start_beat: p.start_beat,
            end_beat: mid,
            scale: from,
            kind: RegionKind::Pivot { from, to },
            cause: Some(a.id),
            established: true,
        },
        RegionSpan {
            start_beat: mid,
            end_beat: ret_start,
            scale: to,
            kind: RegionKind::Modulated { from },
            cause: Some(a.id),
            established: true,
        },
    ];
    if ret_start < frame.total_beats - EPS {
        spans.push(RegionSpan {
            start_beat: ret_start,
            end_beat: r_beat,
            scale: from,
            kind: RegionKind::Return { to: from },
            cause: Some(a.id),
            established: true,
        });
    }
    Ok(Modulation {
        chords: out,
        spans,
        edits,
        locked: locked_new,
    })
}

/// Insert an applied dominant before the non-tonic harmony arriving at (or after) `a`: the second
/// half of the preceding span becomes V7/x. The region is unchanged. Never the second half of a
/// chart pointer: the harmony closing a Lift is the song's expectation, not a tonicization's.
fn tonicize(
    chords: &mut Vec<ChordSpan>,
    a: &MusicalAction,
    frame: &HarmonicFrame,
    regions: &RegionTimeline,
    locked: &[(f64, f64)],
) -> Option<HarmonicEdit> {
    let ix = chords
        .iter()
        .position(|c| c.start_beat >= a.start_beat - EPS)?;
    if ix == 0 {
        return None;
    }
    let target = chords[ix].chord;
    if target.root_pc == regions.region_at(chords[ix].start_beat).tonic_pc {
        return None; // a dominant to the local tonic is an arrival, not a tonicization
    }
    let prev = chords[ix - 1];
    if prev.dur_beats < 2.0 - 1e-6
        || overlaps(locked, &prev)
        || frame.closes_lift(prev.start_beat + prev.dur_beats as f64)
    {
        return None;
    }
    let half = (prev.dur_beats / 2.0).max(1.0);
    let at = prev.start_beat + (prev.dur_beats - half) as f64;
    let dom = Chord::new((target.root_pc + 7).rem_euclid(12), Quality::Dom7);
    // Under the vocabulary law the applied dominant is its admitted retraction, and only while it
    // still pulls onto the target (a room without the seventh keeps the leading tone).
    let dom = match frame.vocabulary {
        None => dom,
        Some(v) => v
            .conform(dom)
            .filter(|d| PullEvidence::of(d, target.root_pc).is_dominant())?,
    };
    chords[ix - 1].dur_beats = prev.dur_beats - half;
    chords.insert(
        ix,
        ChordSpan {
            start_beat: at,
            dur_beats: half,
            chord: dom,
            function: Function::Dominant,
            degree: -1,
            note: prev.note,
        },
    );
    Some(HarmonicEdit {
        action: a.id,
        at_beat: at,
        before: prev.chord,
        after: dom,
        kind: EditKind::AppliedDominant,
    })
}

/// Substitute (or, failing that, recolour) the harmony a Reharmonize/Recolor action lands on: the
/// chord sounding at its start, then any chord starting inside its window. Returns the edit and
/// the kind the action really performed.
fn reharmonize(
    chords: &mut [ChordSpan],
    a: &MusicalAction,
    frame: &HarmonicFrame,
    regions: &RegionTimeline,
    locked: &[(f64, f64)],
) -> Option<(HarmonicEdit, ActionKind)> {
    let sounding = chords
        .iter()
        .rposition(|c| c.start_beat <= a.start_beat + EPS);
    let cands: Vec<usize> = sounding
        .into_iter()
        .chain((0..chords.len()).filter(|&i| {
            Some(i) != sounding
                && chords[i].start_beat > a.start_beat + EPS
                && chords[i].start_beat < a.end_beat() - EPS
        }))
        .filter(|&i| !overlaps(locked, &chords[i]))
        .collect();
    let at = |ix: usize| regions.region_at(chords[ix].start_beat);
    let neighbours = |ix: usize| {
        (
            ix.checked_sub(1).map(|j| chords[j].chord),
            chords.get(ix + 1).map(|c| c.chord),
        )
    };
    let mut choice: Option<(usize, Chord, EditKind, ActionKind)> = None;
    if a.kind == ActionKind::Reharmonize {
        choice = cands
            .iter()
            .filter(|&&ix| !frame.protected(&chords[ix]))
            .find_map(|&ix| {
                let (_, next) = neighbours(ix);
                tritone_sub(&chords[ix].chord, next.as_ref(), &at(ix))
                    .filter(|s| frame.lawful(s))
                    .map(|s| (ix, s, EditKind::TritoneSub, ActionKind::Reharmonize))
            });
        choice = choice.or_else(|| {
            cands
                .iter()
                .filter(|&&ix| !frame.protected(&chords[ix]))
                .find_map(|&ix| {
                    let (prev, next) = neighbours(ix);
                    third_sub(&chords[ix].chord, prev.as_ref(), next.as_ref(), &at(ix))
                        .filter(|s| frame.lawful(s))
                        .map(|s| (ix, s, EditKind::ThirdSub, ActionKind::Reharmonize))
                })
        });
    }
    let choice = choice.or_else(|| {
        cands.iter().find_map(|&ix| {
            recolor(&chords[ix].chord)
                .filter(|c| frame.lawful(c))
                .map(|c| (ix, c, EditKind::Recolor, ActionKind::Recolor))
        })
    });
    let (ix, after, kind, verb) = choice?;
    let region = at(ix);
    let before = chords[ix].chord;
    chords[ix].chord = after;
    chords[ix].function = contextual_function(&after, &region);
    Some((
        HarmonicEdit {
            action: a.id,
            at_beat: chords[ix].start_beat,
            before,
            after,
            kind,
        },
        verb,
    ))
}

/// Apply every harmonic action to `chords` and plan the tonal regions: Modulates first (in time
/// order; a refused one is relabelled `Tonicize`), then Tonicize / Reharmonize / Recolor in time
/// order, each in the region in force where it lands. Returns the edits and the region timeline.
pub(super) fn apply_harmonic_actions(
    chords: &mut Vec<ChordSpan>,
    actions: &mut ActionPlan,
    frame: &HarmonicFrame,
) -> (Vec<HarmonicEdit>, RegionTimeline) {
    apply_with_targets(chords, actions, frame, &[])
}

/// [`apply_harmonic_actions`] with forced modulation targets per action (the refusal tests).
fn apply_with_targets(
    chords: &mut Vec<ChordSpan>,
    actions: &mut ActionPlan,
    frame: &HarmonicFrame,
    forced: &[(ActionId, Scale)],
) -> (Vec<HarmonicEdit>, RegionTimeline) {
    let mut regions = RegionTimeline::home(frame.home, frame.total_beats);
    let mut edits = Vec::new();
    let mut locked: Vec<(f64, f64)> = Vec::new();
    // Resolutions and cadences toward home (a return point for a modulation).
    let cadences: Vec<f64> = actions
        .actions
        .iter()
        .filter(|a| {
            a.kind == ActionKind::Resolve
                || (a.kind == ActionKind::Hit
                    && matches!(
                        a.cause,
                        ActionCause::Morphism {
                            morphism: IntentMorphism::Cadence,
                            ..
                        }
                    ))
        })
        .map(|a| a.target_beat.unwrap_or(a.start_beat))
        .collect();
    let order = |actions: &ActionPlan, kinds: &[ActionKind]| -> Vec<ActionId> {
        actions
            .chronological()
            .into_iter()
            .filter(|a| kinds.contains(&a.kind))
            .map(|a| a.id)
            .collect()
    };

    for id in order(actions, &[ActionKind::Modulate]) {
        let Some(a) = actions.actions.iter_mut().find(|a| a.id == id) else {
            continue;
        };
        let to = forced.iter().find(|f| f.0 == id).map(|f| f.1);
        let planned =
            plan_modulation(chords, a, frame, &regions, &locked, &cadences, to).and_then(|m| {
                // Every chord the modulation writes (pivot, new dominant, transposed path, return)
                // must be one the room can sound.
                let written = |c: &&ChordSpan| {
                    !chords.iter().any(|o| {
                        o.start_beat == c.start_beat
                            && o.dur_beats == c.dur_beats
                            && o.chord == c.chord
                    })
                };
                if m.chords
                    .iter()
                    .filter(written)
                    .all(|c| frame.lawful(&c.chord))
                {
                    Ok(m)
                } else {
                    Err("the modulation leaves the world's harmonic vocabulary")
                }
            });
        match planned {
            Ok(m) => {
                *chords = m.chords;
                regions.splice(&m.spans);
                edits.extend(m.edits);
                locked.extend(m.locked);
            }
            Err(reason) => {
                a.kind = ActionKind::Tonicize;
                regions.relabels.push(Relabel {
                    action: id,
                    from: ActionKind::Modulate,
                    to: ActionKind::Tonicize,
                    reason,
                });
            }
        }
    }

    let kinds = [
        ActionKind::Tonicize,
        ActionKind::Reharmonize,
        ActionKind::Recolor,
    ];
    for id in order(actions, &kinds) {
        let Some(a) = actions.actions.iter_mut().find(|a| a.id == id) else {
            continue;
        };
        if a.kind == ActionKind::Tonicize {
            edits.extend(tonicize(chords, a, frame, &regions, &locked));
            continue;
        }
        if let Some((e, verb)) = reharmonize(chords, a, frame, &regions, &locked) {
            if verb != a.kind {
                regions.relabels.push(Relabel {
                    action: id,
                    from: a.kind,
                    to: verb,
                    reason: "no lawful tritone or diatonic third substitute: the chord was recoloured over its own root",
                });
                a.kind = verb;
            }
            edits.push(e);
        }
    }
    (edits, regions)
}

#[cfg(test)]
mod tests {
    use super::super::action::Agent;
    use super::super::context::{analyze, analyze_regions};
    use super::*;

    fn c_major() -> Scale {
        Scale::new(0, Mode::Ionian)
    }

    fn spans(chords: &[(f64, f32, Chord)]) -> Vec<ChordSpan> {
        chords
            .iter()
            .map(|&(s, d, c)| ChordSpan::test(s, d, c))
            .collect()
    }

    fn action(id: u32, kind: ActionKind, start: f64, dur: f64) -> MusicalAction {
        MusicalAction {
            id: ActionId(id),
            cause: ActionCause::Statement { phrase: 0 },
            initiator: Agent::Keys,
            start_beat: start,
            dur_beats: dur,
            kind,
            target_beat: Some(start),
            responders: Vec::new(),
            binding: None,
            pays: None,
            effect: EffectVector::NEUTRAL,
        }
    }

    fn plan_of(actions: Vec<MusicalAction>) -> ActionPlan {
        ActionPlan {
            actions,
            ..ActionPlan::default()
        }
    }

    /// C | Am | F | G | C | Dm | G | C, a bar each.
    fn c_progression() -> Vec<ChordSpan> {
        let q = |r, q| Chord::new(r, q);
        spans(&[
            (0.0, 4.0, q(0, Quality::Maj)),
            (4.0, 4.0, q(9, Quality::Min)),
            (8.0, 4.0, q(5, Quality::Maj)),
            (12.0, 4.0, q(7, Quality::Maj)),
            (16.0, 4.0, q(0, Quality::Maj)),
            (20.0, 4.0, q(2, Quality::Min)),
            (24.0, 4.0, q(7, Quality::Maj)),
            (28.0, 4.0, q(0, Quality::Maj)),
        ])
    }

    #[test]
    fn an_applied_dominant_is_a_tonicization_not_a_region_change() {
        let mut chords = c_progression();
        let mut acts = plan_of(vec![action(0, ActionKind::Tonicize, 4.0, 4.0)]);
        let frame = HarmonicFrame::bare(c_major(), 32.0);
        let (edits, regions) = apply_harmonic_actions(&mut chords, &mut acts, &frame);
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].kind, EditKind::AppliedDominant);
        assert_eq!(edits[0].after, Chord::new(4, Quality::Dom7)); // E7 -> Am
        assert_eq!(regions.spans.len(), 1);
        assert_eq!(regions.transitions(), 0);
        assert!(analyze_regions(&chords, &regions)
            .iter()
            .all(|c| c.region == c_major()));
        // Negative control: a forced Modulated span IS one true region change.
        let mut forced = RegionTimeline::home(c_major(), 32.0);
        forced.splice(&[RegionSpan {
            start_beat: 16.0,
            end_beat: 32.0,
            scale: Scale::new(7, Mode::Ionian),
            kind: RegionKind::Modulated { from: c_major() },
            cause: Some(ActionId(0)),
            established: false,
        }]);
        assert_eq!(forced.transitions(), 1);
        assert_eq!(forced.region_at(20.0), Scale::new(7, Mode::Ionian));
        assert_eq!(forced.region_at(8.0), c_major());
    }

    #[test]
    fn a_modulation_pivots_transposes_the_path_and_returns_home() {
        // Modulate at bar 3 (F), rising -> D major; a Resolve at beat 28 is the planned return.
        let mut chords = c_progression();
        let mut m = action(0, ActionKind::Modulate, 12.0, 4.0);
        m.effect = EffectVector {
            d_pressure: 0.3,
            ..EffectVector::NEUTRAL
        };
        let mut acts = plan_of(vec![m, action(1, ActionKind::Resolve, 28.0, 4.0)]);
        let frame = HarmonicFrame::bare(c_major(), 32.0);
        let (edits, regions) = apply_harmonic_actions(&mut chords, &mut acts, &frame);
        let d = Scale::new(2, Mode::Ionian);
        assert_eq!(acts.actions[0].kind, ActionKind::Modulate, "{:?}", regions);
        let md: Vec<&RegionSpan> = regions.modulations().collect();
        assert_eq!(md.len(), 1, "{}", regions.dump());
        assert_eq!(md[0].scale, d);
        assert!(md[0].established);
        // The pivot is diatonic to both regions.
        let piv = regions
            .spans
            .iter()
            .find(|s| matches!(s.kind, RegionKind::Pivot { .. }))
            .unwrap();
        let pc = chords
            .iter()
            .find(|c| (c.start_beat - piv.start_beat).abs() < 1e-6)
            .unwrap();
        assert!(diatonic_to(&pc.chord, &c_major()) && diatonic_to(&pc.chord, &d));
        // The new V7 then the tonic on a downbeat; the planned return restores home.
        assert!(chords
            .iter()
            .any(|c| c.chord == Chord::new(9, Quality::Dom7)));
        assert!(chords
            .iter()
            .any(|c| c.chord.root_pc == 2 && c.start_beat == 12.0));
        assert_eq!(regions.region_at(29.0), c_major());
        assert_eq!(regions.returns().count(), 1);
        assert_eq!(regions.transitions(), 2);
        assert!(edits.iter().any(|e| e.kind == EditKind::PivotOut));
        // Contexts inside the span are analysed IN the new region.
        let ctx = analyze_regions(&chords, &regions);
        assert!(ctx
            .iter()
            .filter(|c| c.start_beat >= md[0].start_beat && c.start_beat < md[0].end_beat)
            .all(|c| c.region == d));
        // Without rising pressure the rule picks the dominant key.
        assert_eq!(
            modulation_target(&c_major(), false),
            Scale::new(7, Mode::Ionian)
        );
    }

    #[test]
    fn a_relative_key_modulation_is_refused_and_relabelled() {
        // C major -> A minor is the same seven pitch classes: a relabel, not a region change.
        let a_minor = Scale::new(9, Mode::Aeolian);
        assert!(lawful_modulation(&c_major(), &a_minor).is_err());
        assert!(lawful_modulation(&c_major(), &Scale::new(7, Mode::Ionian)).is_ok());
        let mut chords = c_progression();
        let mut acts = plan_of(vec![action(0, ActionKind::Modulate, 12.0, 4.0)]);
        let frame = HarmonicFrame::bare(c_major(), 32.0);
        let (_, regions) =
            apply_with_targets(&mut chords, &mut acts, &frame, &[(ActionId(0), a_minor)]);
        assert_eq!(regions.transitions(), 0);
        assert_eq!(acts.actions[0].kind, ActionKind::Tonicize);
        assert_eq!(regions.relabels.len(), 1);
        assert_eq!(regions.relabels[0].from, ActionKind::Modulate);
    }

    #[test]
    fn a_deceptive_entry_does_not_establish_and_downgrades() {
        // The span opens on a Deflect slot start (the miss stays a miss) and never reaches its
        // tonic before the return: refused, relabelled Tonicize, region unchanged.
        let mut chords = c_progression();
        let mut acts = plan_of(vec![
            action(0, ActionKind::Modulate, 12.0, 4.0),
            action(1, ActionKind::Resolve, 16.0, 4.0),
        ]);
        let mut frame = HarmonicFrame::bare(c_major(), 32.0);
        frame.slots = vec![(12.0, HarmonicGesture::Deflect)];
        let (_, regions) = apply_harmonic_actions(&mut chords, &mut acts, &frame);
        assert_eq!(regions.transitions(), 0, "{}", regions.dump());
        assert_eq!(acts.actions[0].kind, ActionKind::Tonicize);
        assert!(regions.relabels[0].reason.contains("tonic"));
    }

    #[test]
    fn a_tritone_substitute_keeps_the_next_chord_and_the_guide_tritone() {
        let c = c_major();
        let g7 = Chord::new(7, Quality::Dom7);
        let cmaj7 = Chord::new(0, Quality::Maj7);
        let sub = tritone_sub(&g7, Some(&cmaj7), &c).expect("G7 -> C has a tritone substitute");
        assert_eq!(sub, Chord::new(1, Quality::Dom7));
        assert!(common_tones(&g7, &sub) >= 2);
        assert!(PullEvidence::of(&sub, 0).is_dominant());
        // A bare G triad's "substitute" (Db7) shares one tone: rejected.
        let g = Chord::new(7, Quality::Maj);
        assert_eq!(common_tones(&g, &Chord::new(1, Quality::Dom7)), 1);
        assert_eq!(tritone_sub(&g, Some(&cmaj7), &c), None);
        // Through the action: Dm7 | G7 | Cmaj7, Reharmonize on the G7.
        let mut chords = spans(&[
            (0.0, 4.0, Chord::new(2, Quality::Min7)),
            (4.0, 4.0, g7),
            (8.0, 4.0, cmaj7),
        ]);
        let mut acts = plan_of(vec![action(0, ActionKind::Reharmonize, 4.0, 4.0)]);
        let (edits, _) =
            apply_harmonic_actions(&mut chords, &mut acts, &HarmonicFrame::bare(c, 12.0));
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].kind, EditKind::TritoneSub);
        assert_eq!(chords[1].chord, sub);
        assert_eq!(chords[2].chord, cmaj7, "the next chord is unchanged");
        assert_eq!(acts.actions[0].kind, ActionKind::Reharmonize);
    }

    #[test]
    fn a_third_substitute_keeps_function_and_a_fallback_is_labelled_recolor() {
        let c = c_major();
        // F (IV) -> Dm (ii): a third below, two common tones, both departures.
        let f = Chord::new(5, Quality::Maj);
        let sub = third_sub(&f, Some(&Chord::new(0, Quality::Maj)), None, &c).unwrap();
        assert_eq!(sub, Chord::new(2, Quality::Min));
        assert_eq!(contextual_function(&sub, &c), contextual_function(&f, &c));
        // Never on the tonic.
        assert_eq!(
            third_sub(&Chord::new(0, Quality::Maj), None, None, &c),
            None
        );
        // A tonic under a Reharmonize has no lawful substitute: recoloured and LABELLED Recolor.
        let mut chords = spans(&[(0.0, 4.0, Chord::new(0, Quality::Maj))]);
        let mut acts = plan_of(vec![action(0, ActionKind::Reharmonize, 0.0, 4.0)]);
        let (edits, regions) =
            apply_harmonic_actions(&mut chords, &mut acts, &HarmonicFrame::bare(c, 4.0));
        assert_eq!(edits[0].kind, EditKind::Recolor);
        assert_eq!(chords[0].chord, Chord::new(0, Quality::Maj7));
        assert_eq!(acts.actions[0].kind, ActionKind::Recolor);
        assert_eq!(regions.relabels[0].to, ActionKind::Recolor);
    }

    fn compose(
        trace: &super::super::semantic::SemanticTrace,
        world: &super::super::world::MusicWorld,
    ) -> super::super::functor::Composition {
        super::super::functor::compose_full(
            trace,
            world,
            2112,
            Some(super::super::contract::CompositionGrammar::DeflectedLift),
            super::super::performance::PerformanceOptions::default(),
        )
    }

    #[test]
    fn the_cinematic_impact_really_modulates_and_comes_home() {
        use super::super::diagnostics::RealizationDiagnostics;
        use super::super::score::Role;
        let trace = super::super::semantic::demo_trace(120.0);
        for world in super::super::world::MusicWorld::all() {
            let c = compose(&trace, &world);
            let (perf, name) = (&c.perf, world.name);
            let home = perf.region;
            let m = perf
                .actions
                .of_kind(ActionKind::Modulate)
                .next()
                .unwrap_or_else(|| {
                    panic!(
                        "{name}: the Impact's Modulate was downgraded: {:?}",
                        perf.regions.relabels
                    )
                });
            let md: Vec<&RegionSpan> = perf.regions.modulations().collect();
            assert_eq!(md.len(), 1, "{name}: {}", perf.regions.dump());
            let span = md[0];
            assert!(span.established && span.cause == Some(m.id));
            assert!(lawful_modulation(&home, &span.scale).is_ok());
            // The pivot chord is diatonic to both regions.
            let piv = perf
                .regions
                .spans
                .iter()
                .find(|s| matches!(s.kind, RegionKind::Pivot { .. }))
                .unwrap();
            let pc = perf.context_at(piv.start_beat).unwrap().chord;
            assert!(
                diatonic_to(&pc, &home) && diatonic_to(&pc, &span.scale),
                "{name}: pivot {} is not common to both regions",
                pc.label()
            );
            // Contexts inside the span live in the new region; after the return, home again.
            let inside: Vec<_> = perf
                .contexts
                .iter()
                .filter(|x| x.start_beat >= span.start_beat && x.start_beat < span.end_beat)
                .collect();
            assert!(!inside.is_empty() && inside.iter().all(|x| x.region == span.scale));
            let back = perf.regions.returns().next().expect("a planned return");
            assert_eq!(perf.region_at(back.end_beat + 0.5), home, "{name}");
            assert_eq!(perf.regions.transitions(), 2, "{name}");
            // Every note stays justified, the line needs no repair, and the lead inside the span
            // really sings the new key (a pitch class home does not have).
            let d = RealizationDiagnostics::measure(&c.song.plan, &c.score);
            assert!(
                d.unjustified_by_role.iter().all(|r| r.1 == 0),
                "{name}: {d:?}"
            );
            assert_eq!(c.score.melody_repairs, 0, "{name}");
            let lead_in: Vec<_> = c
                .score
                .notes
                .iter()
                .filter(|n| {
                    n.role == Role::Lead
                        && n.start_beat >= span.start_beat
                        && n.start_beat < span.end_beat
                })
                .collect();
            assert!(lead_in.iter().all(|n| n.function.is_some()), "{name}");
            // The audit sees the modulation in the notes.
            let w = super::super::witness::audit(perf, &c.score);
            eprintln!("{name}: {}\n{}", perf.regions.dump(), w.report());
            let row = w.rows.iter().find(|r| r.action == m.id).unwrap();
            assert!(row.witnessed, "{name}: {}", w.report());
            for r in w.rows.iter().filter(|r| {
                matches!(
                    r.kind,
                    ActionKind::Recolor | ActionKind::Tonicize | ActionKind::Modulate
                )
            }) {
                assert!(r.witnessed, "{name}: {}", w.report());
            }
        }
    }

    #[test]
    fn the_flagship_stays_home() {
        // The DeflectedLift flagship fires no Modulate / Reharmonize: one home region, no edits
        // (its audio is bit-identical to the pre-region build).
        let trace = super::super::semantic::deflected_lift_trace(120.0);
        for world in super::super::world::MusicWorld::all() {
            let c = compose(&trace, &world);
            assert_eq!(c.perf.regions.spans.len(), 1, "{}", world.name);
            assert_eq!(c.perf.regions.transitions(), 0);
            assert!(c.perf.edits.is_empty());
            assert!(c.perf.contexts.iter().all(|x| x.region == c.perf.region));
        }
    }

    #[test]
    fn analyze_is_analyze_regions_over_one_home_span() {
        let chords = c_progression();
        let a = analyze(&chords, &c_major());
        let b = analyze_regions(&chords, &RegionTimeline::home(c_major(), 32.0));
        assert_eq!(a, b);
    }
}
