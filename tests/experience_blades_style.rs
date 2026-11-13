//! #369: Blades colours from a consumer palette or skin.
//!
//! The default palette must stay byte-identical to the pre-#369 output. The
//! golden hashes were recorded from `main` at 220e3e5, before `BladesStyle`
//! existed, by painting the same frames through the same pipeline. The styled
//! tests use a synthetic one-hue skin, so every colour Blades paints must carry
//! that hue's signature (`g == b`), which the procedural palette never does.

use gibson::capability::ColorDepth;
use gibson::cell::{Color, Theme};
use gibson::ui::experience::{
    Action, Blades, BladesStyle, Content, Destination, Experience, Fallback, Grammar, Item,
    PresentationState,
};
use gibson::ui::skin::{DepthPalette, Skin, UiEnvironment};
use gibson::ui::MotionPreference;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
struct Msg(u32);

fn fixture() -> Experience<Msg> {
    let items: Vec<Item<Msg>> = (0..7)
        .map(|i| {
            Item::new(format!("i{i}"), format!("UNDERWAY · job {i}"))
                .subtitle(format!("detail {i}"))
                .action(Action::new(format!("open-{i}"), "Open", Msg(i)))
        })
        .collect();
    let prose = |s: &str| Content::Prose(vec![s.to_string()]);
    Experience::new("SYNTHETIC")
        .destination(Destination::new("chat", "Chat", prose("conversation")))
        .destination(Destination::new(
            "tasks",
            "Tasks",
            Content::Collection(items),
        ))
        .destination(Destination::new("memory", "Memory", prose("memory")))
        .destination(Destination::new("beings", "Beings", prose("beings")))
}

fn env(width: u16, height: u16, color_depth: ColorDepth) -> UiEnvironment {
    UiEnvironment {
        width,
        height,
        color_depth,
        ..UiEnvironment::default()
    }
}

/// Paint one settled frame through compile -> layout -> paint.
fn paint(grammar: &mut dyn Grammar<Msg>, e: &UiEnvironment) -> gibson::Surface {
    let experience = fixture();
    let mut state = PresentationState::new(&experience);
    use gibson::ui::element::Key;
    assert!(state.activate(&experience, &Key::named("tasks")));
    assert!(state.select(&experience, &Key::named("i2")));
    // Present twice so the stack spring is initialized and settled on `tasks`.
    let _ = grammar.present(&experience, &state, e, Duration::ZERO);
    let presented = grammar.present(&experience, &state, e, Duration::from_secs(5));
    let cx = gibson::ui::BuildCx::new(gibson::ui::skins::BLACK_ICE, *e);
    let mut node = gibson::ui::compile(&presented.element, &cx)
        .expect("lowers")
        .node;
    gibson::compute_layout(&mut node, e.width, e.height).unwrap();
    let mut surface = gibson::Surface::new(e.width, e.height);
    gibson::paint(&node, &mut surface);
    surface
}

/// FNV-1a over the Debug form of every cell: stable across Rust releases,
/// unlike `DefaultHasher`.
fn fnv(surface: &gibson::Surface) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for cell in &surface.cells {
        for b in format!("{cell:?}").bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

const CASES: [(u16, u16, ColorDepth); 4] = [
    (120, 40, ColorDepth::TrueColor),
    (80, 24, ColorDepth::TrueColor),
    (80, 24, ColorDepth::Ansi256),
    (80, 24, ColorDepth::Mono),
];

const GOLDEN: [u64; 4] = [
    0x7e18_b8b8_47cf_3e2b,
    0x5a00_ac1c_8bf0_ac87,
    0x21be_bd8b_fcb8_36de,
    0xb875_edd6_3860_4ea6,
];

#[test]
fn default_and_procedural_style_match_the_pre_369_frames() {
    for ((w, h, d), want) in CASES.into_iter().zip(GOLDEN) {
        let e = env(w, h, d);
        assert_eq!(
            fnv(&paint(&mut Blades::new(), &e)),
            want,
            "Blades::new {w}x{h} {d:?}"
        );
        assert_eq!(
            fnv(&paint(
                &mut Blades::with_style(BladesStyle::procedural()),
                &e
            )),
            want,
            "procedural style {w}x{h} {d:?}"
        );
        assert_eq!(BladesStyle::default(), BladesStyle::procedural());
    }
}

/// A synthetic one-hue (red) skin: every colour has `g == b`.
fn red_theme(scale: u8) -> Theme {
    let c = |r: u8, gb: u8| Color::Rgb(r, gb / scale, gb / scale);
    Theme {
        text: c(255, 236),
        text_muted: c(200, 150),
        accent: c(220, 30),
        success: c(120, 60),
        warning: c(240, 120),
        error: c(255, 0),
        border: c(90, 20),
        rail: c(150, 20),
        code: c(240, 200),
        bg: c(16, 0),
    }
}

fn red_skin() -> Skin {
    Skin {
        name: "synthetic-red",
        palette: red_theme(1),
        surface: Color::Rgb(40, 8, 8),
        well: Color::Rgb(24, 4, 4),
        ansi16: None,
        ansi256: None,
        ..gibson::ui::skins::BLACK_ICE
    }
}

/// Every RGB colour in the raster rows (all but the footer row, which the
/// lowering skin paints).
fn raster_rgbs(surface: &gibson::Surface) -> Vec<(u8, u8, u8)> {
    let mut out = Vec::new();
    for y in 0..surface.height.saturating_sub(1) {
        for x in 0..surface.width {
            let cell = surface.get(x, y).unwrap();
            for c in [cell.style.fg, cell.style.bg].into_iter().flatten() {
                if let Color::Rgb(r, g, b) = c {
                    out.push((r, g, b));
                }
            }
        }
    }
    out
}

#[test]
fn a_skinned_stack_paints_only_in_the_skin_hue() {
    let e = env(120, 40, ColorDepth::TrueColor);
    let procedural = raster_rgbs(&paint(&mut Blades::new(), &e));
    assert!(
        procedural.iter().any(|&(_, g, b)| g != b),
        "control: the procedural palette is not red-only"
    );
    for (name, mut grammar) in [
        ("with_skin", Blades::with_skin(red_skin())),
        (
            "with_style(from_skin)",
            Blades::with_style(BladesStyle::from_skin(&red_skin(), ColorDepth::TrueColor)),
        ),
    ] {
        let styled = raster_rgbs(&paint(&mut grammar, &e));
        assert!(styled.len() > 1000, "{name}: the stack is painted in RGB");
        let off: Vec<_> = styled.iter().filter(|&&(_, g, b)| g != b).collect();
        assert!(
            off.is_empty(),
            "{name}: colours outside the skin hue: {off:?}"
        );
        // The plane bodies are surface tinted toward accent: red-dominant.
        assert!(styled.iter().filter(|&&(r, g, _)| r > g).count() > styled.len() / 2);
    }
}

#[test]
fn style_fields_reach_the_frame() {
    let e = env(120, 40, ColorDepth::TrueColor);
    let mut style = BladesStyle::from_skin(&red_skin(), ColorDepth::TrueColor);
    let base = fnv(&paint(&mut Blades::with_style(style.clone()), &e));
    style.ink = (255, 255, 255);
    let inked = fnv(&paint(&mut Blades::with_style(style.clone()), &e));
    assert_ne!(base, inked, "ink changes the text colour");
    style.planes = vec![(10, 60, 60)];
    let planed = fnv(&paint(&mut Blades::with_style(style.clone()), &e));
    assert_ne!(inked, planed, "planes change the plane fill");
    style.planes.clear();
    let procedural_planes = fnv(&paint(&mut Blades::with_style(style), &e));
    assert_ne!(
        planed, procedural_planes,
        "empty planes fall back to procedural hues"
    );
}

#[test]
fn a_skin_palette_follows_its_depth_overrides() {
    // An explicit ansi256 palette in a second hue (blue: r == g).
    let blue = |r: u8, b: u8| Color::Rgb(r, r, b);
    let ansi256 = DepthPalette {
        palette: Theme {
            text: blue(230, 255),
            text_muted: blue(150, 200),
            accent: blue(30, 220),
            rail: blue(20, 150),
            bg: blue(0, 16),
            ..red_theme(1)
        },
        surface: blue(8, 40),
        well: blue(4, 24),
    };
    let skin = Skin {
        ansi256: Some(ansi256),
        ..red_skin()
    };
    let grammar = Blades::with_skin(skin);
    assert_eq!(
        grammar.resolved_style(ColorDepth::Ansi256),
        BladesStyle::from_palette(&ansi256)
    );
    assert_eq!(
        grammar.resolved_style(ColorDepth::TrueColor),
        BladesStyle::from_skin(&red_skin(), ColorDepth::TrueColor)
    );
    let s = grammar.resolved_style(ColorDepth::Ansi256);
    assert!(s.planes.iter().all(|&(r, g, b)| r == g && b > r));
}

#[test]
fn reset_colours_fall_back_to_procedural_values() {
    let palette = DepthPalette {
        palette: Theme::default(), // text and bg are Color::Reset
        surface: Color::Reset,
        well: Color::Reset,
    };
    let s = BladesStyle::from_palette(&palette);
    let p = BladesStyle::procedural();
    assert_eq!(s.background, p.background);
    assert_eq!(s.ink, p.ink);
}

#[test]
fn styling_never_changes_semantics() {
    let experience = fixture();
    let mut state = PresentationState::new(&experience);
    use gibson::ui::element::Key;
    assert!(state.activate(&experience, &Key::named("tasks")));
    for e in [
        env(120, 40, ColorDepth::TrueColor),
        env(80, 24, ColorDepth::Ansi16),
        env(60, 9, ColorDepth::Mono),
    ] {
        let a = Blades::new().present(&experience, &state, &e, Duration::ZERO);
        let b = Blades::with_skin(red_skin()).present(&experience, &state, &e, Duration::ZERO);
        assert_eq!(a.receipt, b.receipt);
        assert_eq!(a.demand, b.demand);
    }
}

#[test]
fn a_skinned_blades_works_inside_the_fallback() {
    let experience = fixture();
    let state = PresentationState::new(&experience);
    let mut fallback: Fallback<Msg> = Fallback::new(Box::new(Blades::with_skin(red_skin())));
    let wide = UiEnvironment {
        motion: MotionPreference::Full,
        ..env(120, 40, ColorDepth::TrueColor)
    };
    assert_eq!(
        fallback
            .present(&experience, &state, &wide, Duration::ZERO)
            .receipt
            .style,
        "BLADES"
    );
    let narrow = env(40, 15, ColorDepth::TrueColor);
    assert_eq!(
        fallback
            .present(&experience, &state, &narrow, Duration::ZERO)
            .receipt
            .style,
        "STANDARD"
    );
}
