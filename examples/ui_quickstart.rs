//! Run: cargo run --example ui_quickstart
//! Tab moves focus; Enter activates. Ctrl-C returns to your shell.
//! Change one line below to try VAPOR95 or SWISS_SIGNAL.

use gibson::ui::prelude::*;

#[derive(Clone)]
enum Action {
    Build,
}

fn view(builds: &usize, _: &BuildCx) -> Element<Action> {
    screen().child(heading("MAKE SOMETHING EXCELLENT")).child(
        panel("YOUR FIRST CONSOLE")
            .child(status("Connected").tone(Tone::Success))
            .child(text(format!("{builds} builds completed")))
            .child(progress("context", 0.42))
            .child(button("Run build").key("build").on_press(Action::Build)),
    )
}

fn main() -> std::io::Result<()> {
    App::inline().skin(skins::BLACK_ICE).run(
        0usize,
        |builds, event| {
            if let AppEvent::Action(Action::Build) = event {
                *builds += 1;
            }
            Control::Continue
        },
        view,
    )?;
    Ok(())
}
