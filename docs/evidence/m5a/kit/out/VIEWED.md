# M5-A checkpoint kit: session viewing record (e424e09, 2026-11-26 22:41:45-22:42:54Z, heavy s)

The session opened every PNG listed here: the 15 stills, the contact sheet, and the first, middle
and last frames of the strip. Leah's verdict is still OPEN, and GOD batches the kit to her.

Capture setup: Xvfb :97, XTerm(390) at 80x30, DejaVu Sans Mono 12, TERM=xterm-256color, truecolor.
Determinism: `g1-0420-start-f2` was captured twice and the sha256 values are identical.

## What was seen, per behaviour

**G1 FrameSequence: correct.**
- f0 at 0 ms: arms down, grey band.
- f1 at 300 ms: left arm up, red band.
- f2 at 420 ms and at 650 ms mid-hold: both arms up, green band.
- f3 at 920 ms: right arm up, yellow band.
- Past the 1120 ms total, and late at 1500 ms: rest frame f0, with `next_in=null`.
- The contact sheet at 20 fps switches exactly at 300, 420 (first seen at 450), 920 (first seen
  at 950) and 1120 (first seen at 1150), then holds rest.
- No wrong frame, and no late-sampling replay.

**G1 Reduced: correct.** At 420 ms it shows the rest frame f0, not f2, with no deadline.

**G2 LayerSlots: correct.**
- The eye fill (cyan) and the mouth fill (magenta) sit at their slot offsets.
- The mouth is drawn over the eyes, as declared: the higher z paints later.
- With the slots unfilled, the base shows through (a blank face).
- No slot misplacement and no z-order inversion.

**G3 CueSchedule: correct.**
- The printed cues are k0..k7 = 577, 2105, 3046, 3983, 5494, 6579, 7720, 8844 ms, which equal the
  independent Python `cues_ms_python` in the manifest.
- The ruler ticks sit at x = ms*79/10000 cells.
- The eyes are closed (blinking) at cue 2 (3046 ms) and open at +400 ms and at the origin.

**G4 hidden: correct.**
- After the plane is omitted, there is no sprite and no stale pixels.
- The stderr shows `next_deadline_hidden=null`, `idle_run_once_none=true` and `idle_bytes=0`.
  These byte and deadline numbers are PROXY evidence from a headless twin Context.
- `hidden_bytes=1540` is the one write that erases the sprite.

**after-exit: correct.** The normal screen shows only `RESTORED-OK` and the cursor, with no
residue and no alt-screen leak.

## Defects seen, by name

1. **Glyph colour fringe (terminal-side, cosmetic).**
   - Seen in the G3 open-eye stills, at the top cell row of each pupil: a 1-px light grey,
     cyan and yellow fringe where a white-over-black half-block cell meets its neighbours.
   - The cause is xterm's Xft subpixel antialiasing of the half-block glyph. It is not libgibson
     output: the cell colours are exact.
   - Fix in the kit, not the library: run xterm with grayscale/no-subpixel AA.
2. **Kit label: the legend's colour name is wrong (cosmetic).** The G3 legend says
   "orange=nearest cue", but that tick is drawn yellow.
3. **Kit label: the unit is printed after null (cosmetic).** The header shows `next_in=nullms`
   when there is no deadline.
4. **Kit manifest: `cues_match_python` is null.** The cross-check was never computed. I compared
   the values by eye instead (G3 above).
5. **Coverage limit, not a defect: the strip is not live playback.** Every strip tile is a
   separate process sampled at a fixed time. That proves time sampling only. It does NOT test
   live frame pacing, popping or jitter during continuous playback. A live-run capture is a
   follow-up if Leah wants it.

None of these are defects in libgibson's G1–G4 behaviour.
