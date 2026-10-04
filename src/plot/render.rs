//! Glyph realization: one [`PlotLayout`] → a `Surface`, under a chosen
//! [`SubcellGlyphMode`].
//!
//! Reuses the substrate — [`BrailleCanvas`] for subpixel line/scatter geometry,
//! `Surface` for axes/ticks/labels/annotation text. No second renderer, no raw
//! ANSI. The realization reads one compiled layout; capability only changes
//! *how* the geometry is drawn, never which samples were accepted or where the
//! ticks are (law F — those live in the capability-free `compile`).

use crate::canvas::BrailleCanvas;
use crate::cell::Glyph;
use crate::{Cell, Color, Style, SubcellGlyphMode, Surface};

use super::layout::{PlotLayout, Prims, ProjAnnotation};

/// Colours for the plot chrome. Series colours come from the data.
#[derive(Clone, Copy, Debug)]
pub struct PlotTheme {
    pub axis: (u8, u8, u8),
    pub label: (u8, u8, u8),
    pub title: (u8, u8, u8),
}

impl Default for PlotTheme {
    fn default() -> Self {
        PlotTheme {
            axis: (120, 130, 150),
            label: (170, 185, 205),
            title: (225, 235, 250),
        }
    }
}

fn style_of(rgb: (u8, u8, u8)) -> Style {
    Style {
        fg: Some(Color::rgb(rgb.0, rgb.1, rgb.2)),
        ..Default::default()
    }
}

fn putc(s: &mut Surface, x: u16, y: u16, ch: char, style: Style) {
    s.set_cell(x, y, Cell::new(Glyph::from_char(ch), style));
}

/// `"label"` or `"label (unit)"` — the drawn axis caption.
fn caption(axis: &super::data::AxisSpec) -> String {
    match &axis.unit {
        Some(u) if !u.is_empty() => format!("{} ({})", axis.label, u),
        _ => axis.label.clone(),
    }
}

/// Overlay a canvas's ink onto `dst` through `mode`, preserving each
/// destination cell's background, touching only cells that carry a dot and only
/// within `[ox, ox+cw) × [oy, oy+ch)` — the plot rectangle (law G: a primitive
/// cannot mutate a cell outside the plot area).
fn overlay(
    dst: &mut Surface,
    canvas: &BrailleCanvas,
    origin: (u16, u16),
    dims: (u16, u16),
    style: Style,
    mode: SubcellGlyphMode,
) {
    let (ox, oy) = origin;
    let (cw, ch) = dims;
    for cy in 0..ch {
        for cx in 0..cw {
            if let Some(g) = canvas.glyph_at_mode(cx, cy, mode) {
                if let Some(cell) = dst.get_mut(ox + cx, oy + cy) {
                    let bg = cell.style.bg;
                    cell.glyph = Glyph::from_char(g);
                    cell.style = style;
                    cell.style.bg = bg;
                    cell.is_continuation = false;
                }
            }
        }
    }
}

/// Realize a compiled layout into a `Surface` sized to `layout.area`, with the
/// default theme.
pub fn render(layout: &PlotLayout, mode: SubcellGlyphMode) -> Surface {
    render_themed(layout, mode, &PlotTheme::default())
}

/// Realize a compiled layout with an explicit theme.
pub fn render_themed(layout: &PlotLayout, mode: SubcellGlyphMode, theme: &PlotTheme) -> Surface {
    let area = layout.area;
    let mut s = Surface::new(area.width.max(1), area.height.max(1));
    if layout.plot_rect.width == 0 || layout.plot_rect.height == 0 {
        return s;
    }

    let lx = layout.plot_rect.x - area.x;
    let ly = layout.plot_rect.y - area.y;
    let w = layout.plot_rect.width;
    let h = layout.plot_rect.height;

    let axis_style = style_of(theme.axis);
    let label_style = style_of(theme.label);
    let title_style = style_of(theme.title);

    let ascii = matches!(mode, SubcellGlyphMode::Ascii);
    let vch = if ascii { '|' } else { '│' };
    let hch = if ascii { '-' } else { '─' };
    let corner = if ascii { '+' } else { '└' };
    let ytick = if ascii { '+' } else { '┤' };
    let xtick = if ascii { '+' } else { '┬' };

    // Title: the very top row of the reserved top margin.
    if layout.show_title {
        let maxw = area.width as usize;
        let t: String = layout.title.chars().take(maxw).collect();
        s.print_str(
            lx.min(area.width.saturating_sub(1)),
            0,
            &t,
            title_style,
            None,
        );
    }

    // Y-axis caption: horizontal, on the row directly above the plot (F1).
    if layout.show_y_title && ly >= 1 {
        let maxw = area.width as usize;
        let cap: String = caption(&layout.y_axis).chars().take(maxw).collect();
        s.print_str(
            lx.min(area.width.saturating_sub(1)),
            ly - 1,
            &cap,
            label_style,
            None,
        );
    }

    // Y axis + right-aligned labels (left margin).
    if lx >= 1 {
        for row in ly..ly + h {
            putc(&mut s, lx - 1, row, vch, axis_style);
        }
        if layout.show_y_labels {
            for t in &layout.y_ticks {
                let row = t.cell - area.y;
                putc(&mut s, lx - 1, row, ytick, axis_style);
                let maxw = (lx - 1) as usize;
                let lab: String = t.label.chars().take(maxw).collect();
                let startx = (lx - 1).saturating_sub(lab.chars().count() as u16);
                s.print_str(startx, row, &lab, label_style, None);
            }
        }
    }

    // X axis + centered labels (bottom margin: axis row, then label row).
    let axis_row = ly + h;
    if layout.show_x_labels && axis_row < area.height {
        for col in lx..lx + w {
            putc(&mut s, col, axis_row, hch, axis_style);
        }
        if lx >= 1 {
            putc(&mut s, lx - 1, axis_row, corner, axis_style);
        }
        let label_row = axis_row + 1;
        for t in &layout.x_ticks {
            let col = t.cell - area.x;
            putc(&mut s, col, axis_row, xtick, axis_style);
            if label_row < area.height {
                let half = (t.label.chars().count() as u16) / 2;
                let startx = col.saturating_sub(half).min(area.width.saturating_sub(1));
                let room = (area.width - startx) as usize;
                let lab: String = t.label.chars().take(room).collect();
                s.print_str(startx, label_row, &lab, label_style, None);
            }
        }
    }

    // X-axis caption: centered on its reserved row below the tick labels (F1).
    if layout.show_x_title {
        let cap_row = ly + h + 2; // axis row, tick-label row, then caption row
        if cap_row < area.height {
            let cap = caption(&layout.x_axis);
            let clen = cap.chars().count() as u16;
            let center = lx + w / 2;
            let startx = center
                .saturating_sub(clen / 2)
                .min(area.width.saturating_sub(1));
            let room = (area.width - startx) as usize;
            let t: String = cap.chars().take(room).collect();
            s.print_str(startx, cap_row, &t, label_style, None);
        }
    }

    // Annotations UNDER the data (data overdraws a reference line it crosses).
    for a in &layout.annotations {
        match a {
            ProjAnnotation::VLine { col_px, color } => {
                let mut c = BrailleCanvas::new(w, h);
                c.line(*col_px, 0, *col_px, layout.px_h.saturating_sub(1) as i32);
                overlay(&mut s, &c, (lx, ly), (w, h), style_of(*color), mode);
            }
            ProjAnnotation::HLine { row_px, color } => {
                let mut c = BrailleCanvas::new(w, h);
                c.line(0, *row_px, layout.px_w.saturating_sub(1) as i32, *row_px);
                overlay(&mut s, &c, (lx, ly), (w, h), style_of(*color), mode);
            }
            ProjAnnotation::Point { .. } => {}
        }
    }

    // Series geometry (each into its own canvas → its own colour).
    for ser in &layout.series {
        let mut c = BrailleCanvas::new(w, h);
        match &ser.prims {
            Prims::Scatter(points) => {
                for &(x, y) in points {
                    c.set(x, y);
                }
            }
            Prims::Segments(segs) => {
                for &((x0, y0), (x1, y1)) in segs {
                    c.line(x0, y0, x1, y1);
                }
            }
        }
        overlay(&mut s, &c, (lx, ly), (w, h), style_of(ser.color), mode);
    }

    // Point annotations + their labels, ON TOP.
    for a in &layout.annotations {
        if let ProjAnnotation::Point {
            x_px,
            y_px,
            label,
            color,
        } = a
        {
            let mut c = BrailleCanvas::new(w, h);
            c.set(*x_px, *y_px);
            overlay(&mut s, &c, (lx, ly), (w, h), style_of(*color), mode);
            let cxp = lx + (*x_px as u16) / 2;
            let cyp = ly + (*y_px as u16) / 4;
            if !label.is_empty() && cxp + 1 < area.width {
                let room = (area.width - (cxp + 1)) as usize;
                let lab: String = label.chars().take(room).collect();
                s.print_str(
                    cxp + 1,
                    cyp.min(area.height.saturating_sub(1)),
                    &lab,
                    style_of(*color),
                    None,
                );
            }
        }
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plot::data::{Annotation, AxisSpec, PlotSpec, Series};
    use crate::plot::layout::compile;
    use crate::plot::scale::{AxisScale, FiniteRange};
    use crate::plot::PlotView;
    use crate::Rect;

    const MODES: [SubcellGlyphMode; 4] = [
        SubcellGlyphMode::Braille2x4,
        SubcellGlyphMode::HalfBlock1x2,
        SubcellGlyphMode::Block,
        SubcellGlyphMode::Ascii,
    ];

    fn view() -> PlotView {
        PlotView::new(
            FiniteRange::new(0.0, 1.0).unwrap(),
            FiniteRange::new(0.0, 1.0).unwrap(),
        )
    }
    fn spec(series: Series) -> PlotSpec {
        PlotSpec::new(
            AxisSpec::new(AxisScale::Linear, "x"),
            AxisSpec::new(AxisScale::Linear, "y"),
        )
        .title("P")
        .series(series)
    }
    fn glyph_char(s: &Surface, x: u16, y: u16) -> char {
        s.get(x, y)
            .and_then(|c| c.glyph.grapheme.chars().next())
            .unwrap_or(' ')
    }
    fn nonspace_count(s: &Surface) -> usize {
        let mut n = 0;
        for y in 0..s.height {
            for x in 0..s.width {
                if glyph_char(s, x, y) != ' ' {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn renders_all_modes_and_sizes_without_panic() {
        let sp = spec(Series::line(vec![(0.0, 0.0), (0.5, 0.8), (1.0, 0.3)]))
            .annotate(Annotation::VLine {
                x: 0.5,
                color: (200, 80, 80),
            })
            .annotate(Annotation::Point {
                x: 0.5,
                y: 0.8,
                label: "peak".into(),
                color: (90, 230, 90),
            });
        for rect in [
            Rect::new(0, 0, 120, 40),
            Rect::new(0, 0, 80, 24),
            Rect::new(0, 0, 42, 15),
            Rect::new(0, 0, 3, 2),
            Rect::new(0, 0, 1, 1),
        ] {
            let (layout, _) = compile(&sp, &view(), rect).unwrap();
            for mode in MODES {
                let surf = render(&layout, mode);
                assert_eq!(surf.width, rect.width.max(1));
                assert_eq!(surf.height, rect.height.max(1));
            }
        }
    }

    #[test]
    fn draws_something_in_the_plot() {
        let sp = spec(Series::line(vec![(0.0, 0.0), (1.0, 1.0)]));
        let (layout, _) = compile(&sp, &view(), Rect::new(0, 0, 80, 24)).unwrap();
        let surf = render(&layout, SubcellGlyphMode::Braille2x4);
        assert!(nonspace_count(&surf) > 5, "a line should draw ink + chrome");
    }

    #[test]
    fn clipping_locality_law_g() {
        // A point far outside the view must not draw any geometry in the plot
        // interior, and must not touch the left-margin label columns.
        let far = spec(Series::scatter(vec![(100.0, 100.0)]));
        let (layout, rep) = compile(&far, &view(), Rect::new(0, 0, 80, 24)).unwrap();
        assert_eq!(rep.primitives_emitted, 0, "off-view point emits nothing");
        let surf = render(&layout, SubcellGlyphMode::Braille2x4);
        let lx = layout.plot_rect.x;
        let ly = layout.plot_rect.y;
        // interior: only axis/label chrome, no braille geometry dots
        for y in ly..ly + layout.plot_rect.height {
            for x in lx..lx + layout.plot_rect.width {
                let g = glyph_char(&surf, x, y);
                assert!(
                    !('\u{2800}'..='\u{28FF}').contains(&g),
                    "no geometry dot in interior at ({x},{y})"
                );
            }
        }
    }

    #[test]
    fn point_inside_draws_a_dot() {
        let inside = spec(Series::scatter(vec![(0.5, 0.5)]));
        let (layout, rep) = compile(&inside, &view(), Rect::new(0, 0, 80, 24)).unwrap();
        assert_eq!(rep.primitives_emitted, 1);
        let surf = render(&layout, SubcellGlyphMode::Braille2x4);
        let lx = layout.plot_rect.x;
        let ly = layout.plot_rect.y;
        let mut found = false;
        for y in ly..ly + layout.plot_rect.height {
            for x in lx..lx + layout.plot_rect.width {
                if ('\u{2800}'..='\u{28FF}').contains(&glyph_char(&surf, x, y)) {
                    found = true;
                }
            }
        }
        assert!(found, "a point inside the view must draw a braille dot");
    }

    #[test]
    fn capability_naturality_law_f_ticks_identical() {
        // Chrome (tick label positions) is identical across glyph modes; only
        // geometry glyphs differ. compile() took no capability at all.
        let sp = spec(Series::line(vec![(0.0, 0.0), (1.0, 1.0)]));
        let (layout, _) = compile(&sp, &view(), Rect::new(0, 0, 100, 30)).unwrap();
        let braille = render(&layout, SubcellGlyphMode::Braille2x4);
        let ascii = render(&layout, SubcellGlyphMode::Ascii);
        // y label cells identical
        for t in &layout.y_ticks {
            let row = t.cell;
            for x in 0..layout.plot_rect.x.saturating_sub(1) {
                assert_eq!(
                    glyph_char(&braille, x, row),
                    glyph_char(&ascii, x, row),
                    "label chrome must match across modes at ({x},{row})"
                );
            }
        }
    }
}
