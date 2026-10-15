# Operation: Coherence Lock → Smooth Pop — living evidence ledger

**Branch:** `impl/humanmusic-smooth-pop-2026-10-14` (worktree `/home/leah/LibGibson.smoothpop`)
**Parent:** `b536699ed97c6db4320874d4cf31fb229e90cf25` (PR #114, Johnny Five head; CI run 38513193497 = SUCCESS)
**Plan branch:** `research/humanmusic-c137-freeze-smooth-pop-2026-10-14` @ `dadbcf5`
**Plan docs:** `docs/research/HUMANMUSIC_C137_COHERENCE_LOCK_SMOOTH_POP_PLAN.md`,
`docs/research/HUMANMUSIC_SMOOTH_POP_CLAUDE_HANDOFF.md`

> **Three epistemic lanes, never merged.** CI green and a hash match are machine evidence,
> not musical acceptance. The maintainer's ear is the only promotion gate. Nothing is
> promoted, default-changed, un-drafted, merged, released, or published without her explicit word.

Legend: **[OBSERVED]** = run on the real target this session, output recorded ·
**[EAR]** = maintainer listening verdict · **[PROPOSED]** = designed, not yet built/verified.

---

## First implementation cut (plan priority): R0 → R1 → R2

Everything after R2 (R3 sparse lead + 2×2, R4 Johnny-Five choice + the `kernel.rs:1445`
Keys-anchor bug, R5/R6 32-bar song + ear packet) is gated on this slice genuinely passing.

---

## R0 — immutable acoustic lock (C137_ACCEPTED_V1)

**Goal:** freeze the accepted C137 as an *external-to-candidate* reference (Score canonical
fingerprint **and** WAV SHA-256), with red-first mutation calibration proving each witness bites.

### [OBSERVED] candidate reproduces accepted parent — Full default, 16-bar
Rendered from the fresh `impl/humanmusic-smooth-pop-2026-10-14` binary (built off `b536699`):

```
c137_song --seeds=2112,7,99 --out=<fresh>
```

| seed | `full.normal.wav` SHA-256 | internal Score fp | frozen? |
| --- | --- | --- | --- |
| 2112 | `86a9cf82f5498f2ffa25f3a4f7ac48a187b2cd0ad468eb1caf0905cb1e4c449b` | `ed1815c7f040884a` | ✅ matches baseline |
| 7 | `6d2e42ede99c38ab48c5a071d3ffc0624c0215b875ec0b085878c2deb5af0fbc` | `4fb4458284b6fbf2` | ✅ matches baseline |
| 99 | `dfb0320df07af7f7cf3de6e2056bf42127dc37f30076dbf0001dde25a9824683` | `22a9240d6419806c` | ✅ matches baseline |

Per-run output layout: `<out>/manifest.txt`, `<out>/seed-<N>/full.normal.wav`,
`<out>/seed-<N>/score.tsv`. (Stems: via a flag TBD by recon.)

### [OBSERVED] R0 COMPLETE — the lock exists and bites
Finding that reframed R0: there was **no WAV-byte lock anywhere** before this. Every C137 test is
relational (A==B rebuilds); the "frozen fingerprints pinned in the PCM gates" comments in
`tests/audio_c137_song.rs` describe a gate that did not exist. R0 builds it from zero.

**Committed:** `docs/fixtures/humanmusic-c137-accepted-v1/` — `SHA256SUMS` (51 artifacts: WAV +
`score.tsv` across 7 levels × 3 seeds), `MANIFEST.tsv` (level, scope, flags, seed, tempo, block,
Score fp, frames, WAV sha), `README.md`. WAVs are local-only by repo policy; only hashes committed.

| Level | corpus | scope |
| --- | --- | --- |
| A1 | 8-bar short/plain (`--jerry=short,plain`) | archival control, NOT ear-music |
| A2 | 16-bar plain (`--jerry=plain`) | characterized control |
| B | 16-bar Full (default) | **[EAR] accepted (PR #111)**, provenance-verified |
| C1/C2 | 32-bar `--form=song` / `song-intro` | characterized controls, NOT ear-approved |
| D1 | 32-bar `--form=song-narrative` | **[EAR] accepted** ("sounding MUCH BETTER :D") |
| D2 | 32-bar `--form=song-intro-narrative` | **[EAR] accepted** ("hell yeah") |

**Verified this session (run, not pretended):**
- **Provenance:** level B rendered by the candidate binary AND the accepted parent `f68c9bf` binary
  → byte-identical on all 3 seeds. `C137_ACCEPTED_V1` *is* the ear-accepted music, not "today's build".
- **Acoustic lock reproduces:** `scripts/verify-c137-accepted-v1.sh` → all 51 artifacts OK, exit 0.
- **Red-first WAV layer:** `--calibrate` flips one PCM byte → check FAILS as required, exit 0.
- **Red-first Score layer:** `tests/audio_c137_accepted_v1_lock.rs` 7/7 — witness moves on
  pitch/onset/dynamic/drum/harmony, stays blind to tempo/sections (the gap the WAV layer closes).
- **Regression:** `audio_c137_song` 58/58, `audio_canonical_fingerprint` 5/5. `fmt` + `clippy -D` clean.

Two-layer design: CI-fast Rust calibration (no render) + out-of-band acoustic script (real binary).
`cargo test` does not render audio; the acoustic step is deliberately separate, per the plan.

---

## R1 — style-neutral lawful core (opt-in SourceDomain) — [OBSERVED] DONE
The "domain" was not a type — it was one predicate (`longform_domain`, `kernel.rs:911`) with the
tonic law baked as literal tuples at the tail. And the C137 longform harmonic *witness* is vacuous
(`harmonic_violations` only fires on steps with a `completion`, which C137 songs lack) — so today
the real longform harmonic gate is the route pins alone.

**Seam (additive, opt-in):**
- `argument::SourceDomain { C137, SmoothPop(PopSourceGrammar) }` + `PopSourceGrammar { loop_bars }`,
  new `pub domain` field on `CompiledArgument`, defaulted to `C137` in `compile()` (the one literal).
- Extracted the route-pin tail **verbatim** into `kernel::c137_route_law`; added `pop_route_law`
  (the route is a whole number of exact loop repetitions — groove-cycle closure — not a literal-tonic
  arrival). `longform_domain` now dispatches `match argument.domain`. Everything else (shape,
  validate, relations, carriers, lattice, source-transform equality) is shared and untouched.
- Closure distinction made executable: C137 = tonic/harmonic closure; SmoothPop = groove-cycle
  closure. The Aeolian-diatonic pop loop (Am9–Fmaj7–Cmaj7–G6, i–VI–III–VII) keeps `theme_family`'s
  scale pool valid, so no melody-generator rewrite.

**Verified:** route-law unit tests (`route_law`) prove each law admits its own kind and REJECTS the
other (the pop loop fails the C137 tonic pins; a drifting/mis-sized loop fails the pop law). C137
**byte-identical** — the lock verifier reproduces all 51 artifacts from the R1 binary. lib 874,
audio_c137_song 58, canonical 5, lock 7, source_authority + conversation all pass. fmt + clippy -D clean.

**Deferred (noted, not hidden):** a stronger independent *sounded-note* pop-harmony witness (read the
Keys/Pad/Bass pitch classes per bar, not the route labels) — C137's own longform witness is vacuous,
so this would raise the bar for pop above what C137 currently has. The shared gates
(source-transform equality + `source_harmony::admits`) + `pop_route_law` are the first-cut gate.

## R2 — band-led pop groove (lead-free, ~104 BPM) — [OBSERVED] rendered; [EAR] PENDING
The first audible proof that the categorical machinery can host a genuinely different groove.

**Seam (additive, opt-in):**
- `GrooveLaw { bass_cell: Vec<(offset, BassDegree, dur, vel)> }` + `BassDegree {Root,Third,Fifth,Octave}`
  on `argument`; new `pub groove: Option<GrooveLaw>` field on `CompiledArgument`, default `None`.
- kernel bass loop: when `groove` is `Some`, the per-bar figure is the authored cell (resolved
  against the bar's chord), keeping the `(0.0, Root)` downbeat anchor; `None` falls through to the
  C137 walk/anchor, byte-identical.
- `examples/c137_song/source_pop.rs`: `source_pop_groove(seed)` — a lead-free source, **two
  Keys-carried `Establish` episodes** (lead lane genuinely empty, not a bus-muted ghost), pop loop
  **Am7–Fmaj7–Cmaj7–G** (i7–VImaj7–IIImaj7–VII, A-Aeolian, all diatonic), `SmoothPop{loop_bars:4}`,
  the syncopated bass cell, a sparse guide-tone keys comp.
- CLI `--form=pop-groove` (own `pop_groove` preset arm, bypasses the C137 capability caps; own honest
  manifest header). Render the pop feel at `--tempo=104`.

**[OBSERVED] render (seed 2112, structure verified in score.tsv):** `lead=0`, `keys=16`, `bass=40`
(5-note cell × 8 bars), `pad=0`, `drums=164`, fp `541690a7b8eb6b0d`. Bass anchors the root on every
bar downbeat (A=45 over Am7, F=41 over Fmaj7) with syncopated octave/fifth/root off-beats; keys are
guide tones (E/C over Am7, A/F over Fmaj7). Counterfactual tests 4/4: lead-free + bass anchors every
bar · the cell drives the bass (≥5 attacks/bar, off-beat inside beat 1) · the pop domain is
load-bearing (same source under `C137` domain is REFUSED) · the foundation law bites (a rootless
downbeat is refused). C137 **byte-identical** (lock 51/51 from the R2 binary). fmt + clippy -D clean.

**[EAR] PENDING — delivered 2 MP3s:** `pop-groove-104bpm.mp3` + `pop-groove-144bpm.mp3` (same groove,
the tempo A/B the pocket-scaling question needs). Nothing promoted.

**Known first-cut nags (honest):**
- Seed-INVARIANT: the groove is hand-authored (keys/bass/loop fixed), so seeds 2112/7/99 render
  identically. Fine for the R2 proof; per-seed variation is later work.
- The octave bass pop clamps at the register ceiling (A's octave → G/b7). Musically a colour, not a
  bug; revisit if the ear dislikes it.
- Drums are the existing C137 kit at 104 BPM (lead-free, so no lead-echo); a bespoke pop kit + more
  conscious negative space is the obvious next refinement if the ear wants it.
- The pop-harmony *witness* is still the route-law + shared gates (sounded-note witness deferred, R1).

## R2.1 — ear verdict → smooth revision (de-cliché harmony + de-robotic groove) — [OBSERVED] rendered; [EAR] PENDING

**[EAR] verdict on the first R2 cut:** *"the chord progression is offensively cliché, and the groove
is there but feels robotic. it needs to be smooth."* Two distinct, correct defects: (1) the loop was
the Axis-of-Awesome i–VI–III–VII; (2) one fixed bass cell stamped on all 8 bars — the pocket was
applied but the monotony read as a machine.

**Recon that reframed the fix (read-only scout, verified file:line):**
- The black_ice A-Aeolian world the pop source renders in has `use_sevenths=true`, `allow_modal_mixture=true`,
  and the default language's `color_depth=2` — so **colour 9ths, modal mixture and secondary dominants
  are all admitted**. The hand-set `route` is **not** conformed on the perform path (`kernel::source_harmony`):
  a chord sounds exactly as authored or is hard-refused (`VOCABULARY_REFUSAL`) — never silently retracted.
- **The carried keys do NOT follow the route.** `keys_comp()` is hand-authored tonic-relative material;
  the chord-derived keys comp runs only under `Consequent`/`Return`, and this source has only `Establish`.
  So a new progression REQUIRES a re-voiced keys comp or the keys clash — and no gate but the ear catches
  it (`observe_argument` returns `_ => true` for `Establish`).
- The octave-pop clamp (`kernel.rs`) hard-clamped `root+12` to 55, sounding a WRONG G for roots pc 8–11.

**The revision (additive, opt-in, C137 untouched):**
- `GrooveLaw.bass_cell` → `GrooveLaw.bass_cells: Vec<Vec<..>>`: the kernel plays `bass_cells[bar % len]`,
  so the groove is FOUR varied per-bar cells (settled statement · walking answer · driving bar · turnaround
  push) with ghost notes and a dynamic contour, not one stamped bar. Every cell still opens `(0.0, Root)`.
- Octave-pop fixed to fold DOWN by whole octaves into the bass register → always the root's own pitch
  class, never a wrong note. (Groove-branch only; C137 never enters it.)
- An ear-gated harmonic **A/B** (`PopFlavor`, `--form=pop-groove` / `--form=pop-groove-lift`), each with a
  keys comp voiced to ITS guide tones (3rd/7th + 9th colour):
  - **Circle:** `Am9 – Dm9 – G9 – Cmaj9` (i9–iv9–♭VII9–♭IIImaj9; circle-of-4ths turnaround, A-Aeolian).
  - **Lift:** `Am9 – Fmaj9 – Dm9 – E7` (i9–VImaj9–iv9–V7; E7 borrows G♯ via mixture — the leading-tone pull).

**[OBSERVED] renders (seed 2112, `--tempo=104`):**
| flavor | Score fp | lead | keys | bass | drums | WAV SHA-256 |
| --- | --- | --- | --- | --- | --- | --- |
| Circle | `4bafe9bc6c2d9366` | 0 | 24 | 46 | 166 | `b024f9c156cdcf07c639d1e30fc6339d3715bc3d808f33fc1937090f32b4a42d` |
| Lift | `634987fdc0f0b5f7` | 0 | 24 | 46 | 166 | `0bcf1e5cdd7217787c3551ed529a257cb1d09b2fa8ba65396a9712c9196495c4` |

`bass=46` = the four cells (5+6+6+6 = 23/episode) × 2. Both lead-free. The two renders are byte-distinct.

**Verified:** `audio_smooth_pop` 5/5 (the four counterfactuals + a new one proving the **Lift** flavor's
borrowed E7 — a G♯ — is admitted and sounded, so modal mixture is load-bearing). C137 **byte-identical**:
`C137_ACCEPTED_V1` lock reproduces all 51 artifacts from this binary. `audio_c137_song` 58/58,
`audio_canonical_fingerprint` 5/5, lock 7/7, kernel `route_law` 2/2. `fmt` + `clippy -D` clean.

**[EAR] PENDING — delivered 2 MP3s:** `pop-circle-104bpm.mp3` + `pop-lift-104bpm.mp3` (same breathing
groove, the harmonic A/B). Nothing promoted. Still open if the ear wants more: a bespoke pop drum kit
(the drums are still the C137 kit, pocket-leaned but a straight backbeat) and a deeper pop-specific lean.

## R2.2 — timbre: the "8-bit beeps" are a palette, not the engine — [OBSERVED] rendered; [EAR] PENDING

**[EAR] verdict (round 2):** *"getting better, but the songs sound kinda static… no forward pop motion.
also… it's hard to make smooth pop with 8bit beeps lol."* Two orthogonal axes: timbre (this section) and
forward motion (next, bigger — see FORWARD-MOTION PLAN below).

**Recon (read-only scout, verified file:line) — the engine is NOT chiptune.** It has polyBLEP
band-limited saw/pulse + a 2-op FM oscillator + noise (`dsp/osc.rs`), a per-voice resonant TPT-SVF
low-pass with a filter envelope, ADSR, unison/detune (`instrument.rs`, `synth.rs`), a memory-bus
chorus, a tempo-synced echo, a 4-line FDN reverb, `tanh` saturation and a master comp/limiter
(`dsp/fx.rs`, `synth.rs`). The "8-bit" sound is the **BLACK_ICE palette**: its keys voice is a 140 ms
`Pulse(0.35)` with **sustain 0.0** — a NES pluck (`world.rs:228-239`). Timbre lives entirely in the
`MusicWorld`; there is no timbre field on the source/`SourceDomain`. And `perform_song(&src, &world)`
and `render(&score, &world)` take **separate world args** (precedent: `examples/vapor95_style_lab.rs`
"palette mode" composes under one world, renders through another), so the pop arm can render through a
warmer palette with **zero `src/` change** and C137 byte-identical.

**The fix (render-only, opt-in A/B):** `--pop-palette=warm|beep` (default `warm`) on the pop arm.
`pop_world()` reuses the existing **VAPOR95** palette (built for "glassy 80s pop/R&B": an FM
electric-piano keys voice, a filtered round bass, a soft machine kit, memory-bus chorus, tempo echoes,
a size-1.2 damped room with a 180 Hz low-cut), retuned to the pop key/mode/tempo. Composition still
happens under the BLACK_ICE-derived `world`; only `render()` sees `pop_world()`. No C137 path is
`pop_groove`, so `rw == &world` for every C137 render.

**[OBSERVED]:** warm and beep render the **same Score** (fp `4bafe9bc6c2d9366`, lead=0 keys=24 bass=46
drums=166) — only the render palette differs, so the A/B isolates timbre exactly. WAVs are byte-distinct
(warm `9d0cd759…`, beep `b024f9c1…`). C137 **byte-identical**: `C137_ACCEPTED_V1` lock reproduces all 51
artifacts from this binary. Delivered 2 **loudness-matched** MP3s (`pop-WARM-104` / `pop-BEEP-104`) so
the ear judges tone, not level. [EAR] PENDING.

**Scout nags (unverified by ear):** VAPOR95's chorus is slow/shallow (a "memory smear", not an ensemble
chorus) and its echo is a dotted-eighth/quarter feedback delay — may read as washy/80s rather than clean
smooth-pop. All warmth claims are param-inference; the ear is the arbiter. Tunable if "too washy".

## FORWARD-MOTION PLAN (the bigger axis — [PROPOSED], not yet built)

**Recon (read-only scout, verified file:line) — the blunt finding:** in `perform_song`, **bass and drum
velocity do NOT depend on the section relation at all.** The "band arrives full" energy fader
(`argument::backing_energy_at`, `argument.rs:511`) is wired only into the *other* perform path
(`functor.rs`), never into `perform_song` (no hit in `kernel.rs`). So even the accepted C137 song only
"lifts" into a chorus via the **lead carrier's** `section_gain` (Establish 0.74 → Consequent 0.98 →
Return 1.0, `argument.rs:344`) + the pad + the keys comp — and the comp/pad/earned-return all require a
**Lead carrier** (`kernel.rs:1602-1608`, `2195-2229`). The lead-free pop arm has none of that. There is
also **no drop/break** possible for bass/drums: `validate_song` rejects off-stage bass/drum hits and
*requires* a root on every downbeat (`kernel.rs:2285-2340`).

**Two tiers to real forward motion:**
- **(A) source-only, lawful, lead-free, zero kernel change, C137-safe:** lengthen the pop form to a
  multi-section arc (more `Establish`/`Depart` episodes); widen `PopSourceGrammar.loop_bars` so the
  chorus can have its *own* chords (verse chords ≠ chorus chords); author per-section keys referents
  that climb in density/accent/register (carrier velocity = `accent * section_gain`); make
  `GrooveLaw.bass_cells` section-aware (a longer cycle: sparser verse cells, denser/louder chorus
  cells); set `intro_build = true` for a 4-bar entry ramp. Gives harmonic motion + carrier dynamics +
  bass-density build — but the bass/drum *velocity* still won't swell by section (only by chord drive).
- **(B) minimal pop-gated kernel wire (the actual "arrival"):** wire a pop-only per-episode energy into
  bass + drum + pad velocity (reuse `backing_energy_at`, or a small `SourceDomain::SmoothPop` energy
  vector), gated so C137 is byte-identical (the lock is the arbiter). This is the band-arrives-full lift
  the lead-free arm otherwise cannot get. A true *drop* still needs a `validate_song` relaxation (later).

Recommended: A + B together for a convincing verse→build→chorus. Build after the timbre verdict; the arc
is composition, so it renders on whichever palette the ear blesses.

## R2.3 — pocket lock + energy + collective push (warm accepted) — [OBSERVED]; [EAR] PENDING

**[EAR] verdict (round 3):** *"warm actually sounds better, but the keys/lead is offbeat/out of
pocket/out of sync. and it needs more of that high energy smooth pop feel… the instruments need to
stay semantically expressive while coalescing into more collective structures."* Warm palette
ACCEPTED as the direction. Three issues: keys timing, energy, collective structure.

**Root cause of "out of sync" (OBSERVED):** `pocket_offset` (the shared laid-back lean every voice
orbits) is applied to the **bass** (`kernel.rs:1814`) and **drums** (`:1919`), and to the auto keys
comp (`:1585`) — but that auto comp only runs under Consequent/Return. The pop keys are **carriers**,
placed by `argument_transport.rs:362` at `placement.start_beat + event.onset` with **no pocket lean**.
So the rhythm section laid back and the keys sat dead on the grid, ahead of the pocket. (The warm
palette's dotted-eighth tempo echo was a second smear.)

**The fix (pop-only, C137 byte-identical):**
- **Pocket lock:** `keys_comp` now lays every onset into `theme_family::pocket_offset` (imported), so
  the carried keys lay back WITH the bass/drums instead of floating. (Applied in the source, not the
  shared `argument_transport` path, so no C137 carrier is touched.)
- **Collective push:** every bar the keys' loudest stab and the bass's octave pop both land on the
  "and of 4" (offset 3.5 → pocketed 3.534, same microtime) — the band coalescing into one gesture
  while each voice keeps its own line. Bass cells rewritten bouncier, each ending on that push.
- **Energy:** the comp is now driving — four off-beat stabs a bar (the "a" of 1, the "and" of 2/3, the
  push), guide tones (3rd/7th + 9th), keys count 24 → 32.
- **Palette tame:** `pop_world` drops the VAPOR95 tempo echo (`echo = None` — the dotted-eighth
  feedback delay that smeared the keys) and pulls the room back (`space.mix` 0.28 → 0.20) for a
  tighter, more present, higher-energy pocket. Chorus kept (width).

**[OBSERVED] renders (seed 2112, `--tempo=104`, warm):** Circle fp `700458dcbeda5621`, Lift fp
`94e320bade89b4a4`; both lead=0, keys=32, bass=46, drums=166; WAVs byte-distinct. `audio_smooth_pop`
5/5; C137 **byte-identical** (lock 51/51). fmt + clippy -D clean. Delivered 2 loudness-matched MP3s
(`pop-warm-circle-v2`, `pop-warm-lift-v2`). [EAR] PENDING.

**Still open (next, bigger):** the forward-motion SONG ARC (verse→build→chorus, A then B tiers above)
— this cut locked the pocket and made the band coalesce within the 8-bar loop first. Pad still silent
(a fullness lever); drums still can't lift by section without the pop-gated kernel wire (tier B).

## R2.4 — keys as a flowing line ("move through the geometry") — [OBSERVED]; [EAR] PENDING

**[EAR] verdict (round 4):** *"it's technically hitting the origin points that characterize the
pockets, but it still feels off — it needs to semantically move through the geometry that the pocket
creates on the field."* Read: the parts land on the right pocket positions but are still discrete
point-events; they don't trace continuous, meaningful motion THROUGH the groove's shape.

**The change (one variable, pop-only, C137-safe):** the keys stopped being four detached per-bar
stabs (maximally point-like) and became a **flowing, legato guide-tone LINE** that steps and arcs
through the progression — rising through the first half of the loop, resolving down through the
second, each note all but reaching the next (legato connection) with a breathing dynamic arc. Still
pocket-locked and still pushing the "and of 4" with the bass. Circle: E-G-A-B ↑ / A-G-F-D ↓ / F-G-A-B
↑ / B-G-E-C ↓. Lift resolves its final bar B-G♯-D-E (leading tone home).

**[OBSERVED]:** Circle warm fp `9cf08748acb807ab` (was `700458dc` for the stab version), keys=32,
byte-distinct from v2. `audio_smooth_pop` 5/5; C137 **byte-identical** (lock 51/51); fmt + clippy -D
clean. Delivered `pop-warm-circle-v3-line.mp3` for the A/B against v2 (stabs).

**[EAR] verdict:** *"the band could use some tightening, and the pop vibe/smoothness could use some
refinement, but this is definitely a step in the right direction :D"* — the LINE-LEVEL reading is
CONFIRMED the right direction (the intra-note DSP fork is not needed yet). Open refinements the ear
named, for the next cuts: (1) band **tightening**; (2) pop **vibe/smoothness** refinement; plus the
standing work — roll the flowing-line treatment into the bass + both flavors, and the forward-motion
SONG ARC (tiers A/B). HEAD `4560ead`, CI run 38639015191 SUCCESS, all pushed, tree clean.

**Honest engine limit surfaced to the maintainer:** expression is at the note/line level (placement,
length, connection, dynamics). There is **no continuous intra-note expression** — no pitch
slide/bend or amplitude swell WITHIN a single held note (the timbre scout: no portamento, no per-voice
LFO/vibrato, velocity only scales amplitude). So the LINE can move through the geometry (this cut); if
the ear means motion INSIDE each note, that is a DSP build (a per-note pitch-glide / amplitude-contour
path), scoped separately. Asked the maintainer which reading.

---

## OPERATION JOHNNY FIVE — R1: the pop SONG (Lead-carried verse → hook)

**Branch** `impl/humanmusic-johnny-five-soul-2026-10-14` off the audited impl head `f3725fd` (PR #116).
**The question:** can the pop arm stop being an 8-bar lead-free vamp and become a SONG — a recognizable
verse that sets up a hook, carried by a foreground voice, with the band supporting it?

**[BUILD] — opt-in `--form=pop-song` / `--form=pop-song-lift`, ZERO shared-engine edits.** A new
`source_pop_song` beside the lead-free `source_pop_groove` (which stays as the control):
- A hand-authored vocal-proxy **Lead** carries both episodes: a 4-bar VERSE (`Establish`, ends OPEN on
  the 9th) and a 4-bar HOOK (`Consequent`, lands HOME on the tonic). Middle-register, prosodic — a
  three-note germ head (C D E) shared verse→hook, with holds, breaths and pickups; not a note per half-beat.
- The HOOK is a **certified `altered_consequent`** of the verse (the engine's own kinship law): onset_iou
  0.9375, head true, parsons 0.857, exact_interval 0.357 (re-voiced, not transposed), offset_spread 10,
  mean_delta +3.52 st, floor≥tonic, peak_profile_differs, Open→Closed, scale_fraction 1.0, step_rate 0.80.
  A scrambled hook (same note count, reversed pitches) FAILS the law — the relationship is content, not count.
- The keyboard is **freed to comp** (`comp_keys`): guide-tone stabs in the holes under the hook.
- The **pad rug** enters on the hook only (Consequent-gated) — an earned harmonic arrival, not wallpaper.
- The drummer **answers the lead's breaths** (`answer_breaths`) and **translates its phrase-end rhythm**
  (`translate_lead_rhythm`); `anticipate_bass` requested. Breathing pop bass + warm VAPOR95 palette unchanged.

**[SCORE witness] seed 2112, Circle, warm, 104 BPM:** fp `7dc82f14cf0efb9e` — lead=31 (verse 15 + hook
16, matches source), keys=26, bass=46, pad=20, drums=171; expressive=ACTIVE(breaths=6, vspread=0.33),
breath-answer=ACTIVE(3), keys-comp=ACTIVE(18), bass-antic=requested/0-this-seed (reported honestly),
drum-echo=ACTIVE(6), rug=20. A/B control (lead-free groove) fp `9cf08748acb807ab`: lead=0 pad=0, band off.

**[C137 LOCK] GREEN** — `scripts/verify-c137-accepted-v1.sh` reproduces all **51/51** accepted artifacts
byte-for-byte at this HEAD. Pop-only changes; no engine file touched.

**[GATE]** fmt clean · clippy `-D warnings` clean · `audio_smooth_pop` 8/8 (incl. the lawfulness proof,
the scramble falsifier, and the lead/keys/pad render witness) · `audio_c137_song` 58/58 ·
`audio_c137_accepted_v1_lock` 7/7.

**[REPRO]** `cargo +1.99.0 build --release --example c137_song`, then
`target/release/examples/c137_song --form=pop-song --pop-palette=warm --tempo=104 --seeds=2112 --out=DIR`
(control: `--form=pop-groove …`). Loudness-matched A/B MP3s (−16 LUFS, 21 s) delivered to the maintainer.

**[EAR] verdict:** PENDING. The hook gates the op — if it does not read as a recognizable verse→hook SONG,
the next cuts do not build on it. **Deliberately held for later checkpoints** (so the ear vetoes first):
the BASS-aware drummer (Priority 3, a domain-gated kernel edit), section travel to 32 bars (Priorities
4/6), the expression A/B (Priority 5), the independent sounded-harmony witness + the full 2×2 factorial
(§8), and the required-CI acoustic gate (§9, with its cross-runner byte-determinism caveat).

---

## OPERATION JOHNNY FIVE — R2 (timbre) + R3 (funk rhythm section)

**R1 ear verdict:** the verse→hook SONG direction is ACCEPTED (a refinement, not a rework). Two rounds followed.

**R2 (commit `d577ad2`) — the PUNCHY palette.** Ear: *"more upbeat motion, more bounce, timbre a little
less smooth."* Answered the timbre ask as a NEW opt-in render palette `--pop-palette=punchy` (beside
warm/beep): the VAPOR95 render world de-smoothed — brighter/snappier FM keys+lead (index 0.25→0.55 /
0.15→0.65, cutoffs up, shorter tails), a growly plucky bass (cutoff 560→760, sustain 0.72→0.52), a
punchier kit (kick click 0.22→0.5, snappier snare, hats 7800→9500, ghosts 0.2→0.32), less chorus wash
(0.4→0.22) + more drive (1.4→1.6). `pop_world_punchy()` in `examples/c137_song.rs`. RENDER-ONLY: pop
Score fp UNCHANGED `7dc82f14` across warm/punchy (isolates timbre), warm byte-identical to R1, C137 lock
51/51. The accepted warm groove control is untouched. 3 MP3s (warm-104 / punchy-104 / punchy-110).

**R3 (this commit) — the FUNK rhythm section.** Ear: *"approaching slow Ace-of-Base shaped sound; make
it more Espresso / Feels / Off the Wall / All in the Way You Get Down."* Read: the groove is square; the
four refs share a danceable funk/disco pocket. Two levers (the funk rhythm section), both gated/pop-only:
- A GATED funk kit in the kernel — new `pop_funk_kit` `CompiledArgument` flag; a four-on-the-floor kick,
  sixteenth hats throughout with disco OPEN-hats on the "and of 2/4", a crisp backbeat + a ghost pickup.
  Gated on `argument.pop_funk_kit`, so C137 and the lead-free groove run the EXACT archived else-branch
  (kick on 1 & 3) — the lock confirms byte-identity.
- A funk BASS for the song (`song_funk_bass_cells`): a syncopated root–fifth–octave boogie with space.
  LESSON (`Capacity(Bass)` debugged): the kernel folds the `Octave` degree down onto the root's pitch
  when the chord root sits high (`kernel.rs:1770`), so Root and Octave can be the SAME pitch; with
  `pocket_offset` a 0.5-beat Root↔Octave pair drifts under the 0.5-beat same-pitch floor and `validate_lane`
  rejects it. Fix: keep Root/Octave ≥ 1 beat apart within a bar and let the Fifth (which never folds onto
  the root) carry the off-beat bounce. Across bars the chord changes, so that boundary is safe.

**[SCORE witness] seed 2112, Circle, punchy, 104:** fp `5482f041fbb022d4` — lead=31 (topline unchanged),
keys=26, bass 46→52, pad=20, drums 171→211 (the four-floor + 16ths). **[TEST]** `audio_smooth_pop` 9/9
incl. `the_funk_kit_is_load_bearing_four_on_the_floor` (flip `pop_funk_kit` off → the beat-2/4 kicks
vanish; the archived kit never places them). **[LOCK] GREEN — 51/51 byte-for-byte AFTER the kernel edit.**
fmt + clippy-D clean. **[REPRO]** `--form=pop-song --pop-palette=punchy --tempo=104|110`. 3 MP3s
(before-punchy-no-funk / funk-104 / funk-110). **EAR PENDING on R3.**

---

## STOP conditions (plan §8)
Frozen WAV drift · source-observation contradiction · ghost feature claimed active while inert ·
unresolved clip/collision/silent-note-masquerade · valid C137 rejected by refactor ·
style-specific exception leaking into C137 · no distinguishable improvement under blind matched listening.
Fix the smallest implicated seam or retract the claim — never weaken a witness to go green.
