# HumanMusic C137 — integrate the causal kernel, then make it WRITE A SONG
## Claude implementation and safe local-worktree consolidation handoff

**Research only.** Audited source: \`feat/humanmusic-c137-causal-kernel@cfd5aea3243da2156681590e98c482c61a67018e\`, draft **PR #104**, based on \`feat/humanmusic-c137-ensemble-agency@29527a891ed7bfed9657a99046884a61670897e3\` (PR #102). Independent architecture audit #103: \`12eb9f2b72c80ff116bcf584aacfe0751bde286a\`. PR #104 exact-head [CI run 38202899251](https://github.com/femboy2112/libgibson/actions/runs/38202899251) **PASSED all 7 jobs**. The **runtime/audio source** was frozen earlier at \`ac97bccba88beb768720c67ac64200f6778d0584\`; the later commits to PR #104 are documentation/evidence. Refresh all refs before changing anything.

**Human verdict:** New ~9-second causal conversation clips sound **more dynamic but information-sparse**. This is encouraging, not C137 acceptance. The target remains the seven accepted \`teacher_golden7\` handcrafted renders, *not merely a formally coherent 4-bar experiment*. This author inspected GitHub code, hosted CI and retained acoustic receipts, **not the local PCM or actual accepted golden7 assets**. Codex's measurements below are independently documented by its source-side pass, not remeasured here.

## 1. What's actually good, and MUST be retained

Codex implemented a meaningful small causal kernel rather than a batch of after-the-fact note changes:

- \`kernel.rs\` imports the existing \`SongMap\`, \`CompiledArgument\`, source transport, \`PerformancePlan\`, \`Stage\`, \`ActionPlan\`, \`InteractionMaterial\`, \`Score\`, and \`HumanMusicSynth\`. It does **not** require another songwriter ontology.
- \`AdmissionInputs.argument\` now installs the final source-owned harmonic route BEFORE the generic planning decisions on this opt-in path, without changing historical public entry points.
- \`CommittedPerformance\` owns immutable role commitments; exported candidate mutations or cross-role false hearings are refused. \`EffectiveHearing\` includes actual event IDs, gates, velocities, function and continuity in bounded consumed windows.
- Source-bound lead→keys and keys→drum replies have demonstrable causal consequences: deleting the keys answer and **re-performing** removes two later dependent snare hits, while related unaffected lead/bass and 48 pulse drum events stay identical. Simply exporting an after-the-fact keys mute is correctly rejected, not reported as an equivalent take.
- Bass is a single chronological foundation lane with a licensed approach, no enabled low \`BASS_WEAVE\`, no uncontrolled rapid retriggers or note overlaps. This honors the maintainer's bass-glitch rollback rather than resurrecting a killed route.
- The default \`perform*\` paths and old samples stay byte-exact. 1,681 Cargo tests passed with 22 intentionally ignored, 19 example-specific checks, 48 R17 archived WAV hashes and 757 diagnostic receipts replayed exactly. Exact-head CI green across MSRV, Rust, parsers, bindings, ASan, PTY and Go.
- 81 real 48kHz PCM16 WAVs, 18 Score TSVs, SHA256 and copy custody were produced; **actual media are local**, not GitHub-distributed. Check before moving or cleaning their worktrees.

The authoritative detailed handoff is [PR #104 HANDOFF](../../research/c137_kernel/HANDOFF.md) and [acoustic candidate report](../../research/c137_kernel/acoustic_candidate.md) on the PR #104 branch. Keep the aggressive hostile red/green tests and the earlier R8 \`CoupledR8\` ear-rejected negative control.

## 2. Why these samples are ~9 seconds and sparse — exact source facts

At 144 BPM the music is only **16 beats / 4 bars = 6.667 seconds**. The Synth appends a **2.5-second release/reverb tail**; that is why the WAV is **9.167 seconds**, not a 9-second composition.

The first kernel explicitly refuses anything except:
- \`song.plan.form.total_beats == 16.0\`;
- four bar chords and exactly TWO source placements;
- source reservations \`[0,12)\` and \`[12,16)\`;
- tonic return in bar four, specified semitone/pitch domain;
- quarter-beat onset cell and source events fully within the first bar's musical statement/breath.

The example builds a rich seeded \`ThemeFamily::generate_pair\`, then deliberately keeps only **\`pair.verse[..4]\`**. That is a fine deterministic experiment for establishing a readable call and return, but it has discarded the distinct hook and bridge's long-term material. Its keys decision collapses many possible motifs to only \`Silence\`, \`Invert\`, or \`Quote\` based on “tonic-close / last pitch above first / otherwise”; keys enter at fixed **beat 4.0625**, and the second opportunity is always silent **8–12**. Bass choices derive from the same four frozen chords with 2–3 notes per bar; the kit plays near-fixed hats/kick/snare over all four bars, plus up to two echo responses; pad is always silent.

Codex's actually measured candidate score (seed 2112/7) is:
- Lead attacks per bar: **4, 0, 0, 5**; only **29.69%** written occupancy.
- Keys: **0, 4, 0, 0**; **10%** written occupancy.
- Bass: **2, 2, 3, 2**; **53.13%** written occupancy.
- Drums: **12, 12, 14, 12**.
- Pad: **zero**, by design.
- Normal full-mix RMS by bar: 0.141940 / 0.143189 / 0.146648 / 0.146146, only **0.2835 dB** max/min spread (unweighted).

**Seed variation is severely compressed.** Seeds 2112 and 7 have distinct source/debug identities but **IDENTICAL Score TSV and all 8 corresponding audio variants**. Seed 99 changes lead notes; keys/bass/drums/pad stems remain **byte-identical**. Thus three seeds are two unique candidate Scores, and no broad generative ensemble variation has been demonstrated. This is not a probabilistic curiosity: we are discarding most thematic source material before performance and using role policies that read only a small quotient of what survives. The correct next repair is *more expressive source-conditioned decisions*, not arbitrary random texture to force nonmatching hashes.

Codex correctly does not claim that the four bars are a complete ThemeFamily or general player state, and explicitly deferred R3 long-form generalization and R5 human acceptance. DO NOT label these samples failed or claim that they already reached C137: they validate the causal performance seam.

## 3. The next abstraction is NOT “an array of the kernel song repeated N times”

**We need a long-form arrangement that composes small causal conversations without resetting their state.**

A four-bar episode is a partial state transformation, conceptually:
\`(SongContext_k, PlayerMemory_k, OpenDebts_k) -> (CommittedMusic_k, SongContext_{k+1}, PlayerMemory_{k+1}, OpenDebts_{k+1})\`.

Use the existing \`ThemeFamily\` VERSE/HOOK/BRIDGE, \`MusicalArgument::fusion\`, \`ArgumentArc\`, real harmonic route, \`ActionPlan\`, Stage, AccentGrid and source IDs to assign the next episode's meaning and harmonic journey; do not invent a fourth songwriting engine. Every episode has a real **entry and exit context**:
- absolute time and shared beat phase;
- final harmonic arrival/next chord forecast;
- motif/source lineage and what its relationship/answer means;
- unresolved/paid/deferred musical obligations;
- stage, carrier and foundation responsibilities;
- instrument-local memory, breath, recoveries and scheduled responses;
- active/releasing notes, physical mono/poly voice capacity and authorized silences.

Their gluing is PARTIAL, meaning incompatible handoffs are refused: no duplicate role attacks, no snap/reset of groove at boundaries, no orphaned lead/germ source, no duplicate/contradictory harmonic writers, no discontinuity in held note/voice continuity, no answer to an unasked question, no keys or drum claim to a nonexistent cue.

**Do not** render isolated 16-beat WAVs, concatenate them (especially through independent synth/reverb tails), and call it a 56-bar song. Preserve one global Score/timebase and one final continuous synth render. Composition of episodes must be audible as a continuously developing band.

**Do not** generalize \`kernel::perform\` by simply removing the length check: the hardcoded keys-at-4.0625, bar-3 Withhold, four-chord bass/drum patterns, source statement slots and final tonic must become explicit parameterized, source-driven choices, or the engine will merely loop its demo.

## 4. Work sequence — smallest honest steps with listening gates

### R0 — Integrate the proven Codex contribution ONCE

Claude should inspect latest branch heads and worktree state. Use an explicit new **non-main** C137 integration branch from the current accepted lineage. PR #104 is **stacked on #102**, which already descends through #100/98/earlier integration. Verify merge-base and ancestry before deciding \`--ff-only\` versus reviewed merge. **Do not cherry-pick every upstream PR again and duplicate changes**. Do not merge PR #104 to \`main\` or release anything. The runtime source ends at \`ac97bcc\`; later PR #104 commits carry important docs/evidence and should not be accidentally omitted when integrating. Keep its original branch/PR traceable.

Repeat exact-head tests and one actual render; validate \`CommittedPerformance\`, source identity, cue→response difference, hard silence and historical R17/covers. Put the static review and evidence summary in the integration PR, not just chat text. Keep \`BASS_WEAVE=false\`.

### R1 — Move from a toy four-note call to an *actual generated phrase*

Before scaling duration, feed the kernel a complete **authored/generative timed source phrase** from ThemeFamily—not an arbitrary 4-event prefix. Preserve independently authored onset, gate, accent, motif relationships and harmonic destinations. The locally bounded 4-bar conversation still needs a genuine foreground statement/hook and a meaningful contrast.

Create at least one distinct but related source answer whose RHYTHM and melodic content depend on the original phrase and its harmonic context, rather than only a coarse sign classification. Keep \`Silence\` valid and at times stronger than the dense answer.

Predeclare falsifiers: equal note counts and equal overall energy with different source questions produce different, SOURCE-BOUND responses (or intentionally same response for a musically justified reason, explicitly witnessed); irrelevant metadata never changes PCM. If seeds collapse to the same *audible material*, report how and where, not force salt-based random filler. Preserve all event IDs and typed role capacity.

### R2 — First multi-episode global Score: 8 bars, then 16–32 bars

Build one continuous source-owned song with at least two **causally connected** different episodes: an actual musical idea, another player's complementary answer, and a second episode that REMEMBERS that exchange and changes something as a consequence. Instrument responses must read the final authorized source, not only the previous bar's note count.

Source-driven long-range sequence: theme established; related but distinct hook; harmonic departure; prepared return. Use the actual \`ArgumentArc\` from PR #100. Keep a shared pulse and source IDs across seams, with voice and silence state explicitly carried. Parameterize musical windows and player actions. The lead must not only sing bars 1 and 4 of every repeated cell; bass/keys/drums should not be byte-identical whenever the theme varies.

A 32-bar intermediate audition is more valuable than a massive untested 56-bar rewrite. Get green source/causal tests and a real WAV/stems for 8 bars, 16 bars, then 32. Move toward a **48–56-bar full song** once the short musical grammar demonstrably develops across scenes. If 56 bars fails, keep the strongest closed 16/32-bar checkpoint and label the remaining obstruction.

### R3 — Each musician's own long-range job: controlled density, memory and interlock

Lead: meaningful phrasing and source-linked verse/hook/bridge returns, intentional rests and evolving rhythmic shapes; not a compulsory phrase at the beginning of every section.

Keys: maintain harmonic support when necessary; source-related comp, answer or silence when the lead opens a legitimate opportunity. The 4-event quote/inversion currently caps variety; let response length/shape depend on role capacity and the particular source, without destroying source identity. More notes aren't always better.

Bass: preserve harmonic floor/root/3rd/7th/approach when needed; richer propulsion in admissible bars, temporary melodic approach only in a register and source duty that the monophonic bass can sustain. No resurrection of the late low \`BASS_WEAVE\`. Model when bass has to carry floor, when it may skip the beat, and who supports harmony if it withholds.

Drums: keep the car/pocket with role-specific reactivity. Actual cue-dependent fills, breaks, accents and silence; NOT the same 12/12/14/12 strokes in every episode. The drummer may anticipate DECLARED source harmonic destinations, or respond to already committed keys/bass; never retroactively pretend to have heard a later line.

Pad: sometimes quiet, sometimes legitimate long-tone texture during a chorus/return if it strengthens and doesn't mask other identities. The pad needn't come back just to satisfy a density metric.

All of this remains source-first and bounded: final role event commits before downstream hearing, no big same-gain fader, no global vertical rewrite as in the rejected R8 CoupledR8. Keep local pocket relations with legal phase offsets; do not inject arbitrary humanizing jitter or spread notes off their source lattice.

### R4 — A real test of musical information and style, not simply note totals

Produce fresh seeds, lengths and at least two compatible worlds (BLACK_ICE / SWISS_SIGNAL if their harmonic vocabularies admit source; explicit refusal otherwise). Every retained generated piece should be source-coherent, harmonic-context true, and DIFFERENT because source history differs, not because a random synth setting changed.

Adversarial contrasts:
- Same short opening but different later thematic material: music must diverge LATER, with earlier unrelated audio invariant.
- Same final chord but changed/removed bridge: final semantic return cannot claim the same earned obligations.
- Same note count/loudness but altered/foreign call: dependent keys and drums must change/refuse if source relation demands.
- No keys answer: planned later drum response must change; pulse anchors remain stable.
- No bass approach: compare genuine musical significance, not just the test's ChangedCommitment error.
- One segment ends with an unfinished note / different groove phase: next segment must carry/close correctly or reject, not retrigger or slide without license.
- Keys/drums silence at a critical payoff: missing duty should be visible separately from ordinary legitimate absence.
- Identical seed replay is deterministic across scheduling/block-size; unwanted duplicate seeds must be disclosed.
- Wrong source IDs/voice overlap/harmonic arrival/after-the-fact gain: hard refusal.
- Excess extra fills from a naive density augmenter: controls may pass event count but should lose specific melodic dialogue or be rejected by the user as overplaying.

Make existing R gates and semantic/source/commitment witnesses independent; do NOT tune toward nine green numbers as though they prove jazz sophistication.

### R5 — Listening and acoustic triangulation

Retain all pre-existing controls:
- Real user-accepted \`teacher_golden7\` files **only if actually available with verified hashes**.
- Exact **reconstructed narrative teacher** found by Codex (not a surrogate for all seven), normal/solo stems.
- PR #102 current held-bass full 128-beat fusion.
- PR #104 verified bounded 16-beat conversation.
- New 8/16/32/long song, role stems and controlled source/cue/mute variants.
- Same Score normal/nocomp/harmonic-reference production and level-matched blind audio packets.

Measure role onsets, real note gates, source identity, harmonic movement, relevant silence, per-role versus whole-mix envelopes, counterpoint interlock, PCM low-frequency occupancy and masking proxies. Distinguish Song/Score truth, physical render, and HUMAN verdict. A full mix can have stable RMS while musicians have meaningful independent activity; don't set an arbitrary loudness-variance target.

Asking a blind listener to identify an exact literary story is unreasonable. Ask: what motif is being answered, does the band make room for it, can one hear the bridge transform the final return, and is the density interesting rather than pretentious? Mark human acceptance PENDING until actual listening.

### R6 — Hardening, integration handoff and simplification

Full fmt, strict all-target/all-feature clippy, warnings-denied rustdoc, Cargo suite, MSRV/ABI/PTY/parser/C-Go CI, R17 48 WAV / 757 diagnostic replay, covers/fingerprint/PCM history. Verify exact final SHA, audio artifact manifest, source/Score fingerprints and generated variants. Preserve app consumers, LibGibson 0.4 defaults, future 0.5 film and 1.0 language-parity roadmap.

This round should **reduce** overlapping authorities on the NEW opt-in path, not increase a zoo of player objects and score rewrite passes. Keep previous historical adapters as frozen compatibility routes. No main merge, release, tag, or default enable without explicit owner approval.

## 5. Local home-directory worktree CLEANUP — IMPORTANT, deliberately conservative

**This audit can only see GitHub and Codex's recorded paths; it CANNOT inspect or modify the user's local home directory. Claude must do that on the real machine.** Codex documents these distinct working directories (check existence—do not assume):

- \`/home/leah/LibGibson.kernel\`: Codex active PR #104 worktree, local \`target/c137-audition-ac97bcc\` and recordings.
- \`/home/leah/LibGibson.kernel-bass\`: scout/checkpoints with \`target/c137-controls\` and \`target/reconstructed-teacher\`.
- \`/home/leah/LibGibson.kernel-hostile\`: independent hostile test worktree.
- Other local LibGibson clones/worktrees may exist; discover them, do not guess their paths.
- A verified copy of three artifact families exists on removable media (when actually mounted): \`/media/leah/SPANKBANK/LibGibson-c137/ac97bcc/\`, with \`audition\`, \`pr102-controls\`, \`reproduced-teacher\`. See \`research/c137_kernel/copy_custody.json\` and SHA256 manifest. **Do not assume the volume is mounted or that the data is safe solely because docs say so.**

**SAFE PRE-FLIGHT (READ-ONLY, run in each real clone or its primary root):**

\`\`\`bash
pwd
git rev-parse --show-toplevel
git status --porcelain=v1 --untracked-files=all
git branch -vv
git worktree list --porcelain
git worktree prune --dry-run
git remote -v
git log -1 --format='%H %s'
df -h "$HOME"
find "$HOME" -maxdepth 2 -iname '*libgibson*' -print
\`\`\`

For each listed worktree:
1. Record canonical path, exact HEAD, branch/detached status, upstream/PR, uncommitted **tracked and untracked** files, ignored target/WAV/artifacts, and whether live Claude/Codex agents are still using that directory. Respect active processes/sessions and mounted paths; never move/delete an active worktree.
2. Preserve every unpushed commit, unmerged branch, draft PR, WIP diff, stash, and generated audio. \`git status\` does NOT list \`target\` / ignored artifacts. Check explicitly; store SHA and custody. If the media mount is unavailable, do not remove their original local copies.
3. Make a dated recovery manifest and local \`git bundle\` covering all relevant refs before structural cleanup; backup untracked/ignored valuable data separately to VERIFIED storage. Test the bundle and copied SHA manifests.
4. Choose ONE principal LibGibson development workspace for Claude; use \`git worktree\` correctly for future independent short-lived experiments. If worktrees must be reorganized, use supported \`git worktree move\` ONLY when stopped/inactive, then recheck refs and builds; do not raw-move linked worktree directories.
5. Remove only PROVEN redundant, **clean**, **inactive** worktrees with \`git worktree remove <exact-listed-path>\` (no \`--force\`); prefer leaving any unresolved worktree alone. Branch refs are a separate issue: local \`git branch -d\` only after verifying commit ancestry/PR state and preserving recovery refs. Remote branches with open PRs are NOT garbage and must not be force-deleted.
6. Absolutely avoid \`rm -rf\`, \`git clean -fdx\`, \`git worktree remove --force\`, \`git branch -D\`, force-pushing or remote branch deletion as "cleanup" shortcuts. Defer ambiguous deletions and give the owner a short explicit review list.
7. Confirm the resulting primary workspace can still access the correct branch/head, build and render, and has surviving audio custody. Report what was archived/moved/removed, what remains and why.

**IMPORTANT:** If \`~/LibGibson.kernel\`, \`~/LibGibson.kernel-bass\`, or \`~/LibGibson.kernel-hostile\` has unique ignored artifacts or scout research not pushed, do not delete merely because PR #104 commits and GitHub CI are green. The local output is evidence and cannot be recreated identically without exactly pinned sources/settings.

The cleanup goal is **fewer active worktree directories and one clear integration lineage**, not fewer GitHub refs at any cost. Leave remote PR #95–104 lineage intact until reviewed/merged.

## 6. Expected Claude handoff to the owner

Provide:
- Exact source/integration branch ancestry and HEADs, no duplicate cherry-picks.
- What PR #104 already proved, what was repeated locally, what remains a hypothesis.
- The smallest validated step from 4 bars toward 8/16/32/48–56 bars.
- Real role-stem/mix audition packet and executable render commands.
- Fresh seed diversity / musical-source equivalence report, NOT merely seed checksum counts.
- Note/voice/cue/Score witnesses, production variants, source/relationship/duty/phase regressions.
- Full local worktree inventory and archived WIP/media manifest; a short list of paths safely removed and paths deliberately retained; no unsafe deletion.
- Exact CI status, historical replay evidence, change inventory and open human ear gate.

**Artistic imperative:** keep Codex's new responsiveness. Fill the time with *causal musical information*, not random density. A bass line should remember the upcoming harmony, keys should know when to play or listen, drums should react to the committed band, and the bridge should change what the returning hook means. If the simplest full arrangement sounds best, keep it.
