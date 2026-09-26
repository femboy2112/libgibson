# Experimental semantic UI

`gibson::ui` is a Rust-only composition layer for semantic components, skins,
typed actions and finite presentation motion. It lowers into the existing
`Node` / Taffy / `Surface` / differential renderer pipeline. It adds no terminal
renderer, layout engine, terminal owner, dependency, or C ABI.

This is an **experimental branch API**, not part of the tagged v0.1.1 release or
the CORE stability tier. Use a reviewed branch revision to try it. The existing
`Node`, `Context`, `Scene`, `Surface`, `Theme` and lower-level examples remain
valid. See [the release contract](RELEASE_CONTRACT.md) and
[the measured validation record](UI_LAYER_VALIDATION.md).

## Start with a small application

```rust
use gibson::ui::prelude::*;

#[derive(Clone)]
enum Action { Build }

fn view(builds: &usize, _: &BuildCx) -> Element<Action> {
    screen()
        .child(heading("OPERATIONS"))
        .child(panel("WORKSPACE")
            .child(status("Connected").tone(Tone::Success))
            .child(text(format!("{builds} builds completed")))
            .child(progress("context", 0.42))
            .child(button("Run build").key("build").on_press(Action::Build)))
}

fn main() -> std::io::Result<()> {
    App::inline().skin(skins::BLACK_ICE).run(
        0usize,
        |builds, event| {
            if let AppEvent::Action(Action::Build) = event { *builds += 1; }
            Control::Continue
        },
        view,
    )?;
    Ok(())
}
```

Run `cargo run --example ui_quickstart`. Tab / Shift-Tab move focus, Enter
activates the focused control, and Ctrl-C exits. Replace `BLACK_ICE` with
`VAPOR95` or `SWISS_SIGNAL` to change the design grammar. No raw colors are needed.

`App::fullscreen()` uses the ordinary fullscreen `Context` path. `App::run`
returns the final model and restores the context it created. The update receives
`AppEvent::Action(A)`, unconsumed `AppEvent::Input(Event)`, and
`AppEvent::Tick(Duration)` with elapsed time. Return `Control::Quit` to stop.
This helper is deliberately small; it is not an asynchronous application
framework or a source-window virtualizer.

If your application already owns a `Context`, use `App::run_with_context`.
Its update callback receives `&mut Context` for ordinary scrollback commits and
other substrate operations. It does not acquire another terminal session or
restore the caller's context on return; the caller owns that lifecycle.

## Examples and inspection

| Example | What to inspect |
| --- | --- |
| `ui_quickstart` | A short App/model/update/view application with no raw color values |
| `ui_gallery` | Controlled selection/input, statuses, charts, table, command-menu modal, toast and custom canvas |
| `skin_gallery` | The same gallery view and harness, with a different initial skin choice |
| `polished_agent_ui` | Context scrollback, permission capture, simulated failure/recovery, editable prompt, code viewport and telemetry |

The galleries and agent accept `--skin=vapor95|black-ice|swiss-signal`,
`--mono`, `--ansi16`, `--ansi256`, `--truecolor`, `--glyphs=...`,
`--reduced-motion`, `--no-motion` and `--fullscreen`. Galleries have a
"Switch skin" control and `--page=0|1|2` for overview, components and custom
canvas. `--modal` starts the command menu open.

Use `--dump --width=80 --height=24` for a deterministic settled text capture or
`--dump-ansi` to preserve output styles. Repeat at 120×40 and a narrow size.
These use Context headless capture and the existing terminal pipeline.

`polished_agent_ui --auto` runs a deterministic simulated approve-once path.
`--stage=plan|permission|failure|prompt|rejected|cancelled` selects a coherent
inspection state. The simulation never edits files or invokes real tools.
Approve-once, approve-for-session, rejection and cancellation are distinct
application outcomes; the UI only routes those actions. Actual render-byte
telemetry is separate from its simulated tool work.

The original `polished_agent.rs` is preserved. Its counterpart reduces the
manual skin, focus and presentation glue while retaining the central permission,
failure/recovery, input, scrollback and viewport interactions. It is not a copy
of the original's complete Story inspector or cinematic art direction. Constrained
views deliberately show nearby tasks and omit secondary telemetry/stream text;
source line count is not evidence of novice usability or feature-for-feature parity.

## Compose intent with ordinary Rust

Builders return `Element<Action>`. `.child(...)` and `.children(iter)` retain
the tree's visible structure. Functions that return elements are useful custom
components without a trait or macro. Leaf components reject semantic children
with `UiError::InvalidChildren`; a viewport accepts at most one child. Put a
column inside a viewport when several pieces form its world. This prevents
interaction metadata for children that would never become visible.

| Family | Initial vocabulary |
| --- | --- |
| Composition | `screen`, `row`, `column`, `stack`, `spacer`, `list`, `viewport` |
| Information | `text`, `label`, `heading`, `code`, `status`, `badge` |
| Containers | `panel`, `card`, `section`, `divider` |
| Controlled interaction | `button`, `choice`, `text_input`, `tabs` |
| Visualization | `progress`, `sparkline`, `table` |
| Floating presentation | `modal`, `toast`, `.overlay(...)` |
| Substrate | `raw(Node)`, `surface(Arc<Surface>)`, `raster(Surface)` |

`stack()` follows ordinary `Node::stack` sizing: give it an explicit size or a
bounded parent. For a floating layer over intrinsically sized content, prefer
`.overlay(...)`.

`tabs()` is a row of application-supplied choices, not a second selection model.
Tables are small, equally allocated cell columns, not sortable/virtualized data
grids. `list()` is a column, not a virtual collection. `viewport(x, y)` clips an
already-created subtree using the existing camera primitive; it does not avoid
laying out an enormous offscreen world. A command menu can be composed from an
input and keyed choices; a built-in searchable command palette is deferred.

Semantic builders include `.tone(Tone::Success)`, `.emphasis(Emphasis::Strong)`,
`.density(Density::Compact)`, `.elevation(Elevation::Raised)`, `.selected(bool)`
and `.disabled(bool)`. Domain selection remains a value supplied by the view.
Density inherits down the semantic tree; a component can override it locally.

Layout uses `.width(cells)`, `.height(cells)`, `.grow(weight)`, `.gap(cells)` and
`.padding(cells)`. `row().responsive(72)` becomes a column below terminal width
72; this breakpoint uses the environment width, not a nested container query.
`BuildCx.environment` is available for larger responsive changes or hiding
secondary information. Explicit keys survive those changes. Small terminals
still require deliberate content priorities: clipping is not a promise that an
arbitrary tree fits every size.

Horizontal `row()` children with `.grow(weight)` and no explicit width share the
available width by weight. Explicit widths retain their basis. When a responsive
row becomes a column, children recover their intrinsic width. Other containers
use ordinary substrate flex growth.

Mount a dialog with `screen().child(base).overlay(modal(...))`. Overlays preserve
an in-flow base and use ordinary absolutely positioned Node children, so they
do not reflow the base or erase its intrinsic size. Placement resolves inside
the containing box, including locally nested overlays; modals center through
ordinary Taffy constraints and toasts align to its bottom right. A screen with
an overlay uses the environment height unless an explicit `.height(...)` sets
its local extent.

## Theme is a palette; Skin is a design grammar

The existing `Theme` and `ThemeStyles` remain the semantic palette. `Skin`
builds above them with chrome, spacing, title treatment, glyphs, visualization
and motion tokens. `skin.resolve(&UiEnvironment { ... })` produces a
`ResolvedSkin` for width, height, color depth, glyph mode and motion preference.
`UiRuntime` caches that resolution until the environment or skin changes.

| Skin | Structure and controls | Motion language |
| --- | --- | --- |
| `VAPOR95` | Framed diagnostic windows, title bars, highlighted edges, physical button delimiters and chunky meters; gray/plum and cyan/lavender accents | Short physical reveal and activation feedback |
| `BLACK_ICE` | Technical rails, compact spacing, geometric status marks and command-style controls; near-black field with cyan/green instrumentation | Acquisition dissolve/scanline, focus illumination, brief error displacement |
| `SWISS_SIGNAL` | Numbered sections, horizontal rules, restrained boxing and more negative space; ivory/charcoal/cobalt hierarchy | Editorial wipes and restrained rule emphasis |

The same semantic gallery is used by `ui_gallery` and `skin_gallery`; skin
selection does not choose a different application tree. Within a manual loop,
`runtime.set_skin(skins::SWISS_SIGNAL)` changes it in one call. Focus and business
state survive; active effects from the previous grammar are cleared.

Choosing a skin opts into its designed surface/background treatment. This is
an explicit high-level visual choice, including Swiss's ivory field, above the
existing native-background-oriented `Theme`/direct-Node defaults. It does not
change those defaults or the terminal protocol.

### Capability degradation

Color realization continues to use LibGibson's existing quantization ladder:
TrueColor, ANSI256, ANSI16 and Mono. Mono also uses structural treatments
(borders, reverse/bold/dim, labels, spacing and glyphs); semantic distinctions
must not rely exclusively on color. Glyph mode is a separate policy axis, not
font detection. Built-in visualization components reuse `gibson::show` and
skin-specific compact/fallback treatments.

Set `UiEnvironment.color_depth` consistently with the `Context` capability
snapshot. `App` does this automatically and honors the existing environment
glyph policy. A custom raster/canvas still owns its color/glyph realization;
use `glyph::transcode_surface_glyphs` or the existing raster realization path
before embedding it. `raw(Node)` does not rewrite an expert's glyphs or styling.

## Actions, keys, focus and editor ownership

`button("Approve").key("permission.approve").on_press(Action::Approve)`
binds a typed value. UI routing returns that action; it does not decide what
approval means. `A: Clone` is required for lowering/routing. There are no hidden
domain reducers.

Explicit `.key(...)` values are **globally unique within one semantic tree**,
including overlays. Duplicate keys return `UiError::DuplicateKey` before a new
frame mutates runtime state. Unkeyed elements get positional path identities in
a separate namespace. Use domain-stable keys for reorderable lists, removable
controls and modals; an index or changing label does not provide stable identity.

Focus order follows semantic traversal, including enabled bound controls.
Disabled ancestors disable descendant interaction. Stable keys retain focus
through reordering/reflow. Removal or disabling chooses an available control;
an absent key is not retained forever. The substrate `FocusRing` still performs
traversal. `runtime.focus()` and `runtime.set_focus(&Key::named(...))` expose
presentation focus without putting it in business state.

`handle_event(&Event)` returns `EventOutcome { actions, consumed }`. Dispatch
actions first; pass the original event to application shortcuts only if it was
not consumed. Tab / Shift-Tab traverse. Enter activates an action; unmodified
Space activates non-editor controls. `.on_event(...)` gives a focused custom
control its unhandled events, for example an application-owned `ViewportState`.

A topmost modal captures keyboard/paste routing. Escape invokes its optional
`.on_dismiss(Action)`; unbound keys do not leak to the background. Nested modal
capture restores the previous live focus on removal, falling back if that key
has disappeared. The runtime owns capture/restoration, while the model owns
whether the modal exists and what dismissal does. Last modal in traversal is
the active scope; this is a keyboard scope, not a general capture/bubble DOM.
The optional App loop reserves Ctrl-C for termination, including during modal
capture; it delivers an application Input notification before stopping.

Editors are controlled too:

```rust,ignore
text_input(&model.editor)
    .key("prompt")
    .placeholder("Ask for a build")
    .on_edit(Action::Edit)
    .on_press(Action::Submit)
```

`Action::Edit(TextInputState)` receives an edited copy through the existing
grapheme-safe editor. Store it in the model, then rebuild the frame **before
dispatching the next edit**. Dispatching multiple edits against the same
interaction snapshot would repeatedly edit the old value. `App` rebuilds between
events. Only the focused input requests the hardware cursor; inactive inputs
lower to text. A caller-applied `SurfaceFx` on an ancestor can suppress the
cursor under existing painter semantics.

Toast existence and expiry remain application-owned. `toast(...)` supplies
presentation; it does not create an invisible timer or mutate the model later.

## Finite semantic motion

`MotionRole` includes Enter, Exit, Focus, Blur, Activate, Success, Warning,
Error, Busy, ModalEnter, ModalExit and Change. Skins resolve roles into pure,
finite `MotionPlan`s using existing `SurfaceFx` and easing. Busy means one
explicit pulse, not an unbounded implicit loop.

The runtime compares keys and presentation state (`selected`, `disabled`, modal
status and `.revision(n)`). A changed string is not guessed to be a new semantic
event: increment `.revision(n)` when content warrants a new signal, optionally
with `.motion(MotionRole::Success)`. Named elements and modals can acquire entry
motion; unkeyed layout wrappers are not all animated automatically. Focus and
activation get finite feedback. `runtime.changes()` exposes Enter / Exit /
Stable / Change / Focus / Blur classifications.

`MotionPreference::Full` uses the skin's language. `Reduced` uses a brief style
emphasis with no displacement or hidden information. `None` settles immediately.
At its deadline a plan returns the exact empty effect chain: it adds no residual
style to the settled tree. Given the same ordered frames/events and explicit
times, presentation is deterministic. Runtime times clamp backwards samples;
create a fresh runtime for an independent replay.

**Removal is immediate in this implementation.** Exit is reported, and departed
keys/actions/effects are discarded. No exit ghosts are retained. Exit/ModalExit
plans are available for explicitly controlled composition: keep the outgoing
element in your view while sampling the plan, then remove it at the deadline.
The convenience runtime does not automatically perform that lifecycle.

## Use your own loop and inspect the lowering

```rust,ignore
let environment = UiEnvironment {
    width: 80,
    height: 24,
    color_depth: context.capabilities().color_depth,
    motion: MotionPreference::None,
    ..UiEnvironment::default()
};
let frame = runtime.frame(&view, environment, elapsed)?;
// These are public, ordinary outputs rather than an opaque runtime widget.
let node = frame.node;
let action_map = frame.interactions;
let keyed_state = frame.keys;
context.set_root(node);
context.render_now()?;
```

For pure lowering without reconciliation, use
`compile(&tree, &BuildCx::new(skin, environment))`. `Compiled<A>` always exposes
its ordinary `Node`, interaction sidecar and key metadata. This is also useful
with `Context::headless(mode, width, height)` and rendered-buffer capture.

## Local escape hatches and custom components

`raw(existing_node)` preserves the existing node's layout unless a semantic
layout builder explicitly overrides it. Wrap it in ordinary semantic rows,
panels, viewports or overlays. `surface(Arc<Surface>)` shares prebuilt content;
`raster(Surface)` transfers it to the ordinary substrate raster node. Existing
Braille/half-block canvases, software 3D and raster FX end at those surfaces.
Raw elements are leaves of the semantic tree: compose additional semantic
children alongside them in a container, or build a complete ordinary Node
subtree before passing it to `raw`.

`.post_process([SurfaceFx::...])` appends ordinary local post-processing after
the semantic motion chain. Effects remain ordered and need not commute. There
is no direct raw-ANSI escape in the UI layer; terminal ownership stays in Context.

An optional extension trait is available:

```rust,ignore
struct Instrument;

impl Component<Action> for Instrument {
    fn build(self, cx: &BuildCx) -> Element<Action> {
        // Any ordinary Node/Surface/canvas/raster composition can be used here.
        panel("INSTRUMENT")
            .child(raw(make_instrument_node(cx.environment.width, cx.time)))
    }
}

let custom = component(Instrument, &build_cx);
```

`BuildCx` carries resolved skin, environment, explicit time, focus and current
motion chains. `ui_gallery` includes a custom canvas component beyond the
built-in catalog. Raw nodes do not invent interaction metadata: bind actions or
custom events on their semantic wrapper when keyboard routing is needed.

Scene integration works in both directions: embed `raw(scene.to_node(...))`,
or place a compiled `frame.node` inside an ordinary `SceneEntity`. Retain the
runtime/interaction sidecar when you need typed keyboard actions. Scene visual
transforms do not create mouse hit testing or a spatial event-routing contract.

## Rules for skin authors

Skin constants are ordinary editable values:

```rust
use gibson::ui::{skins, Density};

let mut editorial = skins::SWISS_SIGNAL;
editorial.name = "Wide editorial";
editorial.density = Density::Spacious;
editorial.spacing.spacious_gap = 2;
```

1. Start from an existing `Skin`, preserving `Theme` as the palette. Modify
   public design tokens; do not add a second palette model or terminal writer.
2. Choose a coherent chrome/spacing/title/control/visualization grammar. Prove
   that an unchanged semantic tree differs structurally, including in Mono.
3. Keep success, warning, danger, selection and disabled state legible without
   RGB. Respect the glyph policy and the established central color quantizer.
   Chrome/status glyph tokens should occupy one displayed cell; supply their
   ASCII alternatives. Application text is never automatically transliterated.
4. Map motion to meaning. Effects must be deterministic, finite, exact at their
   settled endpoint, and sensible under Reduced/None. Avoid continuous noise.
5. Exercise at least 80×24, 120×40 and a narrow case, with long labels, Unicode,
   active input, modal focus and raw/custom content. Do not infer font support
   or cross-platform behavior from an in-process capture.

The first skin model exposes a small vocabulary of chrome/motion grammars.
Entirely new structural grammars may need an experimental lowering extension
or a custom component; arbitrary skin plugins/configuration files are not promised.

## Resource and scope boundaries

Modal focus memory retains one eligible focus key per live scope plus the base
view. Covered sibling modals restore focus just like nested modals; departed
scopes are pruned. `App` reserves the renderer's final terminal row in inline
mode and uses the full row budget in fullscreen mode.

Runtime maps retain live keys and at most one active finite motion per key;
modal capture is bounded by live modal count, and removed state is discarded.
Semantic compilation rejects trees beyond its element/depth limits. These limits
do not cap application string/vector sizes or raw subtree/Surface allocations.

Views, semantic metadata and ordinary Node trees are rebuilt. Skin resolution is
cached, and shared Surface storage stays shared, but this is not incremental
layout, a zero-allocation frame loop or a general retained widget framework.
Keep worlds bounded; profile before adding caches with their own invalidation
and retention rules.

Mouse hit testing, event bubbling, exit ghosts, automatic toast lifetimes,
virtualized lists/tables, automatic content-diff animation, a built-in command
palette, foreign-language bindings and human usability studies are deferred.
The existing crossterm resize/input collision limitation still applies. Tests
and visual inspection establish the recorded finite cases, not universal
expressivity, universal ease, production stability or a new platform guarantee.
