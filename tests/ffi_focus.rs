//! #298 — focus events and the terminal-size query through the C ABI (v1, additive):
//! `gibson_set_focus_reporting`, `gibson_get_terminal_size`, and the event types
//! `GIBSON_EVENT_FOCUS_GAINED` / `GIBSON_EVENT_FOCUS_LOST`.
//!
//! A headless context asks no terminal, so the opt-in is a no-op there; the focus events carry no
//! fields, so the whole payload is the discriminant and it must poll back exactly as pushed. The
//! real terminal path (EnableFocusChange / the CSI I/O sequences) lives with the session, not here.
use gibson::ffi::*;
use std::ptr;

struct Ctx(*mut GibsonContextOpaque);

impl Ctx {
    fn headless(cols: u16, rows: u16) -> Self {
        let mut ctx = ptr::null_mut();
        let st =
            unsafe { gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, cols, rows, &mut ctx) };
        assert_eq!(st, GibsonStatus::Ok);
        Ctx(ctx)
    }

    fn push(&self, e: &GibsonEvent) -> GibsonStatus {
        unsafe { gibson_push_event(self.0, e) }
    }

    fn poll(&self) -> GibsonEvent {
        let mut e = blank(-1);
        let st = unsafe { gibson_poll_event(self.0, 0, &mut e) };
        assert_eq!(st, GibsonStatus::Ok);
        e
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

fn fields(e: &GibsonEvent) -> (i32, u32, u8, u16, u16, bool) {
    (
        e.event_type,
        e.key_code,
        e.modifiers,
        e.cols,
        e.rows,
        e.paste_str.is_null(),
    )
}

#[test]
fn the_focus_event_type_values_are_fixed() {
    // Appended after the #275 key-release set (4-6); the earlier values are unchanged.
    assert_eq!(GIBSON_EVENT_FOCUS_GAINED, 7);
    assert_eq!(GIBSON_EVENT_FOCUS_LOST, 8);
}

#[test]
fn focus_events_poll_back_exactly_as_pushed() {
    let c = Ctx::headless(40, 10);
    // Fieldless: a blank event bearing only the discriminant is the whole event. Round-trip twice
    // each, interleaved, to prove neither type leaks fields into or steals state from the other.
    let script = [
        GIBSON_EVENT_FOCUS_GAINED,
        GIBSON_EVENT_FOCUS_LOST,
        GIBSON_EVENT_FOCUS_LOST,
        GIBSON_EVENT_FOCUS_GAINED,
    ];
    for &ty in &script {
        assert_eq!(c.push(&blank(ty)), GibsonStatus::Ok, "push {ty}");
    }
    for &ty in &script {
        let got = c.poll();
        assert_eq!(fields(&got), fields(&blank(ty)), "poll {ty}");
    }
    assert_eq!(c.poll().event_type, GIBSON_EVENT_NONE, "queue drained");
}

#[test]
fn the_focus_opt_in_rejects_null_and_is_a_no_op_on_a_headless_context() {
    assert_eq!(
        unsafe { gibson_set_focus_reporting(ptr::null_mut(), 1) },
        GibsonStatus::ErrInvalidParam
    );
    let c = Ctx::headless(40, 10);
    for on in [1, 0, 1, 0] {
        assert_eq!(
            unsafe { gibson_set_focus_reporting(c.0, on) },
            GibsonStatus::Ok
        );
    }
    // A headless context asks no terminal, so nothing was written into its capture.
    let mut len = 0usize;
    let st = unsafe { gibson_get_rendered_output(c.0, ptr::null_mut(), 0, &mut len) };
    assert_eq!(st, GibsonStatus::Ok);
    assert_eq!(len, 0);
}

#[test]
fn terminal_size_reports_the_creation_geometry_and_rejects_nulls() {
    let c = Ctx::headless(123, 45);
    let (mut cols, mut rows) = (0u16, 0u16);
    let st = unsafe { gibson_get_terminal_size(c.0, &mut cols, &mut rows) };
    assert_eq!(st, GibsonStatus::Ok);
    // Live device geometry, answered before any render — the size it was created with.
    assert_eq!((cols, rows), (123, 45));
    assert!(cols != 0 && rows != 0, "both outputs are always non-zero");

    assert_eq!(
        unsafe { gibson_get_terminal_size(ptr::null_mut(), &mut cols, &mut rows) },
        GibsonStatus::ErrInvalidParam
    );
    assert_eq!(
        unsafe { gibson_get_terminal_size(c.0, ptr::null_mut(), &mut rows) },
        GibsonStatus::ErrInvalidParam
    );
    assert_eq!(
        unsafe { gibson_get_terminal_size(c.0, &mut cols, ptr::null_mut()) },
        GibsonStatus::ErrInvalidParam
    );
}

#[test]
fn a_polled_resize_updates_the_reported_terminal_size() {
    // review P3: the reported size is the CURRENT headless geometry, not frozen at creation — a
    // polled RESIZE updates it (as #293/#308 made Context::poll_from do for a headless context).
    let c = Ctx::headless(123, 45);
    let mut resize = blank(GIBSON_EVENT_RESIZE);
    resize.cols = 40;
    resize.rows = 10;
    assert_eq!(c.push(&resize), GibsonStatus::Ok);
    let got = c.poll();
    assert_eq!(got.event_type, GIBSON_EVENT_RESIZE);
    assert_eq!((got.cols, got.rows), (40, 10));
    let (mut cols, mut rows) = (0u16, 0u16);
    assert_eq!(
        unsafe { gibson_get_terminal_size(c.0, &mut cols, &mut rows) },
        GibsonStatus::Ok
    );
    assert_eq!(
        (cols, rows),
        (40, 10),
        "the query follows the resize, not the creation size"
    );
}
