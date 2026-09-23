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

use gibson::ansi::AnsiCompiler;
use gibson::cell::{Color, Line, RichText, Span, Style, Theme, ThemeStyles};
use gibson::diff::compute_diff;
use gibson::node::{Node, WrapMode};
use gibson::scene::{Effect, Presentation, Scene, SceneEntity, SceneTarget};
use gibson::story::{Beat, Condition, Story, StoryAction, StoryEvent};
use gibson::surface::{BorderType, Rect, Surface};
use gibson::{
    BrailleCanvas, Context, Mesh, Projector, TimeSource, Transform3, Vec3, ViewportState,
};

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
    "Near-plane clipping",
    "Wide-glyph clip boundary",
    "Logical damage vs wire cost",
    "Mono ordered dithering",
    "Gibson data city",
    "Packet routes",
    "Water particles",
    "Scene algebra: sequence vs parallel",
    "Story graph: branch & rejoin",
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

    fn header(&self, stats: &gibson::RenderStats) -> Node {
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
        if self.debug {
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
            9 => {
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
                            "logical damage cells over {} distinct positions",
                            self.damage.len()
                        ),
                        fx.st.muted,
                    ))
                    .child(Node::raster(canvas.to_surface(fx.st.warning)))
            }
            10 => near_plane_clip_scene(fx, inner_w, content_rows, self.t),
            11 => wide_clip_scene(fx, inner_w),
            12 => logical_vs_wire_scene(fx),
            13 => mono_dither_scene(fx, inner_w, content_rows, self.t),
            14 => data_city_scene(fx, inner_w, content_rows, self.t),
            15 => packet_routes_scene(fx, inner_w, content_rows, self.t),
            16 => water_particles_scene(fx, inner_w, content_rows, self.t),
            17 => scene_algebra_scene(fx, inner_w, content_rows, self.t),
            _ => story_graph_scene(fx, inner_w, content_rows, self.t),
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

fn bright(fx: &Fx) -> Style {
    if fx.color {
        fx.st.accent
    } else {
        Style::default().bold()
    }
}

fn dim(fx: &Fx) -> Style {
    if fx.color {
        fx.st.muted
    } else {
        Style::default().dim()
    }
}

fn near_plane_clip_scene(fx: &Fx, inner_w: u16, rows: u16, t: f32) -> Node {
    let cw = inner_w.clamp(8, 90);
    let ch = rows.max(5);
    let proj = Projector {
        camera_z: 3.0,
        near: 0.6,
    };
    let mesh = Mesh::box_xyz(1.6, 1.6, 1.6);
    let mut canvas = BrailleCanvas::new(cw, ch);
    // Push the box toward the camera so its front face sits behind the near
    // plane. Crossing edges must render their visible truncated part.
    let tf = Transform3 {
        rx: 0.5,
        ry: t * 0.6,
        rz: 0.0,
        scale: 1.0,
        offset: Vec3::new(0.0, 0.0, 2.4),
    };
    proj.draw(&mesh, &tf, &mut canvas, 1.0);
    Node::col()
        .child(Node::text(
            format!(
                "near plane z={:.2} (inclusive) · crossing edges are truncated, never dropped",
                proj.camera_z - proj.near
            ),
            fx.st.muted,
        ))
        .child(Node::raster(canvas.to_surface(bright(fx))))
}

fn wide_clip_scene(fx: &Fx, inner_w: u16) -> Node {
    let w = inner_w.clamp(12, 60);
    let style = if fx.color {
        fx.st.text
    } else {
        Style::default()
    };
    let letters = "abcdefghij".repeat((w as usize / 10) + 1);
    let mut base = Surface::new(w, 2);
    base.print_str(0, 0, &letters, style, Some(w));
    base.print_str(0, 1, &"-".repeat(w as usize), dim(fx), Some(w));

    let mut layer = Surface::new_transparent(w, 2);
    // Lead lands on the final clip column: suppressed (no continuation leak).
    layer.print_str(w.saturating_sub(1), 0, "你", style, None);
    // Fully inside the clip: placed normally.
    layer.print_str(2, 1, "你", style, None);
    base.blit_transparent_clipped(&layer, 0, 0, Rect::new(0, 0, w, 2));

    Node::col()
        .child(Node::text(
            "top: wide lead on the final clip column is suppressed, base untouched",
            fx.st.muted,
        ))
        .child(Node::text(
            "bottom: a wide glyph that fits is placed with its continuation",
            fx.st.muted,
        ))
        .child(Node::raster(base))
}

fn logical_vs_wire_scene(fx: &Fx) -> Node {
    let cols = 60u16;
    let mut long = Surface::new(cols, 1);
    long.print_str(0, 0, &"#".repeat(cols as usize), Style::default(), None);
    let blank = Surface::new(cols, 1);
    let erase = compute_diff(Some(&long), &blank);
    let explicit = erase.explicit_dirty_count();
    let logical = erase.logical_dirty_count();
    let mut comp = AnsiCompiler::new();
    let wire = comp.compile(&erase).len();

    let mut short = Surface::new(cols, 1);
    short.print_str(0, 0, "status: ok", Style::default(), None);
    let shrink = compute_diff(Some(&long), &short);
    let mut comp2 = AnsiCompiler::new();
    let wire2 = comp2.compile(&shrink).len();

    Node::col()
        .child(Node::text(
            format!(
                "erase whole row  →  logical {:>2}  explicit {:>2}  wire {:>2} B",
                logical, explicit, wire
            ),
            fx.st.text,
        ))
        .child(Node::text(
            format!(
                "shrink to 'status: ok'  →  logical {:>2}  explicit {:>2}  wire {:>2} B",
                shrink.logical_dirty_count(),
                shrink.explicit_dirty_count(),
                wire2
            ),
            fx.st.text,
        ))
        .child(Node::text(
            "logical damage counts erased visible cells; wire cost counts emitted bytes",
            fx.st.muted,
        ))
}

fn mono_dither_scene(fx: &Fx, inner_w: u16, rows: u16, t: f32) -> Node {
    let cw = inner_w.clamp(8, 90);
    let ch = (rows.saturating_sub(2) / 2).max(3);
    let style = if fx.color {
        fx.st.accent
    } else {
        Style::default()
    };
    let mut dither = BrailleCanvas::new(cw, ch);
    gibson::render_field_braille_dithered(&mut dither, t, 0.3, 5.0, 1.0);
    let mut hard = BrailleCanvas::new(cw, ch);
    gibson::render_field_braille(&mut hard, t, 0.3, 0.5, 5.0);
    Node::col()
        .child(Node::text("ordered 4x4 Bayer dither", fx.st.muted))
        .child(Node::raster(dither.to_surface(style)))
        .child(Node::text(
            "hard threshold (collapses to solid)",
            fx.st.muted,
        ))
        .child(Node::raster(hard.to_surface(dim(fx))))
}

/// A reusable "Gibson data city": perspective circuit plane, data towers and a
/// central rotating core. Near edges are drawn bright, far edges dim, using the
/// projector's depth output (no z-buffer, no occlusion claim).
fn data_city(fx: &Fx, cw: u16, ch: u16, t: f32) -> Node {
    let mut mesh = Mesh::grid_xz(6.0, 6.0, 12);
    let towers: &[(f32, f32, f32, f32, f32)] = &[
        (-4.0, -3.0, 0.8, 2.2, 0.8),
        (-2.0, 2.0, 1.0, 3.4, 1.0),
        (1.0, -2.0, 0.9, 2.8, 0.9),
        (3.5, 2.5, 1.2, 4.0, 1.2),
        (0.0, 4.0, 0.7, 1.8, 0.7),
        (-4.5, 3.5, 0.6, 2.6, 0.6),
    ];
    for &(x, z, w, h, d) in towers {
        mesh.append(&Mesh::data_tower(x, z, w, h, d));
    }
    mesh.append(&Mesh::octahedron(0.7).translated(Vec3::new(0.0, 2.8, 0.0)));

    let tf = Transform3 {
        rx: -0.75,
        ry: t * 0.22,
        rz: 0.0,
        scale: 1.0,
        offset: Vec3::new(0.0, -1.1, 0.0),
    };
    let pw = (cw as f32) * 2.0;
    let ph = (ch as f32) * 4.0;
    let edges = Projector::default().project_mesh(&mesh, &tf, pw, ph, 1.0);
    let mut depths: Vec<f32> = edges.iter().map(|e| e.depth).collect();
    depths.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = depths.get(depths.len() / 2).copied().unwrap_or(0.0);

    let mut far = BrailleCanvas::new(cw, ch);
    let mut near = BrailleCanvas::new(cw, ch);
    for e in &edges {
        if e.depth <= median {
            near.line(e.a.0, e.a.1, e.b.0, e.b.1);
        } else {
            far.line(e.a.0, e.a.1, e.b.0, e.b.1);
        }
    }
    let mut layer = Surface::new_transparent(cw, ch);
    far.paint_into(&mut layer, (0, 0), dim(fx));
    near.paint_into(&mut layer, (0, 0), bright(fx));
    Node::raster(layer)
}

fn data_city_scene(fx: &Fx, inner_w: u16, rows: u16, t: f32) -> Node {
    let cw = inner_w.clamp(8, 100);
    let ch = rows.max(5);
    Node::col()
        .child(Node::text(
            "near edges bright, far edges dim · towers + circuit plane + core",
            fx.st.muted,
        ))
        .child(data_city(fx, cw, ch, t))
}

fn packet_routes_scene(fx: &Fx, inner_w: u16, rows: u16, t: f32) -> Node {
    let cw = inner_w.clamp(8, 100);
    let ch = rows.max(5);
    let pw = (cw as f32) * 2.0;
    let ph = (ch as f32) * 4.0;
    let nodes = [
        (0.12, 0.25),
        (0.38, 0.12),
        (0.72, 0.28),
        (0.88, 0.62),
        (0.58, 0.85),
        (0.22, 0.72),
    ];
    let pos = |i: usize| (nodes[i].0 * pw, nodes[i].1 * ph);
    let routes = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 4),
        (4, 5),
        (5, 0),
        (0, 3),
        (2, 5),
    ];
    let mut canvas = BrailleCanvas::new(cw, ch);
    for &(a, b) in &routes {
        let (x0, y0) = pos(a);
        let (x1, y1) = pos(b);
        canvas.line(x0 as i32, y0 as i32, x1 as i32, y1 as i32);
    }
    // Deterministic packet dots travelling each route.
    for (k, &(a, b)) in routes.iter().enumerate() {
        let (x0, y0) = pos(a);
        let (x1, y1) = pos(b);
        for j in 0..3 {
            let phase = (t * 0.55 + k as f32 * 0.13 + j as f32 * 0.34).fract();
            let x = x0 + (x1 - x0) * phase;
            let y = y0 + (y1 - y0) * phase;
            canvas.set(x.round() as i32, y.round() as i32);
            canvas.set(x.round() as i32 + 1, y.round() as i32);
        }
    }
    Node::col()
        .child(Node::text(
            "network graph + moving packet particles along fixed routes",
            fx.st.muted,
        ))
        .child(Node::raster(canvas.to_surface(bright(fx))))
}

fn water_particles_scene(fx: &Fx, inner_w: u16, rows: u16, t: f32) -> Node {
    let cw = inner_w.clamp(8, 100);
    let ch = rows.max(5);
    let cycle = 2.0f32;
    let local = (t % cycle) / cycle;
    let mut ps = gibson::ParticleSystem::new(0xBEEF_1234);
    ps.burst_directional(
        220,
        cw as f32 * 0.5,
        0.0,
        12.0,
        cycle,
        std::f32::consts::FRAC_PI_2,
        1.6,
    );
    ps.update(local * cycle);
    let mut canvas = BrailleCanvas::new(cw, ch);
    ps.render_braille(&mut canvas);
    Node::col()
        .child(Node::text(
            "directional burst (rooftop pool) spilling down the viewport",
            fx.st.muted,
        ))
        .child(Node::raster(canvas.to_surface(bright(fx))))
}

/// Scene algebra demo: one entity driven by `sequence` (composition), one by
/// `parallel` (monoidal product). The entities are ordinary `Node`s; the effects
/// only write presentation channels, and the result renders through the normal
/// pipeline via the `Render : SCENE → UI` functor.
fn scene_algebra_scene(fx: &Fx, inner_w: u16, rows: u16, t: f32) -> Node {
    let w = inner_w.clamp(24, 100) as f32;
    let h = rows.max(6) as f32;
    let mut scene = Scene::new();
    let seq = scene.add(SceneEntity::new(
        "seq",
        Node::text("◈", fx.st.accent).width(2.0).height(1.0),
    ));
    let par = scene.add(SceneEntity::new(
        "par",
        Node::text("◆", fx.st.warning).width(2.0).height(1.0),
    ));

    let period = Duration::from_millis(2400);
    let half = Duration::from_millis(1200);
    let seq_eff = Effect::sequence([
        Effect::translate(SceneTarget::Id(seq), (2.0, 1.0), (w - 4.0, 1.0), half),
        Effect::translate(
            SceneTarget::Id(seq),
            (w - 4.0, 1.0),
            (w - 4.0, h - 2.0),
            half,
        ),
    ]);
    let par_eff = Effect::parallel([
        Effect::translate(
            SceneTarget::Id(par),
            (2.0, h - 2.0),
            (w - 4.0, h - 2.0),
            period,
        ),
        Effect::reveal(SceneTarget::Id(par), 0.25, 1.0, period),
    ]);

    let local = (t % 2.4) / 2.4;
    let local_dur = Duration::from_secs_f32(local * 2.4);
    let mut p = Presentation::new();
    seq_eff.eval(local_dur, &scene, &mut p);
    par_eff.eval(local_dur, &scene, &mut p);
    let overlay = scene.to_node(&p, w, h);

    let caption = format!(
        "sequence ◈ = (right ; down)   ·   parallel ◆ = (right ⊗ fade)   ·   phase {:.0}%",
        local * 100.0
    );
    Node::col()
        .child(Node::text(caption, fx.st.muted))
        .child(overlay)
}

/// A small tactical StoryGraph used by the story-graph lab scene.
fn lab_tactical_story() -> Story {
    Story::new("grand-central")
        .beat(
            Beat::new("grand-central")
                .transition(Condition::user("crew"), "crew")
                .transition(Condition::user("pool"), "pool")
                .after(Duration::from_millis(1400), "download"),
        )
        .beat(
            Beat::new("pool")
                .on_enter(StoryAction::set_bool("pool-distraction", true))
                .after(Duration::ZERO, "download"),
        )
        .beat(
            Beat::new("crew")
                .on_enter(StoryAction::set_bool("crew-online", true))
                .after(Duration::ZERO, "download"),
        )
        .beat(Beat::new("download").terminal())
}

/// Story-graph demo: replays a deterministic session in which the crew branch is
/// chosen, then shows the live beat, facts and mounted bundles. Branch and rejoin
/// are real story-graph transitions, not a `match` on a selection index.
fn story_graph_scene(fx: &Fx, _inner_w: u16, rows: u16, t: f32) -> Node {
    let story = lab_tactical_story();
    let mut d = story.start();
    let mut elapsed_ms: u64 = 0;
    let mut chosen = false;
    let cap_ms = ((t * 1000.0) as u64).min(8000);
    while elapsed_ms < cap_ms && !d.is_finished() {
        elapsed_ms += 16;
        let evs: Vec<StoryEvent> = if elapsed_ms >= 900 && !chosen {
            chosen = true;
            vec![StoryEvent::user_selected("crew")]
        } else {
            Vec::new()
        };
        d.update(Duration::from_millis(16), &evs);
    }

    let mut rt = RichText::new();
    rt = rt.line(Line::new().span(Span::styled(
        format!(
            "beat {}   ·   t {:.2}s",
            d.current_beat(),
            d.elapsed().as_secs_f32()
        ),
        fx.st.accent,
    )));
    rt = rt.line(Line::new().span(Span::styled(
        "grand-central ─┬─ pool ─┐",
        if d.current_beat() == "pool" || d.facts().bool("pool-distraction") {
            fx.st.warning
        } else {
            fx.st.muted
        },
    )));
    rt = rt.line(Line::new().span(Span::styled(
        "               └─ crew ─┴─→ download",
        if d.facts().bool("crew-online") || d.current_beat() == "crew" {
            fx.st.success
        } else {
            fx.st.muted
        },
    )));
    for (k, v) in d.facts().iter() {
        rt = rt.line(Line::new().span(Span::styled(format!("fact {k} = {v:?}"), fx.st.muted)));
    }
    for m in d.mounted() {
        rt = rt.line(Line::new().span(Span::styled(format!("mounted {m}"), fx.st.muted)));
    }
    rt = rt.line(Line::new().span(Span::styled(
        format!("trace: {}", d.trace().beat_sequence().join(" > ")),
        fx.st.text,
    )));
    let _ = rows;
    Node::panel("STORY GRAPH", BorderType::Rounded, fx.st.border)
        .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
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
    let debug_damage = has("--debug-damage");
    let mut lab = Lab::new(Fx::new(theme, !no_color), debug);
    if debug {
        // Start on the damage scene when explicitly debugging damage.
        lab.scene = 9;
    }
    if debug_damage {
        // Logical damage vs wire cost is the most direct damage comparison.
        lab.scene = 12;
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
            .child(lab.header(&stats))
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
                    KeyCode::Char('0') => lab.scene = 9,
                    KeyCode::Right | KeyCode::Char('n') | KeyCode::Char(']') => {
                        lab.scene = (lab.scene + 1) % SCENES.len();
                    }
                    KeyCode::Left | KeyCode::Char('p') | KeyCode::Char('[') => {
                        lab.scene = (lab.scene + SCENES.len() - 1) % SCENES.len();
                    }
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
