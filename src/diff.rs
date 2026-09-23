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
/// Two distinct notions of "damage" are tracked here and must not be collapsed:
///
/// * **Logical damage** ([`SurfaceDiff::logical_dirty_count`],
///   [`SurfaceDiff::logical_dirty_cells`]) — every *visible terminal cell* whose
///   state changes this frame. It is the union of explicit changed runs, the
///   region logically erased by an erase-to-EOL (`CSI K`), and any trailing rows
///   that must be cleared. This is what a damage heatmap or a "dirty %" should
///   show.
/// * **Wire cost** — the bytes actually emitted, measured by the renderer, not
///   here. A single `CSI K` can logically clear dozens of cells while costing a
///   handful of bytes.
///
/// The old `total_dirty_cells`/`dirty_cells` pair counted only explicit runs and
/// was therefore systematically optimistic for shrinking/disappearing content.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SurfaceDiff {
    pub patches: Vec<RowPatch>,
    pub prev_height: u16,
    pub next_height: u16,
    /// Number of rows below next_height that existed in previous surface and must be cleared.
    pub rows_to_clear: u16,
    /// Width of the previous surface (used to size cleared trailing rows).
    pub prev_width: u16,
    /// Width of the next surface (used to size erase-to-EOL regions).
    pub next_width: u16,
}

impl SurfaceDiff {
    pub fn is_empty(&self) -> bool {
        self.patches.is_empty() && self.rows_to_clear == 0
    }

    /// Cells covered by explicit `CellRun` writes only.
    ///
    /// This is *not* logical damage: it excludes erase-to-EOL regions and
    /// cleared trailing rows. Prefer [`SurfaceDiff::logical_dirty_count`] for
    /// heatmaps and percentages.
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

    /// Logical damage: the number of visible cells whose terminal state changes,
    /// including erase-to-EOL regions and cleared trailing rows.
    pub fn logical_dirty_count(&self) -> usize {
        let mut n = self.explicit_dirty_count();
        for p in &self.patches {
            if let Some(x) = p.erase_eol_from {
                n += self.next_width.saturating_sub(x) as usize;
            }
        }
        n += self.rows_to_clear as usize * self.prev_width as usize;
        n
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

    /// Coordinates of every logically-dirty cell this frame, row-major.
    ///
    /// Intended for debug overlays (damage maps) and "dirty cell %" readouts.
    /// Includes erase-to-EOL regions and cleared trailing rows, so a shrinking
    /// line reports the cells it actually erased, not zero. Cheap; only call
    /// when needed.
    pub fn logical_dirty_cells(&self) -> Vec<(u16, u16)> {
        let mut out = Vec::with_capacity(self.logical_dirty_count());
        for patch in &self.patches {
            for run in &patch.runs {
                for i in 0..run.cells.len() {
                    out.push((run.x.saturating_add(i as u16), patch.y));
                }
            }
            if let Some(x) = patch.erase_eol_from {
                for cx in x..self.next_width {
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

/// Compares `prev` (if any) and `next` surfaces, producing a minimal diff.
pub fn compute_diff(prev: Option<&Surface>, next: &Surface) -> SurfaceDiff {
    let mut patches = Vec::new();
    let prev_height = prev.map(|p| p.height).unwrap_or(0);
    let next_height = next.height;

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

        let patch = diff_row(y, prev_row, next_row, next.width);
        if let Some(p) = patch {
            patches.push(p);
        }
    }

    let rows_to_clear = prev_height.saturating_sub(next_height);

    SurfaceDiff {
        patches,
        prev_height,
        next_height,
        rows_to_clear,
        prev_width: prev.map(|p| p.width).unwrap_or(0),
        next_width: next.width,
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
            let last_non_empty = next.iter().rposition(|c| {
                !c.glyph.is_empty() && c.glyph.grapheme.as_str() != " " || !c.style.is_default()
            });

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
}
