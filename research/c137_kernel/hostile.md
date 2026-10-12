# Independent hostile kernel audit

Baseline: `f042bb055c966fd79fab0823e72a53409bdaffad`, inspected in
`/home/leah/LibGibson.kernel-hostile` on branch `research/c137-kernel-hostile`.
The integrator owned all runtime changes. This scout added only targeted tests
and this audit record. The review shares the repository and requested architecture
with the integrator; it is an independent mutation pass, not a blind external or
acoustic evaluation.

## Predictions registered before execution

`tests/audio_c137_kernel_hostile.rs` asserts desired refusal rather than asserting
the presence of a bug. The baseline command is:

```sh
CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=/home/leah/LibGibson.kernel/target cargo test --test audio_c137_kernel_hostile -- --nocapture
```

The raw first execution is retained in `hostile_red.txt`. No full suite, rendered
audio, or listening acceptance is supplied by this scout.

The predicted failures preserve the actual lead/keys hearing windows while
mutating a downstream commitment or a redundant source description:

- Bass with all velocities zero still meets the existing bar-root predicate.
- A bass note relabelled as an unowned counterline, with its explicit continuity
  links consistently retargeted, still meets register and gate checks.
- The second bass note can move to Bb over Am, retaining its false `ChordTone`
  declaration; no active harmonic obligation checks that non-root duty.
- The same note can move to an arbitrary `+0.137` beat offset, retaining its
  final continuity, without violating the implemented lane check.
- Drum responses can be deleted, the entire kit can be deleted, an arbitrary
  off-phase reply can be appended, or all drum velocities can become zero.
  None changes the notes the drummer previously consumed.
- Score meter and final harmony can diverge from the frozen source base.
- An existing compiled return placement can claim opening phrase zero, while
  the source arc can have false time, a nonexistent phrase, or no spans.

Instrument controls predict refusal already at baseline for changed consumed
keys dynamics and for an extra keyboard attack in the explicitly withheld
third bar. A referent pitch-basis mutation additionally checks whether existing
source validation already closes that candidate.

## Executed baseline result

The focused command completed in 3m42s with exit status 101: **16 tests executed,
14 failed, 2 passed, 0 ignored**. Every predicted accepted mutation above was
actually admitted by the baseline kernel. The two passing controls were
`effective_hearing_rejects_dynamic_and_false_withholding_mutations` and
`source_pitch_basis_must_match_the_compiled_interpretation`. Pitch-basis lies
are therefore not an uncovered defect. The baseline has no demonstrated
velocity-blindness defect in its new effective hearing.

These are runtime counterexamples to the relative candidate-admission and
compiled-source consistency claims at this exact commit. They do not show that
the unmutated initial take contains those defects, or that its actual rendered
music sounds bad. The first ten failed mutants concern immutable performance
commitments and frozen base data. The other four concern source metadata that
must be refused before planning.

## Smallest categorical correction

The committed take is already immutable. The exported `Score` is a separate
candidate, so admission relative to this take can require preservation of its
exact committed musical events, continuity and common base. It need not invent
another set of bass/drum planning rules. A changed source must enter through
`perform` and make new commitments and observations.

This gives three concrete domains:

1. Source admission validates that arc, placement and actual statement data
   describe the same source object. Public mutable compiled metadata cannot
   silently become a second authority.
2. A role proposes its own finalized stream against the frozen base; hard
   source/stage/voice constraints admit it before anyone consumes it.
3. An immutable committed take supplies actual dependency windows. Any later
   exported change either preserves the exact commitment or is refused.

Checking only consumed inputs protects arrows' domains while permitting their
codomains to disappear. Exact commitment preservation is the missing identity
law for this API, not evidence that every alternative performance must equal
the current take. Separate fresh performances may legitimately differ.

The bounded kernel does not yet replace full fusion performance, establish a
general theory of bass agency, or prove musical quality. Those are outside this
review. Hard refusal of false source identity and broken commitments is inside
the claimed four-bar domain and must hold before its receipts are trusted.
