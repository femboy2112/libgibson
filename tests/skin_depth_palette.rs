//! Per-colour-depth skin palettes and tone-collision reporting (consumer request,
//! Renaissance XXXII). A local skin with a gold accent and an amber warning must be
//! able to keep them distinct at reduced depths without giving up its RGB palette.
use gibson::capability::ColorDepth;
use gibson::ui::prelude::*;
use gibson::{compute_layout, paint, Color, Surface, Theme};

const GOLD: Color = Color::Rgb(212, 175, 55);
const AMBER: Color = Color::Rgb(255, 176, 0);
const PURPLE: Color = Color::Rgb(36, 14, 58);
const PURPLE_WELL: Color = Color::Rgb(26, 8, 44);

fn rgb_palette() -> Theme {
    Theme {
        text: Color::Rgb(232, 224, 240),
        text_muted: Color::Rgb(150, 130, 170),
        accent: GOLD,
        success: Color::Rgb(90, 200, 120),
        warning: AMBER,
        error: Color::Rgb(230, 60, 70),
        border: Color::Rgb(90, 60, 120),
        rail: Color::Rgb(160, 120, 220),
        code: Color::Rgb(240, 210, 120),
        bg: PURPLE,
    }
}

fn plain_skin() -> Skin {
    Skin {
        name: "gold-on-purple",
        palette: rgb_palette(),
        surface: PURPLE,
        well: PURPLE_WELL,
        ..skins::BLACK_ICE
    }
}

fn ansi16_override() -> DepthPalette {
    DepthPalette {
        palette: Theme {
            text: Color::White,
            text_muted: Color::BrightBlack,
            accent: Color::BrightYellow,
            success: Color::Green,
            warning: Color::Red,
            error: Color::BrightRed,
            border: Color::Magenta,
            rail: Color::BrightMagenta,
            code: Color::Yellow,
            bg: Color::Black,
        },
        surface: Color::Black,
        well: Color::Black,
    }
}

fn ansi256_override() -> DepthPalette {
    DepthPalette {
        palette: Theme {
            accent: Color::Ansi256(178),
            warning: Color::Ansi256(202),
            ..rgb_palette()
        },
        surface: Color::Ansi256(53),
        well: Color::Ansi256(52),
    }
}

fn skin_with_overrides() -> Skin {
    Skin {
        ansi16: Some(ansi16_override()),
        ansi256: Some(ansi256_override()),
        ..plain_skin()
    }
}

fn env(depth: ColorDepth) -> UiEnvironment {
    UiEnvironment {
        color_depth: depth,
        motion: MotionPreference::None,
        ..UiEnvironment::default()
    }
}

const DISTINCT: [Tone; 4] = [Tone::Success, Tone::Warning, Tone::Danger, Tone::Accent];

fn distinct_collisions(r: &gibson::ui::ResolvedSkin) -> Vec<(Tone, Tone)> {
    r.tone_collisions()
        .into_iter()
        .filter(|(a, b)| DISTINCT.contains(a) && DISTINCT.contains(b))
        .collect()
}

#[test]
fn reproducer_gold_and_amber_merge_at_ansi16_without_an_override() {
    // Negative control: this is the defect the override exists for.
    let r = plain_skin().resolve(&env(ColorDepth::Ansi16));
    assert_eq!(
        r.palette.accent, r.palette.warning,
        "nearest mapping merges them"
    );
    assert!(r.tone_collisions().contains(&(Tone::Accent, Tone::Warning)));
}

#[test]
fn override_keeps_safety_tones_distinct_at_every_colour_depth() {
    let skin = skin_with_overrides();
    for depth in [
        ColorDepth::Ansi16,
        ColorDepth::Ansi256,
        ColorDepth::TrueColor,
    ] {
        let r = skin.resolve(&env(depth));
        assert_eq!(distinct_collisions(&r), vec![], "{depth:?}");
    }
}

#[test]
fn override_colours_are_used_verbatim_at_their_depth() {
    let skin = skin_with_overrides();
    let r16 = skin.resolve(&env(ColorDepth::Ansi16));
    assert_eq!(r16.palette.accent, Color::BrightYellow);
    assert_eq!(r16.palette.warning, Color::Red);
    assert_eq!(r16.surface, Color::Black);
    assert_eq!(r16.well, Color::Black);
    assert_eq!(
        r16.style(Tone::Warning, Emphasis::Normal).fg,
        Some(Color::Red)
    );

    let r256 = skin.resolve(&env(ColorDepth::Ansi256));
    assert_eq!(r256.palette.accent, Color::Ansi256(178));
    assert_eq!(r256.palette.warning, Color::Ansi256(202));
    assert_eq!(r256.surface, Color::Ansi256(53));
    assert_eq!(r256.well, Color::Ansi256(52));
}

#[test]
fn truecolor_and_mono_ignore_depth_overrides() {
    for depth in [ColorDepth::TrueColor, ColorDepth::Mono] {
        assert_eq!(
            skin_with_overrides().resolve(&env(depth)),
            plain_skin().resolve(&env(depth)),
            "{depth:?}"
        );
    }
    let tc = skin_with_overrides().resolve(&env(ColorDepth::TrueColor));
    assert_eq!(tc.palette.accent, GOLD);
    assert_eq!(tc.surface, PURPLE);
}

#[test]
fn an_override_equal_to_the_skin_palette_is_neutral() {
    // Built-in skins carry None; spelling their own colours as an explicit override
    // must resolve identically, so the None path and the override path agree.
    for base in [skins::BLACK_ICE, skins::SWISS_SIGNAL, skins::VAPOR95] {
        assert_eq!(base.ansi16, None);
        assert_eq!(base.ansi256, None);
        let own = DepthPalette {
            palette: base.palette,
            surface: base.surface,
            well: base.well,
        };
        let explicit = Skin {
            ansi16: Some(own),
            ansi256: Some(own),
            ..base
        };
        for depth in [
            ColorDepth::Mono,
            ColorDepth::Ansi16,
            ColorDepth::Ansi256,
            ColorDepth::TrueColor,
        ] {
            assert_eq!(base.resolve(&env(depth)), explicit.resolve(&env(depth)));
        }
    }
}

#[test]
fn ansi16_override_reaches_painted_cells() {
    let e = UiEnvironment {
        width: 40,
        height: 6,
        ..env(ColorDepth::Ansi16)
    };
    let tree: Element<()> = column()
        .child(label("WARN").tone(Tone::Warning))
        .child(label("GOLD").tone(Tone::Accent));
    let paint_with = |skin: Skin| {
        let mut node = compile(&tree, &BuildCx::new(skin, e)).unwrap().node;
        compute_layout(&mut node, e.width, e.height).unwrap();
        let mut s = Surface::new(e.width, e.height);
        paint(&node, &mut s);
        s
    };
    let fg_of = |s: &Surface, word: &str| {
        for y in 0..s.height {
            for x in 0..s.width {
                let row: String = (x..s.width)
                    .map(|cx| s.get(cx, y).unwrap().glyph.grapheme.to_string())
                    .collect();
                if row.starts_with(word) {
                    return s.get(x, y).unwrap().style.fg;
                }
            }
        }
        panic!("{word} not painted");
    };
    let plain = paint_with(plain_skin());
    assert_eq!(
        fg_of(&plain, "WARN"),
        fg_of(&plain, "GOLD"),
        "control: merged"
    );
    let fixed = paint_with(skin_with_overrides());
    assert_eq!(fg_of(&fixed, "WARN"), Some(Color::Red));
    assert_eq!(fg_of(&fixed, "GOLD"), Some(Color::BrightYellow));
}

#[test]
fn tone_collisions_report_identical_pairs_only() {
    // Every tone identical: all 15 pairs. Truecolor distinct palette: none of the four.
    let flat = Skin {
        palette: Theme {
            text: GOLD,
            text_muted: GOLD,
            accent: GOLD,
            success: GOLD,
            warning: GOLD,
            error: GOLD,
            border: GOLD,
            rail: GOLD,
            code: GOLD,
            bg: PURPLE,
        },
        ..plain_skin()
    };
    let r = flat.resolve(&env(ColorDepth::TrueColor));
    assert_eq!(r.tone_collisions().len(), 15);
    let r = plain_skin().resolve(&env(ColorDepth::TrueColor));
    assert_eq!(distinct_collisions(&r), vec![]);
}

#[test]
fn tone_collisions_treat_a_base_colour_and_its_256_index_as_one_colour() {
    // SGR 33 and 38;5;3 select the same palette slot.
    let skin = Skin {
        ansi256: Some(DepthPalette {
            palette: Theme {
                warning: Color::Yellow,
                accent: Color::Ansi256(3),
                ..rgb_palette()
            },
            ..ansi256_override()
        }),
        ..plain_skin()
    };
    let r = skin.resolve(&env(ColorDepth::Ansi256));
    assert_ne!(r.palette.warning, r.palette.accent, "distinct values");
    assert!(r.tone_collisions().contains(&(Tone::Accent, Tone::Warning)));
}
