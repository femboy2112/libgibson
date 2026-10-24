# The Visual Atlas Plan (authoring template)

Fill this in **before** you write paint code for any cinematic or
multi-representation LibGibson piece. It is an *authoring* artifact — LibGibson
never reads it at runtime. Its whole job is to make you state the reasoning that,
skipped, produces a panel farm or a slideshow: what the system *is*, what stays
the *same* as it changes clothes, and how each change is a *move* and not a cut.

If you cannot fill a field in one or two lines, you have found the hole in your
design — fill it before you code. See the doctrine in
[`AI_VISUAL_AUTHORING.md` §6](AI_VISUAL_AUTHORING.md#6-directed-representation-the-atlas).

Once this is filled, copy the compact
[`recipe_atlas_skeleton.rs`](../examples/recipe_atlas_skeleton.rs) and grow it into
your plan — reach for the full exemplars (`recipe_identity_transport.rs`,
`recipe_directed_atlas.rs`) only when you need a specific implementation detail.

---

## The template

```yaml
semantic_model:        # Layer 1. The finite system, as DATA. One model, shared by
                       # every representation. No pixels here.
  # e.g. a 4-node job graph + its message routes.

anchors:               # 2–5 persistent identities. An anchor = a semantic identity
                       # carried by ≥2 REDUNDANT channels, so it survives a basis
                       # change AND Mono. A stable position is just ONE possible
                       # channel — NOT required. Colour alone NEVER suffices.
  - id:
    semantic_role:     # what this identity IS in the model (not how it looks)
    channels:          # pick ≥2 from: glyph/signature · accent · silhouette ·
                       # topology · adjacency/relation · trajectory · causal role ·
                       # recurring motion · ordering · spatial position
      - ...
      - ...
    manifestations:    # how this anchor appears in EACH basis (one line per basis)
      basis_a:
      basis_b:
  # (repeat for each anchor)

representations:       # The genuinely DISTINCT visual bases, in order. Name the
                       # LibGibson primitive each uses; if two share a primitive and
                       # a layout, they are not distinct — merge or cut one.
  - id:
    primitive:         # Node::canvas / RgbRaster / Surface / Camera / Scene
    what_it_shows:     # how THIS basis renders the model + where each anchor lands

transitions:           # One entry PER ADJACENT pair. A transition is a MOVE.
  - from: -> to:
    mechanism:         # visible transport (conduit / shared coordinate basis /
                       # morph) — NOT a page-swap
    anchor_transport:  # PER ANCHOR: how the viewer can follow THIS identity across
                       # the seam (which channel bridges it — a travelling glyph, a
                       # shared site, a preserved adjacency, a continuous trajectory)
      - anchor_id:  how:
    kind:              # continuous | declared-hard-cut (hard cuts must be explicit)

director:              # frame = f(t). Time is EXPLICIT STATE, never a paint-time
                       # clock. List the cue sheet.
  cues:
    - act:  start: end:
  seekable: true       # if false, say why; you lose determinism and testability

shots:                 # OPTIONAL attention grammar WITHIN bases. Where does the eye
                       # go, and when? Tie each to an anchor where possible.
  - at:  focus:        # time/window -> which anchor or region

responsive_fallback:   # What is Essential / Secondary / Tertiary? Small sizes drop
                       # Tertiary and KEEP the hero. Mono carries meaning on shape +
                       # luminance (use to_mono_surface for rasters), colour only
                       # enhances.

acceptance_keyframes:  # The exact (time, size, depth) frames you will RENDER AND
                       # LOOK AT. Include establish / each transition midpoint /
                       # reveal / hold / payoff, at least one boundary pair
                       # (b-ε, b, b+ε), and ≥1 Mono. Tests check structure; your
                       # eye checks spectacle.
  - t:  size:  depth:
```

---

## Structural invariants (mechanizable — assert them so regressions can't rot the piece)

The four `recipe_directed_atlas.rs --selftest` checks directly (copy them):

- **director determinism** — `frame(t) == frame(t)`;
- **direct seek** — `frame(seek→t) == frame(run→t)` (keep the clock in state);
- **boundary continuity** — no identity teleports across a cue boundary (any hard
  cut is *declared*);
- **frozen-hold zero-diff** — a held frame emits 0 cell / 0 byte delta.

Two more are mechanizable but the atlas selftest does **not** cover — assert them
yourself where they live, and don't claim the selftest does it:

- **anchor-mapping totality** — every anchor in `anchors:` has a `manifestations:`
  entry in every basis (check this list against the code);
- **editorial locality** — a bounded overlay mutates only its own region. This is
  the `CINEMATIC OVERLAY` contract: `recipe_cinematic_overlay.rs` demonstrates it
  and the intro's reaction compositor enforces it — verify by capture + the overlay
  pattern, not via the atlas selftest.

## What stays human (do not pretend a test covers these)

Whether the transitions read as *transformations of one system*, whether identity
is actually *felt* across a basis change, and whether the whole thing has impact,
beauty, legibility, surprise and payoff — these are **art direction**. Render the
`acceptance_keyframes` and look. Expect several passes. The structure above gets
you a coherent world that transforms; your eye gets you the rest.
