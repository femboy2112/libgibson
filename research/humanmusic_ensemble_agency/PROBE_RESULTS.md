# C137 ensemble-energy diagnostic — executed scope and next live experiment

**Pinned audit:** \`feat/humanmusic-c137-semantic-arc@aceed7e690acaf7a9fb00f01c4a6396bcce23e56\`. Research branch contains NO Rust source or sound/default changes. \`research/humanmusic_ensemble_agency/energy_probe.py\` is the exact byte sequence tested locally and published through GitHub connector.

## Actual executed evidence (2026-10-11 ET)

\`\`\`text
python /mnt/data/c137_energy_probe.py --self-test
# 5 tests OK, 0 failures, 0 errors (synthetic controls)

python -m py_compile /mnt/data/c137_energy_probe.py
# exit 0

sha256sum /mnt/data/c137_energy_probe.py
# d44a2e6e73acbdf879bba3386e311b4e1fd703d5b62d4fda4761bfeedea4f4af
\`\`\`

The GitHub connector's final content blob SHA-1 is \`02b9e15b2b7158f158b203a13adb5de6a8f46ace\`, **exactly equal** to the locally computed Git blob hash over the tested bytes (9,996 UTF-8 bytes). Source/commit provenance is not fabricated.

| Synthetic test | Executed result | What it actually validates |
| --- | --- | --- |
| Parse the actual \`fusion_gates\` T/N/D/C TSV row schema | PASS | parser interprets tempos, pitched note events and kit hits; no provenance/action graph |
| Copy one common energy envelope onto all roles | PASS | simple zero-lag attack correlation correctly reports 1.0 |
| Stagger lead attacks and keys responses one beat later | PASS | negative same-time correlation and fully observed lead drop→keys entrance |
| Include genuinely silent written intervals | PASS | independent Score-activity proxy distinguishes some all-silent bins |
| Reject malformed event/TSV/invalid bin sizes | PASS | parser refuses missing timeline, off-end notes, unusable resolution |

**Not executed here:** Rust \`cargo\`, real \`fusion_gates\` Score TSV, \`teacher_golden7\` WAV or Score, rendered PCM, production-toggle A/B, real-device listening, or a causal source mutation run. A self-test pass establishes diagnostic arithmetic on synthetic inputs, **not** that the existing band's energies are numerically averaged or that any candidate is musically superior.

## Run live on the next implementation branch

At the latest accepted PR #100 head or a new non-main integration branch from it:

\`\`\`bash
mkdir -p target/c137-agency/off target/c137-agency/on
cargo run --release --example fusion_gates -- \
    --seeds=2112,7,99 --beats=128 --tempo=144 \
    --dump-tsv=target/c137-agency/off --wav=target/c137-agency/off

cargo run --release --example fusion_gates -- \
    --seeds=2112,7,99 --beats=128 --tempo=144 --counterpoint \
    --dump-tsv=target/c137-agency/on --wav=target/c137-agency/on

python3 research/humanmusic_ensemble_agency/energy_probe.py --self-test

python3 research/humanmusic_ensemble_agency/energy_probe.py \
  --tsv target/c137-agency/off/fusion_2112.tsv \
  --out target/c137-agency/off/energy.json
python3 research/humanmusic_ensemble_agency/energy_probe.py \
  --tsv target/c137-agency/on/fusion_2112.tsv \
  --out target/c137-agency/on/energy.json
\`\`\`

Do not interpret \`R1–R9\` or weave gate PASS as proof of responsive ensemble energies. Pairwise Pearson and lagged correlation on Note onset/velocity are **observational proxies**, not effects of cues, PCM envelope, player agency or listener-recognized interaction. Constant role lanes can make correlation undefined (\`null\`), correctly not \`1\`.

To test what actually caused each musician to alter their playing, mutate ONE legitimate source-level cue while holding global song/tempo/harmony/seed fixed, recompute the \`PerformancePlan\` and compare authorized changed event windows (and unrelated invariant windows). Implement before asserting causal correlation.

For sound, render the EXACT SAME Score under NORMAL and diagnostic \`nocomp\`/\`dry\` \`ProductionControl\` policies (limiter and other release tails still exist), with appropriate stem-only and full-mix versions. Find whether apparent leveling originates in the Score, role coupling, or shared DSP.

The maintainer remains the sole artistic acceptance authority: current level is **nearly C137, but too cohesive in ensemble energy**. The accepted \`teacher_golden7\` render set's actual audio/metadata were not accessible here and must be pinned from the maintainer's local environment if available.
