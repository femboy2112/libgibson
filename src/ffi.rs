#![allow(clippy::missing_safety_doc)]

//! C ABI for LibGibson.
//!
//! ## ABI safety rules
//!
//! 1. **No internal Rust struct is exposed directly.** The public `gibson_stats_t`
//!    layout is a dedicated `#[repr(C)]` type, not `crate::scheduler::RenderStats`.
//! 2. **No `#[repr(C)]` enum crosses the ABI.** Every enum-like value that can be
//!    produced by foreign code is transported as a raw `i32` and validated
//!    explicitly. Constructing an invalid Rust enum discriminant from foreign
//!    memory is UB that `catch_unwind` cannot repair.
//! 3. **Versioned structs with an exact-version contract.** `gibson_stats_t`
//!    begins with `struct_size` and `abi_version`.
//!
//!    The contract is deliberately **one** of the two possible strategies, not
//!    both: [`GIBSON_ABI_VERSION`] is bumped whenever the struct layout changes,
//!    and callers must pass a buffer at least as large as
//!    `sizeof(gibson_stats_t)`. The engine checks `abi_version` for equality and
//!    `struct_size >= sizeof(gibson_stats_t)` before writing, then writes exactly
//!    `min(struct_size, sizeof(gibson_stats_t))` bytes. A caller with a larger
//!    buffer is therefore safe and its trailing bytes are left untouched; a
//!    caller built against an *older, smaller* struct is rejected with
//!    `GIBSON_ERR_INVALID_PARAM` rather than silently truncated.
//!
//!    (True append-compatibility — accepting smaller callers and writing a
//!    prefix — is intentionally *not* claimed.)

use crate::cell::{Color, Line, RichText, Span, Style, TextAlign};
use crate::context::Context;
use crate::input::{Event, KeyCode};
use crate::node::{Node, WrapMode};
use crate::renderer::RenderMode;
use crate::scheduler::RenderStats;
use crate::surface::BorderType;
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;
use std::time::Duration;

/// Current ABI version. Bump on any breaking layout/semantic change.
pub const GIBSON_ABI_VERSION: u32 = 1;

// ---------------------- Validated raw enum-like constants ----------------------

pub const GIBSON_MODE_INLINE: i32 = 0;
pub const GIBSON_MODE_FULLSCREEN: i32 = 1;

pub const GIBSON_BORDER_SINGLE: i32 = 0;
pub const GIBSON_BORDER_DOUBLE: i32 = 1;
pub const GIBSON_BORDER_ROUNDED: i32 = 2;
pub const GIBSON_BORDER_THICK: i32 = 3;
pub const GIBSON_BORDER_ASCII: i32 = 4;

pub const GIBSON_WRAP_NONE: i32 = 0;
pub const GIBSON_WRAP_CHAR: i32 = 1;
pub const GIBSON_WRAP_WORD: i32 = 2;

pub const GIBSON_COLOR_RESET: i32 = 0;
pub const GIBSON_COLOR_BLACK: i32 = 1;
pub const GIBSON_COLOR_RED: i32 = 2;
pub const GIBSON_COLOR_GREEN: i32 = 3;
pub const GIBSON_COLOR_YELLOW: i32 = 4;
pub const GIBSON_COLOR_BLUE: i32 = 5;
pub const GIBSON_COLOR_MAGENTA: i32 = 6;
pub const GIBSON_COLOR_CYAN: i32 = 7;
pub const GIBSON_COLOR_WHITE: i32 = 8;
pub const GIBSON_COLOR_BRIGHT_BLACK: i32 = 9;
pub const GIBSON_COLOR_BRIGHT_RED: i32 = 10;
pub const GIBSON_COLOR_BRIGHT_GREEN: i32 = 11;
pub const GIBSON_COLOR_BRIGHT_YELLOW: i32 = 12;
pub const GIBSON_COLOR_BRIGHT_BLUE: i32 = 13;
pub const GIBSON_COLOR_BRIGHT_MAGENTA: i32 = 14;
pub const GIBSON_COLOR_BRIGHT_CYAN: i32 = 15;
pub const GIBSON_COLOR_BRIGHT_WHITE: i32 = 16;
pub const GIBSON_COLOR_ANSI256: i32 = 17;
pub const GIBSON_COLOR_RGB: i32 = 18;

pub const GIBSON_ALIGN_LEFT: i32 = 0;
pub const GIBSON_ALIGN_CENTER: i32 = 1;
pub const GIBSON_ALIGN_RIGHT: i32 = 2;

pub const GIBSON_EVENT_NONE: i32 = 0;
pub const GIBSON_EVENT_KEY: i32 = 1;
pub const GIBSON_EVENT_PASTE: i32 = 2;
pub const GIBSON_EVENT_RESIZE: i32 = 3;

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

fn set_last_error(msg: String) {
    LAST_ERROR.with(|e| *e.borrow_mut() = msg);
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GibsonStatus {
    Ok = 0,
    ErrInvalidParam = -1,
    ErrIo = -2,
    ErrPanic = -3,
    ErrNotInitialized = -4,
}

/// FFI-safe color descriptor. `color_type` is a raw `i32` validated on entry.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GibsonColor {
    /// One of the `GIBSON_COLOR_*` constants.
    pub color_type: i32,
    /// Also used as the ANSI-256 index.
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub _pad: u8,
}

impl Default for GibsonColor {
    fn default() -> Self {
        Self {
            color_type: GIBSON_COLOR_RESET,
            r: 0,
            g: 0,
            b: 0,
            _pad: 0,
        }
    }
}

/// Validates a foreign-supplied color. Returns `Err` for unknown discriminants.
pub fn color_from_ffi(c: &GibsonColor) -> Result<Option<Color>, GibsonStatus> {
    Ok(match c.color_type {
        GIBSON_COLOR_RESET => None,
        GIBSON_COLOR_BLACK => Some(Color::Black),
        GIBSON_COLOR_RED => Some(Color::Red),
        GIBSON_COLOR_GREEN => Some(Color::Green),
        GIBSON_COLOR_YELLOW => Some(Color::Yellow),
        GIBSON_COLOR_BLUE => Some(Color::Blue),
        GIBSON_COLOR_MAGENTA => Some(Color::Magenta),
        GIBSON_COLOR_CYAN => Some(Color::Cyan),
        GIBSON_COLOR_WHITE => Some(Color::White),
        GIBSON_COLOR_BRIGHT_BLACK => Some(Color::BrightBlack),
        GIBSON_COLOR_BRIGHT_RED => Some(Color::BrightRed),
        GIBSON_COLOR_BRIGHT_GREEN => Some(Color::BrightGreen),
        GIBSON_COLOR_BRIGHT_YELLOW => Some(Color::BrightYellow),
        GIBSON_COLOR_BRIGHT_BLUE => Some(Color::BrightBlue),
        GIBSON_COLOR_BRIGHT_MAGENTA => Some(Color::BrightMagenta),
        GIBSON_COLOR_BRIGHT_CYAN => Some(Color::BrightCyan),
        GIBSON_COLOR_BRIGHT_WHITE => Some(Color::BrightWhite),
        GIBSON_COLOR_ANSI256 => Some(Color::Ansi256(c.r)),
        GIBSON_COLOR_RGB => Some(Color::Rgb(c.r, c.g, c.b)),
        other => {
            set_last_error(format!("Invalid color type: {}", other));
            return Err(GibsonStatus::ErrInvalidParam);
        }
    })
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GibsonStyle {
    pub fg: GibsonColor,
    pub bg: GibsonColor,
    pub bold: u8,
    pub dim: u8,
    pub italic: u8,
    pub underline: u8,
    pub reverse: u8,
    pub _reserved: [u8; 3],
}

/// Validates a foreign-supplied style (including its embedded colors).
pub fn style_from_ffi(s: &GibsonStyle) -> Result<Style, GibsonStatus> {
    Ok(Style {
        fg: color_from_ffi(&s.fg)?,
        bg: color_from_ffi(&s.bg)?,
        bold: s.bold != 0,
        dim: s.dim != 0,
        italic: s.italic != 0,
        underline: s.underline != 0,
        reverse: s.reverse != 0,
    })
}

/// Reads an optional style pointer, defaulting to plain when null.
unsafe fn read_style(style: *const GibsonStyle) -> Result<Style, GibsonStatus> {
    if style.is_null() {
        Ok(Style::default())
    } else {
        style_from_ffi(&*style)
    }
}

#[repr(C)]
pub struct GibsonEvent {
    /// One of the `GIBSON_EVENT_*` constants (written by the engine).
    pub event_type: i32,
    pub key_code: u32,
    pub modifiers: u8,
    pub _pad: [u8; 3],
    pub cols: u16,
    pub rows: u16,
    pub paste_str: *const c_char,
}

pub const GIBSON_KEY_ENTER: u32 = 0x1001;
pub const GIBSON_KEY_LEFT: u32 = 0x1002;
pub const GIBSON_KEY_RIGHT: u32 = 0x1003;
pub const GIBSON_KEY_UP: u32 = 0x1004;
pub const GIBSON_KEY_DOWN: u32 = 0x1005;
pub const GIBSON_KEY_HOME: u32 = 0x1006;
pub const GIBSON_KEY_END: u32 = 0x1007;
pub const GIBSON_KEY_BACKSPACE: u32 = 0x1008;
pub const GIBSON_KEY_DELETE: u32 = 0x1009;
pub const GIBSON_KEY_ESC: u32 = 0x100A;
pub const GIBSON_KEY_TAB: u32 = 0x100B;
pub const GIBSON_KEY_PAGE_UP: u32 = 0x100C;
pub const GIBSON_KEY_PAGE_DOWN: u32 = 0x100D;

/// FFI-safe, versioned statistics snapshot.
///
/// Callers must initialize `struct_size` to `sizeof(gibson_stats_t)` and
/// `abi_version` to `GIBSON_ABI_VERSION` (see `gibson_stats_init`). The engine
/// writes at most `struct_size` bytes and never overflows the caller's buffer.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GibsonStats {
    pub struct_size: u32,
    pub abi_version: u32,
    pub frames: u64,
    pub skipped_frames: u64,
    pub dirty_cells: u64,
    pub total_cells: u64,
    pub frame_bytes: u64,
    pub full_repaints: u64,
    pub last_render_duration_micros: u64,
    pub history_insertions: u64,
    pub insertion_repaints: u64,
    pub fast_insertions: u64,
    pub insertion_bytes: u64,
    pub anchor_resyncs: u64,
    pub commit_bytes: u64,
    pub control_bytes: u64,
}

impl GibsonStats {
    fn from_render_stats(s: &RenderStats) -> Self {
        Self {
            struct_size: std::mem::size_of::<GibsonStats>() as u32,
            abi_version: GIBSON_ABI_VERSION,
            frames: s.frames,
            skipped_frames: s.skipped_frames,
            dirty_cells: s.dirty_cells,
            total_cells: s.total_cells,
            frame_bytes: s.frame_bytes,
            full_repaints: s.full_repaints,
            last_render_duration_micros: s.last_render_duration_micros,
            history_insertions: s.history_insertions,
            insertion_repaints: s.insertion_repaints,
            fast_insertions: s.fast_insertions,
            insertion_bytes: s.insertion_bytes,
            anchor_resyncs: s.anchor_resyncs,
            commit_bytes: s.commit_bytes,
            control_bytes: s.control_bytes,
        }
    }
}

pub struct GibsonContextOpaque {
    pub inner: Context,
    last_paste_cstring: Option<CString>,
}

pub struct GibsonNodeOpaque {
    pub inner: Node,
}

pub struct GibsonLineOpaque {
    spans: Vec<Span>,
    align: TextAlign,
}

pub struct GibsonRichTextOpaque {
    lines: Vec<Line>,
}

// ---------------------- Validation helpers ----------------------

fn validate_mode(mode: i32) -> Result<RenderMode, GibsonStatus> {
    match mode {
        GIBSON_MODE_INLINE => Ok(RenderMode::Inline),
        GIBSON_MODE_FULLSCREEN => Ok(RenderMode::Fullscreen),
        other => {
            set_last_error(format!("Invalid render mode: {}", other));
            Err(GibsonStatus::ErrInvalidParam)
        }
    }
}

fn validate_border(b: i32) -> Result<BorderType, GibsonStatus> {
    match b {
        GIBSON_BORDER_SINGLE => Ok(BorderType::Single),
        GIBSON_BORDER_DOUBLE => Ok(BorderType::Double),
        GIBSON_BORDER_ROUNDED => Ok(BorderType::Rounded),
        GIBSON_BORDER_THICK => Ok(BorderType::Thick),
        GIBSON_BORDER_ASCII => Ok(BorderType::Ascii),
        other => {
            set_last_error(format!("Invalid border type: {}", other));
            Err(GibsonStatus::ErrInvalidParam)
        }
    }
}

fn validate_wrap(w: i32) -> Result<WrapMode, GibsonStatus> {
    match w {
        GIBSON_WRAP_NONE => Ok(WrapMode::NoWrap),
        GIBSON_WRAP_CHAR => Ok(WrapMode::CharWrap),
        GIBSON_WRAP_WORD => Ok(WrapMode::WordWrap),
        other => {
            set_last_error(format!("Invalid wrap mode: {}", other));
            Err(GibsonStatus::ErrInvalidParam)
        }
    }
}

fn validate_align(a: i32) -> Result<TextAlign, GibsonStatus> {
    match a {
        GIBSON_ALIGN_LEFT => Ok(TextAlign::Left),
        GIBSON_ALIGN_CENTER => Ok(TextAlign::Center),
        GIBSON_ALIGN_RIGHT => Ok(TextAlign::Right),
        other => {
            set_last_error(format!("Invalid alignment: {}", other));
            Err(GibsonStatus::ErrInvalidParam)
        }
    }
}

unsafe fn read_utf8<'a>(p: *const c_char, what: &str) -> Result<&'a str, GibsonStatus> {
    if p.is_null() {
        set_last_error(format!("Null {} pointer", what));
        return Err(GibsonStatus::ErrInvalidParam);
    }
    match CStr::from_ptr(p).to_str() {
        Ok(s) => Ok(s),
        Err(_) => {
            set_last_error(format!("Invalid UTF-8 in {}", what));
            Err(GibsonStatus::ErrInvalidParam)
        }
    }
}

// ---------------------- FFI Functions ----------------------

/// Returns the runtime ABI version. Compare against `GIBSON_ABI_VERSION`.
#[no_mangle]
pub extern "C" fn gibson_abi_version() -> u32 {
    GIBSON_ABI_VERSION
}

/// Initializes a `gibson_stats_t` header before passing it to `gibson_get_stats`.
#[no_mangle]
pub unsafe extern "C" fn gibson_stats_init(out: *mut GibsonStats) {
    if !out.is_null() {
        (*out).struct_size = std::mem::size_of::<GibsonStats>() as u32;
        (*out).abi_version = GIBSON_ABI_VERSION;
    }
}

#[no_mangle]
pub unsafe extern "C" fn gibson_create_context(
    mode: i32,
    out: *mut *mut GibsonContextOpaque,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if out.is_null() {
            set_last_error("Null pointer provided for out context".into());
            return GibsonStatus::ErrInvalidParam;
        }
        let m = match validate_mode(mode) {
            Ok(m) => m,
            Err(e) => return e,
        };

        match Context::new(m) {
            Ok(ctx) => {
                let boxed = Box::new(GibsonContextOpaque {
                    inner: ctx,
                    last_paste_cstring: None,
                });
                *out = Box::into_raw(boxed);
                GibsonStatus::Ok
            }
            Err(e) => {
                set_last_error(e.to_string());
                GibsonStatus::ErrIo
            }
        }
    }));

    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_destroy_context(ctx: *mut GibsonContextOpaque) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !ctx.is_null() {
            let mut boxed = Box::from_raw(ctx);
            let _ = boxed.inner.restore();
        }
    }));
}

#[no_mangle]
pub unsafe extern "C" fn gibson_set_sync_updates(
    ctx: *mut GibsonContextOpaque,
    enabled: i32,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        (*ctx).inner.set_sync_updates(enabled != 0);
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_set_max_fps(
    ctx: *mut GibsonContextOpaque,
    fps: u32,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        (*ctx).inner.set_max_fps(fps);
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_set_root_node(
    ctx: *mut GibsonContextOpaque,
    node: *mut GibsonNodeOpaque,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() || node.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        // Take ownership of the node tree
        let boxed_node = Box::from_raw(node);
        (*ctx).inner.set_root(boxed_node.inner);
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_render(ctx: *mut GibsonContextOpaque) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        match (*ctx).inner.render() {
            Ok(_) => GibsonStatus::Ok,
            Err(e) => {
                set_last_error(e.to_string());
                GibsonStatus::ErrIo
            }
        }
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

/// Commits structured plain text (width-aware, control characters neutralized).
#[no_mangle]
pub unsafe extern "C" fn gibson_commit_text(
    ctx: *mut GibsonContextOpaque,
    utf8_text: *const c_char,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let s = match read_utf8(utf8_text, "text") {
            Ok(s) => s,
            Err(e) => return e,
        };
        match (*ctx).inner.commit_text(s) {
            Ok(_) => GibsonStatus::Ok,
            Err(e) => {
                set_last_error(e.to_string());
                GibsonStatus::ErrIo
            }
        }
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

/// Backwards-compatible alias for [`gibson_commit_text`]. Prefer the explicit name.
#[no_mangle]
pub unsafe extern "C" fn gibson_commit(
    ctx: *mut GibsonContextOpaque,
    utf8_text: *const c_char,
) -> GibsonStatus {
    gibson_commit_text(ctx, utf8_text)
}

/// Raw ANSI escape hatch: the payload is written verbatim.
#[no_mangle]
pub unsafe extern "C" fn gibson_commit_raw_ansi_unchecked(
    ctx: *mut GibsonContextOpaque,
    utf8_text: *const c_char,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let s = match read_utf8(utf8_text, "text") {
            Ok(s) => s,
            Err(e) => return e,
        };
        match (*ctx).inner.commit_raw_ansi_unchecked(s) {
            Ok(_) => GibsonStatus::Ok,
            Err(e) => {
                set_last_error(e.to_string());
                GibsonStatus::ErrIo
            }
        }
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_render_if_due(
    ctx: *mut GibsonContextOpaque,
    out_rendered: *mut i32,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        match (*ctx).inner.render_if_due() {
            Ok(rendered) => {
                if !out_rendered.is_null() {
                    *out_rendered = if rendered { 1 } else { 0 };
                }
                GibsonStatus::Ok
            }
            Err(e) => {
                set_last_error(e.to_string());
                GibsonStatus::ErrIo
            }
        }
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_request_render(ctx: *mut GibsonContextOpaque) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        (*ctx).inner.request_render();
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

/// Inserts **raw** UTF-8 text into scrollback above the active live region.
///
/// The text is split on newlines and written as a terminal byte stream with
/// **no sanitization**: embedded escape/OSC/CSI sequences reach the terminal.
/// Use [`gibson_insert_text_before_live`] for untrusted text.
#[no_mangle]
pub unsafe extern "C" fn gibson_insert_raw_lines_before_live_unchecked(
    ctx: *mut GibsonContextOpaque,
    utf8_text: *const c_char,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let s = match read_utf8(utf8_text, "text") {
            Ok(s) => s,
            Err(e) => return e,
        };
        let lines: Vec<&str> = s.lines().collect();
        match (*ctx).inner.insert_raw_lines_before_live_unchecked(&lines) {
            Ok(_) => GibsonStatus::Ok,
            Err(e) => {
                set_last_error(e.to_string());
                GibsonStatus::ErrIo
            }
        }
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

/// Inserts safe, width-aware plain text into scrollback above the active live
/// region. Terminal control characters are neutralized.
#[no_mangle]
pub unsafe extern "C" fn gibson_insert_text_before_live(
    ctx: *mut GibsonContextOpaque,
    utf8_text: *const c_char,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let s = match read_utf8(utf8_text, "text") {
            Ok(s) => s,
            Err(e) => return e,
        };
        match (*ctx).inner.insert_text_before_live(s) {
            Ok(_) => GibsonStatus::Ok,
            Err(e) => {
                set_last_error(e.to_string());
                GibsonStatus::ErrIo
            }
        }
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_commit_node(
    ctx: *mut GibsonContextOpaque,
    node: *mut GibsonNodeOpaque,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() || node.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let mut boxed = Box::from_raw(node);
        match (*ctx).inner.commit_node(&mut boxed.inner) {
            Ok(_) => GibsonStatus::Ok,
            Err(e) => {
                set_last_error(e.to_string());
                GibsonStatus::ErrIo
            }
        }
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_insert_node_before_live(
    ctx: *mut GibsonContextOpaque,
    node: *mut GibsonNodeOpaque,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() || node.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let mut boxed = Box::from_raw(node);
        match (*ctx).inner.insert_node_before_live(&mut boxed.inner) {
            Ok(_) => GibsonStatus::Ok,
            Err(e) => {
                set_last_error(e.to_string());
                GibsonStatus::ErrIo
            }
        }
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_clear_live_region(ctx: *mut GibsonContextOpaque) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        match (*ctx).inner.clear_live_region() {
            Ok(_) => GibsonStatus::Ok,
            Err(e) => {
                set_last_error(e.to_string());
                GibsonStatus::ErrIo
            }
        }
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_poll_event(
    ctx: *mut GibsonContextOpaque,
    timeout_ms: u32,
    out_event: *mut GibsonEvent,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() || out_event.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }

        (*out_event).event_type = GIBSON_EVENT_NONE;
        (*out_event).key_code = 0;
        (*out_event).modifiers = 0;
        (*out_event).cols = 0;
        (*out_event).rows = 0;
        (*out_event).paste_str = ptr::null();

        match (*ctx)
            .inner
            .poll_event(Duration::from_millis(timeout_ms as u64))
        {
            Ok(Some(ev)) => {
                match ev {
                    Event::Key(k) => {
                        (*out_event).event_type = GIBSON_EVENT_KEY;
                        (*out_event).modifiers = k.modifiers.bits();
                        (*out_event).key_code = match k.code {
                            KeyCode::Char(c) => c as u32,
                            KeyCode::Enter => GIBSON_KEY_ENTER,
                            KeyCode::Left => GIBSON_KEY_LEFT,
                            KeyCode::Right => GIBSON_KEY_RIGHT,
                            KeyCode::Up => GIBSON_KEY_UP,
                            KeyCode::Down => GIBSON_KEY_DOWN,
                            KeyCode::Home => GIBSON_KEY_HOME,
                            KeyCode::End => GIBSON_KEY_END,
                            KeyCode::Backspace => GIBSON_KEY_BACKSPACE,
                            KeyCode::Delete => GIBSON_KEY_DELETE,
                            KeyCode::Esc => GIBSON_KEY_ESC,
                            KeyCode::Tab => GIBSON_KEY_TAB,
                            KeyCode::BackTab => GIBSON_KEY_TAB,
                            KeyCode::PageUp => GIBSON_KEY_PAGE_UP,
                            KeyCode::PageDown => GIBSON_KEY_PAGE_DOWN,
                        };
                    }
                    Event::Paste(s) => {
                        (*out_event).event_type = GIBSON_EVENT_PASTE;
                        if let Ok(cs) = CString::new(s) {
                            (*out_event).paste_str = cs.as_ptr();
                            (*ctx).last_paste_cstring = Some(cs);
                        }
                    }
                    Event::Resize(cols, rows) => {
                        (*out_event).event_type = GIBSON_EVENT_RESIZE;
                        (*out_event).cols = cols;
                        (*out_event).rows = rows;
                    }
                    Event::Tick => {}
                }
                GibsonStatus::Ok
            }
            Ok(None) => GibsonStatus::Ok,
            Err(e) => {
                set_last_error(e.to_string());
                GibsonStatus::ErrIo
            }
        }
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_get_stats(
    ctx: *mut GibsonContextOpaque,
    out_stats: *mut GibsonStats,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() || out_stats.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }

        let out = &mut *out_stats;
        // Reject uninitialized or incompatible callers *before* writing anything.
        if out.abi_version != GIBSON_ABI_VERSION {
            set_last_error(format!(
                "ABI version mismatch: caller {} engine {}",
                out.abi_version, GIBSON_ABI_VERSION
            ));
            return GibsonStatus::ErrInvalidParam;
        }
        if (out.struct_size as usize) < std::mem::size_of::<GibsonStats>() {
            set_last_error(format!(
                "gibson_stats_t too small: caller {} engine {}",
                out.struct_size,
                std::mem::size_of::<GibsonStats>()
            ));
            return GibsonStatus::ErrInvalidParam;
        }

        let full = GibsonStats::from_render_stats(&(*ctx).inner.stats());
        let copy_len = (out.struct_size as usize).min(std::mem::size_of::<GibsonStats>());
        ptr::copy_nonoverlapping(
            &full as *const GibsonStats as *const u8,
            out_stats as *mut u8,
            copy_len,
        );
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

// ---------------------- Node Creation FFI ----------------------

#[no_mangle]
pub unsafe extern "C" fn gibson_node_box_col(out: *mut *mut GibsonNodeOpaque) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if out.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        *out = Box::into_raw(Box::new(GibsonNodeOpaque { inner: Node::col() }));
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_box_row(out: *mut *mut GibsonNodeOpaque) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if out.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        *out = Box::into_raw(Box::new(GibsonNodeOpaque { inner: Node::row() }));
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_stack(out: *mut *mut GibsonNodeOpaque) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if out.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        *out = Box::into_raw(Box::new(GibsonNodeOpaque {
            inner: Node::stack(),
        }));
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

/// Creates a dim-veil node (style-only overlay when composited in a stack).
#[no_mangle]
pub unsafe extern "C" fn gibson_node_dim(out: *mut *mut GibsonNodeOpaque) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if out.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        *out = Box::into_raw(Box::new(GibsonNodeOpaque { inner: Node::dim() }));
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_text(
    utf8_text: *const c_char,
    style: *const GibsonStyle,
    wrap: i32,
    out: *mut *mut GibsonNodeOpaque,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if out.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let text = match read_utf8(utf8_text, "text") {
            Ok(s) => s,
            Err(e) => return e,
        };
        let st = match read_style(style) {
            Ok(s) => s,
            Err(e) => return e,
        };
        let wm = match validate_wrap(wrap) {
            Ok(w) => w,
            Err(e) => return e,
        };
        let node = Node::text_wrapped(text, st, wm);
        *out = Box::into_raw(Box::new(GibsonNodeOpaque { inner: node }));
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_spinner(
    frame_index: u32,
    style: *const GibsonStyle,
    label: *const c_char,
    out: *mut *mut GibsonNodeOpaque,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if out.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let st = match read_style(style) {
            Ok(s) => s,
            Err(e) => return e,
        };
        let lbl_str = if label.is_null() {
            None
        } else {
            match CStr::from_ptr(label).to_str() {
                Ok(s) => Some(s),
                Err(_) => {
                    set_last_error("Invalid UTF-8 in spinner label".into());
                    return GibsonStatus::ErrInvalidParam;
                }
            }
        };
        let node = Node::spinner(frame_index as usize, st, lbl_str);
        *out = Box::into_raw(Box::new(GibsonNodeOpaque { inner: node }));
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_text_input(
    value: *const c_char,
    cursor_grapheme: u32,
    placeholder: *const c_char,
    style: *const GibsonStyle,
    out: *mut *mut GibsonNodeOpaque,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if out.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let val_str = if value.is_null() {
            ""
        } else {
            match CStr::from_ptr(value).to_str() {
                Ok(s) => s,
                Err(_) => {
                    set_last_error("Invalid UTF-8 in text input value".into());
                    return GibsonStatus::ErrInvalidParam;
                }
            }
        };
        let ph_str = if placeholder.is_null() {
            None
        } else {
            match CStr::from_ptr(placeholder).to_str() {
                Ok(s) => Some(s),
                Err(_) => {
                    set_last_error("Invalid UTF-8 in placeholder".into());
                    return GibsonStatus::ErrInvalidParam;
                }
            }
        };
        let st = match read_style(style) {
            Ok(s) => s,
            Err(e) => return e,
        };

        let node = Node::text_input(val_str, cursor_grapheme as usize, ph_str, st);
        *out = Box::into_raw(Box::new(GibsonNodeOpaque { inner: node }));
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_border_box(
    border_type: i32,
    style: *const GibsonStyle,
    out: *mut *mut GibsonNodeOpaque,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if out.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let st = match read_style(style) {
            Ok(s) => s,
            Err(e) => return e,
        };
        let bt = match validate_border(border_type) {
            Ok(b) => b,
            Err(e) => return e,
        };
        let node = Node::border_box(bt, st);
        *out = Box::into_raw(Box::new(GibsonNodeOpaque { inner: node }));
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_rule(
    title: *const c_char,
    style: *const GibsonStyle,
    out: *mut *mut GibsonNodeOpaque,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if out.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let t_str = if title.is_null() {
            None
        } else {
            match CStr::from_ptr(title).to_str() {
                Ok(s) => Some(s),
                Err(_) => {
                    set_last_error("Invalid UTF-8 in rule title".into());
                    return GibsonStatus::ErrInvalidParam;
                }
            }
        };
        let st = match read_style(style) {
            Ok(s) => s,
            Err(e) => return e,
        };
        let node = Node::rule(t_str, st);
        *out = Box::into_raw(Box::new(GibsonNodeOpaque { inner: node }));
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_rail(
    style: *const GibsonStyle,
    out: *mut *mut GibsonNodeOpaque,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if out.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let st = match read_style(style) {
            Ok(s) => s,
            Err(e) => return e,
        };
        let node = Node::rail(st);
        *out = Box::into_raw(Box::new(GibsonNodeOpaque { inner: node }));
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

/// Builds a node from language-neutral structured rich text. Borrows `rich`;
/// the caller still owns and must free it.
#[no_mangle]
pub unsafe extern "C" fn gibson_node_rich_text(
    rich: *const GibsonRichTextOpaque,
    wrap: i32,
    out: *mut *mut GibsonNodeOpaque,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if rich.is_null() || out.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let wm = match validate_wrap(wrap) {
            Ok(w) => w,
            Err(e) => return e,
        };
        let rt = RichText::from_lines((*rich).lines.clone());
        let node = Node::rich_text_wrapped(rt, wm);
        *out = Box::into_raw(Box::new(GibsonNodeOpaque { inner: node }));
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_add_child(
    parent: *mut GibsonNodeOpaque,
    child: *mut GibsonNodeOpaque,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if parent.is_null() || child.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let child_boxed = Box::from_raw(child);
        (*parent).inner.add_child(child_boxed.inner);
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

macro_rules! node_dim_setter {
    ($name:ident, $field:ident, $variant:ident) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(node: *mut GibsonNodeOpaque, value: f32) -> GibsonStatus {
            let res = catch_unwind(AssertUnwindSafe(|| {
                if node.is_null() || !value.is_finite() {
                    return GibsonStatus::ErrInvalidParam;
                }
                (*node).inner.layout_style.$field = crate::node::Dimension::$variant(value);
                GibsonStatus::Ok
            }));
            res.unwrap_or(GibsonStatus::ErrPanic)
        }
    };
}

node_dim_setter!(gibson_node_set_width, width, Length);
node_dim_setter!(gibson_node_set_height, height, Length);
node_dim_setter!(gibson_node_set_percent_width, width, Percent);
node_dim_setter!(gibson_node_set_min_width, min_width, Length);
node_dim_setter!(gibson_node_set_max_width, max_width, Length);

#[no_mangle]
pub unsafe extern "C" fn gibson_node_set_flex_grow(
    node: *mut GibsonNodeOpaque,
    grow: f32,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if node.is_null() || !grow.is_finite() {
            return GibsonStatus::ErrInvalidParam;
        }
        (*node).inner.layout_style.flex_grow = grow;
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_set_gap(
    node: *mut GibsonNodeOpaque,
    gap: f32,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if node.is_null() || !gap.is_finite() {
            return GibsonStatus::ErrInvalidParam;
        }
        (*node).inner.layout_style.gap_row = gap;
        (*node).inner.layout_style.gap_col = gap;
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_set_padding(
    node: *mut GibsonNodeOpaque,
    padding: f32,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if node.is_null() || !padding.is_finite() {
            return GibsonStatus::ErrInvalidParam;
        }
        (*node).inner.layout_style.padding_top = padding;
        (*node).inner.layout_style.padding_bottom = padding;
        (*node).inner.layout_style.padding_left = padding;
        (*node).inner.layout_style.padding_right = padding;
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_set_padding_sides(
    node: *mut GibsonNodeOpaque,
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if node.is_null()
            || !left.is_finite()
            || !right.is_finite()
            || !top.is_finite()
            || !bottom.is_finite()
        {
            return GibsonStatus::ErrInvalidParam;
        }
        (*node).inner.layout_style.padding_left = left;
        (*node).inner.layout_style.padding_right = right;
        (*node).inner.layout_style.padding_top = top;
        (*node).inner.layout_style.padding_bottom = bottom;
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_free(node: *mut GibsonNodeOpaque) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !node.is_null() {
            drop(Box::from_raw(node));
        }
    }));
}

// ---------------------- Rich Text Construction FFI ----------------------

#[no_mangle]
pub unsafe extern "C" fn gibson_line_new(out: *mut *mut GibsonLineOpaque) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if out.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        *out = Box::into_raw(Box::new(GibsonLineOpaque {
            spans: Vec::new(),
            align: TextAlign::Left,
        }));
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_line_add_span(
    line: *mut GibsonLineOpaque,
    utf8_text: *const c_char,
    style: *const GibsonStyle,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if line.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let text = match read_utf8(utf8_text, "span text") {
            Ok(s) => s,
            Err(e) => return e,
        };
        let st = match read_style(style) {
            Ok(s) => s,
            Err(e) => return e,
        };
        (*line).spans.push(Span::styled(text, st));
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_line_set_align(
    line: *mut GibsonLineOpaque,
    align: i32,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if line.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        match validate_align(align) {
            Ok(a) => {
                (*line).align = a;
                GibsonStatus::Ok
            }
            Err(e) => e,
        }
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_line_free(line: *mut GibsonLineOpaque) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !line.is_null() {
            drop(Box::from_raw(line));
        }
    }));
}

#[no_mangle]
pub unsafe extern "C" fn gibson_rich_text_new(out: *mut *mut GibsonRichTextOpaque) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if out.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        *out = Box::into_raw(Box::new(GibsonRichTextOpaque { lines: Vec::new() }));
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

/// Appends a **copy** of `line` to `rich`. `line` remains owned by the caller.
#[no_mangle]
pub unsafe extern "C" fn gibson_rich_text_add_line(
    rich: *mut GibsonRichTextOpaque,
    line: *const GibsonLineOpaque,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if rich.is_null() || line.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let mut new_line = Line::from_spans((*line).spans.clone());
        new_line.align = (*line).align;
        (*rich).lines.push(new_line);
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_rich_text_free(rich: *mut GibsonRichTextOpaque) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !rich.is_null() {
            drop(Box::from_raw(rich));
        }
    }));
}

#[no_mangle]
pub unsafe extern "C" fn gibson_commit_rich_text(
    ctx: *mut GibsonContextOpaque,
    rich: *const GibsonRichTextOpaque,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() || rich.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let rt = RichText::from_lines((*rich).lines.clone());
        match (*ctx).inner.commit_rich_text(&rt) {
            Ok(_) => GibsonStatus::Ok,
            Err(e) => {
                set_last_error(e.to_string());
                GibsonStatus::ErrIo
            }
        }
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_insert_rich_text_before_live(
    ctx: *mut GibsonContextOpaque,
    rich: *const GibsonRichTextOpaque,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() || rich.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let rt = RichText::from_lines((*rich).lines.clone());
        match (*ctx).inner.insert_rich_text_before_live(&rt) {
            Ok(_) => GibsonStatus::Ok,
            Err(e) => {
                set_last_error(e.to_string());
                GibsonStatus::ErrIo
            }
        }
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_last_error_message(buf: *mut c_char, buf_len: usize) -> i32 {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if buf.is_null() || buf_len == 0 {
            return -1;
        }
        LAST_ERROR.with(|err| {
            let msg = err.borrow();
            let bytes = msg.as_bytes();
            let copy_len = bytes.len().min(buf_len - 1);
            ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, buf, copy_len);
            *buf.add(copy_len) = 0;
            copy_len as i32
        })
    }));
    res.unwrap_or(-1)
}
