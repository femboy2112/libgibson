# C137 performance-kernel seam probe: executed evidence

**Audit source:** feat/humanmusic-c137-ensemble-agency SHA 29527a891ed7bfed9657a99046884a61670897e3. **Status:** RESEARCH-ONLY. The new Python program is a *transcribed minimal diagnostic*, NOT actual Rust/Cargo execution, a Score render, or a musical listening test.

- Program: [seam_probe.py](seam_probe.py).
- Root audit: [HUMANMUSIC_C137_PERFORMANCE_KERNEL_ROAST.md](../../docs/research/HUMANMUSIC_C137_PERFORMANCE_KERNEL_ROAST.md).

## Executed local checks

Python code was written and run before uploading; GitHub's final file blob SHA was verified to match the code that executed.

    python /mnt/data/c137_architectural_audit/seam_probe.py --out /mnt/data/c137_architectural_audit/result.json
    # exit 0: 5 synthetic tests, zero failures, zero errors

    python -m py_compile /mnt/data/c137_architectural_audit/seam_probe.py
    # exit 0

    python /mnt/data/c137_architectural_audit/seam_probe.py --repo /mnt/data/c137_architectural_audit
    # exit 2: intentional missing-checkout negative control, SOURCE_DRIFT_REVIEW_REQUIRED

    SHA-256: dba255a6f3cbce498c1fd0d0e4ad17758874b5f094a01a1b45f3a406701c3635
    Git blob SHA-1: f41273c5327b4574a77f3582e1b557ce61d5c1ea
    UTF-8 bytes: 6,935

Five synthetic tests:
1. Old hearing projection cannot distinguish velocity changed from 0.4 to 0.95, while a richer candidate projection can.
2. Old projection CAN distinguish inserted note events **when explicitly evaluated**.
3. Two different source meanings with identical coarsely observed energy/onset/duration remain indistinguishable under that coarse view.
4. No-op and genuinely retimed events have different observational consequences.
5. Missing full checkout refuses source validation rather than reporting false VERIFIED status.

**Separate connector-backed PINNED SOURCE VERIFICATION EXECUTED:** The GitHub connector fetched six current Rust source files at this exact commit and confirmed all six Git blob SHAs matched the script's published pin set. Ten targeted structural text assertions were then evaluated in JavaScript against those full fetched source files:

- Old Hearing projection omits note velocity.
- Drums' Keys Hearing occurs before the optional counterweave.
- Final shared backing gain occurs after the Keys Hearing.
- Argument harmony preparation occurs after the generic PerformancePlan assembly.
- Argument drive=true is assigned after the generic PerformancePlan assembly.
- BASS_WEAVE is false in the latest source.
- Keys Listen is chosen from lead onset-count thresholds.
- Fusion bass drive admits fixed eighth-grid steps.
- admit_take only checks orchestration violations, not a comprehensive dynamic/hearing observer.
- Earlier CoupledR8 is retained as an ear-rejected negative control.

**ALL TEN PINNED STRUCTURAL ASSERTIONS RETURNED TRUE.** This verifies narrow facts about the source and compiler chronology, not the production semantics of Rust execution, the magnitude of the acoustic defect, or that a proposed refactor improves music.

To rerun the Python source-guarded check in Astra's real checkout of this exact SHA:

    python3 research/c137_performance_kernel/seam_probe.py --repo . --out /tmp/c137-kernel-seams.json

On source edits it intentionally refuses to claim an unchanged pin (returns exit 2). Update its source pins only after inspecting the changed code and reconciling its proof claims; do not just replace hashes to force green.

## Not executed in this audit

No cargo build/test, actual experimental Score::stale_hearings, WAV export, teacher_golden7 file examination, human ear A/B or default PCM hashing were performed here. A PR #102 earlier baseline CI was green, but exact-head latest CI was still pending at initial audit; refresh before implementation.

**Next decisive Rust tasks:** run Score::stale_hearings on fusion_gates with and without counterpoint; exercise a velocity-only post-hearing mutation; compare dependent planning before/after argument harmonic preparation; then implement one opt-in causal four-bar band episode before attempting generalized musician dynamics or reviving bass weave.
