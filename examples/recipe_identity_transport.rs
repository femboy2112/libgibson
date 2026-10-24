//! RECIPE — IDENTITY TRANSPORT
//! Run:      cargo run --example recipe_identity_transport
//! Capture:  cargo run --example recipe_identity_transport -- --capture 120x40
//!           cargo run --example recipe_identity_transport -- --at=1.0 --capture 120x40
//!           cargo run --example recipe_identity_transport -- --at=1.5 --capture 80x24:mono
//!
//! COMPOSITION LAW — one semantic system, THREE visual bases, the same identities
//! transported between them. Changing representation is a *move*, not a page-swap.
//!
//!   LEDGER basis   ──▶   GRAPH basis   ──▶   FIELD basis
//!   rows of marks        node-link web       glowing wells
//!        └──────────── the SAME three marks ───────────┘
//!
//! The world-first guide (§1–8) gets you ONE coherent world. This recipe is the
//! next idea: a *representation atlas*. Three genuinely different visual bases
//! express the same tiny system (three signal nodes). The three identities —
//! each an accent colour + a one-cell signature glyph + a shared site — persist
//! through all three bases and visibly TRAVEL between them as `phase` sweeps
//! 0 → 1 → 2. Nothing here is a different screen; it is one system changing how
//! it is drawn. Identity is carried by shape AND colour AND position, so it
//! survives both the basis change and a drop to Mono (try `:mono`).

use gibson::raster::RgbRaster;
use gibson::ui::prelude::*;
use gibson::{Color, ColorDepth, Event, KeyCode, Node, Rect, Style, Surface};

#[path = "recipes_support/mod.rs"]
mod recipes_support;

/// One persistent identity anchor. The three channels that carry it —
/// `accent`, `glyph`, and a per-basis `site` — are exactly what gets transported
/// from one representation to the next.
#[derive(Clone, Copy)]
struct Anchor {
    accent: (u8, u8, u8),
    glyph: &'static str,
    label: &'static str,
    /// Normalized [0,1]² position in each basis: LEDGER, GRAPH, FIELD.
    ledger: (f32, f32),
    graph: (f32, f32),
    field: (f32, f32),
}

/// The whole semantic system: three signal nodes. Every basis reads THIS — there
/// is no second model per view.
const ANCHORS: [Anchor; 3] = [
    Anchor {
        accent: (76, 218, 244), // cyan
        glyph: "●",
        label: "PULSE",
        ledger: (0.16, 0.30),
        graph: (0.30, 0.34),
        field: (0.26, 0.40),
    },
    Anchor {
        accent: (249, 193, 95), // amber
        glyph: "◆",
        label: "RELAY",
        ledger: (0.16, 0.52),
        graph: (0.66, 0.26),
        field: (0.70, 0.32),
    },
    Anchor {
        accent: (174, 143, 251), // violet
        glyph: "▣",
        label: "SINK",
        ledger: (0.16, 0.74),
        graph: (0.54, 0.74),
        field: (0.50, 0.70),
    },
];
/// The one relationship in the system: PULSE → RELAY → SINK.
const LINKS: [(usize, usize); 2] = [(0, 1), (1, 2)];

#[derive(Clone, Copy)]
struct Atlas {
    /// 0.0 = LEDGER, 1.0 = GRAPH, 2.0 = FIELD; fractional values are mid-transit.
    phase: f32,
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Where an anchor sits *right now*, interpolated across whichever two bases the
/// current phase lies between. This is the transport made literal: the mark does
/// not teleport, it travels.
fn anchor_pos(a: &Anchor, phase: f32) -> (f32, f32) {
    if phase <= 1.0 {
        (
            lerp(a.ledger.0, a.graph.0, phase),
            lerp(a.ledger.1, a.graph.1, phase),
        )
    } else {
        let t = phase - 1.0;
        (lerp(a.graph.0, a.field.0, t), lerp(a.graph.1, a.field.1, t))
    }
}

fn paint(rect: Rect, phase: f32, mono: bool) -> Surface {
    let w = rect.width.max(1);
    let h = rect.height.max(1);

    // How much each basis "owns" the frame at this phase. The three cross-fade,
    // so no transition is an instant cut.
    let ledger_w = (1.0 - phase).clamp(0.0, 1.0);
    let field_w = (phase - 1.0).clamp(0.0, 1.0);
    let graph_w = (1.0 - (phase - 1.0).abs()).clamp(0.0, 1.0);

    // --- Base texture, painted as a continuous RGB field -------------------
    // The FIELD basis emerges here: each anchor becomes a glowing well whose
    // reach follows `field_w`, so at phase→2 the frame is a lit field and at
    // phase 0 it is near-black. Half-block packs two pixels per cell row.
    let mut px = RgbRaster::new(w, h.saturating_mul(2));
    let pw = px.width() as f32;
    let ph = px.height() as f32;
    let wells: Vec<(f32, f32, (u8, u8, u8))> = ANCHORS
        .iter()
        .map(|a| {
            let (nx, ny) = anchor_pos(a, phase);
            (nx * pw, ny * ph, a.accent)
        })
        .collect();
    let reach = 0.22 * pw;
    for y in 0..px.height() as i32 {
        for x in 0..px.width() as i32 {
            let (mut r, mut g, mut b) = (6.0_f32, 9.0, 14.0); // deep base
            for &(cx, cy, accent) in &wells {
                let dx = x as f32 - cx;
                let dy = (y as f32 - cy) * 2.0; // cells are ~2:1
                let dist = (dx * dx + dy * dy).sqrt();
                let fall = (1.0 - dist / reach).clamp(0.0, 1.0);
                let e = fall * fall * field_w;
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
    // Capability-safe base: in Mono, route the field through the library's
    // ordered-dither luminance path so the wells read as DENSITY (silhouette),
    // not an undifferentiated half-block wall. Colour is enhancement, not meaning.
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

    // --- LEDGER basis: a faint rule + value bar behind each mark's row -----
    if ledger_w > 0.02 {
        let rule = Style {
            fg: Some(Color::rgb(
                (34.0 * ledger_w) as u8 + 20,
                (44.0 * ledger_w) as u8 + 26,
                (58.0 * ledger_w) as u8 + 32,
            )),
            ..Default::default()
        };
        for a in &ANCHORS {
            let (_, ny) = anchor_pos(a, phase);
            let (_, cy) = cell(0.0, ny);
            let bar_cells = ((0.30 + 0.0) * fw * ledger_w) as u16;
            let rule_str: String = "─".repeat((fw as u16).min(bar_cells.max(1)) as usize);
            s.print_str(2, cy, &rule_str, rule, None);
        }
    }

    // --- GRAPH basis: dotted links between the marks' live positions -------
    if graph_w > 0.02 {
        let link = Style {
            fg: Some(Color::rgb(
                (70.0 * graph_w) as u8 + 20,
                (86.0 * graph_w) as u8 + 24,
                (104.0 * graph_w) as u8 + 30,
            )),
            ..Default::default()
        };
        for &(from, to) in &LINKS {
            let (ax, ay) = anchor_pos(&ANCHORS[from], phase);
            let (bx, by) = anchor_pos(&ANCHORS[to], phase);
            let steps = 24;
            for i in 1..steps {
                let t = i as f32 / steps as f32;
                let (sx, sy) = cell(lerp(ax, bx, t), lerp(ay, by, t));
                s.print_str(sx, sy, "·", link, None);
            }
        }
    }

    // --- The identities themselves: ALWAYS present, full accent, labelled --
    // These are the invariant. They ride every basis and travel between them;
    // a reader tracks the same three marks the whole way across.
    for a in &ANCHORS {
        let (nx, ny) = anchor_pos(a, phase);
        let (cx, cy) = cell(nx, ny);
        let accent = Style {
            fg: Some(Color::rgb(a.accent.0, a.accent.1, a.accent.2)),
            ..Default::default()
        };
        s.print_str(cx, cy, a.glyph, accent, None);
        let lx = cx.saturating_add(2);
        if lx < w {
            s.print_str(lx, cy, a.label, accent, None);
        }
    }

    // --- One in-world caption: which basis are we in? ----------------------
    let name = if phase < 0.5 {
        "BASIS: LEDGER"
    } else if phase < 1.0 {
        "BASIS: LEDGER → GRAPH"
    } else if phase < 1.5 {
        "BASIS: GRAPH"
    } else if phase < 2.0 {
        "BASIS: GRAPH → FIELD"
    } else {
        "BASIS: FIELD"
    };
    let cap = Style {
        fg: Some(Color::rgb(150, 170, 190)),
        ..Default::default()
    };
    s.print_str(2, 0, name, cap, None);
    s
}

fn view(m: &Atlas, cx: &BuildCx) -> Element<()> {
    let phase = m.phase;
    let mono = matches!(cx.environment.color_depth, ColorDepth::Mono);
    let hero = raw(Node::canvas(move |rect| paint(rect, phase, mono))).grow(1.0);

    let status_row = row()
        .child(status("ATLAS").tone(Tone::Info))
        .child(spacer())
        .child(text(format!("phase {phase:.2} / 2.00")))
        .height(1);

    let hint = text("</> move through bases   same 3 marks persist   Ctrl-C quit").height(1);

    screen()
        .height(cx.environment.height)
        .child(status_row)
        .child(hero)
        .child(hint)
}

fn update(m: &mut Atlas, event: AppEvent<()>) -> Control {
    if let AppEvent::Input(Event::Key(k)) = event {
        match k.code {
            KeyCode::Left => m.phase = (m.phase - 0.05).max(0.0),
            KeyCode::Right => m.phase = (m.phase + 0.05).min(2.0),
            _ => {}
        }
    }
    Control::Continue
}

/// Pull `--at=PHASE` out of argv so a capture is a pure projection of explicit
/// phase (the shared harness pins presentation time to zero). This lives here,
/// not in the harness, so the recipe stays a self-contained lesson.
fn initial_phase() -> f32 {
    for a in std::env::args().skip(1) {
        if let Some(v) = a.strip_prefix("--at=") {
            if let Ok(p) = v.parse::<f32>() {
                return p.clamp(0.0, 2.0);
            }
        }
    }
    0.0
}

fn main() -> std::io::Result<()> {
    recipes_support::present(
        skins::BLACK_ICE,
        Atlas {
            phase: initial_phase(),
        },
        update,
        view,
    )
}
