//! **Identity projections** — a role is an instrument; a declared anchor names song identity.
//!
//! ```text
//! declared anchor ──► identity-bearing source material ──► realized evidence
//!   Motif             the lead statements at the song's       the lead events the realizer
//!                     identity theme sites (ThemeSite::        made FROM that material
//!                     is_identity)                            (provenance: its MaterialId)
//!   BassFigure        the bass planner's own structural       the bass events it made as
//!                     line (roots, fifths, pedal, walk,       that line (provenance tags)
//!                     counterline, octaves)
//!   Riff              the identity of the riff lane: the lead's identity statements, else the
//!                     bass figure (a generated song makes no third riff object)
//!   Groove            the kit's own pocket anchors            the strokes the drummer
//!                     (downbeat / mid-bar kick, backbeat)     RECORDED as anchors
//! ```
//!
//! Never promoted into identity: responses and answers, quotes, fills and action figures,
//! unison/interaction figures, connective ornament (chromatic approaches), or any other material
//! that happens to share an instrument — even at the same onset. A cover's lift marks every event
//! it realizes from a pinned identity ([`PINNED_IDENTITY`]); that realized pin is the lane's
//! identity in the cover.

use super::functor::Composition;
use super::ids::MaterialId;
use super::performance::PerformancePlan;
use super::score::{DrumHit, DrumVoice, Note, Role, Score, StrokeOrigin};
use super::song::SongMap;

/// The provenance tags the bass realizer gives its own structural line: the bass figure.
pub const BASS_FIGURE_TAGS: [&str; 6] = ["root", "fifth", "pedal", "walk", "counter", "octave"];

/// The provenance tag a cover's lift gives every event it realizes from a pinned identity.
pub const PINNED_IDENTITY: &str = "cover-identity";

/// The provenance tag the lead realizer gives a statement's own notes.
pub const STATEMENT_NOTE: &str = "melody";

/// The identity-bearing material of one performance: the materials of the lead statements at the
/// song's identity theme sites.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IdentityMaterial {
    pub theme: Vec<MaterialId>,
}

impl IdentityMaterial {
    /// Read from the song's declaration (its identity theme sites) and the performance's plan (the
    /// statement planned at each).
    pub fn of(song: &SongMap, perf: &PerformancePlan) -> Self {
        Self {
            theme: perf
                .statements
                .iter()
                .filter(|st| {
                    song.thematic
                        .sites
                        .iter()
                        .any(|s| s.phrase == st.phrase && s.is_identity())
                })
                .map(|st| st.material)
                .collect(),
        }
    }

    /// Whether realized note `n` carries its lane's song identity.
    pub fn carries(&self, n: &Note) -> bool {
        if n.prov.role_note == PINNED_IDENTITY {
            return matches!(n.role, Role::Lead | Role::Bass);
        }
        match n.role {
            Role::Lead => {
                n.prov.role_note == STATEMENT_NOTE
                    && n.prov.material.is_some_and(|m| self.theme.contains(&m))
            }
            Role::Bass => BASS_FIGURE_TAGS.contains(&n.prov.role_note),
            _ => false,
        }
    }
}

/// The identity-bearing events of `role` in `c` (in score order).
pub fn identity_notes(c: &Composition, role: Role) -> Vec<&Note> {
    let material = IdentityMaterial::of(&c.song, &c.perf);
    c.score
        .role_notes(role)
        .filter(|n| material.carries(n))
        .collect()
}

/// The kit's identity strokes (recorded pocket anchors and pinned strokes, kick and snare) with
/// their recorded origins. `None` when the score records no stroke origins (the historical,
/// unarbitrated drummer, whose strokes keep only their performed float).
pub fn groove_strokes(score: &Score) -> Option<Vec<(&DrumHit, &StrokeOrigin)>> {
    let origins = score.stroke_origins.as_deref()?;
    Some(
        score
            .drums
            .iter()
            .filter(|d| matches!(d.voice, DrumVoice::Kick | DrumVoice::Snare))
            .filter_map(|d| StrokeOrigin::of(origins, d).map(|o| (d, o)))
            .filter(|(_, o)| o.pocket)
            .collect(),
    )
}
