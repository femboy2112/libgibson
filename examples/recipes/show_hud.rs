//! Recipe: **crisp chrome over a rendered scene, with no opaque cell blocks.**
//!
//! The one law: text laid over a drawn raster must never punch an opaque terminal
//! cell. [`Surface::bake_text`] solves this by setting each glyph's *background* to
//! the pixel colour already beneath it — so a caption sits *on* the scene, only the
//! character showing, never a rectangle of dead background. A soft lower-third plate
//! (an alpha-composited [`RgbRaster::round_rect`]) gives the text something legible
//! to sit on without hiding the whole frame.
//!
//! Teaches: an anti-aliased rounded plate composited at `alpha`, author-side
//! word-wrapping, and baked text whose background is the raster — the whole
//! "lower-third / now-showing chip" chrome pattern, built only on public API.
//!
//!   cargo run --example show_hud            # live, loops
//!   cargo run --example show_hud -- at 4.0  # headless text snapshot of one instant

use std::io;

use gibson::raster::RgbRaster;
use gibson::timeline::Timeline;
use gibson::ui::{raster, screen, skins, App, AppEvent, BuildCx, Control, Element};
use gibson::Surface;

#[derive(Clone)]
enum Msg {}

/// Each shot carries its own chrome copy — a title and a longer caption that the
/// HUD will word-wrap. The timeline never inspects the payload; this is all yours.
#[derive(Clone, Copy)]
struct Card {
    title: &'static str,
    caption: &'static str,
    tint: (u8, u8, u8),
}

fn film() -> Timeline<Card> {
    Timeline::new()
        .cut(
            4.0,
            Card {
                title: "ESTABLISH",
                caption: "A HUD is just baked text on a soft plate — the plate is a rounded raster, the text reads the pixels beneath it.",
                tint: (40, 24, 60),
            },
        )
        .crossfade(
            4.0,
            1.0,
            Card {
                title: "DEVELOP",
                caption: "Word-wrap is the author's five lines; the library gives you the plate and the baked glyphs.",
                tint: (18, 44, 66),
            },
        )
        .crossfade(
            3.0,
            1.0,
            Card {
                title: "RESOLVE",
                caption: "Because bake_text sets each glyph's bg to the scene, nothing ever punches an opaque cell.",
                tint: (22, 54, 40),
            },
        )
}

/// Greedy word-wrap to `width` columns. Chrome copy is the author's; the library has
/// no opinion about line breaking, so this lives right here in the recipe.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if cur.is_empty() {
            cur = word.to_string();
        } else if cur.chars().count() + 1 + word.chars().count() <= width {
            cur.push(' ');
            cur.push_str(word);
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

fn frame(w: u16, h: u16, edit: f32) -> Surface {
    let f = film();
    let dur = f.duration();
    let ph = h.saturating_mul(2).max(2);

    // Clamp to the last resolvable instant (half-open window) so the final frame holds.
    let card = f
        .top(edit.min(dur - 1e-3).max(0.0))
        .map(|a| *a.payload)
        .unwrap_or(Card {
            title: "",
            caption: "",
            tint: (10, 10, 18),
        });

    // The scene: a drifting field in the card's tint. Any raster would do here.
    let mut r = RgbRaster::new(w.max(1), ph);
    for x in 0..w as i32 {
        let t = ((x as f32 / w as f32) * 5.0 + edit * 1.2).sin() * 0.5 + 0.5;
        let c = (
            card.tint.0.saturating_add((t * 70.0) as u8),
            card.tint.1.saturating_add((t * 60.0) as u8),
            card.tint.2.saturating_add((t * 90.0) as u8),
        );
        for y in 0..ph as i32 {
            r.set(x, y, c);
        }
    }

    if w >= 24 && h >= 8 {
        // --- The chrome, drawn in RASTER PIXELS (two pixel rows per cell) ---
        // A "now showing" pill, top-left. round_rect composites with alpha and
        // anti-aliased corners, so it reads as a soft chip, not a blocky box.
        r.round_rect(
            4,
            4,
            (w as i32 - 8).min(22 * 2),
            2 * 2,
            5.0,
            (12, 10, 22),
            0.72,
        );

        // The lower-third plate: a wide soft panel across the bottom few cell rows.
        let band_cells = 1 + wrap(card.caption, (w as usize).saturating_sub(6)).len() as i32 + 1;
        let band_cells = band_cells.clamp(3, (h as i32 - 2).max(3));
        let plate_top_px = (h as i32 - band_cells - 1).max(0) * 2;
        let plate_h_px = (band_cells + 1) * 2;
        r.round_rect(
            4,
            plate_top_px,
            w as i32 - 8,
            plate_h_px,
            7.0,
            (8, 7, 16),
            0.68,
        );
    }

    // --- Bake the TEXT into the cells, over the plate pixels ---
    let mut s = r.to_surface();
    if w >= 24 && h >= 8 {
        // Top-left chip label.
        s.bake_text(3, 2, "● NOW SHOWING", (235, 160, 180), true);

        let band_cells = 1 + wrap(card.caption, (w as usize).saturating_sub(6)).len() as u16 + 1;
        let band_cells = band_cells.clamp(3, h.saturating_sub(2).max(3));
        let top = h.saturating_sub(band_cells + 1);
        s.bake_text(4, top, card.title, (245, 245, 255), true);
        for (i, line) in wrap(card.caption, (w as usize).saturating_sub(6))
            .iter()
            .enumerate()
        {
            s.bake_text(4, top + 1 + i as u16, line, (210, 214, 235), false);
        }
    }
    s
}

fn main() -> io::Result<()> {
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
    let edit = cx.time.as_secs_f32() % film().duration().max(0.001);
    screen::<Msg>().child(raster::<Msg>(frame(w, h, edit)).grow(1.0))
}

fn update(_m: &mut (), event: AppEvent<Msg>) -> Control {
    use gibson::input::{Event, KeyCode, KeyModifiers};
    if let AppEvent::Input(Event::Key(key)) = event {
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            return Control::Quit;
        }
    }
    Control::Continue
}
