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

## R2 — band-led pop groove (lead-muted, ~104 BPM) — [PROPOSED]
First 4–8 bars, no foreground lead; source-justified bass cell; `GrooveLaw`; render + stems + mutation proofs.

---

## STOP conditions (plan §8)
Frozen WAV drift · source-observation contradiction · ghost feature claimed active while inert ·
unresolved clip/collision/silent-note-masquerade · valid C137 rejected by refactor ·
style-specific exception leaking into C137 · no distinguishable improvement under blind matched listening.
Fix the smallest implicated seam or retract the claim — never weaken a witness to go green.
