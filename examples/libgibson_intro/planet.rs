//! An illustrated network Earth and a deliberately excessive chrome end card.
//! Pure sampled time, no image assets. The city and globe share one geographic
//! beacon through `site`; its four coloured routes retain the agent identities.
use super::identity::IDENTITIES;
use gibson::geom::Vec3;
use gibson::raster::{Rgb, RgbRaster};
use gibson::raster3d::Camera;
use gibson::raster_fx::RasterFx;
use std::f32::consts::{PI, TAU};

#[path = "wordart.rs"]
mod wordart;

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}
fn shade(c: Rgb, gain: f32) -> Rgb {
    (
        (c.0 as f32 * gain).clamp(0., 255.) as u8,
        (c.1 as f32 * gain).clamp(0., 255.) as u8,
        (c.2 as f32 * gain).clamp(0., 255.) as u8,
    )
}
fn add(a: Rgb, b: Rgb) -> Rgb {
    (
        a.0.saturating_add(b.0),
        a.1.saturating_add(b.1),
        a.2.saturating_add(b.2),
    )
}
fn safe_time(t: f32) -> f32 {
    if t.is_finite() {
        t.clamp(0., 72.)
    } else {
        0.
    }
}

// City coordinates are a tangent frame at the geographic beacon. Earth is
// hundreds of city units across; zooming out changes only the perspective pose.
const RADIUS: f32 = 600.;
const CENTER: Vec3 = Vec3 {
    x: 0.,
    y: -RADIUS,
    z: 4.,
};

/// One perspective pullback, shared by the city, spherical ground, routes and
/// atmosphere. After orbital altitude is reached the globe turns under the
/// camera: transport the camera into the body's rotating coordinate frame.
pub fn camera(width: u16, height: u16, seconds: f32) -> Camera {
    let seconds = safe_time(seconds);
    let t = smooth((seconds - 56.) / 7.);
    let start = Vec3::new(6., 35., -34.);
    let initial = start.minus(CENTER);
    let altitude = (initial.length() - RADIUS) * (1450. / (initial.length() - RADIUS)).powf(t);
    let direction = initial
        .normalize()
        .scale(1. - t)
        .plus(Vec3::new(0., 0.68, -0.7332121).scale(t))
        .normalize();
    let position = CENTER.plus(direction.scale(RADIUS + altitude));
    let target = Vec3::new(0., 0., 4.).scale(1. - t).plus(
        CENTER
            .plus(Vec3::new(0., 0.7332121, 0.68).scale(RADIUS * 0.36))
            .scale(t),
    );
    let spin =
        ((seconds - 60.).max(0.).powi(2) / ((seconds - 60.).max(0.) + 2.)) * 8.0f32.to_radians();
    let axis = local(geo(90., 0.));
    // Rodrigues rotation about the Earth's axis. The same rigid transform is
    // used for camera position, look target and up, so projected geometry agrees.
    let rotate = |p: Vec3| {
        p.scale(spin.cos())
            .plus(axis.cross(p).scale(spin.sin()))
            .plus(axis.scale(axis.dot(p) * (1. - spin.cos())))
    };
    Camera {
        position: CENTER.plus(rotate(position.minus(CENTER))),
        target: CENTER.plus(rotate(target.minus(CENTER))),
        up: rotate(Vec3::new(0., 1., 0.)),
        fov_y: 0.91
            + if width < height.saturating_mul(3) {
                0.18
            } else {
                0.
            },
        far: 6000.,
        ..Camera::default()
    }
}

// Geographic unit vectors <-> the city's east / up / north tangent frame.
fn local(p: Vec3) -> Vec3 {
    let east = Vec3::new(8f32.to_radians().cos(), 0., -8f32.to_radians().sin());
    let north = geo(48., 8.).cross(east);
    Vec3::new(p.dot(east), p.dot(geo(48., 8.)), p.dot(north))
}
fn geographic(p: Vec3) -> Vec3 {
    let east = Vec3::new(8f32.to_radians().cos(), 0., -8f32.to_radians().sin());
    let up = geo(48., 8.);
    east.scale(p.x)
        .plus(up.scale(p.y))
        .plus(up.cross(east).scale(p.z))
}

#[derive(Clone, Copy)]
struct Globe {
    camera: Camera,
    width: u16,
    height: u16,
    forward: Vec3,
    right: Vec3,
    up: Vec3,
    focal: f32,
}
impl Globe {
    fn at(width: u16, height: u16, seconds: f32) -> Self {
        let camera = if seconds < 56. {
            super::world::camera(seconds, width, height)
        } else {
            camera(width, height, seconds)
        };
        let forward = camera.target.minus(camera.position).normalize();
        let right = camera.up.cross(forward).normalize();
        let up = forward.cross(right);
        Self {
            camera,
            width,
            height: height.saturating_mul(2),
            forward,
            right,
            up,
            focal: height as f32 / (camera.fov_y * 0.5).tan(),
        }
    }
    fn ray(self, x: f32, y: f32) -> Vec3 {
        self.forward
            .plus(self.right.scale((x - self.width as f32 * 0.5) / self.focal))
            .plus(self.up.scale((self.height as f32 * 0.5 - y) / self.focal))
            .normalize()
    }
    fn hit(self, ray: Vec3) -> Option<Vec3> {
        // f64 discriminant prevents cancellation close to the surface.
        let o = self.camera.position.minus(CENTER);
        let b = f64::from(o.x) * f64::from(ray.x)
            + f64::from(o.y) * f64::from(ray.y)
            + f64::from(o.z) * f64::from(ray.z);
        let c = f64::from(o.x).powi(2) + f64::from(o.y).powi(2) + f64::from(o.z).powi(2)
            - f64::from(RADIUS).powi(2);
        let discriminant = b * b - c;
        if discriminant < 0. {
            return None;
        }
        let distance = -b - discriminant.sqrt();
        (distance > 0.).then(|| o.plus(ray.scale(distance as f32)).normalize())
    }
    fn project(self, p: Vec3) -> (f32, f32, f32) {
        let world = CENTER.plus(local(p).scale(RADIUS));
        let offset = world.minus(self.camera.position);
        let depth = offset.dot(self.forward);
        // The rotating network can pass close to the camera during ascent.
        // Respect the near plane before division, just as the city rasterizer does.
        if depth < self.camera.near {
            return (0., 0., -1.);
        }
        let x = self.width as f32 * 0.5 + self.focal * offset.dot(self.right) / depth;
        let y = self.height as f32 * 0.5 - self.focal * offset.dot(self.up) / depth;
        let visible = self.hit(offset.normalize()).is_none_or(|normal| {
            CENTER
                .plus(normal.scale(RADIUS))
                .minus(self.camera.position)
                .length()
                + 0.5
                >= offset.length()
        });
        (x, y, if visible { 1. } else { -1. })
    }
}
fn geo(latitude: f32, longitude: f32) -> Vec3 {
    let (lat_s, lat_c) = latitude.to_radians().sin_cos();
    let (lon_s, lon_c) = longitude.to_radians().sin_cos();
    Vec3::new(lat_c * lon_s, lat_s, lat_c * lon_c)
}

/// RGB pixel coordinates of the same illustrated geographic site throughout
/// ascent and the globe hold. Arguments describe terminal cells, not pixels.
pub fn site(width: u16, height: u16, seconds: f32) -> (f32, f32) {
    let (x, y, _) =
        Globe::at(width.min(320), height.min(100), safe_time(seconds)).project(geo(48., 8.));
    (x, y)
}

/// Conservative visible footprint of the graphical wordmark and subtitle,
/// `(left, top, right_exclusive, bottom_exclusive)` in RGB pixel coordinates.
/// `None` before their entrance. Useful for bounds checks without recognising art.
pub fn title_bounds(width: u16, height: u16, seconds: f32) -> Option<(i32, i32, i32, i32)> {
    wordart::bounds(
        width.min(320),
        height.min(100).saturating_mul(2),
        safe_time(seconds),
    )
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

fn light(raster: &mut RgbRaster, x: i32, y: i32, color: Rgb) {
    if let Some(base) = raster.get(x, y) {
        raster.set(x, y, add(base, color));
    }
}
fn light_line(raster: &mut RgbRaster, a: (i32, i32), b: (i32, i32), color: Rgb) {
    if a == b {
        return;
    }
    let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs()).clamp(1, 640);
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        light(
            raster,
            (a.0 as f32 + (b.0 - a.0) as f32 * t).round() as i32,
            (a.1 as f32 + (b.1 - a.1) as f32 * t).round() as i32,
            color,
        );
    }
}

/// Four messages become four long-distance routes. A raised great-circle chord
/// is enough for the visual metaphor; these are fictional addresses, not traffic.
fn arc_point(index: usize, phase: f32) -> Vec3 {
    const DESTINATIONS: [(f32, f32); 4] = [(-17., -43.), (6., 72.), (-34., 23.), (37., -78.)];
    let a = geo(48., 8.);
    let b = geo(DESTINATIONS[index].0, DESTINATIONS[index].1);
    let p = a.scale(1. - phase).plus(b.scale(phase));
    let norm = (p.x * p.x + p.y * p.y + p.z * p.z).sqrt().max(0.01);
    let altitude = 1. + (phase * PI).sin() * (0.17 + index as f32 * 0.022);
    p.scale(altitude / norm)
}
fn mark(raster: &mut RgbRaster, x: f32, y: f32, index: usize, color: Rgb) {
    let (x, y) = (x.round() as i32, y.round() as i32);
    let offsets: &[(i32, i32)] = match index {
        0 => &[
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ],
        1 => &[
            (0, -2),
            (-1, -1),
            (0, -1),
            (1, -1),
            (-2, 0),
            (-1, 0),
            (0, 0),
            (1, 0),
            (2, 0),
        ],
        2 => &[(-1, -1), (0, 0), (1, 1), (-1, 1), (1, -1)],
        _ => &[(-1, 0), (0, 1), (1, 0), (2, -1)],
    };
    for &(dx, dy) in offsets {
        light(raster, x + dx, y + dy, color);
    }
}
fn network(raster: &mut RgbRaster, globe: Globe, seconds: f32) {
    let reveal = smooth((seconds - 58.5) / 4.5);
    if reveal <= 0. {
        return;
    }
    for (index, identity) in IDENTITIES.iter().enumerate() {
        let mut previous = None;
        let head = ((seconds - 59.) * 0.11 + index as f32 * 0.22).rem_euclid(1.);
        for step in 0..=80 {
            let phase = step as f32 / 80.;
            let (x, y, z) = globe.project(arc_point(index, phase));
            let visible = z > 0.;
            let pattern = match index {
                1 => step % 9 < 6,
                2 => step % 5 != 0,
                3 => step % 4 < 2,
                _ => true,
            };
            if visible && pattern {
                let distance = (phase - head).rem_euclid(1.);
                let wake = (1. - distance).powi(18);
                let color = shade(identity.accent, reveal * (0.15 + wake * 0.56));
                if let Some((px, py)) = previous {
                    light_line(
                        raster,
                        (px, py),
                        (x.round() as i32, y.round() as i32),
                        color,
                    );
                }
                previous = Some((x.round() as i32, y.round() as i32));
            } else {
                previous = None;
            }
        }
        let (x, y, z) = globe.project(arc_point(index, head));
        if z > 0. {
            light(
                raster,
                x.round() as i32,
                y.round() as i32,
                shade(identity.accent, reveal),
            );
        }
        let (x, y, z) = globe.project(arc_point(index, 1.));
        if z > 0. {
            mark(raster, x, y, index, shade(identity.accent, reveal * 0.65));
        }
    }
    let (x, y, visible) = globe.project(geo(48., 8.));
    if visible <= 0. {
        return;
    }
    let pulse = (seconds * 0.7).sin() * 0.5 + 0.5;
    let mut previous = None;
    for step in 0..36 {
        let angle = step as f32 / 36. * TAU;
        let radius = 1.7 + pulse * 1.1;
        let point = (
            (x + radius * angle.cos()).round() as i32,
            (y + radius * angle.sin()).round() as i32,
        );
        if previous != Some(point) {
            light(
                raster,
                point.0,
                point.1,
                shade((105, 209, 240), reveal * (0.36 - pulse * 0.16)),
            );
        }
        previous = Some(point);
    }
    light(
        raster,
        x.round() as i32,
        y.round() as i32,
        shade((227, 255, 246), reveal),
    );
}

/// Opaque software RGB. The intended presentation is ordinary half-block cells;
/// Mono uses the existing deterministic Braille-density realization upstream.
pub fn raster(width: u16, height: u16, seconds: f32) -> RgbRaster {
    let width = width.min(320);
    let height = height.min(100);
    let seconds = safe_time(seconds);
    let mut raster = RgbRaster::new(width, height.saturating_mul(2));
    if width == 0 || height == 0 {
        return raster;
    }
    let globe = Globe::at(width, height, seconds);
    let sunlight = globe
        .right
        .scale(-0.70)
        .plus(globe.up.scale(0.40))
        .plus(globe.forward.scale(-0.58))
        .normalize();
    for y in 0..raster.height() {
        for x in 0..raster.width() {
            let ray = globe.ray(x as f32 + 0.5, y as f32 + 0.5);
            let color = if let Some(normal) = globe.hit(ray) {
                let geographical = geographic(normal);
                let latitude = geographical.y.clamp(-1., 1.).asin().to_degrees();
                let longitude = geographical.x.atan2(geographical.z).to_degrees();
                let land = LAND
                    .iter()
                    .any(|polygon| inside(longitude, latitude, polygon));
                let light = normal.dot(sunlight).max(0.);
                let relief = 0.91 + 0.09 * (longitude * 0.23 + latitude * 0.41).sin();
                let base = if land { (24, 128, 120) } else { (9, 52, 115) };
                let mut c = shade(base, 0.12 + light * 1.13 * relief);
                // Shallow hand-drawn coast relief and sparse bands suggest a
                // physical globe without passing an illustration off as a map.
                let coast = land
                    && !LAND
                        .iter()
                        .any(|p| inside(longitude + 1.4, latitude + 1.1, p));
                if coast {
                    c = add(c, shade((30, 76, 58), light));
                }
                let clouds = ((longitude * 0.10 + latitude * 0.14).sin()
                    + (longitude * 0.034 - latitude * 0.19).cos())
                .max(1.57)
                    - 1.57;
                c = add(c, shade((80, 115, 133), clouds * light * 0.7));
                let lon_grid = longitude
                    .rem_euclid(20.)
                    .min(20. - longitude.rem_euclid(20.));
                let lat_grid = latitude.rem_euclid(15.).min(15. - latitude.rem_euclid(15.));
                let grid = (1. - lon_grid.min(lat_grid) / 0.55).clamp(0., 1.);
                c = add(c, shade((29, 62, 78), grid * (0.24 + light * 0.3)));
                let atmosphere = (1. - normal.dot(ray.scale(-1.)).max(0.)).powi(4);
                c = add(c, shade((46, 147, 207), atmosphere * (0.52 + light * 0.48)));
                // City lights are locked to longitude/latitude, never screen pixels.
                let gx = ((longitude + 180.) * 1.6).floor() as u32;
                let gy = ((latitude + 90.) * 1.6).floor() as u32;
                let hash = gx.wrapping_mul(1973).wrapping_add(gy.wrapping_mul(9277));
                if land && light < 0.42 && hash % 47 < 2 {
                    c = add(c, shade((183, 157, 89), 1. - light));
                }
                // Near the city the surface reads as a dark circuit substrate;
                // altitude reveals its geography without exchanging framebuffers.
                shade(c, 0.16 + 0.84 * smooth((seconds - 53.) / 6.))
            } else {
                let offset = globe.camera.position.minus(CENTER);
                let along = -offset.dot(ray);
                let closest = offset.plus(ray.scale(along.max(0.))).length();
                let distance = ((closest - RADIUS) * globe.focal / offset.length()).max(0.);
                let outer = (-distance * 0.60).exp();
                let inner = (-distance * 2.0).exp();
                let mut c = add((2, 4, 12), shade((15, 49, 98), outer));
                c = add(c, shade((53, 125, 185), inner));
                let seed = u32::from(x)
                    .wrapping_mul(1973)
                    .wrapping_add(u32::from(y).wrapping_mul(9277))
                    .wrapping_add(u32::from(x) * u32::from(y) * 17);
                if seed % 1301 < 4 {
                    let pulse = 0.82 + (seconds * 0.25 + seed as f32).sin() * 0.10;
                    c = add(c, shade((108, 143, 188), pulse));
                }
                c
            };
            raster.set(i32::from(x), i32::from(y), color);
        }
    }
    // The atmospheric horizon develops out of the existing dark circuit floor
    // at ascent entry. Every sample is already the same perspective sphere.
    let emergence = smooth((seconds - 53.) / 2.);
    if emergence < 1. {
        for pixel in raster.pixels_mut() {
            *pixel = add(
                (2, 4, 12),
                shade(
                    (
                        pixel.0.saturating_sub(2),
                        pixel.1.saturating_sub(4),
                        pixel.2.saturating_sub(12),
                    ),
                    emergence,
                ),
            );
        }
    }
    network(&mut raster, globe, seconds);
    // A spatial orbit passes behind the globe, with actual ray/sphere occlusion.
    let orbit_reveal = smooth((seconds - 60.) / 3.);
    let mut previous = None;
    for step in 0..=240 {
        let a = step as f32 / 240. * TAU;
        let p = Vec3::new(a.cos(), a.sin() * 0.25, a.sin() * 0.968246).scale(1.24);
        let (x, y, visible) = globe.project(p);
        let point = (x.round() as i32, y.round() as i32);
        if visible > 0. {
            if let Some(old) = previous {
                light_line(
                    &mut raster,
                    old,
                    point,
                    shade((75, 147, 191), orbit_reveal * 0.46),
                );
            }
            previous = Some(point);
        } else {
            previous = None;
        }
    }
    RasterFx::apply_chain(
        &mut raster,
        &[RasterFx::Glow {
            radius: 1,
            threshold: 176,
            strength: 0.45,
        }],
    );
    wordart::paint(&mut raster, seconds);
    raster
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn city_site_and_spherical_ground_share_perspective_rays() {
        for (w, h) in [(56, 24), (120, 32), (160, 40)] {
            for seconds in [53., 55.9, 56., 58., 60., 63.] {
                let globe = Globe::at(w, h, seconds);
                let (x, y, visible) = globe.project(geo(48., 8.));
                assert!(visible > 0.);
                let city = globe
                    .camera
                    .project(Vec3::new(0., 0., 4.), w, h * 2)
                    .unwrap();
                assert!((x - city.0).abs() < 0.001 && (y - city.1).abs() < 0.001);
                let normal = globe.hit(globe.ray(x, y)).unwrap();
                assert!(normal.minus(Vec3::new(0., 1., 0.)).length() < 0.0001);
            }
            // Genuine perspective scale: moving away reduces projected city
            // extent, rather than resampling a frame captured at an earlier time.
            let mut previous = f32::INFINITY;
            for seconds in [56., 57., 58., 59., 60., 61., 62., 63.] {
                let camera = camera(w, h, seconds);
                let left = camera.project(Vec3::new(-7., 0., 4.), w, h * 2).unwrap();
                let right = camera.project(Vec3::new(7., 0., 4.), w, h * 2).unwrap();
                let span = (left.0 - right.0).hypot(left.1 - right.1);
                assert!(span < previous, "city did not recede at {seconds}s");
                previous = span;
            }
        }
    }

    #[test]
    fn orbital_hold_rotates_geography_with_fixed_scale_and_correct_occlusion() {
        let a = Globe::at(120, 32, 63.);
        let b = Globe::at(120, 32, 70.);
        assert!(
            (a.camera.position.minus(CENTER).length() - b.camera.position.minus(CENTER).length())
                .abs()
                < 0.001
        );
        let landmark = geo(0., 0.);
        let pa = a.project(landmark);
        let pb = b.project(landmark);
        assert!(pa.2 > 0. && pb.2 > 0.);
        assert!((pa.0 - pb.0).hypot(pa.1 - pb.1) > 4.);
        for globe in [a, b] {
            let (x, y, _) = globe.project(landmark);
            let recovered = geographic(globe.hit(globe.ray(x, y)).unwrap());
            assert!(recovered.minus(landmark).length() < 0.0001);
            let backside = geographic(globe.camera.position.minus(CENTER).normalize().scale(-1.));
            assert!(
                globe.project(backside).2 < 0.,
                "hidden-side marker leaked through Earth"
            );
            let near = globe
                .camera
                .position
                .plus(globe.forward.scale(globe.camera.near * 0.5));
            assert!(
                globe
                    .project(geographic(near.minus(CENTER).scale(1. / RADIUS)))
                    .2
                    < 0.
            );
        }
        assert_eq!(raster(120, 32, 70.), raster(120, 32, 70.));
    }

    #[test]
    fn emerging_network_light_never_erases_the_underlying_globe() {
        for seconds in [58.5, 59.0, 59.8, 60.2, 68.0] {
            let mut raster = RgbRaster::new(120, 64);
            let base = (28, 112, 84);
            raster.clear(base);
            network(&mut raster, Globe::at(120, 32, seconds), seconds);
            assert!(
                raster
                    .pixels()
                    .iter()
                    .all(|&(r, g, b)| r >= base.0 && g >= base.1 && b >= base.2),
                "emissive route/mark/beacon darkened its substrate at {seconds}"
            );
        }
    }
}
