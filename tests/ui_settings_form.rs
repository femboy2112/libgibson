//! Tests for the v0.9.0 Settings form primitives: `toggle`, `form_row`,
//! `Element::error` and `Element::hint`. The `red_` tests were written first and
//! now pass; `guard_` tests passed against the stubs and must keep passing.
//! See docs/design/v0.9-settings-form.md.
use gibson::ui::prelude::*;
use gibson::ui::Key;
use gibson::{
    compute_layout, paint, Event, KeyCode, KeyEvent, KeyModifiers, Surface, TextInputState,
};
use std::time::Duration;

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}
fn key(value: &str) -> Key {
    Key::named(value)
}
fn event(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::empty()))
}
fn env() -> UiEnvironment {
    UiEnvironment {
        motion: MotionPreference::None,
        ..UiEnvironment::default()
    }
}
fn surface_of<A: Clone>(tree: &Element<A>) -> Surface {
    surface_at(tree, 80, 24)
}
fn surface_at<A: Clone>(tree: &Element<A>, width: u16, height: u16) -> Surface {
    let environment = UiEnvironment {
        width,
        height,
        ..env()
    };
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    let mut node = runtime.frame(tree, environment, ms(0)).unwrap().node;
    compute_layout(&mut node, width, height).unwrap();
    let mut surface = Surface::new(width, height);
    paint(&node, &mut surface);
    surface
}
/// Every cell, glyph and style, row-major.
fn cells_of(surface: &Surface) -> Vec<(String, gibson::Style)> {
    (0..surface.height)
        .flat_map(|y| (0..surface.width).map(move |x| (x, y)))
        .map(|(x, y)| {
            let cell = surface.get(x, y).unwrap();
            (cell.glyph.grapheme.to_string(), cell.style)
        })
        .collect()
}
fn lines_of(surface: &Surface) -> Vec<String> {
    (0..surface.height)
        .map(|y| {
            (0..surface.width)
                .filter_map(|x| {
                    let cell = surface.get(x, y).unwrap();
                    (!cell.is_continuation).then_some(cell.glyph.grapheme.as_str())
                })
                .collect::<String>()
        })
        .collect()
}
fn lines<A: Clone>(tree: &Element<A>) -> Vec<String> {
    lines_of(&surface_of(tree))
}
fn line_with(lines: &[String], needle: &str) -> Option<usize> {
    lines.iter().position(|line| line.contains(needle))
}

#[test]
fn red_toggle_on_renders_checked_marker() {
    let out = lines(&column::<()>().child(toggle("Enable sync", true)));
    assert!(
        out.iter().any(|l| l.contains("[x] Enable sync")),
        "missing checked marker: {out:?}"
    );
}

#[test]
fn red_toggle_off_renders_unchecked_marker() {
    let out = lines(&column::<()>().child(toggle("Enable sync", false)));
    assert!(
        out.iter().any(|l| l.contains("[ ] Enable sync")),
        "missing unchecked marker: {out:?}"
    );
    assert!(!out.iter().any(|l| l.contains("[x]")));
}

#[test]
fn red_toggle_stays_focusable_and_enter_space_fire_on_press() {
    let tree: Element<u8> = column().child(toggle("Notify", false).key("notify").on_press(7));
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    runtime.frame(&tree, env(), ms(0)).unwrap();
    assert!(runtime.set_focus(&key("notify")));
    assert_eq!(runtime.handle_event(&event(KeyCode::Enter)).actions, [7]);
    assert_eq!(
        runtime.handle_event(&event(KeyCode::Char(' '))).actions,
        [7]
    );
    // The marker is what is RED here: focus and press already work via `choice`.
    assert!(lines(&tree).iter().any(|l| l.contains("[ ] Notify")));
}

#[test]
fn red_form_row_shows_label_left_of_control_on_one_row() {
    let tree: Element<()> = column().child(form_row("Hostname", button("Edit")));
    let out = lines(&tree);
    let y = line_with(&out, "Hostname").expect("label not rendered");
    let line = &out[y];
    let (l, c) = (line.find("Hostname").unwrap(), line.find("Edit"));
    assert!(
        c.is_some_and(|c| c > l),
        "control not right of label: {line:?}"
    );
}

#[test]
fn red_form_row_aligns_the_control_column_across_rows() {
    let tree: Element<()> = column()
        .child(form_row("Host", button("Edit")))
        .child(form_row("Maximum connections", button("Edit")));
    let out = lines(&tree);
    let col = |label: &str| {
        let y = line_with(&out, label).expect("label not rendered");
        out[y].find("Edit").expect("control not on the label row")
    };
    assert_eq!(col("Host"), col("Maximum connections"));
}

#[test]
fn red_error_some_shows_danger_message_on_the_row_under_the_control() {
    let msg = "Port must be 1-65535";
    let tree: Element<()> =
        column().child(form_row("Port", button("Edit").key("port")).error(Some(msg.to_string())));
    let surface = surface_of(&tree);
    let out = lines_of(&surface);
    let row = line_with(&out, "Port").expect("row not rendered");
    let under = line_with(&out, msg).expect("error message not rendered");
    assert_eq!(under, row + 1, "message must be on the row directly under");
    // Style: the message uses the same fg as a Danger-toned label, not the body text.
    let reference: Element<()> = column().child(toned_label([tone_span(msg).tone(Tone::Danger)]));
    let ref_surface = surface_of(&reference);
    let x = out[under].find(msg).unwrap() as u16;
    let got = surface.get(x, under as u16).unwrap().style.fg;
    assert_eq!(got, ref_surface.get(0, 0).unwrap().style.fg);
    assert_ne!(got, surface.get(0, row as u16).unwrap().style.fg);
}

#[test]
fn red_error_keeps_the_element_focusable() {
    let tree: Element<u8> = column().child(
        form_row("Flag", toggle("Verbose", true).key("flag").on_press(3))
            .error(Some("Not allowed here".to_string())),
    );
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    runtime.frame(&tree, env(), ms(0)).unwrap();
    assert!(runtime.set_focus(&key("flag")));
    assert_eq!(runtime.handle_event(&event(KeyCode::Enter)).actions, [3]);
    assert!(lines(&tree).iter().any(|l| l.contains("Not allowed here")));
}

/// Guard: may PASS today (stubs ignore the call) and must pass after the
/// implementation too: `error(None)` is exactly "no error".
#[test]
fn guard_error_none_renders_identically_to_no_error() {
    let plain: Element<()> = column()
        .child(form_row("Port", button("Edit")))
        .child(button("After"));
    let none: Element<()> = column()
        .child(form_row("Port", button("Edit")).error(None))
        .child(button("After"));
    assert_eq!(lines(&plain), lines(&none));
}

#[test]
fn red_settings_column_of_eight_rows_and_one_error_fits_80x24() {
    let rows: Vec<Element<()>> = (0..8)
        .map(|i| {
            let row = form_row(format!("Setting {i}"), button(format!("Value{i}")));
            if i == 3 {
                row.error(Some("Value3 is out of range".to_string()))
            } else {
                row
            }
        })
        .collect();
    let tree: Element<()> = column().children(rows);
    let surface = surface_of(&tree);
    let out = lines_of(&surface);
    let mut last = 0;
    for i in 0..8 {
        let y = line_with(&out, &format!("Setting {i}"))
            .unwrap_or_else(|| panic!("Setting {i} clipped or missing: {out:?}"));
        assert!(
            out[y].contains(&format!("Value{i}")),
            "row {i} split: {:?}",
            out[y]
        );
        last = y;
    }
    let err = line_with(&out, "out of range").expect("error clipped or missing");
    assert_eq!(err, line_with(&out, "Setting 3").unwrap() + 1);
    assert!(last < 24 && out.iter().all(|l| l.chars().count() <= 80));
}

/// Guard: passes today. Editing keeps working through `form_row` (today a
/// pass-through) and must keep working once it wraps the control in a row.
#[test]
fn guard_text_input_editing_works_inside_form_row() {
    let state = TextInputState::with_text("ab");
    let tree: Element<String> = column().child(form_row(
        "Name",
        text_input(&state).key("name").on_edit(|next| next.text),
    ));
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    runtime.frame(&tree, env(), ms(0)).unwrap();
    assert!(runtime.set_focus(&key("name")));
    let outcome = runtime.handle_event(&event(KeyCode::Char('x')));
    assert!(outcome.consumed);
    assert_eq!(outcome.actions, ["abx".to_string()]);
}

fn tab_key(code: KeyCode) -> Event {
    event(code)
}
/// The focus after the first frame (the runtime focuses the first stop), then
/// after each of `presses` Tab presses: `presses + 1` entries.
fn tab_walk(tree: &Element<u8>, presses: usize) -> Vec<Option<Key>> {
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    runtime.frame(tree, env(), ms(0)).unwrap();
    let mut seen = vec![runtime.focus().cloned()];
    for _ in 0..presses {
        runtime.handle_event(&tab_key(KeyCode::Tab));
        seen.push(runtime.focus().cloned());
    }
    seen
}

#[test]
fn toggle_disabled_is_not_a_focus_stop_and_emits_nothing() {
    // Alone, so no other stop can take focus or the keys.
    let tree: Element<u8> = column().child(
        toggle("Locked", true)
            .key("locked")
            .on_press(9)
            .disabled(true),
    );
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    runtime.frame(&tree, env(), ms(0)).unwrap();
    // Same semantics as a disabled `choice`: focus cannot land on it.
    assert!(!runtime.set_focus(&key("locked")));
    assert_eq!(runtime.focus(), None);
    assert_eq!(
        runtime.handle_event(&event(KeyCode::Enter)).actions,
        [0u8; 0]
    );
    assert_eq!(
        runtime.handle_event(&event(KeyCode::Char(' '))).actions,
        [0u8; 0]
    );
    // It still renders, with its marker, in the disabled skin.
    let out = lines(&tree);
    assert!(out.iter().any(|l| l.contains("[x] Locked")), "{out:?}");
    // Tab skips it in favour of an enabled sibling.
    let both: Element<u8> = column()
        .child(
            toggle("Locked", true)
                .key("locked")
                .on_press(9)
                .disabled(true),
        )
        .child(toggle("Open", false).key("open").on_press(1));
    assert_eq!(tab_walk(&both, 1), [Some(key("open")), Some(key("open"))]);
}

#[test]
fn form_row_adds_no_focus_stop_and_choice_keeps_cycling_keys() {
    let tree: Element<u8> = column()
        .child(button("Before").key("before").on_press(0))
        .child(form_row(
            "Provider",
            choice("Local", true).key("provider").on_press(5),
        ))
        .child(button("After").key("after").on_press(0));
    assert_eq!(
        tab_walk(&tree, 3),
        [
            Some(key("before")),
            Some(key("provider")),
            Some(key("after")),
            Some(key("before")),
        ]
    );
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    runtime.frame(&tree, env(), ms(0)).unwrap();
    assert!(runtime.set_focus(&key("provider")));
    assert_eq!(runtime.handle_event(&event(KeyCode::Enter)).actions, [5]);
}

#[test]
fn form_row_around_a_read_only_control_is_skipped_by_tab() {
    let tree: Element<u8> = column()
        .child(form_row(
            "Provider",
            choice("Local", true).key("a").on_press(1),
        ))
        .child(form_row("Cloud sharing", label("off")))
        .child(form_row(
            "Reduced motion",
            toggle("on", true).key("c").on_press(2),
        ));
    assert_eq!(
        tab_walk(&tree, 2),
        [Some(key("a")), Some(key("c")), Some(key("a"))]
    );
}

#[test]
fn hint_none_is_byte_identical_to_omitting_it() {
    let build = |hint: bool| -> Element<()> {
        let row = form_row("Provider", button("Local").key("p"));
        let row = if hint { row.hint(None) } else { row };
        column().child(row).child(button("After"))
    };
    let (plain, none) = (surface_of(&build(false)), surface_of(&build(true)));
    assert_eq!(cells_of(&plain), cells_of(&none));
}

#[test]
fn error_none_is_byte_identical_cell_for_cell() {
    let build = |error: bool| -> Element<()> {
        let row = form_row("Provider", button("Local").key("p"));
        let row = if error { row.error(None) } else { row };
        column().child(row).child(button("After"))
    };
    assert_eq!(
        cells_of(&surface_of(&build(false))),
        cells_of(&surface_of(&build(true)))
    );
}

#[test]
fn hint_some_renders_after_the_control_on_its_row() {
    let tree: Element<()> = column().child(
        form_row("Reduced motion", toggle("on", true).key("m"))
            .hint(Some("applies now".to_string())),
    );
    let out = lines(&tree);
    let y = line_with(&out, "Reduced motion").expect("row");
    let (c, h) = (
        out[y].find("[x] on").expect("control"),
        out[y].find("applies now"),
    );
    assert!(
        h.is_some_and(|h| h > c),
        "hint not after control: {:?}",
        out[y]
    );
}

#[test]
fn long_hint_is_clipped_never_wrapped_and_adds_no_rows() {
    let long = "x".repeat(300);
    let with: Element<()> = column()
        .child(form_row("Provider", button("Local")).hint(Some(long)))
        .child(button("After"));
    let without: Element<()> = column()
        .child(form_row("Provider", button("Local")))
        .child(button("After"));
    let (a, b) = (lines(&with), lines(&without));
    assert_eq!(
        line_with(&a, "After"),
        line_with(&b, "After"),
        "row count changed"
    );
    let y = line_with(&a, "Provider").unwrap();
    assert!(a[y].contains("xxx"));
    assert!(a.iter().all(|l| l.chars().count() <= 80));
    // Nothing below the row but the next control.
    assert_eq!(
        a.iter()
            .filter(|l| l.contains('x') && !l.contains("Provider"))
            .count(),
        0
    );
}

#[test]
fn hint_is_dim_and_never_a_focus_stop() {
    let tree: Element<u8> = column()
        .child(
            form_row("Provider", button("Local").key("p").on_press(1))
                .hint(Some("note".to_string())),
        )
        .child(button("After").key("after").on_press(2));
    assert_eq!(
        tab_walk(&tree, 2),
        [Some(key("p")), Some(key("after")), Some(key("p"))]
    );
    let surface = surface_of(&tree);
    let out = lines_of(&surface);
    let y = line_with(&out, "note").unwrap();
    let x = out[y].find("note").unwrap() as u16;
    let hint = surface.get(x, y as u16).unwrap().style;
    let label_x = out[y].find("Provider").unwrap() as u16;
    assert_ne!(hint, surface.get(label_x, y as u16).unwrap().style);
}

#[test]
fn error_row_never_receives_focus() {
    let build = |error: bool| -> Element<u8> {
        let row = form_row("Port", button("Edit").key("port").on_press(1));
        let row = if error {
            row.error(Some("Port must be 1-65535".to_string()))
        } else {
            row
        };
        column()
            .child(row)
            .child(button("After").key("after").on_press(2))
    };
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    runtime.frame(&build(false), env(), ms(0)).unwrap();
    assert!(runtime.set_focus(&key("port")));
    // The error appears on the next frame; focus stays on the control's key.
    runtime.frame(&build(true), env(), ms(16)).unwrap();
    assert_eq!(runtime.focus(), Some(&key("port")));
    runtime.handle_event(&tab_key(KeyCode::Tab));
    assert_eq!(runtime.focus(), Some(&key("after")));
    runtime.handle_event(&tab_key(KeyCode::BackTab));
    assert_eq!(runtime.focus(), Some(&key("port")));
    runtime.handle_event(&tab_key(KeyCode::BackTab));
    assert_eq!(runtime.focus(), Some(&key("after")));
}

#[test]
fn narrow_settings_column_shows_every_label_in_full_at_60x20() {
    let tree: Element<u8> = column()
        .child(form_row(
            "Provider",
            choice("Local", true).key("provider").on_press(1),
        ))
        .child(form_row("Personality", label("Curious")))
        .child(form_row(
            "Reduced motion",
            toggle("on", true).key("motion").on_press(2),
        ))
        .child(form_row(
            "Avatar visible",
            toggle("visible", false).key("avatar").on_press(3),
        ))
        .child(form_row("Cloud sharing", label("off")));
    let out = lines_of(&surface_at(&tree, 60, 20));
    for needle in [
        "Provider",
        "Personality",
        "Reduced motion",
        "Avatar visible",
        "Cloud sharing",
    ] {
        assert!(
            line_with(&out, needle).is_some(),
            "{needle} missing: {out:?}"
        );
    }
    // Stacked: the label is on a row above its control.
    let l = line_with(&out, "Reduced motion").unwrap();
    assert!(out[l + 1].contains("[x] on"), "{out:?}");
}
