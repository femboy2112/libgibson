# v0.7.0 qualification plan

**Status: the plan as accepted.** Written 2026-11-13 from `main` `068e7d6` (draft #378) and
landed with the release preparation. The gate table below is the plan's starting point, not a
record of results. What actually closed each gate is recorded in
[decision 0002](decisions/0002-v0.7.0-qualification.md), the final ledger in
[`V07_EXECUTION_GRAPH.md`](V07_EXECUTION_GRAPH.md), and the release PR. No v0.7.0 tag is
authorized by this document; every tag needs its own explicit go.

**Outcome (2026-11-13):** released as `v0.7.0` → `main` `1df9690`
([release](https://github.com/femboy2112/libgibson/releases/tag/v0.7.0)), on the owner's per-tag go.

**Why now.** MINERVA (ProjectGHOST) is preparing its own 1.0.0-rc.1. It wants to pin a LibGibson
release whose version has no pre-release suffix (`0.7.0`, not `0.7.0-alpha.N`). LibGibson itself
stays 0.x: [RELEASE_CONTRACT.md](RELEASE_CONTRACT.md) §1 reserves 1.0 for a frozen CORE surface,
and that is out of scope here. A downstream 1.0 can pin a stable 0.x tag:

```toml
libgibson = { git = "https://github.com/femboy2112/libgibson", tag = "v0.7.0" }
```

## 1. What qualifying v0.7.0 requires

Sources:
- the v0.7 gates the CHANGELOG lists under `0.7.0-alpha.1`;
- the owner's Renaissance XXXII brief (Phase 6, the v0.7 release gates);
- RELEASE_CONTRACT §1–§7;
- RELEASING.md §0–§6.

Each row shows the state at `068e7d6`.

| # | Gate | State at `068e7d6` | Remaining work |
|---|---|---|---|
| G1 | Terminal lifecycle qualified for the declared Linux scope (input endpoint, output endpoint, geometry, raw-mode baseline) | **Mostly done.** #349 (input endpoint, #345); #371 (output endpoint and geometry); [TERMINAL_LEASE_CONTRACT.md](TERMINAL_LEASE_CONTRACT.md) gives each behaviour a SUPPORTED / SAFE REFUSAL / EXCLUDED class. #375 recovers the screen after a caught owner panic. | #330 (a restored session can still set modes) is open. Its status line still says "v0.7 candidate". |
| G2 | Original in-scope failing acceptance tests resolved or classified | #344's A/B red passes (#349). #348's resize/endpoint acceptance landed in #371, minus two tests: host-owned raw mode (the documented restore semantics stay; coordinator decision, 2026-11-13) and queued input across a stdin retarget. | Record those two exclusions as rows in the lease contract, so the drafts can close against them. |
| G3 | Renderer target-binding stack #338 → #339 → #343 | **Not merged, by design.** #371 took #343's endpoint and #348's geometry. A standalone `Renderer` on an arbitrary writer is **EXPLICITLY EXCLUDED** in the lease contract. #339's `bind_target` API change was declined (coordinator decision, 2026-11-13). | Decide: close #338/#339/#343/#348 as superseded (credited in #371), or keep any part for v0.8. **Owner decision.** |
| G4 | Supported API behaviour explicit: the API freeze for 0.7 | **Gap:** `hold` and `text` are public modules added since v0.6.0 that appear in neither the CORE nor the EXPERIMENTAL list. The `0.7.0-alpha.N` labels in §2 must become `0.7.0`. | WP3. |
| G5 | Termination signals (#268) | Docs merged (#374). The opt-in shutdown API is open: options A/B/C, recommended A. | **Owner decision.** A means no code. |
| G6 | Caught owner panic (#328) | Input fixed (#329); screen, cursor and renderer fixed via `Context` (#375). | A bare `TerminalSession` caller is not covered: document it as EXCLUDED or fix it. WP2. |
| G7 | Exact-candidate CI 8/8, including first-attempt PTY/hangup gates (single-attempt since #282) | Green on every recent main. | Re-run on the release-candidate head (WP7). |
| G8 | Publication-free Release Preflight | Passed on the alpha.4 and alpha.5 prep heads. | Run on the candidate (WP4). Never a required check. |
| G9 | MSRV 1.85 | CI job green. | Unchanged. |
| G10 | C ABI v1 compatible; historical installed clients pass | 67 symbols (v0.6.0 had 59; all additions are additive, `GIBSON_ABI_VERSION` 1); a layout-freeze test runs in CI. | Run the historical v0.6.0 client and ABI checks against the candidate SDK (WP4). |
| G11 | All five language consumers pass from exact installed artifacts (SDK parity) | The v0.8 parity harness (`scripts/dev/parity_v08.sh`) exists, with C, C++, Python, Go and Rust over one script. | Run it against an SDK staged from the candidate (WP4). |
| G12 | Frozen PCM/HumanMusic golden unchanged | Checked by preflight. | WP4. |
| G13 | MINERVA downstream canary passes on the release candidate | MINERVA pins `v0.7.0-alpha.4`; it is adopting alpha.5/alpha.6. | MINERVA runs its gate on the candidate **SHA** before any tag (WP5). |
| G14 | Resource and latency envelopes stated | v0.6 envelopes (ROADMAP R6.5) exist; nothing has been restated for 0.7. | A short 0.7 statement, citing existing measurements, not new claims (WP3). |
| G15 | Node J: GPL adapter delivered, or an approved decision revising its scope | **Partial.** The in-repo clean-room adapter is merged (#299). The GPL port is local only; creating its repository needs the owner. | **Owner decision:** publish it, or record (as `docs/decisions/0002-…`) that v0.7.0 ships without it. |
| G16 | Licenses, CHANGELOG, notices, package contents | Correct for each alpha (download-verified). | Merge the alpha sections into one `## [0.7.0]` entry; regenerate notices (WP7). |
| G17 | Independent final review | Not done. | A reviewer that did not write the code reviews `v0.6.0...candidate` against this plan (WP6). |

**Packaging decisions for the owner:**
1. **Go module tag.** RELEASE_CONTRACT state 2 pairs `v<version>` with `bindings/go/v<version>`. v0.6.0 had one; the alphas did not. Should v0.7.0 have one?
2. **GitHub prerelease flag.** Even v0.6.0 was marked "prerelease" (engineering alpha). Should v0.7.0 be a regular GitHub release? The version string `0.7.0` is what a Cargo pin sees; the flag only changes how GitHub presents it.
3. **Registries** remain out of scope unless separately authorized.

## 2. What matters for MINERVA's actual usage

MINERVA will report its exact LibGibson API surface in its RC1 plan. Until then, this is what
this campaign has **seen** it use or request: `Context` (inline/fullscreen, `run_once`,
`poll_event`, `enter_interactive`, `restore`), `ui` and `ui::experience` (`Blades`, `Skin`,
`Blades::with_skin`), `raster::RgbaRaster` / `composite_over`, the `ui` labels, and
`text::display_width`. That list is provisional.

| Gate | Affects MINERVA? | Why |
|---|---|---|
| G1/G2 terminal lifecycle | **Yes** | Every interactive `Context` runs through the lease. #371 changed where frames go when fd 1 is redirected; MINERVA said it will check whether it ever redirects fd 1. |
| G3 renderer stack drafts | No | MINERVA uses `Context`, not a standalone `Renderer` on its own writer (the EXCLUDED row). |
| G4 API freeze | **Yes** | `ui`, `ui::experience` and `raster` are **EXPERIMENTAL**. A stable 0.7.0 still lets them change in 0.8. MINERVA's 1.0 would pin 0.7.x, which promises only best-effort stability for experimental items within the patch line (§2). If MINERVA needs more, the fix is to promote what it uses to CORE, or to accept the risk in writing. |
| G5 signals | Partly | MINERVA's host loop must follow the #374 pattern if it wants the terminal restored on SIGTERM. This is MINERVA's code, not a LibGibson gate. |
| G6 caught panic | **If** MINERVA catches panics on its owner thread | Covered by #375 through `Context`. Only bare-session use is uncovered. |
| G11 SDK parity | No | MINERVA is a Rust consumer. |
| G13 canary | **Yes** | It is MINERVA's own gate. |
| G15 node J | No | It is a forcing-consumer milestone unrelated to MINERVA. It blocks the tag only by contract, hence the owner decision. |

## 3. Would v0.8.0 or later bring something MINERVA can use for its 1.0?

Candidates, from ROADMAP and the v0.8 docs:
- **v0.8 cross-language parity** (V08_PARITY_SLICE.md): no. MINERVA is Rust-only.
- **Streaming-viewport primitive** (prototyped as an example in #346): possibly, for MINERVA's conversation view. It is not built as a library API, and the brief says to promote it only if the data justify it.
- **Protected presentation regions** (brief §5.3): possibly. The brief prefers proving that existing APIs suffice (an example) over a new primitive.
- **Kitty image and math transport** (#334 M1): no, not for a 1.0. It is experimental and terminal-specific.
- **Opt-in shutdown API** (#268 B/C): small. It would fit 0.7.x as an additive change if MINERVA wants a sub-`max_wait` shutdown.

**Recommendation: no v0.8 bump for MINERVA's 1.0.** Qualify **0.7.0** stable. If MINERVA's RC1
surface report names an additive need (for example a viewport helper), ship it as **0.7.x**: the
contract allows additive changes in a patch. A **0.8** is warranted only by a breaking change
MINERVA actually needs, and none is known.

## 4. Sequenced work packages

Each WP is one PR, gated on exact-head CI 8/8, merged with `--match-head-commit`. "Remote CI
only" unless a HEAVY slot is granted.

| WP | Content | Gate | Depends on |
|---|---|---|---|
| WP0 | **Owner decisions:** G3 drafts, G5 option, G6 exclude-or-fix, G15 node J, Go tag, GitHub prerelease flag. Record them in `docs/decisions/0002-v0.7.0-qualification.md`. | Owner and coordinator go | none |
| WP1 | **#330:** a released or restored session cannot set modes. Red-first PTY test on CI, then the fix and a mutant (the #375 pattern). | CI red then 8/8; mutant killed | none |
| WP2 | **#328 bare session:** an EXCLUDED row in the lease contract (docs), or a `TerminalSession`-level fix with a test, per WP0. | CI 8/8 | WP0 |
| WP3 | **API freeze docs:** classify `hold` and `text`; relabel `0.7.0-alpha.N` → `0.7.0` in RELEASE_CONTRACT §2; move the lease contract status from "candidate" to the qualified rows plus the G2 exclusions; write the 0.7 resource-envelope statement; update the final V07_EXECUTION_GRAPH ledger. | CI 8/8 | WP0–WP2 |
| WP4 | **Candidate evidence** on the WP7 prep head: Release Preflight (dispatch); the parity v08 harness against an SDK staged from that head; historical v0.6.0 client and ABI checks; the PCM golden. Uses CI where possible; local pieces under a HEAVY slot. | All exit 0, receipts kept | WP7 PR open |
| WP5 | **MINERVA canary** on the candidate SHA, pinned by `rev`, never by tag. | MINERVA gate green | WP7 PR open |
| WP6 | **Independent review** of `v0.6.0...candidate` against this plan. | Review with no blocking findings | WP3 |
| WP7 | **Release PR 0.7.0:** version in the 5 lockstep files, CHANGELOG `## [0.7.0]` consolidating the alphas, notices. Merge, then a **tag only on its own explicit go**, then the 4 assets, download-verify, and a post-release docs PR. | CI 8/8; WP4–WP6 green; tag go | WP1–WP6 |

Critical path: WP0 → WP2/WP3 → WP7 prep → WP4 + WP5 + WP6 in parallel → tag go. WP1 can start
at once.

## Not claimed

This plan does not certify any gate. Gates marked done cite their merged PRs; everything else
is open. The MINERVA usage list in §2 is provisional until MINERVA's own report arrives.
