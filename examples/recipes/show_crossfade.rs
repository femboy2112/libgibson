//! Recipe: **two surfaces, one dissolve.**
//!
//! The one law: [`Timeline::crossfade`] overlaps a new cue with the previous one,
//! and during the overlap [`Timeline::resolve`] reports *both* cues live, each with
//! a `weight` in `0..1` that sums to 1. The timeline decides the weights; *you*
//! decide what compositing means. Here two independent `RgbRaster` producers are
//! blended per-pixel by weight — the timeline owns the *when*, never the *how*.
//!
//! Nothing here is scene-specific: swap in any two `Surface`/`RgbRaster` producers
//! and the dissolve still works, because the timeline carries only opaque ids.
//!
//! Teaches: `Timeline::crossfade`, `resolve()` returning overlapping weighted cues,
//! and weight-driven compositing.
//!
//!   cargo run --example show_crossfade            # live, loops
//!   cargo run --example show_crossfade -- at 2.4  # the dissolve midpoint (both cues ~0.5)

use gibson::input::{Event, KeyCode, KeyModifiers};
use gibson::raster::RgbRaster;
use gibson::timeline::Timeline;
use gibson::ui::{raster, screen, skins, App, AppEvent, BuildCx, Control, Element};
use gibson::Surface;

#[derive(Clone)]
enum Msg {}

/// Producer A: warm horizontal bands that drift with local time.
fn producer_a(local: f32, w: u16, ph: u16) -> RgbRaster {
    let mut r = RgbRaster::new(w.max(1), ph.max(1));
    for y in 0..ph as i32 {
        let t = (y as f32 * 0.25 + local * 6.0).sin() * 0.5 + 0.5;
        let c = (40 + (t * 200.0) as u8, 20 + (t * 90.0) as u8, 30);
        for x in 0..w as i32 {
            r.set(x, y, c);
        }
    }
    r
}

/// Producer B: cool vertical bands drifting the other way.
fn producer_b(local: f32, w: u16, ph: u16) -> RgbRaster {
    let mut r = RgbRaster::new(w.max(1), ph.max(1));
    for x in 0..w as i32 {
        let t = (x as f32 * 0.18 - local * 5.0).sin() * 0.5 + 0.5;
        let c = (20, 30 + (t * 120.0) as u8, 60 + (t * 180.0) as u8);
        for y in 0..ph as i32 {
            r.set(x, y, c);
        }
    }
    r
}

/// Add `layer * weight` into `acc`. Outside the overlap one cue has weight 1, so
/// `acc` is exactly that producer; inside the overlap the two weighted producers
/// sum to a true cross-dissolve (weights sum to ~1).
fn add_weighted(acc: &mut RgbRaster, layer: &RgbRaster, weight: f32) {
    let w = weight.clamp(0.0, 1.0);
    let dst = acc.pixels_mut();
    for (d, s) in dst.iter_mut().zip(layer.pixels()) {
        d.0 = (d.0 as f32 + s.0 as f32 * w).min(255.0) as u8;
        d.1 = (d.1 as f32 + s.1 as f32 * w).min(255.0) as u8;
        d.2 = (d.2 as f32 + s.2 as f32 * w).min(255.0) as u8;
    }
}

fn frame(w: u16, h: u16, edit: f32) -> Surface {
    let ph = h.saturating_mul(2).max(2);
    // A for 3 s, then B crossfading in over a 1.2 s overlap.
    let tl = Timeline::new().cut(3.0, 0u8).crossfade(3.0, 1.2, 1u8);

    let mut acc = RgbRaster::new(w.max(1), ph);
    acc.clear((0, 0, 0));
    for a in tl.resolve(edit) {
        let layer = match *a.payload {
            0 => producer_a(a.local, w, ph),
            _ => producer_b(a.local, w, ph),
        };
        add_weighted(&mut acc, &layer, a.weight);
    }

    let mut surface = acc.to_surface();
    if w >= 10 && h >= 3 {
        // Name whichever cue is dominant, so the dissolve is legible.
        let label = match tl.top(edit).map(|a| *a.payload) {
            Some(0) => "A",
            Some(_) => "B",
            None => "-",
        };
        let live = tl.resolve(edit).len();
        let caption = format!("dominant {label} · {live} cue(s) live");
        surface.bake_text(2, 1, &caption, (245, 245, 255), true);
    }
    surface
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
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

fn view(_m: &(), cx: &BuildCx) -> Element<Msg> {
    let (w, h) = (cx.environment.width, cx.environment.height);
    let edit = cx.time.as_secs_f32()
        % Timeline::new()
            .cut(3.0, 0u8)
            .crossfade(3.0, 1.2, 1u8)
            .duration();
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
