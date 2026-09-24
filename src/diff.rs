use crate::cell::Cell;
use crate::surface::Surface;

/// A contiguous run of modified cells on a single row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellRun {
    pub x: u16,
    pub cells: Vec<Cell>,
}

/// A set of updates for a single row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowPatch {
    pub y: u16,
    pub runs: Vec<CellRun>,
    /// If Some(x), erase from column x to the end of the line (CSI K).
    pub erase_eol_from: Option<u16>,
}

/// A diff between two surfaces.
///
/// # Damage model
///
/// Three distinct notions are tracked and must never be collapsed into one
/// misleading number:
///
/// * **Exact semantic delta** ([`SurfaceDiff::exact_changed_cell_count`]) — the
///   number of cells whose *rendered state* actually changed, treating absent
///   rows/columns as blank. This is the only metric that is a true "state delta".
/// * **Logical affected footprint** ([`SurfaceDiff::logical_dirty_count`], also
///   [`SurfaceDiff::affected_cell_count`]) — the cells *addressed* by the logical
///   update semantics: the union of explicit changed runs, the region covered by
///   an erase-to-EOL (`CSI K`), and any cleared trailing rows. This is a
///   conservative superset of the exact delta: a `CSI K` over a row where some
///   cells were already blank still counts those blank cells, because the wire
///   operation addresses them.
/// * **Wire cost** — the bytes actually emitted, measured by the renderer, not
///   here. A single `CSI K` can address dozens of cells while costing a handful
///   of bytes.
///
/// Because cleared trailing rows are counted against `prev_width`, the affected
/// footprint can **exceed the current framebuffer area** when rows are removed.
/// That is expected: it is an addressed-cell count, not a fraction of the live
/// screen. Callers that want a percentage must choose the metric whose
/// denominator matches: `exact_changed_cell_count() / union_area` for a state
/// delta, and the raw affected count (not a fraction) when rows may be removed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SurfaceDiff {
    pub patches: Vec<RowPatch>,
    pub prev_height: u16,
    pub next_height: u16,
    /// Number of rows below next_height that existed in previous surface and must be cleared.
    pub rows_to_clear: u16,
    /// Width of the previous surface (used to size cleared trailing rows).
    pub prev_width: u16,
    /// Width of the next surface (erase-to-EOL uses the larger of both widths).
    pub next_width: u16,
    /// Exact number of cells whose rendered state differs from `prev`, treating
    /// absent rows/cells as blank. This is a true semantic delta.
    pub exact_changed: usize,
    /// Exact semantic delta retained as `(row, start_column, length)` spans.
    ///
    /// `compute_diff` produces nonempty, disjoint spans in row-major order.
    /// This includes erasures and removed cells that cannot be recovered from
    /// output patches. Like `exact_changed`, this metadata describes the
    /// original comparison and must be kept consistent if manually constructed.
    pub exact_changed_spans: Vec<(u16, u16, u16)>,
}

impl SurfaceDiff {
    pub fn is_empty(&self) -> bool {
        self.patches.is_empty() && self.rows_to_clear == 0
    }

    /// Cells covered by explicit `CellRun` writes only.
    ///
    /// This is *not* the logical footprint: it excludes erase-to-EOL regions and
    /// cleared trailing rows. Prefer [`SurfaceDiff::logical_dirty_count`] (a.k.a.
    /// [`SurfaceDiff::affected_cell_count`]) for damage heatmaps.
    pub fn explicit_dirty_count(&self) -> usize {
        self.patches
            .iter()
            .map(|p| p.runs.iter().map(|r| r.cells.len()).sum::<usize>())
            .sum()
    }

    /// Backwards-compatible alias for [`SurfaceDiff::explicit_dirty_count`].
    ///
    /// Kept so callers that genuinely mean "explicit run cells" keep working.
    pub fn total_dirty_cells(&self) -> usize {
        self.explicit_dirty_count()
    }

    /// Logical affected footprint: cells *addressed* by the update semantics,
    /// including erase-to-EOL regions and cleared trailing rows.
    ///
    /// Conservative superset of the exact state delta. May exceed the current
    /// framebuffer area when rows are removed — see the type docs.
    pub fn logical_dirty_count(&self) -> usize {
        let mut n = self.explicit_dirty_count();
        for p in &self.patches {
            if let Some(x) = p.erase_eol_from {
                n += self.prev_width.max(self.next_width).saturating_sub(x) as usize;
            }
        }
        n += self.rows_to_clear as usize * self.prev_width as usize;
        n
    }

    /// Honest alias for [`SurfaceDiff::logical_dirty_count`]: the affected
    /// footprint, not an exact state delta.
    pub fn affected_cell_count(&self) -> usize {
        self.logical_dirty_count()
    }

    /// The exact number of cells whose rendered state actually changed.
    ///
    /// Unlike [`SurfaceDiff::logical_dirty_count`], this never counts a cell that
    /// was already blank. Use this for exact percentages:
    /// `exact_changed_cell_count() / union_area` (previous and next extents).
    pub fn exact_changed_cell_count(&self) -> usize {
        self.exact_changed
    }

    /// Coordinates of every cell whose rendered state actually changed.
    ///
    /// Row-major and unique for diffs returned by [`compute_diff`]; excludes
    /// already-blank cells addressed by a `CSI K`. Removed columns/rows remain
    /// in the previous coordinate space. Wide glyph continuations count as
    /// occupied cells, including when an entire row is added or removed.
    pub fn exact_changed_cells(&self) -> Vec<(u16, u16)> {
        let mut out = Vec::with_capacity(self.exact_changed);
        for &(y, x, length) in &self.exact_changed_spans {
            for cx in x..x.saturating_add(length) {
                out.push((cx, y));
            }
        }
        out
    }

    /// Explicit run-cell coordinates only (row-major).
    pub fn explicit_dirty_cells(&self) -> Vec<(u16, u16)> {
        let mut out = Vec::with_capacity(self.explicit_dirty_count());
        for patch in &self.patches {
            for run in &patch.runs {
                for i in 0..run.cells.len() {
                    out.push((run.x.saturating_add(i as u16), patch.y));
                }
            }
        }
        out
    }

    /// Coordinates of every logically-affected cell this frame, row-major.
    ///
    /// Intended for debug overlays (damage maps). Includes erase-to-EOL regions
    /// and cleared trailing rows, so a shrinking line reports the cells it
    /// actually addressed. Cheap; only call when needed.
    pub fn logical_dirty_cells(&self) -> Vec<(u16, u16)> {
        let mut out = Vec::with_capacity(self.logical_dirty_count());
        for patch in &self.patches {
            for run in &patch.runs {
                for i in 0..run.cells.len() {
                    out.push((run.x.saturating_add(i as u16), patch.y));
                }
            }
            if let Some(x) = patch.erase_eol_from {
                for cx in x..self.prev_width.max(self.next_width) {
                    out.push((cx, patch.y));
                }
            }
        }
        for ry in 0..self.rows_to_clear {
            let y = self.next_height.saturating_add(ry);
            for cx in 0..self.prev_width {
                out.push((cx, y));
            }
        }
        out
    }
}

/// True when a cell renders identically to an absent/blank cell.
fn cell_is_blank(c: &Cell) -> bool {
    let g = c.glyph.grapheme.as_str();
    !c.is_continuation && (g.is_empty() || g == " ") && c.style.is_default()
}

/// Compares `prev` (if any) and `next` surfaces, producing a minimal diff.
pub fn compute_diff(prev: Option<&Surface>, next: &Surface) -> SurfaceDiff {
    let mut patches = Vec::new();
    let prev_height = prev.map(|p| p.height).unwrap_or(0);
    let next_height = next.height;
    let mut exact_changed = 0usize;
    let mut exact_changed_spans = Vec::new();

    // Diff row by row for all rows in `next`
    for y in 0..next_height {
        let prev_row = prev.and_then(|p| {
            if y < p.height {
                let start = (y as usize) * (p.width as usize);
                let end = start + (p.width as usize);
                Some(&p.cells[start..end])
            } else {
                None
            }
        });

        let next_start = (y as usize) * (next.width as usize);
        let next_end = next_start + (next.width as usize);
        let next_row = &next.cells[next_start..next_end];

        // Retain exact spans independently of wire patches: CSI K deliberately
        // collapses changed and already-blank cells into one output operation.
        record_exact_row(
            y,
            prev_row,
            next_row,
            &mut exact_changed_spans,
            &mut exact_changed,
        );

        let patch = diff_row(y, prev_row, next_row, next.width);
        if let Some(p) = patch {
            patches.push(p);
        }
    }

    let rows_to_clear = prev_height.saturating_sub(next_height);
    // Removed rows: count the non-blank cells that disappear.
    if let Some(p) = prev {
        for y in next_height..prev_height {
            let start = (y as usize) * (p.width as usize);
            let end = start + (p.width as usize);
            record_exact_row(
                y,
                Some(&p.cells[start..end]),
                &[],
                &mut exact_changed_spans,
                &mut exact_changed,
            );
        }
    }

    SurfaceDiff {
        patches,
        prev_height,
        next_height,
        rows_to_clear,
        prev_width: prev.map(|p| p.width).unwrap_or(0),
        next_width: next.width,
        exact_changed,
        exact_changed_spans,
    }
}

// Compare the union of row extents, treating absent cells as default blanks.
// Wide continuations occupy a cell even when their stored grapheme is empty.
fn record_exact_row(
    y: u16,
    prev: Option<&[Cell]>,
    next: &[Cell],
    spans: &mut Vec<(u16, u16, u16)>,
    count: &mut usize,
) {
    let prev = prev.unwrap_or(&[]);
    let width = prev.len().max(next.len());
    let mut start = None;
    for x in 0..width {
        let changed = match (prev.get(x), next.get(x)) {
            (Some(a), Some(b)) => a != b,
            (Some(c), None) | (None, Some(c)) => !cell_is_blank(c),
            (None, None) => false,
        };
        if changed {
            *count += 1;
            start.get_or_insert(x);
        } else if let Some(first) = start.take() {
            spans.push((y, first as u16, (x - first) as u16));
        }
    }
    if let Some(first) = start {
        spans.push((y, first as u16, (width - first) as u16));
    }
}

fn diff_row(y: u16, prev: Option<&[Cell]>, next: &[Cell], _width: u16) -> Option<RowPatch> {
    let mut runs: Vec<CellRun> = Vec::new();
    let mut current_run: Option<CellRun> = None;
    let mut erase_eol_from: Option<u16> = None;

    match prev {
        None => {
            // No previous surface: entire row is new.
            // Find rightmost non-default cell to avoid writing trailing blank spaces
            let last_non_empty = next.iter().rposition(|c| !cell_is_blank(c));

            if let Some(last_idx) = last_non_empty {
                let count = last_idx + 1;
                runs.push(CellRun {
                    x: 0,
                    cells: next[..count].to_vec(),
                });
            }
        }
        Some(prev_cells) => {
            let common_len = prev_cells.len().min(next.len());

            // Check if trailing content in prev was erased in next
            // e.g. prev had "hello world" and next has "hello"
            let prev_last_non_space = prev_cells
                .iter()
                .rposition(|c| c.glyph.grapheme.as_str() != " " || !c.style.is_default());
            let next_last_non_space = next
                .iter()
                .rposition(|c| c.glyph.grapheme.as_str() != " " || !c.style.is_default());

            // If prev had content further right than next, we can use CSI K (erase to EOL)
            if let Some(prev_last) = prev_last_non_space {
                let next_last = next_last_non_space.map(|idx| idx as isize).unwrap_or(-1);
                if (prev_last as isize) > next_last {
                    // We can erase from next_last + 1
                    let erase_x = (next_last + 1) as u16;
                    erase_eol_from = Some(erase_x);
                }
            }

            // Cell by cell diff up to common length (or up to erase_eol_from if erasing to EOL)
            let limit_check = match erase_eol_from {
                Some(erase_x) => common_len.min(erase_x as usize),
                None => common_len,
            };

            for x in 0..limit_check {
                let cell_changed = prev_cells[x] != next[x];

                if cell_changed {
                    if let Some(ref mut run) = current_run {
                        run.cells.push(next[x].clone());
                    } else {
                        current_run = Some(CellRun {
                            x: x as u16,
                            cells: vec![next[x].clone()],
                        });
                    }
                } else if let Some(run) = current_run.take() {
                    runs.push(run);
                }
            }

            if let Some(run) = current_run.take() {
                runs.push(run);
            }

            // If next is wider than prev, the extra cells are new
            if next.len() > prev_cells.len() {
                let extra_start = prev_cells.len();
                let extra_cells = &next[extra_start..];
                let last_non_space = extra_cells
                    .iter()
                    .rposition(|c| c.glyph.grapheme.as_str() != " " || !c.style.is_default());

                if let Some(extra_last) = last_non_space {
                    runs.push(CellRun {
                        x: extra_start as u16,
                        cells: extra_cells[..=extra_last].to_vec(),
                    });
                }
            }
        }
    }

    if runs.is_empty() && erase_eol_from.is_none() {
        None
    } else {
        Some(RowPatch {
            y,
            runs,
            erase_eol_from,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Style;

    #[test]
    fn test_identical_surfaces_empty_diff() {
        let mut s1 = Surface::new(10, 2);
        s1.print_str(0, 0, "hello", Style::default(), None);
        let s2 = s1.clone();

        let diff = compute_diff(Some(&s1), &s2);
        assert!(diff.is_empty());
        assert_eq!(diff.patches.len(), 0);
        assert_eq!(diff.rows_to_clear, 0);
    }

    #[test]
    fn test_single_cell_diff() {
        let mut s1 = Surface::new(10, 2);
        s1.print_str(0, 0, "hello", Style::default(), None);

        let mut s2 = s1.clone();
        s2.print_str(1, 0, "a", Style::default(), None); // "hallo"

        let diff = compute_diff(Some(&s1), &s2);
        assert!(!diff.is_empty());
        assert_eq!(diff.patches.len(), 1);
        let patch = &diff.patches[0];
        assert_eq!(patch.y, 0);
        assert_eq!(patch.runs.len(), 1);
        assert_eq!(patch.runs[0].x, 1);
        assert_eq!(patch.runs[0].cells.len(), 1);
        assert_eq!(patch.runs[0].cells[0].glyph.grapheme.as_str(), "a");
    }

    #[test]
    fn test_erase_to_eol_when_text_shrinks() {
        let mut s1 = Surface::new(20, 2);
        s1.print_str(0, 0, "hello world 12345", Style::default(), None);

        let mut s2 = Surface::new(20, 2);
        s2.print_str(0, 0, "hello", Style::default(), None);

        let diff = compute_diff(Some(&s1), &s2);
        assert_eq!(diff.patches.len(), 1);
        let patch = &diff.patches[0];
        assert_eq!(patch.erase_eol_from, Some(5));
    }

    #[test]
    fn test_height_shrink_clears_rows() {
        let s1 = Surface::new(10, 5);
        let s2 = Surface::new(10, 2);

        let diff = compute_diff(Some(&s1), &s2);
        assert_eq!(diff.rows_to_clear, 3);
    }

    // -----------------------------------------------------------------------
    // Logical damage accounting (erase-to-EOL + cleared rows).
    // -----------------------------------------------------------------------

    #[test]
    fn logical_damage_counts_erase_to_eol_cells() {
        let mut s1 = Surface::new(20, 1);
        s1.print_str(0, 0, "HELLO WORLD", Style::default(), None);
        let s2 = Surface::new(20, 1);

        let diff = compute_diff(Some(&s1), &s2);
        // The erased region is columns 0..20 (the whole row).
        assert_eq!(diff.patches[0].erase_eol_from, Some(0));
        // Explicit runs may be empty; the logical count must still see the erase.
        assert_eq!(diff.explicit_dirty_count(), 0);
        assert_eq!(diff.logical_dirty_count(), 20);
        assert_eq!(diff.logical_dirty_cells().len(), 20);
    }

    #[test]
    fn logical_damage_counts_shrinking_line_region() {
        let mut s1 = Surface::new(20, 1);
        s1.print_str(0, 0, "hello world 12345", Style::default(), None);
        let mut s2 = Surface::new(20, 1);
        s2.print_str(0, 0, "hello", Style::default(), None);

        let diff = compute_diff(Some(&s1), &s2);
        assert_eq!(diff.patches[0].erase_eol_from, Some(5));
        // Cells 5..20 are logically erased.
        assert_eq!(diff.logical_dirty_count(), 15);
        let cells = diff.logical_dirty_cells();
        assert!(cells.iter().all(|(_, y)| *y == 0));
        assert!(cells.iter().any(|(x, _)| *x == 5));
        assert!(cells.iter().any(|(x, _)| *x == 19));
        assert!(!cells.iter().any(|(x, _)| *x == 4));
    }

    #[test]
    fn logical_damage_counts_cleared_trailing_rows() {
        let s1 = Surface::new(10, 5);
        let s2 = Surface::new(10, 2);
        let diff = compute_diff(Some(&s1), &s2);
        // Rows 2, 3, 4 cleared, each 10 columns wide.
        assert_eq!(diff.logical_dirty_count(), 30);
        let cells = diff.logical_dirty_cells();
        assert!(cells.iter().any(|(x, y)| *x == 9 && *y == 4));
        assert!(cells.iter().all(|(_, y)| *y >= 2));
    }

    #[test]
    fn logical_damage_is_zero_for_identical_surfaces() {
        let mut s1 = Surface::new(12, 3);
        s1.print_str(0, 0, "stable", Style::default(), None);
        let s2 = s1.clone();
        let diff = compute_diff(Some(&s1), &s2);
        assert_eq!(diff.logical_dirty_count(), 0);
        assert!(diff.logical_dirty_cells().is_empty());
    }

    #[test]
    fn logical_damage_superset_of_explicit_runs() {
        let mut s1 = Surface::new(30, 2);
        s1.print_str(0, 0, "AAAA", Style::default(), None);
        s1.print_str(0, 1, "long second row of text", Style::default(), None);
        let mut s2 = Surface::new(30, 2);
        s2.print_str(0, 0, "ABBB", Style::default(), None);
        s2.print_str(0, 1, "short", Style::default(), None);
        let diff = compute_diff(Some(&s1), &s2);
        assert!(diff.logical_dirty_count() >= diff.explicit_dirty_count());
        // Explicit runs are a strict subset here (row 1 shrank to a CSI K).
        assert!(diff.logical_dirty_count() > diff.explicit_dirty_count());
    }

    // -----------------------------------------------------------------------
    // Exact semantic delta vs affected footprint.
    // -----------------------------------------------------------------------

    #[test]
    fn affected_footprint_counts_already_blank_cells_but_exact_does_not() {
        // prev: "HELLO WORLD" then blanks; next: all blanks addressed by CSI K.
        let mut s1 = Surface::new(20, 1);
        s1.print_str(0, 0, "HELLO WORLD", Style::default(), None);
        let s2 = Surface::new(20, 1);
        let diff = compute_diff(Some(&s1), &s2);
        // Affected = every addressed cell (the whole row).
        assert_eq!(diff.affected_cell_count(), 20);
        assert_eq!(diff.logical_dirty_count(), 20);
        // Exact = only the 10 non-blank glyph cells (the space between the words
        // is already blank and did not change).
        assert_eq!(diff.exact_changed_cell_count(), 10);
        assert!(diff.exact_changed_cell_count() < diff.affected_cell_count());
    }

    #[test]
    fn exact_delta_is_zero_for_identical_surfaces_and_changed_counts_differ() {
        let mut s1 = Surface::new(12, 3);
        s1.print_str(0, 0, "stable", Style::default(), None);
        let s2 = s1.clone();
        let diff = compute_diff(Some(&s1), &s2);
        assert_eq!(diff.exact_changed_cell_count(), 0);
        assert_eq!(diff.affected_cell_count(), 0);

        let mut s3 = s1.clone();
        s3.print_str(1, 0, "X", Style::default(), None);
        let d3 = compute_diff(Some(&s1), &s3);
        assert_eq!(d3.exact_changed_cell_count(), 1);
        assert_eq!(d3.affected_cell_count(), 1);
    }

    #[test]
    fn affected_footprint_may_exceed_removed_surface_area() {
        // Shrinking height addresses cleared trailing rows, so the affected count
        // can exceed the live (next) area. That is why it is shown as a count.
        let mut s1 = Surface::new(10, 5);
        for y in 0..5 {
            s1.print_str(0, y, "XXXXXXXXXX", Style::default(), None);
        }
        let s2 = Surface::new(10, 2);
        let diff = compute_diff(Some(&s1), &s2);
        let next_area = 10 * 2;
        assert!(
            diff.affected_cell_count() > next_area,
            "affected {} should exceed live area {} (removed rows)",
            diff.affected_cell_count(),
            next_area
        );
        // exact counts every non-blank cell that vanished: 20 on the surviving
        // blanked rows + 30 on the removed rows.
        assert_eq!(diff.exact_changed_cell_count(), 50);
        // A true state delta never exceeds the union area (prev ∪ next = 50 here).
        assert!(diff.exact_changed_cell_count() <= 10 * 5);
    }

    #[test]
    fn exact_delta_matches_cell_by_cell_comparison() {
        // Property: the count equals a direct prev/next cell comparison, treating
        // absent cells as blank.
        let mut prev = Surface::new(8, 3);
        prev.print_str(0, 0, "hello", Style::default(), None);
        prev.print_str(1, 2, "xyz", Style::default(), None);
        let mut next = Surface::new(8, 3);
        next.print_str(0, 0, "heLLo", Style::default(), None);

        let diff = compute_diff(Some(&prev), &next);
        let mut direct = 0usize;
        for y in 0..3 {
            for x in 0..8 {
                let p = prev.get(x, y).unwrap();
                let n = next.get(x, y).unwrap();
                let blank = |c: &Cell| {
                    let g = c.glyph.grapheme.as_str();
                    (g.is_empty() || g == " ") && c.style.is_default()
                };
                let changed = match (blank(p), blank(n)) {
                    (true, true) => false,
                    (true, false) => true,
                    (false, true) => true,
                    (false, false) => p != n,
                };
                if changed {
                    direct += 1;
                }
            }
        }
        assert_eq!(diff.exact_changed_cell_count(), direct);
    }
}
