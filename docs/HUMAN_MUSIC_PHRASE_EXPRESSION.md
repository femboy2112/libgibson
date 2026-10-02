# HumanMusic Round XVI — coherent gestures and heard voicing paths

Mission starts at `bc9fbd7c1ea1c2688708a48ec37b493f7e6a2583`, verified clean and equal to
`origin/feat/v0.4-humanmusic-audio` after fetch. Main/base remains
`7b4efdc4c7fc6523d890a28f2aa8e27a6ba59286`. PR #70 stays draft. Package, ABI 1,
MSRV 1.85, Rust-only audio and release gates are unchanged.

## New human evidence supersedes the old acceptance premise

The maintainer rejected recurring SWISS Cmaj7 sonorities around 16 seconds, and heard
BLACK R15 as closer but stuttering. The Round XV report's premise that SWISS was accepted
is therefore refuted. The historical report and its exact R14/R15 audio remain controls;
no identity diagnostic overrides this new listening evidence.

There are two separate unresolved perceptual questions. A chord can retain its intended
identity while its register/path is objectionable. Individually subordinate connectors
can form a discontinuous phrase. Neither problem is answered by a scalar quality score.

## Frozen falsifiers and source causes

Before production changes, commit `e9d013e` records two active failing witnesses and
passing instrument controls. The [experiment contract](fixtures/humanmusic-r16/experiment-contract.txt)
separates rivals and reserves fresh seeds/worlds from fitting.

BLACK R15's frozen lead has all 17 connectives locally viable; 16 share one 20-ms-binned
target-latency/gate shape, 15 have long silence followed by tiny grace and target, and all
17 moved onsets are off the exact musical grid. A negative control preserves regular
eighth-note repetition. Repetition alone is not the failing condition.

The near-identical morphology follows directly from R15's formula: its kinetic gate
coefficient and step coefficient nearly cancel. Its small seconds-based variations do
not create a different rhythmic thought. The new planner does not modify that historical
formula or the physical ruler.

The keys' onset-clearance rule uses a 0.2475-beat neighborhood. Moving a lead event from
27.5 to about 27.753445 vacates its original slot just past that cutoff. R15 admits four
new comp attacks at 27.5, 59.5, 96.5, 99.5, yielding ten added keys notes and changing the
chosen-stab shell order. That is an observed source dependency, not a listening inference.

For SWISS, the reported contexts 28/36/44 share E4 C5 G5 B5. The old R12 cluster was
E4 B4 C5 G5. R14's first admissible octave-spacing move raises B4 before considering
lowering C5; the result puts the seventh on top and moves G5→B5→G5. Controls elsewhere
use C4 G4 B4 E5. In particular, contexts36/60 share keys material, and44/108 share both
keys and lead material: chord labels or band-content labels alone do not explain the
reported difference. The diagnostic retains the actual sounding sets, guide/root/bass
relations, register, overlap contacts, onset fusion, exposure proxies and adjacent motion.

## New opt-in source architecture

`perform_phrased` builds on the same SongMap and PerformancePlan. `perform_coherent`
(R14) and `perform_expressive` (R15) keep their exact historical audible fingerprints.
`perform_phrase_experiment` exposes independent phrase, ownership and support-voicing
factors for explicit ablations. Historical post-hoc coupling modes are rejected at this
new entry point before realization.

```text
SongMap → PerformancePlan
  → authored lead + phrase ownership → phrase-expressed lead
  → keys hear final lead and independently read authored ownership
  → bass reads final lead plus authored lead reservations
  → authored bass → phrase-expressed bass (committed lead gestures available)
  → pad chooses its support path against final lead/keys/bass
  → drums hear final lead/bass and read authored bass pulse intent
  → Score
```

The hearing ledger contains only final acoustic notes. Ownership contains no pretend notes.
Shared unisons inherit the lead's chosen gesture, with role-specific pitch/gate/velocity.
The drum pocket reads original bass pulse reservations rather than rounding a grace into
a new arrangement. Shared-action accents still follow the final gesture.

`AuthoredOccupancy` distinguishes reserved attacks, internal rests, phrase ownership and
actual floor yield. A lead rest does not command the whole band to stop: previously
available accompaniment may coexist, while a solo/fill opening needs genuine yield.
Stage holes remain the orchestration authority. Expressive retiming or omission cannot
manufacture a newly available accompaniment slot. Bass counterline selection also checks
original activity reservations and uses authored attack counts for its density budget,
while retaining final acoustic collision checks. A synthetic Counter-mode control proves
that literal silence alone would otherwise grant a new counterline. The default acceptance
arrangement does not exercise that Counter-mode failure; it is a consumer-boundary witness.

## Phrase fiber and rhythm syntax

`PhrasePlan` retains the authored thought, source rhythm, structural/local destination,
material/action context, phase, kinetic/accent evidence, previous gesture and rejected
candidates. It can gather an optional stable precursor and chromatic connector into one
compressed fragment. Single pickups select discrete musical positions using phase and
groove context. Swing is transported through the world's declared offbeat rule; triplets
are restricted to triplet subdivision worlds/languages. The unchanged R15 physical
observer accepts or rejects the resulting attacks in seconds.

The projection `q` forgets explicitly optional performance detail. The source asserts
`q(E(L)) = q(L)` with structural pitch, onset, gate, velocity and provenance fixed.
Local stable destinations also stay fixed even when their motif slot is optional.
Expression cannot cross harmonic/stage boundaries, erase required actions/materials, or
move an anchored Push/Hit to rescue an ornament. Optional bass space is explicit: it may
leave a destination's pickup to a committed lead fragment when no shared action requires
its participation. Chromatic vocabulary is not replaced by canned licks or random jitter.

Every candidate remains a source-owned proposal until validation finishes. No finished
Score is inspected and repaired. R15's physical fallback is retained as a lower-level
instrument, with final source path obligations checked for effects of omission.

## Flagship result and explicit tradeoffs

BLACK now has five genuine two-note fragments into destinations28/50/60/90/100. The
source notes at26.5+27.5 become27.5+27.75 before fixed C@28; notes48.5+49 become49.5+49.75
before fixed C@50. Pitch order is unchanged. Ten chromatic lead pickups use a quarter-beat
latency (170.455 ms at88 BPM); seven use a dotted-sixteenth latency (255.682 ms). Twelve
remain single-note pickups. Chromatic gates are68.182 or95.881 ms, with velocity contours
across the fragments. For the first fragment, source onsets26.5/27.5 beats
(18.068182/18.750000 seconds) become27.5/27.75 beats (18.750000/18.920455 seconds),
with its destination fixed at28 beats (19.090909 seconds). This is real metrical variation plus actual multi-note grouping,
not merely tiny seconds-based changes labeled as new strategies.

The calibrated isolated-tiny-grace witness falls15→0. **Part of that change follows the
new gates exceeding its65-ms boundary. Zero is not proof of good phrasing or removal of
perceived stutter.** The multi-note transformations, exact grids and independent occupancy
ablation are the more substantive mechanical evidence. A listener can still reject them.

Bass leaves three optional pickups to committed lead fragments (original beats27.5/59.5/99.5).
All structural arrivals stay fixed; Push15.5/79.5 keep their exact onset. The shared unison
inherits the lead, with a role-specific gate. The ordinary drum pocket is now exactly R14;
only explicit shared-unison accents follow the new gesture. A failing full-score test
exposed a second semantic leak through inherited bass unison slots; the reservation map
now recovers their authored origin without falsifying acoustic hearings.

BLACK R15→R16 perturbations (categories overlap):

| Role | Before→after count | Retimed | Gate changed | Velocity changed | Pitch changed | Omitted | Added |
|---|---:|---:|---:|---:|---:|---:|---:|
| Lead |80→80|22|22|22|0|0|0|
| Bass |96→93|13|14|12|0|3|0|
| Keys |166→157|1|9|0|0|10|1|
| Pad |116→116|0|0|0|0|0|0|

The lead's22 changes are17 connectors plus5 stable optional precursors. Keys remove the
10 accidentally added R15 support notes and restore the previously displaced B4 extension;
the shared unison follows the final lead. Relative to R14, keys have only one retimed/gate-changed
shared-unison event, with no inserted or removed notes. Full BLACK count is449 (R14),458
(R15),446 (R16). There is no pitch substitution in this acceptance example. Drums remain479 hits. Relative
to R15,53 ordinary hit records and2 unison records differ; relative to R14, all475 ordinary
hits are exact and only2 of4 unison accents differ. These are event-record differences,
including timing/velocity, not55 newly authored hits. SWISS retains all402 drum hits exactly.
The [drum receipt](fixtures/humanmusic-r16/final/drum-perturbation.txt) keeps this separate
from pitched-note perturbations.

The 2×2 phrase/ownership experiment holds pad selection off and independently toggles
phrase planning and semantic occupancy. Comp attacks are52/48/52/48 for00/01/10/11.
Ownership changes no lead notes. All four cells have current hearing receipts. Thus the
accidental micro-hole comping dependency is eliminated independently of the phrase recipe.

SWISS changes exactly six pad pitches: C5→C4 and B5→B4 at each of28/36/44. The result is
C4 E4 B4 G5. Chord pitch-class multiplicity, four voices, onset, gate and velocity remain
unchanged. Lead, keys, bass, drums and all seven named Cmaj7 control sites remain exact.
The top line G5→B5→G5 becomes G5→G5→G5. Ordered incoming/outgoing displacement decreases;
overall span stays19 semitones, so this is not a simplistic narrower-is-better rule.

The source receipt explicitly records five new adjacent-pad release contacts, each
259.065 ms. At28, the lower root also contacts bass B2@31 for312.741 ms before the bass
arrives at C3@31.5 (254.237 ms latency, nonweaker destination). The original generic guard
rejected this candidate. Its narrowly justified alternative uses the unchanged physical
ruler on the bass's original ChordTone stamp and actual immediate next note. Identical
resolving contacts already occur in within-song controls4 and108; slower88-BPM and
non-resolving immediate-next controls reject the exception. This is a **provisional
listening hypothesis**, not a declaration that the new contacts are harmless.

## Adversarial evidence and source boundaries

Fresh validation spans120 performances: five seeds3/7/19/43/101, two composers, two
stories, three worlds, and nominal/12-BPM-slower tempos. VAPOR95 was withheld from
candidate fitting. The sweep checks historical conformance, every original structural
lead destination, every declared local destination, individual transformed connective
functions, no newly held identity-flip interval relative to frozen R15, no lost interaction receipt, no fake
SlidePath and no stale hearing. This is finite coverage, not a universal proof. Non-flagship historical identity errors can
remain; non-regression is not a claim of globally zero flips. The separate historical R15
sweep compares R15 against R14 on its own 240-case family.

Directed cases include empty/singleton fibers, consecutive connectors, kinetic extremes,
16 tempo/patch/kinetic combinations, shared-action anchors, harmony boundaries, internal
rests versus true yields, and a connector trying to jump over a hole. The last falsifier
showed why a legal landing is insufficient: the whole relocation corridor must remain
inside stage/accent authority. The source now explicitly yields optional phrase space
when a hole divides the thought, and rejects any surviving illegal relocation.

A separate long-release/low-kinetic falsifier exposed fallback omission changing B→C→B
into B→B while retaining ChromaticApproach. New-source finalization checks actual surviving
neighbors and records truthful stable membership or optional space. Frozen R15 is not
silently repaired. Failed witnesses and successful repairs both remain in the fixture ledger.

## Evidence status

| Claim | Status and boundary |
|---|---|
| R15 microtiming created four new keys opportunities | Observed in exact source and frozen/factorial receipts |
| Structural projection, historical fingerprints, hearings and action receipts | Machine invariants within executed tests; not universal musical claims |
| Discrete fragment planning addresses the maintainer's phrase complaint | Calibrated design hypothesis; diagnostic morphology is not an audition |
| Root/inversion and upper-guide voice path explain SWISS's ugly family | Leading hypothesis supported by within-song contrasts, not uniquely identified |
| Spectral fields establish musical quality | Not claimed; PCM measurements share the synth lineage |
| R16 sounds better | Human-unverified |

The three scouts, generated tests and envelope calculations share model/implementation
provenance. Their agreement is not independent listening evidence. The maintainer's ear
remains the independent acceptance bearing.

## Engineering verification

Final production source is `3f3eaa28486dd114b354a6c9040f204925d086e1`.
[GitHub CI](https://github.com/femboy2112/libgibson/actions/runs/36671679493) and
[Release Preflight](https://github.com/femboy2112/libgibson/actions/runs/36671679458)
both passed at that exact source. CI includes the full default-feature suite, strict
Clippy, formatting, rustdoc with warnings denied, MSRV 1.85, release build/package/ABI
checks, PTY tests, C/C++ sanitizers and C/C++/Python/Go bindings. Release Preflight also
checks fresh declared-range MSRV resolution, clean-room consumers and notice freshness.
Machine job receipts are stored beside the experiment fixtures. The final artifact commit
adds only the already locally compiled/rendered comparison harness, window tool, documentation
and receipts; its separately triggered CI status should not be conflated with the completed
production-source runs.

The complete local `cargo +1.98.1 test --all-features` run passed **1,327 tests**, with
eight intentionally ignored tests. Explicit `cargo +1.85 check --locked --lib --all-features`
passed, including audio-cpal. The normal suite retains the previous HumanMusic conformance,
meaning, phenomenal, temporal, harmonic-identity and physical-gesture controls. New directed
and integration tests cover the phrase and agency boundaries. Flagship SongMap conformance
passes all6 identity sites,15 landmarks and30 chord changes; all six arms retain45/45 actions,
zero stale hearings, zero false temporal claims and zero held identity flips.

An earlier local preflight completed successfully. A second local preflight, begun while
CI was pending, was cancelled after exact-source CI/preflight completed; its partial log is
explicitly labelled **CANCELLED**, not PASS. The already-complete all-feature run and the
exact-source CI gate are the final full-suite evidence. No gate or threshold was weakened.

The explicit historical R15 sweep passes240 performances and2771 source decisions. The
unchanged R14 sweep also passes240 cases under its regression contract, retaining96 known
identity-flip windows (102.8 seconds) outside the acceptance songs. Its name is not a claim
that every chord everywhere is correct. Alongside the new120-case sweep, these are600
explicit sweep evaluations; they are not600 independent human observations.

## Reviewable commit sequence

| Commit | Scope and evidence |
|---|---|
| `e9d013e` | Frozen R15 phrase and SWISS register/path failures, with negative controls |
| `0f3beaa` | Authored ownership distinct from acoustic vacancy |
| `762d393` | Metrical phrase plans and structural projection tests |
| `3104676` | Keys/drum consumers use intent while hearing final notes |
| `272d9cd` | Surviving-path revalidation after optional omission |
| `bfd1939` | Source support-path candidates and explicit contact tradeoffs |
| `2ee330f` | Opt-in R16 arm, historical controls and integration witnesses |
| `f07a37b` | Shared grace no longer leaks into the ordinary drum pocket |
| `065dfa6` | Relocation corridor cannot jump across a stage hole |
| `72d34fa` | Bass counterline agency and density read authored lead reservations |
| `3f3eaa2` | Correct observer entry-silence origin; opening attack is not preceded by fictitious silence |
| Final artifact commit | Three-arm listening harness,84 WAV hashes/windows, raw diagnostics, validation receipts and explicit claim limits |

All commits are on the working branch, pushed with the requested co-author attribution.

## Listening reproduction

```sh
cargo +1.98.1 run --release --example phrase_music_lab -- \
  --out=target/humanmusic-r16/final --render
python3 scripts/dev/humanmusic_r16_windows.py target/humanmusic-r16/final --spectral
```

Local audio directory: `/home/leah/LibGibson/target/humanmusic-r16/final/`.
The 84 WAVs remain local build artifacts; source, reproduction tools, text receipts and
[all 84 exact hashes](fixtures/humanmusic-r16/final/SHA256SUMS) are committed.

| Full mix | SHA256 |
|---|---|
| `black_ice_r14.full.wav` | `cd056dbe7fb8fa0c4fe82c8f8463003ea9d48cc787f15c66a8519034d1a43548` |
| `black_ice_r15.full.wav` | `4bd2dd4625db86b0f0549a262a849735410a4095cbd2c74bcd40feb059ce1516` |
| `black_ice_r16.full.wav` | `5a50584074e48374495624566969bfcb3f8c884795c83d4f2fb6182f6eee8c3a` |
| `swiss_r14.full.wav` | `5b56aea38500c2471e71b45aa438510a47c8081e87ba54909c6f967959cfc792` |
| `swiss_r15.full.wav` | `5b56aea38500c2471e71b45aa438510a47c8081e87ba54909c6f967959cfc792` |
| `swiss_r16.full.wav` | `833fb91981b6afed7b596c6dec2ce6cc0096e17dc0f8f5f646c3d92b2f90e17e` |

BLACK stems use `{black_ice_r14,black_ice_r15,black_ice_r16}.{lead,bass,keys,pad,lead_bass,support,rhythm}.wav`.
The first Swiss comparison window is `swiss_r{14,15,16}.b028.{full,pad,support}.wav`,
13.220339–17.288136 seconds. Homologues use `b036` and `b044`; the within-song control
uses `b060`. Each filename is enumerated in the hash manifest.

Each arm gets full, lead, bass, keys, pad, lead+bass, support (pad+keys+bass), and rhythm
(keys+bass+drums) renders. SWISS windows at beats28/36/44 and control60 include two beats
of lead-in and six following beats. They are cropped from continuously rendered audio,
so incoming voices and reverb tails survive. WAV hashes are exact; separate stems traverse
the production bus independently and are not asserted to add linearly into the full mix.

Optional spectral measurements use mono PCM, Hann FFT8192/hop4096, spectral centroid,
high-band power and nearby-peak crosspower. They are neither a psychoacoustic roughness
model nor a quality score. Cropping/measurement is an external artifact tool; synthesis
remains Rust-only.

At the beat-28 pad window, R15→R16 centroid falls 573.585666→490.103408 Hz, but nearby-peak
crosspower **increases 0.009231→0.175474**. The full-band proxy also increases
0.385432→0.395592. These measurements do not corroborate a roughness reduction. The
candidate is motivated by register/inversion and adjacent voice path, and its lower-register
contacts remain an audition risk. All raw measurements are retained in
[the spectral receipt](fixtures/humanmusic-r16/final/spectral.json).

## Remaining boundary

No portamento or legato was implemented: all these notes still attack real synth voices.
No default-arm promotion, SongMap rewrite, release, merge, version bump, ABI change or
new emotion model. Good machine results cannot establish good licks or a good chord.

**BLACK_ICE:** does this now sound like a musician phrasing coherent syncopated lines,
rather than either slowly playing wrong notes (R14) or stuttering late graces into every
destination (R15)?

**SWISS:** is the recurring ugly Cmaj7 sonority actually gone without damaging the
otherwise-good Swiss arrangement?
