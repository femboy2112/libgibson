//! The introduction's information city. Pure world-space rendering: buildings,
//! mounted receipts and couriers share the same coordinates and camera. No state
//! advances during paint; scrubbing to a time reconstructs the same frame.
use super::identity::IDENTITIES;
use super::model::{AGENTS, MESSAGES};
use super::shots;
use gibson::geom::{CubicPath3, Transform3, Vec3};
use gibson::raster::{Rgb, RgbRaster};
use gibson::raster3d::{Camera, Fog, Material, Rasterizer, TriangleMesh};
use gibson::raster_fx::RasterFx;
use gibson::{Cell, Color, ColorDepth, Glyph, Style, Surface};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy)]
struct Building {
    position: Vec3,
    height: f32,
}
const BUILDINGS: [Building; 4] = [
    Building {
        position: Vec3 {
            x: -4.,
            y: 0.,
            z: 0.,
        },
        height: 5.2,
    },
    Building {
        position: Vec3 {
            x: 4.,
            y: 0.,
            z: 1.,
        },
        height: 4.1,
    },
    Building {
        position: Vec3 {
            x: -7.,
            y: 0.,
            z: 7.,
        },
        height: 6.3,
    },
    Building {
        position: Vec3 {
            x: 7.,
            y: 0.,
            z: 8.,
        },
        height: 4.8,
    },
];

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}
fn mix(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a.scale(1. - t).plus(b.scale(t))
}
fn shade(c: Rgb, gain: f32) -> Rgb {
    (
        (c.0 as f32 * gain).min(255.) as u8,
        (c.1 as f32 * gain).min(255.) as u8,
        (c.2 as f32 * gain).min(255.) as u8,
    )
}
fn safe_time(seconds: f32) -> f32 {
    if seconds.is_finite() {
        seconds.clamp(0., 72.)
    } else {
        0.
    }
}

/// A curved dolly between deliberately held compositions. Camera and target
/// share endpoints at every cue boundary; courier follow is an additive framing
/// influence with zero weight at departure/arrival, not a second camera mode.
pub fn camera(seconds: f32, width: u16, height: u16) -> Camera {
    let seconds = safe_time(seconds);
    let keys = [
        (28., Vec3::new(0., 6., -23.), Vec3::new(0., 2.8, 4.), 0.88),
        (32., Vec3::new(12., 8.5, -20.), Vec3::new(0., 2.8, 4.), 0.85),
        (35., Vec3::new(-9., 6.5, -17.), Vec3::new(-1., 3., 3.), 0.85),
        (
            36.55,
            Vec3::new(-3.4, 4.4, -9.5),
            Vec3::new(-4., 3.1, 0.),
            0.78,
        ),
        (
            37.6,
            Vec3::new(-3.4, 4.4, -9.5),
            Vec3::new(-4., 3.1, 0.),
            0.78,
        ),
        (
            38.55,
            Vec3::new(4.6, 4.3, -10.3),
            Vec3::new(4., 3.3, 1.),
            0.78,
        ),
        (
            39.6,
            Vec3::new(4.6, 4.3, -10.3),
            Vec3::new(4., 3.3, 1.),
            0.78,
        ),
        (
            40.55,
            Vec3::new(-7.5, 4.6, -3.4),
            Vec3::new(-7., 3.5, 7.),
            0.78,
        ),
        (
            41.6,
            Vec3::new(-7.5, 4.6, -3.4),
            Vec3::new(-7., 3.5, 7.),
            0.78,
        ),
        (
            42.55,
            Vec3::new(7.6, 4.2, -1.5),
            Vec3::new(7., 2.9, 8.),
            0.78,
        ),
        (
            43.45,
            Vec3::new(7.6, 4.2, -1.5),
            Vec3::new(7., 2.9, 8.),
            0.78,
        ),
        (44., Vec3::new(-9., 10., -13.), Vec3::new(0., 5., 3.), 0.86),
        (47., Vec3::new(8., 11., -12.), Vec3::new(0., 5., 4.), 0.86),
        (50., Vec3::new(13., 12., -6.), Vec3::new(0., 5., 5.), 0.86),
        (53., Vec3::new(14., 12., -17.), Vec3::new(0., 2.8, 4.), 0.88),
        (56., Vec3::new(6., 35., -34.), Vec3::new(0., 0., 4.), 0.91),
        (60., Vec3::new(5., 50., -57.), Vec3::new(0., 0., 4.), 0.95),
    ];
    let mut position = keys[0].1;
    let mut target = keys[0].2;
    let mut fov_y = keys[0].3;
    for (index, pair) in keys.windows(2).enumerate() {
        if seconds < pair[0].0 {
            break;
        }
        let t = smooth((seconds - pair[0].0) / (pair[1].0 - pair[0].0));
        let displacement = pair[1].1.plus(pair[0].1.scale(-1.));
        let turn = Vec3::new(-displacement.z, 0., displacement.x).scale(0.11);
        let lift = Vec3::new(0., displacement.length() * 0.055, 0.);
        let path = CubicPath3 {
            start: pair[0].1,
            control1: pair[0]
                .1
                .plus(displacement.scale(0.32))
                .plus(turn)
                .plus(lift),
            control2: pair[0]
                .1
                .plus(displacement.scale(0.68))
                .plus(turn)
                .plus(lift),
            end: pair[1].1,
        };
        position = path.sample(t);
        let next = keys[(index + 2).min(keys.len() - 1)].2;
        target = CubicPath3 {
            start: pair[0].2,
            control1: mix(pair[0].2, pair[1].2, 0.33),
            control2: mix(pair[0].2, pair[1].2, 0.72)
                .plus(next.plus(pair[1].2.scale(-1.)).scale(t * (1. - t) * 0.03)),
            end: pair[1].2,
        }
        .sample(t);
        fov_y = pair[0].3 * (1. - t) + pair[1].3 * t;
    }
    let base_position = position;
    let base_target = target;
    for message in MESSAGES
        .iter()
        .filter(|m| seconds >= m.depart && seconds < m.arrive)
    {
        let phase = (seconds - message.depart) / (message.arrive - message.depart);
        let path = route(message.from, message.to);
        let point = path.sample(phase);
        let tangent = path.tangent(phase);
        let lookahead = path.sample((phase + 0.13).min(1.));
        let follow = point.plus(tangent.scale(-6.)).plus(Vec3::new(0., 3.6, -3.));
        let attention = (phase * PI).sin().powi(2) * 0.48;
        position = position.plus(follow.minus(base_position).scale(attention));
        target = target.plus(lookahead.minus(base_target).scale(attention * 1.3));
        fov_y -= attention * 0.06;
    }
    // Narrow terminals frame a single landmark with contextual depth rather than
    // squeezing the wide establishing shot into the available columns.
    if width < height.saturating_mul(3) {
        fov_y += 0.18;
    }
    Camera {
        position,
        target,
        fov_y,
        far: 180.,
        ..Camera::default()
    }
}

pub fn route(from: usize, to: usize) -> CubicPath3 {
    let a = BUILDINGS[from.min(3)];
    let b = BUILDINGS[to.min(3)];
    let start = a.position.plus(Vec3::new(0., a.height + 0.35, 0.));
    let end = b.position.plus(Vec3::new(0., b.height + 0.35, 0.));
    CubicPath3 {
        start,
        control1: start.plus(Vec3::new(0., 3., 1.)),
        control2: end.plus(Vec3::new(0., 3., -1.)),
        end,
    }
}

fn box_lines(
    renderer: &mut Rasterizer,
    camera: &Camera,
    center: Vec3,
    dimensions: Vec3,
    color: Rgb,
    floors: usize,
) {
    // The light rails stand proud of recessed dark cladding. Keeping these
    // distinct physical planes avoids coplanar line/triangle depth contention.
    let mesh = TriangleMesh::box_xyz(
        dimensions.x * 0.96,
        dimensions.y * 0.96,
        dimensions.z * 0.96,
    );
    renderer.draw_mesh(
        &mesh,
        Transform3 {
            offset: center,
            ..Transform3::default()
        },
        camera,
        Material {
            color: shade(color, 0.055),
            ambient: 0.8,
            diffuse: 0.25,
            emissive: 0.,
        },
    );
    let corners: Vec<_> = mesh
        .vertices
        .iter()
        .map(|v| v.scale(1. / 0.96).plus(center))
        .collect();
    for (a, b) in [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ] {
        renderer.line(corners[a], corners[b], camera, color);
    }
    for i in 1..floors {
        let y = center.y - dimensions.y * 0.5 + dimensions.y * i as f32 / floors as f32;
        let x = dimensions.x * 0.505;
        let z = dimensions.z * 0.505;
        let p = [
            Vec3::new(-x, y, -z),
            Vec3::new(x, y, -z),
            Vec3::new(x, y, z),
            Vec3::new(-x, y, z),
        ];
        for j in 0..4 {
            renderer.line(
                p[j].plus(Vec3::new(center.x, 0., center.z)),
                p[(j + 1) % 4].plus(Vec3::new(center.x, 0., center.z)),
                camera,
                shade(color, 0.32),
            );
        }
    }
}

/// A deliberately small bitmap alphabet for world-space receipts and the hero
/// wordmark. It is demo art, not a font API or a text-layout replacement.
fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 25, 21, 19, 19, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        'l' => [12, 4, 4, 4, 4, 4, 14],
        'i' => [4, 0, 12, 4, 4, 4, 14],
        'b' => [16, 16, 22, 25, 17, 17, 30],
        's' => [0, 0, 15, 16, 14, 1, 30],
        'o' => [0, 0, 14, 17, 17, 17, 14],
        'n' => [0, 0, 22, 25, 17, 17, 17],
        _ => [0; 7],
    }
}

fn face_text(
    renderer: &mut Rasterizer,
    camera: &Camera,
    text: &str,
    origin: Vec3,
    pixel: f32,
    color: Rgb,
) {
    for (letter, c) in text.chars().enumerate() {
        for (row, bits) in glyph(c).iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    let p = origin.plus(Vec3::new(
                        (letter * 6 + col) as f32 * pixel,
                        -(row as f32) * pixel,
                        0.,
                    ));
                    renderer.line(p, p.plus(Vec3::new(pixel * 0.76, 0., 0.)), camera, color);
                }
            }
        }
    }
}

/// Quiet architectural frames share the same facade dimensions, but their
/// silhouettes are intentional: ordered terraces, sensor mast, scaffold, vault.
fn hero(renderer: &mut Rasterizer, camera: &Camera, index: usize, gain: f32, seconds: f32) {
    let a = BUILDINGS[index];
    let color = shade(IDENTITIES[index].accent, gain);
    let center = a.position.plus(Vec3::new(0., a.height * 0.5, 0.));
    box_lines(
        renderer,
        camera,
        center,
        Vec3::new(4., a.height, 2.4),
        shade(color, 0.76),
        if index == 2 { 4 } else { 2 },
    );
    match index {
        0 => {
            for tier in 0..3 {
                box_lines(
                    renderer,
                    camera,
                    a.position
                        .plus(Vec3::new(0., a.height + 0.22 + tier as f32 * 0.48, 0.)),
                    Vec3::new(3.6 - tier as f32 * 0.85, 0.44, 2.25 - tier as f32 * 0.5),
                    shade(color, 1. - tier as f32 * 0.12),
                    1,
                );
            }
            for x in [-1.65, 1.65] {
                renderer.line(
                    a.position.plus(Vec3::new(x, 0.1, -1.25)),
                    a.position.plus(Vec3::new(x, a.height, -1.25)),
                    camera,
                    shade(color, 0.55),
                );
            }
        }
        1 => {
            box_lines(
                renderer,
                camera,
                a.position.plus(Vec3::new(0., a.height + 1.15, 0.)),
                Vec3::new(0.75, 2.3, 0.75),
                color,
                3,
            );
            renderer.line(
                a.position.plus(Vec3::new(0., a.height + 2.3, 0.)),
                a.position.plus(Vec3::new(0., a.height + 3.15, 0.)),
                camera,
                color,
            );
            for arm in 0..4 {
                let angle = arm as f32 * PI * 0.5;
                let end = Vec3::new(angle.cos() * 2.5, a.height + 1.8, angle.sin() * 2.5);
                renderer.line(
                    a.position.plus(Vec3::new(0., a.height + 1.8, 0.)),
                    a.position.plus(end),
                    camera,
                    shade(color, 0.68),
                );
                renderer.line(
                    a.position.plus(end),
                    a.position.plus(end.plus(Vec3::new(0., 0.8, 0.))),
                    camera,
                    color,
                );
            }
            ring(
                renderer,
                camera,
                a.position.plus(Vec3::new(0., a.height + 0.4, 0.)),
                1.5,
                shade(color, 0.42),
                seconds * 0.05,
            );
        }
        2 => {
            for tier in 0..4 {
                let y = tier as f32 * a.height / 4.;
                let offset = if tier % 2 == 0 { 0.24 } else { -0.24 };
                let base = a.position.plus(Vec3::new(offset, y + a.height / 8., 0.));
                box_lines(
                    renderer,
                    camera,
                    base,
                    Vec3::new(4.5, a.height / 4., 2.8),
                    shade(color, 0.7),
                    1,
                );
                // Lateral cross-bracing leaves the front information plane quiet.
                for side in [-1., 1.] {
                    let x = side * 2.28 + offset;
                    renderer.line(
                        a.position.plus(Vec3::new(x, y, -1.4)),
                        a.position.plus(Vec3::new(x, y + a.height / 4., 1.4)),
                        camera,
                        shade(color, 0.5),
                    );
                    renderer.line(
                        a.position.plus(Vec3::new(x, y, 1.4)),
                        a.position.plus(Vec3::new(x, y + a.height / 4., -1.4)),
                        camera,
                        shade(color, 0.5),
                    );
                }
            }
            box_lines(
                renderer,
                camera,
                a.position.plus(Vec3::new(0.9, a.height + 0.4, 0.)),
                Vec3::new(1.8, 0.8, 1.8),
                color,
                1,
            );
        }
        _ => {
            for ring_index in 0..3 {
                let r = 2.8 - ring_index as f32 * 0.25;
                let z = a.position.z + 0.5 + ring_index as f32 * 0.25;
                for step in 0..8 {
                    let t = step as f32 / 8. * TAU;
                    let u = (step + 1) as f32 / 8. * TAU;
                    renderer.line(
                        Vec3::new(a.position.x + r * t.cos(), a.height * 0.5 + r * t.sin(), z),
                        Vec3::new(a.position.x + r * u.cos(), a.height * 0.5 + r * u.sin(), z),
                        camera,
                        shade(color, 0.85 - ring_index as f32 * 0.15),
                    );
                }
            }
            for side in [-1., 1.] {
                box_lines(
                    renderer,
                    camera,
                    a.position.plus(Vec3::new(side * 2.3, a.height * 0.5, 0.2)),
                    Vec3::new(0.35, a.height + 0.8, 2.8),
                    shade(color, 0.8),
                    2,
                );
            }
        }
    }
}

fn ring(
    renderer: &mut Rasterizer,
    camera: &Camera,
    center: Vec3,
    radius: f32,
    color: Rgb,
    angle: f32,
) {
    for step in 0..36 {
        let t = step as f32 / 36. * TAU + angle;
        let u = (step + 1) as f32 / 36. * TAU + angle;
        renderer.line(
            center.plus(Vec3::new(t.cos() * radius, 0., t.sin() * radius)),
            center.plus(Vec3::new(u.cos() * radius, 0., u.sin() * radius)),
            camera,
            color,
        );
    }
}

/// A facade is a tiny plane, not a texture system: normalized diagram points
/// become ordinary depth-tested world lines. Four different receipts occupy it.
struct FacadeDisplay {
    origin: Vec3,
    width: f32,
    height: f32,
    color: Rgb,
}
impl FacadeDisplay {
    fn point(&self, x: f32, y: f32) -> Vec3 {
        self.origin
            .plus(Vec3::new(x * self.width, -y * self.height, 0.))
    }
    fn line(
        &self,
        renderer: &mut Rasterizer,
        camera: &Camera,
        a: (f32, f32),
        b: (f32, f32),
        gain: f32,
    ) {
        renderer.line(
            self.point(a.0, a.1),
            self.point(b.0, b.1),
            camera,
            shade(self.color, gain),
        );
    }
    fn node(&self, renderer: &mut Rasterizer, camera: &Camera, x: f32, y: f32, r: f32) {
        let corners = [(x - r, y), (x, y - r), (x + r, y), (x, y + r)];
        for i in 0..4 {
            self.line(renderer, camera, corners[i], corners[(i + 1) % 4], 1.);
        }
    }
    fn content(&self, renderer: &mut Rasterizer, camera: &Camera, index: usize) {
        match index {
            0 => {
                for (a, b) in [
                    ((0.12, 0.5), (0.43, 0.16)),
                    ((0.12, 0.5), (0.43, 0.83)),
                    ((0.43, 0.16), (0.83, 0.5)),
                    ((0.43, 0.83), (0.83, 0.5)),
                ] {
                    self.line(renderer, camera, a, b, 0.65);
                }
                for (x, y) in [(0.12, 0.5), (0.43, 0.16), (0.43, 0.83), (0.83, 0.5)] {
                    self.node(renderer, camera, x, y, 0.07);
                }
            }
            1 => {
                let nodes = [
                    (0.08, 0.6),
                    (0.3, 0.18),
                    (0.37, 0.75),
                    (0.57, 0.39),
                    (0.76, 0.12),
                    (0.93, 0.66),
                ];
                for (a, b) in [(0, 1), (0, 2), (1, 3), (2, 3), (3, 4), (3, 5), (4, 5)] {
                    self.line(renderer, camera, nodes[a], nodes[b], 0.48);
                }
                for (x, y) in nodes {
                    self.node(renderer, camera, x, y, 0.04);
                }
            }
            2 => {
                for lane in 0..3 {
                    let gain = if lane == 1 { 1.1 } else { 0.72 };
                    for step in 0..24 {
                        let t = step as f32 / 24.;
                        let next = (step + 1) as f32 / 24.;
                        let x = 0.09 + t * 0.82;
                        let nx = 0.09 + next * 0.82;
                        let y = 0.5 + (lane as f32 - 1.) * 0.43 * (t * PI).sin();
                        let ny = 0.5 + (lane as f32 - 1.) * 0.43 * (next * PI).sin();
                        self.line(renderer, camera, (x, y), (nx, ny), gain);
                    }
                }
                self.node(renderer, camera, 0.06, 0.5, 0.05);
                self.node(renderer, camera, 0.94, 0.5, 0.05);
                self.line(renderer, camera, (0.67, 0.40), (0.75, 0.50), 1.1);
                self.line(renderer, camera, (0.75, 0.50), (0.67, 0.60), 1.1);
            }
            _ => {
                for row in 0..4 {
                    for col in 0..6 {
                        let x = 0.04 + col as f32 * 0.16;
                        let y = 0.1 + row as f32 * 0.24;
                        self.line(renderer, camera, (x, y + 0.03), (x + 0.035, y + 0.09), 0.85);
                        self.line(
                            renderer,
                            camera,
                            (x + 0.035, y + 0.09),
                            (x + 0.10, y - 0.03),
                            0.85,
                        );
                    }
                }
            }
        }
    }
}

fn facade_graphics(
    renderer: &mut Rasterizer,
    camera: &Camera,
    index: usize,
    gain: f32,
    selected: bool,
) {
    let a = BUILDINGS[index];
    let color = shade(IDENTITIES[index].accent, gain);
    if !selected {
        face_text(
            renderer,
            camera,
            AGENTS[index].name,
            a.position.plus(Vec3::new(-1.65, a.height - 0.55, -1.27)),
            0.065,
            color,
        );
    }
    if index == 2 {
        // BUILDER's structural cross members pass behind its mounted route
        // display. A physical dark plate protects the selected curve's contour.
        renderer.draw_mesh(
            &TriangleMesh::box_xyz(3.45, 1.55, 0.025),
            Transform3 {
                offset: a.position.plus(Vec3::new(0., a.height - 2.66, -1.66)),
                ..Transform3::default()
            },
            camera,
            Material {
                color: (2, 4, 12),
                ambient: 1.,
                diffuse: 0.,
                emissive: 0.,
            },
        );
    }
    let plane = FacadeDisplay {
        origin: a.position.plus(Vec3::new(
            -1.55,
            a.height - if index == 2 { 2.03 } else { 2.55 },
            if index == 2 { -1.7 } else { -1.47 },
        )),
        width: 3.1,
        height: 1.28,
        color,
    };
    plane.content(renderer, camera, index);
    // A paired, identical witness is VERIFY's geometry-level replay receipt.
    if index == 3 {
        for step in 0..10 {
            let x = -1.5 + step as f32 * 0.31;
            let y = 0.38 + ((step * 7) % 5) as f32 * 0.055;
            for offset in [0., 0.2] {
                renderer.line(
                    a.position.plus(Vec3::new(x, y + offset, -1.47)),
                    a.position.plus(Vec3::new(x + 0.2, y + offset, -1.47)),
                    camera,
                    shade(color, 0.7),
                );
            }
        }
    }
}

fn capsule(
    renderer: &mut Rasterizer,
    camera: &Camera,
    point: Vec3,
    tangent: Vec3,
    color: Rgb,
    seconds: f32,
    sender: usize,
) {
    let axis = if tangent.length() > 0.001 {
        tangent
    } else {
        Vec3::new(0., 1., 0.)
    };
    let up = Vec3::new(-axis.z, 0., axis.x).normalize();
    let side = axis.cross(up).normalize();
    let mut rings = [[Vec3::default(); 4]; 2];
    for (end, vertices) in rings.iter_mut().enumerate() {
        for (v, point_out) in vertices.iter_mut().enumerate() {
            let theta = v as f32 * PI * 0.5 + seconds * 1.4 + sender as f32 * 0.3;
            *point_out = point
                .plus(axis.scale(if end == 0 { -0.31 } else { 0.31 }))
                .plus(up.scale(theta.cos() * 0.19))
                .plus(side.scale(theta.sin() * 0.19));
        }
    }
    for v in 0..4 {
        renderer.line(rings[0][v], rings[0][(v + 1) % 4], camera, color);
        renderer.line(rings[1][v], rings[1][(v + 1) % 4], camera, color);
        renderer.line(rings[0][v], rings[1][v], camera, shade(color, 0.8));
    }
    renderer.line(
        point.plus(axis.scale(-0.36)),
        point.plus(axis.scale(0.36)),
        camera,
        (221, 245, 255),
    );
    // Three payload slats trail the cage in its own local tangent frame.
    for strip in 0..3 {
        let p = point.plus(axis.scale(-0.5 - strip as f32 * 0.22));
        renderer.line(
            p.plus(up.scale(-0.12)),
            p.plus(up.scale(0.12)),
            camera,
            shade(color, 0.8 - strip as f32 * 0.18),
        );
    }
}

fn city(width: u16, height: u16, seconds: f32) -> RgbRaster {
    let mut renderer = Rasterizer::new(width, height.saturating_mul(2));
    let camera = camera(seconds, width, height);
    let focal = shots::facade(seconds);
    let focus = if focal.is_some() {
        let local = seconds - shots::at(seconds).start;
        smooth((local - 0.12) / 0.35) * (1. - smooth((local - 1.62) / 0.38))
    } else {
        0.
    };
    let context = 1. - focus * 0.52;
    renderer.clear((2, 4, 12));
    renderer.fog = Some(Fog {
        color: (2, 4, 12),
        start: 20. - focus * 8.,
        end: 92. - focus * 54.,
    });
    let curvature = smooth((seconds - 53.) / 3.) * 0.004;
    // As altitude rises, the same circuit plane bends toward the emerging globe.
    // Each grid line is segmented so its horizon curves continuously in geometry.
    for i in -13..=13 {
        let v = i as f32 * 2.;
        let segments = if curvature > 0. { 14 } else { 1 };
        for segment in 0..segments {
            let a = -24. + segment as f32 * 54. / segments as f32;
            let b = a + 54. / segments as f32;
            for (p, q) in [
                (
                    Vec3::new(v, -curvature * (v * v + a * a), a + 4.),
                    Vec3::new(v, -curvature * (v * v + b * b), b + 4.),
                ),
                (
                    Vec3::new(a, -curvature * (a * a + v * v), v + 4.),
                    Vec3::new(b, -curvature * (b * b + v * v), v + 4.),
                ),
            ] {
                renderer.line(p, q, &camera, shade((14, 48, 77), context));
            }
        }
    }
    // Four repeatable skyline families; open streets protect landmark silhouettes.
    let background_count = if width <= 112 { 22 } else { 36 };
    for i in 0..background_count {
        let side = if i % 2 == 0 { -1. } else { 1. };
        let x = side * (10.5 + (i % 5) as f32 * 3.5);
        let z = (i / 2) as f32 * 3.25 - 11.;
        let h = 1.2 + ((i * 17) % 11) as f32 * 0.42;
        let color = shade((25, 78, 121), context);
        let (w, d, hh) = match i % 4 {
            0 => (3.8, 2.8, h * 0.3),
            1 => (0.7, 0.9, h * 1.65),
            2 => (2.5, 1.3, h),
            _ => (1.25, 1.3, h),
        };
        box_lines(
            &mut renderer,
            &camera,
            Vec3::new(x, hh * 0.5, z),
            Vec3::new(w, hh, d),
            color,
            if i % 4 == 2 { 2 } else { 1 },
        );
        if i % 7 == 0 {
            renderer.line(
                Vec3::new(x, hh, z),
                Vec3::new(x, hh + 2., z),
                &camera,
                shade(color, 0.85),
            );
        }
        if i % 6 == 0 {
            for rail in [-0.35, 0.35] {
                renderer.line(
                    Vec3::new(x, 1.2, z + rail),
                    Vec3::new(x - side * 4.5, 1.2, z + rail + 3.),
                    &camera,
                    shade(color, 0.62),
                );
            }
        }
    }
    for (index, a) in BUILDINGS.iter().enumerate() {
        let arrival = MESSAGES
            .iter()
            .filter(|m| m.to == index)
            .map(|m| {
                let age = seconds - m.arrive;
                if (0.0..0.8).contains(&age) {
                    1. - age / 0.8
                } else {
                    0.
                }
            })
            .fold(0f32, f32::max);
        let gain = if focal.is_none_or(|f| f == index) {
            1.
        } else {
            1. - focus * 0.66
        };
        hero(
            &mut renderer,
            &camera,
            index,
            gain * (0.78 + arrival * 0.38),
            seconds,
        );
        facade_graphics(
            &mut renderer,
            &camera,
            index,
            gain * (0.75 + arrival * 0.35),
            focal == Some(index),
        );
        if arrival > 0. {
            let age = (seconds
                - MESSAGES
                    .iter()
                    .find(|m| m.to == index)
                    .map_or(seconds, |m| m.arrive))
            .max(0.);
            ring(
                &mut renderer,
                &camera,
                a.position.plus(Vec3::new(0., a.height + 0.45, 0.)),
                1.1 + age * 3.,
                shade(IDENTITIES[index].accent, arrival),
                0.,
            );
        }
    }
    for message in MESSAGES {
        let path = route(message.from, message.to);
        let color = IDENTITIES[message.from].accent;
        let phase = ((seconds - message.depart) / (message.arrive - message.depart)).clamp(0., 1.);
        let active = seconds >= message.depart && seconds < message.arrive;
        for step in 0..48 {
            let p = step as f32 / 48.;
            let gain = if active {
                (0.08 + (-(p - phase - 0.06).abs() * 17.).exp() * 0.72) * context
            } else {
                0.045 * context
            };
            renderer.line(
                path.sample(p),
                path.sample((step + 1) as f32 / 48.),
                &camera,
                shade(color, gain),
            );
        }
        if !active {
            continue;
        }
        for tail in (0..20).rev() {
            let t = phase - tail as f32 * 0.011;
            if t < 0. {
                continue;
            }
            renderer.line(
                path.sample(t),
                path.sample((t + 0.014).min(1.)),
                &camera,
                shade(color, 1. - tail as f32 / 22.),
            );
        }
        capsule(
            &mut renderer,
            &camera,
            path.sample(phase),
            path.tangent(phase),
            color,
            seconds,
            message.from,
        );
    }
    RasterFx::apply_chain(
        &mut renderer.raster,
        &[
            RasterFx::Glow {
                radius: 1,
                threshold: 108,
                strength: 0.55,
            },
            RasterFx::Vignette { strength: 0.24 },
        ],
    );
    renderer.raster
}

/// Geometry condenses into the same geographic beacon. There is no circular
/// compositing boundary: only luminous city marks travel, leaving the emerging
/// atmosphere visible in their negative space.
fn site_pullback(width: u16, height: u16, seconds: f32) -> (f32, f32, f32) {
    let t = smooth((seconds - 56.) / 4.);
    let zoom = (1. - t).powf(1.65).max(0.0001);
    let (site_x, site_y) = super::planet::site(width, height, seconds);
    let x = width as f32 * 0.5 * (1. - t) + site_x * t;
    let y = height as f32 * (1. - t) + site_y * t;
    (x, y, zoom)
}

/// Opaque RGB realization. Useful for small optional PPM development captures.
/// Both arguments describe terminal cells; RGB uses two samples per cell row.
pub fn raster(width: u16, height: u16, seconds: f32) -> RgbRaster {
    let seconds = safe_time(seconds);
    if seconds >= 60. {
        return super::planet::raster(width, height, seconds);
    }
    if seconds <= 56. {
        return city(width, height, seconds);
    }
    let site = city(width, height, 56.);
    let mut planet = super::planet::raster(width, height, seconds);
    let reveal = smooth((seconds - 56.) / 4.);
    for color in planet.pixels_mut() {
        *color = shade(*color, reveal);
    }
    let (cx, cy, zoom) = site_pullback(width, height, seconds);
    for y in 0..planet.height() {
        for x in 0..planet.width() {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            {
                let sx = (dx / zoom + width as f32 * 0.5).round() as i32;
                let sy = (dy / zoom + height as f32).round() as i32;
                if let Some(color) = site.get(sx, sy) {
                    if color.0.max(color.1).max(color.2) > 24 {
                        planet.set(x as i32, y as i32, color);
                    }
                }
            }
        }
    }
    planet
}

/// Wireframe edges use the terminal's 2x4 Braille frequency band. The city is
/// sampled at that resolution before selecting dots; this avoids turning every
/// neon edge into a staircase of full-width half blocks. Filled faces remain dark.
fn wire_surface(width: u16, height: u16, seconds: f32, capability: ColorDepth) -> Surface {
    let high = city(width.saturating_mul(2), height.saturating_mul(2), seconds);
    let mut surface = Surface::new(width, height);
    const DOTS: [[u8; 2]; 4] = [[1, 8], [2, 16], [4, 32], [64, 128]];
    for y in 0..height {
        for x in 0..width {
            let mut bits = 0;
            let mut brightest = (0, 0, 0);
            let mut maximum = 0u8;
            for (dy, row) in DOTS.iter().enumerate() {
                for (dx, &bit) in row.iter().enumerate() {
                    let color = high
                        .get(i32::from(x) * 2 + dx as i32, i32::from(y) * 4 + dy as i32)
                        .unwrap_or_default();
                    let light = color.0.max(color.1).max(color.2);
                    if light > 42 {
                        bits |= bit;
                    }
                    if light > maximum {
                        maximum = light;
                        brightest = color;
                    }
                }
            }
            let glyph = char::from_u32(0x2800 + bits as u32).unwrap_or(' ');
            let mut style = if capability == ColorDepth::Mono {
                Style::new()
            } else {
                let color = if capability == ColorDepth::Ansi16 && maximum > 0 {
                    // The small ANSI palette needs discrete lit edges; depth is
                    // carried by DIM below instead of quantizing dusk to black.
                    shade(brightest, 170. / maximum.max(100) as f32)
                } else {
                    brightest
                };
                Style::new()
                    .fg(Color::Rgb(color.0, color.1, color.2))
                    .bg(Color::Rgb(2, 4, 12))
            };
            if matches!(capability, ColorDepth::Mono | ColorDepth::Ansi16) {
                if maximum < 105 {
                    style = style.dim();
                } else if maximum > 175 {
                    style = style.bold();
                }
            }
            surface.set_cell(x, y, Cell::new(Glyph::new(&glyph.to_string()), style));
        }
    }
    surface
}

/// Terminal realization retains a crisp semantic accent over the graphical world.
/// RGB quantization remains the central ANSI compiler's responsibility.
pub fn render(width: u16, height: u16, seconds: f32, capability: ColorDepth) -> Surface {
    let seconds = safe_time(seconds);
    let mut surface = if seconds <= 56. {
        wire_surface(width, height, seconds, capability)
    } else if seconds < 60. {
        let site = wire_surface(width, height, 56., capability);
        let mut rgb = super::planet::raster(width, height, seconds);
        let reveal = smooth((seconds - 56.) / 4.);
        for color in rgb.pixels_mut() {
            *color = shade(*color, reveal);
        }
        let mut planet = if capability == ColorDepth::Mono {
            rgb.to_mono_surface()
        } else {
            rgb.to_surface()
        };
        let (cx, cy, zoom) = site_pullback(width, height, seconds);
        for y in 0..height {
            for x in 0..width {
                let dx = x as f32 - cx;
                let dy = y as f32 * 2. + 0.5 - cy;
                {
                    let sx = (dx / zoom + width as f32 * 0.5).round() as i32;
                    let sy = ((dy / zoom + height as f32) * 0.5).round() as i32;
                    let cell = if sx >= 0 && sy >= 0 {
                        site.get(sx as u16, sy as u16).cloned()
                    } else {
                        None
                    };
                    if let Some(mut cell) = cell {
                        if cell.glyph.grapheme != "⠀" {
                            if capability != ColorDepth::Mono {
                                let top =
                                    rgb.get(i32::from(x), i32::from(y) * 2).unwrap_or_default();
                                let bottom = rgb
                                    .get(i32::from(x), i32::from(y) * 2 + 1)
                                    .unwrap_or_default();
                                let average =
                                    |a: u8, b: u8| ((u16::from(a) + u16::from(b)) / 2) as u8;
                                let background = (
                                    average(top.0, bottom.0),
                                    average(top.1, bottom.1),
                                    average(top.2, bottom.2),
                                );
                                cell.style.bg =
                                    Some(Color::Rgb(background.0, background.1, background.2));
                                if let Some(Color::Rgb(r, g, b)) = cell.style.fg {
                                    cell.style.fg = Some(Color::Rgb(
                                        r.max(background.0.saturating_add(20)),
                                        g.max(background.1.saturating_add(20)),
                                        b.max(background.2.saturating_add(20)),
                                    ));
                                }
                            }
                            planet.set_cell(x, y, cell);
                        }
                    }
                }
            }
        }
        planet
    } else {
        let rgb = raster(width, height, seconds);
        if capability == ColorDepth::Mono {
            rgb.to_mono_surface()
        } else {
            rgb.to_surface()
        }
    };
    if let Some(index) = shots::facade(seconds) {
        // Native type is reserved for the near-frontal hold. Its origin is the
        // very same depth-tested facade plane as the graphic receipt below it.
        let local = seconds - shots::at(seconds).start;
        if (0.52..1.62).contains(&local) {
            let c = camera(seconds, width, height);
            let building = BUILDINGS[index];
            let anchor = building
                .position
                .plus(Vec3::new(-1.65, building.height - 0.6, -1.5));
            let right = building
                .position
                .plus(Vec3::new(1.8, building.height - 0.6, -1.5));
            if let (Some((x, y, _)), Some((rx, _, _))) = (
                c.project(anchor, width, height.saturating_mul(2)),
                c.project(right, width, height.saturating_mul(2)),
            ) {
                let x = x.max(0.) as u16;
                let y = (y.max(0.) / 2.) as u16;
                let available = (rx.max(0.) as u16)
                    .saturating_sub(x)
                    .min(width.saturating_sub(x));
                let heading = format!("{} {}", IDENTITIES[index].signature, AGENTS[index].name);
                let details = if available >= 24 {
                    [
                        ["ACCEPTANCE CONTRACT", "4 contracts / plan sealed"],
                        ["TOPOLOGY / 128 STOPS", "384 links / 6 invariants"],
                        ["CANDIDATE ROUTES", "3 candidates / seeded"],
                        ["REPLAY EQUIVALENCE", "24 fixtures / 24 pass"],
                    ]
                } else {
                    [
                        ["CONTRACT SEALED", "4 contracts / OK"],
                        ["ROUTE TOPOLOGY", "128 / 384 links"],
                        ["ROUTE CANDIDATES", "3 routes / seed"],
                        ["REPLAY WITNESS", "24/24 exact"],
                    ]
                };
                let color = IDENTITIES[index].accent;
                let heading_style = if capability == ColorDepth::Mono {
                    Style::new().bold()
                } else {
                    Style::new()
                        .fg(Color::Rgb(color.0, color.1, color.2))
                        .bg(Color::Rgb(2, 7, 14))
                        .bold()
                };
                let receipt_style = if capability == ColorDepth::Mono {
                    Style::new()
                } else {
                    Style::new()
                        .fg(Color::Rgb(183, 207, 221))
                        .bg(Color::Rgb(2, 7, 14))
                };
                surface.print_str(x, y, &heading, heading_style, Some(available));
                for (row, line) in details[index].iter().enumerate() {
                    surface.print_str(
                        x,
                        y.saturating_add(row as u16 + 1),
                        line,
                        receipt_style,
                        Some(available),
                    );
                }
            }
        }
    } else if (28.0..35.0).contains(&seconds) {
        let c = camera(seconds, width, height);
        let mut labels: Vec<(i32, i32, i32)> = Vec::with_capacity(4);
        for (index, building) in BUILDINGS.iter().enumerate() {
            if let Some((x, y, _)) = c.project(
                building
                    .position
                    .plus(Vec3::new(0., building.height + 1.7, 0.)),
                width,
                height.saturating_mul(2),
            ) {
                let label = if width < 74 {
                    format!(
                        "{} {}",
                        IDENTITIES[index].signature, IDENTITIES[index].short
                    )
                } else {
                    format!("{} {}", IDENTITIES[index].signature, AGENTS[index].name)
                };
                let len = label.chars().count() as i32;
                let x = (x as i32 - len / 2).clamp(0, (i32::from(width) - len).max(0));
                let anchor_y = (y / 2.) as i32;
                let mut row = anchor_y;
                for shift in [0, -2, -4, 2, 4] {
                    let candidate =
                        (anchor_y + shift).clamp(0, i32::from(height.saturating_sub(1)));
                    let overlaps = labels.iter().any(|&(lx, ly, ll)| {
                        (ly - candidate).abs() < 2 && x < lx + ll + 2 && lx < x + len + 2
                    });
                    if !overlaps {
                        row = candidate;
                        break;
                    }
                }
                labels.push((x, row, len));
                let style = Style::new()
                    .fg(IDENTITIES[index].color())
                    .bg(Color::Rgb(2, 4, 12));
                surface.print_str(x as u16, row as u16, &label, style, None);
                if row != anchor_y && width >= 74 {
                    surface.print_str(
                        (x + len / 2) as u16,
                        (row + (anchor_y - row).signum()).max(0) as u16,
                        "·",
                        style,
                        None,
                    );
                }
            }
        }
    }
    if (44.0..53.0).contains(&seconds) {
        for message in MESSAGES {
            if seconds >= message.depart && seconds < message.arrive {
                let progress = (seconds - message.depart) / (message.arrive - message.depart);
                let path = route(message.from, message.to);
                let point = path.sample(progress);
                let c = camera(seconds, width, height);
                if let Some((x, y, _)) = c.project(point, width, height.saturating_mul(2)) {
                    let heading = format!(
                        "{} {} → {}",
                        IDENTITIES[message.from].signature,
                        IDENTITIES[message.from].short,
                        IDENTITIES[message.to].short
                    );
                    let caption = message.payload;
                    let left =
                        (x.max(0.) as u16).min(width.saturating_sub(caption.len() as u16 + 1));
                    let row = ((y.max(0.) / 2.) as u16)
                        .saturating_add(2)
                        .min(height.saturating_sub(2));
                    let style = Style::new()
                        .fg(IDENTITIES[message.from].color())
                        .bg(Color::Rgb(2, 4, 12));
                    // The short tether marks the caption as a packet plate, not
                    // a screen-global log. The payload moves with its carrier.
                    surface.print_str(left, row.saturating_sub(1), "╎", style, None);
                    surface.print_str(left, row, &heading, style.bold(), None);
                    surface.print_str(
                        left,
                        row.saturating_add(1),
                        caption,
                        style,
                        Some(width.saturating_sub(left)),
                    );
                }
            }
        }
    }
    surface
}
