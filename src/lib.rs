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
pub mod renderer;
pub mod scene;
pub mod scheduler;
pub mod session;
pub mod show;
pub mod story;
pub mod surface;
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
pub use scene::{
    Channel, Easing, Effect, EffectBundle, Presentation, ResolvedEntity, Scene, SceneEntity,
    SceneId, SceneTarget, TagId,
};
pub use scheduler::{FrameScheduler, RenderStats, DEFAULT_ANIMATION_INTERVAL};
pub use session::TerminalSession;
pub use story::{
    Beat, Condition, FactValue, Facts, Story, StoryAction, StoryDirector, StoryEvent, StoryTrace,
    Transition,
};
pub use surface::{BorderType, Rect, Surface};
pub use transition::{dissolve, scramble, scramble_line, type_on, SCRAMBLE_GLYPHS};
pub use viewport::ViewportState;
