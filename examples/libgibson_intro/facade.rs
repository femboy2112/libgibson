//! Small ordinary UIs mounted on the city's information architecture. These
//! sealed receipts are projections of the finite harness, not new simulation.
use super::identity::IDENTITIES;
use super::model::{AGENTS, MESSAGES};
use gibson::canvas::BrailleCanvas;
use gibson::cell::{Color, Line, Span, Style};
use gibson::node::Node;
use gibson::surface::{BorderType, Surface};
use gibson::ColorDepth;
use std::f32::consts::PI;

/// One vector recipe serves the world-space facade and the miniature UI chart.
/// Coordinates are normalized; gain distinguishes principal and secondary paths.
pub(super) fn diagram_lines(index: usize, mut draw: impl FnMut((f32, f32), (f32, f32), f32)) {
    fn node(draw: &mut impl FnMut((f32, f32), (f32, f32), f32), x: f32, y: f32, r: f32) {
        let points = [(x - r, y), (x, y - r), (x + r, y), (x, y + r)];
        for i in 0..4 {
            draw(points[i], points[(i + 1) % 4], 1.);
        }
    }
    match index {
        0 => {
            for (a, b) in [
                ((0.12, 0.5), (0.43, 0.16)),
                ((0.12, 0.5), (0.43, 0.83)),
                ((0.43, 0.16), (0.83, 0.5)),
                ((0.43, 0.83), (0.83, 0.5)),
            ] {
                draw(a, b, 0.65);
            }
            for (x, y) in [(0.12, 0.5), (0.43, 0.16), (0.43, 0.83), (0.83, 0.5)] {
                node(&mut draw, x, y, 0.07);
            }
        }
        1 => {
            let nodes = [
                (0.08, 0.6),
                (0.3, 0.18),
                (0.37, 0.75),
                (0.57, 0.39),
                (0.76, 0.12),
                (0.93, 0.66),
            ];
            for (a, b) in [(0, 1), (0, 2), (1, 3), (2, 3), (3, 4), (3, 5), (4, 5)] {
                draw(nodes[a], nodes[b], 0.48);
            }
            for (x, y) in nodes {
                node(&mut draw, x, y, 0.04);
            }
        }
        2 => {
            for lane in 0..3 {
                let gain = if lane == 1 { 1.1 } else { 0.72 };
                for step in 0..24 {
                    let t = step as f32 / 24.;
                    let next = (step + 1) as f32 / 24.;
                    let x = 0.09 + t * 0.82;
                    let nx = 0.09 + next * 0.82;
                    let y = 0.5 + (lane as f32 - 1.) * 0.43 * (t * PI).sin();
                    let ny = 0.5 + (lane as f32 - 1.) * 0.43 * (next * PI).sin();
                    draw((x, y), (nx, ny), gain);
                }
            }
            node(&mut draw, 0.06, 0.5, 0.05);
            node(&mut draw, 0.94, 0.5, 0.05);
            draw((0.67, 0.40), (0.75, 0.50), 1.1);
            draw((0.75, 0.50), (0.67, 0.60), 1.1);
        }
        _ => {
            for row in 0..4 {
                for col in 0..6 {
                    let x = 0.04 + col as f32 * 0.16;
                    let y = 0.1 + row as f32 * 0.24;
                    draw((x, y + 0.03), (x + 0.035, y + 0.09), 0.85);
                    draw((x + 0.035, y + 0.09), (x + 0.10, y - 0.03), 0.85);
                }
            }
        }
    }
}

fn row(left: &str, right: &str, width: u16, style: Style, value: Style) -> Node {
    let left_width = left.chars().count();
    let right_width = right.chars().count();
    let gap = usize::from(width).saturating_sub(left_width + right_width);
    Node::line(Line::from_spans(vec![
        Span::styled(left, style),
        Span::styled(" ".repeat(gap), style),
        Span::styled(right, value),
    ]))
    .width(f32::from(width))
    .height(1.)
    .flex_shrink(0.)
}

fn chart(index: usize, width: u16, height: u16, style: Style) -> Surface {
    let mut secondary = BrailleCanvas::new(width, height);
    let mut primary = BrailleCanvas::new(width, height);
    let xmax = f32::from(width.saturating_mul(2).saturating_sub(1));
    let ymax = f32::from(height.saturating_mul(4).saturating_sub(1));
    diagram_lines(index, |a, b, gain| {
        let canvas = if gain >= 0.8 {
            &mut primary
        } else {
            &mut secondary
        };
        canvas.line(
            (a.0 * xmax).round() as i32,
            (a.1 * ymax).round() as i32,
            (b.0 * xmax).round() as i32,
            (b.1 * ymax).round() as i32,
        );
    });
    let mut surface = secondary.to_surface(style.dim());
    let foreground = primary.to_surface(style.bold());
    // Braille patterns share one cell. Union the bits rather than replacing a
    // path cell with its marker and losing the path underneath it.
    for (dst, src) in surface.cells.iter_mut().zip(&foreground.cells) {
        // Empty Braille cells also belong to the physical display plate.
        dst.style = style.dim();
        let a = dst.glyph.grapheme.chars().next().unwrap_or('⠀') as u32;
        let b = src.glyph.grapheme.chars().next().unwrap_or('⠀') as u32;
        let bits = a.saturating_sub(0x2800) | b.saturating_sub(0x2800);
        if b > 0x2800 {
            dst.style = src.style;
        }
        dst.glyph = gibson::Glyph::new(&char::from_u32(0x2800 + bits).unwrap_or('⠀').to_string());
    }
    surface
}

/// Render a compact, sealed agent workspace. At least 17×8 cells retains the
/// complete title, status, two factual rows, diagram and next-agent handoff.
/// The caller clips this ordinary realization to a projected physical display.
pub(super) fn render(index: usize, width: u16, height: u16, capability: ColorDepth) -> Surface {
    let index = index.min(AGENTS.len() - 1);
    let identity = IDENTITIES[index];
    let mono = capability == ColorDepth::Mono;
    let bg = if mono {
        Color::Reset
    } else {
        Color::Rgb(3, 9, 17)
    };
    let ink = if mono {
        Style::new().bg(bg)
    } else {
        Style::new().fg(Color::Rgb(191, 217, 230)).bg(bg)
    };
    let accent = if mono {
        ink.bold()
    } else {
        ink.fg(identity.color())
    };
    let status = if mono {
        ink.bold().reverse()
    } else {
        Style::new()
            .fg(Color::Rgb(3, 9, 17))
            .bg(identity.color())
            .bold()
    };
    let inner_width = width.saturating_sub(2);
    let inner_height = height.saturating_sub(2);
    let wide = inner_width >= 24;
    let tabs = ["PLAN", "MAP", "ROUTES", "PROOF"];
    let details = if wide {
        [
            [("4 contracts", "sealed"), ("offline transit", "access")],
            [("128 stops", "indexed"), ("384 links", "6 invariants")],
            [("3 candidates", "seeded"), ("tie-break", "deterministic")],
            [("24 fixtures", "24 pass"), ("replay == source", "exact")],
        ]
    } else {
        [
            [("4 contracts", "OK"), ("offline transit", "")],
            [("128 stops", "OK"), ("384 links", "OK")],
            [("3 routes / seed", ""), ("tie-break exact", "")],
            [("24/24 fixtures", ""), ("replay == exact", "")],
        ]
    };
    let extras: [&str; 2] = match index {
        0 => ["acceptance / accessible", "ordered dependency plan"],
        1 => ["disconnected paths checked", "transfer constraints sealed"],
        2 => ["accessible transfers", "candidate / replay witness"],
        _ => ["hostile inputs checked", "same events / same result"],
    };
    let extra_count = if wide {
        inner_height.saturating_sub(6).min(2)
    } else {
        0
    };
    let chart_height = inner_height.saturating_sub(4 + extra_count);
    let mut nodes = vec![
        row(
            &format!("[{}]", tabs[index]),
            " OK ",
            inner_width,
            accent,
            status,
        ),
        row(
            details[index][0].0,
            details[index][0].1,
            inner_width,
            ink,
            accent,
        ),
        row(
            details[index][1].0,
            details[index][1].1,
            inner_width,
            ink,
            accent,
        ),
    ];
    for detail in extras.iter().take(usize::from(extra_count)) {
        nodes.push(row(detail, "", inner_width, ink.dim(), ink));
    }
    if chart_height > 0 {
        nodes.push(Node::raster(chart(index, inner_width, chart_height, accent)).flex_shrink(0.));
    }
    let next = AGENTS[MESSAGES[index].to].name;
    let handoff = if wide {
        format!("handoff › {next}")
    } else {
        format!("› {next}")
    };
    nodes.push(row(&handoff, "", inner_width, accent.bold(), accent));
    let mut root = Node::panel(
        format!("{} {}", identity.signature, AGENTS[index].name),
        BorderType::Rounded,
        accent,
    )
    .background(bg)
    .width(f32::from(width))
    .height(f32::from(height))
    .children(nodes);
    let mut surface = Surface::new(width, height);
    if gibson::layout::compute_layout(&mut root, width, height).is_ok() {
        gibson::painter::paint(&root, &mut surface);
    }
    surface
}
