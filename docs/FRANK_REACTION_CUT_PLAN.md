# Filthy Frank reaction cut — LibGibson intro v2

**Status:** planning/hardening artifact on `feat/temporal-visual-supremacy`. This is not part of main and is not a release claim.

## Goal

Build a second flagship version of `libgibson_intro` in which the keyed greenscreen subject from the local Filthy Frank source clip appears to react to the existing LibGibson short film as it unfolds.

The result should not feel like a video pasted into a terminal. It should feel like one deliberately edited short:

- the original 72-second LibGibson narrative remains semantically recognizable;
- the reaction subject appears, disappears, punches in, holds, stretches/morphs, and changes framing at intentional story beats;
- edits follow an absurdist internet-comedy grammar: smash cuts, sudden punch zooms, deadpan holds, occasional stutter/freeze, deliberate scale/aspect changes, and hard cutaways — **not** constant random glitch;
- the keyed green field is transparency, never content;
- one renderer remains authoritative; the final product is still ordinary LibGibson `Surface` output.

The demo should be a genuine stress test of the new keyed-video + temporal/incremental machinery and a release-quality flagship, not a one-off hack.

## Local source

Expected source in the maintainer's Downloads directory:

```text
~/Downloads/Filthy_Frank_It_s_time_to_stop_full_template-1ELeFgM1EOU.webm
```

If that exact name is absent, search `~/Downloads` for a `*.webm` whose lowercase name contains `frank` and `stop`.

Measured source facts from the supplied clip:

- VP9 video + Opus audio;
- 1280×720;
- 29 fps;
- ~301.721 seconds;
- bright green is intentional greenscreen transparency.

Do not commit the source media to the repository.

## Maintainer source cue sheet

These are the maintainer's source-video candidate windows. Preserve them durably even if the final edit chooses a shorter inner trim. Re-inspect the actual source visually before locking the exact in/out frame.

| Source time | Maintainer label / intent |
| --- | --- |
| 0:00–~0:15/0:17 | intro / welcome; replace the source-show framing with LibGibson framing; opening hostile/comedic welcome |
| ~0:46–~0:50 | prank1 |
| 0:58–1:30 | long rant window; mine several strong visual microcuts rather than assuming all 32 seconds belong in the final cut |
| 1:40–1:56 | stop1 |
| 3:08–3:19 | stop2 |
| 3:19–3:29 | stop3 |
| 4:04–4:16 | quiet WTF reaction |
| 4:35–4:46 | nice prank reaction |
| 4:46–5:01 | idiot / outro reaction |

The labels are editorial shorthand, not verbatim transcript requirements.

## Existing LibGibson intro timeline

The original film remains:

| Intro time | Act |
| --- | --- |
| 0–20 s | Agent harness |
| 20–28 s | Membrane |
| 28–36 s | Information city |
| 36–44 s | Facades |
| 44–53 s | Couriers |
| 53–60 s | Ascent |
| 60–72 s | Earth / title / final hold |

The reaction cut may have a longer **edit clock** than 72 seconds. Keep the original intro's narrative clock separate from the reaction-edit clock so a reaction can freeze/slow/continue the base film without corrupting the existing deterministic intro semantics.

Conceptually:

```text
reaction edit time
      │
      ├── maps to intro narrative time (continue / hold / slow)
      └── maps to source reaction clip time
```

Do not mutate the original `Director` into a general video editor. A second demo-local reaction director/cue sheet is preferable.

## Suggested story mapping

This is art direction, not a byte-locked schedule. Re-inspect the source windows and choose the strongest inner trims.

### Opening / harness — source 0:00–~0:17

Use the source intro/welcome as the cold-open reaction layer while the LibGibson first-contact / harness establishes itself.

- Replace/cover source-specific show framing with LibGibson-native title/chrome rather than trying to preserve source titling.
- Begin relatively normal-sized so the audience understands there is a keyed human reaction layer.
- A hard punch-in or full-frame interruption can land at the first strong reaction beat.
- Do not bury the harness information for the whole opening.

### First impossible transition — source ~0:46–0:50

Use `prank1` around the harness → membrane transition near intro 20 s.

This should be a short, sharp edit: the system starts physically reinterpreting the harness and the reaction cuts in as if the transformation itself were the prank.

### Harness / membrane / city escalation — source 0:58–1:30

This is a long source window. Do not simply play it linearly.

Mine several visually strong microcuts from it and interleave them across:

- late harness escalation;
- membrane distortion;
- early information-city reveal.

Use different composition states coherently:

- normal side reaction;
- sudden punch zoom;
- full-height keyed interruption;
- brief aspect squash/stretch or other finite morph;
- return to normal.

The visual grammar should feel intentionally edited, not continuously distorted.

### City reveal — source 1:40–1:56

Use `stop1` around the 28–36 s information-city reveal.

The reaction subject can partially occlude the city, but prefer framing that leaves enough of the impossible wireframe architecture visible to create the joke: the reaction is responding to the thing we actually want to show off.

### Facades — source 3:08–3:19

Use `stop2` around the 36–44 s facade sequence.

A picture-in-picture / edge-anchored keyed reaction is likely better here than a full-screen cut because the mounted micro-UIs are the point.

### Couriers — source 3:19–3:29

Use `stop3` around 44–53 s courier routing.

Let the reaction track the courier action spatially if it looks good: e.g. anchor the subject to the opposite side of the current camera attention, or use a punch-in at a courier handoff.

### Ascent — source 4:04–4:16

Use the quiet WTF window at the 53–60 s pullback.

This should be one of the calmer edits. Let the subject sit relatively still/small while the city becomes a planet. The restraint is part of the timing.

### Earth/title payoff — source 4:35–4:46

Use `nice prank` as the Earth/title lands.

This is a strong place for a clean keyed reaction over the planet/title rather than a diagnostic-looking box.

### Final sting / outro — source 4:46–5:01

Use the final `idiot` window as the sting after the title locks.

The base intro may hold at its final frame while this reaction plays; the reaction cut's edit clock can therefore extend beyond the base film's 72-second narrative clock without modifying the original film.

End deliberately. No infinite loop unless explicitly requested.

## Compositing architecture: preserve the original intro outside the reaction bbox

The new reaction demo should **not** run the whole `libgibson_intro` Surface back through the RGB/Braille projector every frame. That would degrade terminal-native text/geometry for no reason.

Preferred pipeline:

```text
original libgibson_intro Surface
        │
        │ exact and unchanged outside reaction dirty region
        ▼
union(previous reaction bbox, current reaction bbox)
        │
        ├── reconstruct underlying intro cells into logical subpixel RGB
        ├── sample keyed/scaled/transformed reaction foreground
        ├── alpha composite per logical subpixel
        └── project ONLY those touched cells back through the two-color projector
        ▼
patched final Surface
        ▼
existing differential ANSI renderer
```

This preserves the original intro exactly outside the overlay.

### Base-Surface subpixel reconstruction

For touched cells only, reconstruct a reasonable logical 2×4 RGB basis from the existing cell:

- Braille glyph: decode its 8-bit mask exactly; each dot chooses fg or bg.
- Space/empty: background across all logical subpixels.
- Half-block / block / known subcell glyph families: decode according to their known geometry.
- Ordinary textual/unknown glyph: use a conservative documented fallback, likely foreground-dominant coverage or a simple cell-level treatment. This is acceptable inside an overlaid subject region, but prove it does not corrupt cells outside the overlay.

If a small reusable helper emerges naturally, consider a general logical-subcell sampler for a `Surface`; do not force it into CORE without evidence.

## Reaction transform model

Create a small demo-local transform vocabulary rather than a general video editor framework.

A cue may specify:

- source in/out time;
- intro anchor time;
- base-film time policy: `Continue | Hold | Slow(factor)`;
- placement anchor;
- scale;
- x/y offset;
- horizontal mirror if editorially useful;
- aspect scale x/y;
- finite effect envelope.

Useful finite effect primitives:

- `Normal`
- `PunchZoom`
- `SmashIn`
- `AspectStretch`
- `QuietHold`
- `FreezeStutter`
- optional bounded chromatic/scanline disturbance at a specific beat

Every effect must have a deterministic time function and a clear beginning/end. No random permanent glitch soup.

Use inverse sampling for scaled/morphed reaction imagery so transforms do not punch holes in the subject.

## Aspect ratio

Do not blindly stretch 16:9 source pixels to whatever terminal logical-subpixel grid exists.

Add a deliberate mapping policy for the reaction subject:

- preserve source aspect by default;
- explicit cover/crop only when an edit calls for it;
- account for the logical 2×4 Braille sampling grid;
- visually inspect real terminal results because physical terminal cell aspect remains emulator/font dependent.

This is especially important for facial reaction shots.

## Chroma key / alpha hardening

The current key is deliberately simple and works on the reference clip. Before release:

- visually inspect matte edges across all selected cue windows;
- preserve soft alpha;
- tune spill suppression if skin/clothes are being damaged;
- consider linear-light alpha compositing for the reaction layer if it materially improves edge quality;
- do not overfit to one frame while breaking another cue.

Keep key parameters deterministic and reproducible.

## Video-cache hardening

The current example cache key is based on filename/spec. Harden it before release so a different file with the same name/spec cannot silently reuse stale RGB.

Include a source fingerprint such as a stable combination of:

- canonical path;
- file size;
- modification time;

or a bounded content hash if justified.

For this reaction demo, bake only the required cue windows. Do **not** decode/cache the entire ~302-second source unless measurement proves that is actually sensible.

A cue-cache/segment-cache should make startup predictable and keep memory bounded.

## Deterministic reaction timeline

The current general compositor uses wall-clock elapsed time to choose source frames. The reaction flagship should instead be deterministic and seekable.

Use an explicit edit clock.

Given edit time, the director should deterministically compute:

- base intro narrative time;
- active reaction cue (if any);
- source frame;
- transform;
- placement;
- whether the base intro continues, freezes, or slows.

Required tooling:

- `--at=<edit seconds>`
- `--freeze`
- `--stage=<reaction cue name>` or equivalent
- deterministic dump/capture of selected reaction beats
- replay exactly reproduces the same frame

This is essential for tests and polish.

## Audio

Do not introduce an audio dependency into LibGibson core.

The flagship must work visually without synchronized audio.

If optional source audio playback is explored, keep it strictly example-local / external-tool driven and do not let it block the visual release. The core acceptance test is the rendered reaction edit.

## Hardening before merge

PR #67 is currently a draft research branch. Before it becomes merge/release material:

1. Resolve issue #68: `set_target_region` must not perform O(total cells) eligibility/active rescans for every small region update. Keep a full-grid reference/oracle test and prove incremental accounting matches it.
2. Audit/cache-source identity as described above.
3. Audit empty/all-key and subject-enter/leave edge cases.
4. Add deliberate fit/aspect transform semantics.
5. Stress resize during keyed playback and reaction cut.
6. Stress terminal color capabilities. Temporal safety already resolves through quantized colors; the reaction flagship still needs graceful static fallback.
7. Ensure moving reaction subject never accumulates stale temporal residual.
8. Ensure a reaction leaving the screen restores the original intro Surface exactly in the vacated region.
9. Keep the original `libgibson_intro` example behavior and existing goldens byte/semantically unchanged.
10. No user source clip or decoded cache artifacts are committed.

## Flagship acceptance tests

The new demo should include synthetic/hermetic tests for:

- alpha key and spill behavior;
- transformed bbox;
- source aspect-fit mapping;
- bbox union clearing;
- reaction overlay entering/leaving;
- transformed overlay over a known base Surface;
- untouched base cells remain bit-identical;
- sparse overlay composite equals a full reference composite inside the dirty union;
- deterministic edit-time seeking;
- hold/continue/slow intro-time mapping;
- cue-boundary continuity or deliberate smash-cut semantics;
- no stale residual after moving regions settle/reacquire.

Real source visual inspection remains a human/art-direction receipt, not CI input.

## Release direction

The existing branch already contains genuine v0.3.x correctness fixes plus substantial experimental keyed-video work.

After the hardening + reaction flagship:

- re-evaluate whether the coherent release is `v0.3.1` (hardening/additive experimental work) or whether the breadth honestly deserves a deliberate `v0.4.0` graphics/compositing milestone;
- do not choose the larger number merely because the demo is funny;
- use the release contract and actual public API delta.

Whatever the version, merge/release only after full exact-tip CI, release preflight, docs reconciliation, and a real visual review of the reaction flagship.
