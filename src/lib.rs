pub mod ansi;
pub mod canvas;
pub mod capability;
pub mod cell;
pub mod clock;
pub mod context;
pub mod diff;
pub mod ffi;
pub mod input;
pub mod layout;
pub mod node;
pub mod painter;
pub mod renderer;
pub mod scheduler;
pub mod session;
pub mod show;
pub mod surface;
pub mod transaction;

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
pub use input::{poll_event, Event, KeyCode, KeyEvent, KeyModifiers, TextInputState};
pub use layout::{compute_layout, wrap_rich_text, wrap_text};
pub use node::{
    AlignItems, Dimension, FlexDirection, JustifyContent, LayoutStyle, Node, NodeKind, WrapMode,
};
pub use painter::{paint, PaintContext};
pub use renderer::{AnchorState, InsertStrategy, RenderMode, Renderer};
pub use scheduler::{FrameScheduler, RenderStats, DEFAULT_ANIMATION_INTERVAL};
pub use session::TerminalSession;
pub use surface::{BorderType, Rect, Surface};
