//! Tiny deterministic 3D vector math and a Braille wireframe projector.
//!
//! This is deliberately **not** a 3D engine. It provides just enough to rotate a
//! mesh, perspective-project its edges and draw them as sub-cell Braille lines:
//!
//! ```text
//! vertices + edges -> rotate -> near-plane clip -> perspective project -> Braille lines
//! ```
//!
//! Everything is a pure function of its inputs, so a wireframe frame is
//! reproducible under [`crate::FixedStepClock`]. Projection rejects non-finite
//! coordinates, so NaNs never reach the canvas.

use crate::canvas::BrailleCanvas;

/// A 3D point/vector.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn plus(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }

    pub fn minus(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }

    pub fn scale(self, s: f32) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }

    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalize(self) -> Vec3 {
        let l = self.length();
        if l <= 1e-6 {
            Vec3::default()
        } else {
            self.scale(1.0 / l)
        }
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    pub fn rotate_x(self, a: f32) -> Vec3 {
        let (s, c) = a.sin_cos();
        Vec3::new(self.x, self.y * c - self.z * s, self.y * s + self.z * c)
    }

    pub fn rotate_y(self, a: f32) -> Vec3 {
        let (s, c) = a.sin_cos();
        Vec3::new(self.x * c + self.z * s, self.y, -self.x * s + self.z * c)
    }

    pub fn rotate_z(self, a: f32) -> Vec3 {
        let (s, c) = a.sin_cos();
        Vec3::new(self.x * c - self.y * s, self.x * s + self.y * c, self.z)
    }
}

/// Rotation + uniform scale + translation applied before projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform3 {
    pub rx: f32,
    pub ry: f32,
    pub rz: f32,
    pub scale: f32,
    pub offset: Vec3,
}

impl Default for Transform3 {
    fn default() -> Self {
        Self {
            rx: 0.0,
            ry: 0.0,
            rz: 0.0,
            scale: 1.0,
            offset: Vec3::default(),
        }
    }
}

impl Transform3 {
    pub fn identity() -> Self {
        Self::default()
    }

    pub fn rotation(rx: f32, ry: f32, rz: f32) -> Self {
        Self {
            rx,
            ry,
            rz,
            ..Self::default()
        }
    }

    pub fn apply(&self, v: Vec3) -> Vec3 {
        v.rotate_x(self.rx)
            .rotate_y(self.ry)
            .rotate_z(self.rz)
            .scale(self.scale)
            .plus(self.offset)
    }
}

/// A vertex/edge mesh.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mesh {
    pub vertices: Vec<Vec3>,
    pub edges: Vec<(usize, usize)>,
}

impl Mesh {
    pub fn new(vertices: Vec<Vec3>, edges: Vec<(usize, usize)>) -> Self {
        Self { vertices, edges }
    }

    /// Axis-aligned cube with the given full edge length.
    pub fn cube(size: f32) -> Self {
        let h = size * 0.5;
        let v = vec![
            Vec3::new(-h, -h, -h),
            Vec3::new(h, -h, -h),
            Vec3::new(h, h, -h),
            Vec3::new(-h, h, -h),
            Vec3::new(-h, -h, h),
            Vec3::new(h, -h, h),
            Vec3::new(h, h, h),
            Vec3::new(-h, h, h),
        ];
        let e = vec![
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0), // back face
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4), // front face
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7), // connectors
        ];
        Self::new(v, e)
    }

    /// Regular octahedron with the given circumradius.
    pub fn octahedron(r: f32) -> Self {
        let v = vec![
            Vec3::new(r, 0.0, 0.0),
            Vec3::new(-r, 0.0, 0.0),
            Vec3::new(0.0, r, 0.0),
            Vec3::new(0.0, -r, 0.0),
            Vec3::new(0.0, 0.0, r),
            Vec3::new(0.0, 0.0, -r),
        ];
        let e = vec![
            (0, 2),
            (2, 1),
            (1, 3),
            (3, 0), // equator ring
            (4, 0),
            (4, 1),
            (4, 2),
            (4, 3), // top vertex
            (5, 0),
            (5, 1),
            (5, 2),
            (5, 3), // bottom vertex
        ];
        Self::new(v, e)
    }

    /// Wireframe torus.
    pub fn torus(ring_radius: f32, tube_radius: f32, segments: usize, rings: usize) -> Self {
        let segments = segments.max(3);
        let rings = rings.max(3);
        let mut vertices = Vec::with_capacity(segments * rings);
        for i in 0..segments {
            let u = std::f32::consts::TAU * i as f32 / segments as f32;
            for j in 0..rings {
                let v = std::f32::consts::TAU * j as f32 / rings as f32;
                let r = ring_radius + tube_radius * v.cos();
                vertices.push(Vec3::new(r * u.cos(), tube_radius * v.sin(), r * u.sin()));
            }
        }
        let idx = |i: usize, j: usize| (i % segments) * rings + (j % rings);
        let mut edges = Vec::new();
        for i in 0..segments {
            for j in 0..rings {
                edges.push((idx(i, j), idx(i + 1, j)));
                edges.push((idx(i, j), idx(i, j + 1)));
            }
        }
        Self::new(vertices, edges)
    }

    pub fn transformed(&self, t: &Transform3) -> Vec<Vec3> {
        self.vertices.iter().map(|v| t.apply(*v)).collect()
    }
}

/// Perspective projector with near-plane clipping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projector {
    /// Distance from the camera to the origin plane.
    pub camera_z: f32,
    /// Near-plane distance; segments closer than this are clipped.
    pub near: f32,
}

impl Default for Projector {
    fn default() -> Self {
        Self {
            camera_z: 3.0,
            near: 0.2,
        }
    }
}

impl Projector {
    /// Projects a point to canvas pixel coordinates. Returns `None` when the
    /// point is at/behind the near plane or produces a non-finite coordinate.
    pub fn project(&self, p: Vec3, cx: f32, cy: f32, focal: f32) -> Option<(i32, i32)> {
        if !p.is_finite() {
            return None;
        }
        let z = self.camera_z - p.z;
        if z <= self.near {
            return None;
        }
        let x = cx + p.x * focal / z;
        let y = cy - p.y * focal / z;
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        Some((x.round() as i32, y.round() as i32))
    }

    /// Clips a segment against the near plane, returning the visible portion.
    pub fn clip_near(&self, a: Vec3, b: Vec3) -> Option<(Vec3, Vec3)> {
        let limit = self.camera_z - self.near;
        let a_in = a.z < limit;
        let b_in = b.z < limit;
        match (a_in, b_in) {
            (true, true) => Some((a, b)),
            (false, false) => None,
            _ => {
                let denom = b.z - a.z;
                if denom.abs() < 1e-6 {
                    return None;
                }
                let t = (limit - a.z) / denom;
                let mid = Vec3::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t, limit);
                if a_in {
                    Some((a, mid))
                } else {
                    Some((mid, b))
                }
            }
        }
    }

    /// Draws `mesh` transformed by `t` into a Braille canvas.
    ///
    /// `zoom` scales the projected result (1.0 fits the mesh to the canvas).
    pub fn draw(&self, mesh: &Mesh, t: &Transform3, canvas: &mut BrailleCanvas, zoom: f32) {
        let pw = canvas.pixel_width() as f32;
        let ph = canvas.pixel_height() as f32;
        let cx = (pw - 1.0) * 0.5;
        let cy = (ph - 1.0) * 0.5;
        let focal = pw.min(ph) * 0.5 * zoom;

        let transformed = mesh.transformed(t);
        for (a, b) in &mesh.edges {
            if *a >= transformed.len() || *b >= transformed.len() {
                continue;
            }
            let Some((sa, sb)) = self.clip_near(transformed[*a], transformed[*b]) else {
                continue;
            };
            let pa = self.project(sa, cx, cy, focal);
            let pb = self.project(sb, cx, cy, focal);
            if let (Some(pa), Some(pb)) = (pa, pb) {
                canvas.line(pa.0, pa.1, pb.0, pb.1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_has_eight_vertices_and_twelve_edges() {
        let m = Mesh::cube(2.0);
        assert_eq!(m.vertices.len(), 8);
        assert_eq!(m.edges.len(), 12);
    }

    #[test]
    fn torus_topology_is_consistent() {
        let m = Mesh::torus(1.0, 0.35, 8, 6);
        assert_eq!(m.vertices.len(), 48);
        assert_eq!(m.edges.len(), 8 * 6 * 2);
        for (a, b) in &m.edges {
            assert!(*a < m.vertices.len() && *b < m.vertices.len());
        }
    }

    #[test]
    fn identity_projection_maps_origin_to_centre() {
        let p = Projector::default();
        let (x, y) = p.project(Vec3::default(), 10.0, 5.0, 8.0).unwrap();
        assert_eq!((x, y), (10, 5));
    }

    #[test]
    fn points_behind_near_plane_are_rejected() {
        let p = Projector {
            camera_z: 3.0,
            near: 0.5,
        };
        assert!(p.project(Vec3::new(0.0, 0.0, 3.0), 0.0, 0.0, 1.0).is_none());
    }

    #[test]
    fn projection_is_deterministic_and_finite_across_rotations() {
        let mesh = Mesh::cube(1.5);
        for i in 0..24 {
            let t = Transform3::rotation(i as f32 * 0.3, i as f32 * 0.17, i as f32 * 0.11);
            let mut a = BrailleCanvas::new(20, 10);
            let mut b = BrailleCanvas::new(20, 10);
            Projector::default().draw(&mesh, &t, &mut a, 1.0);
            Projector::default().draw(&mesh, &t, &mut b, 1.0);
            assert_eq!(a, b, "wireframe must be deterministic");
            for v in mesh.transformed(&t) {
                assert!(v.is_finite());
            }
        }
    }

    #[test]
    fn wireframe_draws_some_dots() {
        let mut c = BrailleCanvas::new(24, 12);
        Projector::default().draw(
            &Mesh::cube(1.6),
            &Transform3::rotation(0.7, 0.9, 0.0),
            &mut c,
            1.0,
        );
        assert!(c.to_lines().iter().any(|l| !l.trim().is_empty()));
    }

    #[test]
    fn nan_input_never_reaches_canvas() {
        let p = Projector::default();
        assert!(p
            .project(Vec3::new(f32::NAN, 0.0, 0.0), 0.0, 0.0, 1.0)
            .is_none());
    }
}
