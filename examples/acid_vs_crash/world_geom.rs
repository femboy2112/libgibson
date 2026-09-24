//! The machine's architecture. Every shape belongs to one persistent graph node;
//! filled faces and emissive rails share the ordinary software depth buffer.
use super::battle::{EncounterModel, NodeId};
use super::cyber::position;
use gibson::raster::Rgb;
use gibson::raster3d::{Camera, Material, Rasterizer, TriangleMesh};
use gibson::{Transform3, Vec3};

const CYAN: Rgb = (37, 205, 255);
const ACID: Rgb = (255, 37, 167);

fn mix(a: Rgb, b: Rgb, amount: f32) -> Rgb {
    let f = amount.clamp(0.0, 1.0);
    let c = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * f) as u8;
    (c(a.0, b.0), c(a.1, b.1), c(a.2, b.2))
}
fn gain(c: Rgb, f: f32) -> Rgb {
    (
        (c.0 as f32 * f) as u8,
        (c.1 as f32 * f) as u8,
        (c.2 as f32 * f) as u8,
    )
}

struct Architecture<'a> {
    renderer: &'a mut Rasterizer,
    rails: &'a mut Vec<(Vec3, Vec3, Rgb)>,
    camera: &'a Camera,
    pose: Transform3,
    color: Rgb,
    brightness: f32,
}
impl Architecture<'_> {
    fn point(&self, p: Vec3) -> Vec3 {
        self.pose.apply(p)
    }
    fn line(&mut self, a: Vec3, b: Vec3, color: Rgb) {
        self.rail(a, b, color);
        self.renderer
            .line(self.point(a), self.point(b), self.camera, color);
    }
    /// Fine rails are returned in world space for the caller's Braille pass.
    /// They share the exact pose of their filled architecture and need normal
    /// depth rejection against the completed world before compositing.
    fn rail(&mut self, a: Vec3, b: Vec3, color: Rgb) {
        self.rails.push((self.point(a), self.point(b), color));
    }
    fn mesh(&mut self, mesh: &TriangleMesh, offset: Vec3, color: Rgb, rotation: f32) {
        self.renderer.draw_mesh(
            mesh,
            Transform3 {
                offset: self.point(offset),
                ry: self.pose.ry + rotation,
                rz: self.pose.rz,
                scale: self.pose.scale,
                ..Transform3::default()
            },
            self.camera,
            Material {
                color: gain(color, self.brightness),
                ambient: 0.2,
                diffuse: 0.7,
                emissive: 0.055,
            },
        );
    }
    fn block(&mut self, offset: Vec3, size: Vec3, color: Rgb) {
        self.mesh(
            &TriangleMesh::box_xyz(size.x, size.y, size.z),
            offset,
            color,
            0.0,
        );
    }
    /// Open vault frame: opaque slim bars, not a fake transparent solid cube.
    fn frame(&mut self, center: Vec3, width: f32, height: f32, color: Rgb) {
        for x in [-width * 0.5, width * 0.5] {
            self.block(
                center.plus(Vec3::new(x, 0.0, 0.0)),
                Vec3::new(0.075, height, 0.1),
                color,
            );
        }
        for y in [-height * 0.5, height * 0.5] {
            self.block(
                center.plus(Vec3::new(0.0, y, 0.0)),
                Vec3::new(width, 0.075, 0.1),
                color,
            );
        }
        let corners = [
            Vec3::new(-width * 0.5, -height * 0.5, -0.056),
            Vec3::new(width * 0.5, -height * 0.5, -0.056),
            Vec3::new(width * 0.5, height * 0.5, -0.056),
            Vec3::new(-width * 0.5, height * 0.5, -0.056),
        ];
        for i in 0..4 {
            self.rail(
                center.plus(corners[i]),
                center.plus(corners[(i + 1) % 4]),
                gain(color, self.brightness),
            );
        }
    }
    fn ring(&mut self, center: Vec3, radius: f32, color: Rgb, phase: f32) {
        // A faceted, solid ribbon. Sixteen segments are intentional at subcell resolution.
        for i in 0..16 {
            let point = |j: usize, radius: f32| {
                let angle = j as f32 * std::f32::consts::TAU / 16.0 + phase;
                center.plus(Vec3::new(angle.cos() * radius, angle.sin() * radius, 0.0))
            };
            let a = self.point(point(i, radius));
            let b = self.point(point(i + 1, radius));
            let c = self.point(point(i + 1, radius - 0.09));
            let d = self.point(point(i, radius - 0.09));
            let material = Material {
                color: gain(color, self.brightness),
                ambient: 0.4,
                diffuse: 0.4,
                emissive: 0.2,
            };
            self.renderer
                .draw_triangle([a, b, c], self.camera, material);
            self.renderer
                .draw_triangle([a, c, d], self.camera, material);
            self.line(
                point(i, radius + 0.006),
                point(i + 1, radius + 0.006),
                gain(color, self.brightness),
            );
        }
    }
}

/// Draw the semantic machine into the caller's depth buffer. No timers, model
/// mutations, terminal state, or independently simulated objects live here.
/// Returned world-space rails let a high-resolution Braille pass refine selected
/// silhouettes without rebuilding or duplicating their geometric definitions.
pub fn draw(
    renderer: &mut Rasterizer,
    camera: &Camera,
    world: &EncounterModel,
    t: f32,
    focal: Option<NodeId>,
    intensity: f32,
) -> Vec<(Vec3, Vec3, Rgb)> {
    let mut rails = Vec::with_capacity(160);
    let intensity = if intensity.is_finite() {
        intensity.clamp(0.0, 1.5)
    } else {
        0.0
    };
    let t = if t.is_finite() { t } else { 0.0 };
    for node in &world.graph.nodes {
        if node.decoy && !node.visible {
            continue;
        }
        let acid = node.acid_fraction().clamp(0.0, 1.0);
        let intact = (node.integrity as f32 / 1000.0).clamp(0.0, 1.0);
        let contested = 1.0 - (acid * 2.0 - 1.0).abs();
        let color = if node.isolated {
            (57, 82, 104)
        } else {
            mix(CYAN, ACID, acid)
        };
        // Foreign influence misaligns architecture; damage removes actual pieces.
        let mut a = Architecture {
            renderer,
            rails: &mut rails,
            camera,
            pose: Transform3 {
                offset: position(world, node.id),
                ry: acid * 0.2 + contested * (t * 1.7).sin() * 0.025,
                rz: contested * (t * 2.1).sin() * 0.012,
                scale: if node.decoy {
                    // Construction uses the recorded action cooldown as an explicit
                    // clock; repeated paints never advance the mirror's appearance.
                    (1.0 - world.cooldowns[2] as f32 / 2500.0).clamp(0.12, 1.0)
                } else {
                    1.0
                },
                ..Transform3::default()
            },
            color,
            brightness: (0.38 + intensity * 0.5)
                * (0.6 + intact * 0.4)
                * if node.isolated { 0.5 } else { 1.0 }
                * if focal == Some(node.id) { 1.35 } else { 1.0 },
        };
        match node.id {
            NodeId::Modem => {
                // An antenna gate marks the only ingress, recognizable even in silhouette.
                a.block(Vec3::new(0.0, 0.1, 0.0), Vec3::new(1.8, 0.2, 0.8), color);
                a.ring(Vec3::new(0.0, 1.15, 0.0), 0.88, color, 0.0);
                a.ring(Vec3::new(0.0, 1.15, 0.22), 0.59, gain(color, 0.65), 0.15);
                a.line(
                    Vec3::new(0.0, 2.0, 0.0),
                    Vec3::new(0.0, 2.65, 0.0),
                    gain(color, 1.1),
                );
                a.line(Vec3::new(-0.35, 2.4, 0.0), Vec3::new(0.35, 2.4, 0.0), color);
            }
            NodeId::Route => {
                // Switching prism, lifted above the paths that physically meet beneath it.
                a.block(Vec3::new(0.0, 0.2, 0.0), Vec3::new(1.8, 0.35, 1.5), color);
                a.mesh(
                    &TriangleMesh::octahedron(0.9),
                    Vec3::new(0.0, 1.15, 0.0),
                    color,
                    acid * t * 0.09,
                );
                for x in [-0.6, 0.6] {
                    a.line(
                        Vec3::new(x, 0.4, -0.65),
                        Vec3::new(x, 1.9, -0.65),
                        gain(color, 1.2),
                    );
                }
                a.ring(
                    Vec3::new(0.0, 1.15, -0.02),
                    1.05,
                    gain(color, 0.55),
                    std::f32::consts::FRAC_PI_4,
                );
            }
            NodeId::Auth => {
                // Nested depth frames form a vault corridor. The infection reaches the
                // outer frame before the protected inner core, from the same influence.
                for depth in 0..4 {
                    if intact < 0.7 && depth == 1 {
                        continue;
                    }
                    let s = 1.0 - depth as f32 * 0.12;
                    let infected = (acid * 1.7 - depth as f32 * 0.22).clamp(0.0, 1.0);
                    a.frame(
                        Vec3::new(0.0, 1.4, depth as f32 * 0.42 - 0.65),
                        2.25 * s,
                        2.5 * s,
                        mix(CYAN, ACID, infected),
                    );
                }
                a.mesh(
                    &TriangleMesh::octahedron(0.55),
                    Vec3::new(0.0, 1.4, 0.15),
                    mix(CYAN, ACID, (acid - 0.5) * 2.0),
                    0.0,
                );
                for x in [-1.12, 1.12] {
                    a.line(
                        Vec3::new(x, 0.15, -0.7),
                        Vec3::new(x * 0.6, 0.55, 0.65),
                        gain(color, 0.8),
                    );
                }
            }
            NodeId::Shell => {
                // A sloping console has a different mass from the vault or storage slabs.
                let mut console = TriangleMesh::box_xyz(1.65, 1.15, 1.45);
                for p in &mut console.vertices {
                    if p.y > 0.0 {
                        p.y += p.z * 0.45;
                    }
                }
                a.mesh(&console, Vec3::new(0.0, 0.8, 0.0), color, 0.0);
                a.frame(Vec3::new(0.0, 1.75, 0.45), 1.35, 0.85, color);
                for row in 0..3 {
                    let y = 1.55 + row as f32 * 0.17;
                    a.line(
                        Vec3::new(-0.45, y, 0.38),
                        Vec3::new(0.25 + row as f32 * 0.08, y, 0.38),
                        gain(color, 0.8),
                    );
                }
            }
            NodeId::Files | NodeId::Decoy => {
                let mirror = if node.decoy { -1.0 } else { 1.0 };
                for slab in 0..5 {
                    if slab == 3 && intact < 0.75 {
                        continue;
                    }
                    let y = 0.22 + slab as f32 * 0.42;
                    let shift = (slab as f32 - 2.0) * acid * 0.08;
                    a.mesh(
                        &TriangleMesh::box_xyz(1.65, 0.2, 1.1),
                        Vec3::new(shift, y, 0.0),
                        color,
                        mirror * 0.1,
                    );
                    a.line(
                        Vec3::new(-0.6, y + 0.11, -0.55),
                        Vec3::new(0.6, y + 0.11, -0.55),
                        gain(color, 1.1),
                    );
                }
                if node.decoy {
                    a.frame(Vec3::new(0.0, 1.1, -0.7), 2.0, 2.5, gain(color, 0.7));
                    a.frame(Vec3::new(0.0, 1.1, 0.7), 2.0, 2.5, gain(color, 0.45));
                }
            }
            NodeId::Display => display(&mut a, world, acid, intact, t),
        }
        if intact < 0.9 {
            for shard in 0..4 {
                let angle = shard as f32 * 1.7 + node.id.index() as f32;
                let radius = 0.9 + (1.0 - intact) * 1.8;
                let p = Vec3::new(
                    angle.cos() * radius,
                    0.3 + shard as f32 * 0.22,
                    angle.sin() * radius,
                );
                a.mesh(
                    &TriangleMesh::octahedron(0.065 + (1.0 - intact) * 0.12),
                    p,
                    gain(color, 0.7),
                    angle,
                );
            }
        }
        if node.isolated {
            // A broken local boundary is visible without the event log. The node has
            // already moved away from the floor through cyber::position.
            for side in [-1.0, 1.0] {
                a.line(
                    Vec3::new(side * 1.25, -0.2, -0.8),
                    Vec3::new(side * 1.25, 1.8, -0.8),
                    (70, 155, 190),
                );
                a.line(
                    Vec3::new(side * 0.9, -0.2, -0.8),
                    Vec3::new(side * 1.4, -0.2, -0.8),
                    (220, 149, 68),
                );
            }
            let remaining = world.cooldowns[1].max(world.cooldowns[4].min(800));
            if remaining > 0 {
                let wave = 1.0 - remaining as f32 / 800.0;
                a.ring(
                    Vec3::new(0.0, 0.9, -0.5),
                    1.0 + wave * 1.4,
                    gain(CYAN, 1.0 - wave),
                    0.0,
                );
            }
        }
        if world.cooldowns[3] > 0
            && world.last_action.starts_with("KILL SESSION")
            && world.last_action.contains(node.id.name())
        {
            let collapse = world.cooldowns[3] as f32 / 1800.0;
            a.ring(
                Vec3::new(0.0, 1.0, -0.7),
                0.1 + collapse * 1.4,
                gain(ACID, collapse),
                t * 0.1,
            );
        }
    }
    rails
}

fn display(a: &mut Architecture<'_>, world: &EncounterModel, acid: f32, intact: f32, t: f32) {
    let color = a.color;
    // Portal is a real opaque surface in depth, with one level of machine/UI
    // engraving. It never recursively invokes the current terminal renderer.
    a.block(
        Vec3::new(0.0, 1.8, 0.18),
        Vec3::new(3.8, 3.2 * (0.8 + intact * 0.2), 0.12),
        gain(color, 0.13),
    );
    a.frame(Vec3::new(0.0, 1.8, 0.0), 4.0, 3.4, color);
    a.frame(Vec3::new(0.0, 1.8, 0.4), 4.25, 3.65, gain(color, 0.55));
    // Prompt chevron + baseline: the sharp control island exists inside DISPLAY.
    let ui = mix(CYAN, (220, 243, 255), 0.3);
    a.line(
        Vec3::new(-1.55, 0.52, -0.025),
        Vec3::new(-1.35, 0.42, -0.025),
        ui,
    );
    a.line(
        Vec3::new(-1.35, 0.42, -0.025),
        Vec3::new(-1.55, 0.32, -0.025),
        ui,
    );
    a.line(
        Vec3::new(-1.15, 0.34, -0.025),
        Vec3::new(1.5, 0.34, -0.025),
        gain(ui, 0.75),
    );
    let points = [
        (-1.2, 2.8),
        (-0.55, 2.3),
        (-1.1, 1.55),
        (0.5, 2.15),
        (0.1, 1.25),
        (1.2, 0.9),
        (-1.2, 0.9),
    ];
    for edge in &world.graph.edges {
        if !edge.connected || (edge.to == NodeId::Decoy && !world.graph.node(NodeId::Decoy).visible)
        {
            continue;
        }
        let p = |id: NodeId| {
            let (x, y) = points[id.index()];
            Vec3::new(x, y, -0.03)
        };
        let invasion = (world.graph.node(edge.from).acid_fraction()
            + world.graph.node(edge.to).acid_fraction())
            * 0.5;
        a.line(
            p(edge.from),
            p(edge.to),
            gain(mix(CYAN, ACID, invasion), 0.7),
        );
    }
    // A moving scan is restricted to DISPLAY and follows its actual influence.
    if acid > 0.25 {
        let y = 0.7 + (t * 0.45).fract() * 2.3;
        a.line(
            Vec3::new(-1.75, y, -0.06),
            Vec3::new(1.75, y, -0.06),
            gain(ACID, acid),
        );
    }
    let trace = world.trace_confidence as f32 / 1000.0;
    a.line(
        Vec3::new(-1.55, 3.15, -0.03),
        Vec3::new(-1.55 + trace * 3.1, 3.15, -0.03),
        CYAN,
    );
}
