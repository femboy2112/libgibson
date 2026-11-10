# Terminal lease audit: Renaissance XXV

**Status: WIP.** This is an operation inventory, not a claim of complete
terminal sovereignty. The candidate in this branch builds on #333; #329 and
#327 are not yet integrated. #330 and #328 remain open.

| State | Managed modes / input | Managed Context output | Restore and Drop |
|---|---|---|---|
| Active interactive lease | May request modes; successful write/flush is not terminal capability | May emit on stdout | Owner performs one-shot best-effort cleanup and releases lease, including on error |
| Restored former owner | Mouse and screen guarded in #333; raw and focus fixes are separate drafts | Candidate rejects render, commit, insert and clear before output | Repeat is a no-op |
| Never owned / redirected | Mode operations should be no-ops or reject | Plain redirected output remains supported | Must not modify global owner or terminal state |
| Headless | No OS mode operations | Virtual buffered rendering remains supported | No OS cleanup |
| Torn write / failed flush | Physical state unknown; cleanup responsibility must survive where appropriate | Renderer recovery remains separately qualified | Best effort; no atomic restoration claim |
| Caught owner panic | Hook cleanup invalidates request state; #329/#327 must be combined with #333 | No general screen/renderer recovery is claimed | Cleanup ownership must survive until restore |

## Discriminating observations

Linux PTY tests at #333 `290f7089c43aab21d7403277da300cffa0e8523b`
reproduced: a restored context rendered and committed into its successor's
terminal; an owner setting public `is_tty` false retained raw mode and the
lease after restore; and a never-owner setting `is_tty` true emitted a reset
into a successor's terminal. `tests/terminal_sovereignty_pty.rs` is red on
that revision and passes on the local candidate. It also exercises insertion,
clear, a live owner's commit and canonical restoration. These tests observe
request bytes and termios, not terminal support.

The public `is_tty` flag is a mutable presentation setting, not proof of
ownership. This candidate uses private lease state for cleanup and checks
managed output. Further tests and review are needed before treating it as an
exhaustive boundary.

## Open boundaries

* Public unmanaged input helpers write without a session lease. Public
  `Renderer` methods accept arbitrary writers; a caller can direct a headless
  session's bytes to a terminal. A library-wide invariant cannot be claimed
  while these compatibility escape hatches remain available.
* `Context.session` is public and can be replaced with another live session;
  the renderer may retain frame and cursor assumptions from the former owner.
  A PTY rendered-state test reproduced this. The candidate rejects Context
  **physical stdout output** after replacement; this is a behavior change requiring
  compatibility review. It does not prevent explicitly operating on or
  restoring the transferred live session through the public field or other
  Context methods, so it is not a general Context identity guarantee.
* `Context.renderer` is also public; moving a renderer between contexts can
  carry stale frame and cursor state to another live session. A PTY
  rendered-state test reproduced it; the candidate rejects physical output
  after renderer replacement. This behavior change also requires review.
* Test the final combined #333/#329/#327 state after reconciliation: worker
  versus owner panic; raw, paste, mouse, focus and Kitty; partial write and
  flush; hangup; repeated ownership handoffs; double restore and Drop.
* Audit direct renderer entry points for pre-error mutations, raw unmanaged
  input, cursor, screen transitions, output destinations and C/wrapper clients.
  Do not infer physical terminal support from a completed write.

Focused probe: `cargo test --locked --test terminal_sovereignty_pty -- --test-threads=1`.
The branch needs independent assessment, exact-head CI, compatibility checks
and final combined-state review before integration.
