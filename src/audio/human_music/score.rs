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
    /// The phrase/harmonic obligation this event serves: `"arrival"`, `"lift"`, `"release"`, …
    pub obligation: Option<&'static str>,
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
            obligation: None,
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
        s.notes.push(Note {
            start_beat: 0.0,
            dur_beats: 1.0,
            pitch: 60,
            velocity: 2.0,
            role: Role::Lead,
            prov: Provenance::new(SectionKind::A),
        });
        assert!(s.validate().is_err());
    }
}
