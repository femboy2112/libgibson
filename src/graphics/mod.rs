//! Experimental pixel-image routes. Currently only the kitty graphics
//! Unicode-placeholder route, designed in `docs/design/v0.9-kitty-graphics-route.md`.
//!
//! **RED slice.** Every item here is an API stub that compiles and does nothing yet;
//! `tests/kitty_graphics.rs` pins the behaviour GREEN must deliver. Nothing here is wired into
//! the renderer, and nothing is emitted unless an application opts in.

pub mod kitty;
