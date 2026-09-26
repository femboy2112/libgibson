//! Shared demo flags, never required by the public UI API.

use gibson::context::{Context, RenderMode};
use gibson::glyph::GlyphChoice;
use gibson::ui::prelude::*;
use gibson::ColorDepth;
use std::io;
use std::time::Duration;

pub struct Options {
    pub args: Vec<String>,
    pub skin: Skin,
    pub width: u16,
    pub height: u16,
    pub depth: Option<ColorDepth>,
    pub motion: MotionPreference,
    pub dump: bool,
    pub fullscreen: bool,
}

impl Options {
    pub fn read() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let has = |flag: &str| args.iter().any(|a| a == flag);
        let value = |prefix: &str| args.iter().find_map(|a| a.strip_prefix(prefix));
        let skin = match value("--skin=") {
            Some("vapor95") => skins::VAPOR95,
            Some("swiss-signal") => skins::SWISS_SIGNAL,
            _ => skins::BLACK_ICE,
        };
        let depth = if has("--mono") {
            Some(ColorDepth::Mono)
        } else if has("--ansi16") {
            Some(ColorDepth::Ansi16)
        } else if has("--ansi256") {
            Some(ColorDepth::Ansi256)
        } else if has("--truecolor") {
            Some(ColorDepth::TrueColor)
        } else {
            None
        };
        Self {
            skin,
            width: value("--width=")
                .and_then(|s| s.parse().ok())
                .unwrap_or(80)
                .max(1),
            height: value("--height=")
                .and_then(|s| s.parse().ok())
                .unwrap_or(24)
                .max(1),
            depth,
            motion: if has("--no-motion") {
                MotionPreference::None
            } else if has("--reduced-motion") {
                MotionPreference::Reduced
            } else {
                MotionPreference::Full
            },
            dump: has("--dump") || has("--dump-ansi"),
            fullscreen: has("--fullscreen"),
            args,
        }
    }

    pub fn has(&self, flag: &str) -> bool {
        self.args.iter().any(|a| a == flag)
    }

    pub fn value(&self, prefix: &str) -> Option<&str> {
        self.args.iter().find_map(|a| a.strip_prefix(prefix))
    }

    pub fn context(&self) -> io::Result<Context> {
        let mode = if self.fullscreen {
            RenderMode::Fullscreen
        } else {
            RenderMode::Inline
        };
        let mut context = if self.dump {
            Context::headless(mode, self.width, self.height)
        } else {
            Context::new(mode)?
        };
        if let Some(depth) = self.depth {
            context.set_color_depth(depth);
        } else if self.dump {
            context.set_color_depth(ColorDepth::TrueColor);
        }
        Ok(context)
    }

    pub fn environment(&self, context: &Context) -> UiEnvironment {
        let (width, terminal_height) = context.session.terminal_size();
        // Inline Context reserves one physical row for its cursor/scrollback
        // handoff. Overlays must target the same available region.
        let height = if self.fullscreen {
            terminal_height
        } else {
            terminal_height.saturating_sub(1).max(1)
        };
        UiEnvironment {
            width,
            height,
            color_depth: context.capabilities().color_depth,
            glyph_mode: self
                .value("--glyphs=")
                .and_then(GlyphChoice::parse)
                .unwrap_or(if self.dump {
                    GlyphChoice::Fixed(gibson::SubcellGlyphMode::Braille2x4)
                } else {
                    GlyphChoice::Auto
                })
                .resolve(|key| std::env::var(key).ok()),
            motion: self.motion,
        }
    }

    pub fn print_capture(&self, context: &Context) {
        if self.has("--dump-ansi") {
            use std::io::Write;
            std::io::stdout()
                .write_all(context.rendered_bytes())
                .expect("write capture");
        } else {
            let mut parser = vt100::Parser::new(self.height, self.width, 0);
            parser.process(context.rendered_bytes());
            println!("{}", parser.screen().contents());
        }
    }
}

pub fn present<A: Clone>(
    runtime: &mut UiRuntime<A>,
    tree: &Element<A>,
    context: &mut Context,
    environment: UiEnvironment,
    now: Duration,
) -> io::Result<()> {
    let frame = runtime
        .frame(tree, environment, now)
        .map_err(io::Error::other)?;
    context.set_root(frame.node);
    context.render_now()?;
    Ok(())
}
