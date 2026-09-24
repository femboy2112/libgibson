//! Pure, example-local projection of measured event receipts. A dark checkpoint
//! means no probe, never evidence of a stalled backend. Paint owns no clock.
use gibson::capability::quantize_style;
use gibson::{BrailleCanvas, Cell, Color, ColorDepth, Glyph, Rect, Style, Surface};

pub const STAGES: [&str; 6] = [
    "PTY WRITE",
    "TTY SAMPLE",
    "BACKEND",
    "DECODE",
    "CONTEXT",
    "APP",
];
const CYAN: Color = Color::Rgb(95, 220, 244);
const GOLD: Color = Color::Rgb(242, 193, 101);
const VIOLET: Color = Color::Rgb(176, 150, 249);
const MINT: Color = Color::Rgb(139, 244, 200);
const WHITE: Color = Color::Rgb(222, 237, 246);
const MUTED: Color = Color::Rgb(109, 135, 161);
const GRID: Color = Color::Rgb(35, 60, 79);
const BG: Color = Color::Rgb(5, 13, 23);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    /// Same monotonic epoch as View::micros; the owner aligns child receipts.
    pub micros: u64,
    pub source: String,
    pub kind: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pulse {
    pub id: u64,
    pub key: String,
    /// Observed receipts only, in STAGES order. A queue occupancy sample is not
    /// a key identity; fill TTY only when the probe can actually correlate it.
    pub observed_us: [Option<u64>; 6],
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub micros: u64,
    pub scenario: String,
    pub mode: String,
    pub running: bool,
    pub result: String,
    pub pause_ms: u64,
    pub bytes_generated: u64,
    pub bytes_drained: u64,
    pub frames: u64,
    pub entries: Vec<Entry>,
    pub pulses: Vec<Pulse>,
    pub latencies_us: Vec<u64>,
    pub dropped: u64,
    pub illustrative: bool,
    /// Probe presence, not success. Unsupported intermediate observations must
    /// remain false even if a later application receipt exists.
    pub observed_stages: [bool; 6],
    pub controls: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LatencySummary {
    pub samples: usize,
    pub p50_us: u64,
    pub p95_us: u64,
    pub max_us: u64,
}

/// Nearest-rank diagnostics over the latest 512 samples, not an SLA estimate.
pub fn latency_summary(samples: &[u64]) -> Option<LatencySummary> {
    let mut sorted: Vec<_> = samples.iter().rev().take(512).copied().collect();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_unstable();
    let percentile = |p: usize| sorted[(p * sorted.len()).div_ceil(100).saturating_sub(1)];
    Some(LatencySummary {
        samples: sorted.len(),
        p50_us: percentile(50),
        p95_us: percentile(95),
        max_us: *sorted.last().unwrap(),
    })
}

struct Ink<'a> {
    surface: &'a mut Surface,
    depth: ColorDepth,
}
impl Ink<'_> {
    fn style(&self, color: Color) -> Style {
        let mut style = quantize_style(Style::new().fg(color).bg(BG), self.depth);
        if self.depth == ColorDepth::Mono && matches!(color, MUTED | GRID) {
            style.dim = true;
        }
        style
    }
    fn text(&mut self, x: u16, y: u16, width: u16, text: &str, color: Color, bold: bool) {
        if x >= self.surface.width || y >= self.surface.height || width == 0 {
            return;
        }
        let mut style = self.style(color);
        style.bold = bold;
        // Diagnostic values are data; never let a receipt become terminal code.
        let clean: String = text.chars().filter(|c| !c.is_control()).collect();
        self.surface
            .print_str(x, y, &clean, style, Some(width.min(self.surface.width - x)));
    }
    fn rule(&mut self, x: u16, y: u16, width: u16, color: Color) {
        self.text(x, y, width, &"─".repeat(usize::from(width)), color, false);
    }
    fn dot(&mut self, x: u16, y: u16, glyph: &str, color: Color, bold: bool) {
        self.text(x, y, 1, glyph, color, bold);
    }
    fn canvas(&mut self, canvas: &BrailleCanvas, x: u16, y: u16, color: Color) {
        let style = self.style(color);
        canvas.paint_into(self.surface, (x, y), style);
    }
}

fn ms(us: u64) -> String {
    format!("{}.{:01}", us / 1000, (us % 1000) / 100)
}
fn bytes(n: u64) -> String {
    if n >= 1_048_576 {
        format!("{:.1} MiB", n as f64 / 1_048_576.)
    } else if n >= 1024 {
        format!("{:.1} KiB", n as f64 / 1024.)
    } else {
        format!("{n} B")
    }
}
fn stage_color(i: usize) -> Color {
    [CYAN, GOLD, VIOLET, VIOLET, CYAN, MINT][i.min(5)]
}
fn source_lane(source: &str) -> usize {
    let source = source.to_ascii_uppercase();
    if source.contains("APP") {
        5
    } else if source.contains("CONTEXT") {
        4
    } else if source.contains("CROSSTERM") || source.contains("DECODE") || source.contains("RAW") {
        3
    } else if source.contains("BACKEND") {
        2
    } else if source.contains("TTY") {
        1
    } else if source.contains("PTY") {
        0
    } else {
        6
    }
}
fn event_glyph(kind: &str) -> &'static str {
    let kind = kind.to_ascii_lowercase();
    if kind.contains("resize") || kind.contains("winch") {
        "▲"
    } else if kind.contains("key") || kind.contains("read") || kind.contains("decode") {
        "◆"
    } else {
        "▪"
    }
}

/// Render supplied observations only. Width/height are bounded by the owning
/// lab; this projection scans at most 1,024 entries, 512 samples and 3 pulses.
pub fn frame(view: &View, width: u16, height: u16, depth: ColorDepth) -> Surface {
    let mut surface = Surface::new(width, height);
    surface.fill_rect(
        surface.area(),
        Cell::new(
            Glyph::space(),
            quantize_style(Style::new().bg(BG).fg(WHITE), depth),
        ),
    );
    if width < 32 || height < 18 {
        let mut ink = Ink {
            surface: &mut surface,
            depth,
        };
        ink.text(0, 0, width, "EVENT PRESSURE LAB", CYAN, true);
        ink.text(0, 2, width, "Enlarge to 32x18 for telemetry", MUTED, false);
        ink.text(0, height.saturating_sub(1), width, "Esc exit", WHITE, false);
        return surface;
    }
    let mut ink = Ink {
        surface: &mut surface,
        depth,
    };
    let compact = width < 96;
    let inner = width - 4;
    let status = if view.illustrative {
        "ILLUSTRATIVE"
    } else if view.running {
        "LIVE"
    } else {
        "RECEIPTS"
    };
    ink.text(
        2,
        1,
        inner.saturating_sub(15),
        if compact {
            "LIBGIBSON // EVENT LAB"
        } else {
            "LIBGIBSON // EVENT PRESSURE LAB"
        },
        CYAN,
        true,
    );
    ink.text(
        width - 15,
        1,
        13,
        status,
        if view.illustrative { GOLD } else { MINT },
        true,
    );
    ink.text(
        2,
        2,
        inner,
        &format!(
            "{} / {}   t={} ms",
            view.mode,
            view.scenario,
            ms(view.micros)
        ),
        MUTED,
        false,
    );
    ink.rule(2, 3, inner, GRID);
    pipeline(&mut ink, view, 4, compact);
    pressure(&mut ink, view, 11, compact);

    let content_end = height.saturating_sub(3);
    let region = Rect::new(2, 16, inner, content_end.saturating_sub(16));
    if compact {
        compact_telemetry(&mut ink, view, region);
    } else {
        let waterfall_width = (inner * 3 / 5).saturating_sub(2);
        waterfall(
            &mut ink,
            view,
            Rect::new(region.x, region.y, waterfall_width, region.height),
        );
        latency(
            &mut ink,
            view,
            Rect::new(
                region.x + waterfall_width + 3,
                region.y,
                inner - waterfall_width - 3,
                region.height,
            ),
        );
    }
    ink.rule(2, height - 3, inner, GRID);
    ink.text(
        2,
        height - 2,
        inner,
        &view.result,
        if view.result.to_ascii_lowercase().contains("fail") {
            GOLD
        } else {
            MINT
        },
        false,
    );
    ink.text(
        2,
        height - 1,
        inner.saturating_sub(11),
        &view.controls,
        MUTED,
        false,
    );
    ink.text(width - 10, height - 1, 8, "Esc exit", WHITE, false);
    surface
}

fn pipeline(ink: &mut Ink<'_>, view: &View, top: u16, compact: bool) {
    let width = ink.surface.width - 4;
    ink.text(2, top, width, "01 / INPUT PIPELINE", WHITE, true);
    if !compact {
        ink.text(
            ink.surface.width - 48,
            top,
            46,
            "receipts only · ? = unprobed · ○ = no receipt",
            MUTED,
            false,
        );
    }
    let left = if compact { 8 } else { 14 };
    let span = ink.surface.width - left - 8;
    let xs: [u16; 6] = std::array::from_fn(|i| left + span * i as u16 / 5);
    for (i, x) in xs.into_iter().enumerate() {
        let label = if compact {
            ["PTY", "TTY", "READY", "DECODE", "CTX", "APP"][i]
        } else {
            STAGES[i]
        };
        let label_width = label.len() as u16;
        ink.text(
            x.saturating_sub(label_width / 2),
            top + 1,
            label_width,
            label,
            if view.observed_stages[i] {
                stage_color(i)
            } else {
                MUTED
            },
            true,
        );
    }
    let recent: Vec<_> = view.pulses.iter().rev().take(3).collect();
    for row in 0..3 {
        let y = top + 3 + row;
        ink.rule(left, y, span + 1, GRID);
        let pulse = recent.get(row as usize);
        ink.text(
            2,
            y,
            left - 3,
            &pulse.map_or_else(|| "—".into(), |p| format!("{}#{}", p.key, p.id)),
            CYAN,
            true,
        );
        for (i, x) in xs.into_iter().enumerate() {
            let stamp = pulse
                .and_then(|p| p.observed_us[i])
                .filter(|&t| t <= view.micros);
            let glyph = if !view.observed_stages[i] {
                "?"
            } else if stamp.is_some() {
                "◆"
            } else {
                "○"
            };
            let age = stamp.map_or(u64::MAX, |t| view.micros.saturating_sub(t));
            ink.dot(
                x,
                y,
                glyph,
                if stamp.is_some() {
                    stage_color(i)
                } else {
                    MUTED
                },
                age < 300_000,
            );
            if stamp.is_some() {
                // A tiny shrinking wake marks a *measured* checkpoint. It does
                // not interpolate a key through an unobserved backend.
                if age < 250_000 && x > left + 1 {
                    ink.dot(x - 1, y, "·", stage_color(i), false);
                }
            }
        }
    }
    let note = if view.illustrative {
        "ILLUSTRATIVE fixture; not a historical failure"
    } else if compact {
        "◆ receipt   ○ not yet observed   ? unprobed"
    } else {
        "TTY = queue sample, not byte identity. Connections imply no unmeasured intermediate receipt."
    };
    ink.text(
        2,
        top + 6,
        width,
        note,
        if view.illustrative { GOLD } else { MUTED },
        false,
    );
}

fn pressure(ink: &mut Ink<'_>, view: &View, top: u16, compact: bool) {
    let width = ink.surface.width - 4;
    ink.text(
        2,
        top,
        width,
        &format!("02 / OUTPUT PRESSURE     drain pause {} ms", view.pause_ms),
        WHITE,
        true,
    );
    ink.text(
        2,
        top + 1,
        width,
        &format!(
            "commit {} / drained {} / {} frames",
            bytes(view.bytes_generated),
            bytes(view.bytes_drained),
            view.frames
        ),
        CYAN,
        false,
    );
    let backlog = view.bytes_generated.saturating_sub(view.bytes_drained);
    let rail = if compact { width / 3 } else { width / 2 };
    let occupied = ((u128::from(backlog.min(65_536)) * u128::from(rail)) / 65_536) as u16;
    ink.text(
        2,
        top + 2,
        rail,
        &"░".repeat(usize::from(rail)),
        GRID,
        false,
    );
    ink.text(
        2,
        top + 2,
        occupied,
        &"━".repeat(usize::from(occupied)),
        GOLD,
        true,
    );
    ink.text(
        rail + 4,
        top + 2,
        width - rail - 2,
        &format!("{} EST. unread", bytes(backlog)),
        GOLD,
        false,
    );
    let note = if compact {
        "not kernel queue · excludes in-flight bytes".into()
    } else {
        format!(
            "commit − drained estimate; not kernel queue depth · dropped {}",
            view.dropped
        )
    };
    ink.text(2, top + 3, width, &note, MUTED, false);
    let pending = view
        .entries
        .iter()
        .rev()
        .take(1024)
        .find(|entry| {
            entry.micros <= view.micros
                && entry.source == "RENDER"
                && matches!(entry.kind.as_str(), "FrameBegin" | "FrameCommitted")
        })
        .is_some_and(|entry| entry.kind == "FrameBegin");
    let pending_note = if pending {
        "FRAME PENDING / in-flight bytes unmeasured"
    } else {
        "rail 64KiB · in-flight/control bytes excluded"
    };
    ink.text(
        2,
        top + 4,
        width,
        pending_note,
        if pending { GOLD } else { MUTED },
        pending,
    );
}

fn waterfall(ink: &mut Ink<'_>, view: &View, r: Rect) {
    if r.height < 3 || r.width < 20 {
        return;
    }
    ink.text(r.x, r.y, r.width, "03 / EVENT WATERFALL", WHITE, true);
    let recent: Vec<_> = view
        .entries
        .iter()
        .rev()
        .take(1024)
        .filter(|e| e.micros <= view.micros)
        .collect();
    let start = view.micros.saturating_sub(2_000_000);
    let window = view.micros.saturating_sub(start).max(1);
    let axis_x = r.x + 9;
    let axis_w = r.width - 10;
    let lanes = [
        "PTY", "TTY", "BACKEND", "DECODE", "CONTEXT", "APP", "OUTPUT",
    ];
    for (i, label) in lanes.iter().enumerate() {
        if i as u16 + 2 >= r.height {
            break;
        }
        let y = r.y + 2 + i as u16;
        ink.text(
            r.x,
            y,
            8,
            label,
            if i < 6 { stage_color(i) } else { GOLD },
            false,
        );
        ink.rule(axis_x, y, axis_w, GRID);
        for entry in recent
            .iter()
            .rev()
            .filter(|e| source_lane(&e.source) == i && e.micros >= start)
        {
            let dx = (u128::from(entry.micros - start) * u128::from(axis_w - 1)
                / u128::from(window)) as u16;
            let resize = entry.kind.to_ascii_lowercase().contains("resize");
            ink.dot(
                axis_x + dx,
                y,
                event_glyph(&entry.kind),
                if resize || i == 6 {
                    GOLD
                } else {
                    stage_color(i)
                },
                true,
            );
        }
    }
    ink.text(
        axis_x,
        r.y + 1,
        axis_w,
        &format!("−{} ms", ms(window)),
        MUTED,
        false,
    );
    ink.text(axis_x + axis_w - 3, r.y + 1, 3, "now", MUTED, false);
    if r.height > 10 {
        ink.text(
            r.x,
            r.y + 10,
            r.width,
            "▲ resize  ◆ input  ▪ frame / drain",
            MUTED,
            false,
        );
        for (row, e) in recent.iter().take(usize::from(r.height - 11)).enumerate() {
            ink.text(
                r.x,
                r.y + 11 + row as u16,
                r.width,
                &format!("{:>8} {} / {} {}", ms(e.micros), e.source, e.kind, e.value),
                if source_lane(&e.source) == 5 {
                    MINT
                } else {
                    MUTED
                },
                false,
            );
        }
    }
}

fn latency(ink: &mut Ink<'_>, view: &View, r: Rect) {
    if r.height < 4 || r.width < 14 {
        return;
    }
    ink.text(r.x, r.y, r.width, "04 / DELIVERY LATENCY", WHITE, true);
    let Some(stats) = latency_summary(&view.latencies_us) else {
        ink.text(
            r.x,
            r.y + 2,
            r.width,
            "Awaiting correlated receipts",
            MUTED,
            false,
        );
        return;
    };
    ink.text(
        r.x,
        r.y + 1,
        r.width,
        &format!("p50 {}  p95 {} ms", ms(stats.p50_us), ms(stats.p95_us)),
        MINT,
        false,
    );
    ink.text(
        r.x,
        r.y + 2,
        r.width,
        &format!("max {} ms / n={}", ms(stats.max_us), stats.samples),
        MUTED,
        false,
    );
    sparkline(
        ink,
        view,
        Rect::new(r.x, r.y + 4, r.width, r.height.saturating_sub(5)),
    );
    ink.text(
        r.x,
        r.y + r.height - 1,
        r.width,
        "diagnostic samples · latest 512",
        MUTED,
        false,
    );
}

fn sparkline(ink: &mut Ink<'_>, view: &View, r: Rect) {
    if r.height == 0 || r.width == 0 {
        return;
    }
    let Some(stats) = latency_summary(&view.latencies_us) else {
        return;
    };
    let mut canvas = BrailleCanvas::new(r.width, r.height);
    let count = view.latencies_us.len().min(512);
    let samples = &view.latencies_us[view.latencies_us.len() - count..];
    let max = stats.max_us.max(1000);
    let x_max = i32::from(canvas.pixel_width()) - 1;
    let y_max = i32::from(canvas.pixel_height()) - 1;
    let mut previous = None;
    for (i, value) in samples.iter().enumerate() {
        let x = (i * x_max as usize / count.saturating_sub(1).max(1)) as i32;
        let y = y_max - (u128::from(*value) * y_max as u128 / u128::from(max)) as i32;
        if let Some((px, py)) = previous {
            canvas.line(px, py, x, y);
        }
        canvas.set(x, y);
        previous = Some((x, y));
    }
    ink.canvas(&canvas, r.x, r.y, MINT);
}

fn compact_telemetry(ink: &mut Ink<'_>, view: &View, r: Rect) {
    if r.height == 0 {
        return;
    }
    let label = latency_summary(&view.latencies_us).map_or_else(
        || "LATENCY  awaiting paired receipts".to_owned(),
        |s| {
            format!(
                "LATENCY p50 {} p95 {} max {} ms n={}",
                ms(s.p50_us),
                ms(s.p95_us),
                ms(s.max_us),
                s.samples
            )
        },
    );
    ink.text(r.x, r.y, r.width, &label, MINT, true);
    let graph_rows = if r.height >= 5 && !view.latencies_us.is_empty() {
        2
    } else {
        0
    };
    for (i, entry) in view
        .entries
        .iter()
        .rev()
        .take(1024)
        .filter(|e| e.micros <= view.micros)
        .take(usize::from(r.height.saturating_sub(1 + graph_rows)))
        .enumerate()
    {
        ink.text(
            r.x,
            r.y + 1 + i as u16,
            r.width,
            &format!(
                "{:>7}  {} {} {}",
                ms(entry.micros),
                entry.source,
                entry.kind,
                entry.value
            ),
            if entry.kind.to_ascii_lowercase().contains("resize") {
                GOLD
            } else {
                MUTED
            },
            false,
        );
    }
    if graph_rows > 0 {
        sparkline(
            ink,
            view,
            Rect::new(r.x, r.y + r.height - graph_rows, r.width, graph_rows),
        );
    }
}

/// Art-direction fixture, deliberately not named a reproduced failure. The real
/// host replaces this with measured receipts from its independently owned PTY.
pub fn fixture() -> View {
    let mut entries = Vec::new();
    let mut pulses = Vec::new();
    for (i, key) in ["A", "B", "C"].into_iter().enumerate() {
        let t = 500_000 + i as u64 * 470_000;
        let delay = [1900, 63_800, 2700][i];
        for (source, kind, micros) in [
            ("PTY", "KeyWrite", t),
            ("CONTEXT", "EventReturned", t + delay - 200),
            ("APP", "KeyObserved", t + delay),
        ] {
            entries.push(Entry {
                micros,
                source: source.into(),
                kind: kind.into(),
                value: key.into(),
            });
        }
        entries.push(Entry {
            micros: t.saturating_sub(3000),
            source: "PTY".into(),
            kind: "Resize".into(),
            value: ["56x24", "80x24", "160x40"][i].into(),
        });
        pulses.push(Pulse {
            id: i as u64 + 1,
            key: key.into(),
            observed_us: [
                Some(t),
                None,
                None,
                None,
                Some(t + delay - 200),
                Some(t + delay),
            ],
        });
    }
    entries.sort_by_key(|e| e.micros);
    View {
        micros: 1_500_000,
        scenario: "resize/key".into(),
        mode: "Context".into(),
        running: false,
        result: "ILLUSTRATIVE · use a measured run for causal conclusions".into(),
        pause_ms: 60,
        bytes_generated: 341_680,
        bytes_drained: 319_126,
        frames: 18,
        entries,
        pulses,
        latencies_us: vec![1900, 2300, 2700, 3100, 2900, 4200, 63_800, 3300, 2700],
        dropped: 0,
        illustrative: true,
        observed_stages: [true, false, false, false, true, true],
        controls: "R run · 1–5 scenario · [ ] pressure · Space pause · Esc exit".into(),
    }
}
