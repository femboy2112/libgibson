# HumanMusic Round XV — playing the connective

Starting commit: `89d1d4ce558c7774e1b5fb5c8a4af8879ae9b4b1` on
`feat/v0.4-humanmusic-audio`; verified against origin before edits. Base/main:
`7b4efdc4c7fc6523d890a28f2aa8e27a6ba59286`. PR #70 remains draft.
Package 0.3.1, ABI 1, Rust-only audio, MSRV 1.85 remain unchanged.

## Why Round XIV still failed BLACK_ICE

The maintainer accepted SWISS and rejected BLACK_ICE's slowly attacked chromatic events.
Round XIV correctly removed false SlidePath claims. It did not make an ordinary attacked
neighbor or approach perceptually subordinate to its target. Pitch theory can explain
C#→C while the instrument presents C# as a separate statement.

Frozen acceptance: StablePropulsion, seed 2112, FusionConversation, default options,
120-beat deflected-lift trace, R14 coherent realization.

| Role/world | Target latency | Written duration | Modeled audible duration |
|---|---:|---:|---:|
| BLACK lead, short approach | .340909 s | .306818 s | .406004 s |
| BLACK lead, slow neighbor | .681818 s | .340909 s | .440094 s |
| SWISS lead counterparts | .254237 / .508475 s | .228814 / .254237 s | .093000 s |
| BLACK bass approach | .340909 s | .306818 s | .383507 s |
| SWISS bass approach | .254237 s | .228814 s | .312741 s |

SWISS bass also sustains. Its distinction is physical timing and measured lifetime;
the short pulse/pluck explanation belongs to its lead. All lifetimes use the existing
`sonority::audible_end_at` / release-tail model, at the existing 30 dB floor.
This is an envelope approximation, not proof of a listener's perception.

The failing witnesses were committed before the diagnostic or production implementation.
[Completed baseline output](fixtures/humanmusic-r15/baseline-failures.txt) records two
failures and two passing controls. [Frozen ruler](fixtures/humanmusic-r15/frozen-ruler.txt)
then distinguishes all 17 BLACK lead and 18 bass relationships from the accepted SWISS
control, without changing R14's notes.

## Expression law and source boundary

`expression::ExpressionEvent` keeps the source's structural bit separately from its
`PitchFunction`. The lead retains `motif::LineNote.structural`; an integer-beat heuristic
would wrongly protect several optional BLACK connectors. Bass-owned pickups are explicitly
identified by the bass realizer's `approach` provenance. Shared unisons inherit the final
lead rhythm and are protected from a second independent bass rewrite.

`q` projects an ordered line onto structural pitch, onset, harmonic function, and complete
provenance (including material, motif, action and obligation identity). The source-level
implementation additionally protects structural gate and velocity. It asserts
`q(E(L)) == q(L)` and preservation of material/action identities. Immediate stable local
destinations are anchors even when the motif marks them optional: C at 25.5 must stay at
25.5; the grace must not instead chase the next structural note.

The performance fiber owns optional connective onset, gate, velocity, substitute pitch and
omission. Space is explicit and ledgered. Structural destination pitches and beats never
move to rescue an ornament. The original SongMap and PerformancePlan are read unchanged.

`perform_expressive` is a new opt-in arm. Default `perform`, temporal, mass, tension and
coherent controls remain available. Acceptance uses the default Independent coupling:

```text
SongMap → PerformancePlan → temporal lead → expressive lead
    → keys hear final lead → temporal bass → expressive bass
    → pad hears final lead + keys + bass; drums hear final lead + bass → Score
```

The independent pad and drum consumers are ordered as in R14. Expression runs inside the
lead/bass source realizer, before it returns. No finished Score is inspected and repaired.
`comp::realize_pad_heard`, its spacing/rooting rules, heard identity, and hearing
infrastructure are unchanged. The new API rejects non-Independent coupling before realization: historical CoupledR8 has a
different source order, and Surgical would run a post-hoc repair. Both remain available only
through their existing historical arms. A negative control pins this boundary.

Source lead decisions use the chart's support obligation. They do not pretend to have
heard a future pad. Bass additionally receives committed lead/keys. Frozen/full-score
diagnostics separately report actual audible semitone-class support contacts, including
pad contacts. This distinction prevents a circular causal claim.

## The ruler and realization ladder

`ConnectiveViability` distinguishes AsWritten, NeedsCompression,
NeedsDifferentArticulation and NonViable. Each observation contains pitch/path validity,
role, onset/duration in beats and seconds, destination and function, target latency,
patch-derived lifetime, fresh attack, optionality, metrical push/pickup, kinetic value,
chart dissonance and realized contacts. There is no quality scalar.

The provisional human-calibration boundaries are explicit:

- A naturally fleeting non-bass sound (≤115 ms) may connect across ≤550 ms.
- Other attacked connectors use ≤280 ms latency; bass exposure is bounded at 330 ms,
  exposed lead at 200 ms (240 ms without dissonant support).
- A very short bass pickup (≤115 ms audible) may precede its target by ≤360 ms. This
  allows a metrically required Push to keep its onset while losing its long asserted body.
- Minimum attacked gate is max(25 ms, patch attack). An unavoidable long release tail
  can make a chromatic candidate nonviable even at that gate.

AsWritten is tried first. Failing optional groups compress backward from their fixed
local destination into a grace or burst. Gate, inter-onset spacing and accent depend on
kinetic state, pickup and harmonic contact. Release is included in candidate remeasurement.
No move crosses a harmony boundary, a stage exclusion or a hole. Push/Hit onset obligations
stay fixed and receive a short gate where that passes the same physical ruler. Every
candidate's actual local pitch geometry is rechecked before emission.

If compression cannot work, a nearby chord tone or licensed stable color strictly within the direction of travel
is tried; it cannot duplicate the destination early. Otherwise optional space is permitted
when material/action identity and required onsets survive. Low kinetic state can prefer
space. Chromatic appetite proposes vocabulary; it never mandates retention.

These thresholds are calibrated engineering hypotheses. The complete tempo × patch
synthetic experiment separates their effects; its four cells share one implementation
and are not four independent listening judgments. At a one-beat neighbor latency only
118 BPM with the fleeting patch passes as written; the other three compress. The audible
lifetime interaction is the sustained gate's additional physical time at 88 BPM. The complete matrix is recorded in
[tempo-patch-factorial.txt](fixtures/humanmusic-r15/tempo-patch-factorial.txt). Its audible-duration
mixed difference is +0.086671803 s; the latency mixed difference is zero. Those are modeled
configuration effects, not independent evidence of a perceptual mechanism. No probability,
quantum mechanism or perceptual theorem is inferred.

## BLACK_ICE lead

All 17 optional chromatic lead events are shortened, softened target-relative graces.
No pitches are substituted or omitted. No B4 appears among those declared connectors;
B4 extension vocabulary is left alone.

Representative exact timings (88 BPM, seconds = beats × 60/88):

| Relationship | Old onset → new onset (beats) | Old onset → new onset (seconds) | Fixed target | Written gate before → after |
|---|---|---|---|---|
| C#5→C5, Neighbor | 24.5→25.253445408 | 16.704545455→17.218258233 | 25.5 b / 17.386363636 s | .5→.065589331 b; .340909091→.044719998 s |
| C#5→D5, Approach | 33→33.254237408 | 22.5→22.673343687 | 33.5 b / 22.840909091 s | .45→.072717331 b; .306818174→.049579999 s |
| A#4→A4, Approach | 84.5→84.754211594 | 57.613636364→57.786962451 | 85 b / 57.954545455 s | .45→.072485015 b; .306818174→.049421601 s |

The first neighbor's velocity changes .774199963→.606972814; its audible lifetime falls
from .440094 s to about .143905 s, with .168105 s latency to C. The vacated time is phrase
space. Every old/new velocity, onset, duration, destination, function, reason and strategy
is in the final decisions and perturbation receipts.

## BLACK_ICE bass — all 18 approaches

| Original onset | Relationship | Decision |
|---:|---|---|
| 3.5 | G#2→A2 | Late grace; original pad A3 minor ninth contact |
| 11.5 | G#2→A2 | Late grace |
| 15.5 | D#2→D2 | Keep required ensemble Push beat; 25 ms gate |
| 27.5 | G#2→A2 | Late grace |
| 35.5 | G#2→A2 | Late grace |
| 39.5 | G#2→A2 | Late grace |
| 43.5 | G#2→A2 | Late grace |
| 55.5 | G#2→A2 | Late grace into A from Dm7 |
| 59.5 | G#2→A2 | Late grace |
| 63.5 | G#2→A2 | Late grace |
| 75.5 | G#2→A2 | Late grace |
| 79.5 | D#2→D2 | Keep required ensemble Push beat; 25 ms gate |
| 84.5 | A#2→A2 | Inherit expressed lead/unison, retain action identity |
| 91.5 | G#2→A2 | Late grace |
| 99.5 | G#2→A2 | Late grace |
| 103.5 | G#2→A2 | Late grace |
| 107.5 | G#2→A2 | Late grace |
| 115.5 | G#2→A2 | Late grace even without an audible pad/lead |

Thus 15 source bass graces, two short metrically anchored pickups, and one inherited
unison grace. Zero kept in their complete old articulation, zero substitutes, zero
omissions. The unison's bass gate is 0.1 beat / 68.182 ms, reflecting the existing bass
realizer's minimum; its ~144.87 ms lifetime still passes.

G#2 at 3.5 b / 2.386363636 s becomes 3.786322691 b / 2.581583653 s; target A2 stays
4 b / 2.727272727 s. Gate .45 b / .306818174 s becomes .064533331 b / .043999998 s;
velocity .358136028→.282569349. Audible lifetime is ~.120689 s instead of .383507 s.
The two D# pickups retain .340909 s target latency but sound for only ~.101689 s.

A first candidate incorrectly moved the Push notes and lost action receipt a2. That
failure exposed an onset obligation missing from the performance-fiber contract; the
source now preserves those beats. The receipt audit was not weakened.

## Causal perturbations and heard identity

BLACK notes increase 449→458 because the unchanged keys realizer hears the new lead
space and places additional comping there. The shared keys/bass unison follows the final
lead grace. The bass note preceding that unison can sustain longer before its new next
onset. These are source decisions by dependent players, not post-hoc mutations.

The exact perturbation accounting is below. Categories overlap: one note can be retimed,
shortened and softened. The bass's nineteenth duration change is its structural D2 before
the inherited unison, whose source trim now meets the later pickup; its pitch and onset stay fixed.

| Role | Retimed | Duration | Velocity | Pitch substitute | Omitted | Added |
|---|---:|---:|---:|---:|---:|---:|
| Lead | 17 | 17 | 17 | 0 | 0 | 0 |
| Bass | 16 | 19 | 17 | 0 | 0 | 0 |
| Keys | 1 | 9 | 0 | 0 | 1 | 10 |
| Pad | 0 | 0 | 0 | 0 | 0 | 0 |

The omitted keys B4 at 98.25 is an unstamped comping extension. The existing keys solver
alternates three- and two-note shells according to its chosen stab order; inserting new gap
stabs changes that order. It is not an omitted structural lead/bass connector. Exact rows:
[BLACK perturbation](fixtures/humanmusic-r15/final/black_ice.perturbation.txt).

The BLACK pad is exactly unchanged. No pad architecture was reopened. The drums change
because they consume the final bass; SFX stay identical.

SWISS score fingerprint is exactly `beb08c057ed87d94` in both arms: no note, duration,
velocity, pitch, drum or SFX change. Its pad remains R14. Song fingerprint in both worlds
and arms is `af5360442b9b6729`; performance fingerprints remain unchanged.

The expressive arm records one additional hearing edge: bass consumes the final keys as
harmonic support. R14 has seven edges, R15 has eight. This is SWISS's only hearing-metadata
addition; its audible score and WAV remain identical. The existing stale-hearing checker
is unchanged.

Both acceptance worlds retain 45/45 witnessed interactions, zero false temporal-function
claims, zero identity flips and zero stale hearings. Existing hearing receipts do not
include velocity; the source order and explicit velocity receipts establish where the
velocity changes occur without overstating that checker.

The deliberately stronger exploratory assertion that *every instantaneous identity status*
remain identical failed: expression changes brief implied/passing windows. R14's actual
flip criterion (a rival held ≥0.5 s) remains intact, with zero flips. The final receipt
reports all transient rival windows too; nothing was erased or relabeled. This is not
permission to alter the frozen pad solution.

A fresh VAPOR95 holdout exposed a second source interaction: omitting an unplayable long-tail
connector reduced the lead's bar density, so the existing keys realizer doubled its comp gate
and held a rival chord long enough to flip. The source substitute ladder now considers the
chart's licensed stable tones as well as literal chord tones before omitting. This preserves
a viable diatonic descent in that case without retaining the exposed chromatic pitch, changing
the keys or pad solvers, or excusing the failed identity check. The seed-7 witness is permanent.

## Audio and reproduction

```bash
cargo +1.98.1 run --release --example expressive_music_lab -- \
  --render --out=target/humanmusic-r15/final
cargo +1.98.1 test --test audio_expression
cargo +1.98.1 test --release --test audio_expression fuzz_expressive -- --ignored --nocapture
```

The final directory contains:

- `black_ice_r14.wav`, `black_ice_expressive.wav`
- `black_ice_r14.lead.wav`, `black_ice_expressive.lead.wav`
- `black_ice_r14.bass.wav`, `black_ice_expressive.bass.wav`
- `swiss_r14.wav`, `swiss_expressive.wav`

Both regenerated R14 full mixes are byte-identical to the preserved Round XIV WAVs. SWISS's
expressive WAV is byte-identical to its R14 WAV. BLACK's full mix and both stems differ.
See [audio comparisons](fixtures/humanmusic-r15/final/audio-comparison.txt) and
[SHA-256 hashes](fixtures/humanmusic-r15/final/SHA256SUMS).

Text receipts are mirrored under `docs/fixtures/humanmusic-r15/final/`. WAV hashes bind
the local audio artifacts; each stem is rendered with a fresh synth and explicit mask.

## Verification

Toolchains: stable `+1.98.1`, MSRV `+1.85`. Verification runs use two Cargo build jobs and
one test thread for the complete suites.

- Release preflight: **PASS**, including the full default suite, fmt, strict all-target/all-feature
  clippy, warnings-denied rustdoc, locked/fresh-resolution MSRV, package, ABI v1, clean-room
  C/C++/Python/Go/Rust consumers, notices and licenses.
- The complete all-features suite (including audio-cpal; 1,291 passed, seven ignored) and explicit locked MSRV 1.85
  all-features library check also pass.
- Fourteen expression tests pass; the additional ignored expressive sweep passes **240
  performances / 2,771 source decisions**, in debug and release. It checks source structure,
  SongMap/performance conformance, independent temporal support for every transformed note,
  action receipts, no newly flipped interval, no fake slide and zero stale hearings.
- The unchanged R14 ignored sweep passes **240 performances**. Its known non-acceptance
  ambiguities/flips remain recorded; Round XV does not claim to eliminate every old defect.
- Five repository example suites pass (19 tests).
- The acceptance R9 conformance, R10/R11 unchanged song/regime controls, R12 temporal
  controls, R14 identity/gesture controls and the source-level expression controls pass.
- Manifest, lock, ABI baseline, headers and bindings have no diff from the mission start.

Two earlier concurrent full-suite attempts failed the existing terminal demo's eight-second
`acid_cinematic_auto_completes_full_story_and_restores_terminal` screen deadline. The unchanged
test passed alone, and the complete default and all-features suites then passed serially. The
[failing run](fixtures/humanmusic-r15/concurrent-all-features-failure.txt) and
[isolated pass](fixtures/humanmusic-r15/pty-isolated.txt) are retained. Concurrency/load is a
plausible contributor, not a proved terminal root cause; no timeout or unrelated demo was patched.

The final receipt-only correction recovers retimed optionality from the source ledger rather
than the old motif beat. The API also rejects historical non-Independent coupling before
realization, so an old post-hoc arm cannot invalidate the new causal contract. Neither changes
the acceptance notes or WAVs.

## Claim boundary and parked work

Observed: frozen physical timings, diagnostic separation, source decisions and machine
invariants within the executed coverage. Hypothesis: these calibrated gestures make
BLACK_ICE musically legible. Human listening is the independent acceptance bearing;
three same-model read-only scouts and shared-envelope calculations are not independent
auditions. No claim of hearing the generated audio is made.

Portamento/legato is parked. Every grace and burst still uses fresh SynthVoice attacks;
there is no frequency glide or voice-continuity claim. Deterministic melodic voice
ownership would require a separate synth scheduling contract. Also parked: canned lick
libraries, another emotion model, pad redesign, post-hoc tension repair, default-arm
promotion, Round XVI, merge, version bump, tag and release.

**The engine now adapts connective articulation to the physical performance context.
Whether those realizations sound like good licks remains a listening judgment.**
