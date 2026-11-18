//! RECIPE — ATLAS SKELETON (compact)
//! Run:      cargo run --example recipe_atlas_skeleton
//! Capture:  cargo run --example recipe_atlas_skeleton -- --capture 120x40
//!           cargo run --example recipe_atlas_skeleton -- --at=1.0 --capture 80x24:mono
//!
//! The smallest honest *representation atlas* (see `docs/AI_VISUAL_AUTHORING.md`
//! §6): two anchors, two visual bases, one transport, an explicit cue sheet, pure
//! `frame = f(t)`. Copy it and grow the model/bases while keeping this shape. It
//! is deliberately NOT elaborate art — the full exemplars
//! `recipe_identity_transport.rs` and `recipe_directed_atlas.rs` show production
//! detail once you need it. Plan the fields first with `AI_VISUAL_ATLAS_PLAN.md`.

use gibson::raster::RgbRaster;
use gibson::ui::prelude::*;
use gibson::{Color, ColorDepth, Event, KeyCode, Node, Rect, Style, Surface};

#[path = "recipes_support/mod.rs"]
mod recipes_support;

// ── Layer 1 — the semantic model. Both bases read THIS; no model-per-view. ───
// An anchor = a semantic identity on ≥2 REDUNDANT channels (here: glyph + accent
// + position), so it survives a basis change AND Mono. Colour alone never does.
#[derive(Clone, Copy)]
struct Anchor {
    label: &'static str,
    glyph: &'static str,
    accent: (u8, u8, u8),
    list_site: (f32, f32),  // normalized position in the LIST basis
    field_site: (f32, f32), // normalized position in the FIELD basis
}
const ANCHORS: [Anchor; 2] = [
    Anchor {
        label: "SOURCE",
        glyph: "●",
        accent: (76, 218, 244),
        list_site: (0.10, 0.34),
        field_site: (0.30, 0.42),
    },
    Anchor {
        label: "SINK",
        glyph: "◆",
        accent: (249, 193, 95),
        list_site: (0.10, 0.66),
        field_site: (0.72, 0.60),
    },
];

// ── Director — an explicit cue sheet over frame = f(t), t ∈ [0,1]. Time is
// EXPLICIT STATE (never a paint-time clock), so the piece is seekable. ────────
#[derive(Clone, Copy)]
struct Film {
    t: f32,
}
fn act(t: f32) -> &'static str {
    match t {
        _ if t < 0.30 => "ESTABLISH \u{00b7} LIST",
        _ if t < 0.70 => "TRANSPORT \u{00b7} LIST \u{2192} FIELD",
        _ => "REVEAL \u{00b7} FIELD",
    }
}

// ── The one transport — each anchor travels from its LIST site to its FIELD
// site as t sweeps 0 → 1. A move, not a cut. ─────────────────────────────────
fn anchor_pos(a: &Anchor, t: f32) -> (f32, f32) {
    let k = t.clamp(0.0, 1.0);
    let lerp = |p: f32, q: f32| p + (q - p) * k;
    (
        lerp(a.list_site.0, a.field_site.0),
        lerp(a.list_site.1, a.field_site.1),
    )
}

// ── Layers 2+3 — paint both bases as ONE continuous frame. The LIST basis is
// the left-stacked marks at t≈0; the FIELD basis is the spread glowing wells at
// t≈1; the transport is the continuous slide between. ────────────────────────
fn paint(rect: Rect, t: f32, mono: bool) -> Surface {
    let w = rect.width.max(1);
    let h = rect.height.max(1);
    let field_w = t.clamp(0.0, 1.0); // the FIELD glow grows with t

    // Half-block packs 2 px per cell row; each anchor is a glowing well.
    let mut px = RgbRaster::new(w, h.saturating_mul(2));
    let (pw, ph) = (px.width() as f32, px.height() as f32);
    let reach = 0.26 * pw;
    for y in 0..px.height() as i32 {
        for x in 0..px.width() as i32 {
            let (mut r, mut g, mut b) = (6.0_f32, 9.0, 14.0);
            for a in &ANCHORS {
                let (nx, ny) = anchor_pos(a, t);
                let dx = x as f32 - nx * pw;
                let dy = (y as f32 - ny * ph) * 2.0; // cells are ~2:1
                let e = (1.0 - (dx * dx + dy * dy).sqrt() / reach).clamp(0.0, 1.0);
                r += a.accent.0 as f32 * e * e * field_w;
                g += a.accent.1 as f32 * e * e * field_w;
                b += a.accent.2 as f32 * e * e * field_w;
            }
            px.set(
                x,
                y,
                (r.min(255.0) as u8, g.min(255.0) as u8, b.min(255.0) as u8),
            );
        }
    }
    // Capability-safe: Mono routes through luminance dither, so the wells read as
    // DENSITY (shape) rather than an undifferentiated half-block wall.
    let mut s = if mono {
        px.to_mono_surface()
    } else {
        px.to_surface()
    };

    // The identities: ALWAYS present and labelled, riding both bases. This is the
    // invariant a viewer tracks across the transport.
    let (fw, fh) = (w as f32, h as f32);
    for a in &ANCHORS {
        let (nx, ny) = anchor_pos(a, t);
        let cx = (nx * fw).clamp(0.0, fw - 1.0) as u16;
        let cy = (ny * fh).clamp(0.0, fh - 1.0) as u16;
        let st = Style {
            fg: Some(Color::rgb(a.accent.0, a.accent.1, a.accent.2)),
            ..Default::default()
        };
        s.print_str(cx, cy, a.glyph, st, None);
        if cx + 2 < w {
            s.print_str(cx + 2, cy, a.label, st, None);
        }
    }
    s
}

fn view(m: &Film, cx: &BuildCx) -> Element<()> {
    let t = m.t;
    let mono = matches!(cx.environment.color_depth, ColorDepth::Mono);
    let hero = raw(Node::canvas(move |rect| paint(rect, t, mono))).grow(1.0);
    let status_row = row()
        .child(status("ATLAS").tone(Tone::Info))
        .child(spacer())
        .child(text(format!("t {t:.2}  \u{00b7}  {}", act(t))))
        .height(1);
    let hint = text("</> seek   same 2 marks persist across both bases   Ctrl-C quit").height(1);
    // screen() is natural-height: pin it or .grow() collapses the hero to 1 row.
    screen()
        .height(cx.environment.height)
        .child(status_row)
        .child(hero)
        .child(hint)
}

fn update(m: &mut Film, event: AppEvent<()>) -> Control {
    if let AppEvent::Input(Event::Key(k)) = event {
        match k.code {
            KeyCode::Left => m.t = (m.t - 0.05).max(0.0),
            KeyCode::Right => m.t = (m.t + 0.05).min(1.0),
            _ => {}
        }
    }
    Control::Continue
}

// `--at=T` seeds explicit film-time so a capture is a pure projection of t.
fn initial_t() -> f32 {
    std::env::args()
        .skip(1)
        .find_map(|a| a.strip_prefix("--at=").and_then(|v| v.parse::<f32>().ok()))
        .map(|t| t.clamp(0.0, 1.0))
        .unwrap_or(0.0)
}

fn main() -> std::io::Result<()> {
    recipes_support::present(skins::BLACK_ICE, Film { t: initial_t() }, update, view)
}
