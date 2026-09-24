//! Experimental filled software triangles on [`RgbRaster`]. Camera-space frustum
//! clipping bounds all pixel work; barycentric reciprocal depth supplies actual
//! perspective occlusion. Output remains ordinary half-block Unicode cells.
use crate::geom::{Transform3, Vec3};
use crate::raster::{Rgb, RgbRaster};

type Point = [f64; 3];
fn p(v: Vec3) -> Point {
    [v.x as f64, v.y as f64, v.z as f64]
}
fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: Point) -> Option<Point> {
    let length = dot(a, a).sqrt();
    (length.is_finite() && length > 1e-20).then(|| [a[0] / length, a[1] / length, a[2] / length])
}
fn lerp(a: Point, b: Point, t: f64) -> Point {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Right-handed look-at camera with positive forward depth; vertical FOV is radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::default(),
            target: Vec3::new(0., 0., 1.),
            up: Vec3::new(0., 1., 0.),
            fov_y: std::f32::consts::FRAC_PI_3,
            near: 0.1,
            far: 100.,
        }
    }
}
struct View {
    position: Point,
    right: Point,
    up: Point,
    forward: Point,
    tan_x: f64,
    tan_y: f64,
    near: f64,
    far: f64,
    width: u16,
    height: u16,
}
impl Camera {
    fn view(&self, width: u16, height: u16) -> Option<View> {
        if width == 0
            || height == 0
            || !self.position.is_finite()
            || !self.target.is_finite()
            || !self.up.is_finite()
            || !self.fov_y.is_finite()
            || self.fov_y <= 0.
            || self.fov_y >= std::f32::consts::PI
            || !self.near.is_finite()
            || !self.far.is_finite()
            || self.near <= 0.
            || self.far <= self.near
        {
            return None;
        }
        let forward = unit(sub(p(self.target), p(self.position)))?;
        let right = unit(cross(p(self.up), forward))?;
        let up = cross(forward, right);
        let tan_y = (self.fov_y as f64 * 0.5).tan();
        Some(View {
            position: p(self.position),
            right,
            up,
            forward,
            tan_x: tan_y * width as f64 / height as f64,
            tan_y,
            near: self.near as f64,
            far: self.far as f64,
            width,
            height,
        })
    }
    /// Project a world point inside the frustum to continuous pixel-space x/y
    /// and camera depth. Coordinates use framebuffer edges: exact right/bottom
    /// frustum boundaries may map to `width`/`height`, not valid pixel indices.
    /// Rasterization/clipping owns conversion to discrete raster indices.
    pub fn project(&self, point: Vec3, width: u16, height: u16) -> Option<(f32, f32, f32)> {
        if !point.is_finite() {
            return None;
        }
        let view = self.view(width, height)?;
        let v = view.camera(p(point));
        if (0..6).any(|plane| view.distance(v, plane) < 0.) {
            return None;
        }
        let q = view.project(v);
        Some((q[0] as f32, q[1] as f32, v[2] as f32))
    }
}
impl View {
    fn camera(&self, v: Point) -> Point {
        let v = sub(v, self.position);
        [dot(v, self.right), dot(v, self.up), dot(v, self.forward)]
    }
    fn distance(&self, v: Point, plane: usize) -> f64 {
        match plane {
            0 => v[2] - self.near,
            1 => self.far - v[2],
            2 => v[0] + v[2] * self.tan_x,
            3 => v[2] * self.tan_x - v[0],
            4 => v[1] + v[2] * self.tan_y,
            _ => v[2] * self.tan_y - v[1],
        }
    }
    fn project(&self, v: Point) -> Point {
        [
            (v[0] / (v[2] * self.tan_x) + 1.) * self.width as f64 * 0.5,
            (1. - v[1] / (v[2] * self.tan_y)) * self.height as f64 * 0.5,
            1. / v[2],
        ]
    }
    fn clip(&self, triangle: [Point; 3]) -> Vec<Point> {
        let mut polygon = triangle.to_vec();
        for plane in 0..6 {
            if polygon.is_empty() {
                break;
            }
            let mut output = Vec::with_capacity(10);
            let mut a = *polygon.last().unwrap();
            let mut da = self.distance(a, plane);
            for &b in &polygon {
                let db = self.distance(b, plane);
                if (da >= 0.) != (db >= 0.) {
                    output.push(lerp(a, b, da / (da - db)));
                }
                if db >= 0. {
                    output.push(b);
                }
                a = b;
                da = db;
            }
            polygon = output;
        }
        polygon
    }
}
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TriangleMesh {
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<[usize; 3]>,
}
impl TriangleMesh {
    pub fn cube(size: f32) -> Self {
        Self::box_xyz(size, size, size)
    }
    /// Full dimensions, centered at the origin. Outward-facing triangle winding.
    pub fn box_xyz(x: f32, y: f32, z: f32) -> Self {
        let (x, y, z) = (x * 0.5, y * 0.5, z * 0.5);
        Self {
            vertices: vec![
                Vec3::new(-x, -y, -z),
                Vec3::new(x, -y, -z),
                Vec3::new(x, y, -z),
                Vec3::new(-x, y, -z),
                Vec3::new(-x, -y, z),
                Vec3::new(x, -y, z),
                Vec3::new(x, y, z),
                Vec3::new(-x, y, z),
            ],
            triangles: vec![
                [0, 2, 1],
                [0, 3, 2],
                [4, 5, 6],
                [4, 6, 7],
                [0, 1, 5],
                [0, 5, 4],
                [3, 7, 6],
                [3, 6, 2],
                [0, 4, 7],
                [0, 7, 3],
                [1, 2, 6],
                [1, 6, 5],
            ],
        }
    }
    pub fn octahedron(radius: f32) -> Self {
        let r = radius;
        Self {
            vertices: vec![
                Vec3::new(r, 0., 0.),
                Vec3::new(-r, 0., 0.),
                Vec3::new(0., r, 0.),
                Vec3::new(0., -r, 0.),
                Vec3::new(0., 0., r),
                Vec3::new(0., 0., -r),
            ],
            triangles: vec![
                [4, 0, 2],
                [4, 2, 1],
                [4, 1, 3],
                [4, 3, 0],
                [5, 2, 0],
                [5, 1, 2],
                [5, 3, 1],
                [5, 0, 3],
            ],
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    pub color: Rgb,
    pub ambient: f32,
    pub diffuse: f32,
    pub emissive: f32,
}
impl Default for Material {
    fn default() -> Self {
        Self {
            color: (60, 200, 255),
            ambient: 0.22,
            diffuse: 0.78,
            emissive: 0.,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fog {
    pub color: Rgb,
    pub start: f32,
    pub end: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RasterStats {
    pub triangles_submitted: u64,
    pub triangles_drawn: u64,
    pub z_tests: u64,
}
#[derive(Debug, Clone)]
pub struct Rasterizer {
    pub raster: RgbRaster,
    pub light: Vec3,
    pub fog: Option<Fog>,
    pub cull_backfaces: bool,
    pub stats: RasterStats,
    depth: Vec<f64>,
    depth_size: (u16, u16),
}
impl Rasterizer {
    pub fn new(width: u16, height: u16) -> Self {
        let raster = RgbRaster::new(width, height);
        let depth = vec![f64::INFINITY; raster.pixels().len()];
        let depth_size = (raster.width(), raster.height());
        Self {
            raster,
            depth,
            depth_size,
            light: Vec3::new(-0.5, 0.8, -1.),
            fog: None,
            cull_backfaces: false,
            stats: RasterStats::default(),
        }
    }
    /// Clear color, depth and per-frame counters together.
    pub fn clear(&mut self, color: Rgb) {
        self.sync_depth();
        self.raster.clear(color);
        self.depth.fill(f64::INFINITY);
        self.stats = RasterStats::default();
    }
    pub fn depth(&self, x: i32, y: i32) -> Option<f32> {
        if self.depth_size != (self.raster.width(), self.raster.height()) {
            return None;
        }
        self.raster
            .get(x, y)
            .and_then(|_| {
                self.depth
                    .get(y as usize * self.raster.width() as usize + x as usize)
            })
            .map(|z| *z as f32)
    }
    pub fn draw_mesh(
        &mut self,
        mesh: &TriangleMesh,
        transform: Transform3,
        camera: &Camera,
        material: Material,
    ) {
        // Public raster replacement is supported; restore matching depth storage.
        self.sync_depth();
        let Some(view) = camera.view(self.raster.width(), self.raster.height()) else {
            return;
        };
        for indices in &mesh.triangles {
            self.stats.triangles_submitted = self.stats.triangles_submitted.saturating_add(1);
            let Some(a) = mesh.vertices.get(indices[0]) else {
                continue;
            };
            let Some(b) = mesh.vertices.get(indices[1]) else {
                continue;
            };
            let Some(c) = mesh.vertices.get(indices[2]) else {
                continue;
            };
            let vertices = [
                transform.apply(*a),
                transform.apply(*b),
                transform.apply(*c),
            ];
            if vertices.iter().any(|v| !v.is_finite()) {
                continue;
            }
            self.triangle(vertices.map(p), &view, material);
        }
    }
    pub fn draw_triangle(&mut self, vertices: [Vec3; 3], camera: &Camera, material: Material) {
        self.sync_depth();
        self.stats.triangles_submitted = self.stats.triangles_submitted.saturating_add(1);
        if vertices.iter().any(|v| !v.is_finite()) {
            return;
        }
        if let Some(view) = camera.view(self.raster.width(), self.raster.height()) {
            self.triangle(vertices.map(p), &view, material);
        }
    }
    fn sync_depth(&mut self) {
        if self.depth_size != (self.raster.width(), self.raster.height()) {
            self.depth_size = (self.raster.width(), self.raster.height());
            self.depth = vec![f64::INFINITY; self.raster.pixels().len()];
        }
    }
    fn triangle(&mut self, world: [Point; 3], view: &View, material: Material) {
        if !material.ambient.is_finite()
            || !material.diffuse.is_finite()
            || !material.emissive.is_finite()
        {
            return;
        }
        let Some(normal) = unit(cross(sub(world[1], world[0]), sub(world[2], world[0]))) else {
            return;
        };
        if self.cull_backfaces && dot(normal, sub(world[0], view.position)) >= 0. {
            return;
        }
        let diffuse = unit(p(self.light)).map_or(0., |light| dot(normal, light).max(0.));
        let intensity = (material.ambient as f64
            + material.diffuse as f64 * diffuse
            + material.emissive as f64)
            .clamp(0., 8.);
        let lit = (
            material.color.0 as f64 * intensity,
            material.color.1 as f64 * intensity,
            material.color.2 as f64 * intensity,
        );
        let polygon = view.clip(world.map(|v| view.camera(v)));
        let mut drawn = false;
        for i in 1..polygon.len().saturating_sub(1) {
            drawn |= self.fill(
                [
                    view.project(polygon[0]),
                    view.project(polygon[i]),
                    view.project(polygon[i + 1]),
                ],
                lit,
            );
        }
        if drawn {
            self.stats.triangles_drawn = self.stats.triangles_drawn.saturating_add(1);
        }
    }
    fn fill(&mut self, v: [Point; 3], lit: (f64, f64, f64)) -> bool {
        let edge = |a: Point, b: Point, x: f64, y: f64| {
            (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0])
        };
        let area = edge(v[0], v[1], v[2][0], v[2][1]);
        if area.abs() < 1e-12 || !area.is_finite() {
            return false;
        }
        let x0 = v
            .iter()
            .map(|p| p[0])
            .fold(f64::INFINITY, f64::min)
            .floor()
            .max(0.) as i32;
        let x1 = v
            .iter()
            .map(|p| p[0])
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil()
            .min(self.raster.width() as f64 - 1.) as i32;
        let y0 = v
            .iter()
            .map(|p| p[1])
            .fold(f64::INFINITY, f64::min)
            .floor()
            .max(0.) as i32;
        let y1 = v
            .iter()
            .map(|p| p[1])
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil()
            .min(self.raster.height() as f64 - 1.) as i32;
        let mut drawn = false;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                let a = edge(v[1], v[2], px, py) / area;
                let b = edge(v[2], v[0], px, py) / area;
                let c = 1. - a - b;
                if a < -1e-10 || b < -1e-10 || c < -1e-10 {
                    continue;
                }
                let inverse = a * v[0][2] + b * v[1][2] + c * v[2][2];
                if inverse <= 0. || !inverse.is_finite() {
                    continue;
                }
                let depth = 1. / inverse;
                let color = self.shade(lit, depth);
                drawn |= self.write_depth(x, y, depth, color);
            }
        }
        drawn
    }
    fn shade(&self, lit: (f64, f64, f64), depth: f64) -> Rgb {
        let (fog, t) = match self.fog {
            Some(f) if f.start.is_finite() && f.end.is_finite() && f.end > f.start => (
                f.color,
                ((depth - f.start as f64) / (f.end as f64 - f.start as f64)).clamp(0., 1.),
            ),
            _ => ((0, 0, 0), 0.),
        };
        let c = |v: f64, f: u8| (v * (1. - t) + f as f64 * t).round().clamp(0., 255.) as u8;
        (c(lit.0, fog.0), c(lit.1, fog.1), c(lit.2, fog.2))
    }
    fn write_depth(&mut self, x: i32, y: i32, depth: f64, color: Rgb) -> bool {
        if self.raster.get(x, y).is_none() {
            return false;
        }
        self.stats.z_tests = self.stats.z_tests.saturating_add(1);
        let index = y as usize * self.raster.width() as usize + x as usize;
        // Exact coplanar ties choose a stable color, independently of submission order.
        if depth < self.depth[index]
            || (depth == self.depth[index] && color > self.raster.pixels()[index])
        {
            self.depth[index] = depth;
            self.raster.set(x, y, color);
            true
        } else {
            false
        }
    }
    /// Depth-tested world segment, clipped to all six camera planes. Lines are
    /// emissive and use the same reciprocal-depth interpolation as triangles.
    pub fn line(&mut self, a: Vec3, b: Vec3, camera: &Camera, color: Rgb) {
        self.sync_depth();
        if !a.is_finite() || !b.is_finite() {
            return;
        }
        let Some(view) = camera.view(self.raster.width(), self.raster.height()) else {
            return;
        };
        let (mut a, mut b) = (view.camera(p(a)), view.camera(p(b)));
        for plane in 0..6 {
            let (da, db) = (view.distance(a, plane), view.distance(b, plane));
            if da < 0. && db < 0. {
                return;
            }
            if (da >= 0.) != (db >= 0.) {
                let q = lerp(a, b, da / (da - db));
                if da < 0. {
                    a = q;
                } else {
                    b = q;
                }
            }
        }
        let (a, b) = (view.project(a), view.project(b));
        // Even cancellation at extreme finite coordinates cannot extend a walk
        // beyond the framebuffer dimensions.
        let steps = ((b[0] - a[0]).abs().max((b[1] - a[1]).abs()).ceil() as usize)
            .min(self.raster.width().max(self.raster.height()) as usize);
        for step in 0..=steps {
            let t = if steps == 0 {
                0.
            } else {
                step as f64 / steps as f64
            };
            let q = lerp(a, b, t);
            if q[2] > 0. {
                let depth = 1. / q[2];
                let color = self.shade((color.0 as f64, color.1 as f64, color.2 as f64), depth);
                self.write_depth(q[0].floor() as i32, q[1].floor() as i32, depth, color);
            }
        }
    }
}
