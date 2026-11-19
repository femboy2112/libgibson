//! Popups over an inline screen must not reflow (0.8.1 hardening; `docs/UI_LAYER.md`).
//!
//! Recipe under test: give the inline `screen()` a constant `.height(...)`. Opening a modal
//! overlay then leaves the live region's height and the scrollback untouched, and closing it
//! restores every cell exactly. The characterization test pins the footgun the recipe avoids:
//! an auto-height screen grows to the full inline height the moment an overlay is mounted.
//! Synthetic ASCII only; public Rust SDK APIs.
use gibson::ui::prelude::*;
use gibson::{ColorDepth, Context, RenderMode};
use std::num::NonZeroU16;
use std::time::Duration;

const COLS: u16 = 60;
const ROWS: u16 = 20;
/// The inline environment height the App loop uses: one row less than the terminal.
const INLINE_H: u16 = ROWS - 1;
const PINNED_H: u16 = 10;

struct Scene {
    ctx: Context,
    rt: UiRuntime<()>,
    vt: vt100::Parser,
}

impl Scene {
    fn new() -> Self {
        let mut ctx = Context::headless(RenderMode::Inline, COLS, ROWS);
        ctx.set_color_depth(ColorDepth::Mono);
        Self {
            ctx,
            rt: UiRuntime::new(skins::BLACK_ICE),
            vt: vt100::Parser::new(
                NonZeroU16::new(ROWS).unwrap(),
                NonZeroU16::new(COLS).unwrap(),
                256,
            ),
        }
    }

    /// Render one frame; return `Renderer::live_region_height` afterwards.
    fn frame(&mut self, tree: &Element<()>) -> u16 {
        let env = UiEnvironment {
            width: COLS,
            height: INLINE_H,
            motion: MotionPreference::None,
            ..UiEnvironment::default()
        };
        let f = self.rt.frame(tree, env, Duration::ZERO).unwrap();
        self.ctx.set_root(f.node);
        self.ctx.render_now().unwrap();
        let bytes = self.ctx.take_output();
        self.vt.process(bytes.as_bytes());
        self.ctx.renderer.live_region_height
    }

    fn scrollback_len(&self) -> usize {
        self.vt.screen().scrollback_len()
    }
}

fn base(height: Option<u16>) -> Element<()> {
    let mut s = screen().key("stage");
    if let Some(h) = height {
        s = s.height(h);
    }
    s.child(text("STAGE_ROW_A"))
        .child(text("STAGE_ROW_B"))
        .child(text("STAGE_ROW_C"))
}

fn with_popup(height: Option<u16>) -> Element<()> {
    base(height).overlay(modal("CONSENT").key("consent").child(text("POPUP_TERMS")))
}

/// Open then close a popup over a pinned-height inline screen: no reflow, exact restore.
fn assert_popup_is_reflow_free(height: Option<u16>) {
    let mut s = Scene::new();
    let before_h = s.frame(&base(height));
    let before_screen = s.vt.screen().contents_formatted();
    let before_scrollback = s.scrollback_len();

    let open_h = s.frame(&with_popup(height));
    assert_eq!(
        open_h, before_h,
        "opening the popup changed the live region height"
    );
    assert_eq!(
        s.scrollback_len(),
        before_scrollback,
        "opening the popup pushed rows into scrollback"
    );
    let open = s.vt.screen().contents();
    assert!(open.contains("POPUP_TERMS"), "popup not painted:\n{open}");
    assert!(
        open.contains("STAGE_ROW_A"),
        "base hidden by the popup:\n{open}"
    );

    let closed_h = s.frame(&base(height));
    assert_eq!(closed_h, before_h, "closing the popup changed the height");
    assert_eq!(s.scrollback_len(), before_scrollback);
    assert_eq!(
        s.vt.screen().contents_formatted(),
        before_screen,
        "closing the popup did not restore the stage cells exactly"
    );
}

#[test]
fn popup_over_a_pinned_inline_screen_does_not_reflow_and_restores_exactly() {
    // RED: deliberately un-pinned, to prove the test discriminates.
    let _ = PINNED_H;
    assert_popup_is_reflow_free(None);
}

/// Characterization of the documented footgun (compile.rs: a `screen()` with an overlay and no
/// explicit height takes the environment height). If this starts failing, the footgun is gone:
/// update `docs/UI_LAYER.md` and the `Element::overlay` rustdoc.
#[test]
fn popup_over_an_auto_height_inline_screen_grows_the_live_region() {
    let mut s = Scene::new();
    let before_h = s.frame(&base(None));
    assert!(before_h < INLINE_H, "the base should be content-sized");
    assert_eq!(s.frame(&with_popup(None)), INLINE_H);
}
