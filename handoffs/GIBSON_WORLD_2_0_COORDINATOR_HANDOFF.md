# GIBSON WORLD 2.0 — Coordinator Handoff

**Date:** 2026-11-19 EST. **Work type:** docs-only architecture/design, not implementation.  
**Source baseline:** LibGibson main 9e3f9cf55b027bcec24ca9128d69a9b335e6be1f.  
**Canonical documents:** [architecture](../docs/GIBSON_WORLD_2_0_SPEC.md) and [execution/tests](../docs/GIBSON_WORLD_2_0_EXECUTION.md).  
**Status:** GW0–GW7 all future; NO native windows, GPU pipeline, approved character rig, real platform probes or release asserted by this handoff.

## Owner's target

“Hack the planet” quality GUI interfaces made from the same semantic experience that already drives LibGibson's terminal. **Windows are viewports/cameras onto a world, not owners of the world's state**. MINERVA is the demanding external consumer; her process/agent stays owned by ProjectGHOST. The host machine can be her operating substrate only under existing host permissions. Her canonical avatar stays in the main window until rig/puppetry and identity tests are passed. Do not infer authorization for a desktop overlay from this spec.

## Engineering priority (do not invert)

1. **Read and reconcile** exact main, Stage/Experience, release rules, PHOSPHOR #419, transitions #434, Kitty #438. Distinguish shipped from draft.
2. **Establish pre-cell pixel/semantic seam** and deterministic software oracle with synthetic high-frequency RGBA and typed hit map (GW1).
3. **Real first native window** with keyboard/pointer, Unicode/IME, DPI, resize and GPU/software fallback (GW2).
4. **Semantic preservation** under terminal/native projection and modal/action-negative tests (GW3).
5. **Cinematic grammar** (Blades and Media Shelf) with ergonomic conversation and settling motion (GW4).
6. **Two projections + hostile platform/hardware matrix** (GW5–GW7).
7. Only then expose an experimental consumer integration API. Art/rig and host permissions remain private external work.

## What to delegate

- Agent A (read-only): current code/API/ABI/MSRV/dependency matrix, baseline CLI/test descriptions, duplicate issues and branch conflicts.
- Agent B: pixel-space scene-to-display-list software oracle and losslessness mutation test.
- Agent C: native desktop backend spike, branch/build isolation, accurate Wayland/X11 capability report.
- Agent D: typed pointer/key hit-map and semantics-preservation oracle.
- Agent E: visual grammar + synthetic character asset fixture; no real image pack.
- Agent F: platform/cpu-pressure/accessibility hostile review; fresh implementation/holdout independence.

## Stop and escalation conditions

- A proposed generic API would require updating the existing terminal renderer/ABI without a compatible adapter.
- A change loses a host-critical action, alters typed permission semantics, or replaces source facts with decorative status.
- A transparent desktop overlay silently acquires OS capture/control powers.
- An agent reports a “passed” native window, GPU benchmark, user approval, or character fidelity test without an actual run and raw receipt.
- A v0.9 draft's intentional RED tests are mistaken for shipped functionality.
- Any private consumer artwork, host identifiers, raw model context or secrets appear in public diffs.

## Deliverables from each agent

Exact head/baseline, changed path list, source-to-contract argument, RED+GREEN where applicable, mutation and null controls, target/runtime versions, fail/ambiguous cases, resource envelope, and next verdict-changing probe. Keep release/version decisions with the release coordinator. No main merges, force-push, tags or production permissions implied.

**Immediate success metric:** one synthetic semantic scene rendered natively with correct source-resolution pixels, verified typed input and an equivalent terminal projection; **not** a 3D avatar or root-privileged desktop agent.
