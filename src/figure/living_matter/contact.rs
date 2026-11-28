//! Narrow-band collision on actual unmasked triangular surfaces. Each spatial bin
//! stores triangle indices in input order. The field is rebuilt from the current pose.
//! [`Collider`] indexes only the bins the simulated particles can reach and falls back to the
//! full surface for any other query, so its answers are those of the full surface.
use super::Vec3;
use std::{cell::OnceCell, collections::HashMap};

const CELL: f32 = 0.06;
const BAND: f32 = 0.028;
/// `v.floor() as i32` for every `f32` (NaN, infinities and saturation included) without the
/// software `floorf` call the baseline x86-64 target makes: truncate, then step down once if
/// the truncation rounded up.
fn floor_i32(v: f32) -> i32 {
    let t = v as i32;
    if (t as f32) > v {
        t.saturating_sub(1)
    } else {
        t
    }
}
fn index(p: Vec3) -> [i32; 3] {
    [
        floor_i32(p.x / CELL),
        floor_i32(p.y / CELL),
        floor_i32(p.z / CELL),
    ]
}

/// Reject a malformed or exploded primitive before expanding the broad phase.
fn account_cells(lo: [i32; 3], hi: [i32; 3], total: &mut usize) -> Result<(), &'static str> {
    let mut count = 1usize;
    for k in 0..3 {
        let span = i64::from(hi[k]) - i64::from(lo[k]) + 1;
        if !(1..=80).contains(&span) {
            return Err("collision primitive exceeds spatial budget");
        }
        count *= span as usize;
    }
    if count > 4096 || count > 2_000_000usize.saturating_sub(*total) {
        return Err("collision grid exceeds spatial budget");
    }
    *total += count;
    Ok(())
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
    postings: usize,
    bins: HashMap<[i32; 3], Vec<u32>>,
    /// When set, only these cells are indexed, densely (`dense[slot]`), and `bins` stays empty.
    reach: Option<Reach>,
    /// The reach dilated by one and by two cells. A triangle narrower than 0.49 cell in every
    /// axis can post to a reach cell only if each vertex lies in a cell of the first; one
    /// narrower than a cell, of the second (the band is 0.467 cell, so its bin range reaches
    /// at most 0.957 and 1.467 cells beyond a vertex's cell).
    outer: Option<Outer>,
    dense: Vec<Vec<u32>>,
}
impl Surface {
    pub(super) fn new() -> Self {
        Self {
            faces: Vec::new(),
            postings: 0,
            bins: HashMap::new(),
            reach: None,
            outer: None,
            dense: Vec::new(),
        }
    }
    /// A surface indexed only in the cells of `reach`.
    fn within(reach: Reach) -> Self {
        Self {
            dense: vec![Vec::new(); reach.cells.len()],
            outer: Some(Outer::new(&reach)),
            reach: Some(reach),
            ..Self::new()
        }
    }
    /// Whether `project(p)` reads the same bin as the full surface would.
    fn covers(&self, p: Vec3) -> bool {
        self.reach.as_ref().is_none_or(|r| r.contains(index(p)))
    }
    /// Insert triangles, validating each and charging it to the posting budget over its whole
    /// bin range in input order. With a reach, a triangle whose padded bounds cannot meet the
    /// reach's box is dropped before validation (it could only post to cells no query reads),
    /// and each indexed cell's bin holds exactly the full insert's triangles in the full
    /// insert's order. So a culled insert can succeed where the full insert refuses a
    /// primitive (or the total budget) out of reach; in reach the two agree.
    pub(super) fn insert(
        &mut self,
        v: &[Vec3],
        indices: impl Iterator<Item = [u32; 3]>,
    ) -> Result<(), &'static str> {
        let near = self.reach.as_ref().map(Reach::padded_bounds);
        if let Some((lo, hi)) = near {
            // A whole source outside the padded box cannot post anywhere in the reach.
            let (mut mn, mut mx) = (
                Vec3::new(f32::MAX, f32::MAX, f32::MAX),
                Vec3::new(f32::MIN, f32::MIN, f32::MIN),
            );
            for p in v {
                mn = Vec3::new(mn.x.min(p.x), mn.y.min(p.y), mn.z.min(p.z));
                mx = Vec3::new(mx.x.max(p.x), mx.y.max(p.y), mx.z.max(p.z));
            }
            if mx.x < lo.x
                || mn.x > hi.x
                || mx.y < lo.y
                || mn.y > hi.y
                || mx.z < lo.z
                || mn.z > hi.z
            {
                return Ok(());
            }
        }
        // Per vertex, evaluated on first use: bit 0 known, bit 1 in the one-cell dilation,
        // bit 2 in the two-cell dilation.
        let mut held = vec![0u8; if self.outer.is_some() { v.len() } else { 0 }];
        for xyz in indices {
            let [a, b, c] = xyz.map(|i| v[i as usize]);
            let mn = Vec3::new(
                a.x.min(b.x).min(c.x),
                a.y.min(b.y).min(c.y),
                a.z.min(b.z).min(c.z),
            );
            let mx = Vec3::new(
                a.x.max(b.x).max(c.x),
                a.y.max(b.y).max(c.y),
                a.z.max(b.z).max(c.z),
            );
            if let Some((lo, hi)) = near {
                if mx.x < lo.x
                    || mn.x > hi.x
                    || mx.z < lo.z
                    || mn.z > hi.z
                    || mx.y < lo.y
                    || mn.y > hi.y
                {
                    continue;
                }
                let extent = (mx.x - mn.x).max(mx.y - mn.y).max(mx.z - mn.z);
                let need = if extent < 0.49 * CELL {
                    2
                } else if extent < CELL {
                    4
                } else {
                    0
                };
                if let (Some(outer), true) = (&self.outer, need != 0) {
                    let mut all = true;
                    for (i, p) in xyz.iter().zip([a, b, c]) {
                        let h = &mut held[*i as usize];
                        if *h == 0 {
                            *h = outer.classify(p);
                        }
                        if *h & need == 0 {
                            all = false;
                            break;
                        }
                    }
                    if !all {
                        continue;
                    }
                }
            }
            let Some(normal) = b.sub(a).cross(c.sub(a)).try_normalize() else {
                continue;
            };
            let lo = index(Vec3::new(mn.x - BAND, mn.y - BAND, mn.z - BAND));
            let hi = index(Vec3::new(mx.x + BAND, mx.y + BAND, mx.z + BAND));
            account_cells(lo, hi, &mut self.postings)?;
            // At most 2 000 000 postings, each face posts at least once: ids fit in u32.
            let id = self.faces.len() as u32;
            if let Some(r) = &self.reach {
                let Some((lo, hi)) = r.clip(lo, hi) else {
                    continue;
                };
                let mut posted = false;
                let dense = &mut self.dense;
                r.each_indexed(lo, hi, |s| {
                    dense[s].push(id);
                    posted = true;
                });
                if posted {
                    self.faces.push(Tri { a, b, c, normal });
                }
                continue;
            }
            self.faces.push(Tri { a, b, c, normal });
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        self.bins.entry([x, y, z]).or_default().push(id);
                    }
                }
            }
        }
        Ok(())
    }
    pub(super) fn project(&self, p: Vec3, thickness: f32) -> Vec3 {
        let cell = index(p);
        let ids = match &self.reach {
            Some(r) => r.indexed(&cell).map(|s| self.dense[s].as_slice()),
            None => self.bins.get(&cell).map(Vec::as_slice),
        };
        let Some(ids) = ids else {
            return p;
        };
        let mut best = BAND * BAND;
        let mut result = p;
        for &i in ids {
            let face = self.faces[i as usize];
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

/// The grid cells the simulated particles can query: their own cells, dilated.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Reach {
    lo: [i32; 3],
    dim: [i32; 3],
    cells: Vec<bool>,
}
impl Reach {
    /// The cells holding the points, dilated by `margin` metres (whole cells, rounded up);
    /// `None` for no point, a non-finite point, or an implausibly large extent (the caller
    /// then indexes everything).
    pub(super) fn around(points: impl Iterator<Item = Vec3>, margin: f32) -> Option<Self> {
        let d = (margin / CELL).ceil().max(0.) as i32;
        let mut seen = Vec::new();
        for p in points {
            if !(p.x.is_finite() && p.y.is_finite() && p.z.is_finite()) {
                return None;
            }
            seen.push(index(p));
        }
        let first = *seen.first()?;
        let (mut lo, mut hi) = (first, first);
        for c in &seen {
            for k in 0..3 {
                lo[k] = lo[k].min(c[k]);
                hi[k] = hi[k].max(c[k]);
            }
        }
        let lo = lo.map(|v| i64::from(v) - i64::from(d));
        let span: [i64; 3] = std::array::from_fn(|k| i64::from(hi[k]) + i64::from(d) - lo[k] + 1);
        // At most 2^18 cells (64 per axis is 3.8 m at 6 cm): beyond that, index everything.
        if span.iter().product::<i64>() > 1 << 18
            || lo.iter().any(|&v| v < i64::from(i32::MIN) + 8)
            || hi
                .iter()
                .any(|&v| i64::from(v) + i64::from(d) > i64::from(i32::MAX) - 8)
        {
            return None;
        }
        let lo = lo.map(|v| v as i32);
        let dim = span.map(|n| n as i32);
        // The culled insert's float tests assume moderate coordinates (f32 spacing far below
        // a millimetre): far from the origin, index everything instead.
        if (0..3).any(|k| {
            (lo[k] as f32 * CELL).abs() > 1e4 || ((lo[k] + dim[k]) as f32 * CELL).abs() > 1e4
        }) {
            return None;
        }
        let mut r = Self {
            lo,
            dim,
            cells: vec![false; (dim[0] * dim[1] * dim[2]) as usize],
        };
        for c in seen {
            let i = r.slot(c)?;
            r.cells[i] = true;
        }
        r.dilate(d);
        Some(r)
    }
    /// Grow every set cell into the cube of radius `d` cells around it (within the box).
    fn dilate(&mut self, d: i32) {
        let dim = self.dim;
        for k in 0..3 {
            let mut next = self.cells.clone();
            for x in 0..dim[0] {
                for y in 0..dim[1] {
                    for z in 0..dim[2] {
                        let c = [x, y, z];
                        if self.cells[self.local(c)] {
                            for t in -d..=d {
                                let mut e = c;
                                e[k] += t;
                                if (0..dim[k]).contains(&e[k]) {
                                    let j = self.local(e);
                                    next[j] = true;
                                }
                            }
                        }
                    }
                }
            }
            self.cells = next;
        }
    }
    /// These cells dilated by `by` more cells, in a box grown to hold them.
    fn grown(&self, by: i32) -> Self {
        let mut g = Self {
            lo: self.lo.map(|v| v - by),
            dim: self.dim.map(|n| n + 2 * by),
            cells: vec![
                false;
                ((self.dim[0] + 2 * by) * (self.dim[1] + 2 * by) * (self.dim[2] + 2 * by))
                    as usize
            ],
        };
        for x in 0..self.dim[0] {
            for y in 0..self.dim[1] {
                for z in 0..self.dim[2] {
                    if self.cells[self.local([x, y, z])] {
                        let j = g.local([x + by, y + by, z + by]);
                        g.cells[j] = true;
                    }
                }
            }
        }
        g.dilate(by);
        g
    }
    fn local(&self, c: [i32; 3]) -> usize {
        ((c[0] * self.dim[1] + c[1]) * self.dim[2] + c[2]) as usize
    }
    fn slot(&self, c: [i32; 3]) -> Option<usize> {
        let l: [i64; 3] = std::array::from_fn(|k| i64::from(c[k]) - i64::from(self.lo[k]));
        (0..3)
            .all(|k| (0..i64::from(self.dim[k])).contains(&l[k]))
            .then(|| self.local(l.map(|v| v as i32)))
    }
    fn contains(&self, c: [i32; 3]) -> bool {
        self.slot(c).is_some_and(|i| self.cells[i])
    }
    /// The part of an inclusive cell range inside the bounding box, if any.
    fn clip(&self, lo: [i32; 3], hi: [i32; 3]) -> Option<([i32; 3], [i32; 3])> {
        let a: [i32; 3] = std::array::from_fn(|k| lo[k].max(self.lo[k]));
        let b: [i32; 3] = std::array::from_fn(|k| hi[k].min(self.lo[k] + self.dim[k] - 1));
        (0..3).all(|k| a[k] <= b[k]).then_some((a, b))
    }
    /// Visit, in x-major then y then z order, the dense slot of every indexed cell of an
    /// inclusive range already clipped to the box.
    fn each_indexed(&self, lo: [i32; 3], hi: [i32; 3], mut visit: impl FnMut(usize)) {
        let l: [usize; 3] = std::array::from_fn(|k| (lo[k] - self.lo[k]) as usize);
        let h: [usize; 3] = std::array::from_fn(|k| (hi[k] - self.lo[k]) as usize);
        let (dy, dz) = (self.dim[1] as usize, self.dim[2] as usize);
        for x in l[0]..=h[0] {
            for y in l[1]..=h[1] {
                let row = (x * dy + y) * dz;
                for slot in row + l[2]..=row + h[2] {
                    if self.cells[slot] {
                        visit(slot);
                    }
                }
            }
        }
    }
    /// The box in metres outside which a triangle's vertices cannot post to any of its cells:
    /// the cell box grown by the band plus 1 mm for the rounding of `index`.
    fn padded_bounds(&self) -> (Vec3, Vec3) {
        let pad = BAND + 1e-3;
        let lo: [f32; 3] = std::array::from_fn(|k| self.lo[k] as f32 * CELL - pad);
        let hi: [f32; 3] = std::array::from_fn(|k| (self.lo[k] + self.dim[k]) as f32 * CELL + pad);
        (
            Vec3::new(lo[0], lo[1], lo[2]),
            Vec3::new(hi[0], hi[1], hi[2]),
        )
    }
    /// The dense slot of an indexed cell.
    fn indexed(&self, c: &[i32; 3]) -> Option<usize> {
        self.slot(*c).filter(|&i| self.cells[i])
    }
}

/// The reach dilated by one and by two cells, sharing one box (the two-cell one).
struct Outer {
    one: Vec<bool>,
    two: Reach,
}
impl Outer {
    fn new(reach: &Reach) -> Self {
        let two = reach.grown(2);
        let one = reach.grown(1);
        // Re-box the one-cell dilation into the two-cell box: same lo - 1 offset per axis.
        let mut cells = vec![false; two.cells.len()];
        for x in 0..one.dim[0] {
            for y in 0..one.dim[1] {
                for z in 0..one.dim[2] {
                    if one.cells[one.local([x, y, z])] {
                        cells[two.local([x + 1, y + 1, z + 1])] = true;
                    }
                }
            }
        }
        Self { one: cells, two }
    }
    /// Bit 0 set always; bit 1 if the vertex may lie in the one-cell dilation, bit 2 the
    /// two-cell one. A vertex that is non-finite or beyond 10 km (where `index` rounding could
    /// approach the band's slack) is never excluded.
    fn classify(&self, p: Vec3) -> u8 {
        let moderate = |v: f32| v.abs() < 1e4;
        if !(moderate(p.x) && moderate(p.y) && moderate(p.z)) {
            return 7;
        }
        match self.two.slot(index(p)) {
            Some(i) => 1 | (u8::from(self.one[i]) << 1) | (u8::from(self.two.cells[i]) << 2),
            None => 1,
        }
    }
}

/// One posed triangle source: a flat index list or a list of triangles.
#[derive(Clone, Copy)]
pub(super) enum Source<'a> {
    Flat(&'a [Vec3], &'a [u32]),
    Tris(&'a [Vec3], &'a [[u32; 3]]),
}
fn insert_source(s: &mut Surface, src: Source<'_>) -> Result<(), &'static str> {
    match src {
        Source::Flat(v, i) => s.insert(v, i.chunks_exact(3).map(|t| [t[0], t[1], t[2]])),
        Source::Tris(v, t) => s.insert(v, t.iter().copied()),
    }
}

/// The per-frame collision field, indexed only where the simulated particles can query it.
/// A query inside the reach reads a bin identical to the full surface's; a query outside it
/// builds the full surface once (lazily) and reads that, so `project` equals the full
/// surface's `project` for every point, bit for bit.
pub(super) struct Collider<'a> {
    sources: Vec<Source<'a>>,
    near: Surface,
    full: OnceCell<Option<Surface>>,
}
impl<'a> Collider<'a> {
    /// Index the source triangles that can post to `reach`'s cells, validating each (as the
    /// full insert does); `None` indexes and validates everything.
    pub(super) fn build(
        sources: Vec<Source<'a>>,
        reach: Option<Reach>,
    ) -> Result<Self, &'static str> {
        let mut near = reach.map_or_else(Surface::new, Surface::within);
        for &src in &sources {
            insert_source(&mut near, src)?;
        }
        Ok(Self {
            sources,
            near,
            full: OnceCell::new(),
        })
    }
    pub(super) fn project(&self, p: Vec3, thickness: f32) -> Vec3 {
        if self.near.covers(p) {
            return self.near.project(p, thickness);
        }
        // The full surface refuses what the culled one dropped unvalidated (an exploded or
        // over-budget primitive out of reach); such a frame then answers from the culled
        // surface, where the full insert would have failed the whole step.
        let full = self.full.get_or_init(|| {
            let mut s = Surface::new();
            self.sources
                .iter()
                .try_for_each(|&src| insert_source(&mut s, src))
                .ok()
                .map(|()| s)
        });
        full.as_ref().unwrap_or(&self.near).project(p, thickness)
    }
    /// Whether some query left the reach and took the full surface.
    #[cfg(test)]
    pub(super) fn fell_back(&self) -> bool {
        self.full.get().is_some()
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
) -> Result<(), &'static str> {
    const GAP: f32 = 0.005;
    let point = |p: Vec3| {
        [
            (p.x / CELL).floor() as i32,
            (p.y / CELL).floor() as i32,
            (p.z / CELL).floor() as i32,
        ]
    };
    let mut bins: HashMap<[i32; 3], Vec<usize>> = HashMap::new();
    let mut postings = 0;
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
        account_cells(lo, hi, &mut postings)?;
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
    Ok(())
}

#[cfg(test)]
mod cull_tests {
    use super::*;

    fn bits(p: Vec3) -> [u32; 3] {
        [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
    }
    /// A deterministic scatter: a body sheet plus garment triangles near and far from the hair.
    fn scene() -> (Vec<Vec3>, Vec<u32>, Vec<Vec3>, Vec<[u32; 3]>) {
        let mut seed = 0x9e37_79b9_u32;
        let mut r = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as f32 / u32::MAX as f32
        };
        let (mut bv, mut bi) = (Vec::new(), Vec::new());
        for y in 0..40 {
            for z in 0..60 {
                bv.push(Vec3::new(
                    0.01 * r(),
                    0.03 * y as f32 - 0.6,
                    0.03 * z as f32,
                ));
            }
        }
        for y in 0..39u32 {
            for z in 0..59u32 {
                let a = y * 60 + z;
                bi.extend([a, a + 60, a + 1, a + 1, a + 60, a + 61]);
            }
        }
        let (mut gv, mut gt) = (Vec::new(), Vec::new());
        for k in 0..400u32 {
            let c = Vec3::new(0.8 * r() - 0.4, 1.2 * r() - 0.6, 1.8 * r());
            for _ in 0..3 {
                gv.push(c.add(Vec3::new(0.05 * r(), 0.05 * r(), 0.05 * r())));
            }
            gt.push([3 * k, 3 * k + 1, 3 * k + 2]);
        }
        (bv, bi, gv, gt)
    }
    fn hair_points() -> Vec<Vec3> {
        (0..50)
            .map(|i| Vec3::new(0.02, 0.004 * i as f32 - 0.1, 1.3 + 0.006 * i as f32))
            .collect()
    }

    #[test]
    fn culled_projection_is_bit_identical_to_the_full_surface_everywhere() {
        let (bv, bi, gv, gt) = scene();
        let src = vec![Source::Flat(&bv, &bi), Source::Tris(&gv, &gt)];
        let keep = Reach::around(hair_points().into_iter(), 0.12);
        let culled = Collider::build(src.clone(), keep.clone()).unwrap();
        let full = Collider::build(src, None).unwrap();
        assert!(
            culled.near.faces.len() * 3 < full.near.faces.len(),
            "cull keeps a minority"
        );
        let mut inside = 0;
        for x in -20..20 {
            for y in -12..12 {
                for z in 0..40 {
                    let p = Vec3::new(0.011 * x as f32, 0.05 * y as f32 + 0.003, 0.046 * z as f32);
                    inside += usize::from(keep.as_ref().unwrap().contains(index(p)));
                    assert_eq!(
                        bits(culled.project(p, 0.004)),
                        bits(full.project(p, 0.004)),
                        "{p:?}"
                    );
                }
            }
        }
        assert!(inside > 100);
        assert!(culled.fell_back(), "the sweep left the kept cells");
    }

    /// Every reach cell's bin must hold the full surface's triangles, bit for bit, in order.
    fn assert_reach_bins_match(culled: &Collider<'_>, full: &Collider<'_>) -> usize {
        let r = culled.near.reach.as_ref().unwrap();
        let key = |t: &Tri| [t.a, t.b, t.c, t.normal].map(bits);
        let mut checked = 0;
        for x in r.lo[0]..r.lo[0] + r.dim[0] {
            for y in r.lo[1]..r.lo[1] + r.dim[1] {
                for z in r.lo[2]..r.lo[2] + r.dim[2] {
                    let Some(s) = r.indexed(&[x, y, z]) else {
                        continue;
                    };
                    let got: Vec<_> = culled.near.dense[s]
                        .iter()
                        .map(|&i| key(&culled.near.faces[i as usize]))
                        .collect();
                    let want: Vec<_> = full.near.bins.get(&[x, y, z]).map_or(Vec::new(), |b| {
                        b.iter()
                            .map(|&i| key(&full.near.faces[i as usize]))
                            .collect()
                    });
                    assert_eq!(got, want, "cell {:?}", [x, y, z]);
                    checked += want.len();
                }
            }
        }
        checked
    }

    #[test]
    fn reach_bins_equal_the_full_bins_for_small_and_mid_triangles_at_the_boundary() {
        let (bv, bi, gv, gt) = scene();
        let keep = Reach::around(hair_points().into_iter(), 0.12);
        let src = vec![Source::Flat(&bv, &bi), Source::Tris(&gv, &gt)];
        let checked = assert_reach_bins_match(
            &Collider::build(src.clone(), keep.clone()).unwrap(),
            &Collider::build(src, None).unwrap(),
        );
        assert!(checked > 1000, "{checked}");
        // A dense scatter of triangles of every size class up to 1.5 cells, all around the box.
        let mut seed = 0x2545_f491_u32;
        let mut r = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as f32 / u32::MAX as f32
        };
        let (mut sv, mut st) = (Vec::new(), Vec::new());
        for k in 0..20_000u32 {
            let c = Vec3::new(0.7 * r() - 0.33, 0.9 * r() - 0.45, 0.9 * r() + 0.95);
            let size = [0.004, 0.02, 0.029, 0.031, 0.05, 0.059, 0.061, 0.09][k as usize % 8];
            for _ in 0..3 {
                sv.push(c.add(Vec3::new(size * r(), size * r(), size * r())));
            }
            st.push([3 * k, 3 * k + 1, 3 * k + 2]);
        }
        let src = vec![Source::Tris(&sv, &st)];
        let checked = assert_reach_bins_match(
            &Collider::build(src.clone(), keep).unwrap(),
            &Collider::build(src, None).unwrap(),
        );
        assert!(checked > 10_000, "{checked}");
    }

    #[test]
    fn garment_inside_hair_reach_still_collides() {
        let (bv, bi, mut gv, mut gt) = scene();
        // A garment triangle placed across the hair, 1 cm in front of the body sheet.
        let n = gv.len() as u32;
        gv.extend([
            Vec3::new(0.03, -0.2, 1.25),
            Vec3::new(0.03, 0.2, 1.25),
            Vec3::new(0.03, 0.0, 1.7),
        ]);
        gt.push([n, n + 1, n + 2]);
        let keep = Reach::around(hair_points().into_iter(), 0.12);
        let culled =
            Collider::build(vec![Source::Flat(&bv, &bi), Source::Tris(&gv, &gt)], keep).unwrap();
        let p = Vec3::new(0.031, 0.0, 1.4);
        let q = culled.project(p, 0.004);
        assert!(
            (q.x - 0.034).abs() < 1e-5,
            "pushed out of the placed garment: {q:?}"
        );
        assert!(!culled.fell_back());
    }

    #[test]
    fn query_outside_the_kept_cells_takes_the_full_path() {
        let (bv, bi, gv, gt) = scene();
        let src = vec![Source::Flat(&bv, &bi), Source::Tris(&gv, &gt)];
        let keep = Reach::around(hair_points().into_iter(), 0.12);
        let culled = Collider::build(src.clone(), keep.clone()).unwrap();
        let full = Collider::build(src, None).unwrap();
        let near = Vec3::new(0.005, 0.0, 1.4);
        assert_eq!(
            bits(culled.project(near, 0.004)),
            bits(full.project(near, 0.004))
        );
        assert!(!culled.fell_back());
        // A foot-height point next to the body: far outside the hair cells, still in contact.
        let foot = Vec3::new(-0.01, 0.0, 0.1);
        assert!(!keep.as_ref().unwrap().contains(index(foot)));
        let q = culled.project(foot, 0.004);
        assert!(culled.fell_back());
        assert_eq!(bits(q), bits(full.project(foot, 0.004)));
        assert_ne!(bits(q), bits(foot), "the full path really collided");
    }

    #[test]
    fn out_of_reach_budget_refusal_no_longer_fails_and_reach_is_unchanged() {
        let (bv, bi, gv, gt) = scene();
        let clean = vec![Source::Flat(&bv, &bi), Source::Tris(&gv, &gt)];
        let reference = Collider::build(clean, None).unwrap();
        // An exploded garment triangle at floor level, far below the hair: the full surface
        // refuses it; the culled build never validates it and succeeds.
        let (mut xv, mut xt) = (gv.clone(), gt.clone());
        let n = xv.len() as u32;
        xv.extend([
            Vec3::new(0., -3., 0.),
            Vec3::new(0., 3., 0.),
            Vec3::new(0.1, 0., 0.),
        ]);
        xt.push([n, n + 1, n + 2]);
        let src = vec![Source::Flat(&bv, &bi), Source::Tris(&xv, &xt)];
        let reach = Reach::around(hair_points().into_iter(), 0.12);
        assert_eq!(
            Collider::build(src.clone(), None).err(),
            Some("collision primitive exceeds spatial budget")
        );
        let culled = Collider::build(src, reach.clone()).unwrap();
        let mut probed = 0;
        for y in -12..12 {
            for z in 15..30 {
                for x in -6..6 {
                    let p = Vec3::new(
                        0.01 * x as f32 + 0.004,
                        0.017 * y as f32,
                        0.06 * z as f32 + 0.01,
                    );
                    if culled.near.covers(p) {
                        probed += 1;
                        assert_eq!(
                            bits(culled.project(p, 0.004)),
                            bits(reference.project(p, 0.004)),
                            "{p:?}"
                        );
                    }
                }
            }
        }
        assert!(probed > 200, "{probed}");
        assert!(!culled.fell_back());
        // The same primitive inside the reach is still refused.
        let (mut iv, mut it) = (gv.clone(), gt.clone());
        let n = iv.len() as u32;
        iv.extend([
            Vec3::new(0.02, -3., 1.4),
            Vec3::new(0.02, 3., 1.4),
            Vec3::new(0.12, 0., 1.4),
        ]);
        it.push([n, n + 1, n + 2]);
        let inside = vec![Source::Flat(&bv, &bi), Source::Tris(&iv, &it)];
        assert_eq!(
            Collider::build(inside, reach).err(),
            Some("collision primitive exceeds spatial budget")
        );
    }

    #[test]
    fn fast_floor_matches_floor_on_edges_and_a_stride_of_all_bit_patterns() {
        let edges = [
            0.,
            -0.,
            0.5,
            -0.5,
            -1.,
            1.,
            -1e-45,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            2147483520.,
            -2147483648.,
            2147483648.,
            -2147483904.,
            f32::MAX,
            f32::MIN,
        ];
        for v in edges {
            assert_eq!(floor_i32(v), v.floor() as i32, "{v}");
        }
        for bits in (0..=u32::MAX).step_by(9973) {
            let v = f32::from_bits(bits);
            assert_eq!(floor_i32(v), v.floor() as i32, "{bits:#x}");
        }
    }

    #[test]
    fn non_finite_reach_disables_the_cull() {
        let far = [Vec3::new(0., 0., 0.), Vec3::new(30., 30., 30.)];
        assert_eq!(Reach::around(far.into_iter(), 0.06), None);
        let remote = [Vec3::new(2e4, 0., 1.)];
        assert_eq!(Reach::around(remote.into_iter(), 0.06), None);
        assert!(Reach::around([Vec3::new(9e3, 0., 1.)].into_iter(), 0.06).is_some());
        assert_eq!(
            Reach::around([Vec3::new(f32::NAN, 0., 0.)].into_iter(), 0.1),
            None
        );
        assert_eq!(Reach::around(std::iter::empty(), 0.1), None);
    }
}
