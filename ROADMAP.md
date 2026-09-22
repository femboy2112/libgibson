# LibGibson Architectural Roadmap

This document outlines the planned future milestones and next architectural frontiers for LibGibson.

---

## Phase 1: Core Engine & Minimal Vertical Slice (Completed)

- [x] Clean-room cell framebuffer architecture
- [x] Compact, stack-allocated grapheme cluster model (`CompactString`)
- [x] Unicode width measurement and wide glyph overwrite protection
- [x] Taffy Flexbox integration with custom text measurement
- [x] Declarative UI node tree (Box, Row, Column, Text, Border, Spinner, TextInput)
- [x] Differential diff engine with run coalescing and erase-to-end-of-line (`CSI K`)
- [x] Stateful minimal ANSI escape sequence compiler
- [x] Flagship Inline Mode with $O(1)$ immutable scrollback commit semantics
- [x] Fullscreen alternate-buffer mode reusing the same rendering pipeline
- [x] Frame scheduler with coalescing, FPS budget throttling, and telemetry metrics
- [x] Grapheme-aware text input component with navigation and bracketed paste
- [x] RAII terminal lifecycle guard and global panic hook restoration
- [x] Non-interactive / CI redirection detection and plain text degradation
- [x] Stable `extern "C"` ABI with opaque handles and panic containment
- [x] Native bindings and verified examples for C, C++, Python, and Go
- [x] Real PTY integration testing with `portable-pty`

---

## Phase 1.5: Agent-Class Typography & Interactive Primitives (Completed)

- [x] **Chrome Primitives**: `Node::rule` (horizontal divider with optional title) and `Node::rail` (left-border callout).
- [x] **Typography & Hierarchy System**: `Span`, `Line`, `RichText`, and semantic `Theme` tokens with zero raw SGR escape codes.
- [x] **Asynchronous Scrollback Insertion (`insert_before_live`)**: Insert events into native scrollback above active live prompts without dropping frames or triggering full repaints.
- [x] **Display-Width-Aware TextInput**: Proper horizontal scrolling, wide CJK, and emoji display-width calculations.
- [x] **Right-Margin Autowrap Protection**: DECAWM `\x1b[?7l` disabling and right-boundary wide glyph clipping.
- [x] **Responsive Layout Sizing**: Percentage width (`percent_width`), min/max dimensions, and per-side padding.
- [x] **Virtual Terminal Screen State Verification**: `vt100` parser automated tests verifying exact screen character grids and cursor positions.
- [x] **Rapid Resize & Narrow Terminal Torture Tests**: Zero panics, bounded inline heights, and valid cursor coordinates across cyclic resizing down to 10 columns.
- [x] **Flagship Polished Agent CLI Demo**: Restrained, typography-driven agent interface with rule header, rail callouts, live spinner, permission selector, and zero-escape non-TTY redirection.

---

## Phase 2: Input Protocols & Interaction Enhancements

- [ ] **Kitty Keyboard Protocol**: Support progressive enhancement for disambiguated escape keys, key release events, and modifier combinations.
- [ ] **Focus Management Tree**: Hierarchical focus tree with Tab / Shift-Tab cycling and focus restoration.
- [ ] **Event Bubbling and Capture**: Structured event dispatch pipeline allowing parent containers to intercept or bubble user events.
- [ ] **Mouse Tracking**: Optional SGR mouse reporting (`CSI ? 1006 h`) for click-to-focus, scroll wheel handling, and selection.
- [ ] **OSC 8 Terminal Hyperlinks**: Embedded clickable URLs in Text nodes with fallback plain-text formatting.

---

## Phase 3: Advanced Layout & Rich Components

- [ ] **Incremental Layout Caching**: Cache Taffy layout subtrees across frames when node contents are unmodified.
- [ ] **Scrollable Viewport Widgets**: Scrollable virtual boxes with vertical and horizontal scrollbars.
- [ ] **Virtualization Engine**: Virtual list and table rendering supporting datasets with millions of rows without memory pressure.
- [ ] **Rich Component Library**:
  - Tables with auto-sizing columns and alignment.
  - Progress bars with smooth Unicode fraction characters (`▏▎▍▌▋▊▉█`).
  - Tree views with expandable/collapsible nodes.
  - Tab headers and segmented controls.
- [ ] **Markdown and Syntax Highlighting**: Streaming Markdown parser with Syntect or Tree-sitter token colorization.

---

## Phase 4: Styling, Themes, and Accessibility

- [ ] **24-bit Truecolor Palettes and Themes**: Support CSS-like theme definitions with automatic fallback to ANSI-256 or 16-color ANSI.
- [ ] **Terminal Capability Negotiation**: Automatic detection via Primary and Secondary Device Attributes (`CSI c`, `CSI > c`) for synchronized output, color depth, and graphics protocols.
- [ ] **Accessibility (A11y)**: Screen reader annotations, semantic headings, and ARIA-like terminal roles for assistive technology.

---

## Phase 5: Protocol Extensions & Hardening

- [ ] **Terminal Graphics Protocols**: Support inline image rendering via Kitty graphics protocol, iTerm2 inline images, and Sixel graphics.
- [ ] **Windows ConPTY Torture Testing**: Extended automated testing under Windows Console API and ConPTY.
- [ ] **Multiplexer & Remote Shell Hardening**: Specialized test matrix for tmux, screen, and SSH connections over high-latency networks.
- [ ] **Property & Fuzz Testing**: AFL / `cargo-fuzz` test suite exercising arbitrary Unicode sequences, invalid ANSI input, and rapid terminal resizes.
