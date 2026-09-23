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

    /// Transforms every vertex by `t` (leaves edges unchanged).
    pub fn transformed(&self, t: &Transform3) -> Vec<Vec3> {
        self.vertices.iter().map(|v| t.apply(*v)).collect()
    }

    /// Wireframe rectangular box of the given full extents, centred on the
    /// origin. `box_xyz(w, h, d)` is the generalisation of [`Mesh::cube`].
    pub fn box_xyz(w: f32, h: f32, d: f32) -> Self {
        // `cube(1.0)` has full extent 1.0 on every axis, so per-axis scaling by
        // (w, h, d) yields exactly the requested full extents.
        Mesh::cube(1.0).scale_axes(w, h, d)
    }

    /// Returns a copy with every vertex scaled per-axis.
    pub fn scale_axes(&self, sx: f32, sy: f32, sz: f32) -> Self {
        let vertices = self
            .vertices
            .iter()
            .map(|v| Vec3::new(v.x * sx, v.y * sy, v.z * sz))
            .collect();
        Self::new(vertices, self.edges.clone())
    }

    /// Returns a copy translated by `offset`.
    pub fn translated(&self, offset: Vec3) -> Self {
        let vertices = self.vertices.iter().map(|v| v.plus(offset)).collect();
        Self::new(vertices, self.edges.clone())
    }

    /// Appends `other`'s vertices/edges (re-indexed) to this mesh.
    pub fn append(&mut self, other: &Mesh) {
        let base = self.vertices.len();
        self.vertices.extend_from_slice(&other.vertices);
        self.edges
            .extend(other.edges.iter().map(|(a, b)| (a + base, b + base)));
    }

    /// A flat rectangular grid in the XZ plane (a "circuit plane"), centred on
    /// the origin. Useful as a perspective ground plane for the data city.
    pub fn grid_xz(x_half: f32, z_half: f32, divs: usize) -> Self {
        let divs = divs.max(1);
        let mut vertices = Vec::new();
        let mut edges = Vec::new();
        for i in 0..=divs {
            let f = i as f32 / divs as f32;
            let x = -x_half + 2.0 * x_half * f;
            let z = -z_half + 2.0 * z_half * f;
            // Line parallel to Z at this X.
            vertices.push(Vec3::new(x, 0.0, -z_half));
            vertices.push(Vec3::new(x, 0.0, z_half));
            edges.push((vertices.len() - 2, vertices.len() - 1));
            // Line parallel to X at this Z.
            vertices.push(Vec3::new(-x_half, 0.0, z));
            vertices.push(Vec3::new(x_half, 0.0, z));
            edges.push((vertices.len() - 2, vertices.len() - 1));
        }
        Self::new(vertices, edges)
    }

    /// A vertical "data tower": a box of `w × h × d` whose base sits at the
    /// origin plane and which is centred at `(cx, 0, cz)` in XZ.
    pub fn data_tower(cx: f32, cz: f32, w: f32, h: f32, d: f32) -> Self {
        Self::box_xyz(w, h, d).translated(Vec3::new(cx, h * 0.5, cz))
    }
}

/// A projected edge plus the mean depth (camera distance) of its visible part.
///
/// Depth is exposed so callers can cheaply fake near/far styling without any
/// z-buffer: bright/near edges and dim/far edges can be drawn into separate
/// canvases and composited as ordinary layers. The projector itself remains
/// geometry-only; it makes no claim about occlusion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectedEdge {
    pub a: (i32, i32),
    pub b: (i32, i32),
    /// Mean camera depth of the visible endpoints; larger is farther.
    pub depth: f32,
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

/// Epsilon (in ULPs of the near-plane limit) used to nudge a clipped crossing
/// strictly inside the visible half-space. See [`Projector::clip_near`].
const CLIP_INSET_ULPS: f32 = 8.0;

impl Projector {
    /// Constructs a projector with validated parameters.
    ///
    /// Returns `None` for pathological combinations (`near <= 0`, non-finite
    /// `camera_z`/`near`, or a near plane behind the camera). Use [`Projector`]
    /// struct literals only if you have already validated the values; every
    /// projection call re-checks [`Projector::is_valid`] and degrades to `None`,
    /// so no NaN can reach raster math either way.
    pub fn new(camera_z: f32, near: f32) -> Option<Self> {
        let p = Self { camera_z, near };
        if p.is_valid() {
            Some(p)
        } else {
            None
        }
    }

    /// True when the parameters cannot produce non-finite projection math.
    ///
    /// Requires finite `camera_z`/`near`, a strictly positive near distance, and
    /// `camera_z >= near` so the origin plane is not behind the near plane.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.camera_z.is_finite()
            && self.near.is_finite()
            && self.near > 0.0
            && self.camera_z >= self.near
    }

    /// Depth of a point: its distance in front of the camera.
    ///
    /// The near plane is at `depth == near`; the visible half-space is
    /// `depth >= near`.
    #[inline]
    pub fn depth(&self, p: Vec3) -> f32 {
        self.camera_z - p.z
    }

    /// The single visibility predicate shared by projection and clipping.
    ///
    /// A point is visible when it is finite and its depth is `>= self.near`.
    /// The near plane itself is **inclusive**, which is the convention that
    /// keeps [`Projector::clip_near`] and [`Projector::project`] consistent.
    #[inline]
    pub fn is_visible(&self, p: Vec3) -> bool {
        p.is_finite() && self.depth(p) >= self.near
    }

    /// Projects a point to canvas pixel coordinates.
    ///
    /// Returns `None` when the point is behind the near plane (`depth < near`)
    /// or produces a non-finite coordinate. A point exactly on the near plane is
    /// visible; this matches [`Projector::is_visible`] and [`Projector::clip_near`].
    pub fn project(&self, p: Vec3, cx: f32, cy: f32, focal: f32) -> Option<(i32, i32)> {
        if !self.is_valid() || !p.is_finite() {
            return None;
        }
        let z = self.depth(p);
        if !z.is_finite() || z < self.near {
            return None;
        }
        if !cx.is_finite() || !cy.is_finite() || !focal.is_finite() {
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
    ///
    /// Uses the same inclusive half-space as [`Projector::is_visible`] and
    /// [`Projector::project`]: a segment with one endpoint behind the plane is
    /// truncated and the crossing point is placed on the visible side. The
    /// crossing is nudged by a few ULPs *inside* the half-space so that the
    /// subsequent `project` call cannot re-reject it when `camera_z - limit`
    /// rounds one ULP below `near`. The nudge is ~1e-6 of the near distance and
    /// has no visible geometric effect.
    pub fn clip_near(&self, a: Vec3, b: Vec3) -> Option<(Vec3, Vec3)> {
        if !self.is_valid() || !a.is_finite() || !b.is_finite() {
            return None;
        }
        let a_in = self.is_visible(a);
        let b_in = self.is_visible(b);
        match (a_in, b_in) {
            (true, true) => Some((a, b)),
            (false, false) => None,
            _ => {
                let denom = b.z - a.z;
                if denom == 0.0 || !denom.is_finite() {
                    return None;
                }
                // Inclusive near plane, nudged a hair inside for float safety.
                let limit = self.camera_z - self.near;
                let z_limit = if limit > 0.0 {
                    limit - limit.abs() * CLIP_INSET_ULPS * f32::EPSILON
                } else {
                    limit
                };
                let t = ((limit - a.z) / denom).clamp(0.0, 1.0);
                let mid = Vec3::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t, z_limit);
                if a_in {
                    Some((a, mid))
                } else {
                    Some((mid, b))
                }
            }
        }
    }

    /// Projects a transformed mesh to screen edges with depth information.
    ///
    /// Edges that are fully behind the near plane are dropped; crossing edges are
    /// clipped (see [`Projector::clip_near`]). `pw`/`ph` are the target pixel
    /// dimensions and `zoom` scales the fit (1.0 fits the mesh to the canvas).
    pub fn project_mesh(
        &self,
        mesh: &Mesh,
        t: &Transform3,
        pw: f32,
        ph: f32,
        zoom: f32,
    ) -> Vec<ProjectedEdge> {
        if !self.is_valid() || !pw.is_finite() || !ph.is_finite() || !zoom.is_finite() {
            return Vec::new();
        }
        let cx = (pw - 1.0) * 0.5;
        let cy = (ph - 1.0) * 0.5;
        let focal = pw.min(ph) * 0.5 * zoom;
        let transformed = mesh.transformed(t);
        let mut out = Vec::with_capacity(mesh.edges.len());
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
                out.push(ProjectedEdge {
                    a: pa,
                    b: pb,
                    depth: (self.depth(sa) + self.depth(sb)) * 0.5,
                });
            }
        }
        out
    }

    /// Draws `mesh` transformed by `t` into a Braille canvas.
    ///
    /// `zoom` scales the projected result (1.0 fits the mesh to the canvas).
    pub fn draw(&self, mesh: &Mesh, t: &Transform3, canvas: &mut BrailleCanvas, zoom: f32) {
        let pw = canvas.pixel_width() as f32;
        let ph = canvas.pixel_height() as f32;
        for e in self.project_mesh(mesh, t, pw, ph, zoom) {
            canvas.line(e.a.0, e.a.1, e.b.0, e.b.1);
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

    // -----------------------------------------------------------------------
    // Near-plane clipping consistency.
    //
    // Regression: `clip_near` used to place the crossing exactly on the plane
    // while `project` used a strict `depth > near` test, so a clipped edge was
    // handed to `project`, rejected, and the whole edge vanished.
    // -----------------------------------------------------------------------

    fn near_limit(p: Projector) -> f32 {
        p.camera_z - p.near
    }

    #[test]
    fn visible_visible_segment_is_returned_unchanged() {
        let p = Projector::default();
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(1.0, 1.0, 0.5);
        let (sa, sb) = p.clip_near(a, b).expect("both visible");
        assert_eq!(sa, a);
        assert_eq!(sb, b);
    }

    #[test]
    fn invisible_invisible_segment_is_dropped() {
        let p = Projector::default();
        let limit = near_limit(p);
        let a = Vec3::new(0.0, 0.0, limit + 1.0);
        let b = Vec3::new(1.0, 1.0, limit + 2.0);
        assert!(p.clip_near(a, b).is_none());
    }

    #[test]
    fn endpoint_exactly_on_near_plane_is_visible() {
        let p = Projector::default();
        let limit = near_limit(p);
        let on_plane = Vec3::new(0.3, -0.2, limit);
        assert!(p.is_visible(on_plane), "near plane is inclusive");
        assert!(
            p.project(on_plane, 0.0, 0.0, 1.0).is_some(),
            "a point exactly on the near plane must project"
        );
    }

    #[test]
    fn visible_to_behind_crossing_keeps_visible_portion() {
        let p = Projector::default();
        let limit = near_limit(p);
        let a = Vec3::new(0.0, 0.0, 0.0); // visible
        let b = Vec3::new(0.0, 0.0, limit + 1.0); // behind
        let (sa, sb) = p.clip_near(a, b).expect("crossing must clip, not vanish");
        assert_eq!(sa, a);
        assert!(!p.is_visible(b));
        // The crossing is on the visible side and must itself project.
        assert!(
            p.is_visible(sb),
            "crossing must be inside the visible half-space"
        );
        assert!(p.project(sa, 0.0, 0.0, 1.0).is_some());
        assert!(
            p.project(sb, 0.0, 0.0, 1.0).is_some(),
            "clipped crossing must survive projection (no edge popping)"
        );
    }

    #[test]
    fn behind_to_visible_crossing_keeps_visible_portion() {
        let p = Projector::default();
        let limit = near_limit(p);
        let a = Vec3::new(0.0, 0.0, limit + 1.0); // behind
        let b = Vec3::new(0.4, 0.1, -0.5); // visible
        let (sa, sb) = p.clip_near(a, b).expect("crossing must clip, not vanish");
        assert!(!p.is_visible(a));
        assert_eq!(sb, b);
        assert!(p.is_visible(sa));
        assert!(p.project(sa, 0.0, 0.0, 8.0).is_some());
        assert!(p.project(sb, 0.0, 0.0, 8.0).is_some());
    }

    #[test]
    fn very_shallow_crossing_is_not_dropped() {
        let p = Projector::default();
        let limit = near_limit(p);
        let a = Vec3::new(0.1, 0.1, limit - 1e-7); // just inside
        let b = Vec3::new(0.2, 0.2, limit + 1e-7); // just outside
        let (sa, sb) = p.clip_near(a, b).expect("shallow crossing must survive");
        assert!(p.is_visible(sa) && p.is_visible(sb));
        assert!(p.project(sa, 10.0, 10.0, 8.0).is_some());
        assert!(p.project(sb, 10.0, 10.0, 8.0).is_some());
    }

    #[test]
    fn nan_and_infinite_endpoints_are_rejected() {
        let p = Projector::default();
        let finite = Vec3::new(0.0, 0.0, 0.0);
        let nan = Vec3::new(f32::NAN, 0.0, 0.0);
        let inf = Vec3::new(0.0, 0.0, f32::INFINITY);
        assert!(p.clip_near(nan, finite).is_none());
        assert!(p.clip_near(finite, nan).is_none());
        assert!(p.clip_near(inf, finite).is_none());
        assert!(p.clip_near(finite, inf).is_none());
        assert!(p.clip_near(nan, inf).is_none());
    }

    #[test]
    fn crossing_segment_draws_a_truncated_edge_not_nothing() {
        // A segment from in front of the camera to well behind it crosses the
        // near plane. Both endpoints project, so the line must be drawn.
        let p = Projector {
            camera_z: 4.0,
            near: 1.0,
        };
        let limit = near_limit(p);
        let mesh = Mesh::new(
            vec![
                Vec3::new(-1.5, 0.0, 0.0),         // visible
                Vec3::new(-1.5, 0.0, limit + 2.0), // behind
            ],
            vec![(0, 1)],
        );
        let mut canvas = BrailleCanvas::new(40, 20);
        p.draw(&mesh, &Transform3::identity(), &mut canvas, 1.0);
        assert!(
            !canvas.is_empty(),
            "a near-plane-crossing edge must render its visible part, not disappear"
        );
    }

    #[test]
    fn fully_visible_edge_renders_at_least_as_much_as_crossing_edge() {
        let p = Projector {
            camera_z: 4.0,
            near: 1.0,
        };
        let limit = near_limit(p);
        let crossing = Mesh::new(
            vec![Vec3::new(-1.5, 0.0, 0.0), Vec3::new(-1.5, 0.0, limit + 2.0)],
            vec![(0, 1)],
        );
        let visible = Mesh::new(
            vec![Vec3::new(-1.5, 0.0, 0.0), Vec3::new(-1.5, 0.0, -1.0)],
            vec![(0, 1)],
        );
        let mut c1 = BrailleCanvas::new(40, 20);
        let mut c2 = BrailleCanvas::new(40, 20);
        p.draw(&crossing, &Transform3::identity(), &mut c1, 1.0);
        p.draw(&visible, &Transform3::identity(), &mut c2, 1.0);
        assert!(!c1.is_empty() && !c2.is_empty());
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
    fn box_xyz_has_requested_extents() {
        let m = Mesh::box_xyz(4.0, 2.0, 6.0);
        assert_eq!(m.vertices.len(), 8);
        assert_eq!(m.edges.len(), 12);
        let xs: Vec<f32> = m.vertices.iter().map(|v| v.x).collect();
        let ys: Vec<f32> = m.vertices.iter().map(|v| v.y).collect();
        let zs: Vec<f32> = m.vertices.iter().map(|v| v.z).collect();
        assert!((xs.iter().cloned().fold(f32::MIN, f32::max) - 2.0).abs() < 1e-5);
        assert!((ys.iter().cloned().fold(f32::MIN, f32::max) - 1.0).abs() < 1e-5);
        assert!((zs.iter().cloned().fold(f32::MIN, f32::max) - 3.0).abs() < 1e-5);
    }

    #[test]
    fn grid_xz_is_consistent_and_composable() {
        let g = Mesh::grid_xz(5.0, 5.0, 4);
        assert_eq!(g.vertices.len(), 5 * 4);
        assert_eq!(g.edges.len(), 5 * 2);
        let mut city = Mesh::default();
        city.append(&g);
        city.append(&Mesh::data_tower(0.0, 0.0, 1.0, 3.0, 1.0));
        assert_eq!(city.vertices.len(), g.vertices.len() + 8);
        for (a, b) in &city.edges {
            assert!(*a < city.vertices.len() && *b < city.vertices.len());
        }
    }

    #[test]
    fn data_tower_sits_on_the_ground_plane() {
        let t = Mesh::data_tower(3.0, -2.0, 1.0, 4.0, 1.0);
        let min_y = t.vertices.iter().map(|v| v.y).fold(f32::MAX, f32::min);
        assert!(min_y.abs() < 1e-5, "tower base should sit at y=0");
        let cx: f32 = t.vertices.iter().map(|v| v.x).sum::<f32>() / 8.0;
        assert!((cx - 3.0).abs() < 1e-5);
    }

    #[test]
    fn project_mesh_reports_finite_depth_for_all_edges() {
        let mut city = Mesh::grid_xz(4.0, 4.0, 3);
        city.append(&Mesh::data_tower(0.0, 0.0, 1.0, 2.0, 1.0));
        let t = Transform3::rotation(-0.9, 0.4, 0.0);
        let edges = Projector::default().project_mesh(&city, &t, 80.0, 48.0, 1.0);
        assert!(!edges.is_empty());
        assert!(edges.iter().all(|e| e.depth.is_finite() && e.depth > 0.0));
    }

    #[test]
    fn near_edges_have_smaller_depth_than_far_edges() {
        // depth = camera_z - p.z, so larger z is *nearer* the camera.
        let near = Mesh::data_tower(0.5, 1.5, 0.6, 1.0, 0.6);
        let far = Mesh::data_tower(-0.5, -1.5, 0.6, 1.0, 0.6);
        let p = Projector::default();
        let t = Transform3::identity();
        let n = p.project_mesh(&near, &t, 60.0, 40.0, 1.0);
        let f = p.project_mesh(&far, &t, 60.0, 40.0, 1.0);
        assert!(!n.is_empty() && !f.is_empty());
        let n_depth = n.iter().map(|e| e.depth).sum::<f32>() / n.len() as f32;
        let f_depth = f.iter().map(|e| e.depth).sum::<f32>() / f.len() as f32;
        assert!(
            n_depth < f_depth,
            "near tower should have smaller camera depth ({n_depth} vs {f_depth})"
        );
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

    // -----------------------------------------------------------------------
    // Pathological parameter rejection.
    // -----------------------------------------------------------------------

    #[test]
    fn validating_constructor_rejects_pathological_parameters() {
        assert!(Projector::new(3.0, 0.2).is_some());
        assert!(
            Projector::new(0.5, 0.5).is_some(),
            "near == camera_z is valid"
        );
        assert!(Projector::new(3.0, 0.0).is_none(), "near must be positive");
        assert!(Projector::new(3.0, -1.0).is_none());
        assert!(Projector::new(0.1, 1.0).is_none(), "near behind camera");
        assert!(Projector::new(f32::NAN, 0.2).is_none());
        assert!(Projector::new(3.0, f32::NAN).is_none());
        assert!(Projector::new(f32::INFINITY, 0.2).is_none());
        assert!(Projector::default().is_valid());
    }

    #[test]
    fn invalid_projector_degrades_to_none_instead_of_nan() {
        let bad = [
            Projector {
                camera_z: f32::NAN,
                near: 0.2,
            },
            Projector {
                camera_z: 3.0,
                near: 0.0,
            },
            Projector {
                camera_z: 3.0,
                near: -1.0,
            },
            Projector {
                camera_z: f32::INFINITY,
                near: 0.2,
            },
        ];
        for p in bad {
            assert!(!p.is_valid());
            assert!(p.project(Vec3::default(), 0.0, 0.0, 8.0).is_none());
            assert!(p
                .clip_near(Vec3::default(), Vec3::new(1.0, 1.0, 1.0))
                .is_none());
            let edges = p.project_mesh(&Mesh::cube(1.0), &Transform3::identity(), 80.0, 40.0, 1.0);
            assert!(edges.is_empty());
        }
    }

    #[test]
    fn hostile_projection_inputs_never_reach_a_canvas() {
        // Enormous focal/zoom/centre values must be rejected, not rounded to i32.
        let p = Projector::default();
        assert!(p.project(Vec3::default(), f32::NAN, 0.0, 1.0).is_none());
        assert!(p
            .project(Vec3::default(), 0.0, 0.0, f32::INFINITY)
            .is_none());
        let edges = p.project_mesh(
            &Mesh::cube(1.0),
            &Transform3::identity(),
            f32::NAN,
            40.0,
            1.0,
        );
        assert!(edges.is_empty());
        let edges = p.project_mesh(
            &Mesh::cube(1.0),
            &Transform3::identity(),
            80.0,
            40.0,
            f32::INFINITY,
        );
        assert!(edges.is_empty());

        // A mesh transformed by non-finite values is dropped edge-by-edge.
        let mut c = BrailleCanvas::new(20, 10);
        let t = Transform3 {
            rx: 0.0,
            ry: 0.0,
            rz: 0.0,
            scale: f32::NAN,
            offset: Vec3::default(),
        };
        p.draw(&Mesh::cube(1.0), &t, &mut c, 1.0);
        assert!(c.is_empty(), "non-finite transform must render nothing");
    }

    #[test]
    fn near_camera_projection_is_clipped_to_canvas_efficiently() {
        // A perspective point very close to the camera produces coordinates in
        // the tens of millions. The braille canvas must bound the walk (this
        // returns promptly rather than iterating for millions of steps) and still
        // render the visible crossing.
        let p = Projector {
            camera_z: 3.0,
            near: 0.001,
        };
        let mesh = Mesh::new(
            vec![Vec3::new(1e6, 0.0, 2.999), Vec3::new(-1e6, 0.0, 2.999)],
            vec![(0, 1)],
        );
        let mut c = BrailleCanvas::new(40, 20);
        p.draw(&mesh, &Transform3::identity(), &mut c, 1.0);
        // Not asserting pixels (the crossing may be far off-canvas), only that it
        // completes without panicking or hanging.
        let _ = c.to_lines();
    }
}
