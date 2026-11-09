//! #275 — key-release reporting through the C ABI (v1, additive):
//! `gibson_set_key_release_reporting` and the event types `GIBSON_EVENT_KEY_RELEASE`,
//! `GIBSON_EVENT_KEY_RELEASES_ACTIVE` and `GIBSON_EVENT_KEY_RELEASES_UNSUPPORTED`.
//!
//! A headless context cannot ask a terminal anything, so the opt-in is a no-op there; the events
//! are pushed with `gibson_push_event[_kind]` and must poll back exactly as pushed, with the key
//! kind reported for a release as for a press. The real terminal path is covered by
//! `tests/key_release_reporting_pty.rs` (role `c`).
use gibson::ffi::*;
use std::ptr;

struct Ctx(*mut GibsonContextOpaque);

impl Ctx {
    fn headless() -> Self {
        let mut ctx = ptr::null_mut();
        let st =
            unsafe { gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, 40, 10, &mut ctx) };
        assert_eq!(st, GibsonStatus::Ok);
        Ctx(ctx)
    }

    fn push_kind(&self, e: &GibsonEvent, kind: u32) -> GibsonStatus {
        unsafe { gibson_push_event_kind(self.0, e, kind) }
    }

    fn push(&self, e: &GibsonEvent) -> GibsonStatus {
        unsafe { gibson_push_event(self.0, e) }
    }

    fn poll_kind(&self) -> (GibsonEvent, u32) {
        let mut e = blank(-1);
        let mut kind = 99;
        let st = unsafe { gibson_poll_event_kind(self.0, 0, &mut e, &mut kind) };
        assert_eq!(st, GibsonStatus::Ok);
        (e, kind)
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

fn ev(event_type: i32, key_code: u32, modifiers: u8) -> GibsonEvent {
    GibsonEvent {
        key_code,
        modifiers,
        ..blank(event_type)
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
fn the_event_type_values_are_fixed() {
    // Appended after RESIZE (3); the existing values are unchanged.
    assert_eq!(
        (
            GIBSON_EVENT_NONE,
            GIBSON_EVENT_KEY,
            GIBSON_EVENT_PASTE,
            GIBSON_EVENT_RESIZE
        ),
        (0, 1, 2, 3)
    );
    assert_eq!(GIBSON_EVENT_KEY_RELEASE, 4);
    assert_eq!(GIBSON_EVENT_KEY_RELEASES_ACTIVE, 5);
    assert_eq!(GIBSON_EVENT_KEY_RELEASES_UNSUPPORTED, 6);
}

#[test]
fn the_opt_in_rejects_null_and_is_a_no_op_on_a_headless_context() {
    assert_eq!(
        unsafe { gibson_set_key_release_reporting(ptr::null_mut(), 1) },
        GibsonStatus::ErrInvalidParam
    );
    let c = Ctx::headless();
    for on in [1, 0, 1, 0] {
        assert_eq!(
            unsafe { gibson_set_key_release_reporting(c.0, on) },
            GibsonStatus::Ok
        );
    }
    // Nothing was written into the headless capture for it (a headless context asks no terminal).
    let mut len = 0usize;
    let st = unsafe { gibson_get_rendered_output(c.0, ptr::null_mut(), 0, &mut len) };
    assert_eq!(st, GibsonStatus::Ok);
    assert_eq!(len, 0);
}

#[test]
fn releases_and_answers_poll_back_exactly_as_pushed() {
    let c = Ctx::headless();
    let script: Vec<(GibsonEvent, u32)> = vec![
        (
            blank(GIBSON_EVENT_KEY_RELEASES_ACTIVE),
            GIBSON_KEY_KIND_NONE,
        ),
        (ev(GIBSON_EVENT_KEY, 'w' as u32, 0), GIBSON_KEY_KIND_CHAR),
        (
            ev(GIBSON_EVENT_KEY_RELEASE, 'w' as u32, 0),
            GIBSON_KEY_KIND_CHAR,
        ),
        // U+1001 typed, released: the #274 collision range keeps its kind on the way up.
        (
            ev(GIBSON_EVENT_KEY_RELEASE, 0x1001, 0),
            GIBSON_KEY_KIND_CHAR,
        ),
        (
            ev(
                GIBSON_EVENT_KEY_RELEASE,
                GIBSON_KEY_LEFT,
                0x01, // GIBSON_MOD_SHIFT
            ),
            GIBSON_KEY_KIND_SPECIAL,
        ),
        (
            blank(GIBSON_EVENT_KEY_RELEASES_UNSUPPORTED),
            GIBSON_KEY_KIND_NONE,
        ),
    ];
    for (e, kind) in &script {
        let st = if *kind == GIBSON_KEY_KIND_NONE {
            c.push(e)
        } else {
            c.push_kind(e, *kind)
        };
        assert_eq!(st, GibsonStatus::Ok, "push {:?}", fields(e));
    }
    for (want, want_kind) in &script {
        let (got, kind) = c.poll_kind();
        assert_eq!(fields(&got), fields(want));
        assert_eq!(kind, *want_kind, "kind of {:?}", fields(want));
    }
    assert_eq!(c.poll_kind().0.event_type, GIBSON_EVENT_NONE);
}

#[test]
fn the_legacy_poll_reports_a_release_with_its_key_code() {
    let c = Ctx::headless();
    assert_eq!(
        c.push(&ev(GIBSON_EVENT_KEY_RELEASE, GIBSON_KEY_ENTER, 0)),
        GibsonStatus::Ok
    );
    let got = c.poll();
    assert_eq!(
        fields(&got),
        fields(&ev(GIBSON_EVENT_KEY_RELEASE, GIBSON_KEY_ENTER, 0))
    );
}

#[test]
fn a_release_is_validated_like_a_press() {
    let c = Ctx::headless();
    // A surrogate is not a Unicode scalar value; unknown modifier bits; a CHAR kind that is a
    // special constant is fine, a SPECIAL kind that is not a constant is not.
    assert_eq!(
        c.push(&ev(GIBSON_EVENT_KEY_RELEASE, 0xD800, 0)),
        GibsonStatus::ErrInvalidParam
    );
    assert_eq!(
        c.push(&ev(GIBSON_EVENT_KEY_RELEASE, 'a' as u32, 0x80)),
        GibsonStatus::ErrInvalidParam
    );
    assert_eq!(
        c.push_kind(
            &ev(GIBSON_EVENT_KEY_RELEASE, 'a' as u32, 0),
            GIBSON_KEY_KIND_SPECIAL
        ),
        GibsonStatus::ErrInvalidParam
    );
    assert_eq!(c.poll().event_type, GIBSON_EVENT_NONE, "nothing was queued");
    // An event type past the new ones is still refused.
    assert_eq!(c.push(&blank(7)), GibsonStatus::ErrInvalidParam);
}
