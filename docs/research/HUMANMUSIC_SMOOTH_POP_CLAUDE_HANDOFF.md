# CLAUDE EXECUTION HANDOFF — C137 Coherence Lock -> Smooth Pop
**Status:** handoff instructions only; no code or acoustic fixture implemented on this planning branch.

**Authoritative plan:** docs/research/HUMANMUSIC_C137_COHERENCE_LOCK_SMOOTH_POP_PLAN.md

**Repo:** femboy2112/libgibson  
**Plan branch:** research/humanmusic-c137-freeze-smooth-pop-2026-10-14  
**Inspected Johnny Five base:** feat/humanmusic-johnny-five-agency @ b536699ed97c6db4320874d4cf31fb229e90cf25, PR #114  
**Parent accepted C137 branch:** feat/humanmusic-c137-expressive-transfer @ f68c9bfca179f9f248dd5cd601f0c4d3987a1658, PR #111  
**Success CI on Johnny Five base:** https://github.com/femboy2112/libgibson/actions/runs/38513193497  
**Current ear gates:** C137 16-bar Full accepted; 32-bar narrative lead->keys bridge handoff accepted; 32-bar keys->lead intro implemented but pending ear; smooth pop entirely untested.

## Mandate for Claude

Act as an executing principal engineer, music architect, adversarial auditor, and mathematical formalizer. This is NOT a planning-only request: implement **R0 freeze**, then **R1 opt-in PopLoop-domain/source contract**, then **R2 original lead-muted groove** with its test A/B/stem bundle; proceed to R3 sparse source melody and controlled complete factorial only if the preceding gates genuinely pass. Subsequent R4-R6 are documented and should be advanced only by verified evidence and owner listening approval. Be ambitious in hypotheses; restrictive in claims.

Before touching code, inspect actual repo/status/worktree/branch HEAD and open PRs; preserve uncommitted edits; read repository instructions and the master plan; verify parent ancestry and newly advanced Johnny Five work. Work from a NEW implementation branch/worktree based on the plan branch or a deliberately reconciled compatible commit, not main and not directly on another engineer's active worktree. One primary integrator owns kernel modifications; orthogonal scouts may audit source, render, categorical contract, and controls. Keep implementation source changes small and cross-check ownership.

### Nonnegotiable preservation
- No changes to the old default generation path except tightly gated, sound-preserving audit instrumentation; no main merge, release/tag, ABI change, broad dependency churn, or auto-enable.
- Freeze accepted PCM and Score under a **reference outside the candidate code's calculation**. Do not claim relational Score equality alone is immutable audio protection. Do not fabricate WAV hashes; render accepted parent with exact manifest and physically verify. If old artifacts cannot be accessed, label it an access gap and characterize fresh renders honestly.
- Pin seeds 2112/7/99 and all historical controls. Preserve v0.4/R17, cover, C137 Full, old Jerry controls; distinguish the scope of accepted narrative bridge from unapproved intro.
- Do not worsen lawful source/Score causality or conceal mismatched notes under metadata. Do not weaken a validator just because new style cannot yet be admitted.
- Never promote an arbitrary user-specified artist recording or copyrighted MIDI as the target source. Deliver original instrumental pop with constrained musical idiom.

### Detailed ordered work

**R0 / acoustic lock, red first.**
Capture reproducible accepted 16-bar Full and 8-bar short/plain baselines at source SHA, plus 16-bar plain; characterize 32-bar static/narrative variants separately. Archive manifest (SHA, flags, compiler, sample rate/block, exact seeds, hash scopes), immutable reference Score/PCM data and A/B audio if available. Implement generator+verifier with separate read-only verification mode, negative sample/note/provenance/Jerry mutation tests, CI acoustic lock. Distinguish canonical Score fingerprint from actual WAV SHA-256. Existing test_audio_c137_song D1-D11 are necessary but not sufficient.

**R1 / style-neutral source and closure seam.**
Write red first: current C137 legal and illegal acceptance unchanged; a lawful pop major-seventh/funk loop with nonliteral tonic closure is accepted in a NEW typed domain; a falsified chord/unsourced tone/wrong source obligation or false harmonic-resolution label is rejected. Refactor only what the test proves necessary. Audit longform_domain (kernel.rs:911-1045), argument_witness::harmonic_violations, argument_transport, source_harmony, MusicWorld, and source_song builders as a coupled gate. Preserve current 16-beat C137 episode semantics; pop Intro 4/Verse 8/Pre 4/Chorus 8/Break 4/Return 4 is a distinct typed form; don't lie about segment sizes.

**R2 / 4-to-8-bar bass-led loop.**
Construct genuine new pop source/harmony (not just new world patch at 104 BPM). Keep a coherent kick/backbeat, add rhythmically interesting and source-justified repeating bass/pickups, discrete keys/plucks and intentional silence. Validate source identity, harmonic foundation, drum same-sample behavior, and source-specific arrival preparation; render an audible lead-muted band-only WAV and stems. Proposed GrooveLaw must be parameter-real (currently kernel uses hardcoded theme_family::pocket_offset independent of world.swing). No independent random-jitter "humanization." If a bass motif is promoted to a required carrier, handle the downbeat-root foundation law and source witness rather than bypassing them.

**R3 / sparse vocal-proxy lead and factorial.**
Author low-salience-dense but musically expressive hook from SOURCE, with rests, syncopation, holds, dynamical arcs, earned responses and same germ on later restatement. Create all four necessary A/B arms: dense vs sparse source lead crossed with old vs new shared groove, keeping tempo/form/harmony/seed/mix controlled; include grid/jitter, scramble, source-incoherent, extra-fill, bridge-A/B and stem-mute contrasts as independent negative controls. Predeclare acceptance/rejection and hold out unseen seeds. Tests can reject; only maintainer ear can certify the new style.

**R4-R6 / only on verified foundation.**
Bounded performer choice among support/withhold/answer/etc using actual past vs score-known future, source identities and causal debts; source-aware Keys bridge anchor repair test (kernel.rs:1445-1458 currently consults Role::Lead even when bridge Keys-carried), optional progressive returns with real multi-debt, 32-bar original pop form, sound-world fit, physical rendering/mix checks and blind listening.

### Named falsifiers that MUST be written

1. Old accepted lock: new runtime cannot silently change frozen PCM.
2. Pop domain admission: passing does not amount to removing C137 minor/tonic guard.
3. Wrong-note/source identity mutation fails an independent raw Score witness, even at fixed note count.
4. Carrier-handoff signature survives a lead mute, not simply a provenance label.
5. Harmonic return quote and hanging-tone anchor read true declared Keys source on Keys-carried bridge; source-specific bridge A/B moves the desired output without changing prior section.
6. Quiet-note and active-stem changes have actual audible PCM effect; note existence/velocity tag alone is insufficient.
7. Intro and chorus style dials do not regress unrelated v0.4/cover/accepted C137.
8. Matched 2x2 complete factorial prevents attribution of a groove x melody interaction from an incomplete set.
9. Key and tempo transports stay in declared lawful domain with appropriate source/Score judgments; do NOT assert bitwise same PCM under transported parameters.
10. Deliberate Withhold wins over an available legally busy fill at least once; otherwise do not claim restraint-as-decision.
11. Every new advertised flag has a source-to-Score-to-PCM witness on at least one fixture; flag that is requested but idle is labeled honestly.
12. Holdout seeds/hostile regimes are not used to fit the generator before validation.

### Build and media commands (run, do not pretend)

~~~
git status --short --branch
git log -1 --format='%H %s'
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --test audio_c137_song
cargo test --test audio_canonical_fingerprint
cargo test
cargo run --release --example c137_song -- --out=target/c137-lock/parent \
  --seeds=2112,7,99 --tempo=144
~~~

Use per-run empty output directories; the example will refuse a nonempty directory. New proposed CLI options (e.g. style=smooth-pop) DO NOT EXIST YET; implement their end-to-end semantics and manifest truth before advertising them. Compare pinned accepted parent vs new candidate with the same generation recipe, and keep sample rate/PCM encoding constant. Use exact-head CI reports and full output records. If the local ALSA headers or toolchain for all-features Clippy are absent, declare the environment limitation and use GitHub Actions rather than claiming it passed locally.

### Reporting and pushing

Commit cohesive verified steps to the new implementation branch; do not overwrite active branch or merge to main. Open a draft PR against the plan branch (or another explicitly justified stacked feature base). Track source hash, test commands/results, WAV/hash manifest, active/not-yet-audible features, A/B paths, and what exact experiment would overturn the working hypothesis. Keep **observed/verified** separate from **ear approved** and **proposed**. Explicitly list any work not accomplished; continue research until a genuine first pop audio vertical slice exists or a sharp obstruction/constraint is isolated. The ideal deliverable is an R0-locked, testable R1/R2 implementation with actual new WAVs and independent witnesses, followed by a narrow ear packet and the next recommended experiment.

**Critical research question:** Can a categorical, source-faithful band express *more meaningful musical information per salient foreground event* through groove and voice interaction, without either losing the old C137 acceptance or inventing musical success from a self-certifying scalar? Design the test so the answer may be no.
