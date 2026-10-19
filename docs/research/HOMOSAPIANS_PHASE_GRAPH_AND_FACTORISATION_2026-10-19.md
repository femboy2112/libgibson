# Homosapians: Categorical Phase Geometry and Factorisation Witnesses
## A small rigorous mathematical tool for making the ethos *operational*

**Design memo on PR #124, Oct 19 2026. This is a proposed finite model; it is NOT currently implemented or a proved statement about perceived musical quality.**

The overarching problem is not that one Keys onset is "off"; it is that the source says the band has one pocket while the actual Score is produced by several independently authored time maps, some musical and some acoustic, and our witnesses do not connect them.

## I. Two-component timing is an additive group with a tempo-dependent evaluation

A correction that means **musical anticipation** should scale with beat duration. A correction for **physical latency** of an oscillator/envelope/chorus should have approximately tempo-invariant duration in seconds.

Define a typed displacement space:

    D = Beats × Seconds
    d = (delta_metric, delta_physical)
    d1 + d2 = (metric1+metric2, physical1+physical2)

For fixed tempo beta > 0 beats/minute, define the evaluation homomorphism:

    E_beta(delta_metric, delta_physical)
        = (60/beta) * delta_metric + delta_physical  [seconds]

Composition respects addition. A performance pocket is a metric-coordinate delta; measured patch latency is a physical-coordinate delta. An **opt-in** accompaniment can carry both with distinct source IDs and musical reasons. Current KEYS_ATTACK_COMP=-0.03 BEATS has been treated as a physical correction, although its absolute time scales from 22.50ms (80BPM) to 12.86ms (140BPM) and is 17.31ms at 104 BPM.

The physical-latency model is separately measured and may have envelope-dependent uncertainty. This type system prevents applying a raw "-0.03" offset without saying whether it means a musical beat fraction or a physical second amount. Never convert at multiple inconsistent boundaries.

A naturality check across tempo is then finite and falsifiable: preserve metric source identity and musical phase in beats, transform with E_beta once, and confirm measured source-to-acoustic onset differences have the predicted tempo scaling. Dry/wet timbral changes legitimately change measured physical latency, but only with a new AcousticLatency receipt. Do not call a numerical linear relation a deep categorical theorem without its actual domains and maps.

## II. The phase interaction graph and the exact cycle-consistency lemma

For a given source MetricAnchor (e.g. bar 2 downbeat), make a finite graph with vertices representing active instrumental gestures, not just Role labels: KeysComp, KeysCarrier, BassFoundation, Kick, Snare, Hats, and later a particular guitar strum. An edge u→v specifies the **intended** signed delay d_uv in typed space D.

Question: does there exist a consistent assignment of relative times t_v such that

    t_v - t_u = d_uv       on EVERY declared edge?

**Elementary lemma.** For a finite connected graph, a solution exists iff the oriented sum of d along every closed cycle is zero. If solutions exist, they are unique up to a common additive offset (one global time-origin choice).

**Proof outline.** Necessity telescopes around a cycle. For sufficiency, choose a root r and spanning tree, set t_r=0, and define each t_v as the sum of edge displacements along the unique tree path. A non-tree edge agrees with these times iff its fundamental cycle sums to zero. Fundamental cycles generate all cycles. Another solution differs by a common constant because differences vanish on every connected edge.

For inconsistent source requests, return a typed **phase relation refusal** with the minimum conflicting cycle and its residual, rather than silently changing the guitarist's riff or adding arbitrary jitter. For underconstrained requests (missing edges), preserve freedom explicitly; do not invent "everyone must be coincident." Hats serving as a grid reference while snare intentionally lays back is consistent when their delay relation is declared rather than hardcoded as a lie about strict simultaneity.

The current Score's measured phase edges may be consistent mathematically as offsets (all actual times exist), while violating the SOURCE graph's desired Lock edges. Test **intent-vs-observed residuals**, not only whether observed times can be assigned (which is tautologically true).

A useful diagnostic receipt:

    PhaseCycleReceipt {
        metric_anchor_id, edges_with_source_reason,
        prescribed_cycle_sum, measured_relative_delays,
        max_residual, refused_or_admitted, source_sha,
        score_fingerprint, acoustic_measurement_confidence
    }

Do not pretend this scalar determines groove quality: only contradictory declared constraints and false claims are rejected.

## III. Musical relations as a finite groupoid, not an adjective

A bounded phase morphism can be labelled Lock, Anticipate, Behind, Withhold, or Independent. Composition is partial: not every pair of gestures has a declared musical relation. Where two relations compose, signed delays must compose consistently in D; inverses negate offsets for relational description, without implying retrocausal performance.

The intended source-pulse and the actual event streams make a commuting diagram up to explicit acoustic-latency measurement:

    AuthoredRiff ──arrange/perform──> ScoreEvents
         |                               |
    pocket expectation             sample scheduling
         |                               |
         v                               v
    IntendedAttack ──patch-latency──> ObservedAttack

The square has genuine teeth only if both paths refer to the SAME source gesture identity and the same musical time; if one side reads a different bar chord or a stale prior draft, that is not a commuting witness.

A user wants the guitarist to push a sixteenth early? That belongs in the AUTHORED revision and may change metric phase. A sampled FM patch speaks 3ms late? That is a physical realizer issue; it must not silently rewrite the chord into the previous bar, without authorization.

## IV. Factorisation exposes inert agency before it becomes an aesthetic argument

The freeze operation presently reads the riff and bass groove and emits pop_funk_kit. It does **not** read the mutable DrumResponse's reinforce_push, leave_gap or fill choices.

Holding other draft fields fixed:

    freeze(riff, bass, drums1) == freeze(riff, bass, drums2)

for lawful distinct drum decisions. So, over this finite domain, the drum realization **factors through the projection that forgets DrumResponse**. This does not mean the drummer is completely absent—the kit uses bass-derived groove; nor does it negate the guitarist using an earlier DrumResponse to revise the riff. It means a precise extra arrow "this specific discretionary DrumResponse changes the emitted kit" is false.

A required upgrade to that claim:
- Construct two lawful DrumResponse variants with identical riff/harmony/bass/source except one documented discretionary choice;
- freeze them to Score, preserving source and beat spine;
- inspect whether their performed discretionary DrumHit events or explicit withhold receipts differ;
- without a change, report the decision as INERT, not "heard and performed."

This avoids requiring a general faithful functor from all intention to PCM (impossible: different gestures may yield perceptually indistinguishable sound). The obligation is **finite, claim-specific discriminability** with actual emission, including a meaningful silence as an allowed effect.

## V. Source-to-comp is a many-to-one ownership *projection*

One riff attack can cause two or three chordal Keys notes. That is not an illegal invention if the source owns a CompGesture and each emitted chord tone projects to that exact gesture. A future sidecar should record a typed relation:

    CompNoteEvent --owner--> CompGesture --derivedFrom--> RiffOnset

It should certify the **multiplicity fiber** (e.g. guide-tone dyad, optional 9th), the declared voicing law, harmonic context, metric time and allowed relative phase, not just the vague fact that any Keys note is close to a riff onset. Required Keys carrier and comp are distinct event families that may share a role but should NOT acquire independent unlicensed pocket offsets.

The existing argument witness retains exact carrier material by excluding role_note="comp" notes from the carrier window. That is a useful historical-compatible seam, but independent CompGesture projection is missing. Audit fake/extra/moved comp onsets with concrete mutation tests.

## VI. Concrete tests the next Claude round should earn

1. **P1 Score-level**: source-locked KeysComp vs Bass/Kick shows -0.03 beat difference in the current baseline; a future lock witness rejects this unless it is explicitly source-licensed or physically corrected at a documented DSP boundary.
2. **P2 bar crossing**: t=4.0 intended chord must not become t=3.97 next-harmony without a source-owned anticipation or typed physical pre-roll. Admit one deliberately authored pickup as a POSITIVE control.
3. **P3 Keys self-flam**: when source carrier and comp share a metric slot, measure scheduled/dry/wet timing difference, and test a lawful derived single timing relation.
4. **P4 inactive drummer**: mutating only DrumResponse currently has no direct frozen Score effect; after adding DrumGesturePlan, different real legal choices affect actual drum events/silence.
5. **P5 tempo naturality**: same source and physical patch at 80,104,140 BPM, 44.1/48 kHz. Metric displacement scales in physical seconds; physical latency is estimated/calibrated in seconds. Failing this is a typed-unit bug, not a reason to copy and paste one different compensation constant per tempo.
6. **P6 cycle consistency**: synthesize a deliberately contradictory triad of source phase edges (Keys locks Bass, Bass locks Kick, Keys deliberately behind Kick without other declared differences), detect nonzero cycle residual; lawful triangle passes; underconstrained graph remains underconstrained.
7. **P7 original C137 lock**: C137_ACCEPTED_V1 51 references remain byte-for-byte untouched whenever this sidecar is OFF. The first implementation must be entirely additive / opt-in.
8. **P8 auditory interpretation**: same Score notes/chords/riff + phase/audio production 2x2 and human ear. Data establishes attack position, not catchiness or aesthetic superiority.

This is the narrow mathematical advance requested: **use categorical structure to prevent false musical claims from surviving translation into actual events**. Stop asking only whether a source can be performed; ask whether its declared inter-musician relations commute with the independent observed Score and PCM—and refuse unsupported assertions.

No mathematical result here is a new general theorem. The cycle lemma is elementary; its nontrivial value is choosing the CORRECT source objects, preserving typed physical/musical time and turning it into executable hostile tests in the real band system.
