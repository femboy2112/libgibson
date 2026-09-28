//! The **Score IR**: an inspectable, deterministic intermediate representation. Notes,
//! drum hits and SFX events all carry *provenance* (which section, which motif, which
//! semantic role / intent morphism produced them), which makes the score debuggable,
//! replayable and testable — and lets a dump explain *why* every event exists.

use super::form::{Section, SectionKind};
use super::harmony::ChordSpan;
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
    /// A member of the sounding chord — consonant, needs no further justification.
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
    /// The intent-morphism label that produced it.
    pub morphism: Option<&'static str>,
    /// A short human role note: `"comp"`, `"bass"`, `"melody"`, `"sfx"`.
    pub role_note: &'static str,
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

/// An SFX event.
#[derive(Debug, Clone, Copy)]
pub struct SfxEvent {
    pub start_beat: f64,
    pub kind: SfxKind,
    pub velocity: f32,
    pub prov: Provenance,
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
    /// residual `unjustified_nonchord_notes` says 0 wrong notes SURVIVE, this says how many the
    /// search had to fix. Target 0: choose justified tension, do not manufacture then repair.
    pub melody_repairs: usize,
}

impl Score {
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
        }
    }

    /// Notes of a given role.
    pub fn role_notes(&self, role: Role) -> impl Iterator<Item = &Note> {
        self.notes.iter().filter(move |n| n.role == role)
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

    /// Validate structural invariants: finite, in-range times, sane velocities, sorted-able.
    pub fn validate(&self) -> Result<(), String> {
        for n in &self.notes {
            if !n.start_beat.is_finite() || !n.dur_beats.is_finite() {
                return Err("non-finite note time".into());
            }
            if n.start_beat < -1e-6 || n.start_beat > self.total_beats + 1e-6 {
                return Err(format!(
                    "note start {} out of [0,{}]",
                    n.start_beat, self.total_beats
                ));
            }
            if n.dur_beats <= 0.0 {
                return Err("non-positive note duration".into());
            }
            if !(0.0..=1.0).contains(&n.velocity) {
                return Err(format!("note velocity {} out of range", n.velocity));
            }
        }
        for d in &self.drums {
            if !d.start_beat.is_finite() || d.start_beat < -1e-6 {
                return Err("bad drum time".into());
            }
            if !(0.0..=1.0).contains(&d.velocity) {
                return Err("drum velocity out of range".into());
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
            "tempo={:.0}bpm meter={}/4 total_beats={:.0} ({} bars)",
            self.tempo_bpm,
            self.beats_per_bar as u32,
            self.total_beats,
            (self.total_beats / self.beats_per_bar).round() as u32
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
