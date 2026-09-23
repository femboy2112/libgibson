//! FX Lab — a developer gallery for LibGibson's scene/effects primitives.
//!
//! This is not a narrative demo. It isolates each reusable primitive so it can be
//! inspected, benchmarked and used as a renderer torture gallery.
//!
//! Keys: `1..9`/`0` select a scene, `q`/Ctrl-C/End quit.
//!
//! Flags: `--deterministic --freeze-at=N --no-color --debug-renderer`
//!        `--mono --ansi16 --ansi256 --truecolor --no-sync --no-insert-line`

use std::collections::HashMap;
use std::env;
use std::time::Duration;

use gibson::cell::{Color, Line, RichText, Span, Style, Theme, ThemeStyles};
use gibson::node::{Node, WrapMode};
use gibson::surface::BorderType;
use gibson::{Context, Mesh, Projector, TimeSource, Transform3, ViewportState};

const SCENES: &[&str] = &[
    "Braille oscilloscope",
    "HalfBlock plasma",
    "Particles",
    "Rotating cube",
    "Wireframe torus",
    "Scanline overlay",
    "Framebuffer glitch",
    "Compositor (modal+shadow)",
    "Viewport camera pan",
    "Damage heatmap",
];

struct Fx {
    st: ThemeStyles,
    color: bool,
}

impl Fx {
    fn new(theme: Theme, color: bool) -> Self {
        Self {
            st: theme.styles(),
            color,
        }
    }
}

struct Lab {
    scene: usize,
    fx: Fx,
    t: f32,
    wave: Vec<f32>,
    particles: gibson::ParticleSystem,
    cam: ViewportState,
    damage: HashMap<(u16, u16), u32>,
    last_total_bytes: u64,
    frame_bytes: Vec<f32>,
    debug: bool,
}

impl Lab {
    fn new(fx: Fx, debug: bool) -> Self {
        Self {
            scene: 0,
            fx,
            t: 0.0,
            wave: Vec::new(),
            particles: gibson::ParticleSystem::new(0xC0FFEE),
            cam: ViewportState::new(),
            damage: HashMap::new(),
            last_total_bytes: 0,
            frame_bytes: Vec::new(),
            debug,
        }
    }

    fn tick(&mut self, ctx: &mut Context) {
        self.t += 1.0 / 60.0;
        // Deterministic starfield: reseed emission when empty.
        if self.particles.len() < 140 {
            for i in 0..12 {
                self.particles.particles.push(gibson::Particle {
                    x: ((i * 37) % 100) as f32,
                    y: 0.0,
                    vx: 0.0,
                    vy: 3.0,
                    life: 4.0,
                    max_life: 4.0,
                    intensity: 1.0,
                });
            }
        }
        self.particles.update(1.0 / 60.0);
        if self.particles.len() > 1500 {
            self.particles.particles.drain(0..500);
        }

        let stats = ctx.stats();
        let delta = stats.frame_bytes.saturating_sub(self.last_total_bytes) as f32;
        self.last_total_bytes = stats.frame_bytes;
        self.frame_bytes.push(delta);
        if self.frame_bytes.len() > 120 {
            self.frame_bytes.remove(0);
        }
        // Normalize the byte history to [-1, 1] for the oscilloscope.
        let max = self.frame_bytes.iter().cloned().fold(1.0_f32, f32::max);
        self.wave = self
            .frame_bytes
            .iter()
            .map(|v| v / max * 2.0 - 1.0)
            .collect();

        // Camera drift for the viewport scene.
        if self.scene == 8 {
            self.cam.scroll_by(1, 0);
            self.cam.clamp(80, 24, 40, 10);
        }

        if self.scene == 9 {
            for (x, y) in ctx.last_dirty_cells() {
                *self.damage.entry((*x, *y)).or_insert(0) += 1;
            }
        }
    }

    fn header(&self, stats: &gibson::RenderStats, cols: u16) -> Node {
        let fx = &self.fx;
        let dirty_pct = if stats.total_cells > 0 {
            (stats.dirty_cells as f64 / (stats.frames.max(1) as f64 * stats.total_cells as f64)
                * 100.0) as u32
        } else {
            0
        };
        let mut line = Line::new()
            .span(Span::styled("FX LAB ", fx.st.accent.bold()))
            .span(Span::styled(
                format!("[{}] {}", self.scene + 1, SCENES[self.scene]),
                fx.st.text,
            ))
            .span(Span::styled(format!("  t={:>5.1}s", self.t), fx.st.muted));
        if self.debug || cols >= 70 {
            line = line
                .span(Span::styled("  frames ", fx.st.muted))
                .span(Span::styled(format!("{}", stats.frames), fx.st.text))
                .span(Span::styled("  dirty ", fx.st.muted))
                .span(Span::styled(format!("{}", stats.dirty_cells), fx.st.text))
                .span(Span::styled("  bytes ", fx.st.muted))
                .span(Span::styled(format!("{}", stats.frame_bytes), fx.st.text))
                .span(Span::styled("  ~", fx.st.muted))
                .span(Span::styled(format!("{}%", dirty_pct), fx.st.text));
        }
        Node::line(line).height(1.0)
    }

    fn body(&self, cols: u16, rows: u16) -> Node {
        let fx = &self.fx;
        let inner_w = cols.saturating_sub(4);
        let content_rows = rows.saturating_sub(5);
        match self.scene {
            0 => {
                let cw = inner_w.clamp(8, 80);
                let ch = content_rows.saturating_sub(1).max(4);
                let style = if fx.color {
                    fx.st.accent
                } else {
                    Style::default()
                };
                let canvas = gibson::braille_oscilloscope(&self.wave, cw, ch);
                Node::col()
                    .child(Node::text(
                        format!(
                            "waveform: {} samples · per-frame wire bytes",
                            self.wave.len()
                        ),
                        fx.st.muted,
                    ))
                    .child(Node::raster(canvas.to_surface(style)))
            }
            1 => {
                let cw = inner_w.clamp(8, 80);
                let ch = content_rows.max(4);
                if fx.color {
                    let mut canvas = gibson::HalfBlockCanvas::new(cw, ch);
                    gibson::render_plasma_halfblock(&mut canvas, self.t, 0.3);
                    Node::raster(canvas.to_surface())
                } else {
                    let mut canvas = gibson::BrailleCanvas::new(cw, ch);
                    gibson::render_field_braille(&mut canvas, self.t, 0.3, 0.5, 5.0);
                    Node::raster(canvas.to_surface(Style::default()))
                }
            }
            2 => {
                let cw = inner_w.clamp(8, 90);
                let ch = content_rows.max(4);
                let mut canvas = gibson::BrailleCanvas::new(cw, ch);
                self.particles.render_braille(&mut canvas);
                Node::raster(canvas.to_surface(fx.st.accent))
            }
            3 => self.wireframe(Mesh::cube(1.6), inner_w, content_rows),
            4 => self.wireframe(Mesh::torus(1.0, 0.38, 18, 10), inner_w, content_rows),
            5 => {
                let base = Node::col()
                    .child(Node::text("SCANLINE OVERLAY", fx.st.text))
                    .child(Node::rule(None::<String>, fx.st.border))
                    .child(Node::text(
                        "A style-only dim band sweeps the region without reflowing it.",
                        fx.st.muted,
                    ));
                let y = ((self.t * 6.0) as u16) % rows.max(1);
                Node::stack()
                    .percent_width(100.0)
                    .percent_height(100.0)
                    .child(base)
                    .child(
                        Node::col()
                            .percent_width(100.0)
                            .percent_height(100.0)
                            .padding_top(y as f32)
                            .child(Node::dim().percent_width(100.0).height(1.0))
                            .child(Node::dim().percent_width(100.0).height(2.0)),
                    )
            }
            6 => {
                // Safe framebuffer glitch: mutate cells, never the protocol.
                let text = "THE TERMINAL TRANSPORT STAYS IMMACULATE\n"
                    .repeat(content_rows.max(1) as usize);
                let mut node = Node::rich_text_wrapped(RichText::raw(text), WrapMode::NoWrap);
                node.layout_style.width = gibson::node::Dimension::Length(inner_w as f32);
                let base = Node::col()
                    .child(Node::text(
                        "framebuffer glitch (row shift + seeded substitution)",
                        fx.st.warning,
                    ))
                    .child(node);
                let y = ((self.t * 4.0) as u16) % rows.max(1);
                Node::stack()
                    .percent_width(100.0)
                    .percent_height(100.0)
                    .child(base)
                    .child(
                        Node::col()
                            .percent_width(100.0)
                            .percent_height(100.0)
                            .padding_top(y as f32)
                            .child(Node::dim().percent_width(100.0).height(1.0)),
                    )
            }
            7 => {
                let base = Node::panel("BASE", BorderType::Rounded, fx.st.border)
                    .percent_width(100.0)
                    .percent_height(100.0)
                    .child(Node::text(
                        "underlying content does not reflow",
                        fx.st.muted,
                    ));
                let modal = Node::stack()
                    .percent_width(100.0)
                    .percent_height(100.0)
                    .child(Node::dim().percent_width(100.0).percent_height(100.0))
                    .child(
                        Node::col()
                            .percent_width(100.0)
                            .percent_height(100.0)
                            .align_items(gibson::node::AlignItems::Center)
                            .justify_content(gibson::node::JustifyContent::Center)
                            .child(
                                Node::panel("MODAL", BorderType::Rounded, fx.st.warning)
                                    .width(30.0)
                                    .height(5.0)
                                    .background(Color::Reset)
                                    .child(Node::text("floating, composited", fx.st.text)),
                            ),
                    );
                Node::stack()
                    .percent_width(100.0)
                    .percent_height(100.0)
                    .child(base)
                    .child(modal)
            }
            8 => {
                let cw = 40u16;
                let ch = 10u16;
                let mut world = Node::col().width(80.0).height(24.0);
                for r in 0..24 {
                    world = world.child(
                        Node::text(
                            format!("row {r:02}  camera x={:>3}", self.cam.offset_x),
                            fx.st.muted,
                        )
                        .width(80.0)
                        .height(1.0),
                    );
                }
                let view = Node::viewport(self.cam.offset_x, self.cam.offset_y)
                    .width(cw as f32)
                    .height(ch as f32)
                    .child(world);
                Node::col()
                    .child(Node::text(
                        format!("viewport {}x{} inside an 80x24 world", cw, ch),
                        fx.st.text,
                    ))
                    .child(view)
            }
            _ => {
                // Damage heatmap: how often each cell changed over N frames.
                let cw = inner_w.clamp(8, 90);
                let ch = content_rows.max(4);
                let mut canvas = gibson::BrailleCanvas::new(cw, ch);
                let max = self.damage.values().copied().max().unwrap_or(1).max(1);
                for (&(x, y), &count) in &self.damage {
                    if x >= cw || y >= ch {
                        continue;
                    }
                    let lit = ((count as f32 / max as f32) * 8.0).ceil() as u32;
                    let dots = [
                        (0, 0),
                        (0, 1),
                        (0, 2),
                        (1, 0),
                        (1, 1),
                        (1, 2),
                        (0, 3),
                        (1, 3),
                    ];
                    for &(dx, dy) in dots.iter().take(lit as usize) {
                        canvas.set(x as i32 * 2 + dx, y as i32 * 4 + dy);
                    }
                }
                Node::col()
                    .child(Node::text(
                        format!(
                            "cells changed over {} distinct positions",
                            self.damage.len()
                        ),
                        fx.st.muted,
                    ))
                    .child(Node::raster(canvas.to_surface(fx.st.warning)))
            }
        }
    }

    fn wireframe(&self, mesh: Mesh, inner_w: u16, rows: u16) -> Node {
        let fx = &self.fx;
        let cw = inner_w.clamp(8, 90);
        let ch = rows.max(4);
        let mut canvas = gibson::BrailleCanvas::new(cw, ch);
        let t = Transform3::rotation(self.t * 0.9, self.t * 1.3, self.t * 0.4);
        Projector::default().draw(&mesh, &t, &mut canvas, 1.0);
        Node::col()
            .child(Node::text(
                "perspective-projected wireframe, sub-cell Braille lines",
                fx.st.muted,
            ))
            .child(Node::raster(canvas.to_surface(fx.st.accent)))
    }
}

fn apply_capability_flags(ctx: &mut Context, args: &[String]) {
    use gibson::capability::ColorDepth;
    let has = |f: &str| args.iter().any(|a| a == f);
    if has("--mono") {
        ctx.set_color_depth(ColorDepth::Mono);
    } else if has("--ansi16") {
        ctx.set_color_depth(ColorDepth::Ansi16);
    } else if has("--ansi256") {
        ctx.set_color_depth(ColorDepth::Ansi256);
    } else if has("--truecolor") {
        ctx.set_color_depth(ColorDepth::TrueColor);
    } else if has("--no-color") {
        ctx.set_color_depth(ColorDepth::Mono);
    }
    if has("--no-sync") {
        ctx.set_sync_updates(false);
    }
    if has("--no-insert-line") {
        let mut caps = ctx.capabilities();
        caps.insert_line = gibson::Capability::Unsupported;
        ctx.set_capabilities(caps);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    let deterministic = has("--deterministic");
    let freeze_at: Option<usize> = args
        .iter()
        .find_map(|a| a.strip_prefix("--freeze-at=").and_then(|v| v.parse().ok()));
    let auto = deterministic || has("--auto") || has("--scripted");
    let no_color = has("--no-color");
    let debug = has("--debug-renderer");

    let theme = if no_color {
        Theme::no_color()
    } else {
        Theme::default()
    };
    let mut ctx = Context::fullscreen()?;
    apply_capability_flags(&mut ctx, &args);
    ctx.set_max_fps(if auto { 240 } else { 60 });
    ctx.set_animation_interval(Duration::from_millis(if auto { 8 } else { 33 }));
    ctx.set_capture_damage(true);

    let time = if deterministic {
        TimeSource::fixed(Duration::from_millis(16))
    } else {
        TimeSource::real()
    };
    let mut lab = Lab::new(Fx::new(theme, !no_color), debug);
    if debug {
        // Start on the damage scene when explicitly debugging damage.
        lab.scene = 9;
    }
    if let Some(s) = args.iter().find_map(|a| {
        a.strip_prefix("--scene=")
            .and_then(|v| v.parse::<usize>().ok())
    }) {
        // 1-based, matching the keyboard shortcuts shown in the UI.
        lab.scene = s.saturating_sub(1).min(SCENES.len() - 1);
    }

    let mut iterations = 0usize;
    let cap = if auto { 4000 } else { usize::MAX };
    while iterations < cap {
        iterations += 1;
        let frozen = deterministic && freeze_at.is_some_and(|n| iterations > n);
        if !frozen {
            let _ = time.advance();
            lab.tick(&mut ctx);
        }
        let (cols, rows) = ctx.session.terminal_size();
        let stats = ctx.stats();
        let root = Node::col()
            .percent_width(100.0)
            .percent_height(100.0)
            .child(lab.header(&stats, cols))
            .child(
                Node::panel(SCENES[lab.scene], BorderType::Rounded, lab.fx.st.border)
                    .percent_width(100.0)
                    .percent_height(100.0)
                    .child(lab.body(cols, rows)),
            );
        ctx.set_root(root);

        if auto {
            ctx.request_render();
            ctx.run_once(ctx.animation_interval())?;
        } else if let Some(event) = ctx.run_once(Duration::from_millis(40))? {
            use gibson::input::{Event, KeyCode};
            if let Event::Key(k) = event {
                match k.code {
                    KeyCode::Char(c @ '1'..='9') => {
                        lab.scene = (c as usize - '1' as usize).min(SCENES.len() - 1);
                    }
                    KeyCode::Char('0') => lab.scene = SCENES.len() - 1,
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    _ => {}
                }
            }
        }
        if frozen {
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    ctx.restore()?;
    let stats = ctx.stats();
    println!(
        "FX Lab: {} frames, {} dirty cells, {} frame bytes",
        stats.frames, stats.dirty_cells, stats.frame_bytes
    );
    Ok(())
}
