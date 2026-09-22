# Third-Party Dependencies and Licenses

LibGibson relies exclusively on permissively licensed open-source libraries:

| Dependency | Crates.io Version | License | Purpose |
|------------|-------------------|---------|---------|
| **taffy** | 0.14.0 | MIT | Flexible Flexbox / UI layout computation |
| **crossterm** | 0.29.0 | MIT | Cross-platform raw mode, input events, and terminal manipulation |
| **unicode-segmentation** | 1.13.3 | MIT / Apache-2.0 | Grapheme cluster boundaries according to Unicode Annex #29 |
| **unicode-width** | 0.2.2 | MIT / Apache-2.0 | Monospace display cell width calculation (Unicode Annex #11) |
| **compact_str** | 0.9.1 | MIT / Apache-2.0 | Zero-allocation inline stack string for grapheme clusters |
| **bitflags** | 2.13.2 | MIT / Apache-2.0 | Bitflags for key modifiers and styling attributes |
| **thiserror** | 2.0.20 | MIT / Apache-2.0 | Idiomatic Rust error definitions |
| **libc** | 0.2.189 | MIT / Apache-2.0 | Standard C library bindings and POSIX types |
| **portable-pty** (dev) | 0.9.0 | MIT | Cross-platform pseudo-terminal interface for integration testing |

All dependencies are distributed under permissive open-source licenses (MIT or dual MIT/Apache-2.0).
No proprietary, GPL-copyleft, or leaked code is included in this repository.
