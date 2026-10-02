# AI Visual Authoring — Preregistered Holdout Experiment

> **Status:** PREREGISTRATION. Not yet executed. This document is written
> *before* any treatment run so the design cannot be retrofitted to the result.
> It creates no core commitment.
>
> **Do not execute any arm from a context that has read the guide, the audit, the
> recipes, or this file** — that contaminates the model and voids the result. The
> author of this document (and any context that helped write the guide) is
> disqualified as an implementer. See [§6](#6-contamination-controls).
>
> **Date written:** 2026-10-02 · **Substrate under test:** released `v0.4.0`
> (`c2f6483d92fe2b351e6cd50936a97d8cdf73cb79`), unchanged in both arms.

## 1. The decisive question

The audit concluded the panel-farm attractor is a **documentation/composition
gap, not a capability gap**, and that a documentation intervention (the guide +
recipes + router) should move weak agents toward world-first composition. That is
a *claim about behavior*, and it is cheap to fool ourselves about. This experiment
tests exactly one thing:

> **Does the documentation intervention materially change the design behavior of
> a fresh coding agent, given the same substrate and a comparably ambitious
> prompt?**

- **If yes:** documentation was load-bearing. The intervention is justified;
  stop there (do not add API).
- **If no:** documentation alone cannot carry the composition need. The case for
  an explicit composition/`VisualIntent` API strengthens (see
  [`VISUAL_INTENT_API_PROPOSAL.md`](VISUAL_INTENT_API_PROPOSAL.md)), and the API
  proposal should be reconsidered on that evidence.

We do **not** choose the outcome in advance, and we do **not** produce a scalar
"beauty score." The primary readout is a **human maintainer visual verdict** plus
**descriptive behavioral metrics**.

## 2. Arms

Both arms get the **same** released `v0.4.0` substrate and the **same** task
prompts. The only difference is the docs available.

| Arm | Docs provided |
|---|---|
| **CONTROL** | `v0.4.0`-era docs as they shipped: `README.md`, `docs/UI_LAYER.md` (the pre-intervention version — the commit **before** this branch merges), `docs/FEATURES.md`, `docs/GLYPHS.md`, public API docs, existing examples. **No** `AI_VISUAL_AUTHORING.md`, **no** recipes, **no** `AGENTS.md` route, **no** UI_LAYER callout. |
| **TREATMENT** | The same, **plus** `AGENTS.md`, `docs/AI_VISUAL_AUTHORING.md`, the `examples/recipe_*.rs` recipes, and the UI_LAYER callout (i.e. this branch merged). |

Checkout discipline: CONTROL is built against the parent of this branch's merge
commit; TREATMENT against the merge commit. Record both SHAs in the results.

## 3. Subjects (models)

- **Prefer non-Sonnet models.** The observed failure occurred in DeepSeek + agy;
  the positive control was Sonnet. To test whether docs lift a *weaker* composer,
  the subjects must include the model family that failed. Sonnet may be included
  as a ceiling reference but is not the test of interest.
- **Fresh contexts only.** Each run starts from a clean context that has never
  seen the guide, the audit, the recipes, the Chronoscope implementation, or this
  file.
- **≥2 subjects per arm, ≥2 prompts each**, so a single lucky/unlucky run does
  not decide it. Record the exact model id and date of every run.

## 4. Task prompts

Give each subject an ambitious **spatial/cinematic** application prompt with a
real domain. Prompts must be *new* (not Theseus/Cathedral/Chronoscope's domains,
not any example in the dossier). Candidate domains (pick fresh ones at execution
time; do not reuse these verbatim if they have leaked):

- a live scheduler/queue as a physical field of jobs flowing through stages;
- a filesystem/tree explorer as a navigable spatial structure;
- a network/packet flow as a continuous topology under load;
- a state-machine execution as a spatial graph you move through.

**The prompt must NOT contain any of the following** (doing so rescues or poisons
the test):

- the words "panel farm", "DOS", "dashboard", or "don't make it look retro";
- any instruction about composition, hero objects, cameras, or chrome;
- Chronoscope's implementation or its existence;
- which previous agents failed, or that a visual problem exists at all.

The prompt is identical across arms. The ONLY variable is the available docs.

## 5. Measures

Descriptive, not a composite score. Record each per run:

**Primary (decides the result):**
- **Human maintainer visual verdict** on the rendered result (the same kind of
  judgment that produced the original verdict): world-first / panel-farm /
  mixed, with a one-paragraph rationale.
- **Dominant visual object?** Did the agent create one custom object that owns
  the frame? (yes / no / weak.)

**Secondary (behavioral, descriptive):**
- **World area vs chrome area** at 120×40 (rough fraction of the frame the
  custom-rendered world occupies).
- **Panel/border count** and **border dependence** — descriptive only, not a
  quality score (Cathedral proved a low count ≠ good and panels ≠ bad).
- **Information locus** — does readable state live *in* the world (projected
  labels, geometry, color) or beside it in panels/text?
- **Continuity** — one coordinate space + camera, or N swapped views?
- **Source archaeology** — how many `src/` files / approximate lines the agent
  read before it had an architecture. (The guide claims ~600 lines of docs
  replaces this.)
- **API trial-and-error failures** — count of compile/run failures against the
  public API before first working frame.
- **Did it independently adopt** continuous-world / camera / overlay composition?
- **Did it render and inspect** its own output (visual-acceptance behavior)?

## 6. Contamination controls

- The guide/audit/recipe authors and any context that saw them are **disqualified
  implementers**.
- Treatment docs are provided as *files in the repo the agent is working in* —
  the realistic channel — not pasted into the prompt as instructions. We are
  testing discoverability + uptake, not obedience to a pasted spec.
- Do **not** tell any subject that a visual problem exists, that previous agents
  failed, or that it is in an experiment.
- Do **not** "rescue the thesis": if a treatment subject still panel-farms, that
  is data — do not then paste the guide into its prompt and call it a success.
- Record actual context/isolation limits honestly (shared model weights across
  arms are an unavoidable confound; state it).

## 7. Analysis & decision rule

- Report every run's measures in a table; **no averaging into a scalar.**
- The result is read from the **pattern**: does TREATMENT shift the human verdict
  and the behavioral measures (dominant object, information-in-world, continuity,
  reduced archaeology) toward world-first, *relative to CONTROL*, on the weaker
  models?
- **Decision:**
  - Clear, repeated shift toward world-first under treatment → *"Documentation
    was load-bearing."* Ship the docs; do not add API this cycle.
  - No shift, or shift only on models that already compose well (Sonnet) →
    *"Documentation alone cannot express the repeated consumer need."* Promote the
    [`VISUAL_INTENT_API_PROPOSAL.md`](VISUAL_INTENT_API_PROPOSAL.md) to a real
    design review.
  - Partial/mixed → name exactly which behaviors moved and which did not; that
    map tells you which *specific* concept (if any) wants an API vs more docs.

## 8. Threats to validity (stated up front)

- **Shared weights:** control and treatment share a model; we isolate the *docs*
  variable, not the model. A true held-out model family would be stronger but is
  not available.
- **Prompt leakage:** if a candidate domain has appeared in training data as a
  "LibGibson cinematic app", it is contaminated — pick genuinely novel domains.
- **Small N:** this is a behavioral probe, not a powered study. It can show a
  *direction*, not a p-value. Report it as such.
- **Verdict subjectivity:** the human verdict is the acceptance datum by design;
  record the rationale so a reader can disagree with the evidence in hand.

## 9. Execution checklist (for whoever runs this later)

- [ ] Freeze CONTROL SHA (pre-merge) and TREATMENT SHA (post-merge); record both.
- [ ] Select ≥2 non-Sonnet subjects + fresh prompts screened against this dossier
      for leakage.
- [ ] Confirm each implementer context is clean (never saw guide/audit/recipes).
- [ ] Run all arms; capture each result at 120×40, 80×24, 42×15, and Mono.
- [ ] Record all §5 measures per run; do not average.
- [ ] Maintainer renders the verdict; apply the §7 decision rule.
- [ ] Record the outcome and the decision (ship docs / open API review) as a new
      dated results doc; link it here.
