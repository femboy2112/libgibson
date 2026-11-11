# Renderer destinations — experimental contract review

Status: design and compatibility assessment for draft #339. This document does not
qualify that PR, physical stdout ownership, or a release. C ABI v1 and Rust 1.85
remain unchanged.

A differential frame is a belief about one destination. The prior surface alone
is insufficient: SGR, cursor position/visibility, color depth, inline origin,
geometry, history and recovery debt belong to the same destination.

## Competing designs

| Design | Useful guarantee | Cost or missing observation |
|---|---|---|
| Destination-owned emission state | Each output owns all differential and recovery state | Requires a larger refactor; a caller-supplied identity cannot prove a generic writer still reaches that destination |
| Lifetime-bound renderer/session/writer | Ordinary borrows prevent accidental replacement while the guard exists | Internally retargeted or aliased writers remain a caller obligation; session controls and rebinding need further design |
| Stateless arbitrary output plus bound efficient output | A fullscreen arbitrary call re-establishes cells, attributes and cursor; a valid binding retains zero-byte settled frames | Unbound inline origins/history cannot be recovered automatically; existing direct callers must migrate |

The same `Write` object can change its destination internally between calls.
Neither its address, a session identifier, an fd number nor `isatty` proves
persistent destination equivalence. No efficient differential implementation may
infer that equivalence from those observations alone.

## Current experiment and compatibility cost

`Renderer::bind_target` binds a fresh renderer, session and writer. The caller must
keep the writer directed at one destination and exclude other output for the
binding's lifetime. Successful write/flush means writer acceptance, not physical
presentation or terminal support. This is a conditional caller contract.

Unbound fullscreen calls conservatively repaint. Their byte counts, full-repaint
flags and damage counters therefore change. Unbound interactive inline rendering,
relative history insertion, commit and clear return an error before output.
These are deliberate Rust behavior changes, requiring migration notes and review
for the next pre-1.0 minor release. They are not qualified patch compatibility.
`BoundTarget` currently lacks a complete session-control/lifecycle projection.

Tests that model one persistent display now use one bound writer. Draining a
capture does not replace its destination. All previous VT cells, attributes,
cursor, damage/performance and fault assertions remain acceptance criteria.
A changed final display on a valid bound target is a regression.

A fresh `Context::headless` creates a known empty virtual display. Only its first
output operation has that baseline. Arbitrary bound writers have unknown initial
content and must erase the allocated inline rows. Draining a headless capture
must not renew the empty-display assumption. This distinction preserves the
existing 77-byte ANSI parity golden and its six dirty cells without rewriting
that independent historical contract.

## Remaining physical-output boundary

The current Context resolves process stdout when it writes. The separate
Context-only fd-pinning experiment does not qualify session mode changes, the
panic hook, restoration or descriptor closure after direct session restore.
A durable physical endpoint must cover the entire managed lease lifecycle, or
those operations must fail before writing. A Context-only output pin is not a
complete repair.

## Required promotion evidence

- Both distinct-writer and internally retargeted-writer A/B/A controls pass as
  ordinary tests against complete VT cells, attributes and cursor.
- Stable fullscreen and inline targets preserve zero-byte settled frames,
  history ordering, resize, color, clipping, damage and recovery assertions.
- Existing native/C/Python ANSI goldens remain unchanged; installed consumers
  separately qualify bindings and output.
- Partial writes, flush failures, caught panics and owner handoffs preserve
  recovery debt without claiming atomic terminal I/O.
- Full CI, consumer compatibility, combined lifecycle review and the physical
  stdout counterexamples pass on one exact candidate before merge or release.
