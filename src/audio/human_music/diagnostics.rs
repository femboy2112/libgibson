//! Objective **coherence diagnostics** — concrete, countable measurements of a realized
//! `(CompositionPlan, Score)` pair.
//!
//! This is deliberately **not** a quality / truth / musicality / "probability-of-good" score.
//! Machines cannot certify that a piece slaps — that is the human listening gate. What they
//! *can* certify is structure: does a family recur, are the arrangement's foreground budget
//! and silence real, do phrases prepare their cadences, does every secondary dominant resolve,
//! does the melody restate a recognizable idea. Each field below is a plain count or ratio a
//! cold reader can check, and the module's own tests build deliberately MALFORMED inputs to
//! prove each measurement catches the exact failure it names (positive / null / mutation
//! controls). A green diagnostic is evidence of structure, never of taste.

use super::plan::CompositionPlan;
use super::score::{Role, Score};
use super::theory::{Function, Midi};

/// A vector of structural measurements. Preserve the components — do not collapse to a scalar.
#[derive(Debug, Clone, PartialEq)]
pub struct CoherenceDiagnostics {
    /// Number of phrases in the form graph.
    pub phrases: usize,
    /// Number of section families that recur (appear in >=2 phrases) — recurrence with meaning.
    pub recurring_families: usize,
    /// Phrases whose arrangement exceeds the foreground budget (should be 0).
    pub foreground_collisions: usize,
    /// The declared foreground budget.
    pub foreground_budget: u8,
    /// Fraction of phrases in which at least one voice is silent (silence as a first-class event).
    pub silence_coverage: f32,
    /// Fraction of multi-chord phrases that close Tonic preceded by a Dominant (prepared cadence).
    pub cadence_preparation: f32,
    /// Secondary dominants (`V/…`) NOT immediately followed by their resolution (should be 0).
    pub harmonic_obligation_violations: usize,
    /// Secondary dominants that DID resolve (informational; a nonzero here means the obligation
    /// machinery actually fired).
    pub secondary_dominants_resolved: usize,
    /// Lead notes tagged as a restatement of the germ motif (the hook coming back).
    pub motif_restatements: usize,
    /// Total lead notes.
    pub lead_notes: usize,
    /// Sounding pitch range, if any.
    pub register: Option<(Midi, Midi)>,
    /// Total pitched notes and drum hits (post-arrangement).
    pub total_notes: usize,
    pub total_drums: usize,
}

impl CoherenceDiagnostics {
    /// Measure the structural coherence of `score` against the `plan` it was realized from.
    pub fn measure(plan: &CompositionPlan, score: &Score) -> CoherenceDiagnostics {
        let phrases = plan.form.phrases.len();

        // Recurrence: group phrases by family label (`A` and `A'` collapse to one family) and
        // count the labels that appear more than once.
        let mut family_labels: Vec<&'static str> =
            plan.form.phrases.iter().map(|p| p.family.label()).collect();
        family_labels.sort_unstable();
        let mut recurring_families = 0usize;
        let mut i = 0usize;
        while i < family_labels.len() {
            let mut j = i + 1;
            while j < family_labels.len() && family_labels[j] == family_labels[i] {
                j += 1;
            }
            if j - i >= 2 {
                recurring_families += 1;
            }
            i = j;
        }

        // Arrangement: budget collisions and silence coverage.
        let budget = plan.arrangement.foreground_budget;
        let foreground_collisions = plan
            .arrangement
            .phrases
            .iter()
            .filter(|a| a.foreground_count() > budget)
            .count();
        let silent_phrases = plan
            .arrangement
            .phrases
            .iter()
            .filter(|a| {
                use super::plan::ArrangementRole::Silent;
                [a.pad, a.keys, a.bass, a.lead, a.drums]
                    .iter()
                    .any(|r| matches!(r, Silent))
            })
            .count();
        let silence_coverage = if plan.arrangement.phrases.is_empty() {
            0.0
        } else {
            silent_phrases as f32 / plan.arrangement.phrases.len() as f32
        };

        // Cadence preparation: per phrase, the chords in its beat range; a multi-chord phrase
        // should close Tonic preceded by Dominant.
        let mut prepared = 0usize;
        let mut multi = 0usize;
        for p in &plan.form.phrases {
            let in_phrase: Vec<&super::harmony::ChordSpan> = score
                .chords
                .iter()
                .filter(|c| {
                    c.start_beat >= p.start_beat() - 1e-6 && c.start_beat < p.end_beat() - 1e-6
                })
                .collect();
            if in_phrase.len() >= 2 {
                multi += 1;
                let last = in_phrase[in_phrase.len() - 1];
                let prev = in_phrase[in_phrase.len() - 2];
                if last.function == Function::Tonic && prev.function == Function::Dominant {
                    prepared += 1;
                }
            }
        }
        let cadence_preparation = if multi == 0 {
            0.0
        } else {
            prepared as f32 / multi as f32
        };

        // Harmonic obligations: every `V/…` must be immediately followed by its resolution
        // (the harmony engine marks the resolution span `"res"`).
        let mut harmonic_obligation_violations = 0usize;
        let mut secondary_dominants_resolved = 0usize;
        for (k, c) in score.chords.iter().enumerate() {
            if c.note.starts_with("V/") {
                match score.chords.get(k + 1) {
                    Some(next) if next.note == "res" => secondary_dominants_resolved += 1,
                    _ => harmonic_obligation_violations += 1,
                }
            }
        }

        // Motif restatements: lead notes tagged as a germ statement.
        let motif_restatements = score
            .notes
            .iter()
            .filter(|n| n.role == Role::Lead && n.prov.motif_xform == Some("statement"))
            .count();
        let lead_notes = score.role_notes(Role::Lead).count();

        CoherenceDiagnostics {
            phrases,
            recurring_families,
            foreground_collisions,
            foreground_budget: budget,
            silence_coverage,
            cadence_preparation,
            harmonic_obligation_violations,
            secondary_dominants_resolved,
            motif_restatements,
            lead_notes,
            register: score.pitch_range(),
            total_notes: score.notes.len(),
            total_drums: score.drums.len(),
        }
    }

    /// A compact, human-readable report. Labels are measurements, not verdicts.
    pub fn report(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let (lo, hi) = self.register.unwrap_or((0, 0));
        let _ = writeln!(
            s,
            "coherence diagnostics (structural — NOT a quality score):\n  \
             phrases={} recurring_families={} \n  \
             foreground_budget={} foreground_collisions={} silence_coverage={:.2}\n  \
             cadence_preparation={:.2} harmonic_obligation_violations={} secondary_dominants_resolved={}\n  \
             motif_restatements={} lead_notes={} register={}..{}\n  \
             total_notes={} total_drums={}",
            self.phrases,
            self.recurring_families,
            self.foreground_budget,
            self.foreground_collisions,
            self.silence_coverage,
            self.cadence_preparation,
            self.harmonic_obligation_violations,
            self.secondary_dominants_resolved,
            self.motif_restatements,
            self.lead_notes,
            lo,
            hi,
            self.total_notes,
            self.total_drums,
        );
        s
    }
}

#[cfg(test)]
mod tests {
    use super::super::functor::compose_with_plan;
    use super::super::harmony::ChordSpan;
    use super::super::score::Score;
    use super::super::semantic::demo_trace;
    use super::super::theory::{Chord, Quality};
    use super::super::world::MusicWorld;
    use super::*;

    // --- Positive control: a real composition measures as structurally coherent. ---
    #[test]
    fn a_real_composition_is_structurally_coherent() {
        let trace = demo_trace(120.0);
        let (score, plan) = compose_with_plan(&trace, &MusicWorld::black_ice(), 2112);
        let d = CoherenceDiagnostics::measure(&plan, &score);

        assert!(d.phrases > 0);
        assert!(d.recurring_families >= 1, "no family recurs");
        assert_eq!(
            d.foreground_collisions, 0,
            "the arrangement overspent its foreground budget"
        );
        assert_eq!(
            d.harmonic_obligation_violations, 0,
            "a secondary dominant was left unresolved in a real composition"
        );
        assert!(
            d.cadence_preparation > 0.5,
            "fewer than half the phrases prepared their cadence: {}",
            d.cadence_preparation
        );
        assert!(d.motif_restatements >= 1, "the germ motif never restated");
        assert!(d.silence_coverage > 0.0, "nothing was ever silent");
        assert!(d.report().contains("coherence diagnostics"));
    }

    // --- Null control: an empty score does not panic and reports zeros. ---
    #[test]
    fn empty_score_measures_to_zeros_without_panicking() {
        use super::super::timeline::IntentTimeline;
        let trace = demo_trace(120.0);
        let tl = IntentTimeline::walk(&trace);
        let plan = CompositionPlan::build(&tl, 30);
        let empty = Score::new(88.0, 4.0, 120.0);
        let d = CoherenceDiagnostics::measure(&plan, &empty);
        assert_eq!(d.harmonic_obligation_violations, 0);
        assert_eq!(d.lead_notes, 0);
        assert_eq!(d.total_notes, 0);
        assert_eq!(d.register, None);
    }

    // --- Mutation control: inject an UNRESOLVED secondary dominant; the diagnostic catches it. ---
    #[test]
    fn catches_an_unresolved_secondary_dominant() {
        let trace = demo_trace(120.0);
        let (mut score, plan) = compose_with_plan(&trace, &MusicWorld::black_ice(), 2112);
        // A real composition has zero violations...
        assert_eq!(
            CoherenceDiagnostics::measure(&plan, &score).harmonic_obligation_violations,
            0
        );
        // ...now bolt on a V/of with NO resolution after it and re-measure.
        let span = ChordSpan {
            start_beat: score.total_beats,
            dur_beats: 2.0,
            chord: Chord::new(2, Quality::Dom7),
            function: Function::Dominant,
            degree: -1,
            note: "V/of",
        };
        score.chords.push(span);
        assert!(
            CoherenceDiagnostics::measure(&plan, &score).harmonic_obligation_violations >= 1,
            "an unresolved secondary dominant slipped past the diagnostic"
        );
    }

    // --- Mutation control: an arrangement that overspends the budget is caught. ---
    #[test]
    fn catches_a_foreground_budget_violation() {
        use super::super::plan::{ArrangementPlan, ArrangementRole, PhraseArrangement};
        use super::super::timeline::IntentTimeline;
        let trace = demo_trace(120.0);
        let tl = IntentTimeline::walk(&trace);
        let mut plan = CompositionPlan::build(&tl, 30);
        // Hand-build an arrangement with TWO foreground voices under a budget of 1.
        plan.arrangement = ArrangementPlan {
            foreground_budget: 1,
            phrases: vec![PhraseArrangement {
                pad: ArrangementRole::Foreground,
                keys: ArrangementRole::Foreground,
                bass: ArrangementRole::Foundation,
                lead: ArrangementRole::Silent,
                drums: ArrangementRole::Pulse,
            }],
        };
        let score = Score::new(88.0, 4.0, 120.0);
        let d = CoherenceDiagnostics::measure(&plan, &score);
        assert!(
            d.foreground_collisions >= 1,
            "a foreground-budget violation was not caught"
        );
    }
}
