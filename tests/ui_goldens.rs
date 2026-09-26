//! A small structural golden set plus capability/size cross-checks. Regenerate
//! intentionally with UPDATE_UI_GOLDENS=1 cargo test --test ui_goldens.
use gibson::ui::prelude::*;
use gibson::{
    compute_layout, paint, Color, ColorDepth, Context, Node, RenderMode, SubcellGlyphMode, Surface,
    TerminalCapabilities, TextInputState,
};
use std::{path::PathBuf, time::Duration};

fn semantic_view() -> Element<&'static str> {
    screen()
        .key("screen")
        .child(heading("NEURAL OPERATIONS"))
        .child(
            row()
                .responsive(60)
                .gap(1)
                .child(
                    panel("Agents")
                        .grow(1.0)
                        .child(status("Connected").tone(Tone::Success))
                        .child(choice("Builder", true).key("agent").on_press("agent"))
                        .child(button("Approve").key("approve").on_press("approve")),
                )
                .child(
                    panel("Throughput")
                        .grow(1.0)
                        .child(progress("context", 0.625).width(18))
                        .child(sparkline(&[1.0, 4.0, 2.0, 6.0, 5.0, 8.0]).width(18)),
                ),
        )
        .child(
            text_input(&TextInputState::with_text("inspect build"))
                .key("input")
                .on_press("submit"),
        )
}
fn environment(width: u16, height: u16, color_depth: ColorDepth) -> UiEnvironment {
    UiEnvironment {
        width,
        height,
        color_depth,
        motion: MotionPreference::None,
        ..UiEnvironment::default()
    }
}
fn render(skin: Skin, env: UiEnvironment) -> (Surface, String, String) {
    let compiled = UiRuntime::new(skin)
        .frame(&semantic_view(), env, Duration::ZERO)
        .unwrap();
    let mut node = compiled.node;
    compute_layout(&mut node, env.width, env.height).unwrap();
    let mut surface = Surface::new(env.width, env.height);
    paint(&node, &mut surface);
    let mut context = Context::headless(RenderMode::Fullscreen, env.width, env.height);
    context.set_capabilities(TerminalCapabilities {
        color_depth: env.color_depth,
        ..TerminalCapabilities::truecolor()
    });
    context.set_sync_updates(false);
    context.set_root(node);
    context.render().unwrap();
    let ansi = context.take_output();
    let mut terminal = vt100::Parser::new(env.height, env.width, 0);
    terminal.process(ansi.as_bytes());
    (surface, terminal.screen().contents(), ansi)
}
fn plain(surface: &Surface) -> String {
    (0..surface.height)
        .map(|y| {
            (0..surface.width)
                .filter_map(|x| {
                    let cell = surface.get(x, y).unwrap();
                    (!cell.is_continuation).then_some(cell.glyph.grapheme.as_str())
                })
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_owned()
}
fn valid_cells(surface: &Surface) {
    for y in 0..surface.height {
        for x in 0..surface.width {
            let cell = surface.get(x, y).unwrap();
            if cell.is_continuation {
                assert!(x > 0);
                let lead = surface.get(x - 1, y).unwrap();
                assert_eq!(lead.glyph.display_width, 2);
                assert!(!lead.is_continuation);
            } else if cell.glyph.display_width == 2 {
                assert!(x + 1 < surface.width);
                assert!(surface.get(x + 1, y).unwrap().is_continuation);
            }
        }
    }
}

#[test]
fn one_semantic_tree_preserves_information_across_skin_capability_size_matrix() {
    for (width, height) in [(80, 24), (120, 40), (32, 16)] {
        for depth in [
            ColorDepth::TrueColor,
            ColorDepth::Ansi256,
            ColorDepth::Ansi16,
            ColorDepth::Mono,
        ] {
            for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
                let env = environment(width, height, depth);
                let (surface, terminal, ansi) = render(skin, env);
                valid_cells(&surface);
                let info = plain(&surface);
                for label in [
                    "NEURAL OPERATIONS",
                    "Connected",
                    "Builder",
                    "Approve",
                    "context",
                    "inspect build",
                ] {
                    assert!(
                        info.contains(label),
                        "{} {width}x{height} {depth:?}: missing {label}\n{info}",
                        skin.name
                    );
                    assert!(
                        terminal.contains(label),
                        "Context output lost {label}: {terminal}"
                    );
                }
                assert_eq!(
                    surface,
                    render(skin, env).0,
                    "fixed environment must be deterministic"
                );
                if depth == ColorDepth::Mono {
                    assert!(surface.cells.iter().all(|c| {
                        matches!(c.style.fg, None | Some(Color::Reset))
                            && matches!(c.style.bg, None | Some(Color::Reset))
                    }));
                    assert!(!ansi.contains("38;2;") && !ansi.contains("48;2;"));
                }
                if depth == ColorDepth::Ansi16 {
                    assert!(surface.cells.iter().all(|c| {
                        !matches!(c.style.fg, Some(Color::Rgb(..) | Color::Ansi256(_)))
                            && !matches!(c.style.bg, Some(Color::Rgb(..) | Color::Ansi256(_)))
                    }));
                    assert!(!ansi.contains("38;2;") && !ansi.contains("48;2;"));
                }
            }
        }
    }
}

#[test]
fn three_mono_design_grammars_have_distinct_plain_structure_goldens() {
    let env = environment(80, 24, ColorDepth::Mono);
    let mut captures = Vec::new();
    for (name, skin) in [
        ("vapor95", skins::VAPOR95),
        ("black_ice", skins::BLACK_ICE),
        ("swiss_signal", skins::SWISS_SIGNAL),
    ] {
        let actual = format!("{}\n", plain(&render(skin, env).0));
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("tests/goldens/ui_{name}_80x24_mono.txt"));
        if std::env::var_os("UPDATE_UI_GOLDENS").is_some() {
            std::fs::write(&path, &actual).unwrap();
        }
        let expected = std::fs::read_to_string(&path)
            .expect("golden missing; intentionally regenerate with UPDATE_UI_GOLDENS=1");
        assert_eq!(actual, expected, "{} changed structural design", skin.name);
        captures.push(actual);
    }
    for i in 0..captures.len() {
        for j in i + 1..captures.len() {
            assert_ne!(
                captures[i], captures[j],
                "skins must differ before any color is applied"
            );
        }
    }
}

#[test]
fn ascii_chrome_fallback_does_not_transliterate_application_content() {
    let mut env = environment(40, 12, ColorDepth::Mono);
    env.glyph_mode = SubcellGlyphMode::Ascii;
    for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
        let ascii = render(skin, env).0;
        assert!(plain(&ascii).is_ascii());
        let tree: Element<()> = panel("資料").child(text("界 e\u{301}"));
        let compiled = compile(&tree, &BuildCx::new(skin, env)).unwrap();
        let mut node = compiled.node;
        compute_layout(&mut node, 40, 12).unwrap();
        let mut surface = Surface::new(40, 12);
        paint(&node, &mut surface);
        assert!(plain(&surface).contains("界 e\u{301}"));
        valid_cells(&surface);
    }
}

#[test]
fn hostile_zero_tiny_and_extreme_values_stay_in_the_substrate_bounds() {
    for (width, height) in [(0, 0), (1, 1), (2, 2), (7, 3), (32, 16)] {
        for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
            let env = environment(width, height, ColorDepth::Mono);
            let tree: Element<()> = screen()
                .child(
                    row()
                        .responsive(60)
                        .child(progress("NaN", f32::NAN))
                        .child(progress("inf", f32::INFINITY))
                        .child(sparkline(&[f32::NAN, f32::INFINITY, -1.0])),
                )
                .overlay(modal("界").child(text("👩‍🔬")))
                .overlay(toast("tiny"));
            let mut node: Node = compile(&tree, &BuildCx::new(skin, env)).unwrap().node;
            compute_layout(&mut node, width, height).unwrap();
            let mut surface = Surface::new(width, height);
            paint(&node, &mut surface);
            assert_eq!(surface.cells.len(), width as usize * height as usize);
            valid_cells(&surface);
        }
    }
}
