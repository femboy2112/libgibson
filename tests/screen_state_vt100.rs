use gibson::ansi::AnsiCompiler;
use gibson::cell::{Color, Style};
use gibson::diff::compute_diff;
use gibson::renderer::strip_ansi_escapes;
use gibson::surface::Surface;

#[test]
fn test_vt100_screen_state_diff_and_cursor() {
    let mut parser = vt100::Parser::new(24, 80, 0);

    let mut s1 = Surface::new(80, 5);
    s1.print_str(0, 0, "Initial Line", Style::default(), None);

    let mut compiler = AnsiCompiler::new();
    let diff1 = compute_diff(None, &s1);
    let bytes1 = compiler.compile(&diff1);

    parser.process(&bytes1);
    let screen = parser.screen();
    let rows = screen.rows(0, 80).collect::<Vec<_>>();
    assert!(rows[0].starts_with("Initial Line"));

    // Now update: single cell update at (0, 0)
    let mut s2 = s1.clone();
    s2.print_str(0, 0, "Updated", Style::default(), None);

    let diff2 = compute_diff(Some(&s1), &s2);
    let bytes2 = compiler.compile(&diff2);

    parser.process(&bytes2);
    let screen2 = parser.screen();
    let rows2 = screen2.rows(0, 80).collect::<Vec<_>>();
    assert!(rows2[0].starts_with("Updated Line"));
}

#[test]
fn test_vt100_autowrap_protection_at_right_margin() {
    let mut parser = vt100::Parser::new(24, 80, 0);
    let mut compiler = AnsiCompiler::new();

    // Create a surface of width 80 (exact terminal width)
    let mut s = Surface::new(80, 2);
    // Fill row 0 completely to column 79
    let full_80_col: String = "X".repeat(80);
    s.print_str(0, 0, &full_80_col, Style::default(), None);
    s.print_str(0, 1, "Second Row", Style::default(), None);

    let diff = compute_diff(None, &s);
    let bytes = compiler.compile(&diff);

    parser.process(&bytes);
    let screen = parser.screen();
    let rows = screen.rows(0, 80).collect::<Vec<_>>();

    // Verify row 0 has all 80 'X's
    assert_eq!(&rows[0][..80], &full_80_col);

    // Verify row 1 has "Second Row" and was not pushed down or scrolled by autowrap!
    assert!(rows[1].starts_with("Second Row"));
}

#[test]
fn test_vt100_cjk_emoji_placement() {
    let mut parser = vt100::Parser::new(24, 80, 0);
    let mut compiler = AnsiCompiler::new();

    let mut s = Surface::new(80, 2);
    s.print_str(
        0,
        0,
        "🦀 Rust 你好世界",
        Style::new().fg(Color::Green),
        None,
    );

    let diff = compute_diff(None, &s);
    let bytes = compiler.compile(&diff);

    parser.process(&bytes);
    let screen = parser.screen();
    let rows = screen.rows(0, 80).collect::<Vec<_>>();

    // Verify row contents in virtual terminal
    assert!(rows[0].contains("Rust"));
    assert!(rows[0].contains("你好世界"));
}

#[test]
fn test_vt100_strip_ansi_escapes_clean() {
    let raw = "\x1b[1;34m● LibGibson Engine\x1b[0m session started with \x1b[32mcolors\x1b[0m.";
    let stripped = strip_ansi_escapes(raw);
    assert_eq!(stripped, "● LibGibson Engine session started with colors.");
    assert!(!stripped.contains('\x1b'));
    assert!(!stripped.contains('['));
}
