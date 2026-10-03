# The Visual Atlas Plan (authoring template)

Fill this in **before** you write paint code for any cinematic or
multi-representation LibGibson piece. It is an *authoring* artifact — LibGibson
never reads it at runtime. Its whole job is to make you state the reasoning that,
skipped, produces a panel farm or a slideshow: what the system *is*, what stays
the *same* as it changes clothes, and how each change is a *move* and not a cut.

If you cannot fill a field in one or two lines, you have found the hole in your
design — fill it before you code. See the doctrine in
[`AI_VISUAL_AUTHORING.md` §6](AI_VISUAL_AUTHORING.md#6-directed-representation-the-atlas).

---

## The template

```yaml
semantic_model:        # Layer 1. The finite system, as DATA. One model, shared by
                       # every representation. No pixels here.
  # e.g. a 4-node job graph + its message routes.

anchors:               # 2–5 persistent identities. Each MUST carry ≥2 redundant
                       # channels so it survives a basis change AND Mono.
  - name:
    accent:            # colour (enhancement only)
    glyph:             # one-cell signature — survives Mono
    site:              # a stable normalized position the next basis can consume
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
    anchors_preserved: # which anchors are recognizable across the seam, and how
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

## Invariants worth a test (copy `recipe_directed_atlas.rs --selftest`)

These four are *mechanizable*; assert them so regressions can't rot the piece:

- **director determinism** — `frame(t) == frame(t)`;
- **direct seek** — `frame(seek→t) == frame(run→t)` (keep the clock in state);
- **frozen-hold zero-diff** — a held frame emits 0 cell / 0 byte delta;
- **editorial locality** — a bounded overlay mutates only its own region.

## What stays human (do not pretend a test covers these)

Whether the transitions read as *transformations of one system*, whether identity
is actually *felt* across a basis change, and whether the whole thing has impact,
beauty, legibility, surprise and payoff — these are **art direction**. Render the
`acceptance_keyframes` and look. Expect several passes. The structure above gets
you a coherent world that transforms; your eye gets you the rest.
