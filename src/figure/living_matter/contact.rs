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

/// Two-sided non-neighbour vertex/triangle contact on the coarse sewn panel. The previous
/// side of the triangle sets the outward orientation through an approaching fold.
pub(super) fn separate_sheet(
    pos: &mut [Vec3],
    old: &[Vec3],
    inv: &[f32],
    faces: &[[usize; 3]],
    cols: usize,
) {
    const GAP: f32 = 0.005;
    let point = |p: Vec3| {
        [
            (p.x / CELL).floor() as i32,
            (p.y / CELL).floor() as i32,
            (p.z / CELL).floor() as i32,
        ]
    };
    let mut bins: HashMap<[i32; 3], Vec<usize>> = HashMap::new();
    for (j, ids) in faces.iter().enumerate() {
        let [a, b, c] = ids.map(|i| pos[i]);
        let lo = point(Vec3::new(
            a.x.min(b.x).min(c.x) - GAP,
            a.y.min(b.y).min(c.y) - GAP,
            a.z.min(b.z).min(c.z) - GAP,
        ));
        let hi = point(Vec3::new(
            a.x.max(b.x).max(c.x) + GAP,
            a.y.max(b.y).max(c.y) + GAP,
            a.z.max(b.z).max(c.z) + GAP,
        ));
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    bins.entry([x, y, z]).or_default().push(j);
                }
            }
        }
    }
    for i in 0..pos.len() {
        let Some(candidates) = bins.get(&point(pos[i])) else {
            continue;
        };
        for &j in candidates {
            let ids = faces[j];
            if ids.iter().any(|&k| {
                let d = (k % cols).abs_diff(i % cols);
                (k / cols).abs_diff(i / cols) <= 1 && d.min(cols - d) <= 1
            }) {
                continue;
            }
            let [a, b, c] = ids.map(|k| pos[k]);
            let ab = b.sub(a);
            let ac = c.sub(a);
            let Some(mut n) = ab.cross(ac).try_normalize() else {
                continue;
            };
            let side = pos[i].sub(a).dot(n);
            if side.abs() > GAP {
                continue;
            }
            let prev = old[ids[1]]
                .sub(old[ids[0]])
                .cross(old[ids[2]].sub(old[ids[0]]));
            if old[i].sub(old[ids[0]]).dot(prev) < 0. {
                n = n.scale(-1.);
            }
            let tri = Tri {
                a,
                b,
                c,
                normal: ab.cross(ac).try_normalize().unwrap(),
            };
            let q = tri.nearest(pos[i]);
            let d = pos[i].sub(q);
            if d.dot(d) > GAP * GAP {
                continue;
            }
            let aq = q.sub(a);
            let xx = ab.dot(ab);
            let xy = ab.dot(ac);
            let yy = ac.dot(ac);
            let det = xx * yy - xy * xy;
            if det <= 1e-15 {
                continue;
            }
            let u = ((aq.dot(ab) * yy - aq.dot(ac) * xy) / det).clamp(0., 1.);
            let v = ((aq.dot(ac) * xx - aq.dot(ab) * xy) / det).clamp(0., 1. - u);
            let bary = [1. - u - v, u, v];
            let w = inv[i] + (0..3).map(|k| inv[ids[k]] * bary[k] * bary[k]).sum::<f32>();
            if w <= 0. {
                continue;
            }
            let corr = n.scale((GAP - d.dot(n)).max(0.) / w);
            pos[i] = pos[i].add(corr.scale(inv[i]));
            for k in 0..3 {
                pos[ids[k]] = pos[ids[k]].sub(corr.scale(inv[ids[k]] * bary[k]));
            }
        }
    }
}
