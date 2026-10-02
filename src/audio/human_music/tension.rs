//! Round XIIIb sounding tension: a note that sounds wrong in its context must be quickly
//! transient, or foreshadow what justifies it.
//!
//! The Round XIII listen: "the same shitty chord at 16 s" survived the mass arm, and BLACK_ICE
//! kept its wrong notes. The chord ringing from 14.24 s to 16.53 s is the SWISS pad's written
//! Cmaj7, voiced E4 B4 C5 G5: the major seventh a semitone under the root, both held for 2.29 s,
//! and the B moves on only when the whole chord changes. Every pitch in it is a written chord
//! tone, so no ownership audit can see it; what is wrong is what *sounds*. BLACK_ICE has the same
//! shape in every Am9 pad (B4 under C5 for 3.04 s), and a lead that strikes C#5 against the pad's
//! C5 and then falls back onto the C that was sounding all along.
//!
//! So tension is measured on the sounding texture, not on the chord symbol. A [`Clash`] is two
//! pitched notes heard at the same time a minor second (or augmented unison) or a minor ninth
//! apart ([`CLASH_SEMITONES`]), in any roles. Heard windows are Round XIII's: the patch envelope
//! ([`audible_end`]), with a release tail cut at the same role's next attack. One voice's own tail
//! into its next attack is legato, not a clash. A clash's exposure is its overlap in seconds times
//! the metric salience where it begins, classed by the Round XIII bands
//! ([`FLEETING_MAX_SECS`], [`ASSERTED_MIN_SECS`](super::mass::ASSERTED_MIN_SECS)). The note that owes the justification is the
//! less structural one: root < 3rd < 5th < 7th/6th < written extension < diatonic non-chord tone <
//! chromatic tone. A tie goes to the later attack, then the upper note.
//!
//! The responsible note is justified ([`TensionVerdict`]) when the clash is:
//! * **Transient**: its exposure is Fleeting;
//! * **Foreshadowing**: not Asserted, and its voice steps (1–2 semitones) to a new pitch within
//!   [`RESOLVE_WITHIN_SECS`] of its release, unless that pitch is the partner a semitone away,
//!   already sounding (the "resolution" was there all along, so the ear hears two adjacent
//!   pitches, not motion);
//! * **Suspension**: prepared (it sounded before its partner arrived) and resolving by step as
//!   above, at any exposure;
//! * **Anticipation**: not Asserted, a tone of the next harmony, and sounding into it.
//!
//! Anything else is **Unjustified**. Mass is still evidence, never a verdict: a long clash is
//! fine when it is a prepared suspension, and a short one is fine because it is short.
//!
//! [`gate_sounding_tension`] applies the same law to a realized band, one unjustified clash at a
//! time, by the smallest edit that removes it without creating another. A support voicing note
//! (pad; keys comping/holds) makes room: it is shortened (when a line arrives against it), a
//! doubled or bass-covered member is omitted, or it moves by whole octaves (its pitch class
//! kept, so an authored seventh or ninth survives); only then is it omitted outright, or moved
//! to the nearest written chord tone its voicing lacks. A line note (lead, bass, keys answer/figure)
//! becomes transient: it is re-timed as an ornament leaning into its voice's next note (a later
//! onset inside its own written slot, a faster value), else shortened in place, else stepped to a
//! neighbouring scale pitch.

use super::context::{context_at, HarmonicContext};
use super::mass::{patch, Accent, ExposureClass, FLEETING_MAX_SECS};
use super::performance::STEP_BEATS;
use super::score::{Note, PitchFunction, Role, Score};
use super::sonority::audible_end;
use super::theory::{note_name, pitch_class, Midi};
use super::world::MusicWorld;
use std::fmt::Write;

const EPS: f64 = 1e-6;

/// Absolute intervals (semitones) that clash when two pitched notes sound together: the minor
/// second or augmented unison (1) and the minor ninth (13). Witnesses: the SWISS Cmaj7 pad's B4
/// under C5 and the BLACK_ICE lead's C#5 over the pad's C5 (both 1); the SWISS closing pad's B3
/// under C5 (13). Deliberately *not* clashes: the major second, which is the dialects' stock
/// colour (C6's G–A, Am9's A–B), and the tritone, which Am6 contains by definition (C–F#).
pub const CLASH_SEMITONES: [i32; 2] = [1, 13];

/// Seconds after a note's heard end within which its voice's next attack still continues it (a
/// step there is a resolution, not a new thought). Calibrated heuristic: a half beat at 88 BPM
/// (0.34 s). Witness: the BLACK_ICE lead's C#5 at beat 24.5 is heard to 25.15 and followed at 25.5
/// by C5, and that C5 is its continuation.
pub const RESOLVE_WITHIN_SECS: f64 = 0.35;

/// The finest value an ornament is re-timed to, in beats (a thirty-second note).
pub const ORNAMENT_GRID_BEATS: f64 = STEP_BEATS / 2.0;

/// An ornament leans only on a next note that follows its release within this many beats; after
/// a longer rest the re-timed note would belong to a different gesture.
pub const ORNAMENT_REACH_BEATS: f64 = 1.0;

/// The registers (MIDI, inclusive) a support voice may be moved into by whole octaves. The pad's
/// is the union of its voicing ranges (`VoiceRange::pad` 52–79 and `pad_upper` 62–91); the
/// keys' is their line range.
pub const PAD_REGISTER: (Midi, Midi) = (52, 91);
/// See [`PAD_REGISTER`].
pub const KEYS_REGISTER: (Midi, Midi) = (55, 88);

/// How a clash's responsible note is justified, weakest demand first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TensionVerdict {
    /// Fleeting exposure: heard as part of a motion, not evaluated alone.
    Transient,
    /// Steps on to a new pitch that the clash was pointing at.
    Foreshadowing,
    /// Prepared before its partner arrived, and resolving by step.
    Suspension,
    /// A tone of the next harmony, sounding into it.
    Anticipation,
    /// Exposed, and going nowhere that explains it.
    Unjustified,
}

/// Two simultaneously heard notes a [`CLASH_SEMITONES`] interval apart. Indices are into the note
/// slice the clash was measured on. Units: beats, seconds, weighted seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clash {
    /// The note that owes the justification.
    pub tension: usize,
    /// The note it clashes with.
    pub partner: usize,
    /// Where both first sound (beats).
    pub onset_beat: f64,
    /// How long both sound together (seconds).
    pub overlap_secs: f64,
    /// Metric salience where the clash begins.
    pub salience: f64,
    /// `overlap_secs × salience` (weighted seconds).
    pub exposure: f64,
    pub class: ExposureClass,
    /// The tension voice's next note, if one continues it in time.
    pub continuation: Option<usize>,
    pub verdict: TensionVerdict,
}

/// Historical compatibility view of the shared direct-voice observation authority.
/// The next same-role attack masks release tails under the frozen historical model.
pub fn heard_windows(notes: &[Note], world: &MusicWorld, tempo_bpm: f32) -> Vec<(f64, f64)> {
    super::voice::HeardWindows::historical(notes, world, tempo_bpm).into_windows()
}

/// Score-aware observation. Modern empty-edge scores retain the canonical envelope lifetime;
/// only the explicitly archived observation contract applies role masking.
pub fn heard_windows_score(score: &Score, world: &MusicWorld) -> Vec<(f64, f64)> {
    super::voice::HeardWindows::of_score(score, world).into_windows()
}

/// How structural `pitch` is over `ctx`, 0 = the root: its place in the written chord, then 8 for
/// a tone of the region's scale, 9 for a chromatic tone.
fn rank(ctx: &HarmonicContext, pitch: Midi) -> u8 {
    let pc = pitch_class(pitch);
    match ctx.chord.pitch_classes().iter().position(|&c| c == pc) {
        Some(i) => i as u8,
        None if ctx.region.contains_pc(pc) => 8,
        None => 9,
    }
}

/// Every clash in `notes`, in onset order of the earlier note.
pub fn clashes(
    notes: &[Note],
    contexts: &[HarmonicContext],
    world: &MusicWorld,
    tempo_bpm: f32,
    beats_per_bar: f64,
) -> Vec<Clash> {
    let spb = 60.0 / f64::from(tempo_bpm.max(1.0));
    let win = heard_windows(notes, world, tempo_bpm);
    clashes_in_windows(notes, contexts, spb, beats_per_bar, &win, None)
}

fn clashes_in_windows(
    notes: &[Note],
    contexts: &[HarmonicContext],
    spb: f64,
    beats_per_bar: f64,
    win: &[(f64, f64)],
    links: Option<&[super::voice::VoiceContinuation]>,
) -> Vec<Clash> {
    let mut order: Vec<usize> = (0..notes.len()).collect();
    order.sort_by(|&a, &b| notes[a].start_beat.total_cmp(&notes[b].start_beat));
    let mut out = Vec::new();
    for (k, &a) in order.iter().enumerate() {
        for &b in &order[k + 1..] {
            if notes[b].start_beat >= win[a].1 - EPS {
                break;
            }
            if !CLASH_SEMITONES.contains(&(notes[a].pitch - notes[b].pitch).abs()) {
                continue;
            }
            // One voice's legato: its tail into the role's next attack.
            let a_end = notes[a].start_beat + f64::from(notes[a].dur_beats);
            let connected = links.map_or(notes[a].role == notes[b].role, |links| {
                links.iter().any(|l| {
                    l.from == super::voice::VoiceEventId::of(&notes[a])
                        && l.to == super::voice::VoiceEventId::of(&notes[b])
                })
            });
            if connected && a_end <= notes[b].start_beat + EPS {
                continue;
            }
            let (s, e) = (notes[b].start_beat, win[a].1.min(win[b].1));
            if e - s <= EPS {
                continue;
            }
            let Some(ctx) = context_at(contexts, s) else {
                continue;
            };
            let (ra, rb) = (rank(ctx, notes[a].pitch), rank(ctx, notes[b].pitch));
            let (t, c) = if ra != rb {
                if ra > rb {
                    (a, b)
                } else {
                    (b, a)
                }
            } else if notes[b].start_beat > notes[a].start_beat + EPS
                || notes[b].pitch > notes[a].pitch
            {
                (b, a)
            } else {
                (a, b)
            };
            out.push(judge(notes, win, contexts, spb, beats_per_bar, t, c, s, e));
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn judge(
    notes: &[Note],
    win: &[(f64, f64)],
    contexts: &[HarmonicContext],
    spb: f64,
    beats_per_bar: f64,
    t: usize,
    c: usize,
    s: f64,
    e: f64,
) -> Clash {
    let overlap_secs = (e - s) * spb;
    let salience = Accent::of(s, beats_per_bar).salience();
    let exposure = overlap_secs * salience;
    let class = ExposureClass::of(exposure, false);
    let continuation = continuation(notes, win, spb, t);
    let verdict = if class == ExposureClass::Fleeting {
        TensionVerdict::Transient
    } else {
        let (tp, cp) = (notes[t].pitch, notes[c].pitch);
        let step = continuation.filter(|&r| (1..=2).contains(&(notes[r].pitch - tp).abs()));
        let preempted = step.is_some_and(|r| (tp - cp).abs() == 1 && notes[r].pitch == cp);
        let prepared = notes[t].start_beat < notes[c].start_beat - EPS;
        let anticipates = contexts
            .iter()
            .find(|x| x.start_beat > notes[t].start_beat + EPS)
            .is_some_and(|x| {
                x.start_beat <= win[t].1 + EPS && x.chord.contains_pc(pitch_class(tp))
            });
        if step.is_some() && !preempted && (class < ExposureClass::Asserted || prepared) {
            if prepared {
                TensionVerdict::Suspension
            } else {
                TensionVerdict::Foreshadowing
            }
        } else if class < ExposureClass::Asserted && anticipates {
            TensionVerdict::Anticipation
        } else {
            TensionVerdict::Unjustified
        }
    };
    Clash {
        tension: t,
        partner: c,
        onset_beat: s,
        overlap_secs,
        salience,
        exposure,
        class,
        continuation,
        verdict,
    }
}

/// The note of `t`'s role that continues it: the earliest later attack within
/// [`RESOLVE_WITHIN_SECS`] of its heard end, and in that attack the nearest pitch (ties low).
fn continuation(notes: &[Note], win: &[(f64, f64)], spb: f64, t: usize) -> Option<usize> {
    let n = &notes[t];
    let reach = win[t].1 + RESOLVE_WITHIN_SECS / spb;
    let later = |m: &Note| {
        m.role == n.role && m.start_beat > n.start_beat + EPS && m.start_beat <= reach + EPS
    };
    let first = notes
        .iter()
        .filter(|m| later(m))
        .map(|m| m.start_beat)
        .min_by(f64::total_cmp)?;
    (0..notes.len())
        .filter(|&j| later(&notes[j]) && (notes[j].start_beat - first).abs() < EPS)
        .min_by_key(|&j| ((notes[j].pitch - n.pitch).abs(), notes[j].pitch))
}

/// The sounding-tension audit of one realized score.
#[derive(Debug, Clone)]
pub struct TensionDiagnostics {
    pub clashes: Vec<Clash>,
    pub transient: usize,
    pub foreshadowing: usize,
    pub suspension: usize,
    pub anticipation: usize,
    pub unjustified: usize,
    tempo_bpm: f32,
}

impl TensionDiagnostics {
    /// Audit `score` over `contexts` under `world`'s patches.
    pub fn measure(
        score: &Score,
        contexts: &[HarmonicContext],
        world: &MusicWorld,
    ) -> TensionDiagnostics {
        let clashes = if score.observed_lifetime_policy()
            == super::voice::ObservedLifetimePolicy::ExplicitContinuity
        {
            clashes_in_windows(
                &score.notes,
                contexts,
                60.0 / f64::from(score.tempo_bpm.max(1.0)),
                score.beats_per_bar,
                &heard_windows_score(score, world),
                Some(&score.voice_continuity),
            )
        } else {
            clashes(
                &score.notes,
                contexts,
                world,
                score.tempo_bpm,
                score.beats_per_bar,
            )
        };
        let count = |v: TensionVerdict| clashes.iter().filter(|c| c.verdict == v).count();
        TensionDiagnostics {
            transient: count(TensionVerdict::Transient),
            foreshadowing: count(TensionVerdict::Foreshadowing),
            suspension: count(TensionVerdict::Suspension),
            anticipation: count(TensionVerdict::Anticipation),
            unjustified: count(TensionVerdict::Unjustified),
            clashes,
            tempo_bpm: score.tempo_bpm,
        }
    }

    /// One line per clash in `clashes` of `score`.
    fn lines(&self, score: &Score, pick: impl Fn(&Clash) -> bool) -> String {
        let spb = 60.0 / f64::from(self.tempo_bpm.max(1.0));
        let mut out = String::new();
        for c in self.clashes.iter().filter(|c| pick(c)) {
            let (t, p) = (&score.notes[c.tension], &score.notes[c.partner]);
            let _ = writeln!(
                out,
                "{:>8.3} {:>7.3}s {:<13} {:<5} {:<5.2} vs {:<13} {:<5} int={:<2} overlap={:.3}s sal={:.2} exposure={:.3} {:?} next={} {:?}",
                c.onset_beat,
                c.onset_beat * spb,
                format!("{}:{}", t.role.label(), t.prov.role_note),
                note_name(t.pitch),
                t.dur_beats,
                format!("{}:{}", p.role.label(), p.prov.role_note),
                note_name(p.pitch),
                (t.pitch - p.pitch).abs(),
                c.overlap_secs,
                c.salience,
                c.exposure,
                c.class,
                c.continuation
                    .map_or("-".into(), |r| note_name(score.notes[r].pitch)),
                c.verdict
            );
        }
        out
    }

    /// Counts, then every clash that is not transient.
    pub fn report(&self, score: &Score) -> String {
        format!(
            "clashes={} transient={} foreshadowing={} suspension={} anticipation={} unjustified={}\n{}",
            self.clashes.len(),
            self.transient,
            self.foreshadowing,
            self.suspension,
            self.anticipation,
            self.unjustified,
            self.lines(score, |c| c.verdict != TensionVerdict::Transient)
        )
    }

    /// Every clash sounding within `radius_secs` of `center_secs`.
    pub fn around_seconds(&self, score: &Score, center_secs: f64, radius_secs: f64) -> String {
        let spb = 60.0 / f64::from(self.tempo_bpm.max(1.0));
        self.lines(score, |c| {
            let s = c.onset_beat * spb;
            s <= center_secs + radius_secs && s + c.overlap_secs >= center_secs - radius_secs
        })
    }
}

/// What the tension gate did to one note.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TensionAction {
    /// A support voice released early, at this written length (beats): it makes room.
    Shortened(f32),
    /// Removed; its pitch class still sounds (doubled in the voicing, or the root in the bass).
    OmittedCovered,
    /// Removed as a last resort; its instrument still sounds at least two other voices.
    Omitted,
    /// Moved by whole octaves to this pitch: same pitch class, a register where it clashes with
    /// nothing.
    Displaced(Midi),
    /// A line note re-timed as an ornament leaning into its voice's next note.
    Ornament { start_beat: f64, dur_beats: f32 },
    /// Moved to this pitch as a last resort: a support voice to the nearest written chord tone
    /// its voicing lacks; a line note to a neighbouring scale pitch.
    Moved(Midi),
}

/// One tension-gate decision.
#[derive(Debug, Clone, Copy)]
pub struct TensionEdit {
    /// The note edited, as it was.
    pub note: Note,
    /// The clash that asked for the edit, as measured before it: the responsible note and its
    /// partner (one of the two is `note`).
    pub tension: Note,
    pub partner: Note,
    pub exposure: f64,
    pub class: ExposureClass,
    pub action: TensionAction,
}

/// Accepts a candidate band only when it still witnesses every interaction receipt the band
/// witnessed before the gate (see [`gate_sounding_tension`]).
pub type ReceiptGuard<'a> = dyn Fn(&[Note]) -> bool + 'a;

/// A pad voice, or a keys comping stab or hold: a voicing member, not a line.
fn is_voicing(n: &Note) -> bool {
    n.role == Role::Pad || (n.role == Role::Keys && matches!(n.prov.role_note, "comp" | "hold"))
}

fn register(role: Role) -> (Midi, Midi) {
    if role == Role::Keys {
        KEYS_REGISTER
    } else {
        PAD_REGISTER
    }
}

fn same_bundle(a: &Note, b: &Note) -> bool {
    a.role == b.role && (a.start_beat - b.start_beat).abs() < EPS
}

/// Apply the sounding-tension law to a realized band (every pitched note, all roles), editing
/// `notes` in place. Returns the gate's ledger in decision order. Drums, SFX and the chart are not
/// its business; no note is ever added or moved earlier. `keeps_receipts`, when given, must
/// accept every candidate band (true: every interaction receipt the band witnessed is still
/// witnessed); a candidate it refuses is never taken, so the gate cannot spend a receipt silently.
pub fn gate_sounding_tension(
    notes: &mut Vec<Note>,
    contexts: &[HarmonicContext],
    world: &MusicWorld,
    tempo_bpm: f32,
    beats_per_bar: f64,
    keeps_receipts: Option<&ReceiptGuard<'_>>,
) -> Vec<TensionEdit> {
    let spb = 60.0 / f64::from(tempo_bpm.max(1.0));
    let unjustified = |ns: &[Note]| -> Vec<Clash> {
        clashes(ns, contexts, world, tempo_bpm, beats_per_bar)
            .into_iter()
            .filter(|c| c.verdict == TensionVerdict::Unjustified)
            .collect()
    };
    let mut ledger = Vec::new();
    // Clashes no candidate could settle (by tension and partner note), and voicings that already
    // lost a voice (Round VIII: never thin the bed twice in one chord).
    let mut stuck: Vec<(Note, Note)> = Vec::new();
    let mut thinned: Vec<(Role, f64)> = Vec::new();
    let same = |a: &Note, b: &Note| {
        a.role == b.role && a.pitch == b.pitch && (a.start_beat - b.start_beat).abs() < EPS
    };
    for _ in 0..=notes.len() {
        let bad = unjustified(notes);
        let Some(x) = bad
            .iter()
            .filter(|c| {
                !stuck
                    .iter()
                    .any(|(t, p)| same(t, &notes[c.tension]) && same(p, &notes[c.partner]))
            })
            .min_by(|a, b| {
                a.onset_beat
                    .total_cmp(&b.onset_beat)
                    .then(b.exposure.total_cmp(&a.exposure))
            })
            .copied()
        else {
            break;
        };
        let (t, c) = (notes[x.tension], notes[x.partner]);
        let mut candidates: Vec<(usize, TensionAction)> = Vec::new();
        let ctx_at = |beat: f64| context_at(contexts, beat);
        let bundle = |ns: &[Note], i: usize| -> Vec<usize> {
            (0..ns.len())
                .filter(|&j| j != i && same_bundle(&ns[j], &ns[i]))
                .collect()
        };
        let win = heard_windows(notes, world, tempo_bpm);
        let covered = |i: usize| -> bool {
            let n = &notes[i];
            let pc = pitch_class(n.pitch);
            bundle(notes, i)
                .iter()
                .any(|&j| pitch_class(notes[j].pitch) == pc)
                || ctx_at(n.start_beat).is_some_and(|ctx| {
                    pc == ctx.chord.root_pc
                        && (0..notes.len()).any(|j| {
                            notes[j].role == Role::Bass
                                && pitch_class(notes[j].pitch) == pc
                                && win[j].0 <= n.start_beat + EPS
                                && win[j].1 > n.start_beat + EPS
                        })
                })
        };
        // A voice may go when its instrument still sounds two others where it starts, and its
        // own voicing has not already lost one.
        let can_thin = |i: usize| -> bool {
            let n = &notes[i];
            let others = (0..notes.len())
                .filter(|&j| {
                    j != i
                        && notes[j].role == n.role
                        && win[j].0 <= n.start_beat + EPS
                        && win[j].1 > n.start_beat + EPS
                })
                .count();
            others >= 2
                && !thinned
                    .iter()
                    .any(|&(r, b)| r == n.role && (b - n.start_beat).abs() < EPS)
        };
        let octaves = |i: usize| -> Vec<Midi> {
            let n = &notes[i];
            let (lo, hi) = register(n.role);
            let others = bundle(notes, i);
            let centre = if others.is_empty() {
                f64::from(n.pitch)
            } else {
                others
                    .iter()
                    .map(|&j| f64::from(notes[j].pitch))
                    .sum::<f64>()
                    / others.len() as f64
            };
            let mut v: Vec<Midi> = [n.pitch + 12, n.pitch - 12, n.pitch + 24, n.pitch - 24]
                .into_iter()
                .filter(|&q| (lo..=hi).contains(&q) && others.iter().all(|&j| notes[j].pitch != q))
                .collect();
            v.sort_by(|a, b| {
                (f64::from(*a) - centre)
                    .abs()
                    .total_cmp(&(f64::from(*b) - centre).abs())
            });
            v
        };
        let support = |i: usize, against_line: bool, out: &mut Vec<(usize, TensionAction)>| {
            let n = &notes[i];
            let other = if i == x.tension { x.partner } else { x.tension };
            // 1. A line arrived against a held voice: the voice makes room, longest first.
            if against_line && notes[other].start_beat > n.start_beat + EPS {
                let mut len = ((f64::from(n.dur_beats) / STEP_BEATS).ceil() - 1.0) * STEP_BEATS;
                while len >= STEP_BEATS - EPS {
                    out.push((i, TensionAction::Shortened(len as f32)));
                    len -= STEP_BEATS;
                }
            }
            // 2. A member whose pitch class still sounds without it.
            if covered(i) && can_thin(i) {
                out.push((i, TensionAction::OmittedCovered));
            }
        };
        let support_moves = |i: usize, out: &mut Vec<(usize, TensionAction)>| {
            for q in octaves(i) {
                out.push((i, TensionAction::Displaced(q)));
            }
        };
        if is_voicing(&t) {
            let line_partner = !is_voicing(&c);
            support(x.tension, line_partner, &mut candidates);
            if is_voicing(&c) {
                support(x.partner, false, &mut candidates);
            }
            support_moves(x.tension, &mut candidates);
            if is_voicing(&c) {
                support_moves(x.partner, &mut candidates);
            }
            if can_thin(x.tension) {
                candidates.push((x.tension, TensionAction::Omitted));
            }
            // Last: the nearest written chord tone its voicing does not already sound.
            if let Some(ctx) = ctx_at(t.start_beat) {
                let (lo, hi) = register(t.role);
                let others = bundle(notes, x.tension);
                for d in 1..=12 {
                    for q in [t.pitch - d, t.pitch + d] {
                        if (lo..=hi).contains(&q)
                            && ctx.chord.contains_pc(pitch_class(q))
                            && others.iter().all(|&j| notes[j].pitch != q)
                        {
                            candidates.push((x.tension, TensionAction::Moved(q)));
                        }
                    }
                }
            }
        } else {
            // A line note: make it transient, leaning into its voice's next note.
            let i = x.tension;
            let next = notes
                .iter()
                .filter(|m| m.role == t.role && m.start_beat > t.start_beat + EPS)
                .map(|m| m.start_beat)
                .min_by(f64::total_cmp);
            let end = t.start_beat + f64::from(t.dur_beats);
            if let Some(o) = next.filter(|&o| o - end <= ORNAMENT_REACH_BEATS + EPS) {
                let mut g = ((f64::from(t.dur_beats).min(o - t.start_beat)) / ORNAMENT_GRID_BEATS)
                    .floor()
                    * ORNAMENT_GRID_BEATS;
                while g >= ORNAMENT_GRID_BEATS - EPS {
                    let at = o - g;
                    // Played late and quick, but inside its own written slot: the same note in
                    // the same place, not a new one. Fleeting with its whole release tail, not
                    // only up to the next attack that masks it.
                    let heard = audible_end(at, g, patch(world, t.role), tempo_bpm) - at;
                    if heard * spb * Accent::of(at, beats_per_bar).salience() < FLEETING_MAX_SECS
                        && at >= t.start_beat - EPS
                        && at < end - EPS
                    {
                        candidates.push((
                            i,
                            TensionAction::Ornament {
                                start_beat: at,
                                dur_beats: g as f32,
                            },
                        ));
                        break;
                    }
                    g -= ORNAMENT_GRID_BEATS;
                }
            }
            let mut len =
                ((f64::from(t.dur_beats) / ORNAMENT_GRID_BEATS).ceil() - 1.0) * ORNAMENT_GRID_BEATS;
            while len >= ORNAMENT_GRID_BEATS - EPS {
                candidates.push((i, TensionAction::Shortened(len as f32)));
                len -= ORNAMENT_GRID_BEATS;
            }
            // A support partner may make room for the line instead.
            if is_voicing(&c) {
                support(x.partner, true, &mut candidates);
                support_moves(x.partner, &mut candidates);
            }
            if let Some(ctx) = ctx_at(t.start_beat) {
                for d in [1, -1, 2, -2] {
                    let q = t.pitch + d;
                    if ctx.region.contains_pc(pitch_class(q)) {
                        candidates.push((i, TensionAction::Moved(q)));
                    }
                }
            }
        }
        // The first candidate that leaves fewer unjustified clashes and none on the edited note.
        let before = bad.len();
        let mut done = false;
        for (i, action) in candidates {
            let mut trial = notes.clone();
            let edited = trial[i];
            let keep = apply(&mut trial, i, action, contexts);
            let after = unjustified(&trial);
            let clean = keep.is_none_or(|k| after.iter().all(|a| a.tension != k && a.partner != k));
            if after.len() < before && clean && keeps_receipts.is_none_or(|k| k(&trial)) {
                if matches!(
                    action,
                    TensionAction::OmittedCovered | TensionAction::Omitted
                ) {
                    thinned.push((edited.role, edited.start_beat));
                }
                ledger.push(TensionEdit {
                    note: edited,
                    tension: t,
                    partner: c,
                    exposure: x.exposure,
                    class: x.class,
                    action,
                });
                *notes = trial;
                done = true;
                break;
            }
        }
        if !done {
            stuck.push((t, c));
        }
    }
    ledger
}

/// Apply `action` to note `i` of `notes`; the edited note's index afterwards, if it survives.
fn apply(
    notes: &mut Vec<Note>,
    i: usize,
    action: TensionAction,
    contexts: &[HarmonicContext],
) -> Option<usize> {
    match action {
        TensionAction::Shortened(len) => notes[i].dur_beats = len,
        TensionAction::OmittedCovered | TensionAction::Omitted => {
            notes.remove(i);
            return None;
        }
        TensionAction::Displaced(q) => notes[i].pitch = q,
        TensionAction::Ornament {
            start_beat,
            dur_beats,
        } => {
            notes[i].start_beat = start_beat;
            notes[i].dur_beats = dur_beats;
        }
        TensionAction::Moved(q) => {
            notes[i].pitch = q;
            notes[i].function = context_at(contexts, notes[i].start_beat).and_then(|ctx| {
                let pc = pitch_class(q);
                if ctx.chord.contains_pc(pc) {
                    Some(PitchFunction::ChordTone)
                } else if ctx.palette.tensions.contains(&pc) {
                    Some(PitchFunction::LicensedExtension)
                } else {
                    notes[i].function
                }
            });
        }
    }
    Some(i)
}
