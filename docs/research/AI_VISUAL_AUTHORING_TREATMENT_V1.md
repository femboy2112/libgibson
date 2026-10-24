# Visual-authoring treatment — v1 snapshot (frozen evidence)

This records the exact bytes of the **v1** visual-authoring doctrine — the
nine-file overlay that the completed `v0.4.0` blind holdout's TREATMENT subjects
were given. It exists so that strengthening the doctrine (v2) can never erase the
record of what v1 actually was.

- **Repo commit:** `58f3385899170fc665718aecef94cfa7c6fcc3d7`
  (overlay bytes identical to the earlier tip `e7028b4`; the later commit touched
  only the holdout doc, which is not part of the overlay).
- **sha256 of the nine overlay files at that commit** (verified on disk
  2026-10-02, byte-for-byte equal to the lab sanitization record):

```
c534a1dca68f06e8032e51f8673ed4cd54d7f25f0ae34cee0c44c06f397a0dcf  AGENTS.md
fab7590416c17ae0fe5dd1f2a6fc9bb3c3493a752c8aa0e0bf8cb03e59d14bab  docs/AI_VISUAL_AUTHORING.md
289e9da6189c2c1b9e9f9a06e2e4eb6a92dc387cd91970e51c0697a132486f8d  docs/UI_LAYER.md
002b302fb3c9cb9b64dc6d3a3c2637d8bd5e7418c02739612fdcb8aab23767d5  examples/recipe_hero_with_hud.rs
0fb3c8a6ad18b23c992e3ded0500dcf7af6f3520f626445a693bda9cd07861d9  examples/recipe_continuous_world.rs
6b52184f8abd63f07b97b6120e796331675e3c4f9b841bc5dfbb9efb0605f43c  examples/recipe_semantic_zoom.rs
035bcd625156a8b3769b6c71028b53459f80626c78d264a04a9b4941ef997fd6  examples/recipe_cinematic_overlay.rs
deae9b39a512bcb6c4a2a08a424ce7b353653ce3b556bc00a01e10b3b09e76e5  examples/recipe_capability_safe_canvas.rs
22b7e59d3ad562ebc0f74b4138f4800faf5124230d726e6570d614c891ffd9dc  examples/recipes_support/mod.rs
```

## What v1 established, and what it did not

v1 is a strong, correct cure for the **Panel Farm** — the world-first composition
problem: model the domain (layer 1), state a visual law (layer 2), realize it with
a dominant object that owns the frame (layer 3), and *look at the rendered frame*.
The completed holdout tested whether that intervention changes a fresh non-Sonnet
agent's behavior.

v1's blind spot, identified by reading the flagship `libgibson_intro`
(`docs/INTRODUCTORY_CINEMA.md` + `examples/libgibson_intro/`): it teaches a
**single coherent world**, and its `CONTINUOUS WORLD` law even states "one
coordinate space, one camera, you never swap pages." The flagship deliberately
does the opposite — it changes visual basis seven times — while keeping the four
agent identities continuous across every basis. v1 has no vocabulary for *directed
representation change with identity transport*. v2 adds exactly that, as the
smallest delta, and is honest that it buys **coherence**, not spectacle (the
flagship's beauty rides on human art direction its own tests decline to certify).

The v2 delta is additive. These nine files are either unchanged or extended; two
new recipes and a planning template are added. See
`docs/AI_VISUAL_AUTHORING.md` §"Representation atlas" and
`docs/AI_VISUAL_ATLAS_PLAN.md`.
