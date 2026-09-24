//! An ordinary declarative UI. The membrane receives its realized Surface;
//! none of these widgets knows that it will become a city.
use super::identity::{GRAPH_POINTS, IDENTITIES};
use super::model::{JobStatus, AGENTS};
use gibson::canvas::BrailleCanvas;
use gibson::cell::{Color, Line, Span, Style};
use gibson::node::Node;
use gibson::surface::Surface;

const INK: Color = Color::Rgb(207, 220, 237);
const DIM: Color = Color::Rgb(101, 124, 150);
const CYAN: Color = Color::Rgb(76, 218, 244);
const BG: Color = Color::Rgb(7, 12, 23);
const LIFT: Color = Color::Rgb(15, 26, 40);
const RULE: Color = Color::Rgb(32, 50, 69);
fn text(s: impl Into<String>, color: Color) -> Node {
    Node::text(s, Style::new().fg(color).bg(BG)).height(1.0)
}
fn line(parts: &[(&str, Color)]) -> Node {
    line_bg(parts, BG)
}
fn line_bg(parts: &[(&str, Color)], bg: Color) -> Node {
    Node::line(Line::from_spans(
        parts
            .iter()
            .map(|(s, c)| Span::styled(s, Style::new().fg(*c).bg(bg)))
            .collect(),
    ))
    .background(bg)
    .height(1.0)
}
fn graph(width: u16, height: u16, time: f32) -> Surface {
    let mut b = BrailleCanvas::new(width, height);
    let w = width as f32 * 2.0;
    let h = height as f32 * 4.0;
    let p = GRAPH_POINTS.map(|(x, y)| (x * w, y * h));
    for i in 0..3 {
        let (a, c) = (p[i], p[i + 1]);
        b.line(a.0 as i32, a.1 as i32, c.0 as i32, c.1 as i32);
    }
    let mut surface = b.to_surface(Style::new().fg(RULE).bg(BG));
    for (i, (x, y)) in p.into_iter().enumerate() {
        let agent = AGENTS[i];
        let id = IDENTITIES[i];
        let status = agent.status(time);
        let mut layer = BrailleCanvas::new(width, height);
        let running = status == JobStatus::Running;
        let r: i32 = if running { 3 } else { 2 };
        // Different footprints preserve node identity when color is unavailable.
        for dy in -r..=r {
            for dx in -r..=r {
                let on = match i {
                    0 => dx == -r || dx == r || dy == -r || dy == r,
                    1 => dx.abs() + dy.abs() == r,
                    2 => dx == -r || dx == r || dy == 0,
                    _ => dx == 0 || dy == 0,
                };
                if on {
                    layer.set(x as i32 + dx, y as i32 + dy);
                }
            }
        }
        if running {
            let phase = (agent.progress(time) * std::f32::consts::TAU).sin();
            let halo = 4 + (phase > 0.0) as i32;
            for dx in [-halo, halo] {
                layer.set(x as i32 + dx, y as i32);
                layer.set(x as i32, y as i32 + dx);
            }
        }
        if i > 0 && status != JobStatus::Queued {
            let (a, c) = (p[i - 1], p[i]);
            let u = (agent.progress(time) * 3.0).fract();
            // Only the currently executing dependency carries moving light.
            if running {
                for j in 0..5 {
                    let at = (u - j as f32 * 0.035).max(0.0);
                    layer.set(
                        (a.0 + (c.0 - a.0) * at) as i32,
                        (a.1 + (c.1 - a.1) * at) as i32,
                    );
                }
            }
        }
        let color = if status == JobStatus::Queued {
            DIM
        } else {
            id.color()
        };
        let pixels = layer.to_surface(Style::new().fg(color).bg(BG));
        for (dst, src) in surface.cells.iter_mut().zip(pixels.cells) {
            if src.glyph.grapheme.as_str() != " " && src.glyph.grapheme.as_str() != "⠀" {
                *dst = src;
            }
        }
    }
    for c in &mut surface.cells {
        c.style.bg = Some(BG);
    }
    surface
}
fn agents(width: u16, time: f32) -> Node {
    let mut nodes = vec![
        line(&[("01  ", CYAN), ("ORCHESTRATION", INK)]),
        text("PLAN → MAP → BUILD → PROVE", DIM),
        text("", DIM),
    ];
    for (i, a) in AGENTS.into_iter().enumerate() {
        let id = IDENTITIES[i];
        let status = a.status(time);
        let (state, col, bg) = match status {
            JobStatus::Queued => ("queued", DIM, BG),
            JobStatus::Running => ("active", id.color(), LIFT),
            JobStatus::Complete => ("sealed", id.color(), BG),
        };
        let rail = if status == JobStatus::Running {
            "┃"
        } else {
            "│"
        };
        nodes.push(
            line_bg(
                &[
                    (rail, col),
                    (&format!(" {} {:<10}", id.signature, a.name), col),
                    (&format!(" {state}"), DIM),
                ],
                bg,
            )
            .width(width as f32),
        );
        nodes.push(
            line_bg(&[(rail, col), (&format!("   {}", a.role), DIM)], bg).width(width as f32),
        );
        let n = usize::from(width.saturating_sub(6)).min(25);
        let fill = a.progress(time) * n as f32;
        let filled = fill as usize;
        let head = if filled < n && status == JobStatus::Running {
            "╸"
        } else {
            ""
        };
        nodes.push(
            line_bg(
                &[
                    (rail, col),
                    ("   ", DIM),
                    (&"━".repeat(filled), col),
                    (head, INK),
                    (
                        &"─".repeat(n.saturating_sub(filled + usize::from(!head.is_empty()))),
                        RULE,
                    ),
                ],
                bg,
            )
            .width(width as f32),
        );
        nodes.push(text("", DIM));
    }
    Node::col().children(nodes)
}
const STEPS: [[&str; 4]; 4] = [
    [
        "contract / accessibility is invariant",
        "decompose / map before changing policy",
        "reject / disconnected edge proposal",
        "accept / four ordered job contracts",
    ],
    [
        "read / fixtures/transit.graph",
        "index / 128 stops, 384 directed links",
        "resolve / transfer and access constraints",
        "emit / normalized topology witness",
    ],
    [
        "build / stable shortest viable route",
        "refine / three candidate paths",
        "reject / closed transfer in candidate 02",
        "emit / candidate 03 + replay seed",
    ],
    [
        "probe / disconnected destination",
        "probe / ties and zero transfer window",
        "replay / same events, same result",
        "seal / 24 of 24 fixtures converge",
    ],
];
fn workspace(width: u16, height: u16, time: f32) -> Node {
    let current = AGENTS
        .iter()
        .position(|a| a.status(time) == JobStatus::Running)
        .unwrap_or(3);
    let a = AGENTS[current];
    let id = IDENTITIES[current];
    let p = a.progress(time);
    let stage = ((p * 4.0) as usize).min(3);
    let mut nodes = vec![
        line(&[("02  ", CYAN), ("LIVE WORKSPACE", INK)]),
        text("OFFLINE TRANSIT / DISRUPTION PLANNER", DIM),
        text("", DIM),
        line(&[
            (id.signature, id.color()),
            (&format!("  {}", a.name), id.color()),
            ("  /  ", DIM),
            (a.role, DIM),
        ]),
        text(a.task, INK),
        text("", DIM),
    ];
    for (i, s) in STEPS[current].iter().enumerate() {
        if i <= stage {
            let mark = if i == stage && p < 1.0 { "›" } else { "·" };
            nodes.push(line(&[
                (&format!("{mark} "), id.color()),
                (s, if i == stage { INK } else { DIM }),
            ]));
        } else {
            nodes.push(text("", DIM));
        }
    }
    nodes.push(text("", DIM));
    let graph_height = height.saturating_sub(15).clamp(3, 8);
    nodes.push(Node::raster(graph(
        width.saturating_sub(1),
        graph_height,
        time,
    )));
    nodes.push(line(&[
        ("⌂ PLAN  ", IDENTITIES[0].color()),
        ("◇ MAP  ", IDENTITIES[1].color()),
        ("▥ BUILD  ", IDENTITIES[2].color()),
        ("⊞ PROVE", IDENTITIES[3].color()),
    ]));
    nodes.push(text(
        if p >= 1.0 {
            "WITNESS SEALED / all contracts discharged"
        } else {
            "contract → execution → witness"
        },
        DIM,
    ));
    Node::col().children(nodes)
}
fn receipts(width: u16, time: f32) -> Node {
    let mut nodes = vec![
        line(&[("03  ", CYAN), ("RECEIPTS", INK)]),
        text("LOCAL FIXTURES / SIMULATED", DIM),
        text("", DIM),
    ];
    for (i, a) in AGENTS.into_iter().enumerate() {
        if a.status(time) == JobStatus::Complete {
            nodes.push(line(&[
                (IDENTITIES[i].signature, IDENTITIES[i].color()),
                (
                    &format!(" {:05.1}s  {}", a.finish_ms as f32 / 1000.0, a.name),
                    IDENTITIES[i].color(),
                ),
            ]));
            nodes.extend(a.result.split(" / ").map(|s| text(format!("  {s}"), INK)));
            nodes.push(text("", DIM));
        }
    }
    let values: Vec<f32> = (0..40)
        .map(|i| {
            let at = time - (39 - i) as f32 * 0.25;
            AGENTS
                .iter()
                .map(|a| (a.progress(at) - a.progress(at - 0.25)) * 10.0)
                .sum()
        })
        .collect();
    nodes.push(text("WORK DISCHARGED / 250ms", DIM));
    nodes.push(Node::line(gibson::show::sparkline(
        &values,
        usize::from(width.saturating_sub(2)),
        CYAN,
        IDENTITIES[3].color(),
    )));
    nodes.push(text("4 workers / one finite graph", DIM));
    Node::col().children(nodes)
}
pub fn render(width: u16, height: u16, time: f32) -> Surface {
    let complete = AGENTS
        .iter()
        .filter(|a| a.status(time) == JobStatus::Complete)
        .count();
    let margin = if width >= 80 { 2.0 } else { 1.0 };
    let mut root = Node::col()
        .width(width as f32)
        .height(height as f32)
        .background(BG)
        .padding_axes(margin, 1.0)
        .children(vec![
            Node::row().children(vec![
                line(&[("G / ", CYAN), ("LIBGIBSON", INK)]).flex_grow(1.0),
                text("AGENT OPERATIONS    /    01", DIM),
            ]),
            text("", DIM),
            Node::row().children(vec![
                text("Build a resilient transit planner.", INK).flex_grow(1.0),
                text(format!("{complete}/4 SEALED"), IDENTITIES[3].color()),
            ]),
            text(
                "A finite multi-agent simulation. One shared objective.",
                DIM,
            ),
            Node::rule(None::<String>, Style::new().fg(RULE).bg(BG)),
        ]);
    let available = height.saturating_sub(9);
    if width >= 110 {
        let aw = (width as f32 * 0.25) as u16;
        let rw = (width as f32 * 0.27) as u16;
        root.add_child(Node::row().height(available as f32).gap(2.0).children(vec![
            agents(aw, time).width(aw as f32),
            workspace(width.saturating_sub(aw + rw + 10), available, time).flex_grow(1.0),
            receipts(rw, time).width(rw as f32),
        ]));
    } else if width >= 74 {
        root.add_child(Node::row().height(available as f32).gap(2.0).children(vec![
            agents(24, time).width(24.0),
            workspace(width.saturating_sub(32), available, time).flex_grow(1.0),
        ]));
    } else {
        root.add_child(
            workspace(width.saturating_sub(4), available, time).height(available as f32),
        );
    }
    root.add_child(line(&[
        ("◈  ", CYAN),
        (
            if time < 19.5 {
                "orchestrator › contracts in flight"
            } else {
                "orchestrator › evidence sealed. Look closer."
            },
            INK,
        ),
    ]));
    let mut out = Surface::new(width, height);
    if gibson::layout::compute_layout(&mut root, width, height).is_ok() {
        gibson::painter::paint(&root, &mut out);
    }
    out
}
