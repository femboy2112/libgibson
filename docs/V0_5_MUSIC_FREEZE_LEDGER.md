# HumanMusic v0.5 — MUSIC FREEZE / CLOSEOUT LEDGER

Compiled 2026-10-19, read-only, from git state, research docs and `gh pr view`. No cargo/rustc was run;
nothing here was rendered or re-verified acoustically. Every claim cites a path, SHA, or PR. Anything
I could not ground in the repo is labelled **UNVERIFIED**.

## 1. Purpose and freeze declaration

HumanMusic feature expansion (the C137 kernel/long-form line, smooth-pop, Johnny Five, the Homosapians
band and rehearsal composer, and the pocket-commute work) is **FROZEN as good-enough WIP for v0.5**.
This ledger preserves what exists, where it lives, how to verify it, and what is knowingly unfinished,
so the team can pivot to interactive-instrument features without the music rotting or being quietly
rewritten. After this point only separately-justified **critical correctness / regression repairs** may
touch the music paths in section 6. Open problems in section 5 are preserved, not queued. Every
experimental arm below is opt-in, nothing is promoted to a default, and the owner holds the launch
button on any push/PR/default flip (per the pocket-line commit bodies and the Evidence Ledger header).

## 2. Preserved heads

All PRs below are **OPEN DRAFT** and stacked on one another (`gh pr view`, 2026-10-19). Pushed status
is from `git status -sb`: no ahead/behind marker against the tracking branch means in sync with origin.

| Worktree (under /home/leah/) | Branch | HEAD | Pushed? | PR (base) | What the line is | Ear status (evidence) |
| --- | --- | --- | --- | --- | --- | --- |
| LibGibson (main checkout) | feat/v0.5-humanmusic-beefup | fbdbffc | yes, in sync with origin | #87 (main) "the band tells the story (Rick-C137) + beef-up" | Rick-C137 story work, `rick_story --matrix` audition | UNVERIFIED in repo for this head; memory says liked probe awaits her ear |
| LibGibson.longform | feat/humanmusic-c137-expressive-transfer | f68c9bf | yes | #111 "C137 is the default - R1 expressive lead + R2 band answer + the Jerry Law" | C137 long-form: lead + band answer; source of level B | ACCEPTED: `docs/fixtures/humanmusic-c137-accepted-v1/README.md` marks level B "EAR-ACCEPTED (PR #111)" |
| LibGibson.johnny | feat/humanmusic-johnny-five-agency | b536699 | yes | #114 (base #111) "Johnny Five - truthful hearing + lead->keys carrier handoff, ear-gated" | Narrative handoffs, keys foreshadow intro | ACCEPTED: README levels D1 "sounding MUCH BETTER :D", D2 "hell yeah" |
| LibGibson.smoothpop | impl/humanmusic-smooth-pop-2026-10-14 | f3725fd | yes | #116 (base #114) "Coherence Lock -> Smooth Pop: C137_ACCEPTED_V1 lock (R0) + opt-in pop domain/groove" | The byte lock (commit 924e3a9) plus opt-in smooth-pop groove | Lock levels: see section 3. Smooth-pop groove: commit f3725fd "ear verdict 'step in the right direction'; open refinements" (not an acceptance) |
| LibGibson.johnnysoul | impl/humanmusic-johnny-five-soul-2026-10-14 | 4b33062 | yes | #118 (base #116) "smooth-pop SONG - Lead verse->hook" | Opt-in `--form=pop-song` (R1-R12) | EXPERIMENTAL / EAR PENDING. Commits R10-R12 are tagged "opt-in, ear-pending"; ledger records R8 "EAR VERDICT: REJECTED + REVERTED"; flavour Circle vs Lift unpicked (HUMANMUSIC_SMOOTH_POP_LEDGER.md ~l.605) |
| LibGibson.homosapians | impl/homosapians-real-band-2026-10-16 | 56f2391 | yes | #121 (base #120) "R0 verified + R1 band-only lock (opt-in --form=pop-band)" | Lead-free band-only mini-song, woven harmony, pad, 16-bar parallax | Evidence Ledger quotes owner: R2 "MUCH BETTER", R3 "getting better ... 2chord loop sounds dope". R4 loops: "EAR PENDING" at this head |
| LibGibson.rehearsal | impl/homosapians-rehearsal-composer-2026-10-16 | e6fbaff | yes | #123 (base #122) "the band writes the song (opt-in, draft)" | `--form=pop-rehearsal`: riff -> bass -> drums -> guitar revision -> freeze, literal zero Lead | Ledger l.322 "EAR ACCEPTED" (vision round, "hell yeah. starting to ..."); lean comp "very good"; register ceiling l.441 "an improvement". Pocket-phase explicitly open at this head |
| LibGibson.pocket | impl/homosapians-pocket-commute-2026-10-19 | 52c4954 | yes, in sync with origin | none found in the PR list; #124 is the research/audit PR beneath it | Most recent band state: typed `comp_attack` lock, comp witness, drummer plan, section witness, tempo sweep. Contains rehearsal e6fbaff, johnnysoul 4b33062 and homosapians 56f2391 as ancestors (merge-base check) | Commit b0d7e39: "The owner's ear chose the no-advance treatment over the legacy 0.03-beat pull in a loudness-matched A/B (2026-10-19)". The later "B_locked EAR-CONFIRMED" is **UNVERIFIED in repo** (memory only). R1 metrology doc still says "NOT an accepted audio change" |
| LibGibson.fusion-verify | verify/c137-fusion-gates | 7ecf069 | **no upstream**; working tree dirty (Cargo.toml modified, examples/fusion_gates.rs untracked) | none | Verification probes for C137 fusion | EXPERIMENTAL, local-only, uncommitted changes. Not preserved by git until committed |
| LibGibson.kernel-hostile | research/c137-keyboard-capacity | ac97bcc | **no upstream**; untracked tests/audio_c137_keyboard_capacity.rs | none | Hostile kernel-boundary tests, keyboard capacity research | EXPERIMENTAL, local-only. Not preserved by git until committed |

Other stacked PRs for lineage (all OPEN DRAFT): #104 causal kernel, #106 long-form, #108 phase-change,
#110 semantic exchange, #120 Homosapians research, #122 rehearsal research, #124 pocket-naturality
research (base #123's head), #92 Filthy Frank flagship docs requirement.

Pocket-line commits on top of e6fbaff, oldest first (`git log` in LibGibson.pocket): 15790ba
characterization tests, 4ac2cc9 audit + R0-R6 plan, 8591a60 rustfmt, ac63e6b phase-group math doc,
39bf47f R1 metrology lab + typed phase law, b0d7e39 R2 typed comp lock, 8e7b22c R3 comp witness,
d9a75d4 R4 drummer agency, 4427b13 R5 section witness, 52c4954 G2 tempo sweep.

**Preservation gap to fix at the team's discretion, not by me:** fusion-verify and kernel-hostile have
no remote and uncommitted files. Nobody has pushed them.

## 3. The C137 acoustic lock (C137_ACCEPTED_V1)

- Location: `docs/fixtures/humanmusic-c137-accepted-v1/` containing `README.md`, `MANIFEST.tsv`,
  `SHA256SUMS`. Introduced by commit 924e3a9 "C137_ACCEPTED_V1 - immutable acoustic regression lock (R0)".
  Verifier: `scripts/verify-c137-accepted-v1.sh`. Present in .pocket, .homosapians, .johnnysoul (checked
  by `ls`). I diffed the fixture dir of .homosapians, .johnnysoul and .rehearsal against .pocket's; the
  diff output was empty, which I read as identical, but the `diff --stat` tail was blank for all three
  so treat identity as strongly indicated rather than hash-compared.
- **Artifact count: 51 lines in SHA256SUMS** (`wc -l`): 21 `full.normal.wav`, 9 `full.nolead.wav`,
  21 `score.tsv`. By level: A1 6, A2 6, B 9, C1 6, C2 6, D1 9, D2 9. MANIFEST.tsv is 31 lines
  (`wc -l`: 1 header + 30 rows, consistent with 21 normal + 9 nolead; I only read the first 8 rows, so
  that row split is inferred, not inspected) and records tempo, block, `Score::fingerprint()`
  and frame count.
- Common flags `--seeds=2112,7,99 --tempo=144`; block 256; `SampleRate::STUDIO` (48 kHz), hardcoded in
  the example render path (README).

| Level | Flags | Scope per README |
| --- | --- | --- |
| A1 | `--jerry=short,plain` | archival control, NOT ear-gated |
| A2 | `--jerry=plain` | characterized control |
| B | none (public default) | EAR-ACCEPTED (PR #111), provenance-verified byte-for-byte against parent f68c9bf |
| C1 | `--form=song` | characterized control, not ear-approved |
| C2 | `--form=song-intro` | characterized control, not ear-approved |
| D1 | `--form=song-narrative` | EAR-ACCEPTED ("sounding MUCH BETTER :D") |
| D2 | `--form=song-intro-narrative` | EAR-ACCEPTED ("hell yeah") |

Lead-muted `--no-lead` arms exist only for B, D1, D2.

**The lock does NOT cover** any pop-groove, pop-song, pop-band, or pop-rehearsal form. Those arms are
guarded only by their own relational tests, not by a WAV-byte lock.

### How to verify (DOCUMENTED, NOT RUN)

```
scripts/verify-c137-accepted-v1.sh             # verify; exit 0 = all 51 reproduce
scripts/verify-c137-accepted-v1.sh --calibrate # red-first: flips 1 byte, check MUST fail
C137_BIN=/path/to/c137_song scripts/verify-c137-accepted-v1.sh   # skip the build with a prebuilt binary
```

**WARNING (heavy):** without `C137_BIN` the script runs
`CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 cargo +1.99.0 build --release --example c137_song`.
That is a full release build. On a low-RAM box it can brick the machine. I did not run it.

**Drift is STOP/triage, never rebaseline.** The script prints "LOCK DRIFT DETECTED. This is a
STOP/triage, not a rebaseline.", only reads `SHA256SUMS`, and never writes it. Diff the `score.tsv` layer
to tell an audible-content change from a synth/master change. Re-freeze only if the change is
understood, intended, and (for ear levels) re-accepted. WAVs are local-only by repo policy; only hashes
are committed. Note the fingerprint blind spot: `Score::fingerprint()` (`score.rs:539` per README)
excludes tempo, total beats, beats per bar and sections, which is why the WAV bytes are pinned too.
A test, `tests/audio_c137_accepted_v1_lock.rs`, calibrates that blindness.

Pocket-line commits b0d7e39, d9a75d4, 8e7b22c, 4427b13 each state in their bodies
"C137_ACCEPTED_V1 51/51 byte-identical" (author-reported at commit time; I did not re-verify).

## 4. Reproducible render/test instructions (DOCUMENTED, NOT RUN)

Binary: `cargo +1.99.0 run --release --example c137_song -- <flags>` (the verifier builds it with
`--release --example c137_song`). Each lock level is exactly the verifier's `render()` calls:

```
c137_song --seeds=2112,7,99 --tempo=144 --out=DIR/A1 --jerry=short,plain
c137_song --seeds=2112,7,99 --tempo=144 --out=DIR/A2 --jerry=plain
c137_song --seeds=2112,7,99 --tempo=144 --out=DIR/B  --no-lead
c137_song --seeds=2112,7,99 --tempo=144 --out=DIR/C1 --form=song
c137_song --seeds=2112,7,99 --tempo=144 --out=DIR/C2 --form=song-intro
c137_song --seeds=2112,7,99 --tempo=144 --out=DIR/D1 --form=song-narrative --no-lead
c137_song --seeds=2112,7,99 --tempo=144 --out=DIR/D2 --form=song-intro-narrative --no-lead
```

Detail on `--no-lead`: the verifier's `render NAME yes` appends `--no-lead` to the same flags. It does not
replace the normal arm, and the sums cover both `full.normal.wav` and `full.nolead.wav` for B, D1, D2.

Opt-in experimental forms (flag list from `examples/c137_song.rs:201` help text): `pop-groove`,
`pop-groove-lift`, `pop-song`, `pop-song-lift`, `pop-band`, `pop-band-lift`, `pop-rehearsal`. Ledger
renders were at **104 BPM, warm palette**, seeds 2112/7/42 (rehearsal; holdouts 9/13). The exact
`--tempo` flag for those is inferred from "matched 104 BPM" in the ledgers: the full command line for
the ear-delivered MP3s is **UNVERIFIED** (loudnorm MP3 post-processing is described in the ledger, but I
did not locate a script). The rehearsal A/B render control is the env var `HOMOSAPIANS_NO_SYNCOPATE`
(ledger l.414).

Tests (documented only) in .pocket, each a separate `cargo +1.99.0 test --test <name>`:
`audio_c137_accepted_v1_lock`, `audio_c137_song`, `audio_homosapians_rehearsal` (19 per commit text),
`audio_homosapians_rehearsal_gap`, `audio_homosapians_harmony_audit`, `audio_homosapians_phase_law`,
`audio_homosapians_pocket_naturality_audit`, `audio_homosapians_comp_ownership` (7),
`audio_homosapians_drummer_agency` (3), `audio_homosapians_section_form` (6),
`audio_homosapians_tempo_sweep` (1), plus `audio_smooth_pop`. Counts are from commit bodies, not
re-run. Lab: `examples/homosapians_pocket_lab.rs`.

## 5. Known open problems (preserved, NOT to be fixed now)

1. **Timing / pocket phase.** The audit (`docs/research/HOMOSAPIANS_POCKET_NATURALITY_AUDIT_2026-10-19.md`
   sections 2-3) found `KEYS_ATTACK_COMP` 0.03 beat early (17.31 ms @104), bar-crossings, and Keys
   carrier-vs-comp self-flam. R1 metrology (`..._POCKET_R1_METROLOGY_2026-10-19.md`) measured no real
   latency. R2 (b0d7e39) fixed it as a typed opt-in for the rehearsal band only. **Still open:**
   `CompiledArgument::compile` defaults `comp_attack` to `LEGACY_KEYS_PULL`, so every non-rehearsal arm
   keeps the legacy pull; the owner "may still lead/lag the kit" note at Evidence Ledger ~l.449; no
   PCM-level equality proof (reverb memory bus confounds, so proof is Score-level only). The opening
   downbeat chord differs between lock and the emulation (lock is tighter).
2. **Drum response / agency.** R4 (d9a75d4) makes `DrumGesturePlan` audible but it is **not wired into
   `freeze` by default**; the commit says "promotion is ear-gated". Default rehearsal kit still ignores
   the drummer's recorded response. No ear verdict on the plan recorded anywhere in the repo.
3. **True polyphonic Keys.** Evidence Ledger l.281 names "true polyphonic Keys comp with zero Lead" as
   root cause of the fake-carrier problem; the shipped comp is capped at 3 guide-tone notes and sits under
   B4 (`COMP_CEILING`). No fuller piano/pedal model (audit R6 deferred).
4. **Harmonic-camp diversity.** Seeds 2112 and 7 share the i-iv world (ledger ~l.324); cross-seed spread
   2112 = 7 noted open. Camp is chosen from the riff, so diversity is bounded by riff diversity.
5. **Form witness.** `loop_bars == route.len()` made `pop_route_law`'s cycle check a tautology. R5
   (4427b13) adds `section_witness.rs` for the rehearsal backing, but `pop_route_law` itself is unchanged
   and the witness is inert for non-SmoothPop sources. Audit R5 also wanted the section-form witness to
   certify 2-bar vamp / closure; scope of what was done vs the audit's full ask is **UNVERIFIED** beyond
   the commit text.
6. **Lead / topline.** Rehearsal band is literal zero-Lead by owner lock. Adding a source-licensed lead
   "key" needs the owner to lift that lock (audit R5 "only after the rhythm band is ear-approved").
7. **Musical-quality gaps.** Syncopate push "hard to tell" (one 16th on one beat too timid; ledger
   ~l.426). 2112 riff character may be bland (ledger ~l.353). Smooth-pop song: flavour unpicked, deep
   sub-octave blocked by the MIDI-36 bass floor, non-blues colour needs a wider note pool, bars 0-2 octave
   pops still rise (johnnysoul ledger ~l.605, memory). HumanMusic beef-up gaps vs GOOD_MIDI (flat
   dynamics, rigid grid, thin rhythm vocabulary, static harmony, no hook restatement) are recorded in
   memory only; I did not locate the repo doc in the time spent: **UNVERIFIED**.
8. **Physical/FM execution (audit R6) explicitly DEFERRED.** GSPU-FM branch is docs-only (ledger ~l.212).

Where documented: `docs/research/HOMOSAPIANS_EVIDENCE_LEDGER.md` (450 lines),
`HOMOSAPIANS_POCKET_NATURALITY_AUDIT_2026-10-19.md`, `HOMOSAPIANS_POCKET_R1_METROLOGY_2026-10-19.md`,
`HOMOSAPIANS_PHASE_GRAPH_AND_FACTORISATION_2026-10-19.md`, `HUMANMUSIC_SMOOTH_POP_LEDGER.md`,
`HUMANMUSIC_C137_*.md`, all in LibGibson.pocket/docs/research/.

## 6. Regression-safety note

Music code paths (all under `src/audio/human_music/` in LibGibson.pocket, plus examples): `kernel.rs`,
`synth.rs`, `world.rs`, `theme_family.rs`, `argument.rs`, `argument_witness.rs`, `comp.rs`,
`comp_witness.rs`, `section_witness.rs`, `pocket.rs`, `pocket_time.rs`, `rehearsal.rs`, `score.rs`,
`bass.rs`, `percussion.rs`, `groove.rs`, `harmony.rs`, and `examples/c137_song.rs` +
`examples/c137_song/*.rs` (notably `rehearsal.rs`, `preset.rs`). Only critical correctness/regression
repairs, each gated on a clean C137_ACCEPTED_V1 verify.

**Correction to the premise on the zero-Lead linchpin.** I checked `kernel.rs:975` in LibGibson.pocket
(and the same text exists in .rehearsal/.homosapians per grep). That line sits in the *carrier law*:
`carrier_ok = p.carriers == [Agent::Lead] || (p.carriers == [Agent::Keys] && matches!(p.relation,
Depart{..} | Establish))`, which errors with "long-form carriers must be [Lead], or [Keys] on a Depart
(bridge) / Establish (intro)". It is a real load-bearing invariant (lines ~978-994), but it is the
long-form carrier rule, not a literal "zero Lead" check. Line numbers drift between worktrees; I verified
only .pocket. The actual zero-Lead guarantees are:
- `examples/c137_song/rehearsal.rs:21` and `:1179` (frozen Score carries literally zero Lead notes), and
- the test `tests/audio_homosapians_rehearsal.rs:358` asserting "the frozen backing has ZERO Lead notes
  (no fake quiet carrier)".
Other invariants: `validate_lane` (`kernel.rs:2530` at the time of the R4 ledger entry; line may have
drifted) refuses two bass notes in one lane and floors bass at MIDI 36. The comp register ceiling is B4
(`COMP_CEILING`) per the ledger. The `KEYS_ATTACK_COMP` constant was deleted in b0d7e39; the legacy
value now lives only as `Displacement::LEGACY_KEYS_PULL` in `pocket_time.rs`.

Default-off discipline: `drum_plan` defaults `None`; `comp_attack` is ZERO only in rehearsal `freeze`;
`comp_onsets` is `None` for C137, so C137 never reaches the comp site. Do not flip any of these defaults
to "improve" the music.
