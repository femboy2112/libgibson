//! Experimental Rust-only semantic UI composition over LibGibson's existing substrate.
//!
//! Components lower to ordinary [`crate::Node`] trees. [`crate::ui::skin::Skin`] adds a visual
//! grammar above [`crate::Theme`]; [`crate::ui::runtime::UiRuntime`] routes typed actions and retains
//! bounded presentation state. The application owns all domain and editor state.
//! [`crate::ui::compile::Compiled`] exposes the lowering boundary for inspection, Scene integration,
//! and local effects. No additional renderer, layout engine, or terminal owner.
//!
//! This API is experimental and excluded from the C ABI and language wrappers.
pub mod compile;
pub mod element;
/// Experimental semantic *experience grammars*: describe an application once as an
/// [`experience::Experience`] and realize it through radically different
/// presentation grammars (cover shelf, cross-media bar, …) without per-style
/// application forks. Sits above [`skin::Skin`]; preserves declared semantics.
pub mod experience;
pub mod interaction;
pub mod motion;
pub mod prelude;
pub mod runtime;
pub mod skin;
pub mod style;
pub use compile::{compile, compile_presented, BuildCx, Compiled, ElementState, PresentationCx};
pub use element::*;
pub use interaction::*;
pub use motion::{MotionPreference, MotionRole};
pub use runtime::*;
pub use skin::{skins, ResolvedSkin, Skin, UiEnvironment};
pub use style::{Density, Elevation, Emphasis, Tone};
