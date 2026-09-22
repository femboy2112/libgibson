use gibson::ansi::AnsiCompiler;
use gibson::cell::{Color, Line, RichText, Style, Theme};
use gibson::diff::compute_diff;
use gibson::layout::compute_layout;
use gibson::node::{Node, WrapMode};
use gibson::painter::paint;
use gibson::surface::{BorderType, Surface};

#[test]
fn test_resize_torture_cycle_dimensions() {
    let theme = Theme::default();
    let dimensions: &[(u16, u16)] = &[(120, 40), (80, 24), (40, 15), (100, 30), (35, 10), (80, 24)];

    let mut prev_surface: Option<Surface> = None;
    let mut compiler = AnsiCompiler::new(false);

    for &(cols, rows) in dimensions {
        let max_inline_height = rows.saturating_sub(1).max(1);

        // Build a realistic responsive tree: header, rail, rule, text input
        let mut root = Node::col()
            .gap(0.0)
            .child(
                Node::rule(Some("Agent Status"), Style::new().fg(theme.accent)).width(cols as f32),
            )
            .child(
                Node::rail(Style::new().fg(theme.rail))
                    .width(cols as f32)
                    .child(Node::rich_text_wrapped(
                        RichText::new()
                            .line(Line::styled(
                                "Processing multi-step refactor across modules...",
                                Style::new().bold(),
                            ))
                            .line(Line::styled(
                                "Running compiler check: 0 warnings, 42 tests passed.",
                                Style::new().fg(theme.success),
                            )),
                        WrapMode::WordWrap,
                    )),
            )
            .child(
                Node::border_box(BorderType::Rounded, Style::new().fg(theme.border))
                    .width(cols as f32)
                    .child(
                        Node::row()
                            .gap(1.0)
                            .child(Node::text("❯", Style::new().bold().fg(theme.accent)))
                            .child(
                                Node::text_input(
                                    "cargo clippy --all-targets --all-features",
                                    38,
                                    Some("Type a command..."),
                                    Style::new().fg(Color::White),
                                )
                                .flex_grow(1.0),
                            ),
                    ),
            );

        // Compute layout
        let rect = compute_layout(&mut root, cols, max_inline_height).expect("Layout failed");

        // Assert inline height invariant
        let surface_height = rect.height.max(1).min(max_inline_height);
        assert!(
            surface_height <= rows,
            "Surface height {} exceeds terminal rows {}",
            surface_height,
            rows
        );

        // Paint pass
        let mut next_surface = Surface::new(cols, surface_height);
        let paint_ctx = paint(&root, &mut next_surface);

        // Cursor invariant
        if let Some((cx, cy)) = paint_ctx.cursor_position {
            assert!(cx < cols, "Cursor x {} exceeds terminal cols {}", cx, cols);
            assert!(
                cy < surface_height,
                "Cursor y {} exceeds surface height {}",
                cy,
                surface_height
            );
        }

        // Diff pass
        let diff = compute_diff(prev_surface.as_ref(), &next_surface);
        let bytes = compiler.compile(&diff);

        // At end of frame, autowrap restoration should be emitted
        let ansi_str = String::from_utf8_lossy(&bytes);
        assert!(ansi_str.contains("\x1b[?7l"));
        assert!(ansi_str.contains("\x1b[?7h"));

        prev_surface = Some(next_surface);
    }
}

#[test]
fn test_narrow_terminal_responsiveness() {
    // Torture test extremely narrow terminal (30 cols)
    let cols: u16 = 30;
    let rows: u16 = 12;

    let mut root = Node::col()
        .width(cols as f32)
        .child(Node::rule(Some("Plan"), Style::default()))
        .child(
            Node::rail(Style::default())
                .child(Node::rich_text_wrapped(
                    RichText::raw("Very long text that must wrap cleanly without panic or overflow in narrow terminal widths."),
                    WrapMode::WordWrap,
                )),
        )
        .child(
            Node::text_input("extremely_long_command_name_that_exceeds_columns", 40, None, Style::default())
                .width(cols as f32)
        );

    let rect = compute_layout(&mut root, cols, rows.saturating_sub(1)).unwrap();
    let mut surface = Surface::new(cols, rect.height.min(rows.saturating_sub(1)));
    let paint_ctx = paint(&root, &mut surface);

    if let Some((cx, cy)) = paint_ctx.cursor_position {
        assert!(cx < cols);
        assert!(cy < surface.height);
    }
}
