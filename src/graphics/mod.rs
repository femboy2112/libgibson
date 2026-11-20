//! Experimental pixel-image routes. Currently only the kitty graphics
//! Unicode-placeholder route, designed in `docs/design/v0.9-kitty-graphics-route.md`.
//!
//! The byte and `Surface` layer is implemented (the per-frame plan in `frame` is a RED stub);
//! nothing here is wired into the renderer, and nothing is emitted unless an application opts
//! in.

pub mod frame;
pub mod kitty;
mod kitty_diacritics;
