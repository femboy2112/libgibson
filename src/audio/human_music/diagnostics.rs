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

use super::discourse::{Closure, DiscourseRole};
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

/// **Discourse diagnostics** — concrete measurements of musical DIRECTION (not taste, not quality).
///
/// Where [`CoherenceDiagnostics`] asks "is this recognizably itself?", these ask "does it know
/// where it came from and where it's going?": are the cross-phrase debts paid, does the culmination
/// precede its answer, do the phrase roles move tension the way they claim, is a salient event ever
/// swallowed mid-phrase. Each field is an independent count; [`DiscourseDiagnostics::faults`] sums
/// the defects *only* so the adversarial shuffle probe can assert "strictly worse", and every
/// component stays separately inspectable. This is the layer that catches the Round III failure —
/// grammatically valid phrases in an incoherent order.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscourseDiagnostics {
    pub phrases: usize,
    /// Phrases with no discourse relation to past or future (should be ~0).
    pub orphan_phrases: usize,
    /// Non-deferrable obligations left open at the end — a raised expectation never paid.
    pub abandoned_obligations: usize,
    /// Strong terminal closures that settle nothing and are not the final dissolve — a full stop
    /// that resolves no open expectation.
    pub unearned_strong_closures: usize,
    /// Phrases whose role claims one tension direction while the trajectory moves the other way
    /// (e.g. an "intensify" where tension actually falls).
    pub role_direction_contradictions: usize,
    /// Phrases that swallow a salient semantic event strictly inside them (boundary latency).
    pub swallowed_salient_events: usize,
    /// Distinct closure strengths used (>1 means the closure hierarchy is actually in play).
    pub distinct_closures: usize,
    /// Motif questions posed and answers delivered (from lead provenance).
    pub motif_questions: usize,
    pub motif_answers: usize,
    /// Whether the culmination precedes the answer (the core teleology invariant); `true` when the
    /// piece is intentionally unresolved (no answer at all).
    pub culmination_before_answer: bool,
    /// Average strength of the return to the thesis at Return/Answer phrases, in `[0,1]`.
    pub thesis_return_strength: f32,
}

impl DiscourseDiagnostics {
    /// Measure the discourse direction of `score` against the `plan` it was realized from.
    pub fn measure(plan: &CompositionPlan, score: &Score) -> DiscourseDiagnostics {
        let d = &plan.discourse;
        let goals = &d.goals;
        let n = goals.len();

        let orphan_phrases = goals
            .iter()
            .filter(|g| {
                g.refers_to.is_none()
                    && g.next_goal.is_none()
                    && !matches!(g.role, DiscourseRole::Establish | DiscourseRole::Dissolve)
            })
            .count();

        let abandoned_obligations = d.ledger.abandoned_count();

        let unearned_strong_closures = goals
            .iter()
            .filter(|g| {
                g.closure.is_terminal() && g.pays.is_none() && g.role != DiscourseRole::Dissolve
            })
            .count();

        let mut role_direction_contradictions = 0usize;
        for w in goals.windows(2) {
            let dir = w[1].role.tension_direction();
            let delta = w[1].tension_target - w[0].tension_target;
            if (dir > 0 && delta < -0.05) || (dir < 0 && delta > 0.05) {
                role_direction_contradictions += 1;
            }
        }

        let swallowed_salient_events = plan
            .form
            .phrases
            .iter()
            .filter(|p| p.span.crosses_salient())
            .count();

        let mut seen: Vec<Closure> = Vec::new();
        for g in goals {
            if !seen.contains(&g.closure) {
                seen.push(g.closure);
            }
        }
        let distinct_closures = seen.len();

        let motif_questions = score
            .notes
            .iter()
            .filter(|nt| nt.prov.motif_xform == Some("question"))
            .count();
        let motif_answers = score
            .notes
            .iter()
            .filter(|nt| nt.prov.motif_xform == Some("answer"))
            .count();

        let culmination_before_answer = d.answer.is_none_or(|a| d.culmination < a);

        let returns: Vec<f32> = goals
            .iter()
            .filter(|g| matches!(g.role, DiscourseRole::Return | DiscourseRole::Answer))
            .map(|g| (1.0 - g.thematic_distance).clamp(0.0, 1.0))
            .collect();
        let thesis_return_strength = if returns.is_empty() {
            0.0
        } else {
            returns.iter().sum::<f32>() / returns.len() as f32
        };

        DiscourseDiagnostics {
            phrases: n,
            orphan_phrases,
            abandoned_obligations,
            unearned_strong_closures,
            role_direction_contradictions,
            swallowed_salient_events,
            distinct_closures,
            motif_questions,
            motif_answers,
            culmination_before_answer,
            thesis_return_strength,
        }
    }

    /// The total count of discourse defects — used *only* so the adversarial shuffle probe can
    /// assert a scrambled ordering scores strictly worse. Not a quality score: every component
    /// stays individually inspectable above.
    pub fn faults(&self) -> usize {
        self.orphan_phrases
            + self.abandoned_obligations
            + self.unearned_strong_closures
            + self.role_direction_contradictions
            + self.swallowed_salient_events
            + usize::from(!self.culmination_before_answer)
    }

    /// A human-readable dump for the lab.
    pub fn report(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "discourse diagnostics (direction — NOT a quality score):"
        );
        let _ = writeln!(
            s,
            "  phrases={} orphans={} distinct_closures={}",
            self.phrases, self.orphan_phrases, self.distinct_closures
        );
        let _ = writeln!(
            s,
            "  abandoned_obligations={} unearned_strong_closures={} role_direction_contradictions={}",
            self.abandoned_obligations, self.unearned_strong_closures, self.role_direction_contradictions
        );
        let _ = writeln!(
            s,
            "  swallowed_salient_events={} culmination_before_answer={}",
            self.swallowed_salient_events, self.culmination_before_answer
        );
        let _ = writeln!(
            s,
            "  motif_questions={} motif_answers={} thesis_return_strength={:.2}  faults={}",
            self.motif_questions,
            self.motif_answers,
            self.thesis_return_strength,
            self.faults()
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
        // Round III's closure hierarchy: some phrases close with a strong prepared cadence, but
        // NOT every phrase (Half/Deferred/Open leave expectation open). So this sits strictly
        // between "never resolves" and "a full stop every four bars" — the latter was the R2
        // over-cadencing defect.
        assert!(
            d.cadence_preparation > 0.0 && d.cadence_preparation < 1.0,
            "cadence preparation should be partial (strong cadences exist but not everywhere): {}",
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

    // --- Positive control: a real composition reads as directed discourse. ---
    #[test]
    fn a_real_composition_is_directed() {
        let trace = demo_trace(120.0);
        let (score, plan) = compose_with_plan(&trace, &MusicWorld::black_ice(), 2112);
        let d = DiscourseDiagnostics::measure(&plan, &score);
        assert_eq!(
            d.abandoned_obligations, 0,
            "the resolving arc abandoned a debt"
        );
        assert!(
            d.culmination_before_answer,
            "the culmination did not precede the answer"
        );
        assert_eq!(
            d.orphan_phrases, 0,
            "an orphan phrase with no discourse relation"
        );
        assert_eq!(
            d.swallowed_salient_events, 0,
            "a salient event was swallowed mid-phrase"
        );
        assert!(
            d.distinct_closures >= 3,
            "the closure hierarchy is barely used: {} distinct",
            d.distinct_closures
        );
        assert!(d.report().contains("discourse diagnostics"));
    }

    // --- The adversarial probe: shuffling the sentences must score STRICTLY worse. This is the
    //     exact Round III regression — locally valid phrases in an incoherent order. ---
    #[test]
    fn shuffle_the_sentences_scores_strictly_worse() {
        let trace = demo_trace(120.0);
        let (score, plan) = compose_with_plan(&trace, &MusicWorld::black_ice(), 2112);
        let good = DiscourseDiagnostics::measure(&plan, &score);

        // Reverse the rhetorical order (each phrase keeps its own trajectory), re-measure.
        let mut scrambled = plan.clone();
        scrambled.discourse = plan.discourse.scrambled();
        let bad = DiscourseDiagnostics::measure(&scrambled, &score);

        assert_eq!(
            good.faults(),
            0,
            "the correct arc already has faults: {good:?}"
        );
        assert!(
            bad.faults() > good.faults(),
            "scrambling the discourse did not raise measured faults: good {} vs scrambled {} ({bad:?})",
            good.faults(),
            bad.faults()
        );
        // Concretely: the scramble abandons debts and/or lands the culmination after its answer.
        assert!(
            bad.abandoned_obligations > 0 || !bad.culmination_before_answer,
            "the scramble left the obligation/teleology structure intact"
        );
    }

    // --- Mutation control: obligations that never resolve are flagged as abandoned. ---
    #[test]
    fn catches_abandoned_obligations() {
        let trace = demo_trace(120.0);
        let (score, mut plan) = compose_with_plan(&trace, &MusicWorld::black_ice(), 2112);
        assert_eq!(
            DiscourseDiagnostics::measure(&plan, &score).abandoned_obligations,
            0
        );
        // As if every answer/return never came: un-resolve the whole ledger and re-measure.
        for o in &mut plan.discourse.ledger.obligations {
            o.resolved_by = None;
        }
        assert!(
            DiscourseDiagnostics::measure(&plan, &score).abandoned_obligations >= 1,
            "an abandoned obligation slipped past the diagnostic"
        );
    }

    // --- Null control: a tiny piece measures without panicking. ---
    #[test]
    fn discourse_measures_a_tiny_piece_without_panicking() {
        let trace = demo_trace(16.0);
        let (score, plan) = compose_with_plan(&trace, &MusicWorld::black_ice(), 1);
        let d = DiscourseDiagnostics::measure(&plan, &score);
        assert!(d.phrases >= 1);
        let _ = d.report();
    }
}
