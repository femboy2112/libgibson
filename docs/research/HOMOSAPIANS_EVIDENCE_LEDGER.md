# Operation THE HOMOSAPIANS — Evidence Ledger

Implementation branch `impl/homosapians-real-band-2026-10-16`, worktree
`/home/leah/LibGibson.homosapians`, stacked on the research bootstrap
`research/homosapians-real-band-ensemble-2026-10-16` @ `3cf10b0f` (draft PR #120), itself the
Johnny Five R12 parent `4b3306257af8d75beb61d117bb4b32d244c36818` (PR #118) + 3 research files
(code byte-identical). Program + fan-out plan:
`docs/research/HOMOSAPIANS_REAL_BAND_PROGRAM_2026-10-16.md` and
`..._CLAUDE_FANOUT_HANDOFF_2026-10-16.md`.

**Standing rules.** C137_ACCEPTED_V1 (51 artifacts) is sacred — `scripts/verify-c137-accepted-v1.sh`
after every code change, STOP on any drift, never rebaseline. Everything opt-in; no main merge /
default promotion / release / ABI change / fixture replacement without owner approval. Her EAR is
the acceptance oracle; engineering gates only reject. `cargo +1.99.0`. One build lane (OOM reaper
forbids concurrent heavy cargo builds) — builds serialize; only read-only recon parallelizes.

Evidence is kept in four separate columns and never conflated: **SOURCE** (lawful relationships),
**SCORE** (actual committed notes/roles/chords), **PCM** (the rendered sound), **EAR** (the only
musicality verdict).

---

## R0 — integrity + note forensics. DONE, GREEN.

`cargo +1.99.0 test --test audio_homosapians_harmony_audit` → **5/5 pass as-written** (the research
author committed the file but never compiled it; no API fixes were needed). On the real committed
Score of the live pop SONG (not the groove chart):

1. `current_circle_hook_has_real_g_natural_against_e7_g_sharp` — at global beat 29.75 the Lead sounds
   **G♮** while the chord is **E7** (G♯ = its major 3rd) **and the Pad sustains that G♯**. Removing G♯
   from the *lead* three rounds ago never removed the cross-voice blues rub, because it comes from the
   **pad**. CONFIRMED (Score). Not auto-"fixed": a #9/♭3 blues colour may be intentional — EAR decides.
2. `the_latest_circle_bass_turnaround_truly_moves_below_its_e7_root` — R12's `BassDegree::Seventh` (D)
   really descends below the E root. CONFIRMED; preserve the descent.
3. `a_note_mutation_changes_the_vertical_without_changing_the_chart_but_is_refused` — a post-hoc Lead
   pitch mutation changes the vertical yet is refused by the committed witness (`validate_candidate`
   errors). CONFIRMED — no hidden source repair.
4. `a_melodically_closed_last_note_is_not_a_tonic_chord_arrival` — the hook ends on tonic A but over
   E7, not Am. Melodic home ≠ harmonic home ≠ narrative completion. CONFIRMED.
5. `the_distinct_groove_and_lift_sources_have_distinct_sounding_harmonies` — groove-Circle (Dm9 bar 1)
   ≠ song-Lift (Fmaj9 bar 1). CONFIRMED — never judge one form's melody against another's chart.

Verdict: the Aletheia audit is correct. The specific actionable truth is #1 (the pad carries the
G♯ that rubs the lead's G♮). No chromatics were snapped; no notes were changed in R0.

---

## R1 — the BAND-ONLY mini-song (the "lock"). BUILT. LOCK GREEN. EAR PENDING.

New opt-in arm `--form=pop-band` / `pop-band-lift` (Circle / Lift). Lead-free; a complete 8-bar
instrumental statement with a verse→chorus shape, NOT an 8-bar vamp twice.

- SOURCE (`examples/c137_song/source_pop.rs::source_homosapians_backing`): two 16-beat Keys-carried
  episodes over the pop SONG progression (looped ×2). Keys carry a syncopated guide-tone HOOK
  (short-short-long + pickup, outlining each chord's 3rd/7th/9th colour with real space); the verse
  states it mid-register, the chorus LIFTS it ~an octave, fuller (a 5th attack/bar) and louder. EIGHT
  per-bar bass cells (four verse, four driven chorus — not four stamped twice), the ear-liked funk
  bounce + the dark ♭7 at each E7 cadence (R12 preserved). `pop_funk_kit=true`; lead-dependent levers
  (comp-under-lead / breath answers / lead-rhythm translation) OFF because it is lead-free.
- CLI/preset: `pop_band` added to `C137Preset`/`Resolved` + a builder implying `pop_groove` (so it
  renders through the accepted WARM palette); checked before pop_song in `build_source`; its own
  honest manifest header (`HOMOSAPIANS BACKING`).
- SCORE (seed 2112, warm, 104 BPM): Circle `fp=b79a576ab095dc4c` lead=0 keys=36 bass=47 **pad=0**
  drums=198; Lift `fp=e3424f66aa4b82ae` lead=0 keys=36 bass=47 **pad=0** drums=196. Riff
  (verse 16 + chorus 20 = 36) and 8-cell bass (21 verse + 26 chorus = 47) sounded as authored.
- **HONEST GAP (SCORE):** `pad=0`. The pad was wired to enter on the chorus via per-phrase arrangement
  role, but a lead-free Establish/Establish source does NOT grow a pad bed — the engine only seats a
  pad rug under a *Consequent*. So the chorus arrival currently rides on riff-lift + bass-drive + funk
  kit, not a pad. Next lever: make the chorus episode a lawful `Consequent` (the altered-consequent
  kinship law is lead-oriented — must verify it admits a Keys-carried backing, or author a narrow
  backing-consequent path) OR seat the pad another way. Not faked.
- GATES: `audio_smooth_pop` 9/9 (no regression), clippy `-D warnings` clean, fmt clean, and
  **C137_ACCEPTED_V1 LOCK: OK — every accepted artifact reproduces byte-for-byte** (the pop-band arm
  is purely additive; the C137 default path never selects it).
- PCM: 2 loudness-matched MP3s delivered (`homosapians-R1-backing-{CIRCLE,LIFT}-104.mp3`,
  loudnorm I=-16:TP=-1.5:LRA=11). EAR PENDING — does the backing feel like a complete song, and does
  the chorus ARRIVE without a lead?

---

## The vertical slice (plan §6 "band lock / lead key" 2×2) — status

| | band-only | band + lead |
|---|---|---|
| **legacy synth/mix** | **R1 DONE (this ledger), EAR pending** | pending (drop source-licensed lead onto the lock) |
| **new physical/ensemble** | pending (R2 strings / R4 field) | pending |

Legacy band-only cell is delivered. The band+lead cell is the next source-side build (reuse the pop
SONG's lead over this lock). The new-system column is the DSP work (R2/R3/R4).

---

## Scout map (read-only recon, 2026-10-16) — used to size R2/R3/R4

- **GSPU-FM (R3):** `research/humanmusic-gspu-fm-2026-10-09` = docs only, zero Rust. Smallest slice =
  a pure 2/4-op `CompiledFmProgram` in a new `dsp/fm_graph.rs`, OFF by default, reusing `Adsr`; chip
  dialects later. Legacy `OscKind::Fm` is 2-op PM + an extra sine (preserve byte-exact); do not add a
  field to `Patch` (`Copy`, ~33 literal sites) or a `Vec` to `OscKind`.
- **Meatsack (R2):** docs only, zero Rust. Smallest prototype = opt-in `human_music/strum.rs` (NOT
  `gesture.rs` — name/type collision with the R14 slide diagnostic) + a `StrumIntent` sidecar that
  RE-TIMES the composer's existing participant Notes (ghost-note law), a per-string rig, contacts →
  integer samples at one boundary. Falsifiers F1 ghost / F2 down==reversed-up vs white-jitter / F3
  typed Infeasible / F4 ring from string energy not Score dur / F5 block-size determinism / F6 OFF
  byte-exact. `dsp/fx.rs:60` `Delay` is structurally a KS loop; `Score.stroke_origins` is the sidecar
  precedent.
- **Existing shared field (R4) — "no shared effects" is FALSE:** full chain in `synth.rs:823-982`.
  R4 seam = the memory-bus stage `synth.rs:903-907` with the `None`-is-not-computed idiom (how
  VAPOR95 landed without changing BLACK_ICE bytes). `voice::audible_end_at` (`voice.rs:27`) reads
  Patch ADSR only — the coupling that breaks once physical tails exist.

## Next falsifiers / levers (ear-gated)
1. Pad arrival: Consequent chorus vs the current Establish (does the pad bed actually seat, and does
   the arrival improve at matched loudness?).
2. band+lead on this lock (the "key"): the 2×2 legacy row completed.
3. R2 one real strummed-string prototype (the first "new physical system" cell).
4. Circle vs Lift — STILL unpicked across all of Johnny Five; keep offering both.
