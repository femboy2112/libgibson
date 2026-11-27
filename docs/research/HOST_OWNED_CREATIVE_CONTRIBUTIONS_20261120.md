# Host-Owned Ephemeral Creative Contributions — Stage Interface Pressure Test

**Status:** public design-only pressure test / 2026-11-20. **No new libgibson API, runtime, CI, model interface or release commitment.** The library remains a generic presentation system; conversational state, permissions, agents, source memory and model inference belong to the consumer application. This note names a new consumer pressure case for `gibson::stage`, `gibson::ui`, `Surface`, raster composition, and existing headless/PTy fixtures.

## 1. Motivation

A host application may receive a **candidate creative response** from an assistant: illustrate a hypothesis with a small graph, compare two mathematical interpretations, animate a causal relationship, or present a bounded artifact that advances a conversation. The proposal is *not* an instruction to libgibson to execute arbitrary model code; rather, the host may independently validate and materialize it into a typed presentation.

An assistant that spontaneously proposes an artifact should not be able to use the renderer as an unsandboxed shell, UI-authority principal, raw ANSI generator, arbitrary network loader, or source of new world observations. A visual flourish cannot overwrite security notices, alter permissions or certify an effect. A successful render proves only that a bounded picture was displayed, not that an external computation or claimed social meaning was correct.

**Product target:** small, meaningful, optional expressive scenes integrated with a readable conversation. Do not optimize arbitrary animation or attention capture. The host must preserve a do-nothing/no-artifact outcome and reduced-motion/mono accessibility.

## 2. Proposed consumer-side typed chain

```text
Host event + approved conversation/app state
  → model/user suggests CreativeContributionCandidate (untrusted)
  → host parser: closed declarative schema, IDs, sizes, budgets
  → host policy: user consent, current scope and side-effect category
  → host generation: abstract diagram/scene/raster or safe presentation
  → host checks output size/layout/layers/permission precedence
  → gibson Stage/Node/Surface presentation
  → host records applied presentation and actual effect witnesses separately
```

No first-class `CreativeContribution` libgibson type is proposed as a stable CORE API yet; integration may use public existing `gibson::stage::map_actions`, `gibson::ui` and `Surface`. The host adapts to any future API changes and remains the only input dispatcher.

Example **hypothetical** intent, not an accepted libgibson parser:

```json
{
  "kind": "ephemeral_diagram",
  "title": "Two hypotheses, one source map",
  "primitive_budget": 48,
  "text_budget_bytes": 2048,
  "pane": "contextual",
  "motion": "optional",
  "persistent": false,
  "semantic_content_id": "host-minted-id"
}
```

The actual diagram geometry, quotes and source data must be resolved by trusted host code within declared resource bounds. Any feature that writes files, loads media/network content, controls other windows or executes calculations beyond the host's allowed pure procedure remains a *separate* capability. Do not make those possibilities silently available by broadening `Stage`.

## 3. Presentation invariants

1. **Host chrome precedence:** independent STOP, permission overlays, disclosure, user input/editor and error notices cannot be masked, displaced or reordered under visual content. No untrusted `on_press` action can become authorization; typed events return to the owning host.
2. **Lifetime:** a transient diagram is dismissed, replaced, reduced or cancelled by the host. No hidden scheduler, persistent focus theft, off-screen input trap or indefinite animation. Reconcile scene identity under resize and cancellation; a late result after STOP must not repaint an unauthorized scene.
3. **Layout:** conversation remains the reading anchor; the diagram can occupy an opt-in pane, inline preview or inspector. Fail closed to text summary under sub-minimum terminal geometry; do not corrupt terminal history.
4. **Accessibility:** no critical meaning solely in color, blinking or motion. Reduced-motion yields stable final geometry; mono/ANSI16 and headless screenshots carry analogous essential content. Screen reader/plain-text fallback reports the underlying typed proposal and whether it executed, without fictional verification.
5. **Performance:** bound objects, cells, bytes, raster allocation, draw time and GPU/CPU demand; prefer one resolved frame or finite short animation. Under resource pressure, trim visual ornament before degrading host STOP or ordinary chat. No unbounded raw ANSI or compositor hooks from user/model text.
6. **Evidence:** the host distinguishes `Proposed`, `Prepared`, `Rendered`, `Failed`, `Cancelled`. `Rendered` is a presentation receipt only, not execution witness or source truth; a speculative animation never validates the depicted science.

## 4. Consumer-first falsifiers

- Headless 40×15 mono, 80×24 ANSI16, 120×40 truecolor: the host's protected STOP/permission regions remain visible, safe, and keyboard reachable.
- Resize/shrink/expand while a candidate is entering/removing Stage: no stale scene/anchor or input theft; run real PTY where test scope permits.
- Reduced motion and color-degraded modes: the mathematical/causal distinction remains legible.
- Unknown intent, unexpected primitive, corrupted UTF-8, malicious escape text, oversize output, stale semantic ID, duplicate event, hidden control bytes: reject or sanitize without executing anything.
- Two candidate diagrams: one materially useful to a source-labeled question, one irrelevant. Host selection and `Hold` are app-level decision tests, **not** a renderer's ML/psychological responsibility.
- Cancel just before frame commit; check whether prior partial output appears and truthfully report physical display uncertainty rather than inventing a perfect rollback.
- Cloud vs local model may supply *proposals* in consumer tests using synthetic data; libgibson's deterministic materialization should be validated independently of which model generated them.

The host should supply known safe fixture intents and expected screen states. Compare libgibson against a text-only fallback as a simpler rival; retain existing API if host-side adaptation suffices. No extra widget family or engine feature justified without a demonstrated consumer blocker. If an actual general-purpose bug is reproduced, file a sanitized upstream issue with a minimization and exact version/SHA.

## 5. Scope and claim ledger

- **Observed:** existing `Stage` semantic-mount, typed UI, raster/animation surfaces are documented in `docs/STAGE.md`, `docs/UI_LAYER.md` and `docs/SCENE_ALGEBRA_LAWS.md`.
- **Conjectured:** bounded on-demand illustration improves practical conversational collaboration compared with text alone.
- **UNVERIFIED:** this new consumer's headless/PTY, latency, accessibility, usefulness or live inference tests.
- **Excluded:** machine feeling, authority, shared-memory correctness and user's psychological state. These are not claims of a terminal renderer.

**Validation debt:** docs-only GitHub connector write; no cargo build, local PTY, actual consumer acceptance or live cloud inference executed for this RFC. The next integration session must inspect real consumer Cargo pin and libgibson version, run the library's required local gates, and record exact SHA before shipping an API.
