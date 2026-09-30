//! The **Score IR**: an inspectable, deterministic intermediate representation. Notes,
//! drum hits and SFX events all carry *provenance* (which section, which motif, which
//! semantic role / intent morphism produced them), which makes the score debuggable,
//! replayable and testable — and lets a dump explain *why* every event exists.

use super::form::{Section, SectionKind};
use super::harmony::ChordSpan;
use super::ids::{ActionId, ActionStamp, InteractionId, MaterialId, ObligationId};
use super::theory::Midi;

/// A melodic/harmonic instrument role (maps to a world [`super::instrument::Patch`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Pad,
    Bass,
    Lead,
    Keys,
}

impl Role {
    /// Every pitched role, in a fixed order — for exhaustive per-role diagnostics.
    pub const ALL: [Role; 4] = [Role::Pad, Role::Keys, Role::Bass, Role::Lead];

    /// A short lowercase label for dumps and diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            Role::Pad => "pad",
            Role::Bass => "bass",
            Role::Lead => "lead",
            Role::Keys => "keys",
        }
    }
}

/// A synthesized drum voice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrumVoice {
    Kick,
    Snare,
    ClosedHat,
    OpenHat,
    Clap,
}

/// A synthesized SFX sting kind (derived from the world palette).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfxKind {
    Acquire,
    Confirm,
    Warning,
    Danger,
    Transition,
    Impact,
}

impl SfxKind {
    /// Every SFX gesture, in a fixed order — for exhaustive lifecycle tests.
    pub const ALL: [SfxKind; 6] = [
        SfxKind::Acquire,
        SfxKind::Confirm,
        SfxKind::Warning,
        SfxKind::Danger,
        SfxKind::Transition,
        SfxKind::Impact,
    ];

    /// The voice envelope `(attack, decay, sustain_level, release)` in seconds. `sustain_level`
    /// is a level in `[0, 1]` that is held only for [`SfxKind::hold_secs`] before the gesture is
    /// gated off — an SFX is a bounded one-shot, never an indefinitely gated tone. This is the
    /// single source of truth for the envelope; the synth reads it rather than keeping its own copy.
    pub fn envelope(self) -> (f32, f32, f32, f32) {
        match self {
            SfxKind::Acquire => (0.002, 0.08, 0.0, 0.06),
            SfxKind::Confirm => (0.003, 0.18, 0.2, 0.20),
            SfxKind::Warning => (0.004, 0.25, 0.3, 0.15),
            SfxKind::Danger => (0.001, 0.30, 0.0, 0.20),
            SfxKind::Transition => (0.05, 0.30, 0.3, 0.30),
            SfxKind::Impact => (0.0005, 0.20, 0.0, 0.12),
        }
    }

    /// How long (seconds) the gesture is held at its sustain level *after* attack+decay, before
    /// it is gated off. Short and bounded: an SFX is punctuation, not a pad. Kinds whose sustain
    /// level is zero still get a small hold so the gate-off lands cleanly after the decay.
    pub fn hold_secs(self) -> f32 {
        match self {
            SfxKind::Acquire => 0.02,
            SfxKind::Confirm => 0.10,
            SfxKind::Warning => 0.12,
            SfxKind::Danger => 0.05,
            SfxKind::Transition => 0.16,
            SfxKind::Impact => 0.02,
        }
    }

    /// A conservative upper bound on the audible lifetime (seconds): `attack + decay + hold` (the
    /// point at which the voice is gated off) plus a generous release margin — the exponential
    /// release settles to silence within ~2·release, and we allow 3·release + 50 ms of slack. A
    /// voice that is not silent and idle by this time is a bug (see the synth's SFX regressions).
    pub fn max_lifetime_secs(self) -> f32 {
        let (a, d, _, r) = self.envelope();
        a + d + self.hold_secs() + r * 3.0 + 0.05
    }

    /// [`SfxKind::max_lifetime_secs`] in beats at `tempo_bpm` — how long, musically, the gesture
    /// can sound (the span an owned dissonance must fit inside its owner's window).
    pub fn max_lifetime_beats(self, tempo_bpm: f32) -> f32 {
        self.max_lifetime_secs() * tempo_bpm.max(1.0) / 60.0
    }
}

/// The harmonic **function of a pitch** against the chord sounding beneath it — the vocabulary
/// that lets the engine *justify* a note instead of forbidding it (the jazz principle: there are
/// no forbidden pitches, only unjustified ones). A note consonant with the sounding chord is a
/// `ChordTone`, a licensed `LicensedExtension`, or a `PedalTone`. A non-chord note must carry a
/// concrete path-based justification: it is on its way somewhere (`DiatonicPassing`,
/// `ChromaticPassing`, `Neighbor`, `ChromaticApproach`, `Enclosure`, `SlidePath`), it carries
/// tension with a memory or a future (`Suspension`, `Retardation`, `Anticipation`,
/// `Appoggiatura`), or it is a grammar/world-licensed color (`ModalColor`). A pitched note whose
/// function is left unset (`None`) is *unclassified*, which realization diagnostics count as an
/// unjustified note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PitchFunction {
    /// A member of the onset chord. Temporal carry and destination need separate evidence.
    ChordTone,
    /// A chord extension (7/9/11/13, …) the world/grammar licenses as consonant color.
    LicensedExtension,
    /// A sustained tone held under changing harmony (a pedal point).
    PedalTone,
    /// A weak-beat step between two chord tones along the scale.
    DiatonicPassing,
    /// A weak-beat chromatic step between two structural pitches.
    ChromaticPassing,
    /// A step away from and back to a chord tone (upper or lower neighbor).
    Neighbor,
    /// A step (usually a semitone) that leads directly into a target chord tone.
    ChromaticApproach,
    /// A two-sided approach surrounding a target from above and below.
    Enclosure,
    /// A tone held from the previous harmony that resolves down by step into the new chord.
    Suspension,
    /// Like a suspension, but resolving upward.
    Retardation,
    /// A tone belonging to the *upcoming* chord, sounded just before that chord arrives.
    Anticipation,
    /// A leaped-to non-chord tone on a strong beat that then resolves by step.
    Appoggiatura,
    /// A member of a linear passage from a source pitch to a target (the bassist's slide): every
    /// intermediate pitch inherits its justification from the path, not from the local chord.
    SlidePath,
    /// A blue/modal characteristic tone licensed by the grammar or world.
    ModalColor,
}

impl PitchFunction {
    /// A short lowercase label for structural dumps and diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            PitchFunction::ChordTone => "chord",
            PitchFunction::LicensedExtension => "ext",
            PitchFunction::PedalTone => "pedal",
            PitchFunction::DiatonicPassing => "pass",
            PitchFunction::ChromaticPassing => "chr-pass",
            PitchFunction::Neighbor => "neighbor",
            PitchFunction::ChromaticApproach => "approach",
            PitchFunction::Enclosure => "enclosure",
            PitchFunction::Suspension => "susp",
            PitchFunction::Retardation => "retard",
            PitchFunction::Anticipation => "antic",
            PitchFunction::Appoggiatura => "appog",
            PitchFunction::SlidePath => "slide",
            PitchFunction::ModalColor => "color",
        }
    }

    /// Whether the function denotes a note consonant with the sounding chord (a chord tone,
    /// licensed extension or pedal) — i.e. one that needs no path-based justification.
    pub fn is_consonant(self) -> bool {
        matches!(
            self,
            PitchFunction::ChordTone | PitchFunction::LicensedExtension | PitchFunction::PedalTone
        )
    }
}

/// Where an event came from — the provenance the whole IR carries. Round II makes this rich
/// enough that a cold reader of the dump can answer *what is this piece repeating, what
/// changed here, why is this instrument playing now, and what obligation is in force* — all
/// stamped from the [`super::plan::CompositionPlan`], never hardcoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Provenance {
    /// The real section this event belongs to (Round I hardcoded this to `A` for comp/bass).
    pub section: SectionKind,
    /// Phrase index within the plan's `FormGraph`, if placed by the plan.
    pub phrase: Option<u32>,
    /// Section-family label from the plan: `"A"`, `"A'"`, `"B"`, `"climax"`, …
    pub family: Option<&'static str>,
    /// The coherence anchor this event realizes: `"motif"`, `"groove"`, `"riff"`, …
    pub anchor: Option<&'static str>,
    /// The arrangement role this event was voiced as: `"foreground"`, `"support"`, `"silent"`, …
    pub role_kind: Option<&'static str>,
    /// Stable motif identity index, if melodic.
    pub motif_id: Option<u8>,
    /// The motif transformation-chain label, if melodic.
    pub motif_xform: Option<&'static str>,
    /// The groove variation label, if a rhythm event.
    pub groove_variation: Option<&'static str>,
    /// The phrase's discourse role (Round III/IV): `"establish"`, `"culminate"`, `"answer"`, … —
    /// the trajectory-derived rhetorical job, replacing the old positional section obligation. The
    /// obligation ids this phrase opens/settles are reachable per phrase via `phrase` → the plan's
    /// discourse ledger, so they are not duplicated on every note.
    pub role: Option<&'static str>,
    /// The phrase's permitted closure: `"open"`, `"half"`, `"deferred"`, `"strong"`, ….
    pub closure: Option<&'static str>,
    /// The intent-morphism label that produced it (stamped from the action's cause when the
    /// event realizes a morphism-lifted action).
    pub morphism: Option<&'static str>,
    /// A short human role note: `"comp"`, `"bass"`, `"melody"`, `"sfx"`.
    pub role_note: &'static str,
    /// The exact planned actions this event realizes (Round VIIb). A witness is an event that
    /// carries the action's id — not merely an event that happens to fall in its window.
    pub actions: ActionStamp,
    /// The interaction (call → response) this event belongs to, if any.
    pub interaction: Option<InteractionId>,
    /// The interaction material this event is a projection of, if any.
    pub material: Option<MaterialId>,
    /// The discourse obligation this event helps settle, if any.
    pub obligation: Option<ObligationId>,
}

impl Provenance {
    /// A provenance stamped only with its section; every richer field empty until the plan
    /// fills it.
    pub fn new(section: SectionKind) -> Provenance {
        Provenance {
            section,
            phrase: None,
            family: None,
            anchor: None,
            role_kind: None,
            motif_id: None,
            motif_xform: None,
            groove_variation: None,
            role: None,
            closure: None,
            morphism: None,
            role_note: "",
            actions: ActionStamp::NONE,
            interaction: None,
            material: None,
            obligation: None,
        }
    }

    /// This provenance additionally realizing action `id`.
    #[must_use]
    pub fn realizing(mut self, id: ActionId) -> Provenance {
        self.actions = self.actions.with(id);
        self
    }

    /// This provenance additionally realizing `id`, when there is one.
    #[must_use]
    pub fn realizing_opt(self, id: Option<ActionId>) -> Provenance {
        match id {
            Some(id) => self.realizing(id),
            None => self,
        }
    }
}

/// A pitched note.
#[derive(Debug, Clone, Copy)]
pub struct Note {
    pub start_beat: f64,
    pub dur_beats: f32,
    pub pitch: Midi,
    pub velocity: f32,
    pub role: Role,
    pub prov: Provenance,
    /// The pitch's harmonic function against the chord sounding beneath it, once classified by
    /// the melodic/bass realizer. `None` means unclassified; realization diagnostics count a
    /// pitched note left `None` as unjustified. Chord tones are `Some(PitchFunction::ChordTone)`.
    pub function: Option<PitchFunction>,
}

impl Note {
    /// A note with no pitch-function classification yet (`function: None`). The realizer sets
    /// `function` once it has chosen the pitch against the sounding harmony.
    pub fn new(
        start_beat: f64,
        dur_beats: f32,
        pitch: Midi,
        velocity: f32,
        role: Role,
        prov: Provenance,
    ) -> Note {
        Note {
            start_beat,
            dur_beats,
            pitch,
            velocity,
            role,
            prov,
            function: None,
        }
    }
}

/// A drum hit.
#[derive(Debug, Clone, Copy)]
pub struct DrumHit {
    pub start_beat: f64,
    pub voice: DrumVoice,
    pub velocity: f32,
    pub prov: Provenance,
}

/// An SFX event: a short punctuation gesture pitched in the **local** harmony (Round VIIb). Until
/// VIIb an SFX carried no pitch and the synth drew it from the world scale, so the Warning was a
/// fixed tonic+tritone whatever chord it landed on (a D# over the flagship's Fmaj7 Deflect). Now
/// the composer chooses both pitches against the chord sounding at `start_beat`, labels each, and
/// any pitch that is not a chord tone must be *owned*: bounded by a planned action the same
/// semantic event produced.
#[derive(Debug, Clone, Copy)]
pub struct SfxEvent {
    pub start_beat: f64,
    pub kind: SfxKind,
    pub velocity: f32,
    pub prov: Provenance,
    /// The two pitches the gesture sounds (its FM voice, then its triangle voice).
    /// [`SfxEvent::UNPITCHED`] asks the synth for its world-scale fallback.
    pub pitches: [Midi; 2],
    /// Each pitch's function against the local chord; `None` marks a dissonance, which must be
    /// owned (`owned_by`) for no longer than its owner lasts.
    pub function: [Option<PitchFunction>; 2],
    /// The planned action that owns this gesture's dissonance — produced by the same semantic
    /// event (the Hold a Suspend lifted, the Deflect bound to it) — if the gesture carries one.
    pub owned_by: Option<ActionId>,
    /// How long (beats) the owned dissonance can sound: the SFX voice's bounded lifetime
    /// ([`SfxKind::max_lifetime_beats`]), which must fit inside the owner's window.
    pub dissonance_beats: Option<f32>,
}

impl SfxEvent {
    /// No pitch chosen: the synth falls back to the world scale.
    pub const UNPITCHED: [Midi; 2] = [0, 0];

    /// Whether the composer chose this gesture's pitches (vs the world-scale fallback).
    pub fn is_pitched(&self) -> bool {
        self.pitches.iter().all(|&p| p > 0)
    }
}

/// A complete, deterministic score.
#[derive(Debug, Clone)]
pub struct Score {
    pub notes: Vec<Note>,
    pub drums: Vec<DrumHit>,
    pub sfx: Vec<SfxEvent>,
    pub chords: Vec<ChordSpan>,
    pub sections: Vec<Section>,
    pub tempo_bpm: f32,
    pub beats_per_bar: f64,
    pub total_beats: f64,
    /// How many lead notes the melodic snap pass had to repair (the forward DP produced an
    /// unjustified pitch that was snapped to a chord tone). A generation-side honesty metric — the
    /// residual `unjustified_nonchord_notes` says 0 notes lack a local label, this says how many the
    /// search had to fix. Target 0: choose justified tension, do not manufacture then repair.
    pub melody_repairs: usize,
    /// Lead notes the search left unclassified only because their sustain crossed a harmony
    /// change, re-judged as chord tones after being released there. Not a snap, but not free
    /// either — reported next to `melody_repairs` so "0 repairs" hides nothing.
    pub melody_rejudged: usize,
    /// Round VIII: every explained vertical decision the coupled realization made (a bass unison
    /// note re-pitched to the floor, a material line moved an octave, a candidate refused) — so
    /// "zero unowned collisions" is never a hidden repair either. Empty under the independent control.
    pub vertical_decisions: Vec<super::harmonic_state::VerticalDecision>,
    /// Round VIII: what the joint pad+keys solve did (the union cost along the independent control's
    /// choice vs the joint choice). `None` under the independent control.
    pub support_report: Option<super::support::JointReport>,
    /// Round VIIIb: every edit the surgical pass made to the R7b realization (the note, what it
    /// became, the defect it removed, the rung of the repair ladder, any receipt it surrendered).
    /// Empty under every other coupling.
    pub vertical_repairs: Vec<super::surgical::VerticalRepair>,
    /// Round XIIIb: every edit the sounding-tension gate made (the note, the clash that asked for
    /// it, the action). Empty under every arm but `perform_tension`.
    pub tension_edits: Vec<super::tension::TensionEdit>,
    /// Round XIV: every change the pad made to its Round XII voice path at the source (spacing a
    /// minor 2nd/9th by octaves; sounding the chart's root where the heard band flipped it).
    /// Empty under every arm but `perform_coherent`.
    pub pad_voicing_edits: Vec<super::comp::PadVoicingEdit>,
    /// Round XIV: what each dependent player consumed of another's realization, captured when
    /// it was consumed (see [`Score::stale_hearings`]).
    pub hearings: Vec<Hearing>,
    /// Round XV decisions made inside lead/bass before dependent players hear their output.
    pub expression_decisions: Vec<super::expression::ExpressionDecision>,
}

/// A note as a dependent player consumed it: onset (beats), length (beats), pitch, function.
pub type HeardNote = (f64, f32, Midi, Option<PitchFunction>);

/// Round XIV: what a dependent player (`listener`) consumed of `source`'s realization, captured
/// at the moment it was consumed: the keys, the bass and the drums hear the lead; the drums hear
/// the bass; the Round XIV pad hears the lead, the keys and the bass.
#[derive(Debug, Clone, PartialEq)]
pub struct Hearing {
    pub listener: &'static str,
    pub source: Role,
    pub notes: Vec<HeardNote>,
}

impl Hearing {
    /// `listener` consuming `notes` of `source`.
    pub fn of(listener: &'static str, source: Role, notes: &[Note]) -> Hearing {
        Hearing {
            listener,
            source,
            notes: notes.iter().map(heard_note).collect(),
        }
    }
}

fn heard_note(n: &Note) -> HeardNote {
    (n.start_beat, n.dur_beats, n.pitch, n.function)
}

/// A hearing the final score does not honour: `missing` notes the listener heard are not in the
/// final score, `unheard` final notes of the source were never heard.
#[derive(Debug, Clone, PartialEq)]
pub struct StaleHearing {
    pub listener: &'static str,
    pub source: Role,
    pub missing: Vec<HeardNote>,
    pub unheard: Vec<HeardNote>,
}

impl Score {
    /// The score's fingerprint: FNV-1a over what a listener hears — every note (with its function
    /// and provenance), drum hit, SFX event and chord, plus the melody repair counters. The formula
    /// the R7b/R8 pins were computed with.
    pub fn fingerprint(&self) -> u64 {
        super::song::fnv1a(&format!(
            "{:?}|{:?}|{:?}|{:?}|{}|{}",
            self.notes,
            self.drums,
            self.sfx,
            self.chords,
            self.melody_repairs,
            self.melody_rejudged
        ))
    }

    /// An empty score.
    pub fn new(tempo_bpm: f32, beats_per_bar: f64, total_beats: f64) -> Score {
        Score {
            notes: Vec::new(),
            drums: Vec::new(),
            sfx: Vec::new(),
            chords: Vec::new(),
            sections: Vec::new(),
            tempo_bpm,
            beats_per_bar,
            total_beats,
            melody_repairs: 0,
            melody_rejudged: 0,
            vertical_decisions: Vec::new(),
            support_report: None,
            vertical_repairs: Vec::new(),
            tension_edits: Vec::new(),
            pad_voicing_edits: Vec::new(),
            hearings: Vec::new(),
            expression_decisions: Vec::new(),
        }
    }

    /// Notes of a given role.
    pub fn role_notes(&self, role: Role) -> impl Iterator<Item = &Note> {
        self.notes.iter().filter(move |n| n.role == role)
    }

    /// Round XIV: every hearing whose source notes are not the final score's, compared in order
    /// after the piece's end-clip (nothing starts at or after the end, nothing rings past it),
    /// which every realizer honours. Empty when every dependent player consumed the final
    /// upstream realization: the score is the band that generated itself.
    pub fn stale_hearings(&self) -> Vec<StaleHearing> {
        let end = self.total_beats;
        self.hearings
            .iter()
            .filter_map(|h| {
                let heard: Vec<HeardNote> = h
                    .notes
                    .iter()
                    .filter(|n| n.0 < end - 1e-9)
                    .map(|&(s, d, p, f)| {
                        let d = if s + f64::from(d) > end + 1e-9 {
                            (end - s) as f32
                        } else {
                            d
                        };
                        (s, d, p, f)
                    })
                    .filter(|n| n.1 > 0.0)
                    .collect();
                let fin: Vec<HeardNote> = self.role_notes(h.source).map(heard_note).collect();
                if heard == fin {
                    return None;
                }
                Some(StaleHearing {
                    listener: h.listener,
                    source: h.source,
                    missing: heard.iter().filter(|n| !fin.contains(n)).copied().collect(),
                    unheard: fin.iter().filter(|n| !heard.contains(n)).copied().collect(),
                })
            })
            .collect()
    }

    /// Count of drum hits of a voice.
    pub fn drum_count(&self, voice: DrumVoice) -> usize {
        self.drums.iter().filter(|d| d.voice == voice).count()
    }

    /// The lowest and highest sounding pitch (voice-range check).
    pub fn pitch_range(&self) -> Option<(Midi, Midi)> {
        let mut lo = Midi::MAX;
        let mut hi = Midi::MIN;
        for n in &self.notes {
            lo = lo.min(n.pitch);
            hi = hi.max(n.pitch);
        }
        if self.notes.is_empty() {
            None
        } else {
            Some((lo, hi))
        }
    }

    /// Validate structural invariants: finite times, sane velocities, and the **end of the piece**
    /// — every note, drum stroke, SFX and chord span starts inside `[0, total_beats)`, and every
    /// note and chord ends by `total_beats` (a hair of float slack allowed). A score is exactly as
    /// long as it says it is.
    pub fn validate(&self) -> Result<(), String> {
        const EPS: f64 = 1e-6;
        let end = self.total_beats;
        if !end.is_finite() || end < 0.0 {
            return Err(format!("total_beats {end} is not a length"));
        }
        let starts_inside = |beat: f64| beat >= -EPS && beat < end;
        for n in &self.notes {
            if !n.start_beat.is_finite() || !n.dur_beats.is_finite() {
                return Err("non-finite note time".into());
            }
            if !starts_inside(n.start_beat) {
                return Err(format!("note start {} out of [0,{end})", n.start_beat));
            }
            if n.dur_beats <= 0.0 {
                return Err("non-positive note duration".into());
            }
            if n.start_beat + n.dur_beats as f64 > end + EPS {
                return Err(format!(
                    "note at {} ({} beats) rings past the end ({end})",
                    n.start_beat, n.dur_beats
                ));
            }
            if !(0.0..=1.0).contains(&n.velocity) {
                return Err(format!("note velocity {} out of range", n.velocity));
            }
        }
        for d in &self.drums {
            if !d.start_beat.is_finite() || !starts_inside(d.start_beat) {
                return Err(format!("drum time {} out of [0,{end})", d.start_beat));
            }
            if !(0.0..=1.0).contains(&d.velocity) {
                return Err("drum velocity out of range".into());
            }
        }
        for e in &self.sfx {
            if !e.start_beat.is_finite() || !starts_inside(e.start_beat) {
                return Err(format!("sfx time {} out of [0,{end})", e.start_beat));
            }
            if !(0.0..=1.0).contains(&e.velocity) {
                return Err("sfx velocity out of range".into());
            }
            if e.pitches != SfxEvent::UNPITCHED && !e.pitches.iter().all(|p| (1..=127).contains(p))
            {
                return Err(format!("sfx pitches {:?} out of MIDI range", e.pitches));
            }
            if e.dissonance_beats
                .is_some_and(|d| !d.is_finite() || d <= 0.0)
            {
                return Err("sfx dissonance span is not a positive length".into());
            }
        }
        for c in &self.chords {
            if !c.start_beat.is_finite() || !c.dur_beats.is_finite() {
                return Err("non-finite chord span".into());
            }
            if !starts_inside(c.start_beat) || c.start_beat + c.dur_beats as f64 > end + EPS {
                return Err(format!(
                    "chord span {}+{} outside [0,{end}]",
                    c.start_beat, c.dur_beats
                ));
            }
        }
        Ok(())
    }

    /// A compact multi-line human summary for listening review / diagnostics.
    pub fn summary(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "tempo={:.0}bpm meter={}/4 total_beats={} ({} bars)",
            self.tempo_bpm,
            self.beats_per_bar as u32,
            self.total_beats,
            super::form::bars_spanning(self.total_beats, self.beats_per_bar)
        );
        let _ = writeln!(s, "sections:");
        for sec in &self.sections {
            let _ = writeln!(
                s,
                "  [{:>8}] bars {:>2}..{:<2}  E={:.2} T={:.2} D={:.2}",
                sec.kind.label(),
                sec.start_bar,
                sec.end_bar(),
                sec.energy,
                sec.tension,
                sec.density
            );
        }
        let _ = writeln!(s, "chords ({}):", self.chords.len());
        let mut line = String::from("  ");
        for (i, c) in self.chords.iter().enumerate() {
            let tag = if c.note.is_empty() {
                c.chord.label()
            } else {
                format!("{}[{}]", c.chord.label(), c.note)
            };
            let _ = write!(line, "{tag} ");
            if (i + 1) % 8 == 0 {
                let _ = writeln!(s, "{line}");
                line = String::from("  ");
            }
        }
        if !line.trim().is_empty() {
            let _ = writeln!(s, "{line}");
        }
        let (lo, hi) = self.pitch_range().unwrap_or((0, 0));
        let _ = writeln!(
            s,
            "events: {} notes (pad {}, bass {}, lead {}, keys {}), {} drum hits (K{} S{} H{} O{} C{}), {} sfx; pitch range {}..{}",
            self.notes.len(),
            self.role_notes(Role::Pad).count(),
            self.role_notes(Role::Bass).count(),
            self.role_notes(Role::Lead).count(),
            self.role_notes(Role::Keys).count(),
            self.drums.len(),
            self.drum_count(DrumVoice::Kick),
            self.drum_count(DrumVoice::Snare),
            self.drum_count(DrumVoice::ClosedHat),
            self.drum_count(DrumVoice::OpenHat),
            self.drum_count(DrumVoice::Clap),
            self.sfx.len(),
            lo,
            hi
        );
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_score_validates_and_summarizes() {
        let s = Score::new(120.0, 4.0, 64.0);
        assert!(s.validate().is_ok());
        assert!(s.summary().contains("tempo=120bpm"));
    }

    #[test]
    fn validate_holds_the_score_to_its_exact_end() {
        let note = |start: f64, dur: f32| {
            Note::new(
                start,
                dur,
                60,
                0.5,
                Role::Keys,
                Provenance::new(SectionKind::A),
            )
        };
        let drum = |start: f64| DrumHit {
            start_beat: start,
            voice: DrumVoice::Kick,
            velocity: 0.5,
            prov: Provenance::new(SectionKind::A),
        };
        // A 9-beat score: a note may ring right up to beat 9, but not start there or ring past it.
        let mut s = Score::new(120.0, 4.0, 9.0);
        s.notes.push(note(8.5, 0.5));
        s.drums.push(drum(8.75));
        assert!(s.validate().is_ok(), "{:?}", s.validate());
        let mut late = s.clone();
        late.notes.push(note(9.0, 0.25));
        assert!(
            late.validate().is_err(),
            "a note starting at the end passed"
        );
        let mut long = s.clone();
        long.notes.push(note(8.75, 0.5));
        assert!(
            long.validate().is_err(),
            "a note ringing past the end passed"
        );
        let mut drum_late = s.clone();
        drum_late.drums.push(drum(9.0));
        assert!(drum_late.validate().is_err(), "a stroke at the end passed");
        let sfx = |start: f64| SfxEvent {
            start_beat: start,
            kind: SfxKind::Acquire,
            velocity: 0.5,
            prov: Provenance::new(SectionKind::A),
            pitches: [72, 79],
            function: [Some(PitchFunction::ChordTone); 2],
            owned_by: None,
            dissonance_beats: None,
        };
        let mut sting = s.clone();
        sting.sfx.push(sfx(8.0));
        assert!(sting.validate().is_ok());
        sting.sfx.push(sfx(9.0));
        assert!(sting.validate().is_err(), "a sting at the end passed");
    }

    #[test]
    fn validate_rejects_bad_velocity() {
        let mut s = Score::new(120.0, 4.0, 64.0);
        s.notes.push(Note::new(
            0.0,
            1.0,
            60,
            2.0,
            Role::Lead,
            Provenance::new(SectionKind::A),
        ));
        assert!(s.validate().is_err());
    }
}
