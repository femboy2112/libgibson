use gibson::{compute_diff, Cell, Color, Rect, Style, Surface};

#[test]
fn rectangle_endpoints_saturate_consistently() {
    let max = u16::MAX;
    let rect = Rect::new(max - 2, max - 3, 8, 9);
    assert!(rect.contains(max - 1, max - 1));
    assert!(!rect.contains(max, max - 1));
    assert!(!rect.contains(max - 1, max));
    assert_eq!(rect.intersection(&rect), Rect::new(max - 2, max - 3, 2, 3));
    assert_eq!(rect.shrink(0), Rect::new(max - 2, max - 3, 2, 3));
    let terminal_extent = Rect::new(0, 0, max, max);
    assert_eq!(terminal_extent.intersection(&rect), rect.shrink(0));
    assert!(Rect::new(max, 0, 1, 1).is_empty());
    assert!(Rect::new(0, max, 1, 1).is_empty());
    assert!(Rect::new(max, 0, 1, 1)
        .intersection(&Rect::new(0, 0, 1, 1))
        .is_empty());
}

#[test]
fn shrink_is_safe_for_large_amounts_and_clipped_origins() {
    for amount in [0, 1, 32_767, 32_768, u16::MAX] {
        let tiny = Rect::new(2, 3, 1, 1).shrink(amount);
        assert_eq!(
            tiny,
            Rect::new(2, 3, u16::from(amount == 0), u16::from(amount == 0))
        );
    }
    assert_eq!(Rect::new(3, 4, 12, 10).shrink(2), Rect::new(5, 6, 8, 6));
    assert_eq!(
        Rect::new(65_530, 65_530, 100, 100).shrink(1),
        Rect::new(65_531, 65_531, 3, 3)
    );
    assert_eq!(
        Rect::new(65_534, 65_534, 100, 100).shrink(1),
        Rect::new(65_534, 65_534, 0, 0)
    );
}

#[test]
fn intersections_match_a_widened_independent_endpoint_oracle() {
    let values = [0, 1, 2, 32_768, 65_533, 65_534, 65_535];
    for &x in &values {
        for &width in &values {
            let a = Rect::new(x, x, width, width);
            for &other_x in &values {
                let b = Rect::new(other_x, 1, 7, 65_535);
                let intersection = a.intersection(&b);
                assert_eq!(intersection, b.intersection(&a));
                for &coordinate in &values {
                    let oracle = |r: Rect| {
                        let end_x = (u32::from(r.x) + u32::from(r.width)).min(65_535);
                        let end_y = (u32::from(r.y) + u32::from(r.height)).min(65_535);
                        coordinate >= r.x
                            && u32::from(coordinate) < end_x
                            && coordinate >= r.y
                            && u32::from(coordinate) < end_y
                    };
                    assert_eq!(
                        intersection.contains(coordinate, coordinate),
                        oracle(a) && oracle(b)
                    );
                }
            }
        }
    }
}

fn direct_delta(prev: Option<&Surface>, next: &Surface) -> Vec<(u16, u16)> {
    let blank = Cell::default();
    let width = next.width.max(prev.map_or(0, |s| s.width));
    let height = next.height.max(prev.map_or(0, |s| s.height));
    let mut changed = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let a = prev.and_then(|s| s.get(x, y)).unwrap_or(&blank);
            let b = next.get(x, y).unwrap_or(&blank);
            if a != b {
                changed.push((x, y));
            }
        }
    }
    changed
}

fn assert_exact(prev: Option<&Surface>, next: &Surface) {
    let diff = compute_diff(prev, next);
    let expected = direct_delta(prev, next);
    assert_eq!(diff.exact_changed_cell_count(), expected.len());
    assert_eq!(diff.exact_changed_cells(), expected);
    assert!(diff.exact_changed_cell_count() <= diff.affected_cell_count());
    let actual = diff.exact_changed_cells();
    assert!(actual
        .windows(2)
        .all(|pair| (pair[0].1, pair[0].0) < (pair[1].1, pair[1].0)));
}

#[test]
fn erased_cells_are_exact_but_blank_padding_is_only_affected() {
    let mut before = Surface::new(8, 1);
    before.print_str(0, 0, "hello", Style::default(), None);
    let after = Surface::new(8, 1);
    let diff = compute_diff(Some(&before), &after);
    assert_eq!(
        diff.exact_changed_cells(),
        vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)]
    );
    assert_eq!(diff.explicit_dirty_count(), 0);
    assert_eq!(diff.affected_cell_count(), 8);
    assert_exact(Some(&before), &after);
}

#[test]
fn removed_columns_and_rows_remain_in_previous_coordinate_space() {
    let mut before = Surface::new(9, 3);
    before.print_str(0, 0, "abc   xyz", Style::default(), None);
    before.print_str(2, 1, "old", Style::default(), None);
    before.print_str(1, 2, "last", Style::default(), None);
    let mut after = Surface::new(3, 1);
    after.print_str(0, 0, "abc", Style::default(), None);
    let diff = compute_diff(Some(&before), &after);
    assert_eq!(diff.patches[0].erase_eol_from, Some(3));
    assert_eq!(diff.affected_cell_count(), 6 + 18);
    assert!(diff.logical_dirty_cells().contains(&(8, 0)));
    assert_exact(Some(&before), &after);
    assert_exact(Some(&after), &before);
}

#[test]
fn wide_glyphs_count_both_cells_when_rows_appear_or_disappear() {
    let mut wide = Surface::new(8, 2);
    wide.print_str(2, 0, "界", Style::default(), None);
    wide.print_str(0, 1, "🌍", Style::new().fg(Color::Cyan), None);
    assert_exact(None, &wide);
    // Retaining continuation coordinates must not emit a second glyph or
    // change cursor movement: the ANSI compiler owns wide-glyph advancement.
    let complete = compute_diff(None, &wide);
    let mut lead_only = complete.clone();
    for patch in &mut lead_only.patches {
        for run in &mut patch.runs {
            while run.cells.last().is_some_and(|cell| cell.is_continuation) {
                run.cells.pop();
            }
        }
    }
    assert_eq!(
        gibson::ansi::AnsiCompiler::new().compile(&complete),
        gibson::ansi::AnsiCompiler::new().compile(&lead_only),
    );
    assert_exact(Some(&wide), &Surface::new(0, 0));
    assert_exact(Some(&wide), &Surface::new(8, 2));
    let mut changed = wide.clone();
    changed.print_str(2, 0, "a", Style::default(), None);
    assert_exact(Some(&wide), &changed);
}

#[test]
fn first_frame_blank_gaps_and_identical_frames_have_exact_coordinates() {
    let mut frame = Surface::new(9, 2);
    frame.print_str(2, 0, "a b", Style::default(), None);
    frame.print_str(3, 1, " ", Style::new().bg(Color::Blue), None);
    assert_exact(None, &frame);
    assert_exact(Some(&frame), &frame);
    let diff = compute_diff(Some(&frame), &frame);
    assert!(diff.is_empty());
    assert_eq!(diff.affected_cell_count(), 0);
    assert!(diff.exact_changed_spans.is_empty());
    assert!(gibson::ansi::AnsiCompiler::new().compile(&diff).is_empty());
}

#[test]
fn changing_dimensions_match_direct_comparison_in_both_directions() {
    for width in 0..10 {
        for height in 0..4 {
            let mut a = Surface::new(width, height);
            for y in 0..height {
                a.print_str(0, y, "ab 界 xy", Style::default(), None);
            }
            for next_width in 0..10 {
                for next_height in 0..4 {
                    let mut b = Surface::new(next_width, next_height);
                    for y in 0..next_height {
                        b.print_str(1, y, "a 界  z", Style::default(), None);
                    }
                    assert_exact(Some(&a), &b);
                }
            }
        }
    }
}
