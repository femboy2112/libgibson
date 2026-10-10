# Musical audit of the optional argument slice

Inspected baseline: `fbdbffcee3f77dab3cef0234666192cadc7da370`, the refreshed
`feat/v0.5-humanmusic-beefup` head used to start this implementation. This is a
source audit, not a new listening verdict. Historical listening acceptance below
comes from repository records; this auditor did not hear these recordings.
The final source review also inspected the optional `argument.rs`,
`argument_transport.rs`, `argument_witness.rs` and `functor::perform_argument`
integration. Baseline findings and repaired build-time findings are distinguished
below; they must not be read as current defects in the optional path.

## Source facts and the boundary they establish

* `composer::compose_meaning_seeded` selects a chart before evaluating the
  supplied germ. Its aggregate structural-note fit then ranks themes inside that
  already selected chart. It cannot guarantee that a particular question's final
  pitch is open in the *actual* ending harmony, or that its answer prepares and
  completes the same destination.
* `composer::schedule` assigns `hook = thesis`. Different thematic functions can
  therefore share exactly the same source phrase. That is a valid recurrence
  policy; it is not evidence that the hook makes a different argument.
* `Motif::id` survives inversion, retrograde, sequence, arbitrary transposition
  and concatenation. Matching IDs alone cannot witness audible identity. A
  declared lawful transformation and its interval/timing consequences are needed.
* `MaterialEvent` already has separate onsets, sounding durations and accents.
  `InteractionMaterial::from_motif` cannot recover gaps from `Motif` because it
  advances each onset by the preceding duration. The handcrafted `rick_probe`
  arrays contain real gaps, varied gate lengths and per-note accents.
* `material::transform_material` quotes/inverts/completes the last four events,
  while compression uses the first three. These are bounded conversational
  operations. An obligatory thematic statement cannot silently inherit these
  reductions as lossless source transport.
* `material::project` walks the local chord-scale and snaps onto stable tones.
  `stable_for(Bass)` allows chord tones only. A projected bass quote may preserve
  contour direction while changing the defining intervals. The exact source
  carrier path must be distinct from this discretionary figure path.

`docs/HUMAN_MUSIC_BAND_STORY.md` records acceptance of handcrafted carrier
migration, intentional withholding and the lead-muted narrative control. It
also records an unresolved bridge weakness. Its identity-preservation claim is
explicitly inferred rather than isolated by a broken-identity A/B. These facts
support the new controls; they do not certify the new generator.

## Smallest useful joint planning contract

For the first slice, bind one timed referent, its terminal consequence and the
phrase harmonies together before performance. Do not retain an arbitrary old
chart and then expect exact note carriage to make it suitable.

1. Establish the source head and its complete consequence over home harmony.
2. Restate the same head but withhold that consequence. End over a concrete
   dominant whose pitch content pulls toward the declared home.
3. Develop or repeat the open material, preserving a declared head, before the
   payoff. A delay is real elapsed musical time, not a changed adjective.
4. Answer with the same head and the explicitly promised terminal consequence,
   over an actual dominant-to-home path.

The answer witness must include both the linked material relation and its
harmonic arrival. A tonic pitch over a non-tonic chord is not sufficient.

For promise/denial/return, hear the complete expectation first, prepare it again,
replace the expected arrival with a connected different harmony, and only then
return. A denial before an established promise is merely a departure. A later
literal copy can still be meaningful in a changed context; a claimed transformed
return requires a declared change in the actual source.

Conflict/reconciliation needs a separate relation connecting two distinguishable
heads to a compatible consequence, such as a constrained inversion or a shared
cadential cell derived from both. Introducing two IDs and doubling them at the
end is not reconciliation. It is correct to leave this family unsupported until
that relation and its negative controls exist.

## Generative phrase and harmonic compatibility

Generate the short identity cell from a finite grammar of interval movements,
onset patterns and accents, then vary it with fresh seeds. Keep the question and
answer bound to that generated cell; do not select an unrelated answer from a
second seed. At least three salient pitched events should survive the relation
so that the witness is stronger than a ubiquitous final scale step.

The implemented rootless head uses offsets `{2, 7, 14}` from home: ninth, fifth
and octave-displaced ninth. It contains only root/fifth tones over V7, so its
prepared question avoids the earlier tonic-over-dominant fourth clash. The
terminal completion supplies offset `0` over actual home harmony. A terminal
second over V7 is a consonant fifth of that chord, yet remains open because the
dominant is not home. Its eventual step to tonic is only an earned answer when
the chart also arrives home.

This pool is a bounded calibration choice, not a theorem that every source fits
every world. Other heads can introduce a fourth held against a major third or
unsupported chromatic tones. Exact transport must report incompatibility rather
than silently snap pitches or invent a chord-tone label.

The `rick_probe` control fixes its A-minor notes and chord chart while changing
timbres. A semitone source transposed into a re-moded major generator chart is a
different experiment. Preserve the chromatic source and validate the hybrid's
actual chord content, or declare an explicit conversion; mark incompatible
hybrids invalid. Do not classify them as evidence against source preservation.

## Independent exposure and listening controls

`HumanMusicSynth::bus_levels()` intentionally meters every bus even if its
`StemMask` route is muted. A nonzero lead meter therefore cannot witness that
the output PCM exposes the required lead. Use rendered PCM, the required solo
stem and a muted-carrier counterfactual. Compare samples under identical render
conditions; nonlinear master processing means solo stems do not sum exactly to
the full mix.

Before listening, reject missing/corrupted carrier events, a removed second
carrier in a required two-carrier payoff, changed event order, broken source
intervals, wrong terminal consequence and an unprepared announced resolution.
These are necessary structural checks. None establishes that a listener finds
the relationship salient or compelling.

Prepare full mix, source-carrier stem and carrier-muted controls. A naked phrase
being legible while the full mix remains ambiguous points toward masking or
arrangement salience; ambiguity in the naked phrase points toward the source
grammar or the chosen discourse primitive. Keep the production, tempo and
loudness procedure identical across semantic contrasts and randomize labels.
Separate intentional unresolved closure from failed debt payment.

Human listening remains **UNVERIFIED** until the maintainer listens to the new
packet. It may accept, reject or mark the intended relationship ambiguous.

## Fixed build-time red-team findings

The initial compiler scheduled the question source at its phrase's downbeat,
while the inherited Lift placed V7 only in its last harmonic unit. Its short
question therefore ended before the pointer. The revised compiler makes both
Lift anchors explicit V7. It also keeps development on V7, so the eventual
answer really follows a dominant-to-home path. An intermediate IV development
version would have produced a plagal arrival; that version was revised rather
than described as an authentic cadence.

Source-level root identity was also insufficient: the ordinary backbone
colourer could turn V7 into a major-seventh chord or remove its seventh. The
final optional route calls `prepare_performance` before any player chooses
notes. It pins actual Question/Develop chords to V7, Answer/Return/Establish to
home, and Denial to the different vi arrival, checks world/language vocabulary,
and rebuilds harmonic contexts and deflection evidence. Incompatible harmonic
edits or modulation are refused. The independent Score observer inspects chord
pitch content at the question and completion; gesture labels are not its proof.

The initial root/octave head over V7 was its fourth against the dominant third.
The final generated head `{2, 7, 14}` removes that calibration clash while
retaining the same source-specific completion relation. Bass development now
states V7 root/fifth material; the final plural return is Lead+Keys, avoiding a
tonic-ninth bass becoming an unintended slash-chord floor.

The compiler silences the old lead in band-carried phrases, preventing an
unrelated foreground voice from competing with the required carrier. The
promise/denial family's final return uses actual rational augmentation, so its
transformation is in the written source rather than merely in its label.

An initial content witness admitted an arbitrary constant semitone shift for
each statement despite describing octave equivalence. The revised public
observer checks the common tonic frame, so an answer shifted by a semitone
cannot independently pass as the same consequence.

The compiler recomputes `MeaningPlan::target(trace, actual_new_plan)`. It does
not retain a stale old-plan target and does not replace the target with the
observation to force commutation. The old coarse law is still measured as its
own layer, independently of the stronger source witness.

## Current boundaries and next musical gate

* Historical identity projections recognize `melody` plus statement-material
  provenance and certain bass tags. The independent argument transport does not
  forge those identifiers. The old generic `PerformanceReceipt` and coarse
  commutation remain measured separately and are not claimed green by the new
  witness. An honest optional read-side bridge remains integration work; this
  route is not advertised as accepted by `perform_checked`.
* Stock SwissSignal declares `use_sevenths = false` and correctly refuses this
  required V7 calibration. The audition uses explicitly configured comparison
  worlds with a shared A-Aeolian frame, admitted sevenths/mixture and matched
  tempo. These compare timbre/production, not unchanged native-world vocabulary
  or major/minor remoding. Historical defaults are untouched.
* The generated grammar is narrow. It provides original seeded cells and lawful
  source-bound development, not a broad joint search of melodic arguments and
  harmony. Its related bank hook is not independently established as a separate
  audible hook. Conflict/reconciliation remains explicitly unsupported.
* The machine checks establish source/chord relations and can reject absent PCM
  contribution. They do not establish salience, masking adequacy, recognizable
  questions/answers or artistic merit. Listening remains **UNVERIFIED**.

The next musical gate is the blinded full-mix/carrier-stem packet. If the naked
source is legible and the full mix is ambiguous, inspect carrier salience and
masking. If both are ambiguous, change the generated premise or discourse
primitive. Neither result calls automatically for more operators or density.
