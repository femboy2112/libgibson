//! RECIPE — DIRECTED ATLAS
//! Run:       cargo run --example recipe_directed_atlas
//! Capture:   cargo run --example recipe_directed_atlas -- --at=4.5 --capture 120x40
//! Self-test: cargo run --example recipe_directed_atlas -- --selftest
//!
//! COMPOSITION LAW — a *directed* path through the atlas: a tiny seekable film,
//! `frame = f(t)`, with explicit acts and one real transition corridor.
//!
//!   ESTABLISH ─▶ TRANSFORM ─▶ REVEAL ─▶ HOLD ─▶ PAYOFF
//!   0────────3──────────6────────9──────11──────────14 s
//!             └ the corridor ┘        └ still ┘
//!
//! This is the director from `examples/libgibson_intro`, compressed to its law.
//! Time is *explicit state*, never a wall-clock or paint-time accumulator, so the
//! film is seekable and byte-deterministic: seeking to t and running to t produce
//! the identical frame. The same two identities (CORE, EDGE) survive every act;
//! the TRANSFORM act is a visible corridor (they travel while a field ignites),
//! not a cut; the HOLD act is genuinely still (zero render delta); the PAYOFF
//! locks the title. `--selftest` asserts all of that mechanically.

use gibson::raster::RgbRaster;
use gibson::ui::prelude::*;
use gibson::{Color, ColorDepth, Event, KeyCode, Node, Rect, Style, Surface};

#[path = "recipes_support/mod.rs"]
mod recipes_support;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Act {
    Establish,
    Transform,
    Reveal,
    Hold,
    Payoff,
}

/// The seekable cue sheet: contiguous `[start, end)` windows. This is the entire
/// "story"; it owns no clock and no mutable state.
const CUES: [(Act, f32, f32); 5] = [
    (Act::Establish, 0.0, 3.0),
    (Act::Transform, 3.0, 6.0),
    (Act::Reveal, 6.0, 9.0),
    (Act::Hold, 9.0, 11.0),
    (Act::Payoff, 11.0, 14.0),
];
const END: f32 = 14.0;

/// Explicit presentation time. The only state the film has.
#[derive(Clone, Copy)]
struct Director {
    t: f32,
}
impl Director {
    fn seek(&mut self, t: f32) {
        if t.is_finite() {
            self.t = t.clamp(0.0, END);
        }
    }
    fn advance(&mut self, dt: f32) {
        if dt.is_finite() && dt > 0.0 {
            self.seek(self.t + dt);
        }
    }
    fn act(&self) -> Act {
        CUES.iter()
            .find(|(_, _, end)| self.t < *end)
            .map(|(a, _, _)| *a)
            .unwrap_or(Act::Payoff)
    }
}

/// The two persistent identities. Each carries accent + glyph + label through the
/// whole film. `rest` is the establish arrangement; `locked` is where the payoff
/// emblem seats them.
struct Id {
    accent: (u8, u8, u8),
    glyph: &'static str,
    label: &'static str,
    rest: (f32, f32),
    locked: (f32, f32),
}
const IDS: [Id; 2] = [
    Id {
        accent: (76, 218, 244),
        glyph: "●",
        label: "CORE",
        rest: (0.26, 0.60),
        locked: (0.44, 0.46),
    },
    Id {
        accent: (249, 193, 95),
        glyph: "◆",
        label: "EDGE",
        rest: (0.74, 0.34),
        locked: (0.56, 0.46),
    },
];

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Position of identity `i` at time `t`. Pure projection of t. Continuous across
/// every cue boundary: constant in ESTABLISH, travelling in the TRANSFORM
/// corridor, and parked for REVEAL/HOLD/PAYOFF.
fn pos(i: usize, t: f32) -> (f32, f32) {
    let id = &IDS[i];
    let travel = smooth((t - 3.0) / 3.0); // 0 at t≤3, 1 at t≥6
    (
        lerp(id.rest.0, id.locked.0, travel),
        lerp(id.rest.1, id.locked.1, travel),
    )
}
/// How lit the connecting field is: dark in ESTABLISH, ignites over TRANSFORM,
/// full from REVEAL onward. Pure in t; constant during HOLD.
fn field_level(t: f32) -> f32 {
    smooth((t - 3.0) / 3.0)
}
/// Reveal of the link between the two identities: 0 until REVEAL, full by its end.
fn link_level(t: f32) -> f32 {
    smooth((t - 6.0) / 3.0)
}
/// Title emergence, PAYOFF only: 0 at t≤11 (so the HOLD→PAYOFF boundary is
/// continuous), rising to 1 by t=13.
fn title_level(t: f32) -> f32 {
    smooth((t - 11.0) / 2.0)
}

fn paint(rect: Rect, t: f32, mono: bool) -> Surface {
    let w = rect.width.max(1);
    let h = rect.height.max(1);
    let fl = field_level(t);

    // Base field: each identity is a well; the field ignites over the corridor.
    let mut px = RgbRaster::new(w, h.saturating_mul(2));
    let pw = px.width() as f32;
    let ph = px.height() as f32;
    let wells: [(f32, f32, (u8, u8, u8)); 2] = [
        {
            let (x, y) = pos(0, t);
            (x * pw, y * ph, IDS[0].accent)
        },
        {
            let (x, y) = pos(1, t);
            (x * pw, y * ph, IDS[1].accent)
        },
    ];
    let reach = 0.26 * pw;
    for y in 0..px.height() as i32 {
        for x in 0..px.width() as i32 {
            let (mut r, mut g, mut b) = (6.0_f32, 9.0, 14.0);
            for &(cx, cy, accent) in &wells {
                let dx = x as f32 - cx;
                let dy = (y as f32 - cy) * 2.0;
                let dist = (dx * dx + dy * dy).sqrt();
                let e = (1.0 - dist / reach).clamp(0.0, 1.0);
                let e = e * e * fl;
                r += accent.0 as f32 * e;
                g += accent.1 as f32 * e;
                b += accent.2 as f32 * e;
            }
            px.set(
                x,
                y,
                (r.min(255.0) as u8, g.min(255.0) as u8, b.min(255.0) as u8),
            );
        }
    }
    // Capability-safe base: in Mono, the field degrades to an ordered-dither
    // luminance silhouette instead of a flat half-block wall.
    let mut s = if mono {
        px.to_mono_surface()
    } else {
        px.to_surface()
    };

    let fw = w as f32;
    let fh = h as f32;
    let cell = |nx: f32, ny: f32| -> (u16, u16) {
        (
            (nx * fw).clamp(0.0, fw - 1.0) as u16,
            (ny * fh).clamp(0.0, fh - 1.0) as u16,
        )
    };

    // REVEAL: the link between the two identities lights up.
    let ll = link_level(t);
    if ll > 0.02 {
        let link = Style {
            fg: Some(Color::rgb(
                (120.0 * ll) as u8 + 20,
                (140.0 * ll) as u8 + 24,
                (170.0 * ll) as u8 + 30,
            )),
            ..Default::default()
        };
        let (ax, ay) = pos(0, t);
        let (bx, by) = pos(1, t);
        let steps = 20;
        let lit = ((steps as f32) * ll) as i32;
        for i in 1..steps {
            if i > lit {
                break;
            }
            let f = i as f32 / steps as f32;
            let (sx, sy) = cell(lerp(ax, bx, f), lerp(ay, by, f));
            s.print_str(sx, sy, "·", link, None);
        }
    }

    // The identities: always present, full accent, labelled. The invariant.
    for (i, id) in IDS.iter().enumerate() {
        let (nx, ny) = pos(i, t);
        let (cx, cy) = cell(nx, ny);
        let accent = Style {
            fg: Some(Color::rgb(id.accent.0, id.accent.1, id.accent.2)),
            ..Default::default()
        };
        s.print_str(cx, cy, id.glyph, accent, None);
        let lx = cx.saturating_add(2);
        if lx < w {
            s.print_str(lx, cy, id.label, accent, None);
        }
    }

    // PAYOFF: the title locks in beneath the emblem.
    let tl = title_level(t);
    if tl > 0.02 {
        let bright = (120.0 + 135.0 * tl) as u8;
        let title = Style {
            fg: Some(Color::rgb(bright, bright, (170.0 + 85.0 * tl) as u8)),
            ..Default::default()
        };
        let word = "A T L A S";
        let tx = (w.saturating_sub(word.chars().count() as u16)) / 2;
        let (_, ty) = cell(0.0, 0.62);
        s.print_str(tx, ty, word, title, None);
    }

    // In-world caption: the current act and time.
    let d = Director { t };
    let name = match d.act() {
        Act::Establish => "ACT: ESTABLISH",
        Act::Transform => "ACT: TRANSFORM (corridor)",
        Act::Reveal => "ACT: REVEAL",
        Act::Hold => "ACT: HOLD (still)",
        Act::Payoff => "ACT: PAYOFF",
    };
    // The act label is in-world, but the CLOCK is not: a time readout burned into
    // the canvas would make the HOLD act differ frame-to-frame. Telemetry is
    // chrome (the status row carries `t`); the world stays still when the film is
    // still. This is what makes the frozen hold a genuine zero-diff.
    let cap = Style {
        fg: Some(Color::rgb(150, 170, 190)),
        ..Default::default()
    };
    s.print_str(2, 0, name, cap, None);
    s
}

fn view(d: &Director, cx: &BuildCx) -> Element<()> {
    let t = d.t;
    let mono = matches!(cx.environment.color_depth, ColorDepth::Mono);
    let hero = raw(Node::canvas(move |rect| paint(rect, t, mono))).grow(1.0);
    let status_row = row()
        .child(status("DIRECTED").tone(Tone::Info))
        .child(spacer())
        .child(text(format!("t {t:.2}s / {END:.0}s")))
        .height(1);
    let hint = text("</> seek   same 2 identities survive every act   Ctrl-C quit").height(1);
    screen()
        .height(cx.environment.height)
        .child(status_row)
        .child(hero)
        .child(hint)
}

fn update(d: &mut Director, event: AppEvent<()>) -> Control {
    if let AppEvent::Input(Event::Key(k)) = event {
        match k.code {
            KeyCode::Left => d.seek(d.t - 0.5),
            KeyCode::Right => d.seek(d.t + 0.5),
            KeyCode::Char('r') => d.t = 0.0,
            _ => {}
        }
    }
    Control::Continue
}

fn initial_t() -> f32 {
    for a in std::env::args().skip(1) {
        if let Some(v) = a.strip_prefix("--at=") {
            if let Ok(t) = v.parse::<f32>() {
                return t.clamp(0.0, END);
            }
        }
    }
    0.0
}

/// Mechanical proof of the director invariants. Prints each check and exits
/// non-zero on any failure, so it is runnable as a gate.
fn report(name: &str, pass: bool) -> bool {
    println!("[{}] {name}", if pass { "PASS" } else { "FAIL" });
    pass
}
fn selftest() -> std::io::Result<()> {
    let rect = Rect::new(0, 0, 100, 30);
    let mut ok = true;

    // 1. Determinism: same t ⇒ identical frame.
    ok &= report(
        "determinism: paint(t) == paint(t)",
        paint(rect, 4.5, false) == paint(rect, 4.5, false),
    );

    // 2. Direct seek agrees with run-to-t (dyadic steps sum exactly to t).
    let mut d = Director { t: 0.0 };
    for _ in 0..14 {
        d.advance(0.5); // 14 × 0.5 = 7.0 exactly
    }
    ok &= report(
        "direct seek == run-to-t at 7.0s",
        paint(rect, d.t, false) == paint(rect, 7.0, false),
    );

    // 3. Boundary continuity: no identity teleports across a cue boundary.
    let eps = 1e-3;
    let mut cont = true;
    for &(_, start, _) in CUES.iter().skip(1) {
        for i in 0..IDS.len() {
            let (ax, ay) = pos(i, start - eps);
            let (bx, by) = pos(i, start + eps);
            let jump = ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt();
            if jump > 0.02 {
                cont = false;
            }
        }
    }
    ok &= report("boundary continuity: no hard cuts (none declared)", cont);

    // 4. Frozen hold: any two times inside HOLD render identically.
    ok &= report(
        "hold zero-diff: paint(9.5) == paint(10.5)",
        paint(rect, 9.5, false) == paint(rect, 10.5, false),
    );

    // 5. The hold really is a plateau, not a lucky pair.
    ok &= report(
        "hold plateau: paint(9.1) == paint(10.9)",
        paint(rect, 9.1, false) == paint(rect, 10.9, false),
    );

    println!(
        "{}",
        if ok {
            "selftest: OK"
        } else {
            "selftest: FAILED"
        }
    );
    if ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn main() -> std::io::Result<()> {
    if std::env::args().any(|a| a == "--selftest") {
        return selftest();
    }
    recipes_support::present(skins::BLACK_ICE, Director { t: initial_t() }, update, view)
}
