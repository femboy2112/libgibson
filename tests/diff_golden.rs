use gibson::ansi::AnsiCompiler;
use gibson::cell::{Color, Style};
use gibson::diff::compute_diff;
use gibson::surface::Surface;

#[test]
fn test_diff_identical_frames_zero_bytes() {
    let mut s1 = Surface::new(40, 5);
    s1.print_str(0, 0, "Consistent static interface", Style::default(), None);
    let s2 = s1.clone();

    let diff = compute_diff(Some(&s1), &s2);
    assert!(diff.is_empty(), "Diff of identical surfaces must be empty");

    let mut compiler = AnsiCompiler::new();
    let bytes = compiler.compile(&diff);
    assert_eq!(bytes.len(), 0, "Identical frames must emit 0 ANSI bytes");
}

#[test]
fn test_diff_single_cell_bounded_patch() {
    let mut s1 = Surface::new(80, 1);
    s1.print_str(0, 0, "The status is [IDLE]", Style::default(), None);

    let mut s2 = s1.clone();
    // Mutate only 4 cells: IDLE -> BUSY
    s2.print_str(15, 0, "BUSY", Style::new().fg(Color::Yellow), None);

    let diff = compute_diff(Some(&s1), &s2);
    assert_eq!(diff.patches.len(), 1);
    assert_eq!(diff.patches[0].runs.len(), 1);
    assert_eq!(diff.patches[0].runs[0].x, 15);
    assert_eq!(diff.patches[0].runs[0].cells.len(), 4);

    let mut compiler = AnsiCompiler::new();
    let bytes = compiler.compile(&diff);
    let ansi_str = String::from_utf8_lossy(&bytes);

    println!("Single cell change emitted ANSI: {:?}", ansi_str);
    assert!(ansi_str.contains("BUSY"));
    // Bounded patch: changing 4 cells in an 80-column line must not emit the full 80 columns!
    assert!(
        bytes.len() < 35,
        "Patch size was {} bytes (expected < 35)",
        bytes.len()
    );
}

#[test]
fn test_diff_style_only_change() {
    let mut s1 = Surface::new(20, 1);
    s1.print_str(0, 0, "ALERT", Style::default(), None);

    let mut s2 = Surface::new(20, 1);
    s2.print_str(0, 0, "ALERT", Style::new().fg(Color::Red).bold(), None);

    let diff = compute_diff(Some(&s1), &s2);
    assert_eq!(diff.total_dirty_cells(), 5);

    let mut compiler = AnsiCompiler::new();
    let bytes = compiler.compile(&diff);
    let ansi_str = String::from_utf8_lossy(&bytes);

    println!("Style change emitted ANSI: {:?}", ansi_str);
    assert!(ansi_str.contains("\x1b[1;31m")); // Bold + Red
    assert!(ansi_str.contains("ALERT"));
}

#[test]
fn test_diff_line_shrinks_emits_erase_to_eol() {
    let mut s1 = Surface::new(40, 1);
    s1.print_str(
        0,
        0,
        "Long previous status message from server",
        Style::default(),
        None,
    );

    let mut s2 = Surface::new(40, 1);
    s2.print_str(0, 0, "Short", Style::default(), None);

    let diff = compute_diff(Some(&s1), &s2);
    assert_eq!(diff.patches.len(), 1);
    assert_eq!(diff.patches[0].erase_eol_from, Some(5));

    let mut compiler = AnsiCompiler::new();
    let bytes = compiler.compile(&diff);
    let ansi_str = String::from_utf8_lossy(&bytes);

    println!("Shrink line emitted ANSI: {:?}", ansi_str);
    assert!(
        ansi_str.contains("\x1b[K"),
        "Must emit CSI K (erase to end of line)"
    );
}

#[test]
fn test_diff_wide_character_overwrite() {
    let mut s1 = Surface::new(20, 1);
    s1.print_str(0, 0, "你好世界", Style::default(), None);

    // Overwrite the second character "好" (which occupied cols 2 and 3) with "AB"
    let mut s2 = s1.clone();
    s2.print_str(2, 0, "AB", Style::default(), None);

    // Check surface invariants
    assert_eq!(s2.get(0, 0).unwrap().glyph.grapheme.as_str(), "你");
    assert_eq!(s2.get(2, 0).unwrap().glyph.grapheme.as_str(), "A");
    assert_eq!(s2.get(3, 0).unwrap().glyph.grapheme.as_str(), "B");
    assert!(!s2.get(3, 0).unwrap().is_continuation);

    let diff = compute_diff(Some(&s1), &s2);
    let mut compiler = AnsiCompiler::new();
    let bytes = compiler.compile(&diff);
    let ansi_str = String::from_utf8_lossy(&bytes);

    println!("Wide overwrite emitted ANSI: {:?}", ansi_str);
    assert!(ansi_str.contains("AB"));
}

#[test]
fn test_diff_previous_larger_than_next_clears_rows() {
    let s1 = Surface::new(40, 5);
    let s2 = Surface::new(40, 2);

    let diff = compute_diff(Some(&s1), &s2);
    assert_eq!(diff.rows_to_clear, 3);

    let mut compiler = AnsiCompiler::new();
    let bytes = compiler.compile(&diff);
    let ansi_str = String::from_utf8_lossy(&bytes);

    println!("Rows cleared emitted ANSI: {:?}", ansi_str);
    // Must contain 3 CSI K erasures for the deleted rows
    let csi_k_count = ansi_str.matches("\x1b[K").count();
    assert_eq!(csi_k_count, 3, "Expected 3 row clearing CSI K escapes");
}
