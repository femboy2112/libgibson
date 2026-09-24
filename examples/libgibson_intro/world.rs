//! The introduction's information city. Pure world-space rendering: buildings,
//! mounted receipts and couriers share the same coordinates and camera. No state
//! advances during paint; scrubbing to a time reconstructs the same frame.
use super::model::{AGENTS, MESSAGES};
use gibson::geom::{CubicPath3, Transform3, Vec3};
use gibson::raster::{Rgb, RgbRaster};
use gibson::raster3d::{Camera, Fog, Material, Rasterizer, TriangleMesh};
use gibson::raster_fx::RasterFx;
use gibson::{Cell, Color, ColorDepth, Glyph, Style, Surface};
use std::f32::consts::{PI, TAU};

const ICE: Rgb = (96, 224, 255);
const GOLD: Rgb = (255, 193, 87);
const VIOLET: Rgb = (153, 139, 255);

#[derive(Clone, Copy)]
struct Building {
    position: Vec3,
    height: f32,
    color: Rgb,
}
const BUILDINGS: [Building; 4] = [
    Building {
        position: Vec3 {
            x: -4.,
            y: 0.,
            z: 0.,
        },
        height: 5.2,
        color: ICE,
    },
    Building {
        position: Vec3 {
            x: 4.,
            y: 0.,
            z: 1.,
        },
        height: 4.1,
        color: GOLD,
    },
    Building {
        position: Vec3 {
            x: -3.,
            y: 0.,
            z: 7.,
        },
        height: 6.3,
        color: VIOLET,
    },
    Building {
        position: Vec3 {
            x: 4.,
            y: 0.,
            z: 8.,
        },
        height: 4.8,
        color: ICE,
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

/// Camera keyframes describe shots, not simulation changes. The close framing
/// holds long enough to read the actual building face before following its mail.
fn camera(seconds: f32, width: u16, height: u16) -> Camera {
    let keys = [
        (28., Vec3::new(0., 5., -22.), Vec3::new(0., 2., 4.)),
        (34., Vec3::new(10., 9., -17.), Vec3::new(0., 2., 4.)),
        (37., Vec3::new(-4., 3.8, -9.), Vec3::new(-4., 3.2, 0.)),
        (41., Vec3::new(-3.8, 3.8, -9.), Vec3::new(-4., 3.2, 0.)),
        (44., Vec3::new(-11., 8., -8.), Vec3::new(0., 3., 3.)),
        (49., Vec3::new(11., 9., -4.), Vec3::new(0., 3., 5.)),
        (53., Vec3::new(14., 11., -15.), Vec3::new(0., 2., 4.)),
        (60., Vec3::new(5., 50., -57.), Vec3::new(0., 0., 4.)),
    ];
    let mut position = keys[0].1;
    let mut target = keys[0].2;
    for pair in keys.windows(2) {
        if seconds >= pair[0].0 {
            let t = smooth((seconds - pair[0].0) / (pair[1].0 - pair[0].0));
            position = mix(pair[0].1, pair[1].1, t);
            target = mix(pair[0].2, pair[1].2, t);
        }
    }
    if let Some(message) = MESSAGES
        .iter()
        .find(|m| seconds >= m.depart && seconds < m.arrive)
    {
        let phase = (seconds - message.depart) / (message.arrive - message.depart);
        let courier = route(message.from, message.to).sample(phase);
        let weight = (phase * PI).sin().powi(2) * 0.32;
        let shift = courier.plus(target.scale(-1.));
        target = mix(target, courier, weight);
        position = position.plus(shift.scale(weight * 0.18));
    }
    Camera {
        position,
        target,
        fov_y: if width < height.saturating_mul(3) {
            1.05
        } else {
            0.88
        },
        far: 160.,
        ..Camera::default()
    }
}

fn route(from: usize, to: usize) -> CubicPath3 {
    let a = BUILDINGS[from];
    let b = BUILDINGS[to];
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
    let mesh = TriangleMesh::box_xyz(dimensions.x, dimensions.y, dimensions.z);
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
    let corners: Vec<_> = mesh.vertices.iter().map(|v| v.plus(center)).collect();
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

fn city(width: u16, height: u16, seconds: f32) -> RgbRaster {
    let mut renderer = Rasterizer::new(width, height.saturating_mul(2));
    let camera = camera(seconds, width, height);
    renderer.clear((2, 4, 12));
    renderer.fog = Some(Fog {
        color: (2, 4, 12),
        start: 18.,
        end: 80.,
    });
    // Sparse skyline and a circuit plane establish scale without becoming a wall
    // of labels. Streets remain the same world coordinates in every camera shot.
    for i in -12..=12 {
        let v = i as f32 * 2.;
        renderer.line(
            Vec3::new(v, 0., -18.),
            Vec3::new(v, 0., 34.),
            &camera,
            (12, 36, 62),
        );
        renderer.line(
            Vec3::new(-24., 0., v + 8.),
            Vec3::new(24., 0., v + 8.),
            &camera,
            (12, 36, 62),
        );
    }
    for i in 0..28 {
        let side = if i % 2 == 0 { -1. } else { 1. };
        let x = side * (9. + (i % 5) as f32 * 2.5);
        let z = (i / 2) as f32 * 3.1 - 10.;
        let h = 1.2 + ((i * 17) % 11) as f32 * 0.42;
        box_lines(
            &mut renderer,
            &camera,
            Vec3::new(x, h * 0.5, z),
            Vec3::new(1.4, h, 1.5),
            (27, 74, 112),
            4,
        );
    }
    for (index, a) in BUILDINGS.iter().enumerate() {
        let center = a.position.plus(Vec3::new(0., a.height * 0.5, 0.));
        box_lines(
            &mut renderer,
            &camera,
            center,
            Vec3::new(4., a.height, 2.4),
            shade(a.color, 0.8),
            6,
        );
        // Small roof terraces and antennas distinguish the computational actors.
        box_lines(
            &mut renderer,
            &camera,
            a.position.plus(Vec3::new(0., a.height + 0.25, 0.)),
            Vec3::new(2.8, 0.5, 1.6),
            a.color,
            1,
        );
        renderer.line(
            a.position.plus(Vec3::new(0., a.height + 0.5, 0.)),
            a.position.plus(Vec3::new(0., a.height + 1.3, 0.)),
            &camera,
            a.color,
        );
        if index != 0 || !(36.7..41.7).contains(&seconds) {
            face_text(
                &mut renderer,
                &camera,
                AGENTS[index].name,
                a.position.plus(Vec3::new(-1.7, a.height - 0.6, -1.23)),
                0.061,
                a.color,
            );
            for (row, fragment) in AGENTS[index].result.split('/').take(3).enumerate() {
                let text = fragment.trim().to_ascii_uppercase();
                let pixel = (3.4 / (text.len().max(1) as f32 * 6.)).min(0.058);
                face_text(
                    &mut renderer,
                    &camera,
                    &text,
                    a.position
                        .plus(Vec3::new(-1.7, a.height - 1.45 - row as f32 * 0.55, -1.24)),
                    pixel,
                    (175, 214, 227),
                );
            }
        }
        // Mounted tool-result rows: the bars and glyphs live on the facade, get
        // clipped and depth-tested exactly like its architecture.
        for row in 0..3 {
            let y = a.height - 3.3 - row as f32 * 0.25;
            let count = 3 + ((seconds as usize / 2 + row + index) % 5);
            for column in 0..count {
                let x = -1.65 + column as f32 * 0.39;
                renderer.line(
                    a.position.plus(Vec3::new(x, y, -1.25)),
                    a.position.plus(Vec3::new(x + 0.24, y, -1.25)),
                    &camera,
                    shade(a.color, 0.5),
                );
            }
        }
    }
    // Four repeatable transactions: plan, evidence, implementation, verification.
    // Every packet has a sender, receiver, and path; trails sample its own past.
    for message in MESSAGES {
        let from = message.from;
        let path = route(from, message.to);
        let color = BUILDINGS[from].color;
        for step in 0..36 {
            renderer.line(
                path.sample(step as f32 / 36.),
                path.sample((step + 1) as f32 / 36.),
                &camera,
                shade(color, 0.18),
            );
        }
        if seconds < message.depart || seconds > message.arrive + 0.4 {
            continue;
        }
        let phase = ((seconds - message.depart) / (message.arrive - message.depart)).clamp(0., 1.);
        for tail in (0..16).rev() {
            let t = phase - tail as f32 * 0.008;
            if t < 0. {
                continue;
            }
            renderer.line(
                path.sample(t),
                path.sample((t + 0.015).min(1.)),
                &camera,
                shade(color, 1. - tail as f32 / 18.),
            );
        }
        let point = path.sample(phase);
        renderer.draw_mesh(
            &TriangleMesh::octahedron(0.14),
            Transform3 {
                offset: point,
                ry: seconds,
                ..Transform3::default()
            },
            &camera,
            Material {
                color,
                ambient: 1.,
                diffuse: 0.,
                emissive: 0.3,
            },
        );
    }
    RasterFx::apply_chain(
        &mut renderer.raster,
        &[
            RasterFx::Glow {
                radius: 1,
                threshold: 105,
                strength: 0.75,
            },
            RasterFx::Vignette { strength: 0.3 },
        ],
    );
    renderer.raster
}

// Coarse hand-authored geographic silhouettes, explicitly illustration rather
// than cartographic data. Longitude/latitude polygons rotate with the globe.
const LAND: &[&[(f32, f32)]] = &[
    &[
        (-168., 70.),
        (-130., 72.),
        (-108., 57.),
        (-60., 52.),
        (-81., 25.),
        (-98., 15.),
        (-112., 29.),
        (-128., 50.),
        (-160., 57.),
    ],
    &[
        (-80., 12.),
        (-50., 5.),
        (-35., -7.),
        (-45., -24.),
        (-68., -55.),
        (-78., -18.),
    ],
    &[
        (-17., 36.),
        (12., 37.),
        (35., 28.),
        (50., 10.),
        (35., -30.),
        (18., -35.),
        (5., -8.),
        (-15., 8.),
    ],
    &[
        (-10., 36.),
        (-11., 59.),
        (30., 72.),
        (60., 68.),
        (90., 76.),
        (160., 62.),
        (174., 48.),
        (142., 36.),
        (121., 20.),
        (110., 0.),
        (78., 8.),
        (58., 28.),
        (36., 35.),
    ],
    &[
        (113., -12.),
        (137., -10.),
        (154., -24.),
        (148., -39.),
        (116., -34.),
    ],
    &[(-52., 60.), (-22., 69.), (-40., 83.), (-61., 76.)],
];
fn inside(x: f32, y: f32, polygon: &[(f32, f32)]) -> bool {
    let mut hit = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let (ax, ay) = polygon[i];
        let (bx, by) = polygon[j];
        if (ay > y) != (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax {
            hit = !hit;
        }
        j = i;
    }
    hit
}
fn earth(width: u16, height: u16, seconds: f32) -> RgbRaster {
    let mut raster = RgbRaster::new(width, height.saturating_mul(2));
    let w = raster.width() as f32;
    let h = raster.height() as f32;
    let arrive = smooth((seconds - 56.) / 7.);
    let radius = h * (0.83 - 0.45 * arrive);
    let cx = w * 0.5;
    let cy = h * 0.46;
    for y in 0..raster.height() {
        for x in 0..raster.width() {
            let px = (x as f32 - cx) / radius;
            let py = (cy - y as f32) / radius;
            let d = px * px + py * py;
            let mut color;
            if d < 1. {
                let z = (1. - d).sqrt();
                let lat = py.asin() * 180. / PI;
                let lon = (px.atan2(z) * 180. / PI - 30. + (seconds - 60.) * 1.8 + 180.)
                    .rem_euclid(360.)
                    - 180.;
                let land = LAND.iter().any(|polygon| inside(lon, lat, polygon));
                let light = (px * (-0.45) + py * 0.45 + z * 0.77).max(0.);
                let grid = (lon.rem_euclid(15.).min(15. - lon.rem_euclid(15.)) < 0.55
                    || lat.rem_euclid(15.).min(15. - lat.rem_euclid(15.)) < 0.55)
                    as u8;
                let base = if land { (49, 154, 153) } else { (13, 62, 126) };
                color = shade(base, 0.2 + light * 0.95);
                let haze = (1. - z).powi(3) * 0.8;
                color = (
                    color.0.saturating_add((haze * 55.) as u8),
                    color.1.saturating_add((haze * 120.) as u8 + grid * 16),
                    color.2.saturating_add((haze * 150.) as u8 + grid * 23),
                );
                // Sparse cities glow on the darker limb, never random per frame.
                if land && light < 0.45 && (u32::from(x) * 97 + u32::from(y) * 31) % 37 == 0 {
                    color = (153, 158, 104);
                }
            } else {
                let halo = (-(d.sqrt() - 1.) * 29.).exp();
                color = (2, (5. + halo * 69.) as u8, (13. + halo * 139.) as u8);
                let star =
                    (u32::from(x) * 1973 + u32::from(y) * 9277 + u32::from(x) * u32::from(y) * 17)
                        % 1301;
                if star < 5 {
                    color = shade((170, 197, 236), 0.4 + star as f32 * 0.1);
                }
            }
            raster.set(x as i32, y as i32, color);
        }
    }
    // The former city keeps one geographic address through the scale change.
    // A small beacon, not a second city simulation, remains at the landing site.
    let latitude = 48f32.to_radians();
    let longitude = (38. - (seconds - 60.) * 1.8).to_radians();
    let site_x = cx + radius * latitude.cos() * longitude.sin();
    let site_y = cy - radius * latitude.sin();
    if seconds >= 59. {
        raster.disc(site_x, site_y, 1.1, (217, 251, 255));
        let ring_radius = 2.3 + ((seconds - 59.) * 1.3).sin() * 0.4;
        for step in 0..32 {
            let angle = step as f32 / 32. * TAU;
            raster.set(
                (site_x + ring_radius * angle.cos()) as i32,
                (site_y + ring_radius * angle.sin()) as i32,
                (69, 176, 222),
            );
        }
    }
    // Orbiting information: the city becomes one point in a connected planet.
    for step in 0..200 {
        let a = step as f32 / 200. * TAU;
        let x = cx + radius * 1.24 * a.cos();
        let y = cy + radius * 0.24 * a.sin() + radius * 0.22 * a.cos();
        let back = a.sin() < 0.;
        if !back || ((x - cx).powi(2) + (y - cy).powi(2)) > radius * radius {
            raster.set(
                x as i32,
                y as i32,
                shade(ICE, if back { 0.18 } else { 0.6 }),
            );
        }
    }
    if seconds >= 63. {
        wordmark(&mut raster, smooth((seconds - 63.) / 2.));
    }
    RasterFx::apply_chain(
        &mut raster,
        &[RasterFx::Glow {
            radius: 1,
            threshold: 155,
            strength: 0.6,
        }],
    );
    raster
}
fn wordmark(raster: &mut RgbRaster, reveal: f32) {
    let text = "libGibson";
    let scale = (raster.width() as f32 / 56.).floor().clamp(1., 3.) as i32;
    let width = 53 * scale;
    let x0 = (raster.width() as i32 - width) / 2;
    let y0 = (raster.height() as f32 * 0.50) as i32;
    let visible = (text.len() as f32 * reveal).ceil() as usize;
    // Extruded italic chrome: a small graphic wordmark, not terminal text alpha.
    for depth in (0..=3).rev() {
        for (letter, c) in text.chars().take(visible).enumerate() {
            for (row, bits) in glyph(c).iter().enumerate() {
                for col in 0..5 {
                    if bits & (1 << (4 - col)) == 0 {
                        continue;
                    }
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let x = x0
                                + (letter as i32 * 6 + col) * scale
                                + sx
                                + (6 - row as i32) * scale / 4
                                + depth;
                            let y = y0 + row as i32 * scale + sy + depth;
                            let color = if depth > 0 {
                                (37, 40, 108)
                            } else if row < 3 {
                                (210, 244, 255)
                            } else if row == 3 {
                                (255, 236, 152)
                            } else {
                                (72, 175, 244)
                            };
                            raster.set(x, y, color);
                        }
                    }
                }
            }
        }
    }
}

/// The city pulls back into its geographic address. Source coordinates remain
/// centered on the same four-agent site; the enclosing radial boundary shrinks
/// continuously. Empty space reveals the planet behind the contracting geometry.
fn site_pullback(width: u16, height: u16, seconds: f32) -> (f32, f32, f32, f32) {
    let t = smooth((seconds - 56.) / 4.);
    let zoom = (1. - t).powi(2).max(0.0001);
    let radius = height as f32 * 2. * (0.83 - 0.45 * smooth((seconds - 56.) / 7.));
    let latitude = 48f32.to_radians();
    let longitude = (38. - (seconds - 60.) * 1.8).to_radians();
    let site_x = width as f32 * 0.5 + radius * latitude.cos() * longitude.sin();
    let site_y = height as f32 * 0.92 - radius * latitude.sin();
    let x = width as f32 * 0.5 * (1. - t) + site_x * t;
    let y = height as f32 * (1. - t) + site_y * t;
    let boundary = ((width as f32 * 0.5).powi(2) + (height as f32).powi(2)).sqrt() * zoom;
    (x, y, zoom, boundary)
}

/// Opaque RGB realization. Useful for small optional PPM development captures.
/// Both arguments describe terminal cells; RGB uses two samples per cell row.
pub fn raster(width: u16, height: u16, seconds: f32) -> RgbRaster {
    let seconds = safe_time(seconds);
    if seconds >= 60. {
        return earth(width, height, seconds);
    }
    if seconds <= 56. {
        return city(width, height, seconds);
    }
    let site = city(width, height, 56.);
    let mut planet = earth(width, height, seconds);
    let reveal = smooth((seconds - 56.) / 4.);
    for color in planet.pixels_mut() {
        *color = shade(*color, reveal);
    }
    let (cx, cy, zoom, boundary) = site_pullback(width, height, seconds);
    for y in 0..planet.height() {
        for x in 0..planet.width() {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if dx * dx + dy * dy < boundary * boundary {
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
            let style = if capability == ColorDepth::Mono {
                Style::new()
            } else {
                Style::new()
                    .fg(Color::Rgb(brightest.0, brightest.1, brightest.2))
                    .bg(Color::Rgb(2, 4, 12))
            };
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
        let mut rgb = earth(width, height, seconds);
        let reveal = smooth((seconds - 56.) / 4.);
        for color in rgb.pixels_mut() {
            *color = shade(*color, reveal);
        }
        let mut planet = if capability == ColorDepth::Mono {
            rgb.to_mono_surface()
        } else {
            rgb.to_surface()
        };
        let (cx, cy, zoom, boundary) = site_pullback(width, height, seconds);
        for y in 0..height {
            for x in 0..width {
                let dx = x as f32 - cx;
                let dy = y as f32 * 2. + 0.5 - cy;
                if dx * dx + dy * dy < boundary * boundary {
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
    let style = Style::new().fg(Color::Rgb(190, 231, 247));
    if (36.5..41.8).contains(&seconds) {
        // The rasterized receipt stays ON the building. This small readable
        // annotation is tethered to its roof, and disappears with this shot.
        let c = camera(seconds, width, height);
        if let Some((x, y, _)) = c.project(
            BUILDINGS[0].position.plus(Vec3::new(-2., 6.1, 0.)),
            width,
            height.saturating_mul(2),
        ) {
            surface.print_str(
                x.max(0.) as u16,
                (y.max(0.) / 2.) as u16,
                "01 / ARCHITECT",
                style,
                None,
            );
        }
    }
    // During a framed facade shot the UI projection supplies native terminal
    // typography at the SAME plane anchors. This is intentionally restricted to
    // the nearly frontal readable shot; oblique views use the depth-tested glyphs.
    if (36.7..41.7).contains(&seconds) {
        let c = camera(seconds, width, height);
        let building = BUILDINGS[0];
        let anchor = building
            .position
            .plus(Vec3::new(-1.65, building.height - 0.55, -1.27));
        if let Some((x, y, _)) = c.project(anchor, width, height.saturating_mul(2)) {
            let right = c.project(
                building
                    .position
                    .plus(Vec3::new(1.85, building.height - 0.55, -1.27)),
                width,
                height.saturating_mul(2),
            );
            if let Some((rx, _, _)) = right {
                let available = (rx - x).max(0.) as u16;
                let style = Style::new()
                    .fg(Color::Rgb(184, 239, 255))
                    .bg(Color::Rgb(3, 12, 18));
                let lines = [
                    AGENTS[0].name,
                    "ACCEPTANCE PLAN",
                    "4 contracts sealed",
                    "offline transit solver",
                ];
                for (row, line) in lines.iter().enumerate() {
                    surface.print_str(
                        x as u16,
                        (y as u16 / 2).saturating_add(row as u16),
                        line,
                        style,
                        Some(available),
                    );
                }
            }
        }
    }
    if (44.0..53.0).contains(&seconds) {
        for message in MESSAGES {
            if seconds >= message.depart && seconds < message.arrive {
                let progress = (seconds - message.depart) / (message.arrive - message.depart);
                let point = route(message.from, message.to).sample(progress);
                let c = camera(seconds, width, height);
                if let Some((x, y, _)) = c.project(point, width, height.saturating_mul(2)) {
                    let x = (x as u16).min(width.saturating_sub(message.payload.len() as u16));
                    surface.print_str(
                        x,
                        (y as u16 / 2).saturating_add(1),
                        message.payload,
                        style,
                        None,
                    );
                }
            }
        }
    }
    if seconds >= 65. {
        let text = "Hack the planet!";
        let y = (height as f32 * 0.87) as u16;
        surface.print_str(
            width.saturating_sub(text.len() as u16) / 2,
            y,
            text,
            Style::new().fg(Color::Rgb(255, 222, 141)),
            None,
        );
    }
    surface
}
