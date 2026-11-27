//! Narrow-band collision on actual unmasked triangular surfaces. Each spatial bin
//! stores triangle indices in input order. The field is rebuilt from the current pose.
use super::Vec3;
use std::collections::HashMap;

const CELL: f32 = 0.06;
const BAND: f32 = 0.028;
fn index(p: Vec3) -> [i32; 3] {
    [
        (p.x / CELL).floor() as i32,
        (p.y / CELL).floor() as i32,
        (p.z / CELL).floor() as i32,
    ]
}

#[derive(Clone, Copy)]
struct Tri {
    a: Vec3,
    b: Vec3,
    c: Vec3,
    normal: Vec3,
}

fn segment(a: Vec3, b: Vec3, p: Vec3) -> Vec3 {
    let ab = b.sub(a);
    a.add(ab.scale((p.sub(a).dot(ab) / ab.dot(ab).max(1e-16)).clamp(0., 1.)))
}

impl Tri {
    fn nearest(self, p: Vec3) -> Vec3 {
        let point = p.sub(self.normal.scale(p.sub(self.a).dot(self.normal)));
        let edges = [(self.a, self.b), (self.b, self.c), (self.c, self.a)];
        if edges
            .iter()
            .all(|&(a, b)| b.sub(a).cross(point.sub(a)).dot(self.normal) >= 0.)
        {
            return point;
        }
        let mut nearest = self.a;
        let mut best = f32::MAX;
        for (a, b) in edges {
            let q = segment(a, b, p);
            let d = q.sub(p);
            let d2 = d.dot(d);
            if d2 < best {
                best = d2;
                nearest = q;
            }
        }
        nearest
    }
}

pub(super) struct Surface {
    faces: Vec<Tri>,
    bins: HashMap<[i32; 3], Vec<usize>>,
}
impl Surface {
    pub(super) fn new() -> Self {
        Self {
            faces: Vec::new(),
            bins: HashMap::new(),
        }
    }
    pub(super) fn insert(&mut self, v: &[Vec3], indices: impl Iterator<Item = [u32; 3]>) {
        for xyz in indices {
            let [a, b, c] = xyz.map(|i| v[i as usize]);
            let Some(normal) = b.sub(a).cross(c.sub(a)).try_normalize() else {
                continue;
            };
            let lo = index(Vec3::new(
                a.x.min(b.x).min(c.x) - BAND,
                a.y.min(b.y).min(c.y) - BAND,
                a.z.min(b.z).min(c.z) - BAND,
            ));
            let hi = index(Vec3::new(
                a.x.max(b.x).max(c.x) + BAND,
                a.y.max(b.y).max(c.y) + BAND,
                a.z.max(b.z).max(c.z) + BAND,
            ));
            let id = self.faces.len();
            self.faces.push(Tri { a, b, c, normal });
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        self.bins.entry([x, y, z]).or_default().push(id);
                    }
                }
            }
        }
    }
    pub(super) fn project(&self, p: Vec3, thickness: f32) -> Vec3 {
        let Some(ids) = self.bins.get(&index(p)) else {
            return p;
        };
        let mut best = BAND * BAND;
        let mut result = p;
        for &i in ids {
            let face = self.faces[i];
            // Distance to the supporting plane is a lower bound for any triangle point.
            let plane = p.sub(face.a).dot(face.normal);
            if plane * plane >= best {
                continue;
            }
            let q = face.nearest(p);
            let diff = p.sub(q);
            let d2 = diff.dot(diff);
            if d2 >= best {
                continue;
            }
            best = d2;
            result = if plane < thickness {
                q.add(face.normal.scale(thickness))
            } else {
                p
            };
        }
        result
    }
}
