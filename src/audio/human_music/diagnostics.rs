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

use super::action::{ActionCause, ActionKind, Agent};
use super::context::common_tones;
use super::discourse::{Closure, DiscourseRole};
use super::form::BEATS_PER_BAR;
use super::intent::IntentMorphism;
use super::motif::{motif_similarity, MotifIdentity};
use super::performance::{AccentGrid, PerformancePlan, Transform, STEPS};
use super::plan::CompositionPlan;
use super::score::{DrumVoice, Note, PitchFunction, Role, Score};
use super::theory::{pitch_class, Chord, Function, Midi};
use super::timeline::IntentTimeline;

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
            .filter(|n| n.role == Role::Lead && n.prov.motif_xform == Some("restate"))
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
    /// Obligations never settled that outlived their deadline — a raised expectation never paid.
    pub abandoned_obligations: usize,
    /// Obligations settled, but after their deadline — the expectation was kept waiting.
    pub late_obligations: usize,
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
        let late_obligations = d.ledger.late_count();

        let unearned_strong_closures = goals
            .iter()
            .filter(|g| {
                g.closure.is_terminal()
                    && d.ledger.settled_by(g.phrase_ix).next().is_none()
                    && g.role != DiscourseRole::Dissolve
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
            .filter(|nt| nt.prov.motif_xform == Some("call"))
            .count();
        let motif_answers = score
            .notes
            .iter()
            .filter(|nt| nt.prov.motif_xform == Some("response"))
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
            late_obligations,
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
            + self.late_obligations
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
            "  abandoned_obligations={} late_obligations={} unearned_strong_closures={} role_direction_contradictions={}",
            self.abandoned_obligations,
            self.late_obligations,
            self.unearned_strong_closures,
            self.role_direction_contradictions
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

/// Which scale-degree/tension role a lead pitch plays against the chord sounding beneath it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutlineBucket {
    Root,
    Third,
    Fifth,
    Seventh,
    /// A chord-member color beyond the 7th (6/9/11/13), or a licensed color/tension tone.
    Extension,
    /// A justified non-chord tone in motion (passing/neighbor/approach/enclosure/slide/pedal).
    Connective,
    /// A pitched lead note with no pitch-function justification at all.
    Unjustified,
}

/// Classify a lead pitch against the sounding chord by its position in the chord's stacked
/// intervals (root/3rd/5th/7th by index, higher members as color); a non-member is bucketed by
/// its declared `PitchFunction` into color, connective motion, or (if unset) unjustified.
fn outline_bucket(pitch: Midi, chord: &Chord, func: Option<PitchFunction>) -> OutlineBucket {
    let rel = (pitch - chord.root_pc).rem_euclid(12);
    let ivs = chord.quality.intervals();
    if let Some(idx) = ivs.iter().position(|iv| iv.rem_euclid(12) == rel) {
        match idx {
            0 => OutlineBucket::Root,
            1 => OutlineBucket::Third,
            2 => OutlineBucket::Fifth,
            3 if rel == 10 || rel == 11 => OutlineBucket::Seventh,
            _ => OutlineBucket::Extension,
        }
    } else {
        match func {
            Some(
                PitchFunction::LicensedExtension
                | PitchFunction::ModalColor
                | PitchFunction::Suspension
                | PitchFunction::Retardation
                | PitchFunction::Anticipation
                | PitchFunction::Appoggiatura,
            ) => OutlineBucket::Extension,
            Some(_) => OutlineBucket::Connective,
            None => OutlineBucket::Unjustified,
        }
    }
}

/// A structurally strong beat: on the beat grid and on an even beat-in-bar (downbeat / mid-bar).
fn on_strong_beat(beat: f64, beats_per_bar: f64) -> bool {
    let b = beat.rem_euclid(beats_per_bar.max(1.0));
    let nearest = b.round();
    (b - nearest).abs() < 1e-3 && (nearest as i64).rem_euclid(2) == 0
}

/// Lead-line **outline** diagnostics — the anti-triad-noodling instrument.
///
/// `unjustified_nonchord_notes == 0` proves there are no *wrong* notes; it says nothing about
/// whether the melody is *interesting*. A beginner improviser who only ever plays root/3rd/5th of
/// the one chord they know produces zero wrong notes and a boring tune. This measures, for the LEAD
/// only, the harmonic content and shape of the line: how much is plain triad tones vs guide tones
/// (7ths) and licensed color (extensions), how varied the intervals and pitch classes are, how much
/// it repeats a fixed arpeggio outline, and how much it breathes (syncopation, internal rests). None
/// of this is a quality score — a rich melody may score low on one axis by design — but a line that
/// is ~all root/3/5, no color, one repeated outline, no rests is the exact "major-triad exercise"
/// the ear rejects, and [`LeadOutlineDiagnostics::triad_noodle`] flags precisely that shape.
#[derive(Debug, Clone, PartialEq)]
pub struct LeadOutlineDiagnostics {
    /// Lead notes that sound over a known chord (the denominator for the percentages below).
    pub lead_notes: usize,
    /// How many lead notes the snap pass had to repair during generation (read from the Score).
    /// Target 0 — justified tension chosen in the search, not manufactured then fixed.
    pub repairs_performed: usize,
    /// Fraction of lead notes that are the chord root.
    pub root_pct: f32,
    /// Fraction that are the chord third.
    pub third_pct: f32,
    /// Fraction that are the chord fifth.
    pub fifth_pct: f32,
    /// Fraction that are the chord seventh (a guide tone).
    pub seventh_pct: f32,
    /// Fraction that are extended color (6/9/11/13 chord members, or licensed color/tension tones).
    pub extension_pct: f32,
    /// Fraction that are justified non-chord tones in motion (passing/neighbor/approach/…).
    pub connective_pct: f32,
    /// Fraction with no pitch-function justification at all (should be 0 — mirrors realization).
    pub unjustified_pct: f32,
    /// Among strong-beat lead notes, the fraction that are a 7th or extended color rather than a
    /// plain root/3rd/5th — the single clearest "is this more than a triad exercise" signal.
    pub strong_beat_extension_rate: f32,
    /// Distinct absolute interval sizes divided by the number of melodic intervals `[0,1]`.
    pub interval_diversity: f32,
    /// Shannon entropy of the pitch-class histogram, normalized by `log2(12)` `[0,1]`.
    pub pitch_class_entropy: f32,
    /// Fraction of length-3 interval-contour windows that duplicate an earlier window `[0,1)` —
    /// high means the line keeps replaying one arpeggio shape.
    pub exact_arpeggio_recurrence: f32,
    /// Fraction of lead onsets that fall off the integer beat grid `[0,1]`.
    pub syncopation_rate: f32,
    /// Fraction of consecutive lead-note gaps that are real internal rests `[0,1]`.
    pub internal_rest_rate: f32,
    /// Melodic range in semitones (highest minus lowest lead pitch).
    pub range_semitones: i32,
    /// Composite red flag: the line is ~all root/3/5, essentially no color, and one repeated
    /// arpeggio outline — a harmonically-legal but musically-empty triad exercise.
    pub triad_noodle: bool,
}

impl LeadOutlineDiagnostics {
    /// Measure the LEAD line's outline from the realized `score` alone (no plan needed).
    pub fn measure(score: &Score) -> LeadOutlineDiagnostics {
        let mut lead: Vec<&Note> = score
            .notes
            .iter()
            .filter(|n| n.role == Role::Lead)
            .collect();
        lead.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));

        // The chord sounding at a beat: the last span that has started by then.
        let sounding = |beat: f64| -> Option<&Chord> {
            score
                .chords
                .iter()
                .rev()
                .find(|c| c.start_beat <= beat + 1e-6)
                .map(|c| &c.chord)
        };

        let mut counts = [0usize; 7]; // root, third, fifth, seventh, ext, connective, unjustified
        let mut with_chord = 0usize;
        let mut strong_total = 0usize;
        let mut strong_color = 0usize;
        let pitches: Vec<Midi> = lead.iter().map(|n| n.pitch).collect();
        for n in &lead {
            let Some(chord) = sounding(n.start_beat) else {
                continue;
            };
            with_chord += 1;
            let b = outline_bucket(n.pitch, chord, n.function);
            counts[b as usize] += 1;
            if on_strong_beat(n.start_beat, score.beats_per_bar) {
                strong_total += 1;
                if matches!(b, OutlineBucket::Seventh | OutlineBucket::Extension) {
                    strong_color += 1;
                }
            }
        }

        let denom = with_chord.max(1) as f32;
        let pct = |c: usize| c as f32 / denom;

        // Melodic interval contour (signed), for diversity and arpeggio recurrence.
        let contour: Vec<i32> = pitches.windows(2).map(|w| w[1] - w[0]).collect();
        let interval_diversity = if contour.is_empty() {
            0.0
        } else {
            let distinct: std::collections::BTreeSet<i32> =
                contour.iter().map(|d| d.abs()).collect();
            distinct.len() as f32 / contour.len() as f32
        };
        let exact_arpeggio_recurrence = if contour.len() >= 3 {
            let windows: Vec<[i32; 3]> = contour.windows(3).map(|w| [w[0], w[1], w[2]]).collect();
            let distinct: std::collections::BTreeSet<[i32; 3]> = windows.iter().copied().collect();
            (windows.len() - distinct.len()) as f32 / windows.len() as f32
        } else {
            0.0
        };

        // Pitch-class entropy, normalized to [0,1].
        let mut hist = [0u32; 12];
        for &p in &pitches {
            hist[pitch_class(p) as usize] += 1;
        }
        let total = pitches.len() as f32;
        let pitch_class_entropy = if total > 0.0 {
            let h: f32 = hist
                .iter()
                .filter(|&&c| c > 0)
                .map(|&c| {
                    let pr = c as f32 / total;
                    -pr * pr.log2()
                })
                .sum();
            h / 12f32.log2()
        } else {
            0.0
        };

        let syncopation_rate = if lead.is_empty() {
            0.0
        } else {
            let off = lead
                .iter()
                .filter(|n| {
                    let f = n.start_beat.fract().abs();
                    f > 1e-3 && f < 1.0 - 1e-3
                })
                .count();
            off as f32 / lead.len() as f32
        };
        let internal_rest_rate = if lead.len() < 2 {
            0.0
        } else {
            let rests = lead
                .windows(2)
                .filter(|w| w[1].start_beat - (w[0].start_beat + w[0].dur_beats as f64) > 0.25)
                .count();
            rests as f32 / (lead.len() - 1) as f32
        };
        let range_semitones = match (pitches.iter().min(), pitches.iter().max()) {
            (Some(&lo), Some(&hi)) => hi - lo,
            _ => 0,
        };

        let root_pct = pct(counts[0]);
        let third_pct = pct(counts[1]);
        let fifth_pct = pct(counts[2]);
        let seventh_pct = pct(counts[3]);
        let extension_pct = pct(counts[4]);
        let connective_pct = pct(counts[5]);
        let unjustified_pct = pct(counts[6]);
        let strong_beat_extension_rate = if strong_total == 0 {
            0.0
        } else {
            strong_color as f32 / strong_total as f32
        };

        // The noodle flag: overwhelmingly plain triad tones, no color, one repeated outline.
        let triad_noodle = (root_pct + third_pct + fifth_pct) >= 0.9
            && extension_pct <= 0.05
            && exact_arpeggio_recurrence >= 0.5;

        LeadOutlineDiagnostics {
            lead_notes: with_chord,
            repairs_performed: score.melody_repairs,
            root_pct,
            third_pct,
            fifth_pct,
            seventh_pct,
            extension_pct,
            connective_pct,
            unjustified_pct,
            strong_beat_extension_rate,
            interval_diversity,
            pitch_class_entropy,
            exact_arpeggio_recurrence,
            syncopation_rate,
            internal_rest_rate,
            range_semitones,
            triad_noodle,
        }
    }

    /// A compact, human-readable report. Measurements of the lead line, not a verdict on taste.
    pub fn report(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(s, "lead outline (anti-noodle — NOT a quality score):");
        let _ = writeln!(
            s,
            "  lead_notes={} repairs={} root={:.2} third={:.2} fifth={:.2} seventh={:.2} ext={:.2} connective={:.2} unjustified={:.2}",
            self.lead_notes,
            self.repairs_performed,
            self.root_pct,
            self.third_pct,
            self.fifth_pct,
            self.seventh_pct,
            self.extension_pct,
            self.connective_pct,
            self.unjustified_pct,
        );
        let _ = writeln!(
            s,
            "  strong_beat_ext_rate={:.2} interval_diversity={:.2} pc_entropy={:.2} arp_recurrence={:.2} syncopation={:.2} rests={:.2} range={} triad_noodle={}",
            self.strong_beat_extension_rate,
            self.interval_diversity,
            self.pitch_class_entropy,
            self.exact_arpeggio_recurrence,
            self.syncopation_rate,
            self.internal_rest_rate,
            self.range_semitones,
            self.triad_noodle,
        );
        s
    }
}

// ---------------------------------------------------------------------------------------------
// Round VII: actions, rigidity, contextual harmony. Same contract as everything above — plain
// counts and ratios a cold reader can recompute, never a verdict on whether it grooves.
// ---------------------------------------------------------------------------------------------

/// Every [`ActionKind`], in declaration (= label) order, for fixed-order tallies.
const ACTION_KINDS: [ActionKind; 21] = [
    ActionKind::Pickup,
    ActionKind::Push,
    ActionKind::Pullback,
    ActionKind::Accelerate,
    ActionKind::Hit,
    ActionKind::Break,
    ActionKind::ReEntry,
    ActionKind::Hold,
    ActionKind::Reharmonize,
    ActionKind::Tonicize,
    ActionKind::Deflect,
    ActionKind::Resolve,
    ActionKind::Displace,
    ActionKind::Fragment,
    ActionKind::Sequence,
    ActionKind::Thicken,
    ActionKind::Thin,
    ActionKind::Fill,
    ActionKind::Call,
    ActionKind::Answer,
    ActionKind::Unison,
];

/// Every [`Agent`], in declaration order.
const AGENTS: [Agent; 6] = [
    Agent::Lead,
    Agent::Keys,
    Agent::Bass,
    Agent::Drums,
    Agent::Pad,
    Agent::Ensemble,
];

/// Tally `items` against a fixed `order`, keeping only the entries that occur (so `.len()` of the
/// result is the number of DISTINCT values seen).
fn tally<T: Copy + PartialEq>(
    order: &[T],
    items: &[T],
    label: fn(T) -> &'static str,
) -> Vec<(&'static str, usize)> {
    order
        .iter()
        .map(|&k| (label(k), items.iter().filter(|&&i| i == k).count()))
        .filter(|&(_, c)| c > 0)
        .collect()
}

/// Shannon entropy in bits of a histogram of counts (0 for an empty histogram).
fn entropy_bits(counts: impl IntoIterator<Item = usize>) -> f32 {
    let counts: Vec<usize> = counts.into_iter().filter(|&c| c > 0).collect();
    let total: usize = counts.iter().sum();
    if total == 0 {
        return 0.0;
    }
    // `p * log2(1/p)` rather than `-p * log2(p)`: a one-bucket histogram is +0.0, not -0.0 —
    // a perfectly rigid answer should not print as "-0.00 bits" and look like a sign error.
    counts
        .iter()
        .map(|&c| {
            let p = c as f32 / total as f32;
            p * (1.0 / p).log2()
        })
        .sum()
}

/// `num / den`, 0 when the denominator is empty.
fn ratio(num: usize, den: usize) -> f32 {
    if den == 0 {
        0.0
    } else {
        num as f32 / den as f32
    }
}

/// Sorted onset times with near-duplicates (a voiced stab's simultaneous notes) collapsed.
fn distinct_onsets(mut t: Vec<f64>) -> Vec<f64> {
    t.sort_by(|a, b| a.total_cmp(b));
    t.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    t
}

/// Whether some time in sorted `ts` lies within `tol` of `t`.
fn near_any(ts: &[f64], t: f64, tol: f64) -> bool {
    ts.iter().any(|&x| (x - t).abs() <= tol + 1e-9)
}

/// **Action diagnostics** — does the music *do* things, and do the players *listen*? Measured on
/// the [`PerformancePlan`]'s action plan and interactions, plus the realized [`Score`] for the
/// events that only exist as notes (unisons, counterlines). NOT a quality score.
///
/// Round VI failed as "mood shifts and no action": every semantic verb collapsed into scalar
/// energy/tension. These fields count the verbs directly — which morphisms have an action witness,
/// how long the band goes without doing anything it did not declare as stasis, who answers whom,
/// how late, and where in the bar — so the mood-without-action and clockwork-answer probes can be
/// told apart from the real performance by numbers, not by adjectives.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionDiagnostics {
    /// Non-identity (non-`Prolong`) morphisms applied across all intent transitions.
    pub live_morphisms: usize,
    /// Live morphisms with neither an action witness nor a recorded deferral. Target 0.
    pub unwitnessed_morphisms: usize,
    /// Morphisms the action plan explicitly deferred (with a reason).
    pub deferred_morphisms: usize,
    /// Total actions in the plan (lifted morphisms, gesture verbs, calls and answers).
    pub action_count: usize,
    /// Actions per kind, in [`ActionKind`] declaration order; only kinds that occur are listed.
    pub actions_by_kind: Vec<(&'static str, usize)>,
    /// Actions per bar of the piece.
    pub actions_per_bar: f32,
    /// The longest span of `[0, total)` covered by no action window and no declared stasis, in
    /// beats — accidental inactivity, not an intended still point.
    pub longest_actionless_span_beats: f64,
    /// Call initiators across every interaction (answered or not), in [`Agent`] order; only
    /// agents that occur are listed (so `.len()` is the number of distinct initiators).
    pub initiators: Vec<(&'static str, usize)>,
    /// Responders across interactions with a sounding (non-`Silence`) response, same convention.
    pub responders: Vec<(&'static str, usize)>,
    /// Interactions whose planned response is a deliberate `Silence`.
    pub silent_answers: usize,
    /// Sounding-response latency (beats from the call's end, rounded to 0.5), ascending, with
    /// counts. Negative latency = the answer overlaps the call.
    pub latency_histogram: Vec<(f64, usize)>,
    /// Shannon entropy (bits) of the sixteenth-step-in-bar where sounding responses start. 0 = every
    /// answer on the same step; 4 = uniform over all sixteen.
    pub response_placement_entropy: f32,
    /// Fraction of sounding responses whose `(responder, step)` signature already occurred earlier.
    pub same_slot_response_recurrence: f32,
    /// Sounding responses that overlap their call (latency < 0).
    pub overlapping_responses: usize,
    /// Sounding responses whose window crosses into a new harmony.
    pub chord_crossing_responses: usize,
    /// `unison_actions + realized_unison_points`.
    pub unison_events: usize,
    /// Planned actions of kind `Unison`.
    pub unison_actions: usize,
    /// Realized ensemble coincidences: time points where at least three of {lead, keys, bass,
    /// kick/snare} start within 1/32 beat of each other, inside an ensemble `Hit`/`Unison` window.
    pub realized_unison_points: usize,
    /// Bass notes realized as a counterline (`prov.role_note == "counter"`).
    pub counterline_events: usize,
    /// Of the transitions that apply at least one live morphism, the fraction with at least one
    /// morphism actually witnessed by an action (a deferral is not a witness).
    pub semantic_witness_coverage: f32,
    /// Fraction of backbone gesture slots with at least one action caused by that slot's gesture
    /// or starting inside the slot (0 when the plan has no backbone).
    pub backbone_witness_coverage: f32,
}

impl ActionDiagnostics {
    /// Measure the actions of `perf` against the intent `timeline` it was lifted from, the `plan`
    /// (for its backbone slots) and the realized `score`.
    ///
    /// `plan` is needed because the backbone timeline lives on the [`CompositionPlan`], not on the
    /// performance.
    pub fn measure(
        timeline: &IntentTimeline,
        plan: &CompositionPlan,
        perf: &PerformancePlan,
        score: &Score,
    ) -> ActionDiagnostics {
        let ap = &perf.actions;
        let total = perf.total_beats.max(0.0);

        // --- The morphism ledger: witnessed, deferred, or neither. ---
        let mut live_morphisms = 0usize;
        let mut unwitnessed_morphisms = 0usize;
        let mut live_transitions = 0usize;
        let mut witnessed_transitions = 0usize;
        for (ti, t) in timeline.transitions.iter().enumerate() {
            let mut live_here = false;
            let mut witnessed_here = false;
            for &m in t.applied.iter().filter(|&&m| m != IntentMorphism::Prolong) {
                live_morphisms += 1;
                live_here = true;
                let witnessed = ap.witnesses(ti, m);
                let deferred = ap
                    .deferred
                    .iter()
                    .any(|d| d.transition == ti && d.morphism == m);
                witnessed_here |= witnessed;
                if !witnessed && !deferred {
                    unwitnessed_morphisms += 1;
                }
            }
            live_transitions += usize::from(live_here);
            witnessed_transitions += usize::from(witnessed_here);
        }

        // --- Volume and distribution of action. ---
        let kinds: Vec<ActionKind> = ap.actions.iter().map(|a| a.kind).collect();
        let actions_by_kind = tally(&ACTION_KINDS, &kinds, ActionKind::label);
        let bars = (total / BEATS_PER_BAR).max(1.0);
        let actions_per_bar = ap.actions.len() as f32 / bars as f32;

        // Coverage sweep: action windows and declared stasis both count as "accounted for".
        let mut windows: Vec<(f64, f64)> = ap
            .actions
            .iter()
            .map(|a| (a.start_beat.max(0.0), a.end_beat().min(total)))
            .chain(
                ap.stasis
                    .iter()
                    .map(|s| (s.start_beat.max(0.0), s.end_beat.min(total))),
            )
            .filter(|(s, e)| e > s)
            .collect();
        windows.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut cursor = 0.0_f64;
        let mut longest = 0.0_f64;
        for (s, e) in windows {
            if s > cursor {
                longest = longest.max(s - cursor);
            }
            cursor = cursor.max(e);
        }
        let longest_actionless_span_beats = longest.max(total - cursor);

        // --- Interactions: who calls, who answers, when, and where in the bar. ---
        let initiators_raw: Vec<Agent> =
            perf.interactions.iter().map(|i| i.call.initiator).collect();
        let sounding: Vec<_> = perf
            .interactions
            .iter()
            .filter_map(|i| i.response.as_ref())
            .filter(|r| r.transform != Transform::Silence)
            .collect();
        let silent_answers = perf
            .interactions
            .iter()
            .filter_map(|i| i.response.as_ref())
            .filter(|r| r.transform == Transform::Silence)
            .count();
        let responders_raw: Vec<Agent> = sounding.iter().map(|r| r.responder).collect();

        let mut lat_hist: std::collections::BTreeMap<i64, usize> = Default::default();
        let mut step_hist = [0usize; STEPS];
        let mut seen: Vec<(Agent, usize)> = Vec::new();
        let mut recurring = 0usize;
        for r in &sounding {
            *lat_hist
                .entry((r.latency * 2.0).round() as i64)
                .or_default() += 1;
            let step = AccentGrid::step_of(r.start_beat).1.min(STEPS - 1);
            step_hist[step] += 1;
            let sig = (r.responder, step);
            if seen.contains(&sig) {
                recurring += 1;
            } else {
                seen.push(sig);
            }
        }
        let latency_histogram = lat_hist
            .into_iter()
            .map(|(q, c)| (q as f64 / 2.0, c))
            .collect();

        // --- Unisons: planned, and realized as ensemble coincidences inside a Hit/Unison. ---
        let unison_actions = ap.of_kind(ActionKind::Unison).count();
        const COINCIDE: f64 = 1.0 / 32.0;
        let mut onsets: Vec<(f64, u8)> = score
            .notes
            .iter()
            .filter_map(|n| match n.role {
                Role::Lead => Some((n.start_beat, 0u8)),
                Role::Keys => Some((n.start_beat, 1)),
                Role::Bass => Some((n.start_beat, 2)),
                Role::Pad => None,
            })
            .chain(
                score
                    .drums
                    .iter()
                    .filter(|d| matches!(d.voice, DrumVoice::Kick | DrumVoice::Snare))
                    .map(|d| (d.start_beat, 3u8)),
            )
            .collect();
        onsets.sort_by(|a, b| a.0.total_cmp(&b.0));
        let ensemble_window = |t: f64| {
            ap.actions.iter().any(|a| {
                a.initiator == Agent::Ensemble
                    && matches!(a.kind, ActionKind::Hit | ActionKind::Unison)
                    && t >= a.start_beat - COINCIDE
                    && t < a.end_beat()
            })
        };
        let mut realized_unison_points = 0usize;
        let mut i = 0usize;
        while i < onsets.len() {
            // A greedy cluster: everything starting within 1/32 beat of the cluster's first onset.
            let t0 = onsets[i].0;
            let mut j = i;
            let mut parts = [false; 4];
            while j < onsets.len() && onsets[j].0 <= t0 + COINCIDE + 1e-9 {
                parts[onsets[j].1 as usize] = true;
                j += 1;
            }
            if parts.iter().filter(|&&p| p).count() >= 3 && ensemble_window(t0) {
                realized_unison_points += 1;
            }
            i = j;
        }

        let counterline_events = score
            .notes
            .iter()
            .filter(|n| n.role == Role::Bass && n.prov.role_note == "counter")
            .count();

        // --- Does every spine slot get a verb? ---
        let backbone_witness_coverage = match &plan.backbone {
            Some(bb) if !bb.slots.is_empty() => {
                let covered = bb
                    .slots
                    .iter()
                    .enumerate()
                    .filter(|(si, slot)| {
                        let (s, e) = (slot.start_beat(), slot.end_beat());
                        ap.actions.iter().any(|a| {
                            matches!(a.cause, ActionCause::Gesture { slot, gesture }
                                if slot == *si && gesture == bb.slots[*si].gesture)
                                || (a.start_beat >= s - 1e-6 && a.start_beat < e - 1e-6)
                        })
                    })
                    .count();
                ratio(covered, bb.slots.len())
            }
            _ => 0.0,
        };

        ActionDiagnostics {
            live_morphisms,
            unwitnessed_morphisms,
            deferred_morphisms: ap.deferred.len(),
            action_count: ap.actions.len(),
            actions_by_kind,
            actions_per_bar,
            longest_actionless_span_beats,
            initiators: tally(&AGENTS, &initiators_raw, Agent::label),
            responders: tally(&AGENTS, &responders_raw, Agent::label),
            silent_answers,
            latency_histogram,
            response_placement_entropy: entropy_bits(step_hist),
            same_slot_response_recurrence: ratio(recurring, sounding.len()),
            overlapping_responses: sounding.iter().filter(|r| r.latency < 0.0).count(),
            chord_crossing_responses: sounding.iter().filter(|r| r.crosses_chord).count(),
            unison_events: unison_actions + realized_unison_points,
            unison_actions,
            realized_unison_points,
            counterline_events,
            semantic_witness_coverage: ratio(witnessed_transitions, live_transitions),
            backbone_witness_coverage,
        }
    }

    /// A compact, human-readable report. Counts of verbs, not a verdict on the performance.
    pub fn report(&self) -> String {
        use std::fmt::Write;
        let pairs = |v: &[(&'static str, usize)]| {
            v.iter()
                .map(|(k, c)| format!("{k}={c}"))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let lat = self
            .latency_histogram
            .iter()
            .map(|(l, c)| format!("{l:+.1}:{c}"))
            .collect::<Vec<_>>()
            .join(" ");
        let mut s = String::new();
        let _ = writeln!(
            s,
            "action diagnostics (verbs + listening — NOT a quality score):"
        );
        let _ = writeln!(
            s,
            "  morphisms live={} unwitnessed={} deferred={}  semantic_witness_coverage={:.2} backbone_witness_coverage={:.2}",
            self.live_morphisms,
            self.unwitnessed_morphisms,
            self.deferred_morphisms,
            self.semantic_witness_coverage,
            self.backbone_witness_coverage
        );
        let _ = writeln!(
            s,
            "  actions={} per_bar={:.2} longest_actionless_span={:.2}b  by_kind: {}",
            self.action_count,
            self.actions_per_bar,
            self.longest_actionless_span_beats,
            pairs(&self.actions_by_kind)
        );
        let _ = writeln!(
            s,
            "  initiators: {}  responders: {}  silent_answers={}",
            pairs(&self.initiators),
            pairs(&self.responders),
            self.silent_answers
        );
        let _ = writeln!(
            s,
            "  latency[{lat}]  placement_entropy={:.2}b same_slot_recurrence={:.2} overlapping={} chord_crossing={}",
            self.response_placement_entropy,
            self.same_slot_response_recurrence,
            self.overlapping_responses,
            self.chord_crossing_responses
        );
        let _ = writeln!(
            s,
            "  unison_events={} (actions={} realized_points={})  counterline_events={}",
            self.unison_events,
            self.unison_actions,
            self.realized_unison_points,
            self.counterline_events
        );
        s
    }
}

/// A part of the realized score whose per-bar onset pattern [`onset_vectors`] can extract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnsetPart {
    Keys,
    Bass,
    Lead,
    Pad,
    /// Kick hits with velocity `>= 0.3` (ghosts excluded).
    Kick,
    /// Snare hits with velocity `>= 0.3` (ghosts excluded).
    Snare,
    /// Every drum hit, any voice, any velocity.
    DrumsAll,
}

/// The velocity below which a kick/snare hit counts as a ghost for [`OnsetPart::Kick`]/[`OnsetPart::Snare`].
const GHOST_VELOCITY: f32 = 0.3;

/// Per-bar 16-bit onset vectors for `part`: bit `k` of bar `b` is set iff an onset of that part
/// starts in `[b*bar + k*step - step/2, b*bar + k*step + step/2)`, with `step = bar / 16`. The
/// vector has one entry per bar of `score.total_beats` (rounded up).
pub fn onset_vectors(score: &Score, part: OnsetPart) -> Vec<u16> {
    let bpb = if score.beats_per_bar > 0.0 {
        score.beats_per_bar
    } else {
        BEATS_PER_BAR
    };
    let step = bpb / STEPS as f64;
    let bars = (score.total_beats / bpb).ceil().max(0.0) as usize;
    let mut v = vec![0u16; bars];
    let times: Vec<f64> = match part {
        OnsetPart::Keys | OnsetPart::Bass | OnsetPart::Lead | OnsetPart::Pad => {
            let role = match part {
                OnsetPart::Keys => Role::Keys,
                OnsetPart::Bass => Role::Bass,
                OnsetPart::Lead => Role::Lead,
                _ => Role::Pad,
            };
            score.role_notes(role).map(|n| n.start_beat).collect()
        }
        OnsetPart::Kick | OnsetPart::Snare => {
            let voice = if part == OnsetPart::Kick {
                DrumVoice::Kick
            } else {
                DrumVoice::Snare
            };
            score
                .drums
                .iter()
                .filter(|d| d.voice == voice && d.velocity >= GHOST_VELOCITY)
                .map(|d| d.start_beat)
                .collect()
        }
        OnsetPart::DrumsAll => score.drums.iter().map(|d| d.start_beat).collect(),
    };
    for t in times {
        let idx = ((t + step / 2.0) / step).floor();
        if idx < 0.0 {
            continue;
        }
        let idx = idx as usize;
        let (bar, k) = (idx / STEPS, idx % STEPS);
        if let Some(slot) = v.get_mut(bar) {
            *slot |= 1 << k;
        }
    }
    v
}

/// Recurrence and variety of per-bar patterns: `(fraction of non-empty bars equal to some earlier
/// bar, distinct non-empty patterns)`.
fn pattern_recurrence<T: Copy + Eq + Ord + Default>(bars: &[T]) -> (f32, usize) {
    let mut seen: std::collections::BTreeSet<T> = Default::default();
    let (mut nonempty, mut repeats) = (0usize, 0usize);
    for &b in bars.iter().filter(|&&b| b != T::default()) {
        nonempty += 1;
        if !seen.insert(b) {
            repeats += 1;
        }
    }
    (ratio(repeats, nonempty), seen.len())
}

/// **Rigidity diagnostics** — how often each accompanying part replays the same bar. Measured
/// from the realized [`Score`] alone. NOT a quality score: a groove is *supposed* to recur; these
/// numbers say how much, so a form-locked accompaniment (every bar the same onset vector) can be
/// told from one that breathes with the plan.
///
/// The Round VI rigidity witness is kept explicitly: `keys_fixed_offset_share` counts keys onsets
/// landing at a fixed offset (0.5 or 2.5 beats) into their chord span, the exact stamp the old
/// comping pattern left.
#[derive(Debug, Clone, PartialEq)]
pub struct RigidityDiagnostics {
    /// Fraction of non-empty keys bars whose onset vector equals an earlier bar's.
    pub keys_onset_recurrence: f32,
    /// Distinct non-empty keys onset vectors.
    pub keys_distinct_patterns: usize,
    /// Fraction of non-empty bass bars whose onset vector equals an earlier bar's.
    pub bass_onset_recurrence: f32,
    /// Distinct non-empty bass onset vectors.
    pub bass_distinct_patterns: usize,
    /// Fraction of non-empty drum bars whose (kick, snare) non-ghost vector pair equals an earlier
    /// bar's.
    pub drums_onset_recurrence: f32,
    /// Distinct non-empty (kick, snare) vector pairs.
    pub drums_distinct_patterns: usize,
    /// Fraction of distinct keys onsets sitting exactly 0.5 or 2.5 beats into their chord span.
    pub keys_fixed_offset_share: f32,
    /// Fraction of distinct bass onsets within 1/16 beat of a kick onset (bass slaved to kick).
    pub bass_kick_dependence: f32,
    /// Fraction of distinct kick onsets with a bass onset within 1/16 beat (kick doubled by bass).
    pub kick_bass_coverage: f32,
}

impl RigidityDiagnostics {
    /// Measure the rigidity of the realized `score`.
    pub fn measure(score: &Score) -> RigidityDiagnostics {
        let keys = onset_vectors(score, OnsetPart::Keys);
        let bass = onset_vectors(score, OnsetPart::Bass);
        let kick = onset_vectors(score, OnsetPart::Kick);
        let snare = onset_vectors(score, OnsetPart::Snare);
        let drums: Vec<u32> = kick
            .iter()
            .zip(&snare)
            .map(|(&k, &s)| ((k as u32) << 16) | s as u32)
            .collect();
        let (keys_onset_recurrence, keys_distinct_patterns) = pattern_recurrence(&keys);
        let (bass_onset_recurrence, bass_distinct_patterns) = pattern_recurrence(&bass);
        let (drums_onset_recurrence, drums_distinct_patterns) = pattern_recurrence(&drums);

        let keys_t = distinct_onsets(score.role_notes(Role::Keys).map(|n| n.start_beat).collect());
        let bass_t = distinct_onsets(score.role_notes(Role::Bass).map(|n| n.start_beat).collect());
        let kick_t = distinct_onsets(
            score
                .drums
                .iter()
                .filter(|d| d.voice == DrumVoice::Kick)
                .map(|d| d.start_beat)
                .collect(),
        );

        // The R6 stamp: keys at +0.5 / +2.5 beats into the chord span, whatever the harmony did.
        let (mut with_chord, mut fixed) = (0usize, 0usize);
        for &t in &keys_t {
            let Some(span) = score
                .chords
                .iter()
                .filter(|c| c.start_beat <= t + 1e-6)
                .max_by(|a, b| a.start_beat.total_cmp(&b.start_beat))
            else {
                continue;
            };
            with_chord += 1;
            let off = t - span.start_beat;
            if (off - 0.5).abs() < 0.02 || (off - 2.5).abs() < 0.02 {
                fixed += 1;
            }
        }

        const LOCK: f64 = 1.0 / 16.0;
        let bass_on_kick = bass_t
            .iter()
            .filter(|&&t| near_any(&kick_t, t, LOCK))
            .count();
        let kick_with_bass = kick_t
            .iter()
            .filter(|&&t| near_any(&bass_t, t, LOCK))
            .count();

        RigidityDiagnostics {
            keys_onset_recurrence,
            keys_distinct_patterns,
            bass_onset_recurrence,
            bass_distinct_patterns,
            drums_onset_recurrence,
            drums_distinct_patterns,
            keys_fixed_offset_share: ratio(fixed, with_chord),
            bass_kick_dependence: ratio(bass_on_kick, bass_t.len()),
            kick_bass_coverage: ratio(kick_with_bass, kick_t.len()),
        }
    }

    /// A compact, human-readable report. How often bars repeat, not whether they should.
    pub fn report(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "rigidity diagnostics (bar recurrence — NOT a quality score):"
        );
        let _ = writeln!(
            s,
            "  onset_recurrence keys={:.2} bass={:.2} drums={:.2}  distinct_patterns keys={} bass={} drums={}",
            self.keys_onset_recurrence,
            self.bass_onset_recurrence,
            self.drums_onset_recurrence,
            self.keys_distinct_patterns,
            self.bass_distinct_patterns,
            self.drums_distinct_patterns
        );
        let _ = writeln!(
            s,
            "  keys_fixed_offset_share={:.2}  bass_kick_dependence={:.2} kick_bass_coverage={:.2}",
            self.keys_fixed_offset_share, self.bass_kick_dependence, self.kick_bass_coverage
        );
        s
    }
}

/// Every [`HarmonicRelation`] label, in declaration order (payloads ignored).
const RELATION_LABELS: [&str; 10] = [
    "arrival",
    "prolong",
    "depart",
    "prepare",
    "dominant-to",
    "tonicize",
    "deflected",
    "modal-shift",
    "pedal",
    "chromatic",
];

/// **Harmony-context diagnostics** — is the harmony a sequence of *relations* moving through local
/// palettes, or one key with chords stapled on? Measured on the [`PerformancePlan`]'s
/// [`super::context::HarmonicContext`] timeline and deflect witnesses. NOT a quality score.
#[derive(Debug, Clone, PartialEq)]
pub struct HarmonyContextDiagnostics {
    /// Harmonic contexts analysed.
    pub contexts: usize,
    /// Consecutive contexts whose palette chord-scale (`palette.scale`: tonic + mode) differs. Note
    /// this is the *chord-scale*, so a Dorian ii after an Ionian I counts even inside one key.
    pub region_transitions: usize,
    /// Relations per [`super::context::HarmonicRelation`] label, in declaration order; only labels that occur.
    pub relations: Vec<(&'static str, usize)>,
    /// Fraction of consecutive context pairs where every guide tone of the first is held or moves
    /// to a guide tone of the second by at most 2 semitones (pitch-class distance).
    pub guide_tone_step_motion: f32,
    /// Mean common tones between consecutive chords.
    pub mean_common_tones: f32,
    /// Deflect witnesses (backbone misses) realized.
    pub deflects: usize,
    /// Mean common tones between each deflect's expected and actual arrival.
    pub mean_deflect_common: f32,
    /// Mean root distance (semitones, `0..=6`) expected → actual across deflects.
    pub mean_deflect_root_distance: f32,
    /// Deflects whose pointer concretely pulled toward the expected arrival.
    pub deflects_prepared: usize,
    /// Consecutive contexts whose full palette pitch set differs.
    pub palette_changes: usize,
    /// Fraction of contexts whose palette chord-scale IS the piece's global region scale — high
    /// means one key explains everything.
    pub global_scale_only_share: f32,
}

impl HarmonyContextDiagnostics {
    /// Measure the harmonic-context timeline of `perf`.
    pub fn measure(perf: &PerformancePlan) -> HarmonyContextDiagnostics {
        let cx = &perf.contexts;
        let pairs = cx.len().saturating_sub(1);
        let pc_dist = |a: i32, b: i32| {
            let d = (a - b).rem_euclid(12);
            d.min(12 - d)
        };
        let (mut region_transitions, mut palette_changes, mut stepwise, mut common) =
            (0usize, 0usize, 0usize, 0usize);
        for w in cx.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            region_transitions += usize::from(a.palette.scale != b.palette.scale);
            palette_changes += usize::from(a.palette.all() != b.palette.all());
            let steps = a
                .palette
                .guide_tones
                .iter()
                .all(|&g| b.palette.guide_tones.iter().any(|&h| pc_dist(g, h) <= 2));
            stepwise += usize::from(steps);
            common += common_tones(&a.chord, &b.chord) as usize;
        }
        let labels: Vec<&'static str> = cx.iter().map(|c| c.relation.label()).collect();
        let relations = RELATION_LABELS
            .iter()
            .map(|&l| (l, labels.iter().filter(|&&x| x == l).count()))
            .filter(|&(_, c)| c > 0)
            .collect();
        let dn = perf.deflects.len();
        let mean = |f: &dyn Fn(&super::backbone::DeflectWitness) -> f32| {
            if dn == 0 {
                0.0
            } else {
                perf.deflects.iter().map(f).sum::<f32>() / dn as f32
            }
        };
        HarmonyContextDiagnostics {
            contexts: cx.len(),
            region_transitions,
            relations,
            guide_tone_step_motion: ratio(stepwise, pairs),
            mean_common_tones: ratio(common, pairs),
            deflects: dn,
            mean_deflect_common: mean(&|d| d.common_tones as f32),
            mean_deflect_root_distance: mean(&|d| d.root_distance as f32),
            deflects_prepared: perf.deflects.iter().filter(|d| d.prepared).count(),
            palette_changes,
            global_scale_only_share: ratio(
                cx.iter().filter(|c| c.palette.scale == perf.region).count(),
                cx.len(),
            ),
        }
    }

    /// A compact, human-readable report. Relations counted, not judged.
    pub fn report(&self) -> String {
        use std::fmt::Write;
        let rel = self
            .relations
            .iter()
            .map(|(k, c)| format!("{k}={c}"))
            .collect::<Vec<_>>()
            .join(" ");
        let mut s = String::new();
        let _ = writeln!(
            s,
            "harmony-context diagnostics (relations — NOT a quality score):"
        );
        let _ = writeln!(
            s,
            "  contexts={} region_transitions={} palette_changes={} global_scale_only_share={:.2}",
            self.contexts,
            self.region_transitions,
            self.palette_changes,
            self.global_scale_only_share
        );
        let _ = writeln!(s, "  relations: {rel}");
        let _ = writeln!(
            s,
            "  guide_tone_step_motion={:.2} mean_common_tones={:.2}",
            self.guide_tone_step_motion, self.mean_common_tones
        );
        let _ = writeln!(
            s,
            "  deflects={} prepared={} mean_common={:.2} mean_root_distance={:.2}",
            self.deflects,
            self.deflects_prepared,
            self.mean_deflect_common,
            self.mean_deflect_root_distance
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

    // --- Anti-noodle: LeadOutlineDiagnostics (root/3/5 vs guide-tones + color) ---
    fn c_root_span(total: f64, q: Quality) -> ChordSpan {
        ChordSpan {
            start_beat: 0.0,
            dur_beats: total as f32,
            chord: Chord::new(0, q),
            function: Function::Tonic,
            degree: 0,
            note: "",
        }
    }
    fn lead_note(beat: f64, dur: f32, pitch: i32, f: Option<PitchFunction>) -> Note {
        use super::super::form::SectionKind;
        use super::super::score::Provenance;
        let mut n = Note::new(
            beat,
            dur,
            pitch,
            0.8,
            Role::Lead,
            Provenance::new(SectionKind::A),
        );
        n.function = f;
        n
    }

    #[test]
    fn a_repeated_root_third_fifth_arpeggio_trips_the_noodle_flag() {
        // The exact failure the maintainer hears: "1 3 5 3 | 1 3 5 3", every note harmonically
        // legal, the melody musically empty. No wrong notes != good melody.
        let mut score = Score::new(120.0, 4.0, 16.0);
        score.chords.push(c_root_span(16.0, Quality::Maj));
        let cycle = [60, 64, 67, 64]; // C E G E over C major
        for i in 0..16 {
            score
                .notes
                .push(lead_note(i as f64, 1.0, cycle[i % 4], None));
        }
        let d = LeadOutlineDiagnostics::measure(&score);
        assert!(
            d.triad_noodle,
            "a pure root/3/5 arpeggio must trip the noodle flag: {d:?}"
        );
        assert!(d.root_pct + d.third_pct + d.fifth_pct > 0.99);
        assert_eq!(d.extension_pct, 0.0);
        assert!(d.exact_arpeggio_recurrence >= 0.5);
        assert_eq!(d.syncopation_rate, 0.0);
    }

    #[test]
    fn a_line_with_guide_tones_color_and_rests_does_not_trip_the_flag() {
        // A varied line over Cmaj7: a guide-tone 7th (B), a 9th color (D), off-beats and rests.
        let mut score = Score::new(120.0, 4.0, 16.0);
        score.chords.push(c_root_span(16.0, Quality::Maj7));
        let notes = [
            (0.0, 1.0, 60, Some(PitchFunction::ChordTone)), // C root
            (1.5, 0.5, 71, Some(PitchFunction::ChordTone)), // B 7th (guide tone), off-beat
            (2.0, 0.5, 62, Some(PitchFunction::LicensedExtension)), // D 9th color
            (4.0, 1.0, 67, Some(PitchFunction::ChordTone)), // G 5th, after a rest
            (5.5, 0.5, 69, Some(PitchFunction::Neighbor)),  // A neighbor, off-beat
            (6.0, 1.0, 64, Some(PitchFunction::ChordTone)), // E 3rd
            (8.0, 2.0, 72, Some(PitchFunction::ChordTone)), // C octave, held
            (11.0, 0.5, 65, Some(PitchFunction::DiatonicPassing)), // F passing
        ];
        for (b, dur, p, f) in notes {
            score.notes.push(lead_note(b, dur, p, f));
        }
        let d = LeadOutlineDiagnostics::measure(&score);
        assert!(
            !d.triad_noodle,
            "a guide-tone/color/rest line must NOT trip the noodle flag: {d:?}"
        );
        assert!(d.extension_pct > 0.0, "the 9th should register as color");
        assert!(d.seventh_pct > 0.0, "the B should register as a guide tone");
        assert!(
            d.internal_rest_rate > 0.0,
            "the gaps should register as rests"
        );
    }

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

        // The correct arc owes nothing and pays nothing late. Its one standing fault class is
        // honest: the demo's Answer (phrase 6) now settles its whole cycle, so the two strong
        // Returns after it (phrases 7, 8) discharge nothing. The old zero here was the LIFO ledger
        // letting those Returns "pay" the culmination's and a withhold's debt — bookkeeping, not
        // music.
        assert_eq!(
            (good.abandoned_obligations, good.late_obligations),
            (0, 0),
            "the correct arc left a debt unpaid or late: {good:?}"
        );
        assert_eq!(
            good.faults(),
            good.unearned_strong_closures,
            "the correct arc has faults beyond its post-answer returns: {good:?}"
        );
        assert_eq!(
            good.unearned_strong_closures, 2,
            "the HookArc demo's post-answer returns moved: {good:?}"
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
            o.settlement = None;
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

    // --- Kaleidoscope adversarial probe: coherent statements spliced out of order measure worse. ---
    #[test]
    fn the_kaleidoscope_permutation_measures_worse() {
        // Round III's complaint was "a kaleidoscope of coherent song-fragments spliced together."
        // Take the composed lead, whose statements are DEVELOPED one from the last (and return to the
        // thesis), and PERMUTE those statements across slots — each phrase stays internally valid,
        // only the ORDER changes. A shuffle of good fragments must read as worse: its consecutive
        // statements are less alike (a splice of unrelated ideas) than the composed order's, measured
        // by an octave-invariant contour similarity so an intentionally soaring hook does not fool it.
        // That is the kaleidoscope, measured.
        use super::super::contract::CompositionGrammar;
        use super::super::functor::compose_with_grammar;

        let trace = demo_trace(120.0);
        let world = MusicWorld::black_ice();
        let (score, _plan) =
            compose_with_grammar(&trace, &world, 2112, CompositionGrammar::DeflectedLift);

        // Per-phrase realized lead fingerprint (interval contour + rhythm), in ix order.
        let mut by_ix: std::collections::BTreeMap<u32, Vec<(f64, f32, Midi)>> =
            std::collections::BTreeMap::new();
        for n in &score.notes {
            if n.role == Role::Lead {
                if let Some(ix) = n.prov.phrase {
                    by_ix
                        .entry(ix)
                        .or_default()
                        .push((n.start_beat, n.dur_beats, n.pitch));
                }
            }
        }
        let frags: Vec<MotifIdentity> = by_ix
            .into_values()
            .filter_map(|mut ns| {
                ns.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                let pitches: Vec<Midi> = ns.iter().map(|t| t.2).collect();
                let rhythms: Vec<f32> = ns.iter().map(|t| t.1).collect();
                if pitches.len() < 2 {
                    return None;
                }
                Some(MotifIdentity::from_sequence(&pitches, &rhythms))
            })
            .collect();
        let m = frags.len();
        assert!(m >= 4, "need several lead phrases to permute, got {m}");

        // Mean neighbour motif-similarity for a visiting order. In the COMPOSED order each statement
        // is developed one step from the last (with explicit returns to the thesis), so consecutive
        // statements are recognizably the same idea; a shuffle abuts unrelated fragments. Similarity
        // is octave- and transposition-invariant, so it measures the developing IDEA — not a raw
        // pitch leap that an intentionally soaring hook would inflate.
        let mean_sim = |order: &[usize]| -> f32 {
            let sims: Vec<f32> = order
                .windows(2)
                .map(|w| motif_similarity(&frags[w[0]], &frags[w[1]]))
                .collect();
            sims.iter().sum::<f32>() / sims.len().max(1) as f32
        };

        let composed: Vec<usize> = (0..m).collect();
        let composed_sim = mean_sim(&composed);

        // Deterministic scrambles that genuinely break the composed path-adjacency. (A plain reversal
        // is excluded: motif-similarity is symmetric, so a reversed path has the same neighbour set.)
        let evens_then_odds: Vec<usize> = (0..m)
            .filter(|i| i % 2 == 0)
            .chain((0..m).filter(|i| i % 2 == 1))
            .collect();
        let swap_halves: Vec<usize> = (m / 2..m).chain(0..m / 2).collect();
        let folded: Vec<usize> = (0..m)
            .map(|k| if k % 2 == 0 { k / 2 } else { m - 1 - k / 2 })
            .collect();
        let scrambles = [evens_then_odds, swap_halves, folded];
        let scramble_sim: f32 =
            scrambles.iter().map(|p| mean_sim(p)).sum::<f32>() / scrambles.len() as f32;

        assert!(
            composed_sim > scramble_sim,
            "the composed order ({composed_sim:.2} mean neighbour similarity) did not cohere better \
             than a shuffle of the same fragments ({scramble_sim:.2}) — the continuity metric is \
             not order-sensitive"
        );
    }

    // =========================================================================================
    // Round VII probes — actions, listening, rigidity, contextual harmony. Every composition is
    // the flagship: `deflected_lift_trace(120.0)` + DeflectedLift + seed 2112, varied only by the
    // performance options, so any measured difference is the knob and nothing else.
    // =========================================================================================

    use super::super::contract::CompositionGrammar as R7Grammar;
    use super::super::functor::{compose_full, Composition};
    use super::super::performance::{PerformanceOptions, ResponseMode};
    use super::super::semantic::deflected_lift_trace;

    fn r7(world: &MusicWorld, opts: PerformanceOptions) -> (IntentTimeline, Composition) {
        let trace = deflected_lift_trace(120.0);
        let tl = IntentTimeline::walk(&trace);
        let c = compose_full(&trace, world, 2112, Some(R7Grammar::DeflectedLift), opts);
        (tl, c)
    }

    fn r7_actions(world: &MusicWorld, opts: PerformanceOptions) -> ActionDiagnostics {
        let (tl, c) = r7(world, opts);
        ActionDiagnostics::measure(&tl, &c.plan, &c.perf, &c.score)
    }

    fn actions_off() -> PerformanceOptions {
        PerformanceOptions {
            actions: false,
            ..PerformanceOptions::default()
        }
    }

    fn clockwork() -> PerformanceOptions {
        PerformanceOptions {
            responses: ResponseMode::Clockwork,
            ..PerformanceOptions::default()
        }
    }

    /// The vitals chart: every R7 number for every world and every probe, printed so a failing run
    /// (or `--nocapture`) shows the whole picture instead of one assertion's worth of it.
    #[test]
    fn r7_flagship_values_are_reported() {
        for world in MusicWorld::all() {
            for (name, opts) in [
                ("free", PerformanceOptions::default()),
                ("actions_off", actions_off()),
                ("clockwork", clockwork()),
            ] {
                let (tl, c) = r7(&world, opts);
                let a = ActionDiagnostics::measure(&tl, &c.plan, &c.perf, &c.score);
                let r = RigidityDiagnostics::measure(&c.score);
                let h = HarmonyContextDiagnostics::measure(&c.perf);
                eprintln!(
                    "=== {} / {name} ===\n{}{}{}",
                    world.name,
                    a.report(),
                    r.report(),
                    h.report()
                );
                assert!(a.report().contains("action diagnostics"));
                assert!(r.report().contains("rigidity diagnostics"));
                assert!(h.report().contains("harmony-context diagnostics"));
                // The fixed-order tallies drop nothing: the per-kind / per-relation counts sum to
                // the totals (a label table drifting out of sync with its enum would show here).
                assert_eq!(
                    a.actions_by_kind.iter().map(|x| x.1).sum::<usize>(),
                    a.action_count
                );
                assert_eq!(h.relations.iter().map(|x| x.1).sum::<usize>(), h.contexts);
                assert_eq!(a.initiators.iter().map(|x| x.1).sum::<usize>(), {
                    c.perf.interactions.len()
                });
            }
        }
    }

    // --- (a) flagship: every verb has a witness, and the band actually does things. ---
    #[test]
    fn the_flagship_witnesses_every_morphism_and_acts() {
        for world in MusicWorld::all() {
            let d = r7_actions(&world, PerformanceOptions::default());
            assert_eq!(d.unwitnessed_morphisms, 0, "{}: {d:?}", world.name);
            assert!(d.live_morphisms > 0, "{}: no live morphisms", world.name);
            assert!(
                d.action_count >= 30,
                "{}: only {} actions",
                world.name,
                d.action_count
            );
            assert!(
                d.responders.len() >= 2,
                "{}: one responder only: {:?}",
                world.name,
                d.responders
            );
            assert!(
                d.latency_histogram.len() >= 3,
                "{}: latencies too uniform: {:?}",
                world.name,
                d.latency_histogram
            );
            assert!(
                d.response_placement_entropy > 1.0,
                "{}: answers all land in one place: {:.2} bits",
                world.name,
                d.response_placement_entropy
            );
        }
    }

    // --- (b) ADVERSARIAL mood-without-action: identical scalar mood, measurably no verbs. ---
    #[test]
    fn mood_without_action_keeps_the_mood_and_loses_the_verbs() {
        for world in MusicWorld::all() {
            let (tl_on, on) = r7(&world, PerformanceOptions::default());
            let (tl_off, off) = r7(&world, actions_off());
            // The mood is the same patient: per-phrase energy/tension/density/register goals come
            // from one CompositionPlan, untouched by the performance options.
            let mood = |c: &Composition| -> Vec<[f32; 4]> {
                c.plan
                    .discourse
                    .goals
                    .iter()
                    .map(|g| {
                        [
                            g.energy_target,
                            g.tension_target,
                            g.density_target,
                            g.register_target,
                        ]
                    })
                    .collect()
            };
            assert!(!mood(&on).is_empty());
            assert_eq!(mood(&on), mood(&off), "{}: the mood changed", world.name);

            let a = ActionDiagnostics::measure(&tl_on, &on.plan, &on.perf, &on.score);
            let b = ActionDiagnostics::measure(&tl_off, &off.plan, &off.perf, &off.score);
            assert_eq!(b.action_count, 0, "{}: {b:?}", world.name);
            assert!(a.action_count >= 30, "{}: {}", world.name, a.action_count);
            assert!(
                b.longest_actionless_span_beats > a.longest_actionless_span_beats,
                "{}: off {:.2} vs on {:.2}",
                world.name,
                b.longest_actionless_span_beats,
                a.longest_actionless_span_beats
            );
            assert_eq!(b.semantic_witness_coverage, 0.0, "{}", world.name);
            // Every live morphism is witnessed or DEFERRED with a reason (Round VIIb defers the
            // piece-start Prepare: a pickup into the first downbeat has no time before it).
            assert_eq!(a.unwitnessed_morphisms, 0, "{}", world.name);
            assert!(
                a.semantic_witness_coverage >= 0.85,
                "{}: {:.2}",
                world.name,
                a.semantic_witness_coverage
            );
        }
    }

    // --- (c) ADVERSARIAL clockwork: one responder, one slot, every time — and it measures so. ---
    #[test]
    fn clockwork_answers_measure_rigid_and_free_answers_do_not() {
        for world in MusicWorld::all() {
            let free = r7_actions(&world, PerformanceOptions::default());
            let cw = r7_actions(&world, clockwork());
            assert_eq!(
                cw.responders.len(),
                1,
                "{}: {:?}",
                world.name,
                cw.responders
            );
            assert!(
                cw.response_placement_entropy < 0.2,
                "{}: {:.2}",
                world.name,
                cw.response_placement_entropy
            );
            assert!(
                cw.same_slot_response_recurrence > 0.8,
                "{}: {:.2}",
                world.name,
                cw.same_slot_response_recurrence
            );
            assert!(
                free.responders.len() > cw.responders.len(),
                "{}",
                world.name
            );
            assert!(
                free.response_placement_entropy > cw.response_placement_entropy,
                "{}: free {:.2} vs clockwork {:.2}",
                world.name,
                free.response_placement_entropy,
                cw.response_placement_entropy
            );
            assert!(
                free.same_slot_response_recurrence < cw.same_slot_response_recurrence,
                "{}: free {:.2} vs clockwork {:.2}",
                world.name,
                free.same_slot_response_recurrence,
                cw.same_slot_response_recurrence
            );
        }
    }

    // --- (d) POSITIVE CONTROL: variable answers that are really answers to THIS call. ---
    #[test]
    fn free_answers_vary_in_time_and_quote_the_call() {
        for world in MusicWorld::all() {
            let (tl, c) = r7(&world, PerformanceOptions::default());
            let d = ActionDiagnostics::measure(&tl, &c.plan, &c.perf, &c.score);
            assert!(
                d.responders.len() >= 2,
                "{}: {:?}",
                world.name,
                d.responders
            );
            let sounding: Vec<_> = c
                .perf
                .interactions
                .iter()
                .filter_map(|i| i.response.as_ref().map(|r| (i.call, *r)))
                .filter(|(_, r)| r.transform != Transform::Silence)
                .collect();
            assert!(
                sounding.iter().any(|(_, r)| r.latency < 0.0),
                "{}: no answer ever overlaps its call",
                world.name
            );
            assert!(
                sounding.iter().any(|(_, r)| r.latency >= 1.0),
                "{}: no answer ever waits a beat",
                world.name
            );
            assert!(d.overlapping_responses >= 1);
            assert!(
                c.score.notes.iter().any(|n| n.prov.role_note == "answer"),
                "{}: planned answers never became notes",
                world.name
            );
            // Material relation: some keys/bass answer shares pitch classes with the lead's call.
            let related = sounding.iter().any(|(call, r)| {
                let role = match r.responder {
                    Agent::Keys => Role::Keys,
                    Agent::Bass => Role::Bass,
                    _ => return false,
                };
                if call.initiator != Agent::Lead {
                    return false;
                }
                let call_pcs: std::collections::BTreeSet<i32> = c
                    .score
                    .role_notes(Role::Lead)
                    .filter(|n| {
                        n.start_beat >= call.start_beat - 1e-6
                            && n.start_beat < call.end_beat - 1e-6
                    })
                    .map(|n| pitch_class(n.pitch))
                    .collect();
                c.score
                    .role_notes(role)
                    .filter(|n| {
                        n.prov.role_note == "answer"
                            && n.start_beat >= r.start_beat - 1e-6
                            && n.start_beat < r.start_beat + r.dur_beats - 1e-6
                    })
                    .any(|n| call_pcs.contains(&pitch_class(n.pitch)))
            });
            assert!(
                related,
                "{}: no keys/bass answer shares a pitch class with its call",
                world.name
            );
        }
    }

    #[test]
    fn onset_vectors_bin_on_the_sixteenth_grid() {
        use super::super::form::SectionKind;
        use super::super::score::{DrumHit, Provenance};
        let mut s = Score::new(120.0, 4.0, 8.0);
        let p = Provenance::new(SectionKind::A);
        for t in [0.0, 0.5, 4.0 + 2.5, 3.9] {
            s.notes.push(Note::new(t, 0.25, 60, 0.8, Role::Keys, p));
        }
        s.drums.push(DrumHit {
            start_beat: 1.0,
            voice: DrumVoice::Snare,
            velocity: 0.1, // a ghost: excluded from Snare, kept in DrumsAll
            prov: p,
        });
        let k = onset_vectors(&s, OnsetPart::Keys);
        // 3.9 rounds onto step 16 of bar 0 = step 0 of bar 1.
        assert_eq!(k, vec![(1 << 0) | (1 << 2), (1 << 0) | (1 << 10)]);
        assert_eq!(onset_vectors(&s, OnsetPart::Snare), vec![0, 0]);
        assert_eq!(onset_vectors(&s, OnsetPart::DrumsAll), vec![1 << 4, 0]);
    }

    /// Rebuild `score` with keys, bass and kick/snare stamped from ONE bar's onset vector across the
    /// whole piece (the first non-empty bar of each part — bar 0 is an intro for some parts). Pitches
    /// keep cycling through the sounding chord's tones, so only the RHYTHM is frozen: the metric
    /// must catch the lock without leaning on pitch repetition.
    fn form_locked(score: &Score) -> Score {
        use super::super::score::DrumHit;
        let template = |part: OnsetPart| {
            onset_vectors(score, part)
                .into_iter()
                .find(|&v| v != 0)
                .unwrap_or(1)
        };
        let (tk, tb, tkick, tsn) = (
            template(OnsetPart::Keys),
            template(OnsetPart::Bass),
            template(OnsetPart::Kick),
            template(OnsetPart::Snare),
        );
        let chord_at = |t: f64| {
            score
                .chords
                .iter()
                .filter(|c| c.start_beat <= t + 1e-6)
                .max_by(|a, b| a.start_beat.total_cmp(&b.start_beat))
                .map(|c| c.chord)
                .unwrap_or(Chord::new(0, Quality::Maj))
        };
        let proto = |role: Role| {
            score
                .role_notes(role)
                .next()
                .copied()
                .expect("part present")
        };
        let (pk, pb) = (proto(Role::Keys), proto(Role::Bass));
        let pd = score.drums.first().copied().expect("drums present");
        let mut s = score.clone();
        s.notes
            .retain(|n| !matches!(n.role, Role::Keys | Role::Bass));
        s.drums
            .retain(|d| !matches!(d.voice, DrumVoice::Kick | DrumVoice::Snare));
        let bars = onset_vectors(score, OnsetPart::Keys).len();
        let mut cyc = 0usize;
        for bar in 0..bars {
            for k in 0..STEPS {
                let t = bar as f64 * 4.0 + k as f64 * 0.25;
                if t >= score.total_beats - 1e-6 {
                    break;
                }
                let pcs = chord_at(t).pitch_classes();
                for (mask, proto, base) in [(tk, pk, 60), (tb, pb, 36)] {
                    if mask & (1 << k) != 0 {
                        let mut n = proto;
                        n.start_beat = t;
                        n.dur_beats = 0.25;
                        n.pitch = base + pcs[cyc % pcs.len()];
                        n.function = Some(PitchFunction::ChordTone);
                        s.notes.push(n);
                        cyc += 1;
                    }
                }
                for (mask, voice) in [(tkick, DrumVoice::Kick), (tsn, DrumVoice::Snare)] {
                    if mask & (1 << k) != 0 {
                        s.drums.push(DrumHit {
                            start_beat: t,
                            voice,
                            velocity: 0.8,
                            ..pd
                        });
                    }
                }
            }
        }
        s
    }

    // --- (e) ADVERSARIAL form-lock: every bar the same rhythm must measure as rigid. ---
    #[test]
    fn a_form_locked_accompaniment_measures_rigid() {
        for world in MusicWorld::all() {
            let (_, c) = r7(&world, PerformanceOptions::default());
            let real = RigidityDiagnostics::measure(&c.score);
            let locked = RigidityDiagnostics::measure(&form_locked(&c.score));
            eprintln!(
                "{} real:\n{}{} locked:\n{}",
                world.name,
                real.report(),
                world.name,
                locked.report()
            );
            for (part, lk, rl) in [
                (
                    "keys",
                    locked.keys_onset_recurrence,
                    real.keys_onset_recurrence,
                ),
                (
                    "bass",
                    locked.bass_onset_recurrence,
                    real.bass_onset_recurrence,
                ),
                (
                    "drums",
                    locked.drums_onset_recurrence,
                    real.drums_onset_recurrence,
                ),
            ] {
                assert!(lk > 0.9, "{}: locked {part} recurrence {lk:.2}", world.name);
                assert!(
                    rl < lk - 0.2,
                    "{}: real {part} recurrence {rl:.2} is within 0.2 of the form-locked {lk:.2}",
                    world.name
                );
            }
            assert_eq!(locked.keys_distinct_patterns, 1);
            assert_eq!(locked.bass_distinct_patterns, 1);
            assert_eq!(locked.drums_distinct_patterns, 1);
        }
    }

    // --- (f) contextual harmony: the misses are real, prepared, and keep common tones. ---
    #[test]
    fn the_flagship_harmony_deflects_with_prepared_common_tone_misses() {
        for world in MusicWorld::all() {
            let (_, c) = r7(&world, PerformanceOptions::default());
            let h = HarmonyContextDiagnostics::measure(&c.perf);
            assert!(h.deflects >= 3, "{}: {h:?}", world.name);
            assert_eq!(h.deflects_prepared, h.deflects, "{}: {h:?}", world.name);
            assert!(h.mean_deflect_common >= 1.0, "{}: {h:?}", world.name);
        }
    }
}
