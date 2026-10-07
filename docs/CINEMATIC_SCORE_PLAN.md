# Cinematic Score — Round Kickoff

The visual spine is frozen (one authoritative deterministic edit clock; see
[`CINEMATIC_AUTHORING.md`](CINEMATIC_AUTHORING.md) §2 and `examples/cinematic_showcase/reel.rs`).
This document is the **thinking fixture** for the next round: giving the film a
score. It is a plan and a decision surface, *not* an implementation spec, and no
code in it is final until the decisions in §5 are made.

> **One sentence:** the whole round is a single projection
> `M : Timeline<Shot> → SemanticTrace`, written on the *show* side; HumanMusic
> renders the trace and never learns that a film exists.

---

## 1. What is already true (do not rebuild it)

- `reel()` returns a `gibson::timeline::Timeline<Shot>` — the one authoritative edit
  clock, ~62.2 s, cues `Establish → Grammar×6 → Observatory×3 → Reveal`, each a
  half-open window with a local narrative time. Deterministic `f(edit)`.
- The music/reaction **seam** is already documented
  ([`CINEMATIC_AUTHORING.md`](CINEMATIC_AUTHORING.md) §7): the timeline payload stays
  **opaque**; the projection to a score is owned *elsewhere*, never stored in the
  timeline. This round builds that "elsewhere."
- HumanMusic already exposes the exact landing point we need:

  ```rust
  // src/audio/human_music/mod.rs:126 — the one public entry, deterministic.
  pub fn render(
      trace: &SemanticTrace,
      world: &MusicWorld,
      seed: u64,
      sr: SampleRate,
      block: usize,
  ) -> (Score, RenderResult);
  ```

  and `gibson::audio::wav` can write the resulting PCM offline. **No device, no
  `audio-cpal`, no CI audio deps** are needed to produce a WAV.

## 2. The seam, drawn

```
  reel() : Timeline<Shot>          ← FROZEN (the picture)
        │
        │   M  (the ONE thing this round writes; lives on the show side)
        ▼
  SemanticTrace                    ← HumanMusic's public input type
        │
        │   human_music::render(&trace, &world, seed, sr, block)
        ▼
  (Score, RenderResult)  →  audio::wav  →  score.wav   (62.2 s, synced to the film)
```

`M` reads each cue's identity, window and local time and emits semantic *meaning*
(tension, arrival, density, section change — whatever `SemanticTrace` models), **not**
notes or tones. HumanMusic decides the music; `M` decides the *dramatic intent per
beat of the film*. That is the observer-only discipline: the arrow goes one way, and
the picture never reads the score back.

## 3. Hard constraints (carried from the mission)

- **Do not modify `src/audio/human_music/`.** A maintainer consolidation branch is in
  flight on remote (`claude/humanmusic-semantic-consolidation-cc4k3o`). This round
  *consumes* HumanMusic's public API (`render`, `SemanticTrace`, `MusicWorld`,
  `Score`); it adds no code under `human_music/`. If the public surface we target has
  shifted on that branch, **reconcile before building `M`**, don't fork around it.
- **Observer-only / one-directional.** `M : Timeline<Shot> → SemanticTrace` only. No
  feedback path from audio into the picture; no tones/`Density`/`Emphasis` stored in
  the timeline.
- **Deterministic.** Fixed `(world, seed, sr, block)`; the same film always renders
  the same WAV. Capture the receipt (hashes) the way the showcase does.
- **Launch button stays Leah's.** Branch-only work until she says otherwise; no PR,
  merge, tag, or release.
- **Listening is acceptance** (see memory `leah-listening-acceptance`): machine
  receipts never certify "sounds right." The deliverable is a WAV Leah hears, plus an
  honest receipt — never an auto-approved default.

## 4. Smallest first cut (proposed, pending §5)

1. A new show-side module (e.g. `examples/cinematic_showcase/score.rs`) with
   `fn score_trace(film: &Timeline<Shot>) -> SemanticTrace` — the projection `M`,
   and nothing else.
2. A headless `cargo run --example cinematic_showcase -- score out.wav` that runs
   `M` → `human_music::render` → `audio::wav`.
3. A test pinning: trace length matches the film duration; each act boundary lands a
   section/arrival event; deterministic across two runs (same bytes).
4. No change to the picture, the flagship render path, or `human_music/`.

The Part V reaction cut (Filthy Frank, over the *existing* greenscreen reaction
compositor — [`FRANK_REACTION_CUT_PLAN.md`](FRANK_REACTION_CUT_PLAN.md)) and the Part
VI show director are **separate** rounds; this plan is Part IV (the score) only.

## 5. Decisions for Leah before `M` is written

1. **Branch.** New `feat/v0.5-cinematic-score` off the current cinematic branch
   (`feat/v0.5-cinematic-showcase`), or off `feat/v0.5-spatial-ui`, or off `main`?
2. **HumanMusic freeze.** Is `human_music/` still off-limits to *all* edits, and is
   its public surface (`render`/`SemanticTrace`) stable enough to build against now,
   or should this wait on the `claude/humanmusic-semantic-consolidation-cc4k3o`
   consolidation landing first?
3. **Scope of "music."** This round = the forward *score* (`Timeline → SemanticTrace`
   → WAV). Separate from the `feat/v0.5-auditory-observer` line (acoustic *analysis*,
   AXIS B) — confirm we are doing the score, not the observer, this round (or both).
4. **Does it play, or just render?** Offline WAV only (no new deps) for now, with
   real-device playback (`audio-cpal`) deferred — confirm.
