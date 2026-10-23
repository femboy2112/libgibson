//! A cell-framebuffer terminal UI engine with native history/live-region semantics.
//!
//! **Engineering alpha.** The Linux renderer and established C ABI subset have
//! regression coverage; this is not a stable API or universal terminal promise.
//! Start with [`Context`], declarative [`Node`] content and [`Style`]. Layout,
//! painting, [`Surface`], differential updates and ANSI compilation remain
//! separate layers.
//!
//! **Dense / canvas UIs.** For information-dense visuals, compose *chrome* from
//! declarative [`Node`]s (`col`/`row`/`panel`/`rule`/`text`) and paint the
//! *drawing* imperatively into a [`Surface`]: wrap a ready surface with
//! [`Node::surface`] / [`Node::raster`], or — to build the surface at the size
//! the layout engine assigns rather than guessing it up front — use
//! [`Node::canvas`], whose callback receives its resolved cell rectangle after
//! layout. Either composites through the one rendering pipeline, and viewport
//! clipping is handled by the engine.
//!
//! [`ui`], [`scene`], [`story`], [`surface_fx`], [`raster`], [`raster3d`], [`raster_fx`],
//! [`temporal`] and [`audio`] are experimental Rust-only composition/graphics/audio APIs.
//! They do not own terminal lifecycle. Experimental additions may require source changes
//! between releases.
//!
//! Use one active terminal owner per process. Context/session rendering uses
//! stdout and a process-global panic hook; nested/concurrent terminal contexts and
//! arbitrary host-hook composition are not a supported embedding contract.
//! Explicit restore, Drop and panic cleanup are best effort, not guarantees after
//! abort, SIGKILL, OOM or a disconnected output stream.

pub mod ansi;
/// Experimental Rust-only audio realization axis: DSP synthesis, a deterministic offline
/// renderer, and HumanMusic procedural scoring. Orthogonal to the visual pipeline; not
/// exposed through the C ABI.
pub mod audio;
pub mod canvas;
pub mod capability;
pub mod cell;
pub mod clock;
pub mod context;
pub mod diff;
/// Grouped, reversible edit lifecycles and a bounded undo/redo history
/// (issues #132/#134/#137). Pure: no I/O, no clock, no globals.
pub mod edit;
/// Experimental Rust-only injectable, traceable input sources: inject a scripted
/// event stream, or record and replay a real session (issue #131).
pub mod event_source;
pub mod ffi;
pub mod field;
pub mod focus;
pub mod frame;
pub mod geom;
/// Pointer-drag gesture grouping: one begin / preview* / commit-or-cancel
/// lifecycle built on hit-test capture + world transport (issues #132/#137).
pub mod gesture;
pub mod glitch;
pub mod glyph;
/// Layered spatial hit testing and pointer capture (issue #127).
pub mod hit_test;
pub mod input;
pub mod layout;
pub mod node;
/// Validated numeric editors: schema-checked edits that refuse out-of-range or
/// non-finite input rather than silently clamping an authoritative value (#134).
pub mod numeric;
pub mod painter;
pub mod particles;
pub mod raster;
pub mod raster3d;
pub mod raster_fx;
pub mod renderer;
pub mod replication;
pub mod scene;
pub mod scheduler;
/// An identity-keyed `Selection` of shapes: insertion-ordered, idempotent,
/// keyed by `ShapeId` rather than geometry (issue #132).
pub mod selection;
pub mod session;
/// Immutable 2D world-space shape geometry (rect / line / stroke) with pure,
/// non-mutating exporters: no derivation ever rewrites the authoritative
/// shape (issue #132).
pub mod shape;
pub mod show;
pub mod snapshot;
pub mod story;
pub mod surface;
pub mod surface_fx;
pub mod temporal;
// Internal renderer plumbing (the atomic wire transaction). Crate-private: it has
// no external consumers, is not re-exported, and its stability is the renderer's,
// not a standalone public API. See docs/RELEASE_CONTRACT.md §2.
pub mod plot;
pub(crate) mod transaction;
pub mod transition;
/// Experimental Rust-only semantic components, skins, interaction, and motion.
pub mod ui;
pub mod viewport;
/// Reversible 2D world/view/raster/cell coordinate transport (issue #128).
pub mod world_view;

pub use ansi::AnsiCompiler;
pub use canvas::{braille_oscilloscope, BrailleCanvas, HalfBlockCanvas};
pub use capability::{Capability, ColorDepth, TerminalCapabilities};
pub use cell::{Cell, Color, Glyph, Line, RichText, Span, Style, TextAlign, Theme, ThemeStyles};
pub use clock::{
    ease_in, ease_in_out, ease_out, lerp as lerp_f32, phase, pulse, saw, spring, triangle,
    FixedStepClock, FixedTimestep, RealClock, TickDemand, TimeSource,
};
pub use context::Context;
pub use diff::{compute_diff, CellRun, RowPatch, SurfaceDiff};
pub use edit::{
    CancelReason, Compound, EditError, EditId, EditPhase, EditSession, EvictionStatus, Reversible,
    UndoIntent, UndoKeymap, UndoLog,
};
pub use event_source::{
    EventSource, EventTrace, RawEvent, Recorder, ScriptedInput, ScriptedSource, TerminalSource,
    TraceEntry,
};
pub use field::{
    bayer4_threshold, heat_rgb, plasma, render_field_braille, render_field_braille_dithered,
    render_plasma_halfblock, BAYER4,
};
pub use focus::{FocusId, FocusRing};
pub use geom::{Mesh, Projector, Transform3, Vec3};
pub use gesture::{DragGesture, DragIntent};
pub use glitch::{invert_rect, row_shift, sanitize_wide, scramble_rect, tear};
pub use glyph::{
    detect_glyph_mode, detect_glyph_mode_from_env, transcode_surface_glyphs, GlyphChoice,
    SubcellGlyphMode,
};
pub use input::{
    poll_event, set_focus_reporting, set_mouse_capture, Event, FocusChange, KeyCode, KeyEvent,
    KeyModifiers, MouseButton, MouseEvent, MouseKind, TextInputState,
};
pub use layout::{compute_layout, wrap_rich_text, wrap_text};
pub use node::{
    AlignItems, Dimension, FlexDirection, JustifyContent, LayoutStyle, Node, NodeKind, WrapMode,
};
pub use numeric::{NumericEdit, NumericError, NumericField, NumericInput};
pub use painter::{paint, PaintContext};
pub use particles::{Particle, ParticleSystem, Rng};
pub use renderer::{AnchorState, FrameReport, InsertStrategy, RenderMode, Renderer};
pub use replication::Replication;
pub use scene::{
    Channel, Easing, Effect, EffectBundle, Presentation, ResolvedEntity, Scene, SceneEntity,
    SceneError, SceneId, SceneTarget, TagId,
};
pub use scheduler::{FramePacing, FrameScheduler, RenderStats, DEFAULT_ANIMATION_INTERVAL};
pub use selection::Selection;
pub use session::{TerminalLease, TerminalSession};
pub use shape::{Bounds, Point, Shape, ShapeId, ShapeKind};
pub use story::{
    Beat, Condition, FactValue, Facts, Reaction, Story, StoryAction, StoryDirector, StoryError,
    StoryEvent, StoryTrace, TraceRetention, TraceStep, Transition,
};
pub use surface::{BorderType, Rect, Surface};
pub use surface_fx::{FxMask, SurfaceFx};
pub use temporal::{
    PresentationProfile, ResetPolicy, TemporalBrailleField, TemporalDiagnostics,
    TemporalDisplayProcessor, TemporalGate, TemporalSafetyPolicy,
};
pub use transition::{dissolve, scramble, scramble_line, type_on, SCRAMBLE_GLYPHS};
pub use viewport::ViewportState;
