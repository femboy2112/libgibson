//! v0.7 node C (C ABI) — `gibson_push_event`: injected input for a headless context.
//!
//! Contract under test (`include/gibson.h`): a pushed event polls back exactly as
//! `gibson_poll_event` encodes it; delivery is FIFO, exactly once, and never waits; a full queue
//! refuses (`GIBSON_ERR_QUEUE_FULL`) without displacing anything; unrepresentable events and
//! terminal contexts are refused with `GIBSON_ERR_INVALID_PARAM` and queue nothing; a delivered
//! resize sets the headless geometry.
use gibson::ffi::*;
use std::ffi::{CStr, CString};
use std::ptr;
use std::time::{Duration, Instant};

struct Ctx(*mut GibsonContextOpaque);

impl Ctx {
    fn headless(cols: u16, rows: u16) -> Self {
        let mut ctx = ptr::null_mut();
        let st =
            unsafe { gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, cols, rows, &mut ctx) };
        assert_eq!(st, GibsonStatus::Ok);
        Ctx(ctx)
    }
    fn push(&self, ev: &GibsonEvent) -> GibsonStatus {
        unsafe { gibson_push_event(self.0, ev) }
    }
    fn poll(&self) -> GibsonEvent {
        let mut out = blank(GIBSON_EVENT_NONE);
        let st = unsafe { gibson_poll_event(self.0, 0, &mut out) };
        assert_eq!(st, GibsonStatus::Ok);
        out
    }
}

impl Drop for Ctx {
    fn drop(&mut self) {
        unsafe { gibson_destroy_context(self.0) };
    }
}

fn blank(event_type: i32) -> GibsonEvent {
    GibsonEvent {
        event_type,
        key_code: 0,
        modifiers: 0,
        _pad: [0; 3],
        cols: 0,
        rows: 0,
        paste_str: ptr::null(),
    }
}

fn key(code: u32, modifiers: u8) -> GibsonEvent {
    GibsonEvent {
        key_code: code,
        modifiers,
        ..blank(GIBSON_EVENT_KEY)
    }
}

fn resize(cols: u16, rows: u16) -> GibsonEvent {
    GibsonEvent {
        cols,
        rows,
        ..blank(GIBSON_EVENT_RESIZE)
    }
}

fn last_error() -> String {
    let mut buf = [0 as std::os::raw::c_char; 256];
    unsafe { gibson_last_error_message(buf.as_mut_ptr(), buf.len()) };
    unsafe { CStr::from_ptr(buf.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

/// The fields that carry meaning for the event's type.
fn meaning(e: &GibsonEvent) -> (i32, u32, u8, u16, u16, Option<String>) {
    let paste = (!e.paste_str.is_null()).then(|| {
        unsafe { CStr::from_ptr(e.paste_str) }
            .to_string_lossy()
            .into_owned()
    });
    (e.event_type, e.key_code, e.modifiers, e.cols, e.rows, paste)
}

#[test]
fn every_representable_event_polls_back_unchanged_in_order() {
    let ctx = Ctx::headless(20, 5);
    let text = CString::new("héllo, 世界 👋").unwrap();
    let mut pushed = vec![
        key(u32::from('a'), 0),
        key(u32::from('Z'), GIBSON_MOD_SHIFT_BITS),
        key(u32::from('c'), GIBSON_MOD_CONTROL_BITS),
        key(0x1F600, GIBSON_MOD_ALT_BITS | GIBSON_MOD_SHIFT_BITS),
    ];
    for special in [
        GIBSON_KEY_ENTER,
        GIBSON_KEY_LEFT,
        GIBSON_KEY_RIGHT,
        GIBSON_KEY_UP,
        GIBSON_KEY_DOWN,
        GIBSON_KEY_HOME,
        GIBSON_KEY_END,
        GIBSON_KEY_BACKSPACE,
        GIBSON_KEY_DELETE,
        GIBSON_KEY_ESC,
        GIBSON_KEY_TAB,
        GIBSON_KEY_PAGE_UP,
        GIBSON_KEY_PAGE_DOWN,
    ] {
        pushed.push(key(special, GIBSON_MOD_CONTROL_BITS));
    }
    pushed.push(GibsonEvent {
        paste_str: text.as_ptr(),
        ..blank(GIBSON_EVENT_PASTE)
    });
    pushed.push(resize(33, 7));
    for e in &pushed {
        assert_eq!(ctx.push(e), GibsonStatus::Ok, "{:?}", meaning(e));
    }
    for e in &pushed {
        assert_eq!(meaning(&ctx.poll()), meaning(e));
    }
    // Delivered exactly once: the queue is now empty.
    assert_eq!(ctx.poll().event_type, GIBSON_EVENT_NONE);
}

const GIBSON_MOD_SHIFT_BITS: u8 = 0x01;
const GIBSON_MOD_CONTROL_BITS: u8 = 0x02;
const GIBSON_MOD_ALT_BITS: u8 = 0x04;

#[test]
fn a_full_queue_refuses_without_displacing_and_recovers_after_a_poll() {
    let ctx = Ctx::headless(10, 3);
    for i in 0..GIBSON_EVENT_QUEUE_CAPACITY as u32 {
        assert_eq!(ctx.push(&key(0x4E00 + i, 0)), GibsonStatus::Ok, "push {i}");
    }
    let refused = key(u32::from('!'), 0);
    assert_eq!(ctx.push(&refused), GibsonStatus::ErrQueueFull);
    assert!(last_error().contains("1024"), "{}", last_error());
    // The oldest event comes out first; the refused one was not queued.
    assert_eq!(ctx.poll().key_code, 0x4E00);
    assert_eq!(ctx.push(&key(u32::from('#'), 0)), GibsonStatus::Ok);
    let mut rest = Vec::new();
    loop {
        let e = ctx.poll();
        if e.event_type == GIBSON_EVENT_NONE {
            break;
        }
        rest.push(e.key_code);
    }
    let mut expected: Vec<u32> = (1..GIBSON_EVENT_QUEUE_CAPACITY as u32)
        .map(|i| 0x4E00 + i)
        .collect();
    expected.push(u32::from('#'));
    assert_eq!(rest, expected);
}

#[test]
fn unrepresentable_events_are_refused_and_queue_nothing() {
    let ctx = Ctx::headless(10, 3);
    let bad_utf8 = [0xffu8, 0xfe, 0];
    let cases: Vec<(&str, GibsonEvent)> = vec![
        ("NONE", blank(GIBSON_EVENT_NONE)),
        ("unknown type", blank(99)),
        ("negative type", blank(-1)),
        ("unknown modifier bit", key(u32::from('a'), 0x08)),
        ("surrogate", key(0xD800, 0)),
        ("beyond Unicode", key(0x11_0000, 0)),
        ("null paste", blank(GIBSON_EVENT_PASTE)),
        (
            "paste not UTF-8",
            GibsonEvent {
                paste_str: bad_utf8.as_ptr().cast(),
                ..blank(GIBSON_EVENT_PASTE)
            },
        ),
        ("zero cols", resize(0, 5)),
        ("zero rows", resize(5, 0)),
    ];
    for (name, e) in &cases {
        assert_eq!(ctx.push(e), GibsonStatus::ErrInvalidParam, "{name}");
        assert!(
            last_error().starts_with("gibson_push_event: "),
            "{name}: {}",
            last_error()
        );
    }
    assert_eq!(
        ctx.poll().event_type,
        GIBSON_EVENT_NONE,
        "a refused event was queued"
    );
    assert_eq!(
        unsafe { gibson_push_event(ptr::null_mut(), &key(97, 0)) },
        GibsonStatus::ErrInvalidParam
    );
    assert_eq!(
        unsafe { gibson_push_event(ctx.0, ptr::null()) },
        GibsonStatus::ErrInvalidParam
    );
}

#[test]
fn a_terminal_context_refuses_pushed_events() {
    let mut ctx = ptr::null_mut();
    assert_eq!(
        unsafe { gibson_create_context(GIBSON_MODE_INLINE, &mut ctx) },
        GibsonStatus::Ok
    );
    let st = unsafe { gibson_push_event(ctx, &key(u32::from('a'), 0)) };
    let msg = last_error();
    unsafe { gibson_destroy_context(ctx) };
    assert_eq!(st, GibsonStatus::ErrInvalidParam);
    assert!(msg.contains("not a headless context"), "{msg}");
}

#[test]
fn a_delivered_resize_sets_the_headless_geometry() {
    let ctx = Ctx::headless(20, 5);
    let size = |c: &Ctx| {
        assert_eq!(unsafe { gibson_render(c.0) }, GibsonStatus::Ok);
        let (mut w, mut h) = (0u16, 0u16);
        assert_eq!(
            unsafe { gibson_get_frame_size(c.0, &mut w, &mut h) },
            GibsonStatus::Ok
        );
        (w, h)
    };
    let mut root = ptr::null_mut();
    assert_eq!(
        unsafe { gibson_node_text(c"x".as_ptr(), ptr::null(), GIBSON_WRAP_NONE, &mut root) },
        GibsonStatus::Ok
    );
    assert_eq!(
        unsafe { gibson_set_root_node(ctx.0, root) },
        GibsonStatus::Ok
    );
    assert_eq!(size(&ctx), (20, 5));
    assert_eq!(ctx.push(&resize(41, 9)), GibsonStatus::Ok);
    // Queued, not yet delivered: the geometry has not changed.
    assert_eq!(size(&ctx), (20, 5));
    let e = ctx.poll();
    assert_eq!((e.event_type, e.cols, e.rows), (GIBSON_EVENT_RESIZE, 41, 9));
    assert_eq!(size(&ctx), (41, 9));
}

#[test]
fn a_headless_poll_never_waits() {
    let ctx = Ctx::headless(10, 3);
    let mut out = blank(GIBSON_EVENT_KEY);
    let t = Instant::now();
    assert_eq!(
        unsafe { gibson_poll_event(ctx.0, 5_000, &mut out) },
        GibsonStatus::Ok
    );
    assert_eq!(out.event_type, GIBSON_EVENT_NONE);
    assert!(
        t.elapsed() < Duration::from_secs(1),
        "waited {:?}",
        t.elapsed()
    );
}
