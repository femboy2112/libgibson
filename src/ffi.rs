#![allow(clippy::missing_safety_doc)]

use crate::cell::{Color, Style};
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

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GibsonRenderMode {
    Inline = 0,
    Fullscreen = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GibsonColorType {
    Reset = 0,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    Ansi256,
    Rgb,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GibsonColor {
    pub color_type: GibsonColorType,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub _pad: u8,
}

impl Default for GibsonColor {
    fn default() -> Self {
        Self {
            color_type: GibsonColorType::Reset,
            r: 0,
            g: 0,
            b: 0,
            _pad: 0,
        }
    }
}

impl From<GibsonColor> for Option<Color> {
    fn from(c: GibsonColor) -> Self {
        match c.color_type {
            GibsonColorType::Reset => None,
            GibsonColorType::Black => Some(Color::Black),
            GibsonColorType::Red => Some(Color::Red),
            GibsonColorType::Green => Some(Color::Green),
            GibsonColorType::Yellow => Some(Color::Yellow),
            GibsonColorType::Blue => Some(Color::Blue),
            GibsonColorType::Magenta => Some(Color::Magenta),
            GibsonColorType::Cyan => Some(Color::Cyan),
            GibsonColorType::White => Some(Color::White),
            GibsonColorType::BrightBlack => Some(Color::BrightBlack),
            GibsonColorType::BrightRed => Some(Color::BrightRed),
            GibsonColorType::BrightGreen => Some(Color::BrightGreen),
            GibsonColorType::BrightYellow => Some(Color::BrightYellow),
            GibsonColorType::BrightBlue => Some(Color::BrightBlue),
            GibsonColorType::BrightMagenta => Some(Color::BrightMagenta),
            GibsonColorType::BrightCyan => Some(Color::BrightCyan),
            GibsonColorType::BrightWhite => Some(Color::BrightWhite),
            GibsonColorType::Ansi256 => Some(Color::Ansi256(c.r)),
            GibsonColorType::Rgb => Some(Color::Rgb(c.r, c.g, c.b)),
        }
    }
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

impl From<GibsonStyle> for Style {
    fn from(s: GibsonStyle) -> Self {
        Self {
            fg: s.fg.into(),
            bg: s.bg.into(),
            bold: s.bold != 0,
            dim: s.dim != 0,
            italic: s.italic != 0,
            underline: s.underline != 0,
            reverse: s.reverse != 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GibsonBorderType {
    Single = 0,
    Double = 1,
    Rounded = 2,
    Thick = 3,
    Ascii = 4,
}

impl From<GibsonBorderType> for BorderType {
    fn from(b: GibsonBorderType) -> Self {
        match b {
            GibsonBorderType::Single => BorderType::Single,
            GibsonBorderType::Double => BorderType::Double,
            GibsonBorderType::Rounded => BorderType::Rounded,
            GibsonBorderType::Thick => BorderType::Thick,
            GibsonBorderType::Ascii => BorderType::Ascii,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GibsonEventType {
    None = 0,
    Key = 1,
    Paste = 2,
    Resize = 3,
}

#[repr(C)]
pub struct GibsonEvent {
    pub event_type: GibsonEventType,
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

pub struct GibsonContextOpaque {
    pub inner: Context,
    last_paste_cstring: Option<CString>,
}

pub struct GibsonNodeOpaque {
    pub inner: Node,
}

// ---------------------- FFI Functions ----------------------

#[no_mangle]
pub unsafe extern "C" fn gibson_create_context(
    mode: GibsonRenderMode,
    out: *mut *mut GibsonContextOpaque,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if out.is_null() {
            set_last_error("Null pointer provided for out context".into());
            return GibsonStatus::ErrInvalidParam;
        }

        let m = match mode {
            GibsonRenderMode::Inline => RenderMode::Inline,
            GibsonRenderMode::Fullscreen => RenderMode::Fullscreen,
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

#[no_mangle]
pub unsafe extern "C" fn gibson_commit(
    ctx: *mut GibsonContextOpaque,
    utf8_text: *const c_char,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() || utf8_text.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let c_str = match CStr::from_ptr(utf8_text).to_str() {
            Ok(s) => s,
            Err(_) => {
                set_last_error("Invalid UTF-8 string".into());
                return GibsonStatus::ErrInvalidParam;
            }
        };

        match (*ctx).inner.commit(c_str) {
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

#[no_mangle]
pub unsafe extern "C" fn gibson_insert_before_live(
    ctx: *mut GibsonContextOpaque,
    utf8_text: *const c_char,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() || utf8_text.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let c_str = match CStr::from_ptr(utf8_text).to_str() {
            Ok(s) => s,
            Err(_) => {
                set_last_error("Invalid UTF-8 string".into());
                return GibsonStatus::ErrInvalidParam;
            }
        };

        let lines: Vec<&str> = c_str.lines().collect();
        match (*ctx).inner.insert_before_live(&lines) {
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

        (*out_event).event_type = GibsonEventType::None;
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
                        (*out_event).event_type = GibsonEventType::Key;
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
                        };
                    }
                    Event::Paste(s) => {
                        (*out_event).event_type = GibsonEventType::Paste;
                        if let Ok(cs) = CString::new(s) {
                            (*out_event).paste_str = cs.as_ptr();
                            (*ctx).last_paste_cstring = Some(cs);
                        }
                    }
                    Event::Resize(cols, rows) => {
                        (*out_event).event_type = GibsonEventType::Resize;
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
    out_stats: *mut RenderStats,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() || out_stats.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        *out_stats = (*ctx).inner.stats();
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
        let node = Node::col();
        *out = Box::into_raw(Box::new(GibsonNodeOpaque { inner: node }));
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
        let node = Node::row();
        *out = Box::into_raw(Box::new(GibsonNodeOpaque { inner: node }));
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
        if utf8_text.is_null() || out.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let text = match CStr::from_ptr(utf8_text).to_str() {
            Ok(s) => s,
            Err(_) => return GibsonStatus::ErrInvalidParam,
        };
        let st: Style = if style.is_null() {
            Style::default()
        } else {
            (*style).into()
        };
        let wm = match wrap {
            1 => WrapMode::CharWrap,
            2 => WrapMode::WordWrap,
            _ => WrapMode::NoWrap,
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
        let st = if style.is_null() {
            Style::default()
        } else {
            (*style).into()
        };
        let lbl_str = if label.is_null() {
            None
        } else {
            CStr::from_ptr(label).to_str().ok()
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
            CStr::from_ptr(value).to_str().unwrap_or("")
        };
        let ph_str = if placeholder.is_null() {
            None
        } else {
            CStr::from_ptr(placeholder).to_str().ok()
        };
        let st = if style.is_null() {
            Style::default()
        } else {
            (*style).into()
        };

        let node = Node::text_input(val_str, cursor_grapheme as usize, ph_str, st);
        *out = Box::into_raw(Box::new(GibsonNodeOpaque { inner: node }));
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_border_box(
    border_type: GibsonBorderType,
    style: *const GibsonStyle,
    out: *mut *mut GibsonNodeOpaque,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if out.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        let st = if style.is_null() {
            Style::default()
        } else {
            (*style).into()
        };
        let node = Node::border_box(border_type.into(), st);
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
            CStr::from_ptr(title).to_str().ok()
        };
        let st = if style.is_null() {
            Style::default()
        } else {
            (*style).into()
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
        let st = if style.is_null() {
            Style::default()
        } else {
            (*style).into()
        };
        let node = Node::rail(st);
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

#[no_mangle]
pub unsafe extern "C" fn gibson_node_set_width(
    node: *mut GibsonNodeOpaque,
    width: f32,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if node.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        (*node).inner.layout_style.width = crate::node::Dimension::Length(width);
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_set_height(
    node: *mut GibsonNodeOpaque,
    height: f32,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if node.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        (*node).inner.layout_style.height = crate::node::Dimension::Length(height);
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_set_flex_grow(
    node: *mut GibsonNodeOpaque,
    grow: f32,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if node.is_null() {
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
        if node.is_null() {
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
        if node.is_null() {
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
pub unsafe extern "C" fn gibson_node_set_percent_width(
    node: *mut GibsonNodeOpaque,
    percent: f32,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if node.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        (*node).inner.layout_style.width = crate::node::Dimension::Percent(percent);
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_set_min_width(
    node: *mut GibsonNodeOpaque,
    width: f32,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if node.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        (*node).inner.layout_style.min_width = crate::node::Dimension::Length(width);
        GibsonStatus::Ok
    }));
    res.unwrap_or(GibsonStatus::ErrPanic)
}

#[no_mangle]
pub unsafe extern "C" fn gibson_node_set_max_width(
    node: *mut GibsonNodeOpaque,
    width: f32,
) -> GibsonStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if node.is_null() {
            return GibsonStatus::ErrInvalidParam;
        }
        (*node).inner.layout_style.max_width = crate::node::Dimension::Length(width);
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
        if node.is_null() {
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
