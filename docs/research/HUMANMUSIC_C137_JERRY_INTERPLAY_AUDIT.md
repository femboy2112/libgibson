# C137 — JERRY LAW, CAUSAL DRUM VOICES, AND THE NEXT FULL-SONG ROUND
## Exact-head source audit and Claude implementation handoff

**Pinned head:** \`feat/humanmusic-c137-expressive-transfer@125d759f10842b759ef6c1279de63b425a64e1fe\`, draft PR **#111**. After initial audit the [exact-head CI run 38368023355](https://github.com/femboy2112/libgibson/actions/runs/38368023355) completed **SUCCESS**. GitHub PR #111 and branch were refreshed: no newer code head existed when this report was written. Released \`main\` stays separate at \`80706c91b88e59495b131bfd8ab23aa3ed013414\`.

**Scope:** docs/research only; no Rust edits, no new WAV audition, no merge/tag/release/default modification by this branch. GitHub connector fetched ten complete files at their pinned Git blobs, and 26 targeted **static assertions** evaluated true. One precise lead-echo / pulse backbeat same-sample collision was independently calculated in JavaScript from code-defined slots and the published 144 BPM/48kHz fixture. These are source/logic findings, not substitute for runtime Rust event enumeration or listening.

**User's accepted listening:** PR #111 records positive acceptance of expressive lead gates/dynamics, expressive but full keyboard comp after reverting an overzealous dial-back, and destination-oriented bass anticipation. \`125d759f\` lead-rhythm-translating drum fill is pending the maintainer's ear. Earlier \`CoupledR8\` and low \`BASS_WEAVE\` are ear-rejected negative controls, not "quality features" to switch on. User's directive: default C137 Full with all **compatible, proven/accepted** quality features; a Jerry switch explicitly removes features. Maintain byte-identical historical v0.4/R17/cover and earlier A/B arms.

## 1. What actually landed and is a durable improvement

- **C137 Full CLI default (7b8d8bc):** \`examples/c137_song.rs\` calls \`C137Preset::full()\`. \`examples/c137_song/preset.rs\` resolves no flags to 16 bars with \`Phrasing::Ensemble\`; the 8-bar old plain control remains \`--jerry=short,plain\`. The full 16-bar source has verse/hook/bridge/return, expressive gates/phrasing, bridge-dependent keys answer, selective arrival pad.
- **Corrected breath restraint (cfff7aa):** drums do not obligatorily answer every silence; home rests (as classified by harmonic_drive) are intentionally left alone. A source-harmony mutation test supports that invariant. This does **not** yet make the drum response a complete source-ID-based Call→Response ledger entry.
- **Keys (a3ee8ee, eb234a2, d1298ec):** rootless chord voicings, guide-tone shells, lawful color and selective ornament, timed into holes on home arrivals. An attempted drastic color dial-back was reverted on the maintainer's listening decision. Do not undo the accepted sound based solely on a density heuristic.
- **Bass (71fdcd9):** walking foundation's existing approach note becomes functional destination-aware V→I / leading-tone / diatonic approach. Exactly one mono line, no extra low counterline, preserved downbeat-root anchor. Ear accepted.
- **Drums (125d759):** generic phrase-end fill is replaced, when the lead has a qualifying figure, by snare events at \`lead_onset + 2.0 beats\`. Its existing test checks the lead source attacks and preserves plain control; **auditory verdict pending**.
- **Engineering:** last CI success on exact head, PR #111 reports 33/33 focused song tests + 475/475 human_music library checks and historical compatibility. No separate test was run in this audit.

The next fixes must preserve and improve these musical successes, not restart density experiments from scratch.

## 2. P0: the “one Jerry switch removes one capability” law is false in actual code

There is a hidden dependency between LEAD articulation and THREE other band capabilities.

\`examples/c137_song/preset.rs::resolve\` maps \`Jerry::Plain\` → \`Phrasing::Plain\`, advertised as removing expressive lead phrasing and its resulting breath response. However BOTH \`source.rs\` and \`source_bridge.rs\` do:

    compiled.comp_keys = phrasing.lead_is_expressive();
    compiled.anticipate_bass = phrasing.lead_is_expressive();
    compiled.translate_lead_rhythm = phrasing.lead_is_expressive();
    compiled.answer_breaths = matches!(phrasing, Phrasing::Ensemble);

So \`--jerry=plain\` ALSO disables pianist comp voicings, bass destination anticipation, and lead-rhythm translation in the drummer. That is an important *confounded experiment* and an **undisclosed bundle of Jerry reductions**, contrary to the \`Jerry::Plain\` documented scope. Current D2 regression tests check some preserved aspects but do not isolate ALL these band laws. The bundled older plain render is a meaningful archived control; **do not silently change its fingerprint while correcting the API**.

**Suggested narrow repair:**
- One resolved, source-authoritative \`C137Features\` (internal first; names optional) containing independent song-form / lead phrasing / keys comp / bass anticipation / drum lead-rhythm translation / breath-response settings. A dependency rule permits answers only if there is a real expressive breath, but does not require pianist comp to turn off when the lead is played plain.
- Keep \`--jerry=short,plain\` and existing \`--phrasing=plain\` documented/byte-identical as **legacy bundled controls** when explicitly requested; honestly print all removed abilities in the manifest. Introduce true atomic Jerry switches for keys-comp, bass-anticipation and drum-translation as direct, orthogonal A/B reductions. Optionally use a separate explicit \`lead-plain-only\` control for an isolated lead phrasing ablation if it admits a lawful source.
- Preserve old \`Phrasing::Plain\` and old \`CompiledArgument::compile\` defaults on historical routes; the new C137 user-facing path must derive every flag from the authoritative resolved profile, not implicitly from one unrelated bool. Reject incompatible requests early with explicit reason; no silent downgrade.
- Orthogonality tests: Full vs minus-only-keys keeps lead/bass/kit/source unchanged; minus-only-bass keeps lead/keys/kit unchanged; minus-only-drum-fill preserves lead/bass/keys/backbeat; a true lead-plain-only fixture keeps unrelated band feature intent and reports breaths unavailable where absent. Preserve archived 8/16-bar Plain PCM hashes.

\`C137Preset\` currently lives under \`examples/c137_song/preset.rs\`; no reusable library-level C137-first \`perform_c137\` entry was found in \`human_music::kernel\`. If a public reusable API is wanted, introduce ONE non-breaking opt-in library entry or internal adapter. Do not globally flip \`Phrasing::default()\` or historical \`PerformanceProfile::default()\`.

## 3. P0: an exact, schedulable same-snare collision in the new "lead-echo" fill

At current C137 long-form cadence bars, \`theme_family.rs\` has:
- \`CADENCE_GRID = [0.0, 0.5, 1.0, 1.5, 2.0]\`,
- \`BRIDGE_CADENCE_GRID = [0.0, 1.0, 2.0]\`,
- \`pocket_offset(1.0) = pocket_offset(3.0) = +0.030 beats\`.

The actual lead source at relative beat 1.0 is laid to **bar_start+1.030**. At a phrase-ending bar, the new drum law \`let at = o + 2.0\` schedules a \`DrumVoice::Snare\` \`lead-echo\` at **bar_start+3.030**. The simultaneously scheduled steady backbeat snare is \`bar_start+3.0+pocket(3.0)\` = **bar_start+3.030**. At the published 144 BPM/48 kHz fixture, one beat is 20,000 samples and the two events land at the SAME sample index (e.g. for bar_start=12, both sample 300600).

\`HumanMusicSynth\` has ONE \`snare: Snare\`; its event dispatcher \`while ... drums.at <= abs\` triggers BOTH on that sample, and \`Snare::trigger\` resets its envelope and oscillator phases on each call. Thus the two purportedly distinct intentions are NOT mixed as two audible snare voices; a later trigger can overwrite the earlier. \`Score::validate\` only checks drum range/time/velocity, not same-voice event collisions. The current \`the_drums_translate_the_lead_rhythm\` verifies the echo is related to a lead attack and plain control is unchanged but has no snare-capacity assertion. This is a **specific source-scheduling defect**, not proof of an audible flaw in the current WAV (not auditioned here).

**R0 test first:** enumerate DrumHit(s) on actual seed 2112/7/99 Score; convert to integer sample index using the actual Tempo::beat_to_sample, group by \`(sample, DrumVoice)\`, print true duplicates, resulting last-trigger ordering/velocity. Do this for Full and archived Plain. Add a negative control with a source-lifted cadence on 1.0 proving the collision, and compare PCM. Do NOT simply silently delete one event and call it repaired.

**Smallest musical fix:** arbitrate at the AUTHORITATIVE drum-commit stage. Preserve kit's hard pocket spine; if a source-derived echo coincides with a hard snare anchor, choose a single justified accent or a licensed offset (flam only if idiomatic and explicitly planned), with causal provenance recording both intents, or deliberately displace/omit the optional echo. Deterministic, finite, before downstream hearing/mastering; use/extend existing \`groove::arbitrate\` or shared drum-event capacity rules if compatible. Test real Score+PCM and absence of unauthorized same-sample triggers. Preserve drum branch as an A/B control and wait for listener judgment before promoting a changed fill as better. Different DrumVoice families may coincide intentionally (kick+snare) and are NOT invalid by this rule.

**Additional risk:** \`lead-echo\` fill currently does not check whether it overlays the lead's own later climax or a written hard silence; the older ordinary comp stroke does check AuthoredOccupancy. Require source/licensing checks for fill placement, not compulsory extra snare attacks.

## 4. P0/P1: the new drummer is related to the source but its full causal ledger is incomplete

- The breath responder checks \`lead_silent_at\` and \`breath_licensed = !HarmonicDrive::Home\`; source harmony and written gate matter. This is superior to blindly filling all rests, but **no specific named musical call/debt or performed lead source ID** is attached to the new \`"answer"\` DrumHit. It calls a generic \`push\` which produces provenance role_note but no \`ActionKind::Answer\` or registered Interaction.
- The new \`"lead-echo"\` hit similarly has \`role_note\` but does not carry an explicit originating \`MaterialId\`, lead event ID or Action/Response edge. Its test reconstructs a time-shifted lead onset but cannot prove the right *motif* was answered if an equally timed different motif arrives.
- The existing \`"heard-keys"\` drummer fill *does* compute from actual final keys notes; yet it creates a generic \`ActionKind::Fill\` with \`call=None\`, \`phrase=0\` even for later episodes. It is a genuine behavioral dependency with incomplete source provenance.
- \`validate_song\` closes \`prov.interaction\` for \`Score.notes\`, NOT for \`Score.drums\`. There is no independently checked full typed lead/keys→drum interaction graph for every claimed response.
  
**Fix:** use existing \`MaterialSource\`, \`Call\`, \`Response\`, \`ActionId\`, \`InteractionId\` and \`Provenance\`, not a separate giant drummer-agency ontology. Create one type-safe, source-grounded per-role response intent before drum events are committed: lead-call→breath response; lead-fragment→translated fill; keys-answer→drum reply. Stamp exact originating source and accepted response action on the final DrumHit. Reject orphan/foreign IDs and answers to unlicensed hard silence. Allow legitimate unresponsive drums and every silence to stand. Maintain invariant that keys-independent pulse remains byte-identical when a keys-only cue changes; only the specifically dependent drum response may change. Convert future source motif (same onsets/count but different statement) and withhold controls into actual *causal* regression tests.

## 5. P1: possible polyphonic keys voice stealing and acoustic masking

New keys comp introduces rootless chord stacks (sometimes 3+ simultaneous voices), guide-tone shell, optional ninth, grace ornament, plus an earlier keys-answer line. The synth provides only SIX real \`SynthVoice\` slots for Keys, and \`trigger_note\` steals the oldest voice if all are active. \`kernel::validate_role\` enforces mono capacity on Lead/Bass, not poly dynamic lifetime on Keys. Current keys comp code checks that its STROKES don't coincide with lead attacks; it doesn't check keys pool pressure including release tails, pad spectral occupancy, or a whole-sounding-note collision with a sustained lead note.

Do NOT unilaterally thin/revoice the keys: the fuller comp is **EAR-ACCEPTED** and the dial-back was ear-rejected. First instrument the actual SynthVoice pool (occupancy/steals by instrument and sample), \`HeardWindows\` direct audible lifetime, pre-master bus meters and relevant stereo PCM. Compare Full, only-keys-comp-disabled, and selectively revoiced alternatives at identical song/source/patches. If stealing or masking occurs, make only locally justified changes, retaining a frozen accepted mix. A lack of stealing is a useful negative finding.

## 6. P1: downstream quality features lack complete manifest and source-level closure

- The Full example manifest currently prints arc, lead, pad and generic pocket, but does NOT separately report the new \`comp_keys\`, \`anticipate_bass\`, \`translate_lead_rhythm\` CP3 flags or their realized effect. A user can think Full was running all positive features without an independent active/idle witness. Add explicit effective feature manifest rows and reason, never claim every feature always fires.
- \`Phrasing::default\` remains \`Plain\` and \`CompiledArgument::compile\` initializes \`answer_breaths\`, \`comp_keys\`, \`anticipate_bass\`, \`translate_lead_rhythm\` as false. This is CORRECT for historical APIs, but the full C137 configuration exists only in the example CLI and source fixtures. A caller who invokes library \`kernel::perform_song\` with a freshly compiled source does not automatically receive the Full feature set. Prefer one dedicated source-construction / performance entry point clearly labeled C137 Full, without changing archived defaults or source-witness terms.
- The example \`option_env!("GIT_SHA")\` often falls back to "not embedded". Per-seed \`Score::fingerprint()\` is NOT a cryptographic WAV/source custody record, and example output writes only the final full mix and TSV. Add committed/versioned exact-sha run manifests (Git SHA if known and honestly unavailable otherwise), SHA-256 WAV/Score hashes, source/argument/profile/World data, complete CLI and env, actual role stems, per-bus pre-master meters and same-Score NORMAL/nocomp/reference arms. Solo stems traverse shared production; don't claim they linearly sum to mastered mix.

## 7. P1/P2: do not lose full-song semantics or player independence as we zoom out

- \`BandMemory\` remains \`open_call: bool\` plus one \`carried_cell\`, not multiple independent ID-addressed obligations. Its \`prev_answer_events\` field is only written. A longer episode sequence can overwrite earlier unpaid material. Build a three-episode actual source test with two independent unresolved calls and one answer; named settlement and outstanding debt must survive.
- \`groove_swell\` still resets from its bar-index modulo every four bars, and both bass and drums use shared \`drive_energy * swell\` values. The user has previously objected to statistically averaged band energies. Source-owned transition preparation + locally different role envelopes should replace automatic resets **where a new musical source explicitly demands it**, without removing the shared pocket anchor.
- Lead \`VoiceContinuation::new(a,b)\` is still forced at every episode boundary. New section articulation must distinguish held/legato, intentional fresh attack, breath, pickup and capacity refusal; preserve the existing accepted lead where the source truly calls for legato. Don't break the positive expressive gate/breath achievement.
- Long-form C137 currently supports a 16-bar source-owned verse/hook/bridge/return. The next substantial goal is a coherent 32-bar original and then 48–56-bar piece, with independent player memories/meaningful silences and earned musical consequences. Do not obtain length by WAV concatenation, repeating 16-bar scenes or increasing event-count thresholds. Use one global Score and synth.

## 8. Recommended short, reversible next round

**R0: pin/ear freeze** \`125d759f\`, exact source/Cargo/CI, saved current Full WAV and three Jerry controls at hashes; fetch latest accepted golden7 only if truly available. Get maintainer verdict on lead-echo drums A/B (PENDING). Re-run quick reproduction of actual scheduled snare sample collisions and current D1–D7, flag coupling.

**R1: correct feature axes** without altering accepted Full PCM or legacy Jerry PCM. Disentangle the three band optional enhancements from Phrasing; add atomic Jerry A/B reductions and label legacy Plain as a multi-feature downgrade. Export exact feature presence and per-seed effect, no silent degradation; tests for same source/harmony and only one changed role in each ablation.

**R2: snare event capacity + causal ledger** with red-first duplicates/trigger overwrite, source→drum call/response, truthful hit provenance, occupancy/silence checks and matched PCM. Preserve actual selected downbeat/backbeat and source law. Do not assume any audio change better until ear comparison.

**R3: harden accepted keys/bass sound** with real synth keys-pool occupancy and six-voice stealing controls, mono bass regression against held \`BASS_WEAVE\`, harm/attention masking and optional minimal revoice only when needed. Focus on source/physical defects rather than arbitrary note thinning.

**R4: real 32-bar composition** only once R0–R3 pass: one shared global harmonic route and tempo, complete motif/bridge/return with multiple material-identified debts, anticipated transition and differentiated role dynamics. Source-owned pianist/drummer/bass dialogue and deliberate silence. Generate blinded A/B full mixed/stems against 16-bar accepted and verified teacher source. A safe 32-bar coherent composition beats an untested 56-bar dump.

**R5: long-form 48–56 bars** if musical and categorical tests succeed and user's ear likes 32: no cheap repeats, abrupt phase/energy resets, orphaned voice tails, fake source IDs or uniform high-density fill. Source-preserving transformations, deliberate handoffs and purposefully sparse sections.

**R6: CI/custody/hand-off** format, strict clippy all targets/features, rustdoc \`-D warnings\`, actual full Cargo suite and examples, MSRV/API/PTY/Go, exact-head GitHub Actions; R17 archival 48 WAV/757 receipts, covers and fingerprints. Versioned render/candidate stems/PCM manifests. One non-main implementation branch/draft PR, no merge/tag/release/new historic default/ABI change without user permission. Preserve the original accepted C137 16-bar control, negative \`CoupledR8\`, held bass weave and provenance. Snapshot/verify local media before any worktree cleanup.

**Falsifying listening comparisons:** A) accepted Full @125d, B) lead-rhythm fill de-collided, C) source-bound drummer with one optional cue withheld, D) independent keys/bass/drum Jerry reductions, E) 32-bar connected form, F) explicit overplaying/wrong-source controls. Match world/seed/tempo/patch/mastering/source and label noncomparable cases honestly. Ear accepts/rejects; machine gates catch errors but cannot certify C137 quality.

## 9. Claim discipline

**Source-verified:** Full is CLI default, Jerry Plain hides three extra downgrades, CP3 flags remain tied to lead phrasing, source/stroke scheduling creates an exact same-snare same-sample collision on cadence grid, DSP uses a single retriggered Snare voice, action/provenance for newer drum responses lacks a complete typed origin, Score validator does not check drum collision, keys six-voice pool, demo lacks stem/hash custody, 16-bar domain and several fixed transition/memory policies.

**Not independently executed:** actual current seed's Rust Score duplicate count, sampled PCM difference, keys voice-steal occurrences, full suite beyond external GitHub CI, teacher_golden7 listening and 32-bar sound. Require tests and auditory evidence before asserting observed acoustic faults.

**Bottom line:** make the current **ACCEPTED** high-quality band truly reproducible and its individual musical decisions actually causal; stop unintended feature bundling and silent drum-voice collisions. Then lengthen the song only when the same expressive depth survives from the first bar to the last.
