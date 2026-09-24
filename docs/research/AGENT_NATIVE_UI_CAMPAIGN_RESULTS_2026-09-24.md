# Agent-native UI campaign results

**NON-BINDING RESEARCH / NO NEW CORE COMMITMENT.**

The external pressure-test campaign completed without a LibGibson implementation
change. Rendering and keyboard interaction expressivity are **CORROBORATED in
the tested domain**, including three concepts selected after the public substrate
and lab helper/IR freeze. This does not prove universal representability or that
professional UI is easy for independent developers. LibGibson remains an
**ENGINEERING ALPHA** with experimental Rust composition/graphics APIs.

## Authoritative records

- Flagship PR #14 merged ordinarily at
  `e5ede0a4ab8ff52caa567ee10d45824e42c1ebc6`.
  [First exact-main CI 35960000454](https://github.com/femboy2112/libgibson/actions/runs/35960000454)
  executed and passed Rust, bindings, sanitizers, Go and PTY.
- New-main local Rust 1.98.1 gates passed: fmt, strict all-target/all-feature
  Clippy, **574 tests = 226 unit + 348 integration**, release/debug/release-example
  builds, strict rustdoc, 3 FX Lab example tests, C/C++/Python/Go/ASan smoke, and
  diff checking. This campaign adds no LibGibson test count.
- Research dossier commit `8fe58aec3320e8f6b413718c4f352ab9df96fad9` merged through
  PR #16 at `3d9117d4bfa45f7d890bff12d40879a8d2bb6a47`; exact-main
  [CI 35961022598](https://github.com/femboy2112/libgibson/actions/runs/35961022598)
  passed. The original docs-only research ref was preserved; the refreshed branch
  was recreated from green main. Five forbidden control bytes and broken notation
  were repaired without rewriting the research history.
- External lab: [femboy2112/libgibson-agent-native-ui-lab](https://github.com/femboy2112/libgibson-agent-native-ui-lab),
  report snapshot **`0fbb38f83c586784fc9e80bb27ee825dcdf3b433`**.
  [Full final report](https://github.com/femboy2112/libgibson-agent-native-ui-lab/blob/0fbb38f83c586784fc9e80bb27ee825dcdf3b433/docs/AGENT_NATIVE_UI_FINAL_REPORT.md),
  [friction synthesis](https://github.com/femboy2112/libgibson-agent-native-ui-lab/blob/0fbb38f83c586784fc9e80bb27ee825dcdf3b433/docs/CROSS_EXPERIMENT_FRICTION.md),
  and [selection provenance](https://github.com/femboy2112/libgibson-agent-native-ui-lab/blob/0fbb38f83c586784fc9e80bb27ee825dcdf3b433/docs/holdouts/SELECTION.md)
  contain attempted routes, failures, corrections and commands. The exact report
  snapshot also passed [lab CI 35964384000](https://github.com/femboy2112/libgibson-agent-native-ui-lab/actions/runs/35964384000).

## What was actually frozen and tested

`FROZEN_LIBGIBSON_SHA = e5ede0a4ab8ff52caa567ee10d45824e42c1ebc6`.
`FROZEN_LAB_HELPER_IR_SHA = 1d1d27286b50c752527021697c14fd8a32ccd0ea`.
The Git dependency never moved. Later LibGibson merges only documentation.
A committed receipt hashes 21 existing lab source/helper/test/dependency files;
the guard passes and rejects a deliberate helper edit in a disposable clone.
This research freeze is not a public stability promise.

The same finite semantic agent-session trace drove manga, semantic reactions and
validated generated instruments. Explicit actions were recorded separately.
The reaction 3-mode × 2-metaphor comparison retained full semantic equality.
The provisional six-form InstrumentSpec and authority-owning SessionPort remained
external. Static schema programs reject effects/nonzero animation and unsafe
content; native consumers retain the lower public Surface/canvas/raster route.
The holdouts did not establish that every interface fits that provisional IR.

**G1 is now observed ergonomic pressure, not an observed expressive wall.**
Public FocusId/FocusRing and app-owned dispatch supported stable selection,
replacement, explicit actions and modal restoration. The dynamic registry and
policy in one instrument consumer do not yet justify a new core lifecycle/router.
No generic primitive earned promotion; no core-promotion PR was created.

| Post-freeze holdout | Public route and tested semantic contract | Verdict |
|---|---|---|
| Foldroom | Filled depth geometry, stable selected face, hinge/edge and explicit coordinate oracles, recorded controls | PUBLIC API SUFFICIENT for the finite folding workbench |
| Cuebox | Story/Scene/FocusRing plus Surface stage; actual cue branch, actor position, modal restoration and replay | PUBLIC API SUFFICIENT for the finite rehearsal |
| Weavebench | Editable Boolean draft, cyclic-float oracle, actual Mono over/under geometry, subcell viewport and replay | PUBLIC API SUFFICIENT for the finite draft editor |

The initial proposer received only neutral API docs/source. Three of its first
five proposals overlapped preregistered examples and were excluded; replacement
selection is explicit in the record. Foldroom/Cuebox used fresh implementer
contexts. A tool thread limit forced Weavebench to reuse its fresh proposer
context; that weaker separation was recorded before implementation. All contexts
share a model family and filesystem. This is not an OS-isolated blind study or
independent human evidence.

The lab passes **61 integration tests**, including all four terminal sizes,
planned TrueColor/ANSI16/Mono, holdout TrueColor/Mono, deterministic state/frame
replay, explicit action counterfactuals, resource rejection and frozen 0/0/0.
Eighteen release PTY matrix smokes restored terminal state; actual-cell PTY tests
verify delivery of selection/permission actions. The implementation's
[public CI 35964133018](https://github.com/femboy2112/libgibson-agent-native-ui-lab/actions/runs/35964133018)
passed at `15a67ebfbbf8365d24f33107d98196cb9eed761b`. That is a separate lab suite,
not extra LibGibson tests or general platform certification.

Consumer failures remain visible: incomplete focus replay, opaque composition
that erased Cuebox's floor, and too-small Weavebench glyphs that collapsed both
crossing topologies in Mono. They were corrected through the same frozen public
API and checked on actual state/cells. No failed API wall was rescued by changing
core, helper, schema, or the acceptance contract.

## Claim and next work

**CORROBORATED:** public API expressivity across these finite dynamic terminal
interaction domains, including three post-freeze concepts absent from the dossier.
**EXPERIMENTAL:** generated IR, consumers, and modern Rust layers.
**UNVERIFIED:** arbitrary UI universality, comparative developer ease, broad
human usability, large/long-lived sessions, and non-Linux terminal portability.
**REFUTED:** correctness of the first consumer drafts; retained failures matter.

Next prioritize the bounded resize/input/backpressure investigation in issue
#15. D6 API/MSRV/distribution, D7 retention/resources (#10), and D8 embedded
ownership/restoration (#11) remain separate open debt. An independent external
user cohort with larger changing target sets would add more expressivity
information than another same-model graphical showcase. Do not add widgets or
expand the IR merely because a prior prediction expected them.
