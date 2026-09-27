//! Regression coverage for the v0.2.1 consumer-correctness patch.
//!
//! Each test pins a defect reported by an external v0.2.0 consumer and fails
//! against the pre-patch library:
//!   #35 — Alt+<char> chords must not be consumed as text input.
//!   #37 — a keyed element must not dissolve from nothing on its first frame;
//!         entrance motion is opt-in via `.motion(...)`.
//!   #44 — an untitled rule needs no turbofish (`Node::separator`).
//!   #46 — a non-interactive context is detectable via `Context::is_interactive`.
//!   #48 E-01 — a rootless render is observable via `Context::empty_root_renders`.

use gibson::context::{Context, RenderMode};
use gibson::ui::prelude::*;
use gibson::ui::Key;
use gibson::{ColorDepth, Event, KeyCode, KeyEvent, KeyModifiers, Node, Style, TextInputState};
use std::time::Duration;

fn keyev(c: char, m: KeyModifiers) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(c), m))
}
fn full_motion() -> UiEnvironment {
    UiEnvironment {
        motion: MotionPreference::Full,
        ..UiEnvironment::default()
    }
}

// ---- #35: Alt chords are accelerators, not text -------------------------

#[test]
fn issue_35_text_input_distinguishes_text_from_chords() {
    // plain char -> inserted & consumed
    let mut s = TextInputState::new();
    assert!(s.handle_event(&keyev('a', KeyModifiers::empty())));
    assert_eq!(s.text, "a");

    // Shift+char -> uppercase text, inserted
    assert!(s.handle_event(&keyev('B', KeyModifiers::SHIFT)));
    assert_eq!(s.text, "aB");

    // Alt+char -> NOT consumed, NOT inserted (the reported bug)
    let mut s = TextInputState::new();
    assert!(
        !s.handle_event(&keyev('s', KeyModifiers::ALT)),
        "Alt+s must not be treated as text"
    );
    assert_eq!(s.text, "");

    // Alt+digit -> NOT consumed, NOT inserted
    assert!(
        !s.handle_event(&keyev('1', KeyModifiers::ALT)),
        "Alt+1 must not be treated as text"
    );
    assert_eq!(s.text, "");

    // Supported Ctrl editor chord -> consumed, no literal inserted
    let mut s = TextInputState::new();
    assert!(s.handle_event(&keyev('x', KeyModifiers::empty())));
    assert!(
        s.handle_event(&keyev('a', KeyModifiers::CONTROL)),
        "Ctrl-A is a supported editor chord"
    );
    assert_eq!(s.text, "x", "Ctrl-A must not insert a literal 'a'");

    // Unsupported Ctrl chord -> NOT consumed (stays routable to the app)
    assert!(
        !s.handle_event(&keyev('w', KeyModifiers::CONTROL)),
        "Ctrl-W must remain routable"
    );

    // Paste -> inserted
    let mut s = TextInputState::new();
    assert!(s.handle_event(&Event::Paste("hi".to_string())));
    assert_eq!(s.text, "hi");
}

#[test]
fn issue_35_ui_runtime_does_not_swallow_alt_hotkeys() {
    let state = TextInputState::new();
    let mut runtime = UiRuntime::<String>::new(skins::BLACK_ICE);
    let tree = column().child(text_input(&state).key("cmd").on_edit(|s| s.text.clone()));
    runtime
        .frame(&tree, UiEnvironment::default(), Duration::ZERO)
        .unwrap();
    assert!(runtime.set_focus(&Key::named("cmd")));

    // Alt+S is a global accelerator: a focused editor must not eat it.
    let outcome = runtime.handle_event(&keyev('s', KeyModifiers::ALT));
    assert!(
        !outcome.consumed,
        "Alt+S must not be consumed by a focused text input"
    );
    assert!(
        outcome.actions.is_empty(),
        "Alt+S must not produce an edit action"
    );

    // Plain 's' IS text: the editor owns it and emits the edited value.
    let outcome = runtime.handle_event(&keyev('s', KeyModifiers::empty()));
    assert!(
        outcome.consumed,
        "plain text must be owned by the focused editor"
    );
    assert_eq!(outcome.actions, vec!["s".to_string()]);
}

// ---- #37: keyed identity does not imply entrance motion -----------------

#[test]
fn issue_37_keyed_element_visible_on_first_frame() {
    let mut runtime = UiRuntime::<()>::new(skins::BLACK_ICE);
    let tree = row()
        .child(status("UNKEYED_STATUS"))
        .child(status("KEYED_STATUS").key("s1"));
    let frame = runtime.frame(&tree, full_motion(), Duration::ZERO).unwrap();

    // A keyed inert control must not start an implicit entrance animation.
    assert_eq!(
        runtime.active_animation_count(),
        0,
        "a keyed control with no explicit .motion() must not animate on entrance"
    );

    // ...and it must actually render at t=0 rather than dissolve to blanks.
    let mut ctx = Context::headless(RenderMode::Inline, 80, 5);
    ctx.set_color_depth(ColorDepth::TrueColor);
    ctx.set_root(frame.node);
    ctx.render_now().unwrap();
    let out = String::from_utf8_lossy(ctx.rendered_bytes()).to_string();
    assert!(
        out.contains("KEYED_STATUS"),
        "keyed element dissolved to nothing on first frame"
    );
    assert!(out.contains("UNKEYED_STATUS"));
}

#[test]
fn issue_37_explicit_entrance_motion_is_preserved() {
    let mut runtime = UiRuntime::<()>::new(skins::BLACK_ICE);
    let tree = row().child(status("ANIMATED").key("a1").motion(MotionRole::Enter));
    runtime.frame(&tree, full_motion(), Duration::ZERO).unwrap();
    assert!(
        runtime.active_animation_count() >= 1,
        "an element that explicitly requests .motion(Enter) must still animate"
    );
}

// ---- #44: untitled rule needs no turbofish ------------------------------

#[test]
fn issue_44_separator_needs_no_turbofish() {
    // Compiles with no type annotation — the whole point of the fix.
    let _sep = Node::separator(Style::default());
    // rule still accepts both titled and (turbofished) untitled forms.
    let _titled = Node::rule(Some("Section"), Style::default());
    let _untitled = Node::rule(None::<String>, Style::default());
}

// ---- #46: interactive vs non-interactive contexts -----------------------

#[test]
fn issue_46_headless_context_is_not_interactive() {
    let ctx = Context::headless(RenderMode::Inline, 40, 8);
    assert!(
        !ctx.is_interactive(),
        "a headless capture context is not an interactive terminal"
    );
}

// ---- #48 E-01: rootless render is observable ----------------------------

#[test]
fn issue_48_e01_rootless_render_is_observable() {
    let mut ctx = Context::headless(RenderMode::Inline, 40, 8);
    assert_eq!(ctx.empty_root_renders(), 0);

    // Rendering with no root set is a silent no-op — now counted.
    ctx.render_now().unwrap();
    assert_eq!(
        ctx.empty_root_renders(),
        1,
        "rootless render must be counted"
    );
    ctx.render_now().unwrap();
    assert_eq!(ctx.empty_root_renders(), 2);

    // A legitimately empty settled frame sets a root and does NOT increment.
    ctx.set_root(Node::col());
    ctx.render_now().unwrap();
    assert_eq!(
        ctx.empty_root_renders(),
        2,
        "a root-backed render (even an empty tree) is not an empty-root render"
    );
}
