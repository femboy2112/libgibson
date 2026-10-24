# HumanMusic auditory observer: perceptual realization architecture

**Status:** design / future integration; no implementation claim  
**Branch:** `research/humanmusic-auditory-observer`  
**Base:** `main@80706c91b88e59495b131bfd8ab23aa3ed013414`  
**Scope:** HumanMusic proper. Reverse HumanMusic may reuse this layer later, but is deliberately out of scope here.

## 0. Why this document exists

HumanMusic currently distinguishes increasingly sophisticated musical objects upstream of synthesis: semantic intent, SongMap identity, performance action, heard-object and temporal-pitch constraints, source/observer evidence, and the final `Score`. It then realizes the score through `HumanMusicSynth` into PCM.

That architecture still leaves a missing boundary:

```text
written music              physical sound               heard music
Score / performance   !=   rendered PCM            !=   auditory percept
```

The first distinction is already load-bearing in HumanMusic. The second is not yet modeled as a first-class object.

The working hypothesis behind this note is that this omission can explain a family of failures where the score, pitch law, timing, and even gross spectrum are locally defensible while the result still sounds robotic, oddly exposed, masked, brittle, or instrumentally unconvincing. In particular, low-energy or "noise-like" structure may carry disproportionate perceptual information through attack transients, stochastic excitation, temporal fine structure, masking relations, and auditory grouping.

This is a **hypothesis and architecture proposal**, not a claim that those historical failures have already been explained.

The design principle is therefore:

> HumanMusic should eventually optimize the physical signal required to induce the intended auditory object, not merely produce a numerically plausible waveform from a correct score.

The first integration must be observer-only. No synthesis or mix decisions should be changed until the observer survives discriminating controls.

---

## 1. Claim boundary

### Supported background

Human frequency selectivity is not well represented by uniformly spaced FFT bins. Equivalent rectangular bandwidth (ERB) models summarize frequency-dependent auditory-filter bandwidths, and ERB-rate gives a useful engineering coordinate for a first reference filterbank.

Complex band-limited sounds can be separated into slower temporal envelopes and faster temporal fine structure (TFS). TFS contributes to pitch and masking phenomena and should not be silently discarded when the objective is musical perception.

Equal physical level does not imply equal perceived loudness across frequency. ISO 226:2023 specifies equal-loudness-level contours for pure continuous tones under tightly specified free-field, binaural conditions. It is a calibration/control source, **not** a universal music loudness transform.

Auditory grouping is context-sensitive. Common onset and harmonic relations can cause spectrotemporally different components to be heard as one event. This makes a simple additive "analyze every stem independently, then sum percepts" model suspect.

### Not yet supported

We do **not** currently have:

- a calibrated HumanMusic listener model;
- a validated perceptual distance for arbitrary music;
- a lawful scalar "quality", "human-ness", "pleasantness", or affect score;
- an absolute SPL mapping from HumanMusic's dimensionless PCM;
- a measured transfer function from PCM samples through a playback device, room, head/torso, outer ear, and middle ear;
- evidence that one reference listener model predicts the maintainer's judgments;
- evidence that the proposed architecture explains any particular old HumanMusic defect.

Therefore every early `AuditoryModel` result is a **diagnostic proxy** until human A/Bs and mutation controls establish what it can discriminate.

---

## 2. Corrected pipeline

Current coarse path:

```text
SemanticTrace
  -> SongMap
  -> PerformancePlan
  -> Score
  -> HumanMusicSynth
  -> PCM
```

Proposed path:

```text
SemanticTrace
  -> SongMap
  -> PerformancePlan
  -> Score
  -> HumanMusicSynth
  -> PCM
       |
       +---------------------> physical/render diagnostics + WAV
       |
       v
  AuditoryModel A_c
       |
       v
  AuditoryTrace
       |
       +--> generic auditory diagnostics
       |
       v
  HumanMusic audition
       |
       v
  PerceptualReceipt
```

The context-indexed auditory observer is written

[
\mathcal A_c : \mathbf{AcousticScene} \to \mathbf{AuditoryScene},
]

where `c` is an explicit listening context, not hidden global state.

The separation is intentional:

- `Score` says what the band performed.
- PCM says what physical sample stream the synthesizer produced.
- `AuditoryTrace` says what a declared auditory model exposes.
- `PerceptualReceipt` says how that exposure relates to HumanMusic-specific contracts.

No downstream observer is allowed to rewrite upstream history.

---

## 3. Placement in the codebase

Low-level auditory analysis should be reusable outside HumanMusic:

```text
src/audio/perception/
    mod.rs
    listener.rs
    filterbank.rs
    loudness.rs
    temporal.rs
    onset.rs
    masking.rs
    scene.rs
    distance.rs
```

HumanMusic-specific interpretation belongs separately:

```text
src/audio/human_music/
    audition.rs
    perceptual_contract.rs
    perceptual_diagnostics.rs
```

### Boundary law

`audio::perception` must know nothing about:

- chords;
- SongMap;
- bass/lead/keys/pad roles;
- motifs;
- musical grammar;
- "good" music;
- HumanMusic worlds.

It consumes acoustic samples plus declared listening context and produces inspectable auditory evidence.

`human_music::audition` may then interpret that evidence against known score/performance provenance.

This prevents the observer from "passing" because it read the answer key.

---

## 4. Listening context is mandatory

HumanMusic PCM is nominally dimensionless `f32` audio. A value such as `0.2` does not imply a sound-pressure level. Therefore absolute loudness models cannot be used honestly without calibration.

Represent that explicitly:

```rust
pub enum LevelCalibration {
    /// Relative comparisons only; no claim about absolute SPL or sones.
    Relative,

    /// Declared mapping from digital full scale to acoustic level.
    Calibrated {
        db_spl_at_dbfs: f32,
    },
}

pub struct ListeningContext {
    pub level: LevelCalibration,
    pub sample_rate_hz: u32,
    pub channel_model: ChannelModel,
    pub device_response: Option<FrequencyResponse>,
    pub listener: ListenerProfile,
}
```

Later contexts may add room/HRTF/device transforms. Version one should not pretend to possess them.

Any diagnostic requiring absolute SPL must return "undefined under this context" rather than fabricate a value.

---

## 5. First auditory representation: multiscale evidence

Do not replace one lossy scalar with another.

A first `AuditoryTrace` should preserve at least:

```rust
pub struct AuditoryBandTrace {
    pub center_hz: f32,

    // Physical witnesses.
    pub rms: TemporalSeries,
    pub peak: TemporalSeries,

    // Auditory-model witnesses.
    pub excitation: TemporalSeries,
    pub envelope: TemporalSeries,
    pub fine_structure: TemporalSeries,
    pub onset_salience: TemporalSeries,
    pub modulation: TemporalSeries,

    // Optional/undefined unless the listening context supports it.
    pub loudness: Option<TemporalSeries>,
}

pub struct AuditoryTrace {
    pub bands: Vec<AuditoryBandTrace>,
    pub global_onsets: Vec<AuditoryOnset>,
    pub calibration: AuditoryCalibrationReceipt,
}
```

The first filterbank should be ERB-spaced or a declared gammatone-like approximation. Exact model choice is an implementation question; the architectural requirement is that the coordinate system approximate auditory frequency selectivity and remain deterministic and inspectable.

### Never silently collapse

Keep these distinct:

[
E(x) \quad \text{physical energy}
]

[
X_c(x) \quad \text{auditory excitation}
]

[
L_c(x) \quad \text{modeled loudness, when defined}
]

[
S_c(x,t) \quad \text{onset/transient salience}.
]

A low-energy component is not permitted to be declared perceptually negligible merely because its RMS is small.

---

## 6. Structured residual is a first-class synthesis concept

A note should eventually be modeled more richly than

[
N = f_0 + \text{harmonics} + \text{ADSR}.
]

A more useful decomposition is

[
N = B + T + R + C,
]

where:

- `B`: sustained resonant/body component;
- `T`: attack and release transients;
- `R`: structured stochastic/residual component;
- `C`: couplings among excitation, resonator, articulation, and production.

"Residual" does **not** mean dispensable noise.

For a string-like source this may include:

```text
excitation
  pick/string collision
  scrape/friction trajectory
  initial broadband burst
  excitation-position dependence

resonator
  fundamental
  harmonic modes
  inharmonic modes
  body/cabinet coupling

performance evolution
  pitch drift
  damping
  motion/fret/string noise
  release event
```

This is a future synthesis target, not a round-one refactor. First build the observer capable of falsifying whether such structure matters.

---

## 7. Perceptual ablation: the load-bearing primitive

For a rendered scene `x` and component `a`, define diagnostic perceptual leverage

[
\Lambda_c(a;x)
=
D_A\!\left(
\mathcal A_c(x),
\mathcal A_c(x-a)
\right),
]

where `D_A` is an explicitly declared **diagnostic distance**, not a truth probability or quality score.

Compare it separately with physical energy share

[
\epsilon(a;x) = \frac{E(a)}{E(x)}.
]

A useful derived diagnostic is

[
R_c(a;x)
=
\frac{\Lambda_c(a;x)}
     {\epsilon(a;x)+\varepsilon},
]

but it must retain both raw terms. A high ratio means "large observer change per unit physical energy under this model", not "important to all human listeners."

### Band ablation

Let (Pi_k x) denote the content assigned to auditory band `k`. Then

[
\Lambda_{c,k}(x)
=
D_A\!\left(
\mathcal A_c(x),
\mathcal A_c(x-\Pi_kx)
\right)
]

produces a **perceptual-importance map**, which is categorically different from a power spectrum.

The question changes from

> how much energy is here?

to

> what changes in the modeled auditory scene if this structure is removed?

That is the central research object of this proposal.

---

## 8. Mixing is not perceptually additive

For acoustic mixture (otimes),

[
\mathcal A_c(X\otimes Y)
\neq
\mathcal A_c(X)\otimes\mathcal A_c(Y)
]

in general.

Masking, fusion, beating, roughness, common onset, spatial relations, and level-dependent behavior can make the perceptual mixture differ from an independent analysis of the stems.

A useful categorical shape is therefore **lax monoidal**, schematically:

[
\mathcal A_c(X)\otimes\mathcal A_c(Y)
\longrightarrow
\mathcal A_c(X\otimes Y).
]

This is not decorative category language. The comparison map is exactly where HumanMusic should inspect:

- masking;
- reinforcement;
- fusion;
- segregation;
- interaction-created roughness or beating;
- whether two nominally independent player events become one auditory object.

The existing stem and pair-stem infrastructure is unusually well suited to testing this.

For source `i` in a full mix `X`:

[
\Delta_i =
D_A(\mathcal A_c(X),\mathcal A_c(X-X_i)).
]

For a pair:

[
I_{ij}
=
\Delta_{ij} - \Delta_i - \Delta_j.
]

Keep (Delta_i), (Delta_j), and (Delta_{ij}) alongside `I_ij`. The mixed term is a diagnostic interaction, not a mechanism proof.

---

## 9. `PerceptualContract`: beside, not inside, `CoherenceContract`

HumanMusic should eventually be able to state auditory obligations without pretending they are compositional identity.

Possible shape:

```rust
pub struct PerceptualContract {
    pub foreground: ForegroundContract,
    pub masking: MaskingBudget,
    pub transient_identity: TransientContract,
    pub separation: SeparationContract,
    pub dynamic_exposure: DynamicContract,
    pub spatial: SpatialContract,
}
```

Diagnostics remain a vector, for example:

[
P = (
\text{audibility},
\text{masking},
\text{transient clarity},
\text{spectral separation},
\text{roughness},
\text{dynamic contrast},
\text{spatial distinctness}
).
]

There is deliberately **no weighted total** such as `human_music_quality = 0.87`.

The current `phenomenal.rs` should remain separate. It measures score/song affordances such as stability, propulsion, expectation, surprise, openness, and familiarity. Those are not substitutes for a rendered auditory observer.

---

## 10. Closed-loop synthesis comes later

### Phase 0 — observer only

```text
Score -> Synth -> PCM -> AuditoryModel -> receipts
```

No generation behavior changes.

### Phase 1 — mix assistance

Only after calibration, allow a constrained mixer to use auditory evidence to protect declared foreground or articulation cues. The objective is not equal RMS or equal peak; it is preservation of declared auditory exposure under mixture.

### Phase 2 — timbre assistance

When a source loses identity, the controller may choose among physically distinct interventions:

- transient structure;
- upper partial structure;
- stochastic excitation;
- modulation;
- stereo/spatial separation;
- gain.

Turning everything up must not be the universal repair.

### Phase 3 — gesture realization

Performance gestures should have explicit acoustic-to-auditory consequences. For example, an `Accent` should not mean only `gain *= 1.2`; it may alter attack, transient/body ratio, excitation spectrum, and duration in ways whose auditory consequences are inspectable.

### Phase 4 — perceptual planning

Only after the lower layers are validated should `PerceptualContract` constrain performance or synthesis planning upstream.

---

## 11. Minimum viable auditory observer

Do **not** attempt a complete ear/brain simulator.

Version one should have five independent witnesses:

1. an ERB-spaced auditory filterbank;
2. level/excitation handling with explicit calibration boundary;
3. onset/transient salience;
4. envelope and temporal-fine-structure preservation;
5. simultaneous masking / mixture-ablation diagnostics.

Park until those survive controls:

- forward/temporal masking;
- full loudness-standard implementation;
- roughness/beating models;
- general pitch salience;
- binaural localization and HRTFs;
- precedence effect;
- learned auditory-object grouping;
- personalized hearing profiles;
- any affect/pleasantness model.

---

## 12. First decisive HumanMusic experiment

Use one frozen `Score`, one `MusicWorld`, one seed, one sample rate, and one fixed production path. Change only the tested acoustic structure.

Render:

| Variant | Manipulation |
| --- | --- |
| A | original |
| B | suppress selected high-frequency stochastic/transient structure |
| C | B plus same-energy stationary or spectrally matched noise |
| D | preserve approximate long-term spectrum/energy while smearing transient timing |

The key comparison is not merely A vs B. It is B vs C and A vs D.

### Hypothesis H1

If the removed material matters only through gross spectral energy, restoring approximately the same band energy in C should substantially restore the relevant auditory diagnostics and human percept.

### Rival H2

If the detailed event-relative geometry of the residual is load-bearing, C should remain detectably unlike A even though much of the lost energy has been restored.

### Rival H3

If timing/onset geometry is load-bearing independently of long-term magnitude spectrum, D should remain detectably unlike A despite approximately matched long-term energy.

### Predeclare outcomes

- **Supports structured-residual hypothesis:** human A/B and independent auditory witnesses both distinguish A from C/D while energy controls are matched within declared tolerance.
- **Supports gross-energy rival:** C collapses toward A under human A/B and auditory witnesses.
- **Ambiguous:** observer and human judgments disagree, loudness/level matching is inadequate, manipulations alter multiple uncontrolled factors, or the effect is inconsistent across fresh renders.

No result from this one experiment establishes a universal model of hearing.

---

## 13. Controls

### Positive control

A deliberately large, clearly audible perturbation must produce a large observer difference.

### Negative control

A byte-identical or numerically negligible mutation below declared tolerance must not produce a large observer difference.

### Null / nuisance controls

Test:

- global gain with level-normalized comparison;
- channel swap when using a monaural observer;
- time shift with shift-invariant comparison;
- phase perturbations known to preserve the chosen observable;
- seed-stable repeated analysis.

### Mutation controls

Break one feature at a time:

- remove attack transient;
- replace structured residual with stationary noise;
- smear onset;
- remove a narrow auditory band;
- add a masker while preserving source stem;
- alter only a source's RMS.

### Cross-implementation control

Before making architectural decisions from the observer, recover at least one known psychoacoustic result with an independent or reference implementation. Same-model agreement is not independent validation.

---

## 14. Engineering acceptance criteria for the first implementation round

An initial integration is acceptable only if:

- `main` behavior remains byte/fingerprint stable when the observer is disabled;
- observer analysis is deterministic for fixed PCM + context;
- no absolute loudness claim is emitted under `LevelCalibration::Relative`;
- filterbank reconstruction/energy bookkeeping has declared tolerances;
- envelope and TFS witnesses are stored separately;
- perceptual diagnostics retain raw physical witnesses;
- stems and pair-stems can be analyzed without recompiling a different synthesis path;
- at least one positive, negative, null, and mutation control is executable;
- the A/B experiment above can emit machine-readable receipts and WAVs;
- diagnostics do not read `Score` labels unless they are explicitly in the HumanMusic interpretation layer;
- no single scalar "quality" score is introduced;
- docs distinguish **observed**, **corroborated**, **conjectured**, and **UNVERIFIED** claims.

---

## 15. Relation to existing HumanMusic work

This proposal should compose with, not overwrite, the current architecture.

### `phenomenal.rs`

Keep it score/song-level. Its existing warning that written propulsion is not rendered groove continuity becomes even more important.

### Temporal mass / sounding tension / heard objects

These are conceptual predecessors because they already reject zero-duration or purely symbolic interpretations of sounding music. The auditory observer is the next boundary: it asks whether physically present material is actually exposed, masked, fused, or salient under a declared listener model.

### `HumanMusicSynth`

Round one does not change synthesis. It merely exposes deterministic PCM to `audio::perception`.

### Stem and pair-stem laboratory

Reuse aggressively. They provide the right intervention surface for source ablation and mixture interaction.

### Production controls / harmonic references

Reuse as nuisance-factor controls. Auditory experiments must not accidentally compare different scores or different production laws unless that factor is the declared variable.

### Reverse HumanMusic

Park it. Later, Reverse HumanMusic may use `AuditoryTrace` as an additional inverse-model representation. That future use must not distort the forward observer's API today.

---

## 16. Categorical interpretation

The minimal useful categorical picture is:

[
\mathbf{Performance}
\xrightarrow{R}
\mathbf{AcousticScene}
\xrightarrow{\mathcal A_c}
\mathbf{AuditoryScene}.
]

`R` is physical realization. (mathcal A_c) is a context-indexed observer.

Define contextual perceptual equivalence only relative to a declared diagnostic family:

[
x \sim_{c,D} y
\iff
D_A(\mathcal A_c(x),\mathcal A_c(y)) \le \tau.
]

This is **toolkit-relative indistinguishability**, not ontological identity.

The useful structure is not that (mathcal A_c) is magically a strict functor preserving every mixture relation. It is that failures of strict additivity are measurable and can be attached to explicit mixture/comparison morphisms.

This provides a disciplined location for masking and fusion instead of treating them as unexplained errors in the score.

---

## 17. Claim ledger

### Observed

- HumanMusic already has deterministic offline rendering, stems/pair-stems, production ablations, fixed-score controls, witness diagnostics, and listening fixtures suitable for controlled auditory experiments.
- `phenomenal.rs` explicitly limits itself to written/song evidence and does not certify rendered perception.
- HumanMusic PCM is dimensionless and currently lacks an absolute SPL calibration.

### Corroborated background

- Auditory-filter bandwidth varies with center frequency; ERB/ERB-rate is an established engineering description of auditory frequency selectivity.
- Envelope and temporal fine structure are distinct post-filter temporal descriptions, and TFS contributes to pitch and masking behavior.
- Equal-loudness relations depend on frequency and level under specified conditions; physical amplitude alone is not perceived loudness.
- Common onset and harmonic relations are established auditory-grouping cues.

These are background constraints, not validation of the proposed implementation.

### Conjectured

- Some HumanMusic "robotic" or instrument-identity defects arise because high-frequency transient/residual structure is missing or mistimed.
- Perceptual ablation will be more informative than RMS/spectral energy alone for locating those defects.
- Mixture-aware auditory diagnostics will reveal source interactions hidden by solo-stem inspection.
- A listener-aware synthesis loop can eventually improve HumanMusic without hardcoding style-specific patches.

### UNVERIFIED

- the exact filterbank/model family to adopt;
- any perceptual distance `D_A`;
- thresholds/tolerances for HumanMusic material;
- correlation with maintainer listening judgments;
- whether the proposed structured-residual A/B produces the predicted result;
- whether closed-loop synthesis improves music rather than overfitting diagnostics.

---

## 18. Recommended integration sequence

1. **Observer skeleton:** module boundaries, `ListeningContext`, relative calibration, deterministic ERB-like filterbank, raw receipts.
2. **Temporal evidence:** envelope, TFS, onset salience; calibration tests on synthetic tones/transients.
3. **Ablation laboratory:** reuse frozen Score + stem infrastructure; implement physical-energy and observer-distance receipts side by side.
4. **Structured-residual A/B:** A/B/C/D experiment with predeclared controls and human listening.
5. **Masking interaction:** full mix minus source; pair interactions; complete factorial cells before claiming mixed effects.
6. **Reference-result recovery:** reproduce known psychoacoustic behaviors within the limited model boundary.
7. **Only then:** introduce `PerceptualContract` as an opt-in observer target.
8. **Later:** constrained mix/timbre/articulation feedback.
9. **Much later:** upstream perceptual planning or Reverse HumanMusic reuse.

The first four stages are sufficient to falsify the motivating hypothesis. Do not pre-pay the complexity of the later stages.

---

## 19. References / calibration sources

These sources constrain the engineering model; none is treated as a drop-in implementation specification.

- ISO 226:2023, *Acoustics — Normal equal-loudness-level contours*. https://www.iso.org/standard/83117.html
- Moore, B. C. J. & Glasberg, B. R. (1983), "Suggested formulae for calculating auditory-filter bandwidths and excitation patterns", *JASA* 74(3), 750–753. DOI: 10.1121/1.389861. https://pubmed.ncbi.nlm.nih.gov/6630731/
- Moore, B. C. J., Peters, R. W. & Glasberg, B. R. (1990), "Auditory filter shapes at low center frequencies", *JASA* 88(1), 132–140. DOI: 10.1121/1.399960. https://pubmed.ncbi.nlm.nih.gov/2380441/
- Moore, B. C. J. (2008), "The Role of Temporal Fine Structure Processing in Pitch Perception, Masking, and Speech Perception for Normal-Hearing and Hearing-Impaired People", *JARO* 9, 399–406. DOI: 10.1007/s10162-008-0143-x. https://pmc.ncbi.nlm.nih.gov/articles/PMC2580810/
- Siedenburg et al. (2026), "Auditory scene analysis in music: A synthetic review". https://pmc.ncbi.nlm.nih.gov/articles/PMC13429554/

---

## 20. One-line mission

> Build HumanMusic an ear before asking it to use the ear: first expose the difference between written music, physical audio, and modeled auditory experience; then prove which supposedly-small structures are actually load-bearing before allowing perception to steer synthesis.
