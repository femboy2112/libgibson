//! A cell-framebuffer terminal UI engine with native history/live-region semantics.
//!
//! **Engineering alpha.** The Linux renderer and established C ABI subset have
//! regression coverage; this is not a stable API or universal terminal promise.
//! Start with [`Context`], declarative [`Node`] content and [`Style`]. Layout,
//! painting, [`Surface`], differential updates and ANSI compilation remain
//! separate layers.
//!
//! [`scene`], [`story`], [`surface_fx`], [`raster`], [`raster3d`] and [`raster_fx`]
//! are experimental Rust-only composition/graphics APIs. They do not own terminal
//! lifecycle. Experimental additions may require source changes between releases.
//!
//! Use one active terminal owner per process. Context/session rendering uses
//! stdout and a process-global panic hook; nested/concurrent terminal contexts and
//! arbitrary host-hook composition are not a supported embedding contract.
//! Explicit restore, Drop and panic cleanup are best effort, not guarantees after
//! abort, SIGKILL, OOM or a disconnected output stream.

pub mod ansi;
pub mod canvas;
pub mod capability;
pub mod cell;
pub mod clock;
pub mod context;
pub mod diff;
pub mod ffi;
pub mod field;
pub mod focus;
pub mod geom;
pub mod glitch;
pub mod input;
pub mod layout;
pub mod node;
pub mod painter;
pub mod particles;
pub mod raster;
pub mod raster3d;
pub mod raster_fx;
pub mod renderer;
pub mod replication;
pub mod scene;
pub mod scheduler;
pub mod session;
pub mod show;
pub mod story;
pub mod surface;
pub mod surface_fx;
pub mod transaction;
pub mod transition;
pub mod viewport;

pub use ansi::AnsiCompiler;
pub use canvas::{braille_oscilloscope, BrailleCanvas, HalfBlockCanvas};
pub use capability::{Capability, ColorDepth, TerminalCapabilities};
pub use cell::{Cell, Color, Glyph, Line, RichText, Span, Style, TextAlign, Theme, ThemeStyles};
pub use clock::{
    ease_in, ease_in_out, ease_out, lerp as lerp_f32, phase, pulse, saw, spring, triangle,
    FixedStepClock, RealClock, TimeSource,
};
pub use context::Context;
pub use diff::{compute_diff, CellRun, RowPatch, SurfaceDiff};
pub use field::{
    bayer4_threshold, heat_rgb, plasma, render_field_braille, render_field_braille_dithered,
    render_plasma_halfblock, BAYER4,
};
pub use focus::{FocusId, FocusRing};
pub use geom::{Mesh, Projector, Transform3, Vec3};
pub use glitch::{invert_rect, row_shift, sanitize_wide, scramble_rect, tear};
pub use input::{poll_event, Event, KeyCode, KeyEvent, KeyModifiers, TextInputState};
pub use layout::{compute_layout, wrap_rich_text, wrap_text};
pub use node::{
    AlignItems, Dimension, FlexDirection, JustifyContent, LayoutStyle, Node, NodeKind, WrapMode,
};
pub use painter::{paint, PaintContext};
pub use particles::{Particle, ParticleSystem, Rng};
pub use renderer::{AnchorState, InsertStrategy, RenderMode, Renderer};
pub use replication::Replication;
pub use scene::{
    Channel, Easing, Effect, EffectBundle, Presentation, ResolvedEntity, Scene, SceneEntity,
    SceneError, SceneId, SceneTarget, TagId,
};
pub use scheduler::{FrameScheduler, RenderStats, DEFAULT_ANIMATION_INTERVAL};
pub use session::{TerminalLease, TerminalSession};
pub use story::{
    Beat, Condition, FactValue, Facts, Reaction, Story, StoryAction, StoryDirector, StoryError,
    StoryEvent, StoryTrace, TraceRetention, TraceStep, Transition,
};
pub use surface::{BorderType, Rect, Surface};
pub use surface_fx::{FxMask, SurfaceFx};
pub use transition::{dissolve, scramble, scramble_line, type_on, SCRAMBLE_GLYPHS};
pub use viewport::ViewportState;
