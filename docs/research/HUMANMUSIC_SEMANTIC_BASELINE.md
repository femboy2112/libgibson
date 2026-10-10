# Semantic resurrection: legacy witness boundary

Audited source base: `fbdbffcee3f77dab3cef0234666192cadc7da370` on
`feat/v0.5-humanmusic-beefup` (PR #87). This report describes the unchanged legacy
mechanisms at that base, not every mechanism subsequently added by the implementation
branch. PR #94's `docs/HUMAN_MUSIC_SEMANTIC_IDENTIFIABILITY_PLAN.md` was read from
`origin/research/humanmusic-semantic-identifiability-2026-10-09`. Its diagnosis is
confirmed at the boundaries below, with one more unit-related failure identified.

The maintainer reports improved coherence and continued ambiguous musical meaning.
That is listening evidence supplied by the maintainer. This audit has no new human
listening verdict and does not infer aesthetic failure from a symbolic collision alone.

## Actual arrows and their scope

| Stage | Concrete input/output | Load-bearing behavior | Boundary |
| --- | --- | --- | --- |
| `SemanticTrace` | Timed presentation states and event kinds | Tone, pressure, elevation and transition kind enter the intent walk | No specific musical question/answer source identity is requested here |
| `IntentTimeline::walk` | Trace to ordered transitions | Applied morphisms, causal before/after intent, accumulated costs, quantized time and state-delta effects are retained | Richer than the legacy meaning target; distinct traces can remain distinct here |
| `CompositionPlan` | Timeline/contract to form, discourse, arrangement and backbone | Phrase grid, concrete rhetorical roles, obligations and seats constrain downstream choices | A fixed plan is a deliberate test condition; actual plans can differ between traces |
| `MeaningPlan::target` (`F_P`) | Trace and fixed plan to coarse events | Trace supplies only `story_arc`; the plan supplies event sites/roles/backbone | Five possible trace-dependent target classes at most for a fixed plan |
| Meaning-directed composer | Target, chart space, theme grammar/germ, seed | Searches a chart before the theme; uses existing thematic trajectory and schedules thesis/consequent | Old hook equals thesis; target does not bind a new specific argument referent |
| `MeaningPlan::observe` (`μ`) | Actual SongMap theme sites/chart to coarse events and witnesses | Reads material relations and harmonic preparation/misses; catches many genuine symbolic defects | Does not read Score, PCM or listeners; witness payloads are richer than event classes |
| `Commutation` | Target and observation to class/site comparisons | Enforces coarse theme/harmony event equality and reports divergence ownership | Coarse equality is not source faithfulness or human recognition |
| Performance/Score | Song and world/language/options to actions, material and notes | Existing source/provenance machinery can carry particular actions and material IDs | Independent transport/realization checks are still needed |

Do not describe the *entire* trace-to-song pipeline as factoring through `story_arc`.
That claim is false: `IntentTimeline` sees the intermediate path, and the first
counterexample below explicitly verifies different intermediate intents and morphisms.
Only the trace dependency of `MeaningPlan::target` factors that way when its plan is held
fixed.

## Exact fixed-plan quotient theorem

Let `P` be any fixed `CompositionPlan` and `A(t) = story_arc(t)`. By direct inspection
of `meaning.rs`, define `g_P` as the remaining target construction after supplying its
two local variables `arc` and `resolution`. Then

\[
F_P(t) = g_P(A(t)).
\]

Proof: `target` invokes `story_arc(trace)` once. Every subsequent read is from `P`,
local counters, or the returned pair. No other trace field is used. Conversely the
result includes the pair unchanged as public `arc` and `resolution` fields. Therefore

\[
F_P(t_1)=F_P(t_2)\quad\Longleftrightarrow\quad A(t_1)=A(t_2).
\]

The quotient has **at most five** classes: `(Low, Home)`, `(Mid, Home)`, `(Mid, Open)`,
`(High, Home)`, `(High, Open)`. `Low` is always `Home` because pressure below 0.5 is
declared released. This is a finite quotient statement; “low-rank” is only an analogy,
not a claim about linear algebra.

The maximum is taken over presentation-state pressure. Ties select the **last** maximum.
Above the low threshold, release is the existence of a later `Confirmation` or
`SectionResolved`, not a measure of how convincingly musical material resolved.

`tests/audio_argument_collisions.rs` predeclares and checks:

- Two different middle stories (suspension/foregrounding versus an early release/recolor)
  with the same peak and final confirmation collide under each of three fixed plans.
- Their intermediate `IntentTimeline` states and applied morphisms differ. This control
  prevents the mistaken inference of whole-pipeline equality.
- A 72-input finite mutation domain changes intermediate event kind, tone and timing,
  while holding the peak/release constant. Every target still collides under one fixed plan.
- Changing the peak class or removing the final release changes the target. A release
  before the last equal peak does not close it. These controls establish the tested
  boundary is neither an empty plan nor an insensitive test harness.

## Source-unbound obligation discharge

`Obligation` records kind, source phrase, deadline, strength, deferrability and settlement.
It has no concrete `Motif`/`MaterialId` referent. `settle` correctly rejects an unknown,
already settled, not-yet-opened or role-incompatible debt. This is useful temporal/kind
discipline and should remain intact.

However, `discharges(MotifQuestion, action, plan)` accepts any `Answer` action.
`bind_settlement_witnesses` requires only that its start occur in the settling phrase.
It does not inspect a specific question's material, an answer's material, or even an
`Answer.pays` link on this branch of the predicate.

The executable counterexample builds a real question material and a different descending
phrase with different intervals and timing. The second material is an independent
`Statement`, not a `Derived` source. An `Answer` with `pays = None` witnesses the paid
question debt. A separate interval/onset witness establishes the materials differ without
using the generator's transformation predicate. Missing, wrong-kind and outside-window
controls correctly remain unwitnessed.

This result applies to the **legacy obligation action witness**. It does not claim the
complete performance's other conformance checks accept that deliberately synthetic case.
The material is not passed to the legacy binder at all; therefore that binder cannot
certify the relation, even when some other layer independently restricts a generated
response. A stronger optional receipt must check the material relation and actual
carrier fulfillment at its own boundary.

## Declared pitch units are lost by a legacy observer

`Motif.pitch_basis` explicitly distinguishes scale steps from semitones. The timed material
representation and motif pitch realization preserve that distinction. Nevertheless
`ThemeRelation::of` compares coordinate and rhythm vectors without comparing their
pitch basis; equal vectors in different units are classified `Literal`.

The executable probe uses `[0, 4, 3, 5, 2]` in C Ionian. The scale-step version realizes
intervals `[7, -2, 4, -5]` semitones; the semitone version realizes `[4, -1, 2, -3]`.
The legacy relation still calls them literal. This is a unit mismatch, not an allowed
transposition or an aesthetic judgment.

A seventh test applies the unit mutation to the first theme site of the already
established meaning-directed flagship (`deflected_lift_trace(120)`, seed `2112`). Both
the original and altered song pass **full legacy `Commutation::check`**, and their
complete `Observation` values are equal, even though the independently realized
interval vectors differ. This confirms an actual `F/μ` certification collision,
including its witness payloads. It makes no assertion that Score/PCM or every other
performance law also accepts the mutation.

Likewise `landing` uses `degree mod 7` for both units. A semitone endpoint `12` is an
actual tonic octave, while `landing` reports `Open`. `reach` thresholds also operate in
stored coordinates without a unit normalization. These are reasons **not** to use the
legacy predicates as the independent witness for new pitch-aware argument contracts.
Historical behavior is preserved by this test-only audit; changing that observer globally
would be a separate compatibility decision.

## Declarative fields that must not be mistaken for measurements

The existing source documents these limits explicitly:

- `MusicalThesis.anchors` is a descriptive copy read by no consumer; the contract's
  anchors are the authority.
- `PhraseGoal.thematic_distance` is read by a diagnostic as a target, not used to steer
  a realizer. `RealizationDiagnostics` measures realized return separately.
- `PhraseGoal.harmonic_distance` and `novelty_budget` are descriptive and unconsumed.
- `MeaningEvent` kinds are symbolic relation classes. Their equality does not prove
  listener learning, anticipation, recognition or acoustic salience.
- The actual `ThemeRelation` classifier is stronger than ID equality: it compares written
  material, derives contiguous stretches, permits transposition/inversion and uniform
  rhythmic scaling. The gap is not “all IDs pass”; it is source-question binding,
  observational granularity and the unit defect above.

## Historical and current claims

PR #93 reports downstream source-preservation experiments on an earlier audited source.
Its exact artifact/head must remain the authority for what that probe actually ran;
later lead-development/dynamics and narrative audition changes do not retrospectively
alter those outputs. The fixed-plan proof and discharge/unit probes here are newly checked
against the refreshed C137 base named above.

The source diagnosis is confirmed as a restricted mechanism defect. The broader hypothesis
that repairing these mechanisms makes the musical discourse perceptually intelligible
remains **unverified until blind listening**. Symbolic, source, Score, PCM and human
acceptance are separate evidence layers. No additional voices, adjective mappings or
commutation labels alone pay that truth debt.

## Reproduction

```sh
cargo test --test audio_argument_collisions
```

Observed on 2026-10-10 UTC: all **7 tests passed** under Rust **1.85.0**, run in an
isolated detached worktree at the exact `fbdbffcee3f77dab3cef0234666192cadc7da370`
base with only this test file copied in. The full output was:

```text
running 7 tests
legacy_generic_answer_witnesses_a_question_without_referent_or_source_relation ... ok
legacy_fixed_plan_is_sensitive_to_its_declared_peak_and_release_controls ... ok
legacy_question_witness_controls_reject_absence_wrong_kind_and_wrong_window ... ok
legacy_fixed_plan_loses_opposed_intermediate_stories ... ok
legacy_theme_observer_collides_declared_pitch_units ... ok
legacy_fixed_plan_kernel_includes_event_identity_timing_and_subpeak_changes ... ok
legacy_full_commutation_accepts_a_pitch_unit_mutation_of_its_flagship ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

All fixtures use existing public APIs and no device/sample/cloud dependencies. These are
baseline limitation tests: a future repair of the legacy observer/discharge rules should
deliberately revise their pinned expectations and document the changed contract. The new
optional argument path should add its own rejection tests rather than silently changing
this historical control.
