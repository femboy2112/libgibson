# C137 semantic density — source-order probe receipts

**When:** 2026-10-10 user-local, source head \`2465ad06a5424d77ca92f14fffbdab568000c540\`.
**Scope:** independently coded Python reconstruction of PINNED Rust source control flow and fixed \`deflected_lift_trace\` event fractions. **NOT Rust execution, generated Score, PCM hearing, or a proof of all semantics.**

Probe file: [\`form_order_probe.py\`](form_order_probe.py).
Architecture and planned Rust proof: [\`HUMANMUSIC_C137_SEMANTIC_DENSITY_CLOSURE.md\`](../../docs/research/HUMANMUSIC_C137_SEMANTIC_DENSITY_CLOSURE.md).

## Executed local commands

\`\`\`bash
python /mnt/data/c137_form_order_probe.py \
  --out /mnt/data/c137_form_order_probe_results.json
# exit 0, Ran 5 tests, OK

python /mnt/data/c137_form_order_probe.py \
  --repo /mnt/data \
  --out /mnt/data/c137_form_order_probe_negative.json
# exit 2, correct negative control: source files missing
\`\`\`

Successful equation-only output says \`NOT_RUN_NO_CHECKOUT\`; the source-verification status is NOT falsely \`PINNED_MATCH\`. For source-linked verification, use a real \`git\` checkout at the pinned branch:

\`\`\`bash
python3 research/c137_semantic_density/form_order_probe.py --repo . \
  --out /tmp/c137_form_order.json
\`\`\`

The source pin compares the Git **blob** SHA-1 of each of:
\`argument.rs\`, \`semantic.rs\`, \`plan.rs\`, \`timeline.rs\`. A later source edit intentionally returns \`SOURCE_DRIFT_REVIEW_REQUIRED\` until the relevant code is re-audited. The independent Rust regression should not copy/assume the transcribed rule; it should invoke the actual \`FormGraph\`, \`MusicalArgument::fusion\`, source compiler and Score witness.

## Isolated test results

| Test | Result |
| --- | --- |
| At 128 beats the FormGraph has six full 4-bar phrases followed by four 2-bar terminal phrases | PASS |
| At 128 beats the source's last full-length Return is at beat 80 and final four Depart are at 96,104,112,120 | PASS (detects BAD existing behavior) |
| At 160 beats a final Return occurs after the Depart(s) | PASS (positive counterexample to an overgeneralized claim) |
| 96, 112, 120, 128, 144 beats exhibit terminal Depart under the pinned rule | PASS (finite tested set, not all lengths) |
| A counterfactual repositioned Return changes the order witness | PASS (conceptual; does NOT itself prove score capacity) |

\`\`\`text
test_128_form_is_ten_phrases_with_four_short_terminal_phrases ... ok
test_128_return_precedes_bridge_and_final_phrase_is_departure ... ok
test_160_fixture_differs_and_can_mask_default_failure ... ok
test_counterfactual_ordering_is_observably_distinct ... ok
test_failure_extends_beyond_one_seed_or_one_length ... ok

Ran 5 tests in 0.001s
OK
\`\`\`

The key 128-beat reconstructed sequence:

\`\`\`text
0 Establish(verse)
16 Consequent(hook)
32 Establish(verse)
48 Consequent(hook)
64 Establish(verse)
80 Return(hook)
96 Depart(bridge)
104 Depart(bridge)
112 Depart(bridge)
120 Depart(bridge)  ← last event: departed, not returned
\`\`\`

**Specific source flaw:** \`fusion\` selects a Return on its **last full-length phrase** and designates **every shorter phrase** a Depart. Form segmentation late in the piece makes length and narrative phase disagree. Moreover, the 8-bar bridge chord progression restarts from index 0 within each separate two-bar segment, so the harmonic route itself forgets where it was in the bridge.

**Non-vacuous semantic closure needed:** \`ArgumentEnding::Resolved\` currently only refuses unmatched Question debts. Fusion A/B does not necessarily generate Question/Answer at all. Add a *source-linked departure/return debt* for the program that actually promises one; do not outlaw a deliberately open piece or reinterpret a plain consequent as a literal Answer unless its source contract earns that name.

The code can be CI-green and source-witness green while this higher-order program is semantically incomplete, because the existing witnesses do not claim to validate that order. Add the test at the correct authority boundary.

**No auditory test performed:** \`teacher_golden\` accepted set not available as GitHub-tracked WAVs. The teacher source in \`examples/rick_probe.rs\` remains available. Use precise filenames and SHA256 if the maintainer's real local \`teacher_golden\` directory is accessible. Do not infer aesthetic acceptance from mathematical correctness.
