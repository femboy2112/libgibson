# v0.9.2 "clear SIGKILL 2" completion audit (2026-11-26)

Scope, deferrals, classification, consumer gate and name were accepted by the release coordinator
(2026-11-26). The tag needs a separate coordinator GO after the release gates below are met.

Candidate scope: PR #461, `stage::motion` (M5-A G1–G4), on `feat/m5-motion` off `29ca8de`
(v0.9.1 plus docs). Nothing else is proposed for 0.9.2.

| Item | Status | Evidence |
|---|---|---|
| M5-A G1 `FrameSequence`, G2 `LayerSlots`, G3 `CueSchedule`, G4 hidden-quiescence test (EXPERIMENTAL, additive) | **Done** | Merged as `4a86232` (head `5999b7f`: hosted CI run 45907340425 9/9 including the `stage_motion_kit` example build, Release preflight run 45907340415). See the breakdown below the table. |
| Continuous motion: sprite in-betweens (sub-cell offsets, crossfade; Leah ruling 17) | **Deferred to a later 0.9.x** | GOD's ruling at #63. `FrameSequence` is labelled the **keyframe layer** in the module docs and in the CHANGELOG (5999b7f). Packet scope was approved at GOD #63. It starts after M5-B Slice 1 qualifies. |
| Live-playback capture with frame-timing metrics | **Deferred to the next motion checkpoint** | Leah's verdict, binding on the next checkpoint (VIEWED.md). |
| #294 resize (GHOST) | **Not in 0.9.2** | Spec #458 has the probe plan only. ROADMAP:195 already documents the steer to fullscreen. |
| M5-B `figure` (CharacterRevision) | **Not in 0.9.2** | It is on its own branch, #462, behind the off-by-default `figure` feature. Slice 1 is not yet qualified. |
| #444 multi-line text area | **Deferred to 0.10.0** (unchanged) | It breaks `ui::Interaction` literals (Release Contract §1). |
| `max_frame_bytes` for the kitty frame plan, and the prompt editor | **Not in 0.9.2** (unchanged) | Not scheduled. |
| Release classification | **Patch** | It adds a new experimental module and one `pub mod motion;` line in `stage.rs`. No existing all-public struct or exhaustive enum changes. There is no C ABI change; `git diff 29ca8de <rc> -- abi include bindings` must be empty except the version. There is a new `[[example]]`, `stage_motion_kit`. |
| Release gates (exact-head CI including Release preflight and the `stage_motion_kit` example build) | **Pending** | Observed on the final release-prep head. #461 final head `5999b7f`: 9/9 green plus Release preflight. The prep head is observed separately. |
| Consumer gate | **Substituted**, as for 0.9.0 and 0.9.1 (coordinator ruling) | The change is additive and GHOST does not consume `stage::motion` yet. Hosted CI on the exact candidate plus the abi/include/bindings diff. GHOST's pin bump to 0.9.2 is gated at its next integration candidate. |

Evidence for the M5-A row:
- **Narrow gate GREEN.** Gate 2 at `3234d48`. Gate 3 at `e7d2896`: fmt, `stage_motion` 21/21, lib `stage` 3/3, clippy `-D warnings`, `+1.85` check.
- **RED, assertion-level.** Wrong-value stubs, committed under `docs/evidence/m5a/mutants/`:
  - `sample_frame0`: 6 FAIL;
  - `hash_zero`: 4 FAIL;
  - `nth_origin`: 9 FAIL;
  - `planes_flat`: 3 FAIL.
  Every failure is an assertion in `tests/`. GOD ruling #50 accepted the reading of the G3 structural tests.
- **Independent review** (fresh context; 3234d48 plus spec only): 0 blockers. The 2 should-fix items and 1 nit were fixed in `e7d2896` (G4 positive control, the real no-replay re-show).
- **Visual checkpoint.** The kit ran at `e424e09` with a real xterm. 15 stills plus the contact sheet were viewed by the session. Leah's verdict at about 22:50Z was "Pass, add live capture". V-M5A-VIS is closed at `f1ad72c`.

Post-gate commits on #461. Docs and evidence only, except for one example change:
- `85efbad`, `2ae253b`: evidence;
- `4d2c0b5`, `f6129b8`, `e424e09`: the kit;
- `f1ad72c`, `fa493b6`: the kit output, the verdict, and kit label fixes. The example change here (the `stage_motion_kit` labels) is **UNCOMPILED** until hosted CI.
- `5999b7f`: the keyframe-layer docs.
The library code (`src/stage/motion.rs`) has had only a doc change since the e7d2896 gate.

Stale ROADMAP text to fix in the release-prep PR:
- The "Current release" header.
- Add a v0.9.2 section plus this table.
- Mention `stage::motion` under the Living Stage experimental surfaces.

Tag preconditions (coordinator, 2026-11-26), all required before a tag GO:
1. Hosted CI green on the final prep head, all jobs, including Rust, Release preflight and the
   `stage_motion_kit` example build.
2. #461 merged to `main`, then this prep PR merged, both without force and on a coordinator GO.
3. This audit file committed in the prep PR.
4. The tag on the merged `main` commit, matching the `Cargo.toml` version (0.9.2).
