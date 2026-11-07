//! Issue #274 — the C ABI's `GIBSON_KEY_*` constants (0x1001–0x100D) are also Unicode scalar
//! values (U+1001–U+100D, Myanmar letters), so `gibson_poll_event`'s `key_code` cannot tell a
//! typed `ခ` from Enter. `gibson_poll_event_kind` / `gibson_push_event_kind` (ABI v1, additive)
//! carry the kind beside the unchanged encoding.
//!
//! Contract under test (`include/gibson.h`):
//! * every scalar in the collision range and every special key round-trip through
//!   `push_event_kind` → `poll_event_kind` with their kind, and the two never coincide;
//! * the legacy functions are unchanged: `gibson_push_event` still reads 0x1001–0x100D as the
//!   special keys, and `gibson_poll_event` still writes the same `key_code` for both;
//! * `gibson_poll_event_kind` writes `*out_event` exactly as `gibson_poll_event` does;
//! * bad kinds, a SPECIAL kind on a non-special code, and a CHAR kind on a non-scalar are
//!   refused and queue nothing; the kind is ignored for non-key events.
use gibson::ffi::*;
use std::ptr;

const SPECIALS: [u32; 13] = [
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
];

struct Ctx(*mut GibsonContextOpaque);

impl Ctx {
    fn headless() -> Self {
        let mut ctx = ptr::null_mut();
        let st =
            unsafe { gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, 40, 10, &mut ctx) };
        assert_eq!(st, GibsonStatus::Ok);
        Ctx(ctx)
    }
    fn push_kind(&self, ev: &GibsonEvent, kind: u32) -> GibsonStatus {
        unsafe { gibson_push_event_kind(self.0, ev, kind) }
    }
    fn push_legacy(&self, ev: &GibsonEvent) -> GibsonStatus {
        unsafe { gibson_push_event(self.0, ev) }
    }
    fn poll_kind(&self) -> (GibsonEvent, u32) {
        let mut out = blank(GIBSON_EVENT_NONE);
        let mut kind = 0xDEAD_BEEF;
        let st = unsafe { gibson_poll_event_kind(self.0, 0, &mut out, &mut kind) };
        assert_eq!(st, GibsonStatus::Ok);
        (out, kind)
    }
    fn poll_legacy(&self) -> GibsonEvent {
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

/// The fields a key event carries, for comparing encodings.
fn fields(e: &GibsonEvent) -> (i32, u32, u8, [u8; 3], u16, u16, bool) {
    (
        e.event_type,
        e.key_code,
        e.modifiers,
        e._pad,
        e.cols,
        e.rows,
        e.paste_str.is_null(),
    )
}

#[test]
fn every_collision_scalar_and_every_special_key_round_trip_with_distinct_kinds() {
    let ctx = Ctx::headless();
    let mut seen = Vec::new();
    for cp in 0x1001..=0x100Du32 {
        assert_eq!(
            ctx.push_kind(&key(cp, 0), GIBSON_KEY_KIND_CHAR),
            GibsonStatus::Ok
        );
        assert_eq!(
            ctx.push_kind(&key(cp, 0), GIBSON_KEY_KIND_SPECIAL),
            GibsonStatus::Ok
        );
        let (typed, typed_kind) = ctx.poll_kind();
        let (special, special_kind) = ctx.poll_kind();
        assert_eq!((typed.key_code, typed_kind), (cp, GIBSON_KEY_KIND_CHAR));
        assert_eq!(
            (special.key_code, special_kind),
            (cp, GIBSON_KEY_KIND_SPECIAL)
        );
        seen.push((typed.key_code, typed_kind));
        seen.push((special.key_code, special_kind));
    }
    // All 26 (code, kind) pairs are distinct: no typed letter reads as any special key.
    let mut uniq = seen.clone();
    uniq.sort_unstable();
    uniq.dedup();
    assert_eq!(uniq.len(), 26, "{seen:x?}");
    // And the collision range is exactly the special-key range.
    assert_eq!(SPECIALS.to_vec(), (0x1001..=0x100Du32).collect::<Vec<_>>());
}

#[test]
fn the_legacy_special_key_matrix_is_unchanged() {
    // gibson_push_event + gibson_poll_event: every GIBSON_KEY_* with every modifier set polls
    // back unchanged, and the kind-aware poll calls each one SPECIAL.
    let ctx = Ctx::headless();
    for &code in &SPECIALS {
        for mods in 0..=7u8 {
            assert_eq!(ctx.push_legacy(&key(code, mods)), GibsonStatus::Ok);
            let polled = ctx.poll_legacy();
            assert_eq!(fields(&polled), fields(&key(code, mods)));
            assert_eq!(ctx.push_legacy(&key(code, mods)), GibsonStatus::Ok);
            let (polled, kind) = ctx.poll_kind();
            assert_eq!(fields(&polled), fields(&key(code, mods)));
            assert_eq!(kind, GIBSON_KEY_KIND_SPECIAL, "{code:#x} mods {mods}");
        }
    }
}

#[test]
fn legacy_poll_still_reports_a_typed_collision_letter_by_its_code_alone() {
    // The ambiguity the legacy function keeps, by contract: a typed U+1001 pushed as CHAR
    // polls back through gibson_poll_event with key_code 0x1001, the same as Enter.
    let ctx = Ctx::headless();
    assert_eq!(
        ctx.push_kind(&key(0x1001, 0), GIBSON_KEY_KIND_CHAR),
        GibsonStatus::Ok
    );
    let legacy = ctx.poll_legacy();
    assert_eq!(fields(&legacy), fields(&key(GIBSON_KEY_ENTER, 0)));
}

/// The fields a poll wrote, with the paste text copied out.
type Written = (i32, u32, u8, [u8; 3], u16, u16, Option<String>);

/// Everything a poll writes, read back by value: the fields plus the paste text (copied while
/// it is valid), from an output buffer first filled with `poison`. `kind` selects the poll.
fn poll_into(ctx: &Ctx, poison: u8, kind: bool) -> (Written, Option<u32>) {
    let mut out = std::mem::MaybeUninit::<GibsonEvent>::uninit();
    // SAFETY: poisoning the bytes of a plain-old-data struct; every field is overwritten by the
    // poll (the contract under test) before it is read.
    unsafe { std::ptr::write_bytes(out.as_mut_ptr(), poison, 1) };
    let mut k = u32::from_ne_bytes([poison; 4]);
    let st = unsafe {
        if kind {
            gibson_poll_event_kind(ctx.0, 0, out.as_mut_ptr(), &mut k)
        } else {
            gibson_poll_event(ctx.0, 0, out.as_mut_ptr())
        }
    };
    assert_eq!(st, GibsonStatus::Ok);
    let e = unsafe { out.assume_init() };
    let paste = (!e.paste_str.is_null()).then(|| {
        unsafe { std::ffi::CStr::from_ptr(e.paste_str) }
            .to_string_lossy()
            .into_owned()
    });
    (
        (
            e.event_type,
            e.key_code,
            e.modifiers,
            e._pad,
            e.cols,
            e.rows,
            paste,
        ),
        kind.then_some(k),
    )
}

#[test]
fn kind_poll_writes_the_event_exactly_as_the_legacy_poll() {
    // Same pushes, two polls: identical *out_event bytes for keys, pastes, resizes and none.
    let ctx = Ctx::headless();
    let paste = std::ffi::CString::new("héllo").unwrap();
    let events = [
        key('a' as u32, 0),
        key(0x1004, 0),
        key(GIBSON_KEY_UP, 2),
        GibsonEvent {
            paste_str: paste.as_ptr(),
            ..blank(GIBSON_EVENT_PASTE)
        },
        GibsonEvent {
            cols: 33,
            rows: 7,
            ..blank(GIBSON_EVENT_RESIZE)
        },
    ];
    for ev in &events {
        // Each poll writes into a buffer poisoned with a different pattern, so a field one of
        // them forgets to write shows up as a difference; paste text is compared by content,
        // copied before the next poll invalidates it.
        assert_eq!(ctx.push_legacy(ev), GibsonStatus::Ok);
        let a = poll_into(&ctx, 0x11, false).0;
        assert_eq!(ctx.push_legacy(ev), GibsonStatus::Ok);
        let (b, kind) = poll_into(&ctx, 0xEE, true);
        assert_eq!(a, b, "legacy vs kind-aware poll output");
        let want = if ev.event_type == GIBSON_EVENT_KEY {
            if SPECIALS.contains(&ev.key_code) {
                GIBSON_KEY_KIND_SPECIAL
            } else {
                GIBSON_KEY_KIND_CHAR
            }
        } else {
            GIBSON_KEY_KIND_NONE
        };
        assert_eq!(kind, Some(want), "{:?}", fields(ev));
    }
    // An empty queue: NONE event, NONE kind (the poison value is overwritten).
    let (none, kind) = ctx.poll_kind();
    assert_eq!(none.event_type, GIBSON_EVENT_NONE);
    assert_eq!(kind, GIBSON_KEY_KIND_NONE);
}

#[test]
fn every_unicode_scalar_round_trips_as_a_typed_character() {
    // The whole scalar range through push_event_kind(CHAR) → poll_event_kind, in batches that
    // fit the queue: code and kind come back unchanged for all 1,112,064 values.
    let ctx = Ctx::headless();
    let scalars: Vec<u32> = (0..=0x10FFFFu32)
        .filter(|&c| char::from_u32(c).is_some())
        .collect();
    assert_eq!(scalars.len(), 1_112_064);
    for batch in scalars.chunks(1000) {
        for &cp in batch {
            assert_eq!(
                ctx.push_kind(&key(cp, 0), GIBSON_KEY_KIND_CHAR),
                GibsonStatus::Ok
            );
        }
        for &cp in batch {
            let (ev, kind) = ctx.poll_kind();
            assert_eq!(
                (ev.event_type, ev.key_code, kind),
                (GIBSON_EVENT_KEY, cp, GIBSON_KEY_KIND_CHAR)
            );
        }
    }
}

#[test]
fn bad_kinds_and_mismatched_codes_are_refused_and_queue_nothing() {
    let ctx = Ctx::headless();
    for (ev, kind) in [
        (key('a' as u32, 0), GIBSON_KEY_KIND_NONE),
        (key('a' as u32, 0), 3),
        (key('a' as u32, 0), u32::MAX),
        (key('a' as u32, 0), GIBSON_KEY_KIND_SPECIAL), // not a GIBSON_KEY_* value
        (key(0x100E, 0), GIBSON_KEY_KIND_SPECIAL),     // just past the range
        (key(0x1000, 0), GIBSON_KEY_KIND_SPECIAL),     // just before it
        (key(0xD800, 0), GIBSON_KEY_KIND_CHAR),        // a surrogate
        (key(0x11_0000, 0), GIBSON_KEY_KIND_CHAR),     // beyond Unicode
        (key(0x1001, 0x08), GIBSON_KEY_KIND_CHAR),     // unknown modifier bit
    ] {
        assert_eq!(
            ctx.push_kind(&ev, kind),
            GibsonStatus::ErrInvalidParam,
            "{:?} kind {kind}",
            fields(&ev)
        );
    }
    let (ev, kind) = ctx.poll_kind();
    assert_eq!(
        (ev.event_type, kind),
        (GIBSON_EVENT_NONE, GIBSON_KEY_KIND_NONE)
    );
}

#[test]
fn the_kind_is_ignored_for_non_key_events() {
    let ctx = Ctx::headless();
    let resize = GibsonEvent {
        cols: 50,
        rows: 12,
        ..blank(GIBSON_EVENT_RESIZE)
    };
    for kind in [0, 1, 2, 77] {
        assert_eq!(
            ctx.push_kind(&resize, kind),
            GibsonStatus::Ok,
            "kind {kind}"
        );
        let (ev, k) = ctx.poll_kind();
        assert_eq!(
            (ev.event_type, ev.cols, ev.rows, k),
            (GIBSON_EVENT_RESIZE, 50, 12, 0)
        );
    }
}

#[test]
fn null_pointers_are_refused() {
    let ctx = Ctx::headless();
    let mut out = blank(GIBSON_EVENT_NONE);
    unsafe {
        assert_eq!(
            gibson_poll_event_kind(ctx.0, 0, &mut out, ptr::null_mut()),
            GibsonStatus::ErrInvalidParam
        );
        let mut kind = 0;
        assert_eq!(
            gibson_poll_event_kind(ptr::null_mut(), 0, &mut out, &mut kind),
            GibsonStatus::ErrInvalidParam
        );
        assert_eq!(
            gibson_poll_event_kind(ctx.0, 0, ptr::null_mut(), &mut kind),
            GibsonStatus::ErrInvalidParam
        );
        assert_eq!(
            gibson_push_event_kind(ctx.0, ptr::null(), GIBSON_KEY_KIND_CHAR),
            GibsonStatus::ErrInvalidParam
        );
    }
}
