//! Authored conversational space, separate from the notes that physically sound.
//!
//! A grace can move an attack without giving its original rhythmic slot to another player.
//! These reservations are captured from the responsible source BEFORE expression. They contain
//! no pretend sounding notes, and must never be passed to an acoustic hearing or harmony audit.
use super::expression::annotate;
use super::ids::{ActionStamp, MaterialId};
use super::motif::{melodic_events, LineStyle};
use super::performance::{AccentGrid, PerformancePlan, STEP_BEATS};
use super::score::{Note, Role};

/// An authored pulse, not a synth event. Expression does not move this reservation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RhythmReservation {
    pub beat: f64,
    pub structural: bool,
    pub material: Option<MaterialId>,
    pub actions: ActionStamp,
}

/// Why a span belongs to a player even when its instrument is silent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnershipKind {
    Phrase,
    InternalRest,
    Action,
}

/// A half-open authored window. An internal rest retains its phrase's ownership.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OwnershipSpan {
    pub start: f64,
    pub end: f64,
    pub material: Option<MaterialId>,
    pub kind: OwnershipKind,
}

impl OwnershipSpan {
    fn covers(&self, beat: f64) -> bool {
        beat >= self.start - 1e-6 && beat < self.end - 1e-6
    }
}

/// Conversational status, not an acoustic loudness judgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opportunity {
    /// The source reserved an attack here, even if expression moved or omitted it.
    ReservedAttack,
    /// Authored silence inside an unfinished thought; not an invitation to take over.
    InternalRest,
    /// Ordinary accompaniment may coexist, but this is not a new solo/fill opening.
    PhraseOwned,
    /// Outside the source's owned phrase/action: an actual handoff opportunity.
    Yield,
}

/// Immutable intent accompanying a final acoustic line.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthoredOccupancy {
    pub role: Role,
    pub rhythm: Vec<RhythmReservation>,
    pub spans: Vec<OwnershipSpan>,
}

impl AuthoredOccupancy {
    /// Capture the actual temporal lead source, before retiming, gating, or omission.
    pub fn from_lead(perf: &PerformancePlan, source: &[Note]) -> Self {
        Self::from_role(perf, source, Role::Lead)
    }

    /// The bass source already inherits the final lead unison. Recover that shared gesture's
    /// authored pulse for the ordinary drum pocket; the explicit unison accents still consume
    /// the final acoustic lead. Only semantic reservations change, never sounding notes.
    pub fn from_bass(
        perf: &PerformancePlan,
        source: &[Note],
        lead_decisions: &[super::expression::ExpressionDecision],
    ) -> Self {
        let mut intent = Self::from_role(perf, source, Role::Bass);
        for (slot, note) in intent
            .rhythm
            .iter_mut()
            .zip(source.iter().filter(|n| n.role == Role::Bass))
        {
            if note.prov.role_note != "unison" {
                continue;
            }
            let mut pitch = note.pitch;
            for decision in lead_decisions.iter().rev() {
                if let Some(after) = decision.after {
                    if after.role == Role::Lead
                        && (after.start_beat - slot.beat).abs() < 1e-6
                        && (after.pitch - pitch).rem_euclid(12) == 0
                    {
                        slot.beat = decision.before.note.start_beat;
                        pitch += decision.before.note.pitch - after.pitch;
                    }
                }
            }
        }
        intent
    }

    /// Capture a player's own source. Drum interlock can read these semantic pulse slots while
    /// its acoustic hearing continues to record only the final bass notes.
    pub fn from_role(perf: &PerformancePlan, source: &[Note], role: Role) -> Self {
        let source: Vec<_> = source.iter().filter(|n| n.role == role).copied().collect();
        let rhythm = annotate(perf, &source)
            .iter()
            .map(|e| RhythmReservation {
                beat: e.note.start_beat,
                structural: e.structural,
                material: e.note.prov.material,
                actions: e.note.prov.actions,
            })
            .collect();
        let mut spans = Vec::new();
        if role == Role::Lead {
            for statement in &perf.statements {
                // A planned statement that never reached its source is not an imaginary owner.
                if !source
                    .iter()
                    .any(|n| n.prov.material == Some(statement.material))
                {
                    continue;
                }
                spans.push(OwnershipSpan {
                    start: statement.start_beat,
                    end: statement.end_beat(),
                    material: Some(statement.material),
                    kind: OwnershipKind::Phrase,
                });
                let slots = melodic_events(
                    &statement.motif,
                    statement.start_beat,
                    &LineStyle::for_language(&perf.language),
                );
                for (i, slot) in slots.iter().enumerate() {
                    let start = statement.start_beat + slot.onset;
                    let end = slots.get(i + 1).map_or(statement.end_beat(), |next| {
                        statement.start_beat + next.onset
                    });
                    // Full rests and shortened authored breaths both retain ownership. The
                    // latter begin at the ORIGINAL gate, never at an expressive note-off.
                    let rest_start = if slot.rest {
                        start
                    } else {
                        start + f64::from(slot.dur)
                    };
                    if rest_start < end - 1e-6 {
                        spans.push(OwnershipSpan {
                            start: rest_start,
                            end,
                            material: Some(statement.material),
                            kind: OwnershipKind::InternalRest,
                        });
                    }
                }
            }
        }
        for material in &perf.materials {
            if source.iter().any(|n| n.prov.material == Some(material.id))
                && !spans.iter().any(|s| s.material == Some(material.id))
            {
                spans.push(OwnershipSpan {
                    start: material.start_beat,
                    end: material.start_beat + material.length(),
                    material: Some(material.id),
                    kind: OwnershipKind::Phrase,
                });
            }
        }
        for action in &perf.actions.actions {
            if source.iter().any(|n| n.prov.actions.has(action.id)) {
                spans.push(OwnershipSpan {
                    start: action.start_beat,
                    end: action.end_beat(),
                    material: None,
                    kind: OwnershipKind::Action,
                });
            }
        }
        Self {
            role,
            rhythm,
            spans,
        }
    }

    /// Preserve the original rhythm's attack clearance, independently of expression.
    pub fn reserved_attack_near(&self, beat: f64) -> bool {
        self.rhythm
            .iter()
            .any(|r| (r.beat - beat).abs() < STEP_BEATS * 0.99)
    }

    pub fn opportunity(&self, beat: f64) -> Opportunity {
        if self.reserved_attack_near(beat) {
            Opportunity::ReservedAttack
        } else if self
            .spans
            .iter()
            .any(|s| s.kind == OwnershipKind::InternalRest && s.covers(beat))
        {
            Opportunity::InternalRest
        } else if self.spans.iter().any(|s| s.covers(beat)) {
            Opportunity::PhraseOwned
        } else {
            Opportunity::Yield
        }
    }

    /// A fill/solo opening requires real relinquished ownership. Accompaniment has a weaker
    /// contract: it can coexist inside a phrase at its original unreserved opportunities.
    pub fn yields_at(&self, beat: f64) -> bool {
        self.opportunity(beat) == Opportunity::Yield
    }

    /// An accompaniment attack must pass BOTH authored intent and final acoustic onset
    /// clearance. Existing comping may coexist in an owned phrase or its authored breath; a
    /// lead rest does not order the entire band silent. It still grants no new fill/solo opening
    /// (`yields_at` remains false). Expression creates no new attack opportunities. This does not
    /// substitute for the consumer's harmonic audit.
    pub fn allows_comp_at(&self, beat: f64, final_notes: &[Note]) -> bool {
        self.opportunity(beat) != Opportunity::ReservedAttack
            && !final_notes
                .iter()
                .any(|n| n.role == self.role && (n.start_beat - beat).abs() < STEP_BEATS * 0.99)
    }

    /// Authored pulse grid for groove decisions. These are semantic slots, NOT heard notes.
    /// Consumers can explicitly exclude optional ornaments rather than rounding grace attacks
    /// onto a different grid cell and mistaking that for a new bass rhythm.
    pub fn rhythm_steps(&self, bar: u32, structural_only: bool) -> Vec<usize> {
        let start = AccentGrid::beat_of(bar, 0);
        let mut steps: Vec<_> = self
            .rhythm
            .iter()
            .filter(|r| {
                r.beat >= start - 1e-6
                    && r.beat < start + 4.0 - 1e-6
                    && (!structural_only || r.structural)
            })
            .map(|r| AccentGrid::step_of(r.beat).1)
            .collect();
        steps.sort_unstable();
        steps.dedup();
        steps
    }
}
