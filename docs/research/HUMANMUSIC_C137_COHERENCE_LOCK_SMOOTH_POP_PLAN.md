# Operation: Coherence Lock -> Smooth Pop
## Source-grounded HumanMusic research and implementation contract (2026-10-14)

**STATUS: PLAN ONLY / DOCS-ONLY BOOTSTRAP.** This file neither implements smooth pop nor certifies a listening result. It is anchored to the inspected code at PR #114, branch feat/humanmusic-johnny-five-agency, exact parent **b536699ed97c6db4320874d4cf31fb229e90cf25** (successful exact-head CI run 38513193497). The planning branch is **research/humanmusic-c137-freeze-smooth-pop-2026-10-14**. It is stacked on #114, not on main. Re-verify HEAD and rebase deliberately if source advances. No changes to main, release/tag, default audio, public ABI, or historical fixtures are authorized by this plan.

The reference aesthetic is **original smooth, funk-influenced, tightly-produced pop** in the neighborhood of Sabrina Carpenter's *Espresso*, Calvin Harris/Katy Perry's *Feels*, and Charlie Puth's *Voicenotes*. Study groove, phrase, expression, and arrangement; do **not** reproduce their actual melodic, rhythmic, lyrical, production-master, or recording material. First artifact: a **new original instrumental** whose lead acts as a vocal proxy, not a claim to synthesized singing.

### Executive acceptance contract

1. **Keep every approved victory.** The accepted 16-bar C137 Full (the default *on its feature branch*) remains acoustically unchanged by all opt-in work. Preserve the 8-bar short/plain Jerry control, 16-bar plain, historical v0.4/R17 and cover corpus; do not silently enable rejected BassWeave/CoupledR8 or inert legacy policies. The current main branch is **not** the latest HumanMusic implementation.
2. **Freeze more than an equality between two code paths.** An external-to-candidate immutable Score/PCM reference, exact generation manifest, and physical renderer configuration must catch shared regressions. Ear verdicts must be recorded separately from machine evidence.
3. **Separate style from quality.** Pop does not mean C137-minus-notes; author a different lawful source grammar, harmonic closure, role salience distribution, rhythmic pocket, and orchestration. The expressive and causal guarantees generalize, not literal note arrays or fixed hardcoded chords.
4. **Gain relational sophistication under lower foreground throughput.** Every structural melodic note, anticipation, chord color, bass pickup, deliberate silence, and counterline should be supportable by source/harmony/history. Drums may have many quiet events; fewer prominent lead onsets are not equivalent to lower total events.
5. **Make the contrast falsifiable.** Predeclare predictions and negative controls, run a 2x2 complete factorial before claiming a groove x melody interaction, add held-out seeds and hostile tempos/keys, listen blind with loudness-matched A/B, and do not let metrics certify taste.
6. **Owner's ear is the promotion gate.** CI green is necessary, not sufficient. Preserve branch isolation and draft PRs. Do not promote smooth pop to any default, merge to main, publish media, or release without explicit owner authorization.

## 0. Baseline inventory: authority and limits

| Surface | Inspected state | Acceptance scope |
| --- | --- | --- |
| main | 80706c91b88e59495b131bfd8ab23aa3ed013414 | Old public default, NOT the latest C137 source |
| PR #111, feat/humanmusic-c137-expressive-transfer | f68c9bfca179f9f248dd5cd601f0c4d3987a1658 | 16-bar C137 Full accepted by ear; expressive lead, answer breaths, keys comp, bass anticipation, drums lead-echo and Jerry controls. Draft, not merged |
| PR #113, research/humanmusic-johnny-five-agency-2026-10-13 | 8f42ca70c47fb7a3b01b1b5f9e09f94c5714a44f | Source diagnosis, NOT runtime changes |
| PR #114, feat/humanmusic-johnny-five-agency | b536699ed97c6db4320874d4cf31fb229e90cf25 | True hearing/known-score label and keys-carried bridge in opt-in 32-bar narrative; bridge handoff **accepted by ear**. Keys-led narrative intro implemented but **listening pending** |
| CI, PR #114 head | https://github.com/femboy2112/libgibson/actions/runs/38513193497 | Seven configured CI jobs successful, not a pop listening test |

Source audit (line references refer to the pinned parent and MUST be re-checked on a moving branch):
- **examples/c137_song.rs:64-216, 430-500**: no-flag C137 Full; argument switches; source assembly; 144 BPM/BlackIce/A-Aeolian; Score and rendered WAV via HumanMusicSynth/OfflineRenderer at SampleRate::STUDIO; basic nonfinite guard; manifest.
- **examples/c137_song/preset.rs:110-303**: Jerry Law and separately resolved capability booleans. Preserve D1-D11 and independence controls in **tests/audio_c137_song.rs**.
- **examples/c137_song/source_song.rs:1-333**: 32-bar source, same verse/hook/bridge referents, repeated hook, source-bound route and optional bridge in Keys. **:335-580**: intro / keys foreshadow option.
- **src/audio/human_music/theme_family.rs:483-659, 750-835, 869-1027**: AEOLIAN-specific phrase grid and development; up to two inserted passing 16ths on eligible jumps; expressive gates, dynamics, shared pocket with hardcoded beat fractions. Different style source must not blindly inherit its note-adding bias.
- **src/audio/human_music/kernel.rs:893-1045**: exact sixteen-beat episode tiling, 0.06-beat admissible onset offset and a **specific minor V7/tonic route test**. The smooth-pop loop cannot just change MusicWorld.mode or tempo to Ionian and pass.
- **src/audio/human_music/kernel.rs:1265-1519**: Keys-carrier pass, source/actual hearing, memory-law replies, earned return. In the source-bound bridge-return path **:1445-1459**, the hanging-tone anchor reads **Role::Lead in the bridge**, even if that bridge was lawfully carried by Keys; then falls back to tonic. The bridge material is still quoted elsewhere; do not say the *whole* return is broken. Test whether anchor-specific counterfactual dependence is lost and repair in an opt-in arm first.
- **src/audio/human_music/kernel.rs:1535-1651**: hook/return rootless keyboard comps, limited windows, possible 9th/ornament; this is an ACCEPTED C137 idiom, not necessarily a universal pop comp duty.
- **src/audio/human_music/kernel.rs:1663-1807**: bass has root anchor on every bar; resting root+fifth vs quarter-note walk and anticipatory beat four. A new bass-hook owner must not secretly violate the foundation guard (**:2260-2269**).
- **src/audio/human_music/kernel.rs:1809-2114**: metric kick/backbeat, eighth/sixteenth hats and fill patterns plus keyed replies; physical drum synthesis owns one voice per drum kind. An existing accepted test **tests/audio_c137_song.rs:1643-1675** pins FOUR same-sample snare collisions of pulse then lead-echo, not evidence collisions are musically desirable in *new* styles.
- **src/audio/human_music/kernel.rs:2115-2179, 2185-2300**: pad, commitment and source/Score validation, stage/role capacity, drum/provenance checks. Verify these contracts before extending.
- **src/audio/human_music/world.rs:115-153**: MusicWorld holds palette/mix/tempo/swing and harmony vocabulary. **kernel::perform_song** calls theme_family::pocket_offset directly rather than world.swing or a true configurable groove profile. World swing is not a magical C137 pop-style dial.
- **src/audio/human_music/argument.rs, argument_transport.rs, argument_witness.rs**: typed referents/occurrences/partial transforms, reserved carriers, independent source content witness. Do not alter a source event and then ask a provenance tag to absolve it.
- **src/audio/human_music/interaction.rs:127-186**: Response::knowledge distinguishes authored future vs performed past by latency (a useful initial truth repair; harder proof requires explicit authorized reads).
- **src/audio/human_music/synth.rs:547-630**: voice pools / physically rendered drums / stem masking / production. Bus meters under a stem mute are not a witness of actual post-mask audibility. Sample triggering, gain, FX tails and masking matter.
- **docs/HUMAN_MUSIC_SONGCRAFT.md, HUMAN_MUSIC_BAND_STORY.md, docs/research/HUMANMUSIC_JOHNNY_FIVE_MISSING_AGENT.md**: ear-derived origins and explicit separation of scripted response from genuine bounded local choice.

### Epistemic ledger

**Observed**: latest #114 code and exact-head CI; owner-ear accepted the 16-bar C137 features and the 32-bar lead-to-keys bridge handoff. **Unverified**: new pop composition or audible improvement; new immutable cross-revision acoustic baseline committed to source control; keys-carried bridge anchor counterfactual failure in running Rust; whether the current percussion/mastering washes out a subtle bass groove. Do not substitute expected outputs for runs.

## 1. R0 — Golden lock before the first sound-changing edit

Create a standalone, protected **C137_ACCEPTED_V1** fixture namespace with:
- Exact baseline commit/branch; full resolved effective-feature manifest; input seed(s) **2112, 7, 99**; renderer sample rate/block, channel format, tempo, tonic/mode, world/palette and production switches; OS/toolchain/feature configuration, script/command, file lengths.
- **Externally pinned** Score canonical fingerprint, stable score dump SHA-256, **WAV SHA-256**, preferably also deterministic raw PCM hash and selected per-stem hashes. Store actual expected values from a baseline render of the ACCEPTED source; never invent/check in placeholders. The existing short hexadecimal score fingerprints in PR prose are *not* WAV SHA-256s.
- The immutable reference rendered or a verified durable archive with retrieval instructions and checksum (do not bloat git with multi-megabyte WAVs if unnecessary). Hash-only manifests catch drift but do not allow ear A/B if masters are gone: retain accessible audio somewhere durable.
- Separate lock levels: **A** 8-bar archival short/plain and 16-bar plain; **B** 16-bar accepted Full; **C** 32-bar non-narrative song/intro as characterized current controls, not automatically ear-approved; **D** 32-bar narrative bridge *accepted in scope*, and narrative intro *pending ear*. Fingerprints of D must not be mislabeled as final golden approval.
- Script/test verifying frozen hashes on pinned deterministic runner; independent source/Score relations; an evidence JSON/TSV ledger linking each fixture to actual command and artifact location.
- **Red-first calibration**: flip a note pitch/onset, gate, carrier identity, physical WAV sample, or Jerry capability and prove the appropriate witness FAILS; leave a harmless metadata-only mutation passing its intentionally narrower witness. Include valid tempo/key controls under the correct domain; mismatch is a refusal, never silent fallback.
- A release/PR check that historical default/v0.4/R17, cover and C137 fixtures remain unchanged. Distinguish Rust unit tests, actual WAV hashing, and ear acceptance. Do not let a generated candidate replace/update its own accepted goldens in the same command.

Concrete initial commands (the renderer command already exists):
~~~
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --test audio_c137_song
cargo test --test audio_canonical_fingerprint
cargo test
cargo run --release --example c137_song -- \
  --out=target/c137-lock/accepted-full --seeds=2112,7,99 --tempo=144
cargo run --release --example c137_song -- \
  --out=target/c137-lock/archival-short-plain --seeds=2112,7,99 --tempo=144 \
  --jerry=short,plain
~~~
Use new empty output dirs per invocation (the CLI rejects nonempty dirs). If scripts for all required controls are absent, implement them without modifying the accepted candidate generation path. CI currently invokes cargo test and builds examples; a dedicated acoustic verification step must be deliberately added.

**R0 pass**: every actual fixed reference hash reproduces, calibrated positive/negative mutations behave as predicted, and baselines are archived and labeled by acceptance scope. A baseline mismatch is a STOP/triage, not a permission to rebaseline.

## 2. R1 — Style-neutral lawful core; DO NOT relax C137 validation globally

Current bottleneck: C137 controls when a musical argument must reach a **literal tonic** by a fixed minor/dom7 chart at selected bars. Smooth pop can form a stable, comprehensible *cycle* without that exact chord. Needed concept:

    Closure = (thematic, harmonic, groove-cycle, narrative)

These are distinct, typed conditions. Define a new internal, opt-in **PopLoopDomain** and **PopSourceGrammar**, with a documented positive admissibility contract; retain **C137ArcDomain** byte-identical. The exact Rust interface is provisional; prefer existing types and additive internal wrappers before touching public ABI. Crucially, audit **argument_witness::harmonic_violations**, **source_harmony**, **longform_domain**, the composer, and the source builder together. The validator/witness should reconstruct the same declared style-specific harmonic object independently from actual Score, not merely trust new labels.

Candidate loop family for probes: four-bar functional pop or jazz-pop colors (major/minor sevenths, tasteful ninths/sus, slow harmonic rhythm), and a second loop without explicit tonic-bar cadence. Do not copy a reference song's exact riff/hook/chart. An intentionally non-tonic groove-cycle may close *groove* and *theme* while leaving harmonic motion open; it must report those dimensions honestly.

**R1 discriminating tests**
- Existing C137 valid/invalid source corpus has the *same* verdict and frozen sound.
- Positive: a lawful major-key repeating loop with a melodic landing and true groove-cycle return is admitted by PopLoopDomain, witnessed in Score.
- Negative: same declared loop with a wrong functional chord, an unsourced semitone, mismatched source times, erased source silence, or fabricated resolution is refused.
- Mutation: replace a real return with same notes/count but incorrect thematic or dependency source; a content-aware witness rejects it.
- Transport: allowed transposition or tempo transport preserves symbolic relations only within declared domains; it never claims bit-identical PCM.
- Explicitly test an intentionally unresolved harmonic return; valid if the new source declares it, INVALID if claimed fully harmonic-resolved.

**R1 pass**: generalization adds capabilities without weakening a single old C137 gate. If source or closure witness cannot observe what the new profile asserts, stop; fix observation first.

## 3. R2 — A band-led groove, bass identity, and conscious negative space

Write the first four/eight bars **without any lead**. This is the shortest discriminatory probe: can kick/snare/bass/keys create a recognizable and interesting original pocket without foreground virtuosity?

- Approximate first test **104 BPM, 4/4**, bounded alternatives around 96-116 BPM, not a fixed truth about smooth pop.
- Constant clear downbeat/backbeat scaffold, selective eighth/sixteenth texture and ghosts, a justified *syncopated bass cell* with its own recurrence/anticipation identity, sparse chord stabs or plucked accents that occupy the spaces.
- Preserve a common metric carrier/pulse; allow small *role-specific coordinated deviations relative to that shared base*, not independently sampled per-role jitter. Distinguish beat offsets from milliseconds, and satisfy existing source lattice/monophonic limits where retained.
- Bass may own a rhythmic motif WITHOUT being a full required argument carrier: the current source transport for a Bass-carried referent and kernel's mandatory bar-downbeat root need reconciliation before claiming bass can carry an arbitrary whole melody. Keep harmonic foundation and bounded voice occupancy, or explicitly version a new independently witnessed alternative.
- Build **GrooveLaw** as a source/performance relation used by the pop path. The existing C137 beat-offset function stays untouched. Do not describe world.swing as active on perform_song before proving it.
- For a pop drummer, explicitly arbitrate kick/snare sample-time collisions and hat chokes; the four historical Full collisions are preserved in the old path only. Protect tiny intentional ghosts from being inaudible, masked or promoted to full-velocity crashes.

**R2 proof-by-mutation**: intact bass pickup -> expected next harmony; move pickup to an unlicensed downbeat or remove it and predict the changed anticipation; add independent clock jitter and predict pocket quality reduction; mute lead and confirm band groove/harmonic identity persists; silence a strategically relevant keyboard stab, see whether the phrase loses a response.

## 4. R3 — Write the SPARSE lead at the SOURCE, not as a post-hoc thinning filter

The present theme_family::compose_phrase has fixed head/breath/cadence grids and embellish_phrase tends to add up to two extra 16ths per leap. A pop study needs new authored source shapes, not a density slider that drops *required* C137 events after commitment:
- Short repeating melodic hook/germ, vocally playable ranges, displaced entrances, syncopated syllable-like attacks, quiet connective motion, held tones with genuine cutoffs and strategically unfilled gaps.
- Some verses may state a fragment while the chorus completes it; all thematic carriers cite the SAME source family under lawful transformations. Preserve identity across subsequent refrains.
- Keep a small **foreground salience budget** (accent hierarchy, register, exposed pitch, timing placement, sustain) separately from onset density and actual relational responses. A quiet hi-hat sixteenth is not interchangeable with a lead climax in this budget.
- Harmonic color is licensed by the real active chord/scale or a declared intentional tension and later release; never force 9ths over an incompatible harmony. Keys/bass should leave lead attacks physically audible; watch six-voice synth pool and sustained FX tails.
- No fabricated lyric intelligence or singing engine: the pilot is instrumental with a vocal-proxy lead.

**R3 gate**: compare sparse authored version with dense authored version under the SAME backing, same harmonic path/tempo/total loudness; structural hook identity and source witness pass; musical novelty cannot be credited merely to an extra note or louder mix.

## 5. R4 — Johnny Five initiative: bounded local choices, not a canned script

Only after R2/R3 actually sound promising, integrate the existing Johnny Five research: state at decision time includes authored score (what a rehearsed player may know), actually committed history (what has sounded), unmet thematic/call obligations, role duty, groove state and physical voice tails. Use the existing **EffectiveHearing** and **Knowledge** distinction, but assert explicit read provenance in tests.

Possible legal actions: **State / Support / Withhold / Prepare / Anticipate / Answer / Develop**, each with a typed feasibility check (source relation, latency, heard-history boundary, stage capacity, no collision), bounded alternatives, commit, and a reason/source witness. **Withhold** is a genuine winning choice, not default idle behavior. Avoid generalized "AI score", hidden post-fixes, independent random clocks, and made-up psychology.

Test controlled counterfactuals: two legal earlier calls with identical aggregate energy/density but different recognizable figure MUST permit different later source-bound decisions; a future-only *unheard* change must not affect a purported reactive answer; score-known future may legitimately affect a rehearsed anticipation if labeled correctly. Repartition a bar without changing music and require memory/duty to compose across the boundary (no accidental reset).

Do not concurrently rewrite Johnny Five's active PR #114 kernel while another agent owns it. Use a single integrator, worktree isolation, and a narrow reviewed integration seam when its mainline development stabilizes.

## 6. R5/R6 — Song, arrangement, timbre, and ear gate

Prototype 32 bars: **Intro 4 -> Verse 8 -> Pre 4 -> Chorus 8 -> Break 4 -> Return 4** at roughly 104 BPM (approximately 74 s). Separate this proposal from the current eight rigid 16-beat C137 episodes: if the new form layer does not support mixed section lengths, develop a proper typed form adapter and tests instead of lying about the section segmentation. A 4-bar section is not an automatic 16-beat "episode" in a new compositional grammar.

Sound world: clean tight kick/snare, articulate syncopated bass, dry/discrete keys or pluck-like stabs, tasteful sustained harmony, sparing bright accents, clear instrumental lead. Use MusicWorld instruments and HumanMusicSynth first, then add patch-level improvements only when the ear/PCM identifies a genuine timbral obstruction. A guitar-like rhythmic part may be realized with existing keys/pluck synthesis; do not claim physical guitar authenticity.

Chorus returns: preserve the recognizable hook but change something **source-licensed** (a bass inversion/pickup, key counterline, reharmonization, delayed answer, handoff) rather than adding a generic fill/loudness every cycle. Progress across return(s) must consume the right source debt; do not treat the three static post-bridge repeated Return objects as evidence of progressive story.

Deliver per-seed full WAV + stem/lead-muted mixes; Score TSV/canonical fingerprint; sources/arguments; effective-feature manifest; role/time/velocity/silence/groove data; acoustic diagnostics; fixed-seed and held-out comparison reports. Do not silently normalize out differences in loudness, tempo, arrangement, or length during blind A/B.

## 7. Predeclared complete contrast matrix and falsifiers

Separate **necessary machine gates** from human music judgment; no scalar "semantic intelligence" score.

| Probe | Fixed factors | Mutated factor | Predicted evidence; fail/ambiguous cases |
| --- | --- | --- | --- |
| LOCK | exact historical manifest | new code revision | same frozen hash / Score facts; any unexplained bit change FAIL |
| M0-M3 (complete 2x2) | tempo, harmony, source seed, form, world, playback loudness | Dense vs sparse lead x current vs pop shared groove | all FOUR cells required before interaction claim; missing cell => no mixed-term conclusion |
| POCKET | same score/notes | common coordinated phase vs independent jitter vs dead grid | audibly coherent pocket preferred under matched playback; metrics alone AMBIGUOUS |
| BASS | same melody/drums | source-bound anticipation vs generic/shifted pickup | later expected chord relation changes; no audible distinction => hypothesis ambiguous |
| SILENCE | same source context | licensed Withhold vs extra busy fill, matched salience when possible | intentional silence preserves intelligible statement; if fill wins, revise |
| HOOK | same total foreground events | lawful transform vs same-count scrambled note identities | source witness must reject scramble; blind listeners should recognize lawfully recurring hook |
| CARRIER | same section energy | actual thematic handoff vs same-count flat lead-overdub | independent lead-mute must reveal the handoff; waveform effect not enough to prove recognition |
| RETURN | two lawful altered bridges, same final hook | bridge A vs B | source, authorized carrier and return keys anchor reflect bridge content; falling back to tonic unconditionally FAIL for anchor-specific claim |
| LOUD | same score | matched/mastered levels, stem normalization control | avoid "better because louder" false positive |
| SCOPE | different keys/tempos and fresh seeds | transport/recomposition | invariants transport lawfully; do not require bitwise same PCM |

**Before each probe**: declare PASS / FAIL / AMBIGUOUS outcomes and exact observer; include positive, negative, null and mutation controls. Do not tune parameters on holdouts. Use 2112/7/99 for regression and initial development; reserve **fresh unseen seed families** (chosen/reported prior to listening) plus a separate hostile key/tempo regime for adjudication. The same cherry-picked seed cannot validate generality. All comparisons keep reference quality and their source provenance explicit; do not treat three same-model passes as three independent musical witnesses.

Useful observables, deliberately separate: authored source identity; independent Score content witness; per-role foreground attacks and salience; inter-role timing offsets; aggregate and role onset counts; deliberate silent windows; harmonic-loop position and color license; same-sample snare collisions; voice-stealing and releases; post-mask stem audibility; finite/non-clipped PCM; blind ear verdict (with reasons and confidence). The only truthful claim about originality is source-grounded construction and comparison; do not use a guessed novelty statistic as copyright clearance.

## 8. Dependency order, branch discipline, and stop rules

~~~
R0 historical freeze + independent PCM/mutation gate
 |
 +--> R1 source/closure/domain and witness refactor (additive, opt-in)
 |      |
 |      +--> R2 lead-muted groove/bass 4-8 bar vertical slice
 |      |      |
 |      |      +--> R3 sparse source melody + full 2x2 A/B
 |      |                |
 |      |                +--> R4 actual role-choice/memory protocol
 |      |                          |
 |      |                          +--> R5 32-bar form/arrangement -> R6 holdouts/listening
 |
 +--> historical byte lock runs after EVERY rung
~~~

**STOP** on any of these: frozen WAV drift; source-observation contradiction; ghost feature claimed active while inert; unresolved clip/collision or silent-note masquerading as music; valid C137 rejected by refactor; a style-specific exception leaking into ordinary C137; no distinguishable musical improvement under blind matched listening. Fix the smallest implicated seam or retract the claim; never weaken the witness to make the test green.

Worktree strategy: this research branch is docs-only on PR #114 head. Implement on **impl/humanmusic-smooth-pop-<date>** from a refreshed approved compatible parent; checkpoint small PRs and leave main untouched. One principal integrator edits the kernel; parallel scouts audit acoustic baselines, source grammar, pop groove, causal memory, and independent witnesses without racing writes. Use pinned commit/branch, distinguish source audit vs executed test, and retain all raw Score/WAV/fixture diagnostics. No release/ABI or default change without user approval.

## 9. Concrete Claude deliverables

1. A true **C137_ACCEPTED_V1** versioned lock manifest + reproducible generate/verify tool, calibrated with external-to-candidate reference hashes and mutation tests; candidate comparison cannot update the fixture.
2. A **SmoothPopDomain** or equivalently narrow additive grammar + independent source/harmonic witness; C137 hardcoded domain still intact.
3. An original **instrumental 4/8-bar lead-muted groove** and **sparse-lead contrast** with matched 2x2 factorial controls; actual stems/Score/manifest produced.
4. A cautiously integrated **source-aware carrier/return audit**, documented pass/fail (and optional gated repair) for the keys-carried bridge anchor.
5. An ear-gated **32-bar original pop song study** (Intro/Verse/Pre/Chorus/Break/Return), a reproducible blind listening bundle, negative controls, fresh holds, and precise scoped verdicts.
6. Small coherent commits, green relevant CI, draft PR(s), a living status ledger with exact head/commands/run results, known regressions and next verdict-changing probe. Do not fabricate runs or accept your own listening verdict as the maintainer's.

### First implementation cut (highest return)

**R0 -> R1 -> R2 only**, with minimal tests and a 4/8-bar rhythm-section WAV. This demonstrates whether the old categorical source/closure machinery can host a truly different groove before we sink effort into a new voice synthesizer, 32-bar arrangement, or an overgeneralized style system. The full R3-R6 roadmap remains the target, each rung requiring its own evidence.

**Principle:** Protect the accepted exact music; transport source identity, causal duty, and common pulse into a new style; earn additional complexity through expression, groove, and meaningful relationships—not by maximizing notes per second.
