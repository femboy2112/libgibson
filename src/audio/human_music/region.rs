//! **Harmonic edits and tonal regions** — the actions that change the harmony itself
//! (Round VII; split out of [`super::performance`] in Round VIIb).

use super::action::{ActionKind, ActionPlan};
use super::harmony::ChordSpan;
use super::ids::ActionId;
use super::theory::{Chord, Quality, Scale};

/// A harmonic recolouring or tonicization applied by an action.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HarmonicEdit {
    pub action: ActionId,
    pub at_beat: f64,
    pub before: Chord,
    pub after: Chord,
}

/// Recolour/tonicize the harmony where a Reharmonize/Tonicize action fires.
pub(super) fn apply_harmonic_actions(
    chords: &mut Vec<ChordSpan>,
    actions: &ActionPlan,
    region: &Scale,
) -> Vec<HarmonicEdit> {
    let mut edits = Vec::new();
    for a in &actions.actions {
        match a.kind {
            ActionKind::Reharmonize => {
                let Some(ix) = chords
                    .iter()
                    .rposition(|c| c.start_beat <= a.start_beat + 1e-6)
                else {
                    continue;
                };
                let before = chords[ix].chord;
                let q = match before.quality {
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
                if q != before.quality {
                    let after = Chord::new(before.root_pc, q);
                    chords[ix].chord = after;
                    edits.push(HarmonicEdit {
                        action: a.id,
                        at_beat: chords[ix].start_beat,
                        before,
                        after,
                    });
                }
            }
            ActionKind::Tonicize => {
                // Tonicize the harmony arriving at (or after) the action: its preceding span's
                // second half becomes the applied dominant — a real local region change.
                let Some(ix) = chords
                    .iter()
                    .position(|c| c.start_beat >= a.start_beat - 1e-6)
                else {
                    continue;
                };
                if ix == 0 {
                    continue;
                }
                let target = chords[ix].chord;
                if target.root_pc == region.tonic_pc {
                    continue; // a dominant to home is an arrival, not a tonicization
                }
                let prev = chords[ix - 1];
                if prev.dur_beats < 2.0 - 1e-6 {
                    continue;
                }
                let half = (prev.dur_beats / 2.0).max(1.0);
                let dom = Chord::new((target.root_pc + 7).rem_euclid(12), Quality::Dom7);
                chords[ix - 1].dur_beats = prev.dur_beats - half;
                chords.insert(
                    ix,
                    ChordSpan {
                        start_beat: prev.start_beat + (prev.dur_beats - half) as f64,
                        dur_beats: half,
                        chord: dom,
                        function: super::theory::Function::Dominant,
                        degree: -1,
                        note: prev.note,
                    },
                );
                edits.push(HarmonicEdit {
                    action: a.id,
                    at_beat: prev.start_beat + (prev.dur_beats - half) as f64,
                    before: prev.chord,
                    after: dom,
                });
            }
            _ => {}
        }
    }
    edits
}
