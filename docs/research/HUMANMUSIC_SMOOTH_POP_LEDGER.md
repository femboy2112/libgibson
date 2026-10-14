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

---

## STOP conditions (plan §8)
Frozen WAV drift · source-observation contradiction · ghost feature claimed active while inert ·
unresolved clip/collision/silent-note-masquerade · valid C137 rejected by refactor ·
style-specific exception leaking into C137 · no distinguishable improvement under blind matched listening.
Fix the smallest implicated seam or retract the claim — never weaken a witness to go green.
