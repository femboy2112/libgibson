//! RED amendments for the v0.9.0 NAV-1 freeze: `ViewTransition`, the outgoing-view contract
//! (`Element::scoped_keys` + `.disabled(true)`), overlay z-order, the pointer freeze and
//! `surface_fx::resolve_reset`.
//!
//! `red_*` tests fail today on an ASSERTION (the stubs compile and are wrong); `guard_*` tests
//! pass today and must keep passing. Design: `docs/design/v0.9-view-transitions.md`.
//!
//! OWNER RULE: every test that renders or applies transition effects keeps the synthetic
//! truecolor sprite (12x6 half blocks, formula below) present. No image or avatar asset is read.
use gibson::surface_fx::resolve_reset;
use gibson::ui::prelude::*;
use gibson::{
    compute_layout, paint, Cell, Color, Event, Glyph, KeyCode, KeyEvent, KeyModifiers, Node, Rect,
    Style, Surface, SurfaceFx,
};
use std::collections::BTreeSet;
use std::time::Duration;

const W: u16 = 60;
const H: u16 = 20;
const SPRITE_W: u16 = 12;
const SPRITE_H: u16 = 6;
const SPRITE_X: u16 = 6;
const SPRITE_Y: u16 = 4;
const HALF_BLOCK: &str = "\u{2580}";

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

// ---------------------------------------------------------------- sprite fixture

fn sprite_fg(x: u16, y: u16) -> Color {
    Color::Rgb(
        (16 + 17 * x) as u8,
        (16 + 30 * y) as u8,
        ((x * 7 + y * 11) % 200 + 20) as u8,
    )
}

fn sprite_bg(x: u16, y: u16) -> Color {
    match sprite_fg(x, y) {
        Color::Rgb(r, g, b) => Color::Rgb(255 - r, 255 - g, 255 - b),
        _ => unreachable!(),
    }
}

fn sprite_cell(x: u16, y: u16) -> Cell {
    Cell::new(
        Glyph::new(HALF_BLOCK),
        Style::new().fg(sprite_fg(x, y)).bg(sprite_bg(x, y)),
    )
}

fn paint_sprite(s: &mut Surface, ox: u16, oy: u16) {
    for y in 0..SPRITE_H {
        for x in 0..SPRITE_W {
            assert!(s.set_cell(ox + x, oy + y, sprite_cell(x, y)));
        }
    }
}

/// A full view of `W` x `h`: uniform dot backdrop with the sprite on top.
fn view_surface(h: u16) -> Surface {
    let mut s = Surface::new_transparent(W, h);
    for y in 0..h {
        s.print_str(0, y, &".".repeat(usize::from(W)), Style::new(), None);
    }
    paint_sprite(&mut s, SPRITE_X, SPRITE_Y);
    s
}

fn run(effect: SurfaceFx, s: &Surface) -> Surface {
    let mut out = s.clone();
    effect.apply(&mut out);
    out
}

fn blend(toward: Color, t: f32) -> SurfaceFx {
    SurfaceFx::Blend { toward, t }
}

// ---------------------------------------------------------------- effect inspection

fn offsets(v: &[SurfaceFx]) -> Vec<(i32, i32)> {
    v.iter()
        .filter_map(|fx| match fx {
            SurfaceFx::Offset { dx, dy } => Some((*dx, *dy)),
            _ => None,
        })
        .collect()
}

fn blends(v: &[SurfaceFx]) -> Vec<(Color, f32)> {
    v.iter()
        .filter_map(|fx| match fx {
            SurfaceFx::Blend { toward, t } => Some((*toward, *t)),
            _ => None,
        })
        .collect()
}

fn total_dx(v: &[SurfaceFx]) -> i32 {
    offsets(v).iter().map(|o| o.0).sum()
}

/// Start 100 ms, duration 400 ms: samples at 100/160/300/360 avoid rounding ties.
fn tr(kind: TransitionKind) -> ViewTransition {
    ViewTransition::new(ms(100), ms(400), kind)
}

const FULL: MotionPreference = MotionPreference::Full;
const FADE_TO: Color = Color::Rgb(8, 9, 10);

// ---------------------------------------------------------------- ViewTransition: progress

#[test]
fn red_view_transition_progress_is_eased_bounded_and_monotone() {
    let t = tr(TransitionKind::Slide(Dir::Left));
    assert_eq!(t.progress(ms(0), FULL), 0.0, "before start");
    assert_eq!(t.progress(ms(100), FULL), 0.0, "at start");
    let mut prev = 0.0f32;
    for now in (100..=500).step_by(40) {
        let p = t.progress(ms(now), FULL);
        assert!(
            (0.0..=1.0).contains(&p),
            "progress {p} out of range at {now}"
        );
        assert!(p >= prev, "progress went backwards at {now}");
        prev = p;
    }
    let mid = t.progress(ms(300), FULL);
    assert!(
        mid > 0.0 && mid < 1.0,
        "mid progress {mid} must be strictly inside"
    );
    // Ease-in-out: slow start, slow finish.
    assert!(t.progress(ms(200), FULL) < 0.25, "no ease-in");
    assert!(t.progress(ms(400), FULL) > 0.75, "no ease-out");
    assert_eq!(t.progress(ms(500), FULL), 1.0, "exactly 1 at the deadline");
    assert_eq!(t.progress(ms(9000), FULL), 1.0, "stays 1 after");
    assert!(!t.is_settled(ms(499), FULL));
    assert!(t.is_settled(ms(500), FULL));
    assert!(t.is_settled(ms(9000), FULL));
}

#[test]
fn red_motion_none_is_settled_immediately_with_no_effects() {
    for kind in [
        TransitionKind::Slide(Dir::Right),
        TransitionKind::Fade { toward: FADE_TO },
    ] {
        let t = tr(kind);
        let none = MotionPreference::None;
        for now in [0, 100, 300] {
            assert_eq!(t.progress(ms(now), none), 1.0, "{kind:?} at {now}");
            assert!(t.is_settled(ms(now), none), "{kind:?} at {now}");
            let (out, inc) = t.effects(ms(now), none, W);
            assert!(out.is_empty() && inc.is_empty(), "{kind:?} at {now}");
        }
    }
}

/// SPEC UPDATE (GOD #92): this test now covers `Fade` only. A Reduced `Slide` is a single cut
/// with no Blend; see `red_reduced_slide_is_one_cut_with_no_reset_frame`.
#[test]
fn red_reduced_never_slides_and_caps_at_80ms() {
    for kind in [TransitionKind::Fade { toward: FADE_TO }] {
        let t = tr(kind); // 400 ms at full motion
        let red = MotionPreference::Reduced;
        assert!(
            !t.is_settled(ms(140), red),
            "{kind:?}: 40 ms in is not settled"
        );
        assert!(t.is_settled(ms(180), red), "{kind:?}: capped at 80 ms");
        assert_eq!(t.progress(ms(180), red), 1.0, "{kind:?}");
        let mid = t.progress(ms(140), red);
        assert!(
            mid > 0.0 && mid < 1.0,
            "{kind:?}: reduced mid progress {mid}"
        );
        let (out, inc) = t.effects(ms(140), red, W);
        assert!(
            offsets(&out).is_empty() && offsets(&inc).is_empty(),
            "{kind:?}: Reduced slid"
        );
        assert_eq!(blends(&out).len(), 1, "{kind:?}: outgoing Blend-only fade");
        assert_eq!(blends(&inc).len(), 1, "{kind:?}: incoming Blend-only fade");
        let (out, inc) = t.effects(ms(180), red, W);
        assert!(
            out.is_empty() && inc.is_empty(),
            "{kind:?}: settled after the cap"
        );
    }
    // A duration already under the cap is not stretched.
    let short = ViewTransition::new(ms(0), ms(50), TransitionKind::Fade { toward: FADE_TO });
    assert!(short.is_settled(ms(50), MotionPreference::Reduced));
}

fn shows_reset(c: &Cell) -> bool {
    c.style.fg == Some(Color::Reset) || c.style.bg == Some(Color::Reset)
}

/// GOD #92 (ruling A on Q#91): under Reduced a `Slide` is ONE cut at t = 0. The incoming view
/// shows unmodified, the outgoing one is dropped (the transition is settled at once, so both
/// chains are empty and the pointer is live). No frame may show a `Reset` colour that neither
/// the outgoing nor the incoming cell has: a dip to the terminal default is a flash.
#[test]
fn red_reduced_slide_is_one_cut_with_no_reset_frame() {
    let red = MotionPreference::Reduced;
    let outgoing = view_surface(H);
    let mut incoming = Surface::new_transparent(W, H);
    paint_sprite(&mut incoming, SPRITE_X + 20, SPRITE_Y + 2);
    for dir in [Dir::Left, Dir::Right] {
        let t = tr(TransitionKind::Slide(dir));
        for now in [100u64, 120, 140, 160, 179, 180] {
            let (out_fx, in_fx) = t.effects(ms(now), red, W);
            let to_reset: Vec<(Color, f32)> = blends(&out_fx)
                .into_iter()
                .chain(blends(&in_fx))
                .filter(|b| b.0 == Color::Reset)
                .collect();
            assert!(
                to_reset.is_empty(),
                "{dir:?}@{now}: Reduced slide blends toward Reset: {to_reset:?}"
            );
            for (side, chain, view) in [("out", &out_fx, &outgoing), ("in", &in_fx, &incoming)] {
                let mut got = view.clone();
                SurfaceFx::apply_chain(chain, &mut got);
                for (i, c) in got.cells.iter().enumerate() {
                    assert!(
                        !shows_reset(c)
                            || shows_reset(&outgoing.cells[i])
                            || shows_reset(&incoming.cells[i]),
                        "{dir:?}@{now} {side}: cell {i} shows a Reset neither view has"
                    );
                }
            }
            let mut shown = incoming.clone();
            SurfaceFx::apply_chain(&in_fx, &mut shown);
            assert_eq!(shown, incoming, "{dir:?}@{now}: incoming view not shown unmodified");
        }
        assert!(t.is_settled(ms(100), red), "{dir:?}: cut at t = 0");
        assert!(t.accepts_pointer(ms(100), red), "{dir:?}: pointer settles at once");
    }
}

// ---------------------------------------------------------------- ViewTransition: effects

#[test]
fn red_slide_effects_are_horizontal_with_opposite_signs() {
    for dir in [Dir::Left, Dir::Right] {
        let t = tr(TransitionKind::Slide(dir));
        let s = if dir == Dir::Left { 1 } else { -1 };
        let w = f32::from(W);
        for now in [100u64, 160, 300, 360] {
            let p = t.progress(ms(now), FULL);
            let (out, inc) = t.effects(ms(now), FULL, W);
            assert!(
                blends(&out).is_empty() && blends(&inc).is_empty(),
                "{dir:?}@{now}: slide used Blend"
            );
            assert!(
                offsets(&out).len() <= 1 && offsets(&inc).len() <= 1,
                "{dir:?}@{now}"
            );
            assert!(
                offsets(&out).iter().chain(&offsets(&inc)).all(|o| o.1 == 0),
                "{dir:?}@{now}: dy must be 0"
            );
            assert_eq!(
                total_dx(&out),
                -s * (p * w).round() as i32,
                "{dir:?}@{now} outgoing"
            );
            assert_eq!(
                total_dx(&inc),
                s * ((1.0 - p) * w).round() as i32,
                "{dir:?}@{now} incoming"
            );
        }
        // Mid transition: exactly one Offset per side, strictly opposite signs.
        let (out, inc) = t.effects(ms(300), FULL, W);
        let (o, i) = (offsets(&out), offsets(&inc));
        assert_eq!((o.len(), i.len()), (1, 1), "{dir:?}: one Offset per side");
        assert!(
            o[0].0 * i[0].0 < 0,
            "{dir:?}: dx must have opposite signs: {o:?} {i:?}"
        );
    }
}

#[test]
fn red_fade_effects_blend_both_sides_toward_one_colour_with_complementary_t() {
    let t = tr(TransitionKind::Fade { toward: FADE_TO });
    for now in [100u64, 160, 300, 360] {
        let p = t.progress(ms(now), FULL);
        let (out, inc) = t.effects(ms(now), FULL, W);
        assert!(
            offsets(&out).is_empty() && offsets(&inc).is_empty(),
            "@{now}: fade moved"
        );
        let (bo, bi) = (blends(&out), blends(&inc));
        assert_eq!((bo.len(), bi.len()), (1, 1), "@{now}: one Blend per side");
        assert_eq!((bo[0].0, bi[0].0), (FADE_TO, FADE_TO), "@{now}");
        assert!(
            (bo[0].1 - p).abs() < 1e-5,
            "@{now}: outgoing t {} vs p {p}",
            bo[0].1
        );
        assert!(
            (bi[0].1 - (1.0 - p)).abs() < 1e-5,
            "@{now}: incoming t {}",
            bi[0].1
        );
    }
}

/// Passes today only because the stub emits nothing; it must keep passing for the real thing:
/// at and after settling the settled tree is the plain incoming view.
#[test]
fn guard_settled_transition_emits_no_effects() {
    for kind in [
        TransitionKind::Slide(Dir::Left),
        TransitionKind::Fade { toward: FADE_TO },
    ] {
        let t = tr(kind);
        for now in [500u64, 501, 9000] {
            let (out, inc) = t.effects(ms(now), FULL, W);
            assert!(out.is_empty() && inc.is_empty(), "{kind:?}@{now}");
        }
    }
}

/// Sprite present: the chains, applied to a surface holding the synthetic sprite, move it
/// intact (slide) or lerp its own colours exactly (fade).
#[test]
fn red_effects_applied_to_the_sprite_view_move_and_blend_it_exactly() {
    let view = view_surface(H);

    let slide = tr(TransitionKind::Slide(Dir::Left));
    let (out, inc) = slide.effects(ms(300), FULL, W);
    let mut moved_out = view.clone();
    SurfaceFx::apply_chain(&out, &mut moved_out);
    assert_ne!(moved_out, view, "outgoing slide left the view untouched");
    let mut moved_in = view.clone();
    SurfaceFx::apply_chain(&inc, &mut moved_in);
    assert_ne!(moved_in, view, "incoming slide left the view untouched");
    let dx = offsets(&inc).first().map(|o| o.0);
    assert!(dx.is_some(), "incoming slide has no Offset");
    let dx = dx.unwrap_or(0);
    for y in 0..SPRITE_H {
        for x in 0..SPRITE_W {
            let at = moved_in.get(SPRITE_X + x + dx as u16, SPRITE_Y + y);
            assert_eq!(
                at,
                Some(&sprite_cell(x, y)),
                "sprite cell ({x},{y}) after dx {dx}"
            );
        }
    }

    let fade = tr(TransitionKind::Fade { toward: FADE_TO });
    let (_, inc) = fade.effects(ms(300), FULL, W);
    let t = blends(&inc).first().map(|b| b.1);
    assert!(t.is_some(), "incoming fade has no Blend");
    let t = t.unwrap_or(0.0);
    let mut faded = view.clone();
    SurfaceFx::apply_chain(&inc, &mut faded);
    for y in 0..SPRITE_H {
        for x in 0..SPRITE_W {
            let at = faded.get(SPRITE_X + x, SPRITE_Y + y);
            assert_eq!(
                at.map(|c| c.style.fg),
                Some(sprite_fg(x, y).lerp_resolved(FADE_TO, t)),
                "sprite fg ({x},{y}) t {t}"
            );
            assert_eq!(
                at.map(|c| c.style.bg),
                Some(sprite_bg(x, y).lerp_resolved(FADE_TO, t)),
                "sprite bg ({x},{y}) t {t}"
            );
        }
    }
}

// ---------------------------------------------------------------- pointer freeze

#[test]
fn red_pointer_is_frozen_mid_transition_and_live_once_settled() {
    for kind in [
        TransitionKind::Slide(Dir::Left),
        TransitionKind::Fade { toward: FADE_TO },
    ] {
        let t = tr(kind);
        for now in [100u64, 160, 300, 499] {
            assert!(
                !t.accepts_pointer(ms(now), FULL),
                "{kind:?}@{now}: pointer must be frozen"
            );
        }
        for now in [500u64, 501, 9000] {
            assert!(
                t.accepts_pointer(ms(now), FULL),
                "{kind:?}@{now}: pointer must be live"
            );
        }
        // Reduced: frozen for at most 80 ms, then live.
        assert!(
            !t.accepts_pointer(ms(140), MotionPreference::Reduced),
            "{kind:?} reduced mid"
        );
        assert!(
            t.accepts_pointer(ms(180), MotionPreference::Reduced),
            "{kind:?} reduced settled"
        );
        // None: there is no transition to wait for.
        assert!(
            t.accepts_pointer(ms(100), MotionPreference::None),
            "{kind:?} none"
        );
    }
}

// ---------------------------------------------------------------- outgoing-view contract

fn env() -> UiEnvironment {
    UiEnvironment {
        width: W,
        height: H,
        motion: MotionPreference::None,
        ..UiEnvironment::default()
    }
}

fn keyed_view(prefix: &str) -> Element<()> {
    column()
        .key(format!("{prefix}view"))
        .child(button("Save").key(format!("{prefix}save")).on_press(()))
        .child(button("Cancel").key(format!("{prefix}cancel")).on_press(()))
}

fn tab() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()))
}

fn named(s: &str) -> Key {
    Key::from(s)
}

#[test]
fn red_scoped_keys_lets_two_copies_of_a_view_coexist_and_leaves_the_tab_ring() {
    let tree: Element<()> = column()
        .child(keyed_view("").scoped_keys("out").disabled(true))
        .child(keyed_view(""));
    let mut rt: UiRuntime<()> = UiRuntime::new(skins::BLACK_ICE);
    let result = rt.frame(&tree, env(), ms(0));
    assert!(
        result.is_ok(),
        "two copies of a view, one scoped, must compile: {:?}",
        result.as_ref().err()
    );
    let Ok(compiled) = result else { return };
    for k in ["view", "save", "cancel"] {
        assert!(compiled.keys.contains_key(&named(k)), "missing key {k}");
        let out = Key::scoped("out", k);
        assert!(compiled.keys.contains_key(&out), "missing key {out:?}");
    }
    assert!(
        compiled.keys[&Key::scoped("out", "save")].disabled,
        "outgoing must be disabled"
    );
    assert!(!compiled.keys[&named("save")].disabled);
    assert_eq!(
        rt.focus(),
        Some(&named("save")),
        "focus must start on the incoming view"
    );
    assert!(
        !rt.set_focus(&Key::scoped("out", "save")),
        "outgoing control took focus"
    );
    for _ in 0..6 {
        rt.handle_event(&tab());
        let focused = rt.focus().cloned();
        assert!(
            !matches!(&focused, Some(Key::Scoped { scope, .. }) if scope == "out"),
            "Tab reached the outgoing view: {focused:?}"
        );
    }
}

#[test]
fn red_scoped_keys_prefixes_every_explicit_key_including_overlays_and_only_those() {
    fn nested() -> Element<()> {
        screen()
            .key("root")
            .child(
                column()
                    .key("col")
                    .child(button("A").key("a").on_press(()))
                    .child(text("plain")),
            )
            .overlay(modal("M").key("m").child(button("B").key("b").on_press(())))
    }
    let mut rt: UiRuntime<()> = UiRuntime::new(skins::BLACK_ICE);
    let plain: BTreeSet<Key> = match rt.frame(&nested(), env(), ms(0)) {
        Ok(c) => c.keys.keys().cloned().collect(),
        Err(_) => BTreeSet::new(),
    };
    assert!(plain.len() >= 5, "baseline tree did not compile: {plain:?}");
    let expected: BTreeSet<Key> = plain
        .iter()
        .map(|k| match k {
            key @ Key::Named(_) => Key::scoped("v", key.clone()),
            other => other.clone(),
        })
        .collect();
    let mut rt: UiRuntime<()> = UiRuntime::new(skins::BLACK_ICE);
    let scoped: BTreeSet<Key> = match rt.frame(&nested().scoped_keys("v"), env(), ms(0)) {
        Ok(c) => c.keys.keys().cloned().collect(),
        Err(_) => BTreeSet::new(),
    };
    assert_eq!(
        scoped, expected,
        "explicit keys (children and overlays) get the prefix; unkeyed Path keys do not"
    );
}

/// Scoping must never alias a user key: scope `a` + key `b` and a sibling user key `a.b` are two
/// different controls. GREEN either encodes scoped keys unambiguously (the frame compiles with two
/// distinct keys) or rejects the reserved separator with its own error; a `DuplicateKey` between
/// them (naive `"{prefix}.{key}"` joining) or a silently unscoped key both fail.
#[test]
fn red_scoped_keys_never_alias_a_user_key_containing_the_separator() {
    let tree: Element<()> = column()
        .child(
            column()
                .child(button("Scoped").key("b").on_press(()))
                .scoped_keys("a"),
        )
        .child(button("User").key("a.b").on_press(()));
    let mut rt: UiRuntime<()> = UiRuntime::new(skins::BLACK_ICE);
    match rt.frame(&tree, env(), ms(0)) {
        Ok(c) => {
            let explicit: Vec<&Key> = c
                .keys
                .keys()
                .filter(|k| !matches!(k, Key::Path(_)))
                .collect();
            assert!(
                !explicit.contains(&&Key::named("b")),
                "the scoped control kept its bare key `b`: scoping was not applied: {explicit:?}"
            );
            assert!(
                explicit.contains(&&Key::named("a.b")),
                "the user's own key `a.b` must survive unchanged: {explicit:?}"
            );
            assert_eq!(
                explicit.len(),
                2,
                "two explicit keys, one per control: {explicit:?}"
            );
        }
        Err(UiError::DuplicateKey(k)) => {
            panic!("scope `a` + key `b` aliased the user key `a.b`: DuplicateKey({k:?})")
        }
        Err(other) => {
            // Rejecting the reserved separator is an accepted GREEN, but it must say so.
            let msg = other.to_string();
            assert!(
                msg.contains("separator") || msg.contains("reserved"),
                "rejection must name the reserved separator, got: {msg}"
            );
        }
    }
}

/// Hand-namespaced keys compile today: the disabled outgoing view is inert and leaves the Tab
/// ring. This is the half of the contract that needs no new API.
#[test]
fn guard_disabled_outgoing_controls_are_not_focusable() {
    let tree: Element<()> = column()
        .child(keyed_view("out_").disabled(true))
        .child(keyed_view(""));
    let mut rt: UiRuntime<()> = UiRuntime::new(skins::BLACK_ICE);
    let result = rt.frame(&tree, env(), ms(0));
    assert!(result.is_ok(), "{:?}", result.as_ref().err());
    let Ok(compiled) = result else { return };
    for k in ["out_view", "out_save", "out_cancel"] {
        assert!(
            compiled.keys[&named(k)].disabled,
            "{k} must inherit disabled"
        );
    }
    assert!(!compiled.keys[&named("save")].disabled);
    assert_eq!(rt.focus(), Some(&named("save")));
    assert!(!rt.set_focus(&named("out_save")));
    for _ in 0..6 {
        rt.handle_event(&tab());
        let focused = rt.focus().map(ToString::to_string).unwrap_or_default();
        assert!(
            !focused.starts_with("out_"),
            "Tab reached the outgoing view: {focused}"
        );
    }
}

/// Contrast for the guard above: without `.disabled(true)` the outgoing view WOULD own focus
/// first, so the guard is not vacuous.
#[test]
fn guard_enabled_outgoing_view_would_own_the_tab_ring() {
    let tree: Element<()> = column().child(keyed_view("out_")).child(keyed_view(""));
    let mut rt: UiRuntime<()> = UiRuntime::new(skins::BLACK_ICE);
    assert!(rt.frame(&tree, env(), ms(0)).is_ok());
    assert_eq!(rt.focus(), Some(&named("out_save")));
}

// ---------------------------------------------------------------- overlay z-order

fn paint_node(mut node: Node) -> Surface {
    compute_layout(&mut node, W, H).unwrap();
    let mut surface = Surface::new(W, H);
    paint(&node, &mut surface);
    surface
}

fn find(s: &Surface, needle: &str) -> Option<(u16, u16)> {
    let chars: Vec<String> = needle.chars().map(|c| c.to_string()).collect();
    for y in 0..s.height {
        for x in 0..s.width {
            if usize::from(x) + chars.len() > usize::from(s.width) {
                break;
            }
            let hit = chars.iter().enumerate().all(|(i, c)| {
                s.get(x + i as u16, y)
                    .is_some_and(|cell| cell.glyph.grapheme.as_str() == c)
            });
            if hit {
                return Some((x, y));
            }
        }
    }
    None
}

const MODAL_TEXT: &str = "MODAL-SENTINEL";
const TOAST_TEXT: &str = "TOAST-SENTINEL";

fn overlay_frame(fx: Vec<SurfaceFx>) -> Surface {
    let body = raw(Node::raster(view_surface(H))
        .width(f32::from(W))
        .height(f32::from(H)));
    let tree: Element<()> = screen()
        .height(H)
        .child(body.post_process(fx))
        .overlay(modal("DIALOG").key("dlg").width(26).child(text(MODAL_TEXT)))
        .overlay(toast(TOAST_TEXT).key("toast").width(24).height(3));
    let compiled = UiRuntime::new(skins::BLACK_ICE).frame(&tree, env(), ms(0));
    assert!(compiled.is_ok(), "{:?}", compiled.as_ref().err());
    match compiled {
        Ok(c) => paint_node(c.node),
        Err(_) => Surface::new(W, H),
    }
}

#[test]
fn red_modal_and_toast_cells_are_unmoved_and_unblended_while_the_body_transitions() {
    let reference = overlay_frame(vec![]);
    let modal_at = find(&reference, MODAL_TEXT);
    let toast_at = find(&reference, TOAST_TEXT);
    assert!(
        modal_at.is_some(),
        "modal sentinel not painted in the reference frame"
    );
    assert!(
        toast_at.is_some(),
        "toast sentinel not painted in the reference frame"
    );
    let (mx, my) = modal_at.unwrap_or((0, 0));
    let (tx, ty) = toast_at.unwrap_or((0, 0));
    // The sentinel text plus one cell of modal chrome around it; the toast's text cells.
    let modal_rect = Rect::new(mx.saturating_sub(1), my.saturating_sub(1), 16, 3);
    let toast_rect = Rect::new(tx, ty, 14, 1);

    for kind in [
        TransitionKind::Slide(Dir::Left),
        TransitionKind::Slide(Dir::Right),
        TransitionKind::Fade { toward: FADE_TO },
    ] {
        let t = tr(kind);
        for side in ["outgoing", "incoming"] {
            let mut body_moved_mid = false;
            for p in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
                let now = ms(100 + (400.0 * p) as u64);
                let (out, inc) = t.effects(now, FULL, W);
                let got = overlay_frame(if side == "outgoing" { out } else { inc });
                for y in 0..H {
                    for x in 0..W {
                        let protected = modal_rect.contains(x, y) || toast_rect.contains(x, y);
                        let same = got.get(x, y) == reference.get(x, y);
                        if protected {
                            assert!(
                                same,
                                "{kind:?} {side} p {p}: overlay cell ({x},{y}) changed"
                            );
                        } else if p == 0.5 && !same {
                            body_moved_mid = true;
                        }
                    }
                }
            }
            assert!(
                body_moved_mid,
                "{kind:?} {side}: the body did not move or fade at p = 0.5 (RED stub)"
            );
        }
    }
}

// ---------------------------------------------------------------- header / prompt stay fixed

#[test]
fn red_slide_of_the_body_leaves_header_and_prompt_rows_byte_identical() {
    const HEADER: &str = "HEADER-ROW";
    const PROMPT: &str = "PROMPT> ";
    fn frame(fx: Vec<SurfaceFx>) -> Surface {
        let body = raw(Node::raster(view_surface(H - 2))
            .width(f32::from(W))
            .height(f32::from(H - 2)));
        let tree: Element<()> = screen()
            .height(H)
            .child(text(HEADER))
            .child(body.post_process(fx))
            .child(text(PROMPT));
        let compiled = UiRuntime::new(skins::BLACK_ICE).frame(&tree, env(), ms(0));
        assert!(compiled.is_ok(), "{:?}", compiled.as_ref().err());
        match compiled {
            Ok(c) => paint_node(c.node),
            Err(_) => Surface::new(W, H),
        }
    }
    let reference = frame(vec![]);
    let header = find(&reference, HEADER);
    let prompt = find(&reference, PROMPT);
    assert!(
        header.is_some() && prompt.is_some(),
        "header/prompt rows not painted"
    );
    let (hy, py) = (header.map_or(0, |p| p.1), prompt.map_or(0, |p| p.1));
    assert_ne!(hy, py);

    let t = tr(TransitionKind::Slide(Dir::Left));
    for side in ["outgoing", "incoming"] {
        let mut moved_mid = false;
        for p in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            let (out, inc) = t.effects(ms(100 + (400.0 * p) as u64), FULL, W);
            let got = frame(if side == "outgoing" { out } else { inc });
            for y in 0..H {
                for x in 0..W {
                    let same = got.get(x, y) == reference.get(x, y);
                    if y == hy || y == py {
                        assert!(same, "{side} p {p}: fixed row {y} changed at column {x}");
                    } else if p == 0.5 && !same {
                        moved_mid = true;
                    }
                }
            }
        }
        assert!(
            moved_mid,
            "{side}: the body did not slide at p = 0.5 (RED stub)"
        );
    }
}

// ---------------------------------------------------------------- resolve_reset

const RESET_FG: Color = Color::Rgb(230, 230, 230);
const RESET_BG: Color = Color::Rgb(20, 20, 20);
const TARGET: Color = Color::Rgb(100, 200, 0);
const RGB: Color = Color::Rgb(10, 20, 30);

/// Reset in fg, bg, or both, one unset-channel cell, plus the sprite.
fn reset_scene() -> Surface {
    let mut s = Surface::new_transparent(W, H);
    s.print_str(0, 0, "a", Style::new().fg(Color::Reset).bg(RGB), None);
    s.print_str(1, 0, "b", Style::new().fg(RGB).bg(Color::Reset), None);
    s.print_str(
        2,
        0,
        "c",
        Style::new().fg(Color::Reset).bg(Color::Reset),
        None,
    );
    s.print_str(3, 0, "d", Style::new().fg(RGB), None); // bg unset (None)
    paint_sprite(&mut s, SPRITE_X, SPRITE_Y);
    s
}

#[test]
fn red_resolve_reset_replaces_reset_channels_with_the_given_colours() {
    let before = reset_scene();
    let mut s = before.clone();
    resolve_reset(&mut s, RESET_FG, RESET_BG);
    let at = |x| s.get(x, 0).map(|c| (c.style.fg, c.style.bg));
    assert_eq!(at(0), Some((Some(RESET_FG), Some(RGB))));
    assert_eq!(at(1), Some((Some(RGB), Some(RESET_BG))));
    assert_eq!(at(2), Some((Some(RESET_FG), Some(RESET_BG))));
    // Glyphs, attributes and every resolved sprite cell are untouched.
    for (a, b) in before.cells.iter().zip(&s.cells) {
        assert_eq!(a.glyph, b.glyph);
        assert_eq!(a.transparent, b.transparent);
        assert_eq!(a.style.bold, b.style.bold);
    }
    for y in 0..SPRITE_H {
        for x in 0..SPRITE_W {
            assert_eq!(s.get(SPRITE_X + x, SPRITE_Y + y), Some(&sprite_cell(x, y)));
        }
    }
}

/// Passes today (no-op) and must keep passing: unset channels and transparent cells are not
/// invented into concrete colours, and fully resolved cells are not rewritten.
#[test]
fn guard_resolve_reset_leaves_unset_transparent_and_resolved_cells_alone() {
    let before = reset_scene();
    let mut s = before.clone();
    resolve_reset(&mut s, RESET_FG, RESET_BG);
    assert_eq!(
        s.get(3, 0),
        before.get(3, 0),
        "an unset (None) channel was resolved"
    );
    assert_eq!(
        s.get(40, 15),
        before.get(40, 15),
        "a transparent cell was touched"
    );
    assert!(s.get(40, 15).is_some_and(|c| c.transparent));
    for y in 0..SPRITE_H {
        for x in 0..SPRITE_W {
            assert_eq!(s.get(SPRITE_X + x, SPRITE_Y + y), Some(&sprite_cell(x, y)));
        }
    }
}

#[test]
fn red_resolve_reset_then_blend_has_no_reset_left_and_no_step_pop() {
    let mut s = reset_scene();
    resolve_reset(&mut s, RESET_FG, RESET_BG);
    for c in s.cells.iter().filter(|c| !c.transparent) {
        assert_ne!(
            c.style.fg,
            Some(Color::Reset),
            "Reset fg left after resolve_reset"
        );
        assert_ne!(
            c.style.bg,
            Some(Color::Reset),
            "Reset bg left after resolve_reset"
        );
    }
    // Sweep 0..=1 in 11 steps; the formerly-Reset cells must stay Rgb and move gradually:
    // a step rule would jump by the whole distance at t = 0.5.
    let mut prev: Vec<(u8, u8, u8)> = Vec::new();
    for i in 0..=10 {
        let got = run(blend(TARGET, i as f32 / 10.0), &s);
        let mut now = Vec::new();
        for x in 0..3u16 {
            let c = got.get(x, 0).map(|c| c.style).unwrap_or_default();
            for ch in [c.fg, c.bg] {
                assert!(
                    matches!(ch, Some(Color::Rgb(..))),
                    "step {i} cell {x}: {ch:?}"
                );
                now.push(ch.map_or((0, 0, 0), Color::to_rgb));
            }
        }
        if !prev.is_empty() {
            for (a, b) in prev.iter().zip(&now) {
                let d = |p: u8, q: u8| (i32::from(p) - i32::from(q)).abs();
                assert!(
                    d(a.0, b.0).max(d(a.1, b.1)).max(d(a.2, b.2)) <= 30,
                    "pop at step {i}: {a:?} -> {b:?}"
                );
            }
        }
        prev = now;
    }
}

// ---------------------------------------------------------------- doc-contract guards

/// Alt+M under a modal: `UiRuntime::handle_event` consumes it, so the app must route global
/// hotkeys before the runtime. Without a modal the chord stays unconsumed.
#[test]
fn guard_modal_consumes_alt_chords_so_apps_route_global_hotkeys_first() {
    let alt_m = Event::Key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT));
    let base = || column().child(button("Base").key("base").on_press(1u8));

    let mut rt: UiRuntime<u8> = UiRuntime::new(skins::BLACK_ICE);
    let with_modal: Element<u8> = base().overlay(
        modal("M")
            .key("m")
            .child(button("In").key("in").on_press(2u8)),
    );
    assert!(rt.frame(&with_modal, env(), ms(0)).is_ok());
    assert!(rt.modal_depth() == 1);
    let out = rt.handle_event(&alt_m);
    assert!(
        out.consumed && out.actions.is_empty(),
        "modal must consume Alt+M"
    );

    let mut rt: UiRuntime<u8> = UiRuntime::new(skins::BLACK_ICE);
    assert!(rt.frame(&base(), env(), ms(0)).is_ok());
    let out = rt.handle_event(&alt_m);
    assert!(!out.consumed, "without a modal Alt+M is left to the app");
}

/// Read-only rows that explain themselves are focusable `button(..).on_press(Hint)`, not
/// `.disabled(true)`: a disabled control is not in the Tab ring.
#[test]
fn guard_hint_button_is_focusable_but_disabled_row_is_not() {
    let hint = || button("Why read-only?").key("hint").on_press(7u8);
    let mut rt: UiRuntime<u8> = UiRuntime::new(skins::BLACK_ICE);
    assert!(rt.frame(&column().child(hint()), env(), ms(0)).is_ok());
    assert_eq!(rt.focus(), Some(&named("hint")));
    let out = rt.handle_event(&Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::empty(),
    )));
    assert_eq!(out.actions, [7u8]);

    let mut rt: UiRuntime<u8> = UiRuntime::new(skins::BLACK_ICE);
    assert!(rt
        .frame(&column().child(hint().disabled(true)), env(), ms(0))
        .is_ok());
    assert_eq!(rt.focus(), None, "a disabled row must not be focusable");
}
