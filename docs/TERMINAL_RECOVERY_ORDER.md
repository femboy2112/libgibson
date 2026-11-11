# Combined terminal recovery ordering — integration witness

This is evidence for the prospective #333 → #329 → #327 combination, not a
claim that all terminal state is recovered after panic. Issues #328 and #330
remain open until their bounded acceptance is qualified on merged main.

On a Linux PTY, the caught owner-thread panic hook leaves raw input and attempts
reporting cleanup. Retrying focus and mouse reporting before entering interactive
input succeeds at writing requests, but the input remains canonical with kernel
echo enabled until polling or explicit interactive entry. Injected focus gained,
an un-newlined key, and focus lost were echoed before the next poll; the later
poll nevertheless delivered those three events in order in the measured case.
Do not describe this as lost input or full session recovery.

For a caller about to request asynchronous reporting or paint input readiness,
call the public `Context::enter_interactive()` first. In the combined PTY control,
eager entry followed by focus/mouse retries has raw input, no kernel echo and
exactly one focus-gained/key/focus-lost sequence. No-panic and non-owner-panic
controls have the same ready-input observations. Original local flags are checked
after restore. Omitting eager entry in the positive control fails.

The test uses an explicit FIFO rendezvous before polling; it does not infer
readiness from a delay. Reporting request bytes are not evidence that a terminal
supports those protocols. Separate input, mouse and focus invalidation notices,
cleanup obligations and completed write/flush flags remain separate. Kitty
reconciliation must not discard the raw cleanup obligation acquired after panic.

Alternate screen, cursor, renderer output destinations, redirected restoration,
physical terminal acknowledgments and unmanaged helpers are not qualified by
this witness. A complete lease/output lifecycle audit is still required.
