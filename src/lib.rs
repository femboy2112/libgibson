pub mod ansi;
pub mod cell;
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
pub mod surface;

pub use ansi::AnsiCompiler;
pub use cell::{Cell, Color, Glyph, Line, RichText, Span, Style, TextAlign, Theme};
pub use context::Context;
pub use diff::{compute_diff, CellRun, RowPatch, SurfaceDiff};
pub use input::{poll_event, Event, KeyCode, KeyEvent, KeyModifiers, TextInputState};
pub use layout::{compute_layout, wrap_rich_text, wrap_text};
pub use node::{
    AlignItems, Dimension, FlexDirection, JustifyContent, LayoutStyle, Node, NodeKind, WrapMode,
};
pub use painter::{paint, PaintContext};
pub use renderer::{RenderMode, Renderer};
pub use scheduler::{FrameScheduler, RenderStats};
pub use session::TerminalSession;
pub use surface::{BorderType, Rect, Surface};
