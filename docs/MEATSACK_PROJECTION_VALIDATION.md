# MeatsackProjection — Falsification, acceptance, and experimental ladder

> **DESIGN-ONLY research fixture. No implementation or listening result is claimed here.**
>
> Companion to [the architecture](HUMAN_MUSIC_MEATSACK_PROJECTION.md) and [handoff](MEATSACK_PROJECTION_HANDOFF.md) on **research/humanmusic-meatsack-projection**. Do not run this program on Claude's active C137 branch until that work is intentionally frozen and integration is approved.

## 0. Scientific claims and what would refute them

| Candidate claim | Machine discriminator | Ear-first discriminator | Failure interpretation |
| --- | --- | --- | --- |
| Correlated bodily gestures are more convincing than independent jitter | Equal marginal onset spread, different causal correlations | Blind paired A/B with comparable musical score, timbre, gain | Physical constraints may be wrong/inaudible; do not force a win |
| A strum is one physical action, not a simultaneity accident | Ordered string contacts with ONE source gesture ID, down/up reversal | Same chord: down / up / fingerpicked | Grouper or contact-time implementation wrong if unrelated offsets |
| Ordinary drumming is feasible with finite limbs | Resource assignment + movement/recovery proofs | Grooves/fills remain believable | Rig overly restrictive if basic rock groove fails |
| Some sequencer demands are impossible for a human rig | Typed refusal for >available simultaneous distinct stick strikes | N/A (machine witness) | If silently "fixed", physical executor is dishonest |
| Phrase breathing does not destroy pocket | Bounded phase warp, stable anchors, allowed order | Same groove at different phrase energies | Extra phase field double-counted with FeelTransport or drifting |
| Physical controls matter beyond onset offsets | Hi-hat openness, pedal, breath, damping under fixed note-on events | Ablate continuous controls only | Control path is no-op if physically audible controls unchanged |
| Identity survives changing physical performer | Same source SongMap/gesture links across rigs | Recognize same motif through synth/guitar/piano/etc. | A body cannot silently compose a different motif |
| Physical change must not self-heal into another song | Frozen source/approved revisions and bounded one-way realization | No exaggerated dynamics feedback | Automatic optimizer/source mutation violated authority |

These are **hypotheses**, not established human-expressivity theorems.

## 1. Freeze and controls before fitting anything

Choose a small set of **synthetic, original** reference gestures, not copyrighted MIDI songs. Freeze and hash source Score, tempo mapping, assigned rig, options and test code before listening. No adaptive thresholds after seeing results. The initial control modes must expose source-equal intent:

- **ELECTRONIC:** exact sequencer/no body; preserving old synth events.
- **WHITE-JITTER NEGATIVE:** independent per-event perturbations matched *in marginal spread* to a physical trial. Use only as a negative calibration device, never the proposed implementation.
- **PHYSICAL-TIME:** finite effectors/contact ordering/recovery/phrase phase; acoustic patch held fixed.
- **PHYSICAL-FULL:** same feasible assignments plus supported physical attack amplitude/articulation/control trajectories.
- **NO-VARIATION:** the same rig at declared zero variation, an explicit identity source→excitation control.

The first two body modes must use **identical** SongMap/Score identities, timbre patch, loudness target and dry/wet production settings to their comparator, unless testing a single physical-control effect. Capture their exact deviations and any sound rendering differences. Don't compare a warmer patch to a harsher patch and call the body model successful.

**Holdouts:** reserve original novel gesture patterns and at least one alternate rig/configuration before tuning. The independent listening gate uses anonymous randomized labels when practical. Synth-only controls do not prove physical acoustics.

## 2. Global invariants (red tests first)

**Authority / identity**

- Source `SongMap`, form, chord chart, semantic carrier, anchor ids, melodic pitches/contours, required gestures and structural beat coordinates are immutable in physical projection.
- Every emitted physical excitation maps to an explicit source `GestureId` or authorized child of one; no fabricated note-level cause. Every required gesture is accounted for by realized events or **typed refusal**.
- A single intended strum may have multiple physical contacts but remains one musical gesture.
- Re-keying events after timing shifts must **not** break existing `VoiceContinuation`, `ActionId`, `MaterialId` or cover projection; stable identities may not be based on newly computed onset floats.
- Changing performer/technique may change realization while upstream SongMap fingerprint remains unchanged. The new physical execution has its **own** fingerprint/receipt, not a retroactive edit to song identity.
- Existing verified `PerformanceReceipt` / `SongMapConformance` / cover gates still apply to the intended performance. Additional checks apply to actual realization; do not silently reuse pre-physical sound witnesses when physical scheduling has changed them.

**Timing**

- Each real excitation has finite sample-indexed onset at a declared sample rate; physical delays are modeled in seconds then converted to integer samples at one boundary.
- When required, phrase warp is order preserving and within declared displacement/velocity bounds; selective anchors have bounded drift. Physical sequential contacts within one chord are handled as gesture children, not as an illicit global time reversal.
- No negative absolute onset, integer overflow, unbounded buffer growth, invalid duration or out-of-piece placement without an explicit policy.
- Exact metric sources remain recoverable; do not infer score time by subtracting the measured physical offset after the fact.
- `GrooveTransport` / `FeelTransport` are applied **once**; physical gesture offsets are a separate, independently ablative contribution.
- No mandatory nonzero deviation. A requested precise stroke may be precisely aligned.

**Resources / dynamics**

- A resource cannot hold two incompatible simultaneous states. Distinct strokes share a limb only when a declared technique permits rebound/sequence. A physical string cannot produce incompatible independent sustained notes without licensed technique.
- Effector movement, recovery, pedal and breath bounds are explicit for the selected rig. Unknown/unsupported feasibility is neither success nor an invitation to invent a hand.
- Energy variation is bounded and conditioned by the intended expressive gesture plus declared physical state. It does not trigger a hidden global band-level loudness-conservation controller.
- Source-only/physical-only variation can be independently disabled for controls. Same seed, source, rig and block/sample schedule give deterministic repeat results.

**Compatibility and performance**

- OFF/default uses the untouched historical path; demand byte-exact v0.4 PCM/WAV and fingerprint/receipt equality, including R17 and covers.
- Bounded deterministic search, bounded memory and reasonable CPU; no stochastic exponential fingering solver, no per-sample graph search.
- Offline render block size, render segmentation, and seeded scheduling do not change the realized absolute event timeline. Caution: current synth rewind semantics can preserve voice state; use a fair fresh-instance control for exact PCM tests.
- No new C ABI or registry/release promise.

## 3. Drumkit hostile matrix (prototype 1)

All cases use an **explicit rig**: available limbs (default two hands/two feet), stick/pedal assignments, physical targets and declared techniques. Test nonstandard configuration variations too.

| ID | Intended gesture | Required result | Negative mutation |
| --- | --- | --- | --- |
| D01 | Kick + snare + stick hi-hat on one beat | Ordinary coordinated three-contact groove accepted | Rejecting every overlap is too restrictive |
| D02 | Snare + ride (two hands) + kick + left-foot hat pedal | Accepted if target/recovery configuration permits | Treat pedal openness as an extra stick |
| D03 | Four distinct stick targets at exact instant, two stick hands, no exotic technique | **Infeasible** with exact missing-resource witness | Grant phantom extra hands |
| D04 | Rapid alternating 16ths under declared technique | Feasible using alternating hands if transitions allow | Force one hand to exceed recovery |
| D05 | Rapid same-hand two strokes | Accepted only under declared rebound/double-stroke feasibility | Accept any arbitrary repeated attack |
| D06 | Flam on one snare | One source gesture, two linked attacks and declared leading hand | Two independent jittered events / wrong leading stroke |
| D07 | Ride-to-floor-tom transition | Must respect transition graph and recovery | Teleport hand |
| D08 | Stick hi-hat struck while pedal changes open→closed | Separate stick excitation + pedal control state, resulting articulation distinguishable when synth supports it | Ignore continuous pedal control |
| D09 | Right-foot kick succession faster than declared recovery | Typed infeasible or legal alternate pedal configuration | Hide extra foot/pedal |
| D10 | Same score under human kit vs electronic sequencer | Some sequencer-admissible patterns may be physically refused; upstream identity fixed | Change song to "make human win" |
| D11 | Open-handed / left-handed / extra pedal rig | Allocation changes without mutating musical source | Hardcode right hand and kick foot |
| D12 | Required hit conflicts with optional ornament | Omit optional event only under explicit licensed priority; otherwise refuse | Silently drop backbeat |

Drummer measurements: limb occupancy trace, target motion, recovery slack, pedal timeline, event-group identity, acoustic onset correlation and independent tempo/phrase offset contributions.

## 4. Guitar hostile matrix (prototype 2)

Declare tuning, string/fret range, fretting effectors, thumb/barre availability, pick/finger technique, hand-position span, string excitation continuity.

| ID | Intent | Required result |
| --- | --- | --- |
| G01 | One fixed chord, pick downstroke | Ordered string contacts in physical crossing direction, bounded correlated delays, one gesture source |
| G02 | Same chord, upstroke | Reverse string traversal relative to same strings; same song-level chord identity |
| G03 | Same chord, fingerpicked subset | Partial simultaneous/ordered contacts from declared plucking fingers; no forced sequential six-string sweep |
| G04 | Infeasible fret grip under configured span/fingers | Refuse or choose licensed alternative fingering; no magical fifth finger |
| G05 | Feasible barre | One declared fretting finger may cover permitted strings, counting as one constrained resource |
| G06 | Thumb-over-neck opt-in | With thumb enabled a specified alternative grip may become feasible; disabled remains unchanged |
| G07 | Rapid distant shape transition | Movement/recovery lower bound; no instantaneous teleport |
| G08 | Same string asked to independently sustain two pitches | Refuse or require declared slide/legato technique; preserve explicit source continuity |
| G09 | Muted/omitted string inside chord | Permit only when authored intention allows omission/mute; report actual contacts |
| G10 | Physical variation ablation | Time-only vs attack-force/control version, identical source notes and patches |
| G11 | Guitar-family voice through synthesizer | Attack geometry may be tested without claiming physical string acoustics |
| G12 | Same figure across two different rig/technique configurations | Recognizable intended motif and original event IDs survive |

Guitar measurement: ordered string paths, per-string delay and force, fingering feasibility witnesses, hand transition state, source-to-excitation grouping, optional damping/bend trajectories.

## 5. Keyboard and wind extension tests (do not build until first two pass)

**Keyboard:** a two-hand span/reach case; blocked versus rolled chord; one-hand impossible interval under declared span; hand repositioning over phrase time; same note with pedal down/up produces different lifetime when a pedal-aware synth is supported; MIDI synth patch alone does not prove a piano body.

**Wind/horn:** a phrase feasible under declared air budget; an infeasible unbroken long phrase without a breath; tongue vs slurred transitions; repeated attack under declared fingering/embouchure transition; a continuous swell without a fresh onset. Unknown lung capacities and extended techniques remain parameterized/unsupported, never guessed.

**Cross-cutting:** no one performer profile should be presented as physiologically universal; calibration must say which technique/person/device assumptions are modeled.

## 6. Phase-warp and local residual adversaries

- Check `φ(a)=φ(b)=0` for selected anchor boundaries and `1+φ'(t)>0` for any continuous warp model; use exact/tolerant numerical bounds appropriate to the implementation.
- False positive trap: human timing CAN be on a metrical grid. All-zero offsets must be accepted.
- False negative trap: perfectly coordinated common +8-milibeat lean is NOT independent jitter; preserve its attribution to band/player feel if that is where introduced.
- Negative control: matched marginal jitter with no shared trajectory; its pairwise/gesture-level correlation must differ from physical realization.
- Correlated performer signature persists across phrases. Resetting all state and re-seeding independently on every note must fail a state-continuity witness.
- At a planned cadence allow local rubato with bounded return, then immediately check that the next structural anchor has not accumulated phase drift.
- At 60, 88, 104, 138 and 180 BPM, and supported sample rates, compare **physical seconds** vs beat conversions and declared reach/recovery feasibility. These are test tempos, not assertions that all techniques are feasible at all tempos.
- Same source and rig under repeated and differently blocked offline rendering must yield the same attacks/controls; there must be no dependence on terminal/render-frame timing.

## 7. Independent execution receipt (not a planner echo)

**Required fields/witnesses** (concrete format design open):

- Stable source gesture ID + source note/hit IDs, physical performer/rig ID.
- Physical technique, assigned effector(s), string/key/kit targets and transition reasons.
- Intended beat/tempo coordinate; common/player feel already applied; each physical contact time in seconds and integer samples.
- Attack intensity/articulation and continuous control trajectories actually emitted.
- Grouping / order; physical resource occupancy; optional-omission licenses.
- Status with **realized**, **realized-with-declared-deviation**, **infeasible**, **unsupported** or **unknown** and concrete reason.

Observe the **resulting excitations/controls** separately from the planner's claimed assignments where possible; perform negative mutations to the realized record: forged third stick hand, wrong contact order, unlicensed omitted backbeat, orphaned source ID, skipped hi-hat control, exceeded stroke recovery, drifting phrase anchor. A truthful observer must reject these.

No scalar "humanity", "meatsack goodness" or universal perceptual metric.

## 8. Listening protocol

For each chosen controlled phrase prepare anonymous variants: electronic exact, independent jitter (matched marginal spread), physical timing-only and physically controlled realization.

Compare under fixed composition, world patch, loudness target, gain structure, dry/wet mix, sample rate and source melodic structure. Counterbalance order, record which criterion listeners hear, and report ties/ambiguity rather than force wins.

Questions:

1. Does the strum audibly read as one physically caused movement versus random arpeggiation?
2. Does a flam feel like a meaningful gesture rather than an isolated timing mistake?
3. Does a drum fill still sit in the band's pocket when hands switch?
4. Do phrase-scale timing variations breathe without drifting?
5. Are physical expression and musical phrase expression distinguishable?
6. Does a player seem to have a persistent physical signature?
7. Does added local complexity ever damage hook, structural arrival, groove or song meaning?
8. Does muting all physical expressivity restore the exact known control?
9. Does the physical-full version sound better than time-only? If not, do not keep unearned acoustic parameters.
10. Is any new variation actually required for a human sound, or was accurate instrumental articulation enough?

The listener may prefer the electronic version. That is a legitimate outcome; fidelity is a style/performer choice, not a universal mandatory "upgrade."

## 9. Staged build gates after C137 completion

**M0 — Freeze:** obtain maintainer go-ahead, verify completed C137 exact head/CI, fork work from that head. Preserve all previous goldens.

**M1 — Typed seam:** opt-in physical assignment; explicit source grouping; identity passthrough; independent identity vs physical receipt. ZERO audible change in OFF / passthrough arm. Block on any fingerprint, song-meaning or continuity failure.

**M2 — Drummer:** finite resource assignment, gestures/sticking, recovery/target graph, hi-hat control as separate trajectory. Red-first D01–D12. Controlled WAVs; listener must report an intelligible advantage or a useful failure insight.

**M3 — Fretted strings:** tunable string/fingering model, pick traversal / fingerstyle, strum/fingering feasibility and source gesture projection. Red-first G01–G12. Controlled WAVs; no claims of realistic string synthesis from patch oscillator alone.

**M4 — Shared law audit:** inspect what truly repeats across M2/M3; extract common resource/partial-map contract **only after both**. Reject speculative universal physiology and avoid a giant generic gesture DAG.

**M5 — Keyboard/wind:** only if a common resource/control law is validated by M2/M3; each instrument gets its own minimal sufficient state and independent tests.

**M6 — Story + production integration:** same SongMap/EnsembleNarrative and source identities across electronic/physical rigs, no surprise side effects, checked covers, bounded streaming/callback performance, full stem/listening matrix and declared limitations.

Every stage ends in **commit + exact-head CI + test receipts + listening outcome**. A failure in M2/M3 can end the experiment productively; it need not induce a larger framework.

## 10. Broad red search / operational checks

Run exact canonical controls: R17, v0.4 output hashes, all cover checks, frozen fingerprints, BAND receipts, future C137 story receipts. Test all worlds and multiple tempos, optional policies ON/OFF, voice continuation, acoustic tail overlaps, tiny/incomplete songs, malformed instrument definitions and impossible target graphs.

Search the cross-product selectively: use bounded branch/workload slices, explicit complexity limits and fresh seeds. Do not exhaust the machine for an ill-posed "all fingerings" proof.

CI: `cargo fmt --check`, strict `clippy --all-targets --all-features -- -D warnings`, `cargo test`, targeted release tests, `cargo build --examples`, strict docs, `git diff --check`; verify the **exact commit SHA** tested.

**Final go/no-go:** a passing constraint solver is not musical success. A nice WAV with impossible hands is not physical success. A humanized waveform that changes song identity is not compositional success. We require independent evidence for **all three**.
