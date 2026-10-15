# HumanMusic: The Pop Personality Gap (source-first hostile audit)
**Date:** 2026-10-14  
**Audited HEAD:** impl/humanmusic-smooth-pop-2026-10-14 @ **f3725fdb750a62de69124f69800fc2eeec1ac66b** (draft PR #116, exact-head CI 38656815592 SUCCESS).  
**Status:** Docs-only audit and discriminating implementation plan. **NO new Rust, WAV render, MP3 playback, or claim of independent ear audition in this audit.** Maintainer's listening verdicts are quoted only from the branch's evidence ledger.  
**Compatibility:** The current accepted 16-bar C137 Full / Johnny Five progress remains frozen under C137_ACCEPTED_V1; never rebaseline accepted goldens to bless a change.

## Executive verdict

The new pop engine now has a lawful opt-in harmonic-loop domain, a syncopated bass scaffold, pocket-adjusted keys and a warmer palette. These are meaningful advances. The *current pop artifact* is not yet a Sabrina-Carpenter/Halsey/Katy-Perry-like song; it is an intentionally lead-free, approximately 18.46-second **8-bar vamp at 104 BPM** (plus rendering tail), with **two identical 16-beat Keys-carried Establish episodes** and no semantic contrast. The program mostly proves that a loop is admissible. It has not yet generated a distinct, memorable **topline**, a verse→pre→chorus arc, contrastive melodic rhythm, a jointly listening bassist/drummer, or a genuinely multi-role harmonic production.

The next win is **carrier reallocation + prosodic hook/section contrast + bass-aware drum interaction + selective actual chordal voicing**. Swing, ninths and louder hats alone do not supply pop identity. A note-grid may be regular and still sound great; what is missing is the song-level *reason* each gesture belongs there.

## Findings with exact code evidence and separate epistemic status

| ID | Finding | Source / evidence | Consequence / verdict |
| --- | --- | --- | --- |
| F1 | **No topline / no differentiated chorus**. Lead=0; two steps are both Establish, second depends on first but carries a separate referent whose events are the same code-generated keys_comp(). Neither source step transforms, answers, or recognizes a distinct hook. | examples/c137_song/source_pop.rs:224-335; ledger:285-308, PR #116 R2.4 | **Observed (source and ledger)**: a successful groove demo, not a song narrative. |
| F2 | **Uniform phrase rhythm**: each 16-beat keys_comp phrase has exactly 16 attacks at relative offsets .5,1.5,2.5,3.5 in every bar; all 16 have onset phase .5 beat. Gates are .9/.7/.4 beats; tempo 104. Exact phrase reoccurs in episode 2. | source_pop.rs:90-148; static parsed-data probe, 16/16 notes same fractional phase | **Observed**: line continuity improved, but new verse/chorus melodic-rhythm differentiation cannot appear spontaneously. Do not confuse continuous-ish gates with a vocalist's prose/syllabic emphasis. |
| F3 | **Collective push turned into obligatory repetition**: all four bass cells end with offset 3.5 Octave, and the keys have an attack at .5 every beat, thus at 3.5 every bar. | source_pop.rs:181-221 and :94-139 | **Observed**: predictable repeated accent, not a content-sensitive band response. Tests should discriminate matched vs delayed/withheld pushes, not force exact unison in every bar. |
| F4 | **The new GrooveLaw controls BASS ONLY**, while the drummer's comp reads lead occupancy and harmony, never the actual committed bass groove or key carrier (except an existing narrow keys-tail fill). Lead is empty, leaving all lead-space tests permissive. | argument.rs:382-429; kernel.rs:1754-1818 and :1876-2070; source_pop.rs:251-260 | **Observed**: a shared beat/pocket is not a source-aware bass/drum conversation. Drums remain C137's kit, fixed spine/hat carpet, automatic fill at episode ends. |
| F5 | **Harmonic chart not equivalent to audible polyphonic harmony**. The rich Min9/Maj9/Dom9 route is real metadata; synthesis schedules only Score notes. Pop pad is Silent, its keys are a *single* moving material note at a time (32 keys over 8 bars), and there is no independent chordal keyboard support. | source_pop.rs:58-80, :90-148, :254-270; synth.rs:631-649; kernel.rs:2182-2230; ledger R2.4 pad=0 | **Observed**: extensions may be heard via sequential implication, but lush voiced harmonic texture is not yet constructed. Independent sounded-harmony witness is deferred in ledger. |
| F6 | **Timbre is not phrase articulation.** Pop render palette uses VAPOR95 FM EP (attack .003s, sustain .22, 0.45s release), with no installed expressive portamento or per-note within-note amplitude gesture; Keys lane commits with no VoiceContinuations. | world.rs:342-358; kernel.rs:1691; synth.rs:118-185, 631-650, 797+; ledger:310-315 | **Observed** technical boundary; actual heard "plinky" quality is ear hypothesis. A .9-beat source gate can still sound like successive struck EP notes. |
| F7 | **"Closure" is currently exact array repetition**, not a proof of recognizable thematic or emotional payoff. pop_route_law verifies route[i] == route[i % loop_bars] and divisibility; with loop_bars = route.len(), equality is tautological. | kernel.rs:1062-1086; argument.rs:384-400 | **Observed** contract gap, not proof the current legal 4-bar loop is wrong. Later section-specific harmony must have separate lawful transitions/cycle witnesses, not inflate loop_bars to total length to bypass recurrence. |
| F8 | **Pop structural tests are narrow**: five tests check nonzero keys/drums, zero lead, bass root and events, two domain outcomes, and chart G# for E7; they do not test audible poly harmony, actual drummer response, independent narrative source, phrase contrast, or a two-factor listening interaction. | tests/audio_smooth_pop.rs:1-165 | **Observed.** Tests are truthful first-round gates, not a quality pass. |
| F9 | **The accepted C137 PCM lock is real but not wired as a required CI acoustic step.** GitHub CI invokes cargo test; verify-c137-accepted-v1.sh is separate, manually run as documented. | docs/fixtures/humanmusic-c137-accepted-v1; scripts/verify-c137-accepted-v1.sh; .github/workflows/ci.yml (no verify-c137 call) | **Observed.** Local per-rung hash reports are valuable, but an automated PR can pass CI while changing PCM until this becomes a blocking deterministic job. |
| F10 | **Seed generality is absent on the pop arm.** Flavor-specific keys_comp and bass_cells are fixed; both 16-beat keys referents duplicate. | source_pop.rs:58-148, 181-222, 261-270; ledger:132-139 | **Observed** and acknowledged by Claude; one authored demo does not certify a generator. |

**No counterfactual Rust test or acoustic listening was run by this audit.** All missing expressive mechanisms are *candidate diagnoses*, to be tested by the independently defined probes below. F1–F10 are source-observable boundaries, not permission to assert what an inaccessible MP3 sounds like.

## Musical target: do not collapse three artists into one fictitious style

**Sabrina / Feels (playful, funk-pop):** tight bass/guitar/drum bed, nimble foreground phrasing, dry rhythmic accents and reciprocal energy. *Espresso* producer Julian Bunetta describes building from tempo → guitar/chord loop → bassline, with melody writers and kick/snare choices responding to one another: https://www.musicweek.com/interviews/read/hitmakers-the-songwriting-secrets-behind-sabrina-carpenter-s-espresso/090901 . *Feels* relies on bass, guitar/upbeat comping, electric piano inversions and a notably restrained drum base (analysis): https://www.musicradar.com/artists/it-was-time-to-put-all-that-behind-us-then-he-played-me-this-one-part-for-this-new-song-and-i-was-like-oh-yeah-how-calvin-harris-and-katy-perry-buried-the-hatchet-and-crafted-their-groovetastic-2017-hit-feels .

**Halsey-like alternative-pop (exposed, emotive):** allow vulnerable, rhythmically irregular middle-register line against LOW pad / HIGH bell or bright chordal accents, then create contrast through register/mix/section payoff. Rogét Chahayed reports the *Bad at Love* source built from low Prophet 8 pad, high bell-like chords and drums, expressly leaving room for Halsey's middle-register voice: https://blog.native-instruments.com/roget-chahayed-interview/ . Mood is not reducible to minor ninth chords.

**Katy-Perry-like pop payoff:** section-local melodic-rhythm contrast, repeated hook identity, a deliberate pre-chorus expectation and audible chorus arrival. Different from a maximum-density chorus. The repeated rewrites of *Teenage Dream*'s chorus underscore that the hook's recognizable emotional result was the goal: https://americansongwriter.com/songwriter-u-dr-luke/ . Artist names denote broad production targets only; generated music must be fully original, no lyric/recording/hook copying.

Shared abstraction: foreground **intent** must be heard above music that has its own **complementary groove**. A C137-protected, source-bound *carrier* should be able to hand thematic identity between lead, bass, keys and pad without conflating lead-note count, chord labels, or common fader shape with "feeling."

## Critical design repair: make carrier roles separate from arrangement roles

Today the required source melody is **owned by Keys**, filling the entire Keys carrier slot. Extra keys comp would either violate the source's complete required carrier window or become falsely credited as source. Meanwhile **Pad is silent** and **Lead is entirely empty**.

First candidate is a source-level *ownership* change, not a post-hoc sound patch:
- Let **Lead** carry the original vocal-proxy melody (source-preserved, intentional breaths and prosodic contours), while **Keys** supplies separately sourced clipped guide-tone chords/answer accents and **Pad** holds selected background harmonies at low intensity in chosen bars. Bass retains the rhythmic signature; drums consume actual committed sources.
- The old Keys-carried groove remains a clean **control** and accepted *step in the right direction*, not silently overwritten.
- In the new song, establish a distinct **Verse** and **Hook** material family, with an actual source-licensed Consequent/recognition relationship, not two copies both tagged Establish. Use existing argument laws when genuinely supported, or build a narrow **PopKinship** witness. Do not lie to C137's original stricter altered-consequent validator to make a new hook compile.
- If one section is genuinely Keys-carried for a thematic handoff, the Keys support voicing must temporarily step aside or be independently assigned to another role; do not add extra notes inside a reserved strict-carrier window and silence the content witness.
- Source identity should survive transposition/register/voice changes only under explicitly declared transport; category-style metaphors are not executable proof.

## Minimum discriminatory experiment — not a six-month rewrite

**First construct a paired 8-bar sketch (4-bar verse + 4-bar hook) at ~104 BPM:**
- Same underlying two legal pop harmonies and bass identity as current, with actual verse/hook difference over the harmonic base; compare to current 8-bar loop.
- An **original melodic head** with varied onset phases (.0/.25/.5/.75), 1–3 note pickups in some bars, 0-note gaps in others, and at least one meaningful held arrival. Numbers are design starting points, not universal aesthetic thresholds.
- Keys: 2- or 3-note clipped chordal shells at *selected* gaps, silent on active lead phrases; their pitches correspond to sounding chord qualities. Reserve a distinctive chord color for hook arrival rather than full ninths every bar.
- Bass: keep tested 4-cell identity initially; let kick choose a small fraction of its pickups from previously authored/committed bass evidence. Withhold the obligatory end-of-bar push once to make its later return meaningful.
- Drums: keep stable original backbeat as an internal control; remove automatic phrase-end fill and/or dense 16th hats in an opt-in pop arm. Author deliberate open/closed hi-hat/silence responses rather than using chord==dominant as an unconditional 16th-hat trigger. Only the new arm changes.
- Pad or second Keys layer: a selectively gated voiced chord for chorus; compare with lead-only/keys-only/strings-removed stems; ensure the effect is actually audible, not only in Score metadata.
- Physical performance: test whether the newly authored lead feels expressive through current warm patch, and only then experiment with a dedicated legato/non-percussive lead voice, phase-continuous pitch glide, intra-note gain expression or vocal-like envelope. Do not build an entire vocal synthesis stack without this discriminating audition.

**Then expand to 32 bars** (intro, verse, pre, hook, break, return) only when the shorter source/hook and ensemble contrast earn an ear verdict. A new style should not be forced into a literal C137 16-beat fixed-episode semantic regime; use a typed section-form or existing legal tiling where possible, without declaring fake sections.

## Verdict-changing probes (predeclare PASS/FAIL/AMBIGUOUS)

| Probe | Positive prediction | Falsifier / negative control |
| --- | --- | --- |
| P1 Hook recognizability | Verse and hook have different prosodic rhythms but a shared identifiable source germ; hook returns recognizably with new support | Scramble hook notes, same timing/count/energy: independent source content witness must fail. If no listener difference, source relation is not audibly discriminating |
| P2 Sounding harmony | Selective Pad/Keys support adds legitimate chord tones and improves harmonic fullness around the exposed foreground | Mutate a 3rd/7th into a dissonance without altering chart metadata: new independent sounded-harmony witness fails. Audibility must be checked in PCM/stems |
| P3 Bass→drum listening | Change a committed bass pickup at known earlier beat: a *permitted future* drum comp decision changes; immutable earlier backbone remains identical | If drums unchanged, "responsive drummer" claim fails. If earlier samples change, chronology breaks. Fixed drums/independent jitter are negative controls |
| P4 Prosodic diversity | Source-gated varied attack grouping and real breath windows sound more singer-like than the existing four equal offbeats | Matched-density fixed-grid vs authored prosody, same patches, loudness; improvement cannot be inferred from more notes |
| P5 Collective push | An earned delayed bass/keys push returns meaningfully after being withheld; band supports it without identical accents all bars | Unconditional 3.5-every-bar control with same note budget; if indistinguishable or better, do not enshrine the proposed variation |
| P6 Section lift | Hook arrival from a quieter verse is audible in specific changed role participation, melody register/prosody and harmonic exposure, not just gain | Loudness-only chorus control matched to same perceived level; fake fader lift must not get semantic credit |
| P7 Register/mix (Halsey axis) | Low pad/high chord support leaves middle lead exposed; loudness-matched source has better intelligible melodic shape | Full-band pad masks lead; high pad/keys compete, score still passes → PCM/ear veto |
| P8 Palette/performance | Sustained lead phrase carries continuous sounding energy (not just longer source dur) and resolved articulations | Same Score through old FM EP vs restrained legato patch, gain-matched; if no material improvement, do not add DSP complexity |
| P9 C137 lock | Exact 51 historical hashes, source controls, Jerry tests unchanged at each sound-changing rung | Any historical SHA drift stops implementation; hashes never auto-updated |
| P10 Generality | Fresh holdout seeds exhibit distinct source-approved prosodic hook variants while preserving family identity and voice law | All output byte-identical across seeds (unless explicitly labelled fixed demo), or identity degenerates under fresh seeds |
| P11 Form-law loophole | A new 32-bar PopSongForm checks actual section obligations and warranted returns beyond exact chord repetition | Setting pop loop_bars=total bars should not automatically pass as a "completed song" without an independently observable return |
| P12 CI | Acoustic lock is blocking on the deterministic CI runner, not just reported locally | Introduce one known PCM change in an isolated mutation branch; CI must fail until reverted; never rebaseline silently |

**Complete factor contrasts**: full 2×2 crossing **prosodic lead vs fixed-upbeat keys line** × **source-aware bass/drums vs old fixed pulse**, with identical harmony/tempo/mix; separate 2×2 for **selective audible chordal support** × **melody presence**. Run all four cells before attributing interaction. Reserve unseen seeds/tempo transport as fresh holdouts. Do not calculate "information density" as a pseudo-psychophysical scalar without an operational measurement model.

## Work order and CI discipline

**G0 (immediate):** add a required deterministic CI acoustic verification job for the existing lock (when runner/toolchain matches reference). Preserve script's immutable read-only verification and red-first calibration. If cross-runner PCM hashing diverges, pin container/toolchain/architecture, explicitly separate portable score witness from bitwise runner-specific hash.

**G1 (fastest musical discriminator):** original eight-bar verse→hook vertical slice with legitimate distinct source carriers; stems and loudness-matched A/B versus existing 8-bar vamp. Build only what current types legally permit; no full 32-bar push before ear verdict.

**G2:** audible chordal comp + low/high register framing; independent sounded harmony/voice-occupancy witness. Contrast lead alone versus full band without loudness confounding.

**G3:** pop-gated drummer actually reacts to committed bass/key material, preserving fixed pulse with selectively earned syncopation. Test no future hearing and relevant mutation controls.

**G4:** real section-level flow (pre, chorus, break, return), full-song narrative memory, singer-like expressive phrase interface (and eventually better timbral tools as the ear demands).

Prioritized epistemic boundaries:
- **Disclosed/observed**: source facts, current CI, code-inspected deficits and past maintainer ear verdicts.
- **Conjectured**: a lead/topline, chordal comp and source-aware drummer will produce the requested recognizable pop feel.
- **Unverified**: which specific new sound sounds better; how much timbre versus prosody contributes; whether a full Halsey-style expressive lead is feasible with current synth.
- **Next lamp**: P1→P3 and matched blind listening. A single good seed cannot demonstrate generality.

**Do NOT** merge, publish, change default, un-draft, invalidate accepted 16-bar C137 or replace an active Claude implementation branch. Research branch should remain docs-only. The first useful audible victory is not more harmonic ornamentation: it is an **identifiable original melodic personality above a rhythm section that knows when to support, anticipate, and leave space.**
