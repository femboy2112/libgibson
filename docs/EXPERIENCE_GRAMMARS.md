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

This is enforced, not aspirational. As a grammar lowers, it records a
`PresentationReceipt` of exactly what it represented, what it **declared** it
omitted, and what it degraded. `required_semantics(&experience, &state)` computes
what is required at the current state. `receipt.check(&required)` returns every
`LawViolation`:

| Violation | Meaning |
|-----------|---------|
| `MissingDestination` | a destination was not made reachable |
| `WrongActiveDestination` / `WrongSelection` | the style moved a fixed semantic fact |
| `MissingAction` | a reachable action was neither shown nor declared omitted |
| `EssentialOmitted` | an `Essential` item was declared omitted — never allowed |
| `SilentLoss` | a required id is neither represented nor declared omitted |
| `Invented` | the style represented an id the application never defined |

The receipt is a **semantic-honesty ledger, not an aesthetic score.** The only
omissions it tolerates are the ones the grammar *declares* (and never of an
`Essential`).

---

## 4. State ownership

Three owners, kept separate:

- **Application** owns domain data and business state (the real albums, the real
  playback). It builds the `Experience` from that state.
- **Runtime** owns `PresentationState` — a tiny bounded value: the active
  destination and a selection index **per destination**. It is
  *grammar-independent*.
- **Grammar** owns its private camera / transition interpolation (a spring, a pan
  offset). This never leaks into `PresentationState`.

Because `PresentationState` is grammar-independent, **switching the active grammar
cannot move the selection.** Select album 7 under the shelf, switch to the
cross-media bar — still album 7. That is the central guarantee, and it is tested.

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
  `Element` *and* a `PresentationReceipt` that honestly records what you
  represented and what you declared omitted. Run your presentation against
  `STANDARD`'s receipt and `required_semantics` in a test; `receipt.check(...)`
  must be empty.
- `interpret(&self, key, experience, state) -> Option<SemanticInput<A>>` — bind
  your metaphor's keys to intents (or a direct `Invoke`).
- Reduce only `Tertiary`/decorative content under size pressure, and **declare**
  every omission. Carry a non-colour identity channel.

The non-negotiable test for any grammar: for a fixture experience, at every
destination and across the responsive/capability matrix,
`present(...).receipt.check(&required_semantics(...))` is empty, and a live switch
to and from it preserves `active_destination` and `selected`.
