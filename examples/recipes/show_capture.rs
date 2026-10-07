//! Recipe: **capture a show headlessly, in color, as a frame sequence.**
//!
//! The one law: a show is `f(edit)`, so you can render *any* instant without a
//! terminal and write it out. This recipe lowers each frame to truecolor ANSI bytes
//! via `UiRuntime::frame` → `Context::headless` → `rendered_bytes`, and loops an
//! edit clock to produce a numbered sequence — the raw material for an MP4
//! (terminal → PNG → `ffmpeg` downstream). Deterministic: the same `edit` always
//! produces the same bytes, so captures are reproducible.
//!
//! Teaches: headless color capture of a `Surface`, and the fixed-step loop that
//! turns `f(edit)` into a sequence.
//!
//!   cargo run --example show_capture                 # live preview, loops
//!   cargo run --example show_capture -- seq ./frames # write frame_NNNN.ans at 20 fps

use std::io;
use std::time::Duration;

use gibson::capability::ColorDepth;
use gibson::context::{Context, RenderMode};
use gibson::input::{Event, KeyCode, KeyModifiers};
use gibson::raster::RgbRaster;
use gibson::timeline::Timeline;
use gibson::ui::skin::UiEnvironment;
use gibson::ui::{raster, screen, skins, App, AppEvent, BuildCx, Control, Element, UiRuntime};
use gibson::Surface;

#[derive(Clone)]
enum Msg {}

/// A tiny two-cue show so the captured sequence has a visible cut: a warm wash,
/// then a cool one, each with drifting vertical bands so motion is legible frame to
/// frame.
fn film() -> Timeline<(u8, u8, u8)> {
    Timeline::new()
        .cut(3.0, (46, 20, 60))
        .cut(3.0, (18, 40, 72))
}

fn frame(w: u16, h: u16, edit: f32) -> Surface {
    let ph = h.saturating_mul(2).max(2);
    let (base, local) = film()
        .top(edit)
        .map(|a| (*a.payload, a.local))
        .unwrap_or(((10, 10, 20), 0.0));
    let mut r = RgbRaster::new(w.max(1), ph);
    for x in 0..w as i32 {
        let t = ((x as f32 / w as f32) * 6.0 + edit * 1.5).sin() * 0.5 + 0.5;
        let c = (
            base.0.saturating_add((t * 90.0) as u8),
            base.1.saturating_add((t * 70.0) as u8),
            base.2.saturating_add((t * 110.0) as u8),
        );
        for y in 0..ph as i32 {
            r.set(x, y, c);
        }
    }
    let mut surface = r.to_surface();
    if w >= 10 && h >= 2 {
        surface.bake_text(2, 1, "CAPTURE", (245, 245, 255), true);
        let clock = format!("edit {edit:4.1}s · local {local:4.1}s");
        surface.bake_text(2, h.saturating_sub(2), &clock, (210, 210, 235), false);
    }
    surface
}

/// Lower one frame to truecolor ANSI bytes, headlessly. No terminal involved.
fn frame_bytes(w: u16, h: u16, edit: f32) -> Vec<u8> {
    let env = UiEnvironment {
        width: w,
        height: h,
        color_depth: ColorDepth::TrueColor,
        ..UiEnvironment::default()
    };
    let element = screen::<Msg>()
        .child(raster::<Msg>(frame(w, h, edit)).grow(1.0))
        .height(h);
    let mut rt = UiRuntime::new(skins::VAPOR95);
    let compiled = rt
        .frame(&element, env, Duration::from_secs_f32(edit.max(0.0)))
        .expect("frame lowers");
    let mut ctx = Context::headless(RenderMode::Fullscreen, w, h);
    ctx.set_color_depth(ColorDepth::TrueColor);
    ctx.set_root(compiled.node);
    ctx.render_now().expect("headless render");
    ctx.rendered_bytes().to_vec()
}

/// Write the whole show as a numbered 20-fps ANSI sequence into `dir`.
fn capture_sequence(dir: &str, w: u16, h: u16) -> io::Result<u32> {
    std::fs::create_dir_all(dir)?;
    let fps = 20.0_f32;
    let dt = 1.0 / fps;
    let total = film().duration();
    let mut edit = 0.0_f32;
    let mut n = 0u32;
    while edit < total {
        std::fs::write(format!("{dir}/frame_{n:04}.ans"), frame_bytes(w, h, edit))?;
        n += 1;
        edit += dt;
    }
    Ok(n)
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("seq") {
        let dir = args.get(2).cloned().unwrap_or_else(|| "frames".to_string());
        let w: u16 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(120);
        let h: u16 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(36);
        let n = capture_sequence(&dir, w, h)?;
        let total = film().duration();
        println!("show_capture: {n} frames -> {dir} ({w}x{h}, {total:.1}s show)");
        return Ok(());
    }
    App::fullscreen()
        .skin(skins::VAPOR95)
        .fps(30)
        .run((), update, view)?;
    Ok(())
}

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
