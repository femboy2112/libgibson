# Experience Grammars

**Experimental. Rust-only. Not in the C ABI. Not a release.** Part of the v0.5
"Observable Instruments" milestone, Spatial Interface axis. Lives under
`gibson::ui::experience`.

> One application, described once, realized as radically different interfaces —
> a cover shelf, a cross-media bar, an orbital field, a blade stack, a
> typographic panorama — **without the application forking per style.**

This document is the contract. It is deliberately readable before the style
source; a fresh agent should be able to author an application, or a new grammar,
from this page alone (see **Cold start for agents** at the end).

---

## 1. Skin is not Experience

`gibson::ui` already has a `Skin`: it owns palette, chrome, density, glyph
vocabulary, semantic styles and motion tokens. It restyles an element tree. Keep
it exactly as it is — do **not** inflate it into a god object.

The problem this layer solves lives *above* `Skin*`. By the time a `Skin` sees the
element tree, the application has already decided its geometry — these rows, that
column, this panel. A `Skin` can repaint that geometry; it cannot turn a list into
a perspective carousel, because the list was already chosen.

The Experience layer sits higher. The application stops choosing geometry and
instead describes itself **semantically**. A *presentation grammar* then chooses
the geometry — and two grammars may choose wildly different geometry for the same
semantic value.

```
Application (semantic)         ← what exists, what can be done
      │
      ▼
Experience  (gibson::ui::experience::Experience)
      │   no row/column/panel/camera decided yet
      ▼
Grammar::present   F_σ         ← the style picks the geometry
      │
      ▼
Presented { Element, PresentationReceipt }
      │
      ▼
gibson::ui compile → Node → layout → raster → terminal
```

A grammar adds **no** new renderer, layout engine or terminal owner. It lowers
into ordinary `Element`s and the existing `raster3d` / `Surface` escape hatches.

---

## 2. The semantic vocabulary

Minimal on purpose. The application never names a row, a column, a yaw or a blade.

| Type | Meaning |
|------|---------|
| `Experience<A>` | the whole application: a title + ordered `Destination`s |
| `Destination<A>` | a major section you navigate *to* (Library, Now Playing, Settings, About) |
| `Content<A>` | what a destination holds — **four** kinds only |
| `Item<A>` | one selectable entity in a collection (album, settings row, tile) |
| `Action<A>` | a typed, named thing you can do; `invoke: A` is the app's own payload |
| `Facet` | a read-only property (a now-playing field) |
| `Media` | a procedural, copyright-free visual identity (`seed`), realized by media grammars |
| `Priority` | `Essential` \| `Normal` \| `Tertiary` — drives *declared* responsive omission |

The four `Content` kinds:

- `Collection(Vec<Item>)` — an ordered selectable set. The carousel/shelf/tile/list
  domain. **Settings is a `Collection`** whose item actions cycle a value — not a
  separate node kind.
- `Detail { facets, actions }` — properties + actions for the current thing.
- `Prose(Vec<String>)` — read-only lines (about, help, status).
- `Custom(Custom)` — an attached instrument the grammar composites **verbatim**
  (the escape hatch; see §6).

`A` is the application's own action type (an `enum Msg { … }`). The grammar never
interprets `A`; it only surfaces actions and, on activation, hands the stored
`invoke: A` back to the application unchanged. A grammar therefore **cannot invent
or alter an action.**

Identity is the existing `gibson::ui::element::Key` — reused, not reinvented.

---

## 3. The preservation law

A grammar is a presentation *functor* `F_σ`. The law, shared with the rest of
v0.5's Representation Atlas:

```
π_σ(F_σ(A)) = required_semantics(A)      up to declared responsive omission
```

where `π_σ` is the semantic projection of a presentation (the ids it represents).
In plain English, a style may rearrange **everything visible**, but it may not:

- invent an action or id the application never defined;
- silently lose a required action, destination or item;
- change an entity's identity;
- move the selected object merely because the presentation changed.

This is enforced in **two tiers**, not aspirational. As a grammar lowers, it
records a `PresentationReceipt` of exactly what it represented, what it drew into
a raster (`rastered`), what it **declared** it omitted, and what it degraded.
`required_semantics(&experience, &state)` computes what is required at the current
state.

- **Bookkeeping tier — `receipt.check(&required)`** — the receipt's claims against
  the required semantics.
- **Rendered tier — `presented.check(&required)`** — the bookkeeping tier *plus* a
  cross-check that every id the receipt claims is actually in the lowered element
  tree (as a keyed node) or explicitly attested in `receipt.rastered`. This is the
  gate a grammar must pass. It closes the hole where a receipt describes a render
  that was never produced: a grammar that draws only a title can no longer pass by
  writing an honest-*looking* ledger.

Both tiers return `LawViolation`s:

| Violation | Meaning |
|-----------|---------|
| `MissingDestination` | a destination was not made reachable |
| `WrongActiveDestination` / `WrongSelection` | the style moved a fixed semantic fact |
| `MissingAction` | a reachable action was neither shown nor declared omitted |
| `EssentialOmitted` | an `Essential` item was declared omitted — never allowed |
| `SilentLoss` | a required id is neither represented nor declared omitted |
| `Invented` | the style represented an id the application never defined |
| `UnrenderedClaim` | the receipt claims an id that is neither keyed in the tree nor attested rastered (rendered tier only) |

The receipt is a **semantic-honesty ledger, not an aesthetic score.** The only
omissions it tolerates are the ones the grammar *declares* (and never of an
`Essential`). **Boundary:** the law verifies that a claimed id is *present* (a
keyed node, or attested as drawn); it does not and cannot verify the *pixel
content* of a rastered region — that a cover shows the right art, that the right
blade is highlighted. Pixel faithfulness is covered by visual acceptance (§43–44),
the milestone's deliberate machine-vs-human split. So `rastered` is an attestation
the grammar makes; lying in it is possible but now *explicit* rather than silent.

---

## 4. State ownership

Three owners, kept separate:

- **Application** owns domain data and business state (the real albums, the real
  playback). It builds the `Experience` from that state.
- **Runtime** owns `PresentationState` — a tiny bounded value: the active
  destination and a selection **per destination**, both stored *by identity*
  (`Key`), not by position. It is *grammar-independent*.
- **Grammar** owns its private camera / transition interpolation (a spring, a pan
  offset). This never leaks into `PresentationState`.

Because `PresentationState` is grammar-independent, **switching the active grammar
cannot move the selection.** Select album 7 under the shelf, switch to the
cross-media bar — still album 7. That is the central guarantee, and it is tested.

Because state is keyed by **identity**, a dynamic application that inserts,
removes, reorders or filters between frames (the Elm rebuild-in-view pattern)
never has its selection silently retargeted to a different entity: the active
destination and selection are re-resolved against the current `Experience` on
every access through **one** canonical clamping rule (`active_index` /
`selected_index`). `required_semantics` and every grammar read that same rule, so
they never disagree about what is selected — if a selected entity is removed, all
readers fall back to the same clamped position. (A removed selection resetting to
the first item is the defined behavior, not a bug.)

---

## 5. Navigation intents

Physical keys never reach the application. A grammar binds keys to a small
grammar-independent vocabulary:

```
Next · Previous · Enter · Back · NextGroup · PreviousGroup · Home · End
```

Different grammars bind different keys (a shelf uses Left/Right for
Previous/Next; a cross-bar uses them for the destination axis) but the **semantic
effect of each intent is fixed** in `apply_intent`, identical for every grammar.
Item and destination motion **clamp** at the ends — never wrap, never teleport —
so rapid input and mid-motion reversal stay coherent.

The single input entry point:

```rust
// Returns the typed application action to dispatch, if Enter activated one.
let msg: Option<Msg> = handle_key(&*grammar, &experience, &mut state, &key_event);
```

Only the **primary** action (an item's first action, a detail's first action) is
reachable through this vocabulary — `Enter` activates it — and so only the primary
action is *required* by the law. A grammar that wants to surface and reach a
secondary action binds a key directly to `SemanticInput::Invoke(action)`; such
actions are a grammar's optional affordance, represented when the grammar chooses,
never silently required-but-unreachable.

The grammar decides *which* key means *which* intent; `apply_intent` decides what
the intent *does*; the application receives back only its own `Msg` — never a key
code, never an intent.

---

## 6. The custom-content escape hatch

Some domains should not be squeezed into the semantic vocabulary — a dense
observatory instrument, a live visualization. `Content::Custom` carries a closure
`Fn(cols, rows) -> Surface`. The grammar allots a cell rectangle and composites
the produced `Surface` **verbatim** (via `ui::raster` / `ui::surface` /
`ui::presented`); it does not reinterpret the contents. This preserves the
"ordinary `Node`/`Element` APIs remain valid" promise — Experience is an *opt-in*
higher layer, not a replacement, and raw content is always one hatch away.

---

## 7. Responsive & capability law

A grammar may reduce, at small sizes: visible neighbours, tertiary metadata,
decorative depth, reflections, large headings. It may **not** silently lose
current identity, essential navigation, or a required action. Every reduction is a
**declared** omission in the receipt (`omitted`), and an `Essential` is never an
allowed omission.

Colour may never be the sole identity channel. Every grammar must carry at least
one redundant identity channel (glyph, position, topology, label, silhouette,
ordering) so it survives `ColorDepth::Mono`.

The reference grammar `STANDARD` represents everything at every size and
capability — it is the oracle, not the flagship.

---

## 8. Switching style, live

The application holds `Box<dyn Grammar<Msg>>` and swaps it:

```rust
grammar = Box::new(CrossMedia::new());   // selection and destination survive
```

No different view function, no `match style` in the application, no duplicated
tree, no domain reset. The view is always:

```rust
grammar.present(&experience, &state, env, now).element
```

**Application code may say _which_ style is active. It never says _how_ a style
renders.**

---

## 9. Cold start for agents

To author an **application**:

1. Define `enum Msg { … }` — your typed actions.
2. Build an `Experience<Msg>` once: `.destination(Destination::new(key, title,
   Content::…))`. Use `Collection` for selectable sets (including settings),
   `Detail` for a property page, `Prose` for text, `Custom` for an instrument.
3. Hold `state: PresentationState` (`PresentationState::new(&experience)`) and
   `grammar: Box<dyn Grammar<Msg>>`.
4. View: `grammar.present(&experience, &state, env, now).element`.
5. Input: `if let Some(msg) = handle_key(&*grammar, &experience, &mut state,
   &key) { /* update domain */ }`.
6. Switch style: replace `grammar`. Never branch the app on style.

To author a **grammar**, implement `Grammar<A>`:

- `name()` — a stable `&'static str`.
- `present(&mut self, experience, state, env, now) -> Presented<A>` — build an
  `Element` *and* a `PresentationReceipt`. Resolve selection via
  `state.selected_index(experience)` / `state.active_index(experience)` (never
  cache a raw index). Key every node you claim, or list a rastered id in
  `receipt.rastered`. Record every declared omission.
- `interpret(&self, key, experience, state) -> Option<SemanticInput<A>>` — bind
  your metaphor's keys to intents (or a direct `Invoke` for a secondary action).
- Reduce only `Tertiary`/decorative content under size pressure, and **declare**
  every omission. Carry a non-colour identity channel.

The non-negotiable test for any grammar: for a fixture experience, at every
destination and across the responsive/capability matrix,
`present(...).check(&required_semantics(...))` (the **rendered** gate) is empty,
and a live switch to and from it preserves `active_destination` and `selected`.

---

## 10. Known boundaries

Honest limits of the current contract — none a blocker for the six realized
grammars, all worth knowing before you lean on them:

- **The law checks presence, not pixels.** `rastered` is an attestation; the law
  cannot verify a cover shows the right art or the right blade is lit. That is
  visual acceptance's job (§43–44). The receipt describes the *semantic target* of
  a frame, not its instantaneous pixels — during a spring glide the receipt names
  the destination cover while the camera is still sliding toward it. That is a
  normal steady state, by design.
- **`interpret` is keyboard-only and `&self`.** A pointer/spatial grammar that
  wants to turn a click on an orbital node into a `SemanticInput` has no hook yet,
  and cannot consult the camera/layout computed in `present`. Keyboard navigation
  is the current contract; pointer input is a future amendment, not a silent gap.
- **One `Content` kind per `Destination`.** A destination that wants both a
  property sheet *and* a scrollable queue is two destinations. A real
  expressiveness boundary, deliberately accepted to keep the vocabulary at four
  kinds.

---

## 11. The realized grammars

Six grammars ship with the layer. All pass the same non-negotiable test (§9):
the **rendered** gate `present(...).check(&required_semantics(...))` is empty at
every destination across the full responsive/capability matrix, and a live
switch preserves `active_destination` and `selected`.

| Grammar | Axis / metaphor | Technique | Navigation | Identity without colour |
|---|---|---|---|---|
| `STANDARD` | reference list / oracle | Node (`list`/`field set`/`prose`) | ↑↓ item, ←→ group | position + emphasis + the structure itself |
| `MEDIA_SHELF` | Cover-Flow shelf | raster (`raster3d` textured quads, damped spring) | ←→ item, ↑↓ group | centre focus, size, reflection, caption |
| `CROSS_MEDIA` | cross-bar | Node (centred cross, no raster) | ←→ group, ↑↓ item | the crossing focus + emphasis + position |
| `PANORAMA` | typographic panorama | Node (letter-spaced large type, edge-bleed) | ←→ pan sections, ↑↓ column | chevron slivers, ▸ marker, `NN / NN` counter |
| `ORBITAL` | focal radial field | raster (2D polar, procedural orbs, spring) | ←→ ring, ↑↓ group | centre/size/halo, `n/N` counter, `◉`/`○` strip |
| `BLADES` | occluding depth-plane stack | raster (painter's algorithm + per-pixel owner buffer) | ↑↓ blade, ←→ item | stack position, `▲ NN`/`▼ NN` labels, bright rim, reverse-video row |

`STANDARD` is the oracle, not the flagship: it exists to represent *everything*
so a cinematic grammar's receipt can be compared against ground truth.

Two raster grammars (`ORBITAL`, `BLADES`) and one (`MEDIA_SHELF`) derive their
procedural art from the item's `Media { seed }` when present, and **fall back to
a deterministic hash of the item key** when it is absent — so a grammar never
requires media. This is proven by a second, media-free fixture (an ops console:
services / metrics / logs / config) run through every grammar; no grammar
secretly assumes "albums".

The live demo is `cargo run --example experience_lab`: one semantic application
rendered through all six grammars, switchable with `s` / `1`–`6`, with the
selection surviving every switch and **no `match style` anywhere in the app**.
`cargo run --example experience_lab -- dump [W H]` prints each grammar's frame as
visible text for headless inspection.

---

## 12. Temporal contract

What a grammar's frame depends on *in time* — so a runtime knows when it may stop
repainting. Each property is a test (coalescing an obsolete frame is only sound
when a settled frame is byte-identical):

- **Time-invariant** — `STANDARD`, `CROSS_MEDIA`, `PANORAMA` ignore `now`
  entirely. The same semantic state yields a byte-identical frame at any clock;
  paint once and coalesce every later frame until the state changes.
- **Settling** — `MEDIA_SHELF` and `BLADES` animate a damped spring, then settle
  to a true **0-diff**. Once `is_settled(target)`, later frames are identical, so
  repainting can stop. The receipt always names the semantic *target*; during a
  glide the camera is still sliding toward the cover the receipt already claims —
  a normal steady state, not a law violation.
- **Continuously animated** — `ORBITAL` has an ambient field (swirl, drifting
  motes, halo breathe) that is a function of `now` with **no settle point**. Its
  frame genuinely differs over time at a fixed selection, so a runtime must keep
  repainting it while it is on screen. It remains deterministic at a fixed time.

No frame is rebuilt from wall-clock state the law cannot see: every grammar is a
pure function of `(experience, state, env, now)` plus its own spring, and that
spring is itself a pure function of the `now` sequence.
