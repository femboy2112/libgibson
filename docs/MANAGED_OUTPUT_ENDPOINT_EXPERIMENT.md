# Managed output endpoint experiment

Status: experimental successor to #339; **HOLD**, not a released contract.
The renderer target experiment remains separately subject to its Rust API,
inline/history and consumer compatibility gates. No C ABI change is proposed.

A live Unix interactive session now captures an owned duplicate of stdout's
kernel descriptor. Managed Context frames, paste/mouse/focus/Kitty requests,
screen switches, panic cleanup and normal restoration use that same endpoint.
Changing fd 1 later cannot redirect those operations. This uses a retained kernel
reference rather than inferring destination identity from an fd number, writer
address, session ID or `isatty` result.

Only the session keeps a persistent strong reference. Context and the owner-thread
panic hook hold weak references; an operation upgrades one temporarily. Direct
session restoration releases the endpoint as well as the lease, so a surviving
Context cannot retain it or claim a later frame. This does not prove physical
presentation, atomic writes or terminal protocol support.

The initial prototype passed the original complete VT100 stdout-retarget oracle
and redirected-restore/descriptor-release controls. A separate same-model-family
review then found two regressions, both reproduced before repair: buffered Rust
stdout could overtake a managed frame, and closed non-TTY stdout could make
construction fail. Construction now holds the stdout lock through capture and
post-lease-CAS buffer draining; a rejected second owner must not flush another
owner's pending bytes. Non-TTY construction does not duplicate the descriptor.
A failed initial flush releases the unpublished lease. Existing synthetic lease
tests must supply a synthetic endpoint; their reset/next-owner assertions remain.

`tests/session_output_endpoint_pty.rs` checks complete frame/cursor state, restore
while stdout is redirected, both restore entry points' descriptor release,
caught-panic mode re-entry, buffered output order, rejected acquisition and closed
non-TTY construction. `tests/context_output_identity_pty.rs` retains the ordinary
full-grid A/B/A acceptance and its independent pinned/refusal/dropped-frame controls.
The old-source red runs and review counterexamples are preserved in local receipts.

## Boundaries still requiring qualification

- Geometry and input acquisition still use their existing crossterm/global source
  paths. Their relationship to a captured output endpoint, including non-controlling
  output terminals, needs an operational test. A suspected mismatch is not a
  reproduced regression merely from source review.
- Non-TTY Context output retains its existing process-stdout behavior. Headless
  capture and explicit generic-writer bindings are distinct destinations.
- Unmanaged stdout writes, direct OS descriptor mutation, free input helpers and
  externally retargeted generic writers remain outside the managed lease contract.
  During an active session, normal application output should use public managed
  rendering/history operations rather than interleaving independent stdout writes.
- A failed or torn physical request remains uncertain. Existing fault seams and
  cleanup obligations must survive the combined runtime qualification.
- Native Windows endpoint pinning is unverified; the non-Unix branch retains the
  existing stdout path. No Windows support claim follows from this experiment.
- Fresh exact-head CI, Rust 1.85, full scene/surface/PTY/installed-consumer tests,
  resource checks, and final combined review remain required before promotion.

No stable endpoint API, default graphics transport or release approval follows
from these scoped tests. Preserve the old ANSI golden and every surviving oracle.

## Compatibility classification and fault calibration

The first seven-target legacy PTY run returned 68 passes and two failures:
`failed_restore_also_prevents_focus_reenable` and
`restore_reports_output_failure_but_still_tears_down`. Both redirected fd 1 to
`/dev/full` after creating the session. That changes an alias, not the captured
output endpoint; restoration correctly reached the still-live original terminal.
These are **TEST HARNESS MIGRATION** failures under the proposed pinned-output
contract, while retaining the old output-routing contract is an explicit Rust
behavior/compatibility decision. The earlier buffered-output and non-TTY findings
were actual prototype regressions, repaired with red/green controls.

The migrated fixtures select a dedicated output PTY before construction, retain
original stdin/stderr for observation, then close its actual master and observe
kernel HUP within a bound. The original error, raw-mode cleanup, lease release
and stale-focus assertions remain. Additional kernel termios observations prevent
bookkeeping alone from satisfying raw cleanup. All seven targets then passed:
70 passed, zero failed, zero ignored. Keeping the output master alive deliberately
makes the error oracle fail with `RESTORE_RESULT=OK`; removing that mutation
restores the passing test. Targeted ordinary Clippy passed. No oracle was hidden
behind an ignore and the ANSI golden was not rewritten.

Before combining this branch with newer main, also inspect the installed wrapper
focus fault clients: their fd-1-to-`/dev/full` technique must target the captured
endpoint if this contract is adopted. A green parity run on main's existing
stdout behavior is not evidence for this experimental output path. Full installed
compatibility, geometry/input and the parent renderer API remain promotion gates.
