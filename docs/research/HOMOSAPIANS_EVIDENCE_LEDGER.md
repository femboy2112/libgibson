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

## R2 — keys = CHORDS (the AM-radio cohesion fix). BUILT. LOCK GREEN. EAR PENDING.

EAR on R1: "still sounds uncohesive … the way you play the keys makes it sound like a lead lol (keys
should be chords, vibing, with individual notes as accents). pop is like AM radio — the carrier is
simple, expression is modulation of it. keys vibe, drums groove+partition, bass walks+funks in
resonance." So R1's single-note keyboard line was the defect: it read as a lead and held no harmonic
ground. The fix is architectural, and it forced learning the engine's real rules:

- A melodic **referent is strictly monophonic** — `argument.rs::validate_material` (≈:710) rejects any
  non-strictly-increasing onset → `InvalidMaterial`. Chords CANNOT be authored as a keys line.
- The keys lane is NOT monophony-checked (`validate_role` ≈:2515 runs `validate_lane` only on
  Lead/Bass), and the engine **comp** (`kernel.rs` ≈:1583) is the sanctioned chord source — rootless
  voicing on the "and of 2" + a guide-tone shell/colour/grace on the "and of 4". But `verify`
  (`argument_witness.rs` ≈:325 `matches_source`) requires the CARRIER role's realized notes == the
  referent exactly (extras fail), so the comp must live on a **non-carrier** voice, and the comp was
  gated to `Consequent`/`Return` only.

R2 architecture (opt-in): the **LEAD carries** a near-silent structural line (`backing_lead`, vel 0.06
— inaudible scaffold, so the band is lead-free to the ear), and the **KEYS comp the chord carrier on
EVERY bar** via a new pop-only `CompiledArgument.comp_all_sections` flag (kernel gate relaxed only when
set; C137 never sets it). The comp's own colour/grace are the "individual notes as accents," on the
keys voice. Bass = the 8 funk cells (walk/funk in the pocket, ♭7 cadences); drums = funk kit.

- SCORE (seed 2112, warm, 104): Circle `fp=c43330d8fd759a54`, Lift `fp=5031813f23baea43`
  (lead=11 @ vel 0.06, **keys=37 and now POLYPHONIC** — e.g. beat 2.534 voices C-E-G-B {60,64,67,71} =
  Am9 rootless+colour; bass=47; pad=0). Keys are chords, verified in the Score.
- GATES: `audio_smooth_pop` 9/9, clippy `-D` clean, fmt clean, and **C137_ACCEPTED_V1 LOCK byte-for-byte
  green AFTER the kernel comp-gate edit** (the flag defaults false → C137 path unchanged).
- PCM: 2 loudness MP3s delivered (`homosapians-R2-chordkeys-{CIRCLE,LIFT}-104.mp3`). EAR PENDING:
  does the chordal comp cohere (carrier vibing, not soloing)?
- HONEST NOTES: (a) the near-silent lead is a structural carrier, not a musical voice — if the ear
  wants it gone entirely, options are a louder *intended* topline (the "key" round) or a different
  carrier. (b) The comp rhythm (and-of-2 / and-of-4, bar-1 left bare) is the engine's existing comp;
  it may feel sparse/stabby — densifying/sustaining is a further comp tweak if the ear asks.

## R3 — woven harmony + continuous pad bed. BUILT. LOCK GREEN. EAR PENDING. Commit `36f5c2d`.

EAR on R2: "MUCH BETTER" (chordal keys cohere). Two new asks: (a) a soft pad in the background to
smooth the vibe; (b) harmony that is NOT the global progression played straight — "zoom out it's
basic theory, zoom in it's an intricately woven system: a global constraint with local rules that
allow semantic expression" (camp a 2-chord fragment, then hop into a *conjugate* progression that
musically fits and reads as a valid interpretation). The two asks collapsed into one structural move,
shipped as an **EAR-PROBE** (the generative realization law is deferred to Step 2, built only once the
ear blesses the sound).

- **Scout (two read-only Citadel Ricks):** (1) PAD has TWO stacked kernel gates — Gate A
  `on_stage_span(Pad)` (seat the rug on every phrase) + Gate B `kernel.rs:2288` relation filter
  (`Consequent`/`Return` only — the author's "teacher's law: pad on the chorus, never every bar"). My
  R2 source closed BOTH (pad=Silent + both episodes Establish). Witness risk LOW (pad never a declared
  carrier). (2) HARMONY: `pop_route_law` (`kernel.rs:1068`) FORBIDS verse≠chorus by construction
  (`route[i] != route[i%k]` → refused, pinned by a drift test). The real reharm toolkit exists as pure
  fns in `region.rs` (tritone_sub/third_sub/recolor/tonicize) but is FENCED from argument sources, and
  the kinship layer (`theme_family`/`AlteredConsequent`) is MELODIC-ONLY (no harmonic reading). The
  loophole: `loop_bars == route.len()` makes the law vacuous with zero engine code.
- **PAD lever (engine, option b):** new opt-in `CompiledArgument.pad_all_sections` (defaults false,
  mirrors `comp_all_sections`); kernel pad gate relaxed to `if !pad_all_sections && !is_consequent_or_return
  { continue }`. Flag false everywhere else → the gate reduces to its exact prior form → C137 byte-identical.
  The backing seats `p.pad = Support` on every phrase + sets the flag. Pad velocity is still chord-driven
  (`harmonic_drive`), so the bed swells on the E7 cadence bars for free.
- **WOVEN HARMONY (source, via the loophole):** route is no longer `global x2`. New
  `PopFlavor::verse_camp4` (a 2-chord CONTIGUOUS fragment of the global `song_loop4` — i-bVI for Lift,
  i-v for Circle) + `chorus_conjugate4` (the tonic bar reinterpreted through its RELATIVE MAJOR,
  Cmaj9=bIII, a bright lift, then the global's own tail cadences home on E7). `loop_bars=8` makes
  `pop_route_law` vacuous; the law + its pinned drift test are UNTOUCHED. keys-comp/bass/pad all read
  the per-bar route → the whole band reharmonizes off the single woven route. Near-silent lead carrier
  re-voiced onto the new chords (lands on G#, E7's leading tone).
- **SCORE (seed 2112, warm, 104):** pad = 36 (Circle) / 39 (Lift) — was 0. Verse chords != chorus
  chords, confirmed per bar via the pad's committed chord tones: Circle verse Am9-Em7-Am9-Em7, chorus
  Cmaj9-Em7-Fmaj9-E7; Lift verse Am9-Fmaj9-Am9-Fmaj9, chorus Cmaj9-Fmaj9-Dm9-E7 (bar 7 carries G#).
  lead=11 @ vel 0.06, keys=39/40 (polyphonic), bass=47.
- **GATES:** fmt, clippy -D clean; `audio_smooth_pop` 9/9; `audio_homosapians_harmony_audit` (R0) 5/5;
  **C137_ACCEPTED_V1 LOCK byte-for-byte green AFTER the kernel edit.** Opt-in; no C137 source touched.
- **PCM:** 2 loudness MP3s delivered (`homosapians-woven-{CIRCLE,LIFT}-104.mp3`). EAR PENDING: does the
  woven harmony de-cliche it, and does the bed smooth the vibe?
- **STEP 2 (ear-gated, NOT built):** promote the hand-authored route to a generative `pop_realization_law`
  — a new `SmoothPopRealized` domain + global tier + a Score-level witness that each realized bar is a
  lawful transformation of the global function (recolor / sub / applied-dom / relative-swap / a camp
  window), protecting the cadence bar's function. Only build once the ear blesses this sound.

## R4 — semantic parallax: slow repeating loops, 16 bars. BUILT. LOCK GREEN. EAR PENDING. Commit `cfd567b`.

EAR on R3: "getting better … the 2chord loop sounds dope, it gives the music time to be absorbed as one
coherent living thing before immediately changing." The ask: **SEMANTIC PARALLAX** — the background
(harmony/pad) should transition SLOWER than the foreground (comp/bass/kit), both in harmonic phase — and
MORE (semantically valid) REPETITION, time to absorb before changing. Interpretation: "slow" = LOW
harmonic-information rate (few distinct chords, repeated), not literally longer chords; the fast surface
(comp rhythm, bass figures) moves over the slow ground. Applied the 2-chord-loop principle to the WHOLE
song, source-only (no kernel edit this round):

- **Chorus = a repetitive 2-chord VAMP, not a 4-chord march** — a CONJUGATE of the verse loop (tonic pole
  lifted to its relative major, Cmaj9=♭III): Circle Am–Em→C–Em, Lift Am–F→C–F, + one E7 turnaround. So the
  harmonic ground stays slow + repetitive everywhere; the chorus arrives by bright COLOUR + energy, not by
  harmonic rate. (`PopFlavor::chorus_conjugate4` reshaped: Circle C–Em–C–E7, Lift C–F–C–E7.)
- **Form doubled to 16 bars.** ENGINE CONSTRAINT learned: `longform_domain` requires **N contiguous
  16-beat (4-bar) episodes** (error "long-form domain is N contiguous sixteen-beat episodes (N>=2)") — so
  32-beat episodes are illegal; 16 bars = FOUR 4-bar episodes: verse-A, verse-B, chorus-A, chorus-B (the
  slow loop STATED then REPEATED). trace total_beats 32→64; build_with_contract_for_beats 64.0; 4 arc spans
  ×16 beats; 2 carrier referents reused across the repeats; route = verse_camp4 ×2 ++ chorus_conjugate4 ×2
  (loop_bars=16, loophole, pop_route_law + its pinned test untouched); bass = the 4-bar funk phrases looped
  to 16 cells; pad bed every episode (pad_all_sections from R3).
- **BASS MONO-LANE FIX (engine law learned):** `validate_lane` (`kernel.rs:2530`) refuses two bass notes of
  the SAME pitch < 0.5 beats apart (a monophonic-bass law; `Capacity(Bass)` is the shared error code). The
  chorus F bar's "and of 4" FIFTH is C, which collided with the next C bar's ROOT downbeat (both pitch 36,
  0.466 beats apart via pocket offset) — Lift-only (Circle's Em-there has fifth B). Fix: that one chorus
  cell's beat-3.5 push → OCTAVE (distinct pitch, same funk feel). (Found via a temporary diag in
  validate_lane / continuity_violations, since Capacity(Bass) also fronts the continuity check at :2377/:2588;
  diag removed before commit — src/ ended byte-identical to R3.)
- **SCORE (seed 2112, warm, 104):** 64 beats / 16 bars. Verse loop ×4 then chorus vamp ×2 confirmed per bar
  via the pad's committed chord tones; pad on every bar; bass 94, keys ~79, pad ~75. Circle chorus
  C–Em–C–E7, Lift chorus C–F–C–E7 (bars 11,15 = E7 with G♯).
- **GATES:** fmt, clippy -D clean (fixed a `useless_vec` on the bass-cell container → array); audio_smooth_pop
  9/9; harmony audit (R0) 5/5; **C137_ACCEPTED_V1 LOCK byte-for-byte green** (source-only round). Opt-in;
  no C137 source touched. 2 loudness MP3s delivered (`homosapians-parallax-{CIRCLE,LIFT}-104.mp3`).
- EAR PENDING: does it breathe now — time to absorb, more repetition, the parallax feel? Open knob: the
  *amount* of repetition (doubled to 16 bars; trivially tunable). Step 2 (generative realization law) still
  deferred, ear-gated.

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

---

## REHEARSAL COMPOSER — the band WRITES the song (impl branch `impl/homosapians-rehearsal-composer-2026-10-16`, on research PR #122)

The pivot (owner brief, OPERATION THE HOMOSAPIANS "the band writes the song"): stop authoring a
finished backing and making the players perform it; construct the song THROUGH the musicians
negotiating. Built as a PURE SYMBOLIC pre-source composer (`examples/c137_song/rehearsal.rs`) that
emits a `CompiledArgument` the existing witnessed kernel renders with ZERO kernel edits. Opt-in via
`--form=pop-rehearsal` (reuses the pop-band warm render path). Commit `93c6d12`.

**Two causal clocks:** performance time `t` immutable once frozen; rehearsal revision time `k` — the
guitarist may revise an earlier bar in draft `k+1` while the audition take of draft `k` stays a frozen
historical trial (`TrialTake`, append-only). A revision makes a NEW source; never edits a committed take.

**The jam (A→G):** seed → `generate_riffs` (whitened per-candidate RNG, 5 rhythm cells × launches ×
contours over A-Aeolian; the harmonic camp is chosen FROM the riff, so the seed reaches the route) →
`bass_for_riff` (a DETERMINISTIC function of the riff's density / rest / pickup — provably "listening",
traceable to a named riff feature, not an RNG draw) → `drums_for_bass` (reinforce-push / leave-gap /
fill off the committed bass, spine fixed) → `guitar_proposals` (the back-edge: a dense riff thinned for
the drummer's fill; a sparse riff gets a pickup answering the bass turn; a flourish always REJECTED) →
`freeze` (2×16-beat Keys-carried Establish episodes, riff vamped, PAD holds the chords, negotiated bass,
funk kit, `comp_keys=false` ⇒ the Keys window holds only the carrier ⇒ LITERALLY zero Lead).

**Falsification gates (`tests/audio_homosapians_rehearsal.rs`, 10/10):** determinism; G0 seed chooses
the material; G9 held-out seeds are not one stock riff; G1 bassist answers the riff's rhythm (+ deaf
control); G2 drummer follows the bass (spine kept); G3 guitarist revises after the band; G4 committed
takes immutable + rejected flourish never contaminates; G5 frozen Score LEAD==0 with Keys/Pad/Bass>0;
G7 same count + different rhythm changes identity + bass. Characterization control
(`audio_homosapians_rehearsal_gap.rs`, the OLD fixed path) stays 4/4.

**Gates:** fmt + clippy -D clean; rehearsal 10/10; smooth-pop 9/9; harmony audit 5/5; gap 4/4;
**C137_ACCEPTED_V1 LOCK byte-for-byte OK** (no kernel/synth/rendered-source edit).

**Render (warm, 104, loudness-matched MP3):** control pop-band seed 2112 (hand-authored R4) vs
rehearsal seeds 2112/7/42. Negotiated: 2112 = 9-note driving riff thinned to 8 (Simplify); 7 = 7-note
syncopated-funk thinned to 6 (Simplify); 42 = 6-note on-beat statement kept as-is (0 revisions). EAR
PENDING. Known limitation: all three selected riffs landed in the SAME i–iv camp (Am9/Dm9→Cmaj9/Dm9);
`camp_for_riff` selection is biased — a knob to spread harmony across seeds.

**DEFERRED (named, exact seams from read-only recon):**
- **G6 real section-form witness** (kill the `loop_bars==route.len()` loophole): add an opt-in
  `CompiledArgument.pop_form: Option<PopFormLaw>` (NOT a `PopSourceGrammar` field — that has pinned
  literals) + a `pop_section_law()` called AFTER `pop_route_law` in the SmoothPop arm (`kernel.rs`
  ~1039); leaves `pop_route_law` + its pinned tests `the_pop_route_law_refuses_a_drifting_or_mis_sized_loop`
  / `the_c137_and_pop_route_laws_are_distinct_closure_conditions` byte-identical. Checks within-section
  period repetition + verse→chorus conjugacy + return.
- **True polyphonic Keys comp with zero Lead** (the fake-carrier root cause): the carrier window check
  `argument_witness::in_window` counts ALL role notes; the comp stabs (`role_note=="comp"`) collide with
  the carrier count. Narrow fix = exempt `role_note=="comp"` in the carrier-window selection + fix
  `room()` (`kernel.rs:1588`) to include Keys carrier onsets. Both kernel surgery → own round.
- **Camp spread** (`camp_for_riff` bias), and the Circle/Lift-style A/B is now a SEED A/B.

**EAR VERDICT #1 + FIX (commit `cde0f9f`):** owner on the first cut — *"keys sound broken, it sounds
like it's trying to lead and failing."* The monophonic Keys-carried riff reproduced the R1
single-note-keyboard-reads-as-a-lead flaw. Fix = the narrow authorized construction the brief §13
prescribes (NOT a fake Lead): one kernel line in `argument_witness::in_window` excludes
`prov.role_note=="comp"` notes from the carrier-content window, so a Keys-carried step may hold the
engine's real polyphonic comp chords while its own authored line is still checked verbatim. `freeze`
now sets `comp_keys + comp_all_sections` (keys COMP chords every bar); the required carrier is a soft
chord-tone fill in the comp's open bar (local bar 1) so the keys read as CHORDS, zero Lead, no fake
carrier. `camp_for_riff` now picks via the riff fingerprint → seeds 2112/7/42 land in i-♭III / i-♭VI /
i-iv (three harmonic worlds). G5 strengthened: asserts the Keys lane is genuinely polyphonic. Gates:
fmt+clippy-D; **lib 874/874** (route-law + witness pins intact); **audio_c137_song 58/58**; rehearsal
10/10; gap 4/4; smooth-pop 9/9; harmony 5/5; **C137 LOCK byte-for-byte OK** (witness never touches
synthesis). This RESOLVES the deferred "true polyphonic Keys comp" item above. Re-rendered seed-A/B —
EAR PENDING on whether the keys now sit right. Still open: G6 section-form witness; a more
musically-principled camp picker; making the negotiated riff audible (comp-follows-riff-rhythm).

**EAR VERDICT #2 + VISION ROUND (commit `c8651c1`):** owner — *"7 and 42 are starting to sound good,
2112 sounds cliche… the musicians are just throwing shit at the wall… rather than interpreting what's
there and finding the semantic possibilities of what could be, there's no underlying vision."* Owner's
spec for the vision (verbatim intent): the SONG itself is the goal; the seed is clay; each musician
makes an interpretive move; the band steps back ("does this feel like a song? where should it go?") and
repeats. Built:
- Harmony READ OUT OF THE RIFF (`interpret_harmony`): pitch classes weighted by dwell time; the tonic A
  is the fixed home pole, the heaviest chord-IMPLYING degree picks the 2nd chord (D→iv, F→♭VI, G→♭VII,
  E→v). The cliché i-♭III VERSE camp is BANNED (tonic-tone emphasis → warm ♭VI, never ♭III); ♭III is
  reserved for the CHORUS lift. Seeds 2112/7 → i-iv, 42 → i-♭VI (all ear-liked worlds; 2112 de-clichéd).
- Chorus REINTERPRETS the verse (the "where it wants to go"): dark pole (iv/v) → lift tonic to ♭III;
  bright pole (♭VI/♭VII) → tonicise on V7. Shares the verse's tonal content, adds the one dominant.
- Emergent TRANSITION: the last verse bar `transition_walk_cell()` walks R-3-5-8 + a push into the
  chorus downbeat (the bass transition the guitarist/bass exchange implies).
- CONTEMPLATION is first-class: `RehearsalDraft.shaping_log` narrates CLAY → read the riff's ground →
  groove → compensate → fill → CONTEMPLATE ("it circles, needs somewhere to go") → answer into the
  chorus → CONTEMPLATE → DONE. The underlying vision made legible.
Gates: fmt+clippy-D; rehearsal 13/13 (new: harmony-read-from-riff, chorus-reinterprets-verse,
band-contemplates-toward-a-song); gap 4/4; smooth-pop 9/9; harmony 5/5; **C137 LOCK byte-for-byte OK**
(example+test only). Re-rendered seeds 2112/7/42 — **EAR ACCEPTED** (owner: *"hell yeah. starting to
sound like a song now"*): the vision/interpretation approach — the deferred Step-2 realization law, the
band shaping clay toward the goal of a song — is CONFIRMED BY EAR. Build forward from here. Known: 2112 & 7 share the i-iv world
(cross-seed harmonic spread is a knob); still a backing (no sounded topline / true return — needs the
lead "key").

**AUDIBLE RIFF (commit `eb87df1`, owner picked this lever):** the guitarist's riff only shaped the
harmony/bass/transition — it was never HEARD. Now the keys comp stabs its chord voicings ON the riff's
own rhythm (chordal hits, not a sung line — so it is the band's hook felt, not the "failing lead" the
owner rejected). New opt-in `CompiledArgument.comp_onsets: Option<Vec<f64>>` (within-2-bar onsets);
`None` by default so C137 + every lead-carried song keep the fixed "and of 2 / and of 4" comp and render
byte-identical. The kernel comp block branches on it BEFORE the fixed per-bar loop (the 2-bar riff stated
twice across the 16-beat episode, a rootless voicing per onset, bass owns the root). `freeze` passes the
negotiated riff's onsets. New gate `keys_comp_stabs_on_the_riff_rhythm` (every riff onset has a Keys stab
within the pocket tolerance). Gates: fmt+clippy-D; **lib 874/874**; **audio_c137_song 58/58**; rehearsal
14/14; gap 4/4; smooth-pop 9/9; harmony 5/5; **C137 LOCK byte-for-byte OK** (comp_onsets None on every
accepted path). Re-rendered seeds 2112/7/42 — EAR PENDING. Density knob: a dense riff = many stabs (can
thin to salient onsets if busy).
