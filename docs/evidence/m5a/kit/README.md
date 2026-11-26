# M5-A visual checkpoint kit (G1-G4)

Real-terminal screenshots of the experimental `gibson::stage::motion` behaviours, from
`examples/stage_motion_kit.rs`. Every shot is one fixed, synthetic frame (a 32x48 px blocky
figure, no external assets) presented through `PresentationDriver::resolve_frame` into an
alt-screen `Context` at a fixed 80x30 viewport, captured from a real `xterm`.

## Run (under the heavy grant; needs xterm, xdotool, ImageMagick, ffmpeg, Pillow, Xvfb)

    cargo +1.98.1 build --example stage_motion_kit
    docs/evidence/m5a/kit/run_kit.sh --example-bin target/debug/examples/stage_motion_kit \
        --out docs/evidence/m5a/kit/out --only=stills     # ~1 min
    docs/evidence/m5a/kit/run_kit.sh --example-bin target/debug/examples/stage_motion_kit \
        --out docs/evidence/m5a/kit/out --only=strip      # ~1.5 min, 27 captures

`run_kit.sh` starts a private `Xvfb :97` (`--display=:N` to change), caps the run at 480 s, and
kills Xvfb and every xterm on exit. Output: `out/stills/*.png`, `out/g1_contact_sheet.png`,
`out/g1_strip_{first,mid,last}.png`, `out/g1_strip.mp4` (never committed), `out/manifest.json`
(git sha, per-shot command, seed, frame times, xterm geometry/font, tool versions, sha256 and
sizes, `still_png_total_kb`, and `determinism.identical`). Each shot also leaves its stderr JSON
(`scenario, at_ms, frame, next_in, next_deadline`) in the manifest; `frame_ok` compares the
reported frame with the expected one.

## Fixtures

- G1 sequence: 4 frames, holds 300/120/500/200 ms (starts 0/300/420/920, total 1120), rest = 0.
  Frame i has a 1-row colour band across the top: 0 white, 1 red, 2 green, 3 yellow; arms: f0
  down, f1 left arm up, f2 both up, f3 right arm up (the figure's left is screen left).
- G2 slots: `eyes` px (10,10,12x6) and `mouth` px (12,14,8x4); they overlap on rows 14..16. Fills:
  eyes cyan, mouth magenta. Mouth is slot 1, so it is drawn above eyes.
- G3 cue: seed 0x5eed, cue 1, origin 300 ms, period 1200 ms, min_gap 500 ms; the first 8 times
  are printed on screen and cross-checked against an independent Python SplitMix64
  (`cues_match_python` in the manifest). Blink = eyes closed (thin line) for 150 ms after a cue.

## What to look for

- `g1-0000` .. `g1-0920` (start of each frame): the band colour and arm pose match the frame in
  the title line (`frame idx=N`): white/down, red/left-up, green/both-up, yellow/right-up.
  Defects: WRONG FRAME (pose or band disagrees with idx, or idx disagrees with at-ms: 300 must
  be frame 1, not 0; boundaries are half-open); off-by-one at a hold boundary.
- `g1-0650` mid-hold: still frame 2 (green, both arms up), next_in = 270 ms. Defect: advancing early.
- `g1-1120`, `g1-1500` (past total): frame 0 at rest (white band, arms down), `next_in=null`,
  `next_deadline` null. Defects: sticking on the last frame, replaying, a non-null deadline.
- `g1-reduced-0420`: at 420 ms, which would be frame 2 in Full motion, frame 0 shows with no
  deadline. Defect: any motion under Reduced.
- Strip (`g1_contact_sheet.png`, `g1_strip.mp4`): frames step white, red, green, yellow, white at
  the stated times, once, then freeze. Defects: POPPING (a frame shown out of order or for a
  tile where it should not be, a tile showing a blend or half-drawn sprite), JITTER (the body or
  the label moving between tiles; only arms and band may change; the head, torso and legs must be
  pixel-identical in `g1_strip_first/mid/last`), a loop restart after the last frame.
- `g2-unfilled`: base only: skin-coloured head with NO face, no cyan or magenta. Defect: stray fills.
- `g2-slots`: a cyan block over the head's upper face (12x6 px) and a magenta block below it
  (8x4 px), centred; where they overlap (2 px rows) the MAGENTA wins. Defects: SLOT
  MISPLACEMENT (fills shifted by the head offset or off the face by even 1 px), Z-ORDER
  INVERSION (cyan visible in the overlap rows), a fill clipped or scaled.
- `g3-*`: ruler along the bottom with 8 blue ticks, one orange (nearest cue), a white cursor at
  at-ms; the title line names the nearest cue and `blinking`. At `--at-ms` = cue 2 the eyes are
  a closed thin line; at cue 2 + 400 ms they are open (white with black pupils); at 0 open.
  Defects: blink on the wrong cue, tick positions not matching the printed times
  (x = ms*79/10000 cells), cursor off.
- `g4-hidden`: no sprite and no stale pixels where it was (it was shown for 400 ms first, then
  hidden); stderr shows `next_deadline_hidden=null`, `idle_run_once_none=true`,
  `idle_bytes=0`. **These byte and deadline numbers are PROXY evidence**: they come from a
  headless twin `Context`, because the real alt-screen sink cannot be captured. The screenshot is
  the direct evidence. Defect: STALE FRAME AFTER HIDE (a ghost of the figure left in the terminal),
  a non-null deadline, a non-zero idle write.
- `after-exit`: the xterm shows only `RESTORED-OK` on the normal screen with the prompt-less
  shell. Defects: RESIDUE/GARBAGE after exit (sprite cells, stray escape text, alt screen left
  active, cursor hidden), which would also show as a missing/odd cursor.
- Manifest `determinism.identical` must be true. If false, record the differing region by hand
  (likely the xterm cursor or font hinting) before trusting any per-shot diff.

Stills are small; commit them only if `still_png_total_kb` is about 500 or less. The MP4 and
`out/strip/` are git-ignored (`out/.gitignore`).
