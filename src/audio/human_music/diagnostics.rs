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
use super::motif::{motif_similarity, MotifIdentity};
use super::plan::CompositionPlan;
use super::score::{Role, Score};
use super::theory::{pitch_class, Function, Midi};

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

/// One phrase's realized lead statement — the notes the synth will actually play, fingerprinted.
struct PhraseLead {
    ix: u32,
    role: DiscourseRole,
    is_rupture: bool,
    first: Midi,
    last: Midi,
    identity: MotifIdentity,
}

/// **Realization diagnostics** — measurements of the ACTUAL realized [`Score`], independent of the
/// plan's own declared targets.
///
/// [`DiscourseDiagnostics`] and much of [`CoherenceDiagnostics`] read the plan's declarations (a
/// phrase's `thematic_distance`, its role, its closure). Those certify the plan is internally
/// coherent — but a target is not a receipt (Round IV finding G: the discourse
/// `thesis_return_strength` derived the return strength from the declared `goal.thematic_distance`,
/// auditing a declaration with itself). These fields instead read the lead notes the synth will
/// actually sound: how many have no pitch justification, how far the melody leaps across a
/// non-rupture phrase boundary, how alike consecutive statements are, and how strongly a
/// Restate/Return phrase's realized contour resembles the opening statement — all measured from
/// pitches, an independent witness.
#[derive(Debug, Clone, PartialEq)]
pub struct RealizationDiagnostics {
    /// Phrases that actually sound a lead melody.
    pub lead_phrases: usize,
    /// Pitched notes across ALL audible roles (pad/keys/bass/lead) with no pitch-function
    /// justification — unjustified "wrong notes". Target 0. The listener hears the background too.
    pub unjustified_nonchord_notes: usize,
    /// The same count broken down per audible role, for localizing which voice a wrong note is in.
    pub unjustified_by_role: Vec<(Role, usize)>,
    /// Pitched notes whose nominal sounding body crosses a chord boundary into harmony where the
    /// pitch is not a chord tone (an old-chord tone smearing under new harmony). Score-level; the
    /// synth's release tail is a separate audible-lifetime concern.
    pub cross_boundary_dissonances: usize,
    /// The largest melodic leap (semitones) across a NON-rupture phrase boundary — a teleport if big.
    pub max_boundary_leap: i32,
    /// The mean melodic leap (semitones) across non-rupture phrase boundaries.
    pub mean_boundary_leap: f32,
    /// Mean realized-contour similarity between consecutive lead phrases `[0,1]` — continuity: are
    /// neighbouring statements the same idea developing, or unrelated fragments spliced together?
    pub mean_neighbor_similarity: f32,
    /// Mean realized similarity of Restate/Return phrases to the FIRST stated phrase `[0,1]` — the
    /// thesis actually coming back, measured from the notes, not from a declared distance.
    pub thesis_return_similarity: f32,
}

impl RealizationDiagnostics {
    /// Measure the realized `score` against the `plan` it came from — reading notes, not targets.
    pub fn measure(plan: &CompositionPlan, score: &Score) -> RealizationDiagnostics {
        // Count unjustified notes across ALL pitched roles (not just the lead), tallied per role,
        // and group the LEAD notes by phrase for the continuity fingerprints below.
        let mut by_ix: std::collections::BTreeMap<u32, Vec<(f64, f32, Midi)>> =
            std::collections::BTreeMap::new();
        let mut per_role = [0usize; Role::ALL.len()];
        let mut unjustified_nonchord_notes = 0usize;
        for n in &score.notes {
            if n.function.is_none() {
                unjustified_nonchord_notes += 1;
                if let Some(ri) = Role::ALL.iter().position(|&r| r == n.role) {
                    per_role[ri] += 1;
                }
            }
            if n.role == Role::Lead {
                if let Some(ix) = n.prov.phrase {
                    by_ix
                        .entry(ix)
                        .or_default()
                        .push((n.start_beat, n.dur_beats, n.pitch));
                }
            }
        }
        let unjustified_by_role: Vec<(Role, usize)> =
            Role::ALL.iter().copied().zip(per_role).collect();

        // Cross-boundary dissonance: a pitched note whose body sustains AUDIBLY past the next chord
        // change (more than CROSS_OVERHANG_BEATS) into a chord it is not a tone of — an old-chord
        // tone smearing beneath new harmony. A brief passing-tone tail that resolves at once is not
        // a smear and is not counted; a held overhang is.
        const CROSS_OVERHANG_BEATS: f64 = 0.5;
        let mut cross_boundary_dissonances = 0usize;
        for n in &score.notes {
            let end = n.start_beat + n.dur_beats as f64;
            let next_boundary = score
                .chords
                .iter()
                .map(|c| c.start_beat)
                .filter(|&b| b > n.start_beat + 1e-6)
                .min_by(|a, b| a.total_cmp(b));
            if let Some(b) = next_boundary {
                if end > b + CROSS_OVERHANG_BEATS {
                    if let Some(next_span) = score
                        .chords
                        .iter()
                        .find(|c| (c.start_beat - b).abs() < 1e-6)
                    {
                        if !next_span.chord.contains_pc(pitch_class(n.pitch)) {
                            cross_boundary_dissonances += 1;
                        }
                    }
                }
            }
        }

        // Fingerprint each phrase's realized statement (in beat order).
        let mut phrases: Vec<PhraseLead> = Vec::new();
        for (ix, mut notes) in by_ix {
            notes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            let pitches: Vec<Midi> = notes.iter().map(|t| t.2).collect();
            let rhythms: Vec<f32> = notes.iter().map(|t| t.1).collect();
            if pitches.is_empty() {
                continue;
            }
            phrases.push(PhraseLead {
                ix,
                role: plan.discourse.goal(ix as usize).role,
                is_rupture: plan
                    .form
                    .phrases
                    .get(ix as usize)
                    .is_some_and(|p| p.is_rupture),
                first: *pitches.first().unwrap(),
                last: *pitches.last().unwrap(),
                identity: MotifIdentity::from_sequence(&pitches, &rhythms),
            });
        }
        phrases.sort_by_key(|p| p.ix);

        // Consecutive-phrase continuity: neighbour similarity, and boundary leaps at boundaries the
        // discourse does NOT license as a rupture (a Culminate or is_rupture phrase may leap).
        let mut leaps: Vec<i32> = Vec::new();
        let mut sims: Vec<f32> = Vec::new();
        for w in phrases.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            sims.push(motif_similarity(&a.identity, &b.identity));
            let licensed = a.is_rupture
                || b.is_rupture
                || matches!(a.role, DiscourseRole::Culminate)
                || matches!(b.role, DiscourseRole::Culminate);
            if !licensed {
                leaps.push((b.first - a.last).abs());
            }
        }
        let max_boundary_leap = leaps.iter().copied().max().unwrap_or(0);
        let mean_boundary_leap = if leaps.is_empty() {
            0.0
        } else {
            leaps.iter().sum::<i32>() as f32 / leaps.len() as f32
        };
        let mean_neighbor_similarity = if sims.is_empty() {
            0.0
        } else {
            sims.iter().sum::<f32>() / sims.len() as f32
        };

        // Thesis return: the FIRST realized lead statement is the sounded thesis; how strongly do
        // later Restate/Return statements actually resemble it (from the notes)?
        let thesis_return_similarity = match phrases.first() {
            Some(first) => {
                let returns: Vec<f32> = phrases
                    .iter()
                    .skip(1)
                    .filter(|p| matches!(p.role, DiscourseRole::Restate | DiscourseRole::Return))
                    .map(|p| motif_similarity(&first.identity, &p.identity))
                    .collect();
                if returns.is_empty() {
                    0.0
                } else {
                    returns.iter().sum::<f32>() / returns.len() as f32
                }
            }
            None => 0.0,
        };

        RealizationDiagnostics {
            lead_phrases: phrases.len(),
            unjustified_nonchord_notes,
            unjustified_by_role,
            cross_boundary_dissonances,
            max_boundary_leap,
            mean_boundary_leap,
            mean_neighbor_similarity,
            thesis_return_similarity,
        }
    }

    /// A compact, human-readable report. Measurements of the realized score, not verdicts.
    pub fn report(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "realization diagnostics (the actual score — NOT the plan's targets):"
        );
        let _ = writeln!(
            s,
            "  lead_phrases={} unjustified_nonchord_notes={}",
            self.lead_phrases, self.unjustified_nonchord_notes
        );
        let by_role = self
            .unjustified_by_role
            .iter()
            .map(|(r, c)| format!("{}={}", r.label(), c))
            .collect::<Vec<_>>()
            .join(" ");
        let _ = writeln!(
            s,
            "  unjustified_by_role: {by_role}  cross_boundary_dissonances={}",
            self.cross_boundary_dissonances
        );
        let _ = writeln!(
            s,
            "  boundary_leap max={} mean={:.2}  neighbor_similarity={:.2}  thesis_return_similarity={:.2}",
            self.max_boundary_leap,
            self.mean_boundary_leap,
            self.mean_neighbor_similarity,
            self.thesis_return_similarity,
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

    // --- Realization diagnostics: measured from the ACTUAL notes, not the plan's declarations. ---
    #[test]
    fn realization_diagnostics_read_the_actual_score() {
        let trace = demo_trace(120.0);
        for world in MusicWorld::all() {
            let (score, plan) = compose_with_plan(&trace, &world, 2112);
            let d = RealizationDiagnostics::measure(&plan, &score);
            assert!(d.lead_phrases >= 1, "{}: no lead phrases", world.name);
            // The jazz-principle invariant, verified on the realized score across EVERY audible
            // role (not just the lead) — the listener hears pad, keys and bass too.
            assert_eq!(
                d.unjustified_nonchord_notes, 0,
                "{}: realized score has unjustified notes (by role: {:?})",
                world.name, d.unjustified_by_role
            );
            for (role, count) in &d.unjustified_by_role {
                assert_eq!(
                    *count, 0,
                    "{}: {:?} has unjustified notes",
                    world.name, role
                );
            }
            assert_eq!(
                d.cross_boundary_dissonances, 0,
                "{}: a note's sustained body smears into a chord it is not a tone of",
                world.name
            );
            // Continuity: consecutive statements are transforms of one germ, so they share DNA —
            // realized neighbour similarity is strictly positive (not a splice of unrelated ideas).
            assert!(
                d.mean_neighbor_similarity > 0.0,
                "{}: consecutive lead statements share no realized identity: {}",
                world.name,
                d.mean_neighbor_similarity
            );
            assert!((0.0..=1.0).contains(&d.mean_neighbor_similarity));
            assert!((0.0..=1.0).contains(&d.thesis_return_similarity));
            assert!(d.report().contains("realization diagnostics"));
        }
    }
}
