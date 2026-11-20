//! RED slice for the v0.9.0 Settings form primitives: `toggle`, `form_row` and
//! `Element::error`. The API stubs compile but do not implement the behaviour,
//! so the `red_` tests below fail on assertions today. `guard_` tests pass today
//! and must keep passing. See docs/design/v0.9-settings-form.md.
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
fn surface_of<A>(tree: &Element<A>) -> Surface {
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    let mut node = runtime.frame(tree, env(), ms(0)).unwrap().node;
    compute_layout(&mut node, 80, 24).unwrap();
    let mut surface = Surface::new(80, 24);
    paint(&node, &mut surface);
    surface
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
fn lines<A>(tree: &Element<A>) -> Vec<String> {
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
