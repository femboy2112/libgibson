//! A second realization of the same BattleGraph. RGB light, depth-tested mass,
//! sparse Braille structure, and ordinary text all end as ordinary Surface cells.
use super::battle::{EncounterModel, NodeId, Outcome};
use super::presentation::{ShotHistory, ShotPlan};
use gibson::raster::{Rgb, RgbRaster};
use gibson::raster3d::{Camera, Fog, Material, RasterStats, Rasterizer, TriangleMesh};
use gibson::raster_fx::{FeedbackBuffer, RasterFx};
use gibson::{BrailleCanvas, Color, Style, Surface, Transform3, Vec3};
use std::{collections::VecDeque, time::Duration};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VisualMode {
    #[default]
    Auto,
    Flat,
    Cyber,
}

const CYAN: Rgb = (36, 213, 255);
const PINK: Rgb = (255, 35, 142);
const DARK: Rgb = (2, 4, 14);

fn mix(a: Rgb, b: Rgb, f: f32) -> Rgb {
    let f = f.clamp(0.0, 1.0);
    let c = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * f) as u8;
    (c(a.0, b.0), c(a.1, b.1), c(a.2, b.2))
}
fn scale(c: Rgb, f: f32) -> Rgb {
    (
        (c.0 as f32 * f) as u8,
        (c.1 as f32 * f) as u8,
        (c.2 as f32 * f) as u8,
    )
}
fn add(a: Rgb, b: Rgb) -> Rgb {
    (
        a.0.saturating_add(b.0),
        a.1.saturating_add(b.1),
        a.2.saturating_add(b.2),
    )
}
fn lerp(a: Vec3, b: Vec3, f: f32) -> Vec3 {
    a.plus(b.minus(a).scale(f))
}
fn smooth(f: f32) -> f32 {
    let f = f.clamp(0.0, 1.0);
    f * f * (3.0 - 2.0 * f)
}

pub fn position(world: &EncounterModel, id: NodeId) -> Vec3 {
    let mut p = [
        Vec3::new(-4.5, 0.0, -3.0),
        Vec3::new(-1.8, 0.0, -1.5),
        Vec3::new(-3.0, 0.0, 1.8),
        Vec3::new(2.2, 0.0, -0.8),
        Vec3::new(0.0, 0.0, 3.0),
        Vec3::new(3.5, 0.0, 5.0),
        Vec3::new(-3.5, 0.0, 5.2),
    ][id.index()];
    if world.graph.node(id).isolated {
        // Severed components physically leave the common floor.
        p.x *= 1.28;
        p.y += 0.9;
        p.z *= 1.12;
    }
    p
}
/// The actor follows reducer progress, never a looping decorative phase.
pub fn actor_position(world: &EncounterModel) -> Vec3 {
    if world.elapsed_ms < 6500 {
        let f = (world.elapsed_ms as f32 / 6500.0).clamp(0.0, 1.0);
        return position(world, NodeId::Modem).plus(Vec3::new(
            -5.0 * (1.0 - f),
            0.7 + 2.0 * (1.0 - f),
            -7.0 * (1.0 - f),
        ));
    }
    let path = &world.planner.path;
    if path.len() < 2 {
        return position(world, world.planner.location).plus(Vec3::new(0.0, 0.6, 0.0));
    }
    let at = world.planner.progress as f32 / 1000.0 * (path.len() - 1) as f32;
    let i = (at as usize).min(path.len() - 2);
    let (a, b) = (path[i], path[i + 1]);
    if !world.graph.connected(a, b) {
        return position(world, world.planner.location).plus(Vec3::new(0.0, 0.6, 0.0));
    }
    lerp(position(world, a), position(world, b), at - i as f32).plus(Vec3::new(0.0, 0.6, 0.0))
}

fn path_point(world: &EncounterModel, reverse: bool, t: f32) -> Option<Vec3> {
    let path = &world.planner.path;
    if path.len() < 2 {
        return None;
    }
    let phase = (t * 0.6).fract();
    let at = (if reverse { 1.0 - phase } else { phase }) * (path.len() - 1) as f32;
    let i = (at as usize).min(path.len() - 2);
    let (a, b) = (path[i], path[i + 1]);
    if !world.graph.connected(a, b) || world.graph.node(a).isolated || world.graph.node(b).isolated
    {
        return None;
    }
    Some(lerp(position(world, a), position(world, b), at - i as f32).plus(Vec3::new(0.0, 0.3, 0.0)))
}
#[derive(Clone, Debug, PartialEq)]
struct LightSample {
    dt: Duration,
    acid: Option<Vec3>,
    crash: Option<Vec3>,
}
/// Bounded, recorded-input visual state. Samples are taken by update, never paint.
/// Reprojection on resize rebuilds feedback from the same world-space history.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualHistory {
    samples: VecDeque<LightSample>,
    entered_at: Option<Duration>,
    pub aftermath: Duration,
    pub shots: ShotHistory,
}
impl VisualHistory {
    pub fn new(world: &EncounterModel) -> Self {
        Self {
            samples: VecDeque::new(),
            aftermath: Duration::ZERO,
            shots: ShotHistory::new(world),
            entered_at: has_foothold(world)
                .then(|| world.visual_time().saturating_sub(Duration::from_secs(2))),
        }
    }
    pub fn update(&mut self, dt: Duration, world: &EncounterModel) {
        if world.outcome.is_some() {
            self.aftermath = self.aftermath.saturating_add(dt);
        }
        if self.entered_at.is_none() && has_foothold(world) {
            self.entered_at = Some(world.visual_time());
        }
        self.shots.update(world, self.aftermath);
        if dt.is_zero() {
            return;
        }
        let t = world.visual_time().as_secs_f32();
        self.samples.push_back(LightSample {
            dt,
            acid: (world.remote_active && world.elapsed_ms >= 1800).then(|| actor_position(world)),
            crash: (world.trace_confidence > 0)
                .then(|| path_point(world, true, t))
                .flatten(),
        });
        // At most 48 update boundaries; wall time never participates.
        while self.samples.len() > 48 {
            self.samples.pop_front();
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Metrics {
    pub pixels: usize,
    pub triangles: RasterStats,
    pub field_samples: usize,
    pub feedback_passes: usize,
}
pub struct CyberFrame {
    pub raster: RgbRaster,
    pub surface: Surface,
    pub metrics: Metrics,
}

fn has_foothold(world: &EncounterModel) -> bool {
    world.graph.nodes[..6]
        .iter()
        .skip(1)
        .any(|n| n.acid_fraction() > 0.65)
        || world.takeover > 0
}
pub fn immersion(
    world: &EncounterModel,
    history: &VisualHistory,
    mode: VisualMode,
    _beat_time: f32,
) -> f32 {
    if mode == VisualMode::Flat {
        return 0.0;
    }
    if mode == VisualMode::Cyber {
        return 1.0;
    }
    if world.outcome.is_some() {
        return 1.0 - smooth((history.aftermath.as_secs_f32() - 2.0) / 2.5);
    }
    history.entered_at.map_or(0.0, |start| {
        smooth(world.visual_time().saturating_sub(start).as_secs_f32() / 2.0)
    })
}

fn glow_dot(raster: &mut RgbRaster, x: f32, y: f32, radius: f32, color: Rgb) {
    for dy in -4..=4 {
        for dx in -4..=4 {
            let px = x as i32 + dx;
            let py = y as i32 + dy;
            if let Some(old) = raster.get(px, py) {
                let d2 = (px as f32 - x).powi(2) + (py as f32 - y).powi(2);
                let power = (-d2 / radius.max(0.1)).exp();
                raster.set(px, py, add(old, scale(color, power)));
            }
        }
    }
}

fn cinematic_forces(
    renderer: &mut Rasterizer,
    camera: &Camera,
    world: &EncounterModel,
    shot: &ShotPlan,
    history: &VisualHistory,
) {
    use super::battle::Tactic;
    use super::presentation::ShotKind;
    let t = world.visual_time().as_secs_f32();
    let actor = actor_position(world);
    // A faceted remote presence, not a label that teleports between widgets.
    if world.remote_active && world.elapsed_ms >= 1800 {
        renderer.draw_mesh(
            &TriangleMesh::octahedron(0.18),
            Transform3 {
                offset: actor,
                ry: t * 1.1,
                rz: t * 0.7,
                ..Transform3::default()
            },
            camera,
            Material {
                color: PINK,
                ambient: 0.5,
                diffuse: 0.4,
                emissive: 0.5,
            },
        );
    }
    // Trace returns along legal topology. Consecutive luminous samples make a
    // coherent beam; its head moves in the reverse direction of Acid's path.
    if world.trace_confidence > 0 && world.outcome != Some(Outcome::Acid) {
        for segment in world.planner.path.windows(2) {
            if !world.graph.connected(segment[0], segment[1]) {
                continue;
            }
            let a = position(world, segment[0]).plus(Vec3::new(0.0, 0.45, 0.0));
            let b = position(world, segment[1]).plus(Vec3::new(0.0, 0.45, 0.0));
            for j in 0..36 {
                let u = j as f32 / 36.0;
                let pulse = (1.0 - ((u + (t * 0.8).fract()).fract())).powi(5);
                let col = scale(CYAN, 0.18 + pulse * 0.82);
                renderer.line(
                    lerp(a, b, u),
                    lerp(a, b, (j + 1) as f32 / 36.0),
                    camera,
                    col,
                );
            }
        }
    }
    // Feints are visibly incomplete: they branch from her position and fade
    // before reaching a node. They never claim to be connected graph edges.
    if world.remote_active && matches!(world.planner.tactic, Tactic::SplitRoute | Tactic::Feint) {
        for branch in 0..3 {
            for j in 0..28 {
                let point = |k: usize| {
                    let u = k as f32 / 28.0;
                    actor.plus(Vec3::new(
                        (branch as f32 - 1.0) * u * 3.5,
                        (u * std::f32::consts::PI).sin() * (0.6 + branch as f32 * 0.2),
                        u * 3.0,
                    ))
                };
                renderer.line(
                    point(j),
                    point(j + 1),
                    camera,
                    scale(PINK, (1.0 - j as f32 / 28.0) * 0.65),
                );
            }
        }
    }
    let rupture = shot.kind == ShotKind::Isolation || shot.kind == ShotKind::CrashWin;
    if rupture {
        let phase = if shot.kind == ShotKind::CrashWin {
            history.aftermath.as_secs_f32()
        } else {
            shot.phase
        };
        let impact = (1.0 - phase / 2.4).clamp(0.0, 1.0);
        for edge in world.graph.edges.iter().filter(|e| !e.connected) {
            let center = lerp(position(world, edge.from), position(world, edge.to), 0.5);
            for j in 0..9 {
                let angle = j as f32 * 2.4;
                let delta = Vec3::new(angle.cos(), (j as f32 * 1.3).sin() * 0.6, angle.sin());
                let start = center.plus(delta.scale(phase * 0.8));
                renderer.line(
                    start,
                    start.plus(delta.scale(0.15 + impact * 0.4)),
                    camera,
                    scale(CYAN, impact),
                );
            }
        }
    }
    if shot.kind == ShotKind::Stalemate {
        // Balanced ownership creates a motionless standing boundary.
        for j in 0..100 {
            let point = |k: usize| {
                let x = k as f32 / 100.0 * 13.0 - 6.5;
                Vec3::new(x, 0.7 + (x * 1.7).sin() * 0.4, 1.4)
            };
            renderer.line(
                point(j),
                point(j + 1),
                camera,
                if j % 2 == 0 { CYAN } else { PINK },
            );
        }
    }
}

/// Pure realization: replay and frozen paints cannot advance the trail buffer.
pub fn render(
    world: &EncounterModel,
    history: &VisualHistory,
    width: u16,
    height: u16,
    mono: bool,
    dive: f32,
) -> CyberFrame {
    render_inner(world, history, width, height, mono, dive, None)
}

pub fn render_shot(
    world: &EncounterModel,
    history: &VisualHistory,
    width: u16,
    height: u16,
    mono: bool,
    shot: &ShotPlan,
) -> CyberFrame {
    render_inner(world, history, width, height, mono, 1.0, Some(shot))
}

#[allow(clippy::too_many_arguments)]
fn render_inner(
    world: &EncounterModel,
    history: &VisualHistory,
    width: u16,
    height: u16,
    mono: bool,
    dive: f32,
    shot: Option<&ShotPlan>,
) -> CyberFrame {
    // Bound demo work independently of hostile terminal dimensions.
    let width = width.clamp(1, 320);
    let height = height.clamp(1, 120);
    let ph = height * 2;
    let t = world.visual_time().as_secs_f32();
    let mut renderer = Rasterizer::new(width, ph);
    renderer.clear(DARK);
    let display = world.graph.node(NodeId::Display);
    let assault = if world.remote_active && !display.isolated {
        smooth((display.acid_fraction() - 0.5) * 2.0) * (world.takeover as f32 / 600.0).min(1.0)
    } else {
        0.0
    };
    let focus = position(world, world.planner.target);
    let camera = Camera {
        position: Vec3::new(
            9.0 + (t * 0.09).sin() * 1.8,
            10.5 - dive * 3.0,
            -15.0 + dive * 3.0 + assault * 1.5,
        ),
        target: Vec3::new(
            focus.x * (0.12 + assault * 0.13),
            0.2 + assault * 1.4,
            1.3 + focus.z * 0.08,
        ),
        fov_y: 0.69,
        near: 0.15,
        far: 80.0,
        ..Camera::default()
    };
    let camera = shot.map_or(camera, |s| s.camera);
    renderer.fog = Some(Fog {
        color: DARK,
        start: 14.0,
        end: 38.0,
    });
    renderer.light = Vec3::new(-0.4, 0.9, -0.6);
    let project = |p| camera.project(p, width, ph);
    let projected = NodeId::ALL.map(|id| project(position(world, id)));
    // Signed graph influence creates the light field. Isolation removes a
    // source from the shared field; its disconnected geometry still exists.
    for y in 0..ph {
        for x in 0..width {
            let mut crash = 0.0f32;
            let mut acid = 0.0f32;
            for node in &world.graph.nodes {
                if node.isolated || (node.decoy && !node.visible) {
                    continue;
                }
                if let Some((px, py, _)) = projected[node.id.index()] {
                    let dx = (x as f32 - px) / ph as f32;
                    let dy = (y as f32 - py) / ph as f32;
                    let k = 0.005 / (dx * dx + dy * dy + 0.012);
                    let f = node.acid_fraction();
                    crash += k * (1.0 - f);
                    acid += k * f;
                }
            }
            let collision = crash.min(acid);
            let c = add(
                DARK,
                add(scale(CYAN, crash * 0.14), scale(PINK, acid * 0.24)),
            );
            // Isosurfaces of the actual signed influence field become luminous
            // waves during DISPLAY possession. Their sources remain graph nodes.
            let total = crash + acid;
            let field_color = mix(CYAN, PINK, acid / total.max(0.001));
            let ridge = ((total * 14.0 - t * 1.2).sin().max(0.0)).powi(12);
            let mass = smooth((total - 0.08) * 2.1);
            let energy = scale(field_color, assault * (mass * 0.15 + ridge * mass * 0.45));
            renderer.raster.set(
                x as i32,
                y as i32,
                scale(
                    add(
                        add(c, energy),
                        scale((230, 185, 255), collision * (0.11 + assault * ridge * 0.25)),
                    ),
                    shot.map_or(1.0, |s| s.field_strength),
                ),
            );
        }
    }
    // A perspective floor provides depth before the first tower is drawn.
    for i in -6..=6 {
        let k = i as f32 * 2.0;
        renderer.line(
            Vec3::new(k, -0.15, -7.0),
            Vec3::new(k, -0.15, 11.0),
            &camera,
            if shot.is_some() {
                (3, 9, 19)
            } else {
                (8, 24, 43)
            },
        );
        renderer.line(
            Vec3::new(-12.0, -0.15, k),
            Vec3::new(12.0, -0.15, k),
            &camera,
            if shot.is_some() {
                (3, 9, 19)
            } else {
                (8, 24, 43)
            },
        );
    }
    let rails = if let Some(shot) = shot {
        super::world_geom::draw(
            &mut renderer,
            &camera,
            world,
            t,
            shot.focal,
            shot.light_strength,
        )
    } else {
        for node in &world.graph.nodes {
            if node.decoy && !node.visible {
                continue;
            }
            let p = position(world, node.id);
            let height = 0.6
                + node.integrity as f32 / 1000.0
                    * [1.1, 1.6, 2.5, 1.2, 1.8, 3.0, 1.8][node.id.index()];
            let fraction = node.acid_fraction();
            let color = if node.isolated {
                (48, 57, 79)
            } else {
                mix(CYAN, PINK, fraction)
            };
            // Stepped architecture, not isolated identical cubes.
            for tier in 0..3 {
                let size = 1.55 - tier as f32 * 0.3;
                let h = height / 3.0;
                let mesh = TriangleMesh::box_xyz(size, h, size);
                renderer.draw_mesh(
                    &mesh,
                    Transform3 {
                        offset: p.plus(Vec3::new(0.0, h * (tier as f32 + 0.5), 0.0)),
                        ry: if node.decoy { -0.25 } else { 0.0 },
                        ..Transform3::default()
                    },
                    &camera,
                    Material {
                        color,
                        ambient: 0.19,
                        diffuse: 0.85,
                        emissive: 0.07,
                    },
                );
            }
            let mesh = TriangleMesh::octahedron(if node.id == NodeId::Display { 1.0 } else { 0.4 });
            renderer.draw_mesh(
                &mesh,
                Transform3 {
                    offset: p.plus(Vec3::new(0.0, height + 0.7, 0.0)),
                    ry: t * (0.3 + fraction * 0.6),
                    rz: t * 0.17,
                    ..Transform3::default()
                },
                &camera,
                Material {
                    color: mix(color, (220, 245, 255), 0.2),
                    ambient: 0.2,
                    diffuse: 0.8,
                    emissive: 0.15,
                },
            );
        }
        Vec::new()
    };
    if assault > 0.0 {
        let center = position(world, NodeId::Display).plus(Vec3::new(0.0, 3.5, 0.0));
        // DISPLAY's plane opens into an orbital lattice as control is lost.
        for orbit in 0..3 {
            let radius = 2.0 + orbit as f32 * 0.5;
            for segment in 0..48 {
                let point = |step: i32| {
                    let a = step as f32 / 48.0 * std::f32::consts::TAU;
                    Vec3::new(a.cos() * radius, 0.0, a.sin() * radius)
                        .rotate_x(0.5 + orbit as f32 * 0.6)
                        .rotate_z(t * 0.2 * (orbit as f32 + 1.0))
                        .plus(center)
                };
                renderer.line(
                    point(segment),
                    point(segment + 1),
                    &camera,
                    scale(PINK, assault * 0.6),
                );
            }
        }
    }
    let mut vectors = BrailleCanvas::new(width, height.max(1));
    for edge in &world.graph.edges {
        if (edge.from == NodeId::Decoy || edge.to == NodeId::Decoy)
            && !world.graph.node(NodeId::Decoy).visible
        {
            continue;
        }
        let a = position(world, edge.from).plus(Vec3::new(0.0, 0.23, 0.0));
        let b = position(world, edge.to).plus(Vec3::new(0.0, 0.23, 0.0));
        let active = world.planner.path.windows(2).any(|p| {
            (p[0] == edge.from && p[1] == edge.to) || (p[1] == edge.from && p[0] == edge.to)
        });
        let color = if active && world.remote_active && world.elapsed_ms >= 3500 {
            PINK
        } else {
            CYAN
        };
        if edge.connected {
            renderer.line(
                a,
                b,
                &camera,
                scale(
                    color,
                    if active { 0.75 } else { 0.38 } * shot.map_or(1.0, |s| s.light_strength),
                ),
            );
            // One fine structure overlay; alternate dotted Acid grammar.
            if let (Some((ax, ay, _)), Some((bx, by, _))) = (project(a), project(b)) {
                for j in 0..20 {
                    if active && j % 2 == 0 {
                        continue;
                    }
                    let u = j as f32 / 20.0;
                    vectors.set(
                        ((ax + (bx - ax) * u) * 2.0) as i32,
                        ((ay + (by - ay) * u) * 2.0) as i32,
                    );
                }
            }
        } else {
            renderer.line(a, lerp(a, b, 0.32), &camera, (80, 70, 85));
            renderer.line(lerp(a, b, 0.68), b, &camera, (80, 70, 85));
            for u in [0.32, 0.68] {
                if let Some((x, y, _)) = project(lerp(a, b, u)) {
                    glow_dot(&mut renderer.raster, x, y, 1.2, (255, 140, 60));
                }
            }
        }
    }
    let mut feedback = FeedbackBuffer::new(width, ph, Duration::from_millis(220));
    let mut input = RgbRaster::new(width, ph);
    for sample in &history.samples {
        input.clear((0, 0, 0));
        for (point, color) in [(sample.acid, PINK), (sample.crash, CYAN)] {
            if let Some((x, y, z)) = point.and_then(project) {
                if renderer
                    .depth(x as i32, y as i32)
                    .is_none_or(|depth| z <= depth + 0.6)
                {
                    glow_dot(&mut input, x, y, 2.2, color);
                }
            }
        }
        let exposure = (sample.dt.as_secs_f32() * 5.0).min(1.0);
        for pixel in input.pixels_mut() {
            *pixel = scale(*pixel, exposure);
        }
        feedback.update(sample.dt, &input);
    }
    for (pixel, light) in renderer
        .raster
        .pixels_mut()
        .iter_mut()
        .zip(feedback.raster().pixels())
    {
        *pixel = add(*pixel, *light);
    }
    // The moving remote actor has the same position as its current legal path.
    if world.remote_active && world.elapsed_ms >= 3500 {
        let point = if shot.is_some() {
            actor_position(world)
        } else {
            path_point(world, false, t).unwrap_or_else(|| position(world, world.planner.location))
        };
        if let Some((x, y, z)) = project(point.plus(Vec3::new(0.0, 0.5, 0.0))) {
            if renderer
                .depth(x as i32, y as i32)
                .is_none_or(|d| z < d + 0.8)
            {
                glow_dot(&mut renderer.raster, x, y, 4.0, PINK);
            }
        }
    }
    if let Some(shot) = shot {
        cinematic_forces(&mut renderer, &camera, world, shot, history);
    }
    let mut rail_vectors = BrailleCanvas::new(width, height);
    let mut rail_colors = vec![CYAN; width as usize * height as usize];
    for (a, b, color) in rails {
        let steps = if let (Some((ax, ay, _)), Some((bx, by, _))) = (project(a), project(b)) {
            ((ax - bx).abs().max((ay - by).abs()) * 3.0)
                .ceil()
                .clamp(2.0, 180.0) as usize
        } else {
            48
        };
        for j in 0..=steps {
            if let Some((x, y, z)) = project(lerp(a, b, j as f32 / steps as f32)) {
                if x >= 0.0
                    && x < width as f32
                    && y >= 0.0
                    && y < ph as f32
                    && renderer
                        .depth(x as i32, y as i32)
                        .is_none_or(|d| z <= d + 0.2)
                {
                    rail_vectors.set((x * 2.0) as i32, (y * 2.0) as i32);
                    rail_colors[(y as usize / 2) * width as usize + x as usize] = color;
                }
            }
        }
    }
    let display = world.graph.node(NodeId::Display);
    let possession = if world.remote_active && !display.isolated {
        display.acid_fraction()
    } else {
        0.0
    };
    // Raster-only distortion: text is composed later and never channel-warped.
    let mut effects = vec![
        RasterFx::Glow {
            radius: 2,
            threshold: 150,
            strength: 0.6,
        },
        RasterFx::Vignette { strength: 0.45 },
    ];
    if possession > 0.85 && (t * 0.5).fract() < 0.12 {
        effects.push(RasterFx::ChromaticSplit { offset: 1 });
        effects.push(RasterFx::SineWarp {
            amplitude: possession * 1.3,
            frequency: 0.2,
            phase: t * 2.0,
        });
    }
    if world.outcome == Some(Outcome::Acid)
        && world.remote_active
        && (shot.is_none() || history.aftermath.as_secs_f32() < 4.5)
    {
        title_fragments(
            &mut renderer.raster,
            t,
            history.aftermath.as_secs_f32(),
            shot.is_some(),
        );
    }
    RasterFx::apply_chain(&mut renderer.raster, &effects);
    let mut surface = if mono {
        renderer.raster.to_mono_surface()
    } else {
        renderer.raster.to_surface()
    };
    // Sparse vector dots retain the RGB layer's local background. They do not
    // cover the picture with blank Braille cells or a rectangular panel.
    if !mono {
        for y in 0..height {
            for x in 0..width {
                if let Some(glyph) = vectors.glyph_at(x, y).filter(|g| *g != '\u{2800}') {
                    let rgb = renderer.raster.get(x as i32, y as i32 * 2).unwrap_or(DARK);
                    surface.print_str(
                        x,
                        y,
                        &glyph.to_string(),
                        Style::new()
                            .fg(Color::Rgb(120, 205, 230))
                            .bg(Color::Rgb(rgb.0, rgb.1, rgb.2)),
                        Some(1),
                    );
                }
            }
        }
    }
    if shot.is_some() {
        for y in 0..height {
            for x in 0..width {
                if let Some(glyph) = rail_vectors.glyph_at(x, y).filter(|g| *g != '\u{2800}') {
                    let color = rail_colors[y as usize * width as usize + x as usize];
                    let upper = renderer.raster.get(x as i32, y as i32 * 2).unwrap_or(DARK);
                    let lower = renderer
                        .raster
                        .get(x as i32, y as i32 * 2 + 1)
                        .unwrap_or(DARK);
                    let bg = mix(upper, lower, 0.5);
                    let style = if mono {
                        Style::new().bold()
                    } else {
                        Style::new()
                            .fg(Color::Rgb(color.0, color.1, color.2))
                            .bg(Color::Rgb(bg.0, bg.1, bg.2))
                    };
                    surface.print_str(x, y, &glyph.to_string(), style, Some(1));
                }
            }
        }
    }
    for node in &world.graph.nodes {
        if node.decoy && !node.visible {
            continue;
        }
        if shot.is_some_and(|s| {
            width < 90 && s.focal.is_some_and(|f| f != node.id) && node.id != world.planner.location
        }) {
            continue;
        }
        if let Some(shot) = shot {
            use super::presentation::LabelPolicy;
            let anchor = shot.focal == Some(node.id) || node.id == world.planner.location;
            let keep = match shot.labels {
                LabelPolicy::All => true,
                LabelPolicy::FocalAndRoute => anchor || world.planner.path.contains(&node.id),
                LabelPolicy::Minimal => anchor,
                LabelPolicy::Scars => node.integrity < 1000 || node.isolated || anchor,
            };
            if !keep {
                continue;
            }
        }
        if let Some((x, y, _)) = project(position(world, node.id)) {
            if shot.is_some()
                && (x < 2.0 || x > width as f32 - 8.0 || y < 0.0 || y > ph as f32 - 4.0)
            {
                continue;
            }
            let label = format!(
                "{}{}",
                if node.isolated {
                    "× "
                } else if node.influence < -350 {
                    "‹ "
                } else if node.influence <= 350 {
                    "≋ "
                } else {
                    ""
                },
                node.id.name().to_ascii_uppercase()
            );
            let x = (x as i32 - label.len() as i32 / 2)
                .clamp(0, width.saturating_sub(label.len() as u16) as i32)
                as u16;
            let y = (y as i32 / 2 + 1).clamp(0, height.saturating_sub(1) as i32) as u16;
            // Reserve the actor's caption before placing subsystem labels.
            // Otherwise sparse labels can concatenate into invented dialogue.
            if shot.is_some() && world.remote_active && world.elapsed_ms >= 3500 {
                if let Some((ax, ay, _)) =
                    project(actor_position(world).plus(Vec3::new(0.0, 0.6, 0.0)))
                {
                    let row = (ay / 2.0) as i32;
                    let caption = world.remote_line.chars().count().max(12) as i32 + 3;
                    if (y as i32 >= row && y as i32 <= row + 1)
                        && (x as i32) < ax as i32 + caption
                        && x as i32 + label.chars().count() as i32 + 3 > ax as i32
                    {
                        continue;
                    }
                }
            }

            surface.print_str(
                x,
                y,
                &label,
                Style::new()
                    .fg(Color::Rgb(170, 225, 245))
                    .bg(Color::Rgb(2, 4, 14)),
                Some(width - x),
            );
        }
    }
    if shot.is_some() && world.remote_active && world.elapsed_ms >= 3500 {
        if let Some((x, y, _)) = project(actor_position(world).plus(Vec3::new(0.0, 0.6, 0.0))) {
            if x >= 1.0 && x < width as f32 - 12.0 && y >= 2.0 && y < ph as f32 - 6.0 {
                let label = if world.elapsed_ms >= 6500 {
                    "‹ ACID BURN"
                } else {
                    "‹ unknown"
                };
                let style = if mono {
                    Style::new().reverse()
                } else {
                    Style::new()
                        .fg(Color::Rgb(255, 147, 215))
                        .bg(Color::Rgb(2, 4, 14))
                };
                surface.print_str(
                    x as u16,
                    (y / 2.0) as u16,
                    label,
                    style,
                    Some(width - x as u16),
                );
                if world.elapsed_ms.saturating_sub(world.remote_line_at_ms) < 3200
                    && !world.remote_line.is_empty()
                    && world.graph.node(NodeId::Display).acid_fraction() <= 0.55
                {
                    surface.print_str(
                        x as u16,
                        (y / 2.0) as u16 + 1,
                        &world.remote_line,
                        style,
                        Some(width - x as u16),
                    );
                }
            }
        }
    }
    let metrics = Metrics {
        pixels: width as usize * ph as usize,
        triangles: renderer.stats,
        field_samples: width as usize * ph as usize * world.graph.nodes.len(),
        feedback_passes: history.samples.len(),
    };
    CyberFrame {
        raster: renderer.raster,
        surface,
        metrics,
    }
}

// Demo-local 5×7 fragments, deliberately not a general font engine.
fn title_fragments(raster: &mut RgbRaster, t: f32, aftermath: f32, from_machine: bool) {
    // Gather illuminated fragments from the actual realized machine. The finale
    // transports those samples into the lettering instead of inventing a cloud.
    let fragments: Vec<_> = if from_machine {
        raster
            .pixels()
            .iter()
            .enumerate()
            .filter(|(_, c)| c.0.max(c.1).max(c.2) > 55)
            .map(|(i, &c)| {
                (
                    (i % raster.width() as usize) as f32,
                    (i / raster.width() as usize) as f32,
                    c,
                )
            })
            .collect()
    } else {
        Vec::new()
    };
    let align = smooth(aftermath / 1.8);
    let letters: [(&str, [u8; 7]); 8] = [
        ("A", [14, 17, 17, 31, 17, 17, 17]),
        ("C", [15, 16, 16, 16, 16, 16, 15]),
        ("I", [31, 4, 4, 4, 4, 4, 31]),
        ("D", [30, 17, 17, 17, 17, 17, 30]),
        ("B", [30, 17, 17, 30, 17, 17, 30]),
        ("U", [17, 17, 17, 17, 17, 17, 14]),
        ("R", [30, 17, 17, 30, 20, 18, 17]),
        ("N", [17, 25, 25, 21, 19, 19, 17]),
    ];
    let size = (raster.width() / 32).clamp(1, 4) as i32;
    let start_x = (raster.width() as i32 - 23 * size) / 2;
    let start_y = (raster.height() as i32 - 17 * size) / 2;
    for (i, (_, rows)) in letters.iter().enumerate() {
        for (y, row) in rows.iter().enumerate() {
            for x in 0..5 {
                if row & (1 << (4 - x)) == 0 {
                    continue;
                }
                let seed = (i * 37 + y * 17 + x as usize * 11) as i32;
                let dx = ((seed % 53 - 26) as f32 * (1.0 - align)) as i32;
                let dy = ((seed % 37 - 18) as f32 * (1.0 - align)) as i32;
                let target_x = start_x + (i % 4) as i32 * 6 * size + x * size;
                let target_y = start_y + (i / 4) as i32 * 10 * size + y as i32 * size;
                let (px, py, base_color) = if let Some(&(fx, fy, color)) =
                    fragments.get((seed as usize * 37) % fragments.len().max(1))
                {
                    (
                        (fx + (target_x as f32 - fx) * align) as i32,
                        (fy + (target_y as f32 - fy) * align) as i32,
                        mix(color, (255, 155, 225), align),
                    )
                } else {
                    (target_x + dx, target_y + dy, (255, 155, 225))
                };
                let shimmer = 0.75 + 0.25 * (t * 1.4 + i as f32 + x as f32 * 0.3).sin();
                for sy in 0..size {
                    for sx in 0..size {
                        raster.set(px + sx, py + sy, scale(base_color, shimmer));
                    }
                }
            }
        }
    }
}

/// Restrained light under the ordinary map, before the camera dive. Uses the
/// same flat topology anchors so the first visitor visibly arrives at MODEM.
pub fn flat_light(
    world: &EncounterModel,
    positions: &[(i32, i32); 7],
    width: u16,
    height: u16,
) -> Surface {
    let mut raster = RgbRaster::new(width, height.saturating_mul(2));
    raster.clear((1, 3, 9));
    for node in &world.graph.nodes {
        if node.isolated || (node.decoy && !node.visible) {
            continue;
        }
        let (x, y) = positions[node.id.index()];
        let color = scale(mix(CYAN, PINK, node.acid_fraction()), 0.12);
        glow_dot(&mut raster, x as f32, y as f32 * 2.0, 7.0, color);
    }
    if world.elapsed_ms >= 3500 && world.remote_active {
        let (x, y) = positions[world.planner.location.index()];
        let arrival = smooth((world.visual_time().as_secs_f32() - 3.5) / 2.0);
        glow_dot(
            &mut raster,
            x as f32 - 3.0 * (1.0 - arrival),
            y as f32 * 2.0 - 4.0 * (1.0 - arrival),
            2.0,
            PINK,
        );
    }
    raster.to_surface()
}
