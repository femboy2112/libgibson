//! An ordinary declarative UI. The membrane receives its realized Surface;
//! none of these widgets knows that it will become a city.
use super::model::{JobStatus, AGENTS};
use gibson::canvas::BrailleCanvas;
use gibson::cell::{Color, Line, RichText, Span, Style};
use gibson::node::Node;
use gibson::surface::Surface;

const INK: Color = Color::Rgb(203, 216, 233);
const DIM: Color = Color::Rgb(97, 119, 146);
const CYAN: Color = Color::Rgb(101, 225, 237);
const LILAC: Color = Color::Rgb(178, 156, 245);
const BG: Color = Color::Rgb(7, 12, 23);
fn text(s: impl Into<String>, color: Color) -> Node {
    Node::text(s, Style::new().fg(color).bg(BG)).height(1.0)
}
fn line(parts: &[(&str, Color)]) -> Node {
    Node::line(Line::from_spans(
        parts
            .iter()
            .map(|(s, c)| Span::styled(s, Style::new().fg(*c).bg(BG)))
            .collect(),
    ))
    .height(1.0)
}
fn graph(width: u16, height: u16, time: f32) -> Surface {
    let mut b = BrailleCanvas::new(width, height);
    let w = width as f32 * 2.0;
    let h = height as f32 * 4.0;
    let p = [
        (w * 0.12, h * 0.5),
        (w * 0.38, h * 0.25),
        (w * 0.63, h * 0.7),
        (w * 0.88, h * 0.4),
    ];
    for i in 0..3 {
        let (a, c) = (p[i], p[i + 1]);
        b.line(a.0 as i32, a.1 as i32, c.0 as i32, c.1 as i32);
        let u = (time * 0.35 - i as f32 * 0.18).rem_euclid(1.0);
        let x = a.0 + (c.0 - a.0) * u;
        let y = a.1 + (c.1 - a.1) * u;
        for dy in -1..=1 {
            for dx in -1..=1 {
                b.set(x as i32 + dx, y as i32 + dy);
            }
        }
    }
    for (i, (x, y)) in p.into_iter().enumerate() {
        let r: i32 = if AGENTS[i].status(time) == JobStatus::Running {
            4
        } else {
            2
        };
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() + dy.abs() == r {
                    b.set(x as i32 + dx, y as i32 + dy);
                }
            }
        }
    }
    {
        let mut surface = b.to_surface(Style::new().fg(CYAN).bg(BG));
        for c in &mut surface.cells {
            c.style.bg = Some(BG);
        }
        surface
    }
}
fn agents(width: u16, time: f32) -> Node {
    let mut nodes = vec![
        line(&[("01  ", CYAN), ("ORCHESTRATION", INK)]),
        text("PLAN → MAP → BUILD → PROVE", DIM),
        text("", DIM),
    ];
    for a in AGENTS {
        let (mark, state, col) = match a.status(time) {
            JobStatus::Queued => ("○", "queued", DIM),
            JobStatus::Running => ("◈", "running", CYAN),
            JobStatus::Complete => ("✓", "sealed", LILAC),
        };
        nodes.push(line(&[
            (&format!("{mark}  {:<10}", a.name), col),
            (&format!("  {state}"), DIM),
        ]));
        nodes.push(text(format!("   {}", a.role), DIM));
        let n = usize::from(width.saturating_sub(5)).min(30);
        let filled = (a.progress(time) * n as f32) as usize;
        nodes.push(line(&[
            ("   ", DIM),
            (&"━".repeat(filled), col),
            (&"─".repeat(n - filled), Color::Rgb(31, 48, 67)),
        ]));
        nodes.push(text("", DIM));
    }
    Node::col().children(nodes)
}
fn workspace(width: u16, height: u16, time: f32) -> Node {
    let current = AGENTS
        .iter()
        .position(|a| a.status(time) == JobStatus::Running)
        .unwrap_or(3);
    let a = AGENTS[current];
    let p = a.progress(time);
    let mut nodes = vec![
        line(&[("02  ", CYAN), ("LIVE WORKSPACE", INK)]),
        text("OFFLINE TRANSIT / DISRUPTION PLANNER", DIM),
        text("", DIM),
    ];
    nodes.push(text(a.task, INK));
    nodes.push(text("", DIM));
    let lines: [&[&str]; 4] = [
        &[
            "goal   Preserve reachable, accessible routes.",
            "split  Extract topology before changing policy.",
            "gate   No route may invent a disconnected edge.",
            "emit   Acceptance contract → SCOUT",
        ],
        &[
            "read   fixtures/transit.graph",
            "index  128 stops · 384 directed links",
            "check  wheelchair access / transfer windows",
            "emit   Normalized route graph → BUILDER",
        ],
        &[
            "build  Deterministic shortest viable route",
            "refine Three candidates; stable tie ordering",
            "check  Closed transfer must invalidate path",
            "emit   Candidate + replay seed → VERIFY",
        ],
        &[
            "test   Disconnected destination / empty graph",
            "test   Tie ordering / zero transfer window",
            "replay Same events → same answer",
            "seal   24 fixtures passed; no external calls",
        ],
    ];
    for (i, s) in lines[current].iter().enumerate() {
        if p > i as f32 * 0.22 {
            nodes.push(text(format!("  {s}"), if i == 3 { CYAN } else { INK }));
        } else {
            nodes.push(text("", DIM));
        }
    }
    nodes.push(text("", DIM));
    nodes.push(Node::raster(graph(
        width.saturating_sub(1),
        height.saturating_sub(15).clamp(3, 8),
        time,
    )));
    nodes.push(text(
        "A: PLAN        S: MAP        B: BUILD        V: PROVE",
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
    for a in AGENTS {
        if a.status(time) == JobStatus::Complete {
            nodes.push(text(
                format!("{:05.1}s  {}", a.finish_ms as f32 / 1000.0, a.name),
                CYAN,
            ));
            let words = a.result.split(" / ").map(|s| text(format!("  {s}"), INK));
            nodes.extend(words);
            nodes.push(text("", DIM));
        }
    }
    let values: Vec<f32> = (0..40)
        .map(|i| 0.3 + 0.25 * (time * 0.4 + i as f32 * 0.45).sin() + 0.15 * (i as f32 * 1.7).cos())
        .collect();
    nodes.push(text("SCHEDULER ACTIVITY", DIM));
    nodes.push(Node::line(gibson::show::sparkline(
        &values,
        usize::from(width.saturating_sub(2)),
        CYAN,
        LILAC,
    )));
    nodes.push(text("4 workers / bounded fictional run", DIM));
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
                text("G / LIBGIBSON", CYAN).flex_grow(1.0),
                text("AGENT OPERATIONS    /    01", DIM),
            ]),
            text("", DIM),
            Node::row().children(vec![
                text("Build a resilient transit planner.", INK).flex_grow(1.0),
                text(format!("{complete}/4 SEALED"), LILAC),
            ]),
            text(
                "A finite multi-agent simulation. One shared objective.",
                DIM,
            ),
            Node::rule(
                None::<String>,
                Style::new().fg(Color::Rgb(35, 55, 78)).bg(BG),
            ),
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
    root.add_child(
        Node::rich_text(RichText::from_lines(vec![Line::from_spans(vec![
            Span::styled("◈  ", Style::new().fg(CYAN).bg(BG)),
            Span::styled(
                if time < 19.5 {
                    "orchestrator › contracts in flight"
                } else {
                    "orchestrator › evidence sealed. Look closer."
                },
                Style::new().fg(INK).bg(BG),
            ),
        ])]))
        .height(1.0),
    );
    let mut out = Surface::new(width, height);
    if gibson::layout::compute_layout(&mut root, width, height).is_ok() {
        gibson::painter::paint(&root, &mut out);
    }
    out
}
