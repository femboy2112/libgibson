# Terminal lease contract (Linux scope, v0.7.0)

Status: **input side implemented** (issue #345, merged in #349 as `b3428db`); **output
endpoint and resize geometry implemented** (LIB-1; the mechanism and tests come from the draft
stack #338 → #339 → #343 and research #348, without #339's `bind_target` API change); **lease authority complete** (a restored session
cannot set modes, #333; a restored `Context` cannot write, #371; restore and `Drop` are decided
by lease ownership, #379). This document is the decision record for which descriptor changes a managed session
supports. It is not a public type: the lease is private implementation state.

## Model

A managed session (`TerminalSession` / `Context`) that enters interactive mode holds a lease:

| Component | Where it lives | Bound at |
|---|---|---|
| Lease identity (authority to mutate terminal state) | `TERMINAL_LEASE` + owner thread (`session.rs`) | session construction |
| **Input endpoint** | `input_endpoint::ACQUIRED` — our own close-on-exec descriptor for the terminal (dup of stdin when it is a tty, else our own open of `/dev/tty`) | first `enter_interactive` |
| Mode baseline | termios of the input endpoint, read immediately before `cfmakeraw` | raw entry |
| Input generation | `tty_source::GENERATION`; bumped on acquisition and on restore | acquisition / release |
| **Output endpoint** | the session's own close-on-exec duplicate of fd 1, taken when fd 1 is a terminal and the lease is won; `Context` frames, mode and restore bytes, and the panic hook all write it | session construction; closed on restore/Drop |
| **Geometry source** | `TIOCGWINSZ` on the held output descriptor, for both `terminal_size()` and resize events; crossterm's `/dev/tty`-else-stdout only when no lease holds an output | session construction |
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

Rules implemented for the output side:

6. A session that wins the lease on a terminal fd 1 holds its own duplicate of it
   (`F_DUPFD_CLOEXEC`, so spawned children never inherit it). Pending Rust `stdout` buffering
   is flushed first, and only after the lease is won (a refused second owner flushes nothing).
7. Every byte LibGibson writes during the lease (frames, mode changes, restore, the panic hook)
   goes to that descriptor, so the renderer's differential state always describes one
   destination. Restore clears the panic hook's `Weak` reference, then drops the descriptor:
   it is closed exactly once, and a reused descriptor number is never written or queried.
8. `terminal_size()` and every delivered resize read the held descriptor's size at delivery
   time. The input source reaches it through the same `Weak` reference (upgraded per call; no
   allocation), so a delivered resize can never describe another terminal. (A poll already in
   flight before acquisition is refused under rule 4 and delivers nothing.) A failed size read keeps the documented
   80×24 fallback for the getter and delivers no resize event.
9. fd 1 that is not a terminal at construction (a pipe, a file, closed) gives a non-TTY session
   with no held descriptor, exactly as before.

## Behaviour contract

| Case | Class | Evidence |
|---|---|---|
| Stable stdin/stdout, same terminal | SUPPORTED | existing PTY suite (`enter_interactive_pty`, `caught_panic_input_pty`, …) unchanged |
| stdin replaced after acquisition — restore | SUPPORTED: A restored, B untouched, `Ok` | `raw_input_retarget_pty` (#344 oracle, unchanged), `acquired_input_endpoint_pty::retarget_*` (no ctty and controlling A) |
| stdin replaced then put back (A→B→A) | SUPPORTED | `a_to_b_to_a_restores_a` |
| stdin replaced — input, including bytes queued on A and B before the first poll | SUPPORTED: keys come from A only; B's bytes stay queued for B (`FIONREAD` on B unchanged) | `input_comes_from_the_acquired_terminal_after_retarget` |
| stdin replaced — caught owner panic, re-entry | SUPPORTED: hook restores A, re-entry raws A | `caught_owner_panic_restores_a_not_b` |
| Repeated restore / Drop after restore | SUPPORTED: no-ops, B untouched | `repeated_restore_and_drop_never_touch_b` |
| Restore after A hangs up | SAFE: B never touched; result may be `Err` (A is gone) | `hangup_of_a_then_restore_never_touches_b` |
| New owner after restore | SUPPORTED: acquires the stdin it finds (B), restores B | `fresh_owner_on_b_after_restore_owns_b_only` |
| Restored session changes a mode (mouse, focus, key-release reporting, alternate screen, interactive entry) | SAFE REFUSAL in the enabling direction: `NotConnected`. Teardown (mouse or focus or key-release off, leaving the alternate screen) is an `Ok` no-op. Either way no bytes are written and the owner's panic-cleanup flags are untouched | `mouse_lease_pty::{restored_session_cannot_enable_or_disable_mouse_for_new_owner, restored_session_cannot_switch_a_new_owners_screen, stale_mouse_calls_cannot_hide_owner_capture_from_panic_cleanup}`, `key_release_reporting_pty` (`restored`, `focus-restored`) (#333; teardown semantics unified for 0.7.0 after the independent review) |
| Restored `Context` renders or commits text | SAFE REFUSAL: nothing reaches the next owner's terminal | `lease_authority_pty::restored_context_cannot_render_or_commit_into_the_next_owners_terminal` (#371) |
| Restore / `Drop` authority | SUPPORTED: a session that owns the lease restores, whatever its cached tty flag says; one that never won it touches nothing | `lease_authority_pty::{restore_,drop_}ignores_a_cleared_is_tty_field_while_owning_the_lease` (#379; red, then green, and the mutant restoring the old check is killed) |
| No controlling terminal (input/raw) | SUPPORTED | all raw/input cases above run with `setsid()` and no ctty |
| Host entered raw mode **through crossterm** first | SUPPORTED (compat): restore still disables raw mode, as documented | `host_crossterm_raw_is_still_disabled_on_restore` |
| Host crossterm raw + stdin replaced | SAFE REFUSAL: `restore()` returns `Err`, neither A nor B changed | `host_crossterm_raw_with_retarget_is_refused_without_mutation` |
| Host entered raw mode with its own `tcsetattr` | SUPPORTED (unchanged semantics): the baseline read at entry is the host's raw termios and restore returns to it | by construction, same as crossterm's prior baseline capture; no dedicated test |
| Poll blocked **before** acquisition | SAFE REFUSAL: returns `Ok(None)`, its event discarded; owner receives later input | `poll_blocked_before_acquisition_is_not_delivered_to_the_owner` (syscall-entry evidence from `/proc/self/task/<tid>/syscall`, no sleeps) |
| Poll blocked under an old generation when **restore** retires the source | SAFE REFUSAL (same code path as above) | mechanism shared; restore-time case has no dedicated test — UNVERIFIED as a separate scenario |
| Unmanaged `gibson::input::poll_event` with no session | EXPLICITLY EXCLUDED from the lease: resolves its descriptor at source creation, as before | documented API boundary (`input::poll_event` docs) |
| **stdout redirected** — frames, mode and restore bytes, panic hook | SUPPORTED: all reach the acquired terminal; nothing reaches the redirect | `session_output_endpoint_pty` (frame A/B cell equality, restore during redirect, modes + caught panic) |
| Child process spawned during a lease | SUPPORTED: the held descriptor is not inherited | `held_descriptor_is_not_inherited_by_children` |
| Restore then Drop; descriptor number reused | SUPPORTED: closed exactly once, the reusing file survives | `restore_and_drop_close_the_held_descriptor_exactly_once`, `explicit_restore_closes_the_owned_terminal_descriptor` |
| fd 1 a pipe, or closed, at construction | SUPPORTED (unchanged): non-TTY session, no descriptor held, lease untouched | `piped_stdout_at_acquisition_holds_no_descriptor`, `closed_stdout_still_allows_a_non_terminal_session` |
| **Resize geometry** after stdout redirect, with and without ctty | SUPPORTED: getter and event both report the acquired output | `endpoint_geometry_pty::{output_geometry_tracks_the_acquired_endpoint, resize_event_matches_the_acquired_output_geometry}` |
| Resize read by a poll in flight before acquisition | SAFE REFUSAL (rule 4): that poll returns `Ok(None)`; it never reports another terminal's size | `prior_unmanaged_inflight_poll_must_not_report_a_different_terminal` |
| Standalone `Renderer` with a caller-supplied writer | EXPLICITLY EXCLUDED: the caller promises the writer reaches the destination its differential state describes (no `bind_target` API change) | documented `Renderer` boundary |
| Caught owner panic, then more frames through `Context` | SUPPORTED: the next write redraws in full on the screen the session owns, cursor hidden | `caught_panic_screen_pty` (#375), `caught_panic_input_pty` (#329) |
| Caught owner panic, then more frames from a bare `TerminalSession` caller (no `Context`) | EXPLICITLY EXCLUDED: the panic hook leaves the alternate screen and shows the cursor; a caller that drives `TerminalSession` and its own renderer must re-enter its modes and redraw in full itself | decision (#328, LIB-RC0 G6); `Context` path above |
| Non-Linux Unix, Windows | UNVERIFIED (compiles; Windows keeps crossterm's raw path) | no runtime test |

## Consumer note: the application's own stdout writes

LibGibson writes the terminal it acquired. An application that writes `io::stdout()` itself
during a lease writes **fd 1 as it is now**: if the application (or a library it uses)
redirects fd 1, its own bytes follow the redirect while LibGibson's frames stay on the
acquired terminal. Before this change both followed the redirect. Applications that print
around a live frame should use `Context::insert_text_before_live` / `commit_text` (which write the acquired
terminal) rather than raw stdout.

## Host responsibility: termination signals (#268)

LibGibson installs **no** handler for a termination signal. The only signal it registers is
`SIGWINCH`, for resize, through `signal-hook`'s self-pipe. SIGTERM, SIGINT sent with `kill`,
SIGQUIT and SIGHUP therefore keep the action the host gave them, usually the default, which
ends the process at once. No `Drop`, no panic hook and no `restore()` runs. The terminal is left
as the session set it: raw mode (no echo, no canonical input), the cursor hidden, bracketed
paste on, and the alternate screen or mouse/focus reporting if those were enabled. #268 measured
this on a PTY: `lflag` was `0o105073` before and `0o5060` after a SIGTERM.

In raw mode a typed **Ctrl+C is a key**, not a signal (`ISIG` is off). It arrives as
`Event::Key` with `KeyCode::Char('c')` and the control modifier, and the application decides
what it means. The signals above come from outside: a service manager, `timeout(1)`, `kill`, or
a parent tearing down a PTY.

**The host owns them.** If the terminal should be restored when one arrives, the host must catch
it and leave its loop through the normal restore path:

1. **Only record the signal in the handler.** Set an `AtomicBool`; storing to an atomic is
   async-signal-safe. Never call `restore()`, `Drop` or any LibGibson API from the handler. They
   take locks, allocate and write the terminal, and none of that is async-signal-safe.
2. **Check the flag on every loop iteration**, then break and call `ctx.restore()` as on any
   other quit.
3. **Bound the wait.** An interrupted poll is resumed for the time left (`EINTR` is retried until
   `max_wait` runs out), so a signal does **not** cut a `run_once` / `poll_event` call short. The
   exit latency is up to `max_wait` plus one frame. Use a wait such as 50–100 ms rather than a
   long or unbounded one. A consumer using this pattern measured a 33–45 ms exit with termios,
   cursor and paste restored (#268).
4. **Optionally re-raise.** To let the parent still see a death by that signal, reset it to
   `SIG_DFL` after `restore()` and raise it again. `examples/deep_signal.rs` does this.
5. **Optionally add a watchdog.** `restore()` writes, so it blocks if the terminal stopped
   reading. `examples/deep_signal.rs` gives the orderly stop 3 s, then restores cooked mode from
   the saved termios with `tcsetattr` and applies the default action anyway.
   `tests/deep_signal_lifecycle_pty.rs` covers both cases on a PTY.

```rust
// Cargo.toml of the host: signal-hook = "0.3"
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use std::time::Duration;
use gibson::input::{Event, KeyCode};
use gibson::{Context, RenderMode};

fn main() -> std::io::Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        // The handler only stores `true`; everything else happens on the loop thread.
        signal_hook::flag::register(sig, Arc::clone(&stop))?;
    }
    let mut ctx = Context::new(RenderMode::Inline)?;
    while !stop.load(Ordering::Relaxed) {
        // Bounded wait: a signal does not shorten this call.
        match ctx.run_once(Duration::from_millis(50)) {
            Ok(Some(Event::Key(k))) if k.code == KeyCode::Char('q') => break,
            Ok(_) => {}
            Err(_) => break, // includes a hung-up terminal (BrokenPipe): leave, do not retry
        }
    }
    ctx.restore()
}
```

**SIGHUP: keep the default action, unless the host has cleanup of its own.** A hangup usually
means the terminal is gone, so there is nothing to restore, and catching SIGHUP mostly keeps the
process alive with no terminal. Catching it is right in two cases (see "When to catch SIGHUP"
below). History: on
releases before #269, a process that survived SIGHUP could busy-spin in the input poll (#79).
On Linux, since #269 / #282, a poll on a hung-up terminal returns `Err(BrokenPipe)` promptly
even with SIGHUP ignored or handled (`tests/pty_hangup_matrix.rs`). Two cautions remain:

- #79's original `run_once` reproducer, re-run on `b8d076d`, no longer spins: it exits with
  code 1. The v0.4.0 control still spins. The probable fix is #282.
- A loop that retries on poll errors would still spin.

A host that does catch or ignore SIGHUP, including one started under `nohup`, must therefore leave
its loop on any poll error, as the sample does.

**When to catch SIGHUP.** Catch it (register it into the same flag as SIGTERM/SIGINT) when either
case applies:

- **The host has application-level cleanup that must run on hangup.** Examples: cancelling a
  request it started in another process, flushing a journal, releasing a lock. The default action
  ends the process before any of that runs.
- **SIGHUP can arrive while the terminal is still alive.** `kill -HUP`, a service manager or a
  multiplexer can send it without closing the terminal. Under the default action the terminal is
  left raw, with the cursor hidden and paste mode on, exactly as for any other uncaught signal.

If you catch it, the pattern is the sample's, with three facts to rely on:

1. **Leave the loop on any poll or render error. Never retry.** After a real hangup, input polls
   return `Err(ErrorKind::BrokenPipe)` (`src/tty_source.rs`, `hung_up()`), and a frame or commit
   write fails with the OS error unchanged. On Linux that is `EIO` from the hung-up tty. The write
   path never rewraps it: `Transaction::commit_counted` returns the writer's own `io::Error`
   (`src/transaction.rs`), and the renderer propagates it as-is. So
   `e.kind() == BrokenPipe || e.raw_os_error() == Some(libc::EIO)` identifies a lost terminal.
   The only non-OS errors on that path are `WriteZero` (the terminal accepted 0 bytes) and
   `NotConnected` (output used after the session released the terminal). Neither is a hangup.
2. **Expect `restore()` to return `Err` after a real hangup.** `tcsetattr` on a severed terminal
   may fail with `EIO`. Restore is best effort there and still completes without hanging
   (`tests/pty_hangup_restore.rs` records the result but deliberately does not assert it). Treat
   that error as expected when you are leaving because of a hangup. Don't turn it into a failure
   exit.
3. **Do the host's own cleanup before `restore()` only if it cannot block.** Restore writes, so a
   slow cleanup step placed first delays it. Queue the cleanup, restore, then let the cleanup
   finish within a bound.

**SIGQUIT.** Its default action ends the process with a core dump and leaves the terminal raw.
In raw mode a typed Ctrl+\ arrives as a key event, not a signal, so SIGQUIT only comes from
outside (`kill -QUIT`). A host that wants the terminal restored in that case catches it into the
same flag. Everything above applies.

**Not covered:** job control. A typed
Ctrl+Z is a key in raw mode. A `kill -TSTP` / `SIGSTOP` stops the process with the terminal
still raw, and LibGibson does not re-enter its modes on `SIGCONT`. Both are UNVERIFIED beyond
this description.

**Possible library help (not implemented, an API decision):** an opt-in shutdown request that
`run_once` / `poll_event` honour, for example a flag the host registers or an `Event::Shutdown`.
The poll would wake at once, and the host would still restore from its own loop. Until that
exists, the pattern above is the contract.

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

## Open items (v0.7 lifecycle gate)

1. ~~Output endpoint identity~~ — implemented (rules 6–7).
2. ~~Geometry from the acquired output~~ — implemented (rule 8).
3. A shared input/output generation is **not needed** for geometry: sizes are read at delivery
   from the live lease's descriptor (rule 8), so a stale resize cannot report another terminal;
   at worst it is a redundant resize carrying the acquired terminal's true size.
4. ~~#330~~ — closed (#333, #371, #379). ~~#328~~ — `Context` covered (#329, #375); the bare
   `TerminalSession` case is EXPLICITLY EXCLUDED above. #268 — decided: the host owns
   termination signals (section above); an opt-in shutdown API is not part of 0.7.0.
   Non-Linux output pinning stays UNVERIFIED (Windows keeps writing process stdout).
