//! Shared NASA-control-room scaffold. Every Observatory mode hands this file a
//! `View` and gets the same restrained header / panel / events-rail treatment
//! back — mission control, not a rave. Paint owns no clock: `frame()` reads
//! `View` and draws it, full stop. It never advances a tick, never samples
//! `/proc`, never mutates the log it's handed. A frozen frame is a photograph.

use gibson::canvas::braille_oscilloscope;
use gibson::capability::quantize_style;
use gibson::{
    transcode_surface_glyphs, Cell, Color, ColorDepth, Glyph, Rect, Style, SubcellGlyphMode,
    Surface,
};

use crate::diag::{Category, DiagLog};

const INK_NOMINAL: Color = Color::Rgb(139, 244, 200); // mint — steady state
const INK_WARN: Color = Color::Rgb(242, 193, 101); // gold — known, watched
const INK_FAULT: Color = Color::Rgb(240, 110, 110); // red — actually wrong
const CYAN: Color = Color::Rgb(95, 220, 244);
const WHITE: Color = Color::Rgb(222, 237, 246);
const MUTED: Color = Color::Rgb(109, 135, 161);
const GRID: Color = Color::Rgb(35, 60, 79);
const BG: Color = Color::Rgb(4, 9, 16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Nominal,
    Warn,
    Fault,
}
impl Tone {
    fn color(self) -> Color {
        match self {
            Tone::Nominal => INK_NOMINAL,
            Tone::Warn => INK_WARN,
            Tone::Fault => INK_FAULT,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PanelRow {
    pub label: String,
    pub value: String,
    pub tone: Tone,
}
impl PanelRow {
    pub fn new(label: impl Into<String>, value: impl Into<String>, tone: Tone) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            tone,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Spark {
    pub title: String,
    /// Raw retained-window samples, oldest to newest. Empty means "no
    /// observation yet" and is rendered as `?`, never an empty-but-confident
    /// graph.
    pub values: Vec<f64>,
    pub unit: String,
}

/// Everything one rendered frame needs. Mode drivers build this; `frame()`
/// only ever reads it.
pub struct View<'a> {
    pub monotonic_us: u64,
    pub mode_label: String,
    pub hero_state: String,
    pub hero_tone: Tone,
    pub subtitle: String,
    pub panel_title: String,
    pub panel_rows: Vec<PanelRow>,
    /// Pipeline/stage-style rows (Ghost-Key's EVENT PIPELINE list, plus any
    /// witness lines). When non-empty this replaces the sparkline area —
    /// Million-Tick uses `sparks`, Ghost-Key uses this; a mode never needs
    /// both. Empty by default so old modes don't have to know this exists.
    pub stage_rows: Vec<PanelRow>,
    pub sparks: Vec<Spark>,
    pub log: &'a DiagLog,
    pub footer: String,
    pub controls: String,
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
        let avail = width.min(self.surface.width - x);
        let mut clean: String = text.chars().filter(|c| !c.is_control()).collect();
        // Graceful ellipsis instead of a mid-word hard chop: a truncated
        // source string that just stops is easy to misread as complete.
        let char_len = clean.chars().count() as u16;
        if avail > 0 && char_len > avail {
            clean = if avail == 1 {
                "…".to_string()
            } else {
                clean
                    .chars()
                    .take(usize::from(avail - 1))
                    .collect::<String>()
                    + "…"
            };
        }
        self.surface.print_str(x, y, &clean, style, Some(avail));
    }
    fn rule(&mut self, x: u16, y: u16, width: u16, color: Color) {
        self.text(x, y, width, &"─".repeat(usize::from(width)), color, false);
    }
    fn border(&mut self, rect: Rect, color: Color) {
        let style = self.style(color);
        self.surface
            .draw_border(rect, gibson::BorderType::Single, style);
    }
}

fn ms(us: u64) -> String {
    format!("{}.{:03}", us / 1_000_000, (us / 1000) % 1000)
}

/// Pure projection: reads `view`, draws a `Surface`, returns it. No side
/// effects, no clock, no mutation of the log — the caller's frozen frame stays
/// frozen no matter how many times you paint it.
pub fn frame(
    view: &View,
    width: u16,
    height: u16,
    depth: ColorDepth,
    glyphs: SubcellGlyphMode,
) -> Surface {
    let mut surface = Surface::new(width, height);
    surface.fill_rect(
        surface.area(),
        Cell::new(
            Glyph::space(),
            quantize_style(Style::new().bg(BG).fg(WHITE), depth),
        ),
    );
    let mut ink = Ink {
        surface: &mut surface,
        depth,
    };
    if width < 60 || height < 20 {
        ink.text(0, 0, width, "RUNTIME OBSERVATORY", CYAN, true);
        ink.text(0, 2, width, "Enlarge to 60x20 for telemetry", MUTED, false);
        return surface;
    }
    let inner = width - 4;

    // Header: "LIBGIBSON // RUNTIME OBSERVATORY    [STATE]"
    let hero = format!("[{}]", view.hero_state);
    let title = format!("LIBGIBSON // RUNTIME OBSERVATORY // {}", view.mode_label);
    ink.text(
        2,
        1,
        inner.saturating_sub(hero.len() as u16 + 1),
        &title,
        CYAN,
        true,
    );
    ink.text(
        width.saturating_sub(hero.len() as u16 + 2),
        1,
        hero.len() as u16,
        &hero,
        view.hero_tone.color(),
        true,
    );
    ink.text(
        2,
        2,
        inner,
        &format!("{}   t={} s", view.subtitle, ms(view.monotonic_us)),
        MUTED,
        false,
    );
    ink.rule(2, 3, inner, GRID);

    // Layout: numeric panel (left) + subsystem status (right), then an events
    // rail across the bottom.
    let rail_top = height.saturating_sub(9);
    let panel_bottom = rail_top.saturating_sub(1);
    let panel_top = 5;
    let panel_height = panel_bottom.saturating_sub(panel_top);
    let left_w = if inner >= 90 { inner * 3 / 5 } else { inner };
    let left = Rect::new(2, panel_top, left_w, panel_height);
    ink.border(left, GRID);
    numeric_panel(&mut ink, view, left);

    if inner >= 90 {
        let right = Rect::new(2 + left_w + 1, panel_top, inner - left_w - 1, panel_height);
        ink.border(right, GRID);
        subsystem_panel(&mut ink, view, right);
    }

    events_rail(
        &mut ink,
        view,
        Rect::new(2, rail_top, inner, height.saturating_sub(3) - rail_top),
    );

    ink.rule(2, height - 3, inner, GRID);
    ink.text(2, height - 2, inner, &view.footer, MUTED, false);
    ink.text(
        2,
        height - 1,
        inner.saturating_sub(11),
        &view.controls,
        MUTED,
        false,
    );
    ink.text(width - 10, height - 1, 8, "Esc exit", WHITE, false);
    // Sparklines are the only sub-cell (Braille) content; realize them through
    // the requested glyph family. Braille2x4 is a no-op, so deterministic
    // callers stay byte-identical.
    transcode_surface_glyphs(&mut surface, glyphs);
    surface
}

fn numeric_panel(ink: &mut Ink<'_>, view: &View, r: Rect) {
    if r.width < 4 || r.height < 3 {
        return;
    }
    let x = r.x + 2;
    let w = r.width.saturating_sub(4);
    ink.text(x, r.y, w, &view.panel_title, WHITE, true);

    // Two-column metric layout: the numbers earn their keep, but the
    // sparklines are the actual proof (bounded vs. unbounded), so metrics
    // don't get to hog the vertical space they don't need.
    let col_w = (w / 2).max(1);
    let rows = (view.panel_rows.len() as u16).div_ceil(2);
    for (i, row) in view.panel_rows.iter().enumerate() {
        let cx = x + (i as u16 % 2) * col_w;
        let cy = r.y + 2 + (i as u16 / 2);
        if cy >= r.y + r.height.saturating_sub(1) {
            break;
        }
        ink.text(cx, cy, 10, &row.label, MUTED, false);
        ink.text(
            cx + 11,
            cy,
            col_w.saturating_sub(12),
            &row.value,
            row.tone.color(),
            false,
        );
    }
    let y = r.y + 2 + rows + 1;
    let lower_h = (r.y + r.height).saturating_sub(1).saturating_sub(y);
    if lower_h == 0 {
        return;
    }
    if !view.stage_rows.is_empty() {
        stage_list(ink, view, x, y, w, lower_h);
        return;
    }
    let spark_h = lower_h;
    if spark_h < 3 || view.sparks.is_empty() {
        return;
    }
    // Side by side, not stacked: each sparkline keeps the full remaining
    // height instead of splitting it, so the climb/flatten contrast this
    // whole panel exists to show stays actually legible.
    let lanes = view.sparks.len() as u16;
    let col = (w / lanes).max(1);
    for (i, spark) in view.sparks.iter().enumerate() {
        let sx = x + i as u16 * col;
        let cw = if i as u16 + 1 == lanes {
            w - col * i as u16
        } else {
            col.saturating_sub(1)
        };
        if cw < 4 {
            continue;
        }
        ink.text(sx, y, cw, &spark.title, MUTED, false);
        if spark.values.is_empty() {
            ink.text(
                sx,
                y + 1,
                cw,
                "? no observations in retained window",
                MUTED,
                false,
            );
            continue;
        }
        // Scale to the retained window's own min..max, not 0..max: a climb
        // that's still well above zero (e.g. RSS never near 0) deserves to
        // fill the graph, not get flattened against the ceiling.
        let min = spark.values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = spark
            .values
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        let last = spark.values.last().copied().unwrap_or(0.0);
        ink.text(
            sx,
            y + 1,
            cw,
            &format!(
                "last {last:.0}{u} · window {min:.0}..{max:.0}{u}",
                u = spark.unit
            ),
            MUTED,
            false,
        );
        let graph_h = spark_h.saturating_sub(2);
        if graph_h == 0 {
            continue;
        }
        let span = (max - min).max(1.0);
        let normalized: Vec<f32> = spark
            .values
            .iter()
            .map(|v| ((((v - min) / span).clamp(0.0, 1.0) * 2.0) - 1.0) as f32)
            .collect();
        let canvas = braille_oscilloscope(&normalized, cw, graph_h);
        let style = ink.style(INK_NOMINAL);
        canvas.paint_into(ink.surface, (sx, y + 2), style);
    }
}

/// One row per pipeline stage (or witness line) — label, then whatever the
/// mode driver already decided to say about it, tone and all. This function
/// doesn't judge observed-vs-'?'; it just prints what it's handed. If a mode
/// driver lies to it, that's the driver's problem, not this function's.
fn stage_list(ink: &mut Ink<'_>, view: &View, x: u16, y: u16, w: u16, h: u16) {
    for (i, row) in view.stage_rows.iter().take(usize::from(h)).enumerate() {
        let ry = y + i as u16;
        ink.text(x, ry, 14, &row.label, MUTED, true);
        ink.text(
            x + 15,
            ry,
            w.saturating_sub(15),
            &row.value,
            row.tone.color(),
            false,
        );
    }
}

fn subsystem_panel(ink: &mut Ink<'_>, view: &View, r: Rect) {
    if r.width < 4 || r.height < 3 {
        return;
    }
    let x = r.x + 2;
    let w = r.width.saturating_sub(4);
    ink.text(x, r.y, w, "SUBSYSTEM STATUS", WHITE, true);
    for (i, category) in Category::ALL.into_iter().enumerate() {
        let y = r.y + 2 + i as u16;
        if y >= r.y + r.height.saturating_sub(1) {
            break;
        }
        let count = view.log.count_of_category(category);
        let (text, color) = match view.log.last_of_category(category) {
            Some(rec) => (
                format!(
                    "{:<9} {}={} (n={count})",
                    category.label(),
                    rec.kind,
                    rec.value
                ),
                INK_NOMINAL,
            ),
            None => (
                format!("{:<9} ?  unobserved this mode", category.label()),
                MUTED,
            ),
        };
        ink.text(x, y, w, &text, color, false);
    }
}

fn events_rail(ink: &mut Ink<'_>, view: &View, r: Rect) {
    if r.height < 2 || r.width < 4 {
        return;
    }
    let x = r.x;
    let w = r.width;
    let heading = format!(
        "RECENT EVENTS / ANOMALIES   (showing {} / retained {} of cap {} · {} dropped from diagnostic log)",
        view.log.retained().min(usize::from(r.height.saturating_sub(2))),
        view.log.retained(),
        view.log.cap(),
        view.log.dropped()
    );
    ink.text(x, r.y, w, &heading, WHITE, true);
    for (i, rec) in view
        .log
        .recent(usize::from(r.height.saturating_sub(2)))
        .enumerate()
    {
        let y = r.y + 1 + i as u16;
        if y >= r.y + r.height {
            break;
        }
        let line = format!(
            "{:>10.3}s  {:<8} {:<14} {:<28} src={}",
            rec.monotonic_us as f64 / 1_000_000.0,
            rec.category.label(),
            rec.kind,
            rec.value,
            rec.source
        );
        ink.text(x, y, w, &line, MUTED, false);
    }
    if view.log.retained() == 0 {
        ink.text(
            x,
            r.y + 1,
            w,
            "? no diagnostic records observed yet",
            MUTED,
            false,
        );
    }
}
