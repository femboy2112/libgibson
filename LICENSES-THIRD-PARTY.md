# Third-Party Dependencies and Licenses

This is a summary of **direct and development dependencies** resolved in
`Cargo.lock`, checked against `cargo metadata --format-version 1 --locked`.
It is not an exhaustive transitive license inventory or a source-provenance audit.
Use Cargo.lock, Cargo metadata, and each dependency's license files when preparing
a distribution.

| Dependency | Crates.io Version | License | Purpose |
|------------|-------------------|---------|---------|
| **taffy** | 0.14.0 | MIT | Flexible Flexbox / UI layout computation |
| **crossterm** | 0.29.0 | MIT | Cross-platform raw mode, input events, and terminal manipulation |
| **unicode-segmentation** | 1.13.3 | MIT / Apache-2.0 | Grapheme cluster boundaries according to Unicode Annex #29 |
| **unicode-width** | 0.2.2 | MIT / Apache-2.0 | Monospace display cell width calculation (Unicode Annex #11) |
| **compact_str** | 0.10.0 | MIT | Zero-allocation inline stack string for grapheme clusters |
| **bitflags** | 2.13.2 | MIT / Apache-2.0 | Bitflags for key modifiers and styling attributes |
| **thiserror** | 2.0.20 | MIT / Apache-2.0 | Idiomatic Rust error definitions |
| **libc** | 0.2.189 | MIT / Apache-2.0 | Standard C library bindings and POSIX types |
| **portable-pty** (dev) | 0.9.0 | MIT | Cross-platform pseudo-terminal interface for integration testing |
| **vt100** (dev) | 0.16.2 | MIT | Terminal parser used for framebuffer reconstruction tests |

The direct dependencies listed here declare permissive licenses. No known
incompatible direct dependency was identified. Transitive dependency versions
and declared licenses come from Cargo.lock / Cargo metadata; this summary does
not establish the provenance of every source line.
