//! Recipe: **a show is just `f(edit)`.**
//!
//! The one law: a [`gibson::timeline::Timeline`] stores cues on a single edit clock
//! and resolves, for any time `t`, which cue is on screen and how far into its own
//! window it is. Nothing here reads a wall clock, owns a camera, or knows what a
//! "scene" is — the timeline carries an opaque payload (here a 3-variant enum) and
//! you decide what to draw for it. That is the whole trick behind seeking and
//! deterministic replay: the frame at `t` is a pure function of `t`.
//!
//! Teaches: `Timeline::cut`, `top()`, a cue's `local`/`progress`, and direct seek.
//!
//!   cargo run --example show_minimal            # live, loops the 10.5 s show
//!   cargo run --example show_minimal -- at 6.0  # print the single frame at t=6.0 s

use gibson::input::{Event, KeyCode, KeyModifiers};
use gibson::raster::RgbRaster;
use gibson::timeline::Timeline;
use gibson::ui::{raster, screen, skins, App, AppEvent, BuildCx, Control, Element};
use gibson::Surface;

/// This show has no interactive actions, so its message type is uninhabited.
#[derive(Clone)]
enum Msg {}

/// The opaque payload the timeline carries. The timeline never looks inside it —
/// `render` does.
#[derive(Clone, Copy)]
enum Scene {
    Title,
    Rise,
    Rest,
}

/// The edit: three hard cuts, end to end. This *is* the show — everything else is
/// a pure read of it.
fn film() -> Timeline<Scene> {
    Timeline::new()
        .cut(2.5, Scene::Title)
        .cut(4.0, Scene::Rise)
        .cut(4.0, Scene::Rest)
}

/// Draw one cue. `local` is seconds since *this* cue began; `progress` is `0..1`
/// across its window. A real show would animate from these; here they just drive a
/// label and a rising bar so you can see the clock move.
fn render(scene: Scene, local: f32, progress: f32, w: u16, h: u16) -> Surface {
    let ph = h.saturating_mul(2).max(2);
    let mut r = RgbRaster::new(w.max(1), ph);
    match scene {
        Scene::Title => r.clear((18, 10, 30)),
        Scene::Rise => r.clear((10, 14, 34)),
        Scene::Rest => r.clear((12, 24, 26)),
    }
    // A bar that fills with the cue's progress — the clock, made visible.
    let fill = (progress.clamp(0.0, 1.0) * (w as f32)) as i32;
    let bar_y = (ph as i32) / 2;
    for x in 0..fill {
        for dy in -1..=1 {
            r.set(x, bar_y + dy, (120, 200, 255));
        }
    }
    let mut surface = r.to_surface();
    let (label, sub) = match scene {
        Scene::Title => ("GIBSON", "a show is f(edit)"),
        Scene::Rise => ("RISE", "progress drives the bar"),
        Scene::Rest => ("REST", "local time keeps counting"),
    };
    if w >= 8 && h >= 4 {
        let cx = w / 2;
        surface.bake_text_centered(cx, h / 2 - 2, label, (245, 238, 255), true);
        surface.bake_text_centered(cx, h / 2 - 1, sub, (198, 180, 230), false);
        let clock = format!("t_local = {local:4.1}s");
        surface.bake_text_centered(cx, h / 2 + 2, &clock, (150, 170, 210), false);
    }
    surface
}

/// The frame at edit time `edit`: resolve the dominant cue and draw it.
fn frame(w: u16, h: u16, edit: f32) -> Surface {
    let tl = film();
    match tl.top(edit) {
        Some(a) => render(*a.payload, a.local, a.progress, w, h),
        // Gapless cuts never leave a gap in `[0, duration)`; past the end is black.
        None => RgbRaster::new(w.max(1), h.saturating_mul(2).max(2)).to_surface(),
    }
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Direct seek: render exactly one frame at a chosen time and print it. This is
    // the whole point of an explicit clock — any frame, in isolation, deterministically.
    if args.get(1).map(String::as_str) == Some("at") {
        let t: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        for line in frame(100, 30, t).to_visible_lines() {
            println!("{line}");
        }
        return Ok(());
    }

    App::fullscreen()
        .skin(skins::VAPOR95)
        .fps(30)
        .run((), update, view)?;
    Ok(())
}

/// Map the harness clock onto the (looping) edit clock and paint.
fn view(_m: &(), cx: &BuildCx) -> Element<Msg> {
    let (w, h) = (cx.environment.width, cx.environment.height);
    let edit = cx.time.as_secs_f32() % film().duration().max(0.001);
    screen::<Msg>().child(raster::<Msg>(frame(w, h, edit)).grow(1.0))
}

fn update(_m: &mut (), event: AppEvent<Msg>) -> Control {
    if let AppEvent::Input(Event::Key(key)) = event {
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            return Control::Quit;
        }
    }
    Control::Continue
}
