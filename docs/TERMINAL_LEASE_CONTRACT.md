# Terminal lease contract (Linux scope, v0.7 candidate)

Status: **input side implemented** (issue #345, merged in #349 as `b3428db`);
output identity and resize geometry **not yet qualified** (stack #338 → #339 → #343, research
#348). This document is the decision record for which descriptor changes a managed session
supports. It is not a public type: the lease is private implementation state.

## Model

A managed session (`TerminalSession` / `Context`) that enters interactive mode holds a lease:

| Component | Where it lives | Bound at |
|---|---|---|
| Lease identity (authority to mutate terminal state) | `TERMINAL_LEASE` + owner thread (`session.rs`) | session construction |
| **Input endpoint** | `input_endpoint::ACQUIRED` — our own close-on-exec descriptor for the terminal (dup of stdin when it is a tty, else our own open of `/dev/tty`) | first `enter_interactive` |
| Mode baseline | termios of the input endpoint, read immediately before `cfmakeraw` | raw entry |
| Input generation | `tty_source::GENERATION`; bumped on acquisition and on restore | acquisition / release |
| Output endpoint | process `stdout()` **at each write** (unchanged on main) | — (see open items) |
| Geometry source | `crossterm::terminal::size()`: `/dev/tty` when there is a controlling tty, else stdout | — (see open items) |
| Cleanup obligation | `raw_mode_enabled` + `input_endpoint::Acquired::baseline` | raw entry; discharged on restore |

Rules implemented for the input side:

1. Raw mode is entered on, and restored to, the **acquired** descriptor. crossterm 0.29's
   `disable_raw_mode` re-resolves `tty_fd()` on every call (`src/terminal/sys/unix.rs:148`),
   which is the #345 mechanism; LibGibson no longer uses it on Unix except as below.
2. The input source reads the acquired descriptor (a provably-same non-blocking reopen when
   possible), never descriptor 0 as it is at first poll.
3. The input parser's raw-state question (bare LF = Ctrl+J in raw mode) answers for the
   lease's raw mode, or for crossterm's when the host entered raw mode through crossterm.
4. Acquisition and restore retire the input source (generation bump). A poll that was already
   blocked on the retired source returns `Ok(None)`; the event it read is **discarded**, never
   delivered as the new owner's input.
5. The panic hook restores the acquired descriptor without blocking (`try_lock`).

## Behaviour contract

| Case | Class | Evidence |
|---|---|---|
| Stable stdin/stdout, same terminal | SUPPORTED | existing PTY suite (`enter_interactive_pty`, `caught_panic_input_pty`, …) unchanged |
| stdin replaced after acquisition — restore | SUPPORTED: A restored, B untouched, `Ok` | `raw_input_retarget_pty` (#344 oracle, unchanged), `acquired_input_endpoint_pty::retarget_*` (no ctty and controlling A) |
| stdin replaced then put back (A→B→A) | SUPPORTED | `a_to_b_to_a_restores_a` |
| stdin replaced — input | SUPPORTED: keys come from A only; B's bytes stay queued for B | `input_comes_from_the_acquired_terminal_after_retarget` |
| stdin replaced — caught owner panic, re-entry | SUPPORTED: hook restores A, re-entry raws A | `caught_owner_panic_restores_a_not_b` |
| Repeated restore / Drop after restore | SUPPORTED: no-ops, B untouched | `repeated_restore_and_drop_never_touch_b` |
| Restore after A hangs up | SAFE: B never touched; result may be `Err` (A is gone) | `hangup_of_a_then_restore_never_touches_b` |
| New owner after restore | SUPPORTED: acquires the stdin it finds (B), restores B | `fresh_owner_on_b_after_restore_owns_b_only` |
| No controlling terminal (input/raw) | SUPPORTED | all raw/input cases above run with `setsid()` and no ctty |
| Host entered raw mode **through crossterm** first | SUPPORTED (compat): restore still disables raw mode, as documented | `host_crossterm_raw_is_still_disabled_on_restore` |
| Host crossterm raw + stdin replaced | SAFE REFUSAL: `restore()` returns `Err`, neither A nor B changed | `host_crossterm_raw_with_retarget_is_refused_without_mutation` |
| Host entered raw mode with its own `tcsetattr` | SUPPORTED (unchanged semantics): the baseline read at entry is the host's raw termios and restore returns to it | by construction, same as crossterm's prior baseline capture; no dedicated test |
| Poll blocked **before** acquisition | SAFE REFUSAL: returns `Ok(None)`, its event discarded; owner receives later input | `poll_blocked_before_acquisition_is_not_delivered_to_the_owner` (syscall-entry evidence from `/proc/self/task/<tid>/syscall`, no sleeps) |
| Poll blocked under an old generation when **restore** retires the source | SAFE REFUSAL (same code path as above) | mechanism shared; restore-time case has no dedicated test — UNVERIFIED as a separate scenario |
| Unmanaged `gibson::input::poll_event` with no session | EXPLICITLY EXCLUDED from the lease: resolves its descriptor at source creation, as before | documented API boundary (`input::poll_event` docs) |
| **stdout redirected** — frames and restore bytes | UNVERIFIED / HOLD: main writes to process stdout at each write, so they follow the redirect | #339 / #343 stack, `handoffs/RENAISSANCE_XXXI.md` |
| **Resize geometry** after stdout redirect without ctty | UNVERIFIED / red: `size()` falls back to stdout (B) | #348 `resize_event_matches_the_acquired_output_geometry` |
| Non-Linux Unix, Windows | UNVERIFIED (compiles; Windows keeps crossterm's raw path) | no runtime test |

## crossterm interplay (decision)

On Linux the lease enters raw mode **through crossterm** whenever crossterm's `tty_fd()`
provably resolves to the acquired terminal (`TIOCGDEV` match), so crossterm's process-global
raw-mode record stays truthful for hosts that also call `crossterm::terminal::{enable,disable}_raw_mode`
(`examples/event_pressure_lab` does; `host_crossterm_raw_after_acquisition_still_ends_cooked`).
Bypassing crossterm entirely made such a host leave the terminal raw: an ordinary-caller
regression caught by `tests/event_pressure_pty.rs` and rejected.

Restore goes through crossterm only while its target still matches; otherwise the acquired
descriptor is restored directly and crossterm's record is marked stale (never again read as a
host's raw mode by LibGibson).

**Residuals, stated rather than hidden** (from independent adversarial review):

1. After a *replaced-stdin* restore, crossterm's own record still holds A's baseline. LibGibson
   never writes B, but a host that afterwards calls `crossterm::terminal::disable_raw_mode()`
   itself makes crossterm apply A's baseline to its current stdin (B). Clearing that record
   would require writing to B. Class: EXPLICITLY EXCLUDED (host-initiated crossterm call after
   a hostile descriptor replacement); ordinary hosts are unaffected. Owner decision point.
2. The stale mark is cleared only when a later acquisition sees crossterm's flag off. A host
   that disables and re-enables crossterm raw mode in between is then not recognised as host
   raw by the parser or a later session. UNVERIFIED (no test), LOW.
3. The crossterm identity checks are check-then-act; a concurrent `dup2` of stdin between the
   check and crossterm's own re-resolution is not defended. EXPLICITLY EXCLUDED (concurrent
   descriptor mutation during acquisition/restore).
4. A poll blocked on a retired source has already consumed its bytes when it returns; those
   bytes are discarded (the safe-refusal cost). Queued-but-undelivered events from an
   unmanaged pre-session poll are likewise discarded at acquisition.
5. On non-Linux Unix the identity check uses `st_rdev`, which for a `/dev/tty` descriptor is
   `/dev/tty`'s own number. UNVERIFIED.

## Mutation controls (all rejected by the suite)

| Mutation | Failing tests |
|---|---|
| restore applies baseline to `STDIN_FILENO` | 6 of 12 (all retarget variants; A→B→A rightly survives) |
| input source ignores the acquired descriptor | `input_comes_from_the_acquired_terminal_after_retarget` |
| no generation check after a blocked poll | `poll_blocked_before_acquisition_…` |
| no source retirement on acquisition | `poll_blocked_before_acquisition_…` |
| restore always delegates to crossterm (old re-resolving behaviour) | 6 of 14 |
| never enter through crossterm (pure bypass) | `host_crossterm_raw_after_acquisition_still_ends_cooked` |
| parser ignores the lease flag | **survives** on Linux: equivalent mutant while crossterm's record is truthful; only the stale-record cfmakeraw path differs, and it has no LF oracle |

## Open items (release blockers for the v0.7 lifecycle gate)

1. Output endpoint identity: bind writes, restore bytes and the renderer's differential state
   to the acquired output (stack #338 → #339 → #343; #339 has red ordinary acceptance).
2. Geometry: derive both `terminal_size()` and delivered resize events from the acquired
   **output** endpoint, not crossterm's `/dev/tty`-else-stdout resolution (#348).
3. The input endpoint and output endpoint can then share one generation, so a resize from a
   retired output is refused like a retired input event.
