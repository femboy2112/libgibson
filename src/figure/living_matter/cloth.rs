//! Periodic panel grid. Sample the sewn cut in material coordinates and advect the
//! fine surface with the coarse sheet; only the waist is driven by the skeleton.
use super::{
    particles::{Link, Particles},
    DynamicsConfig, Vec3,
};

#[derive(Clone, Debug)]
pub(super) struct Cloth {
    particles: Particles,
    fine_cols: usize,
    fine_rows: usize,
    cols: usize,
    rows: Vec<usize>,
    rest: Vec<Vec3>,
    fine_rest: Vec<Vec3>,
}

impl Cloth {
    pub(super) fn new(
        fine: &[Vec3],
        tris: &[[u32; 3]],
        cfg: DynamicsConfig,
    ) -> Result<Self, &'static str> {
        let first = tris.first().ok_or("empty skirt cut")?;
        let fine_cols = first[1] as usize;
        if fine_cols < 8
            || fine_cols > 512
            || fine.len() % fine_cols != 0
            || first != &[0, fine_cols as u32, (fine_cols + 1) as u32]
        {
            return Err("unsupported garment panel topology");
        }
        let fine_rows = fine.len() / fine_cols;
        if !(4..=128).contains(&fine_rows) || tris.len() != (fine_rows - 1) * fine_cols * 2 {
            return Err("incomplete panel rows");
        }
        for (q, pair) in tris.chunks_exact(2).enumerate() {
            let a = q;
            let b = (q / fine_cols) * fine_cols + (q + 1) % fine_cols;
            if pair[0] != [a as u32, (a + fine_cols) as u32, (b + fine_cols) as u32]
                || pair[1] != [a as u32, (b + fine_cols) as u32, b as u32]
            {
                return Err("unrecognised garment seams");
            }
        }
        let cols = (8..=48.min(fine_cols))
            .rev()
            .find(|n| fine_cols % n == 0)
            .unwrap_or(fine_cols);
        let rows: Vec<_> = (0..fine_rows)
            .filter(|r| *r == 0 || *r == fine_rows - 1 || (*r >= 3 && r % 2 == 1))
            .collect();
        let rest: Vec<_> = rows
            .iter()
            .flat_map(|r| (0..cols).map(move |c| fine[r * fine_cols + c * fine_cols / cols]))
            .collect();
        let mut mass = vec![0.; rest.len()];
        let mut links = Vec::new();
        let props = cfg.fabric.properties();
        let id = |r: usize, c: usize| r * cols + (c % cols);
        let mut edge = |a: usize, b: usize, compliance: f32| {
            links.push(Link {
                a,
                b,
                length: rest[a].sub(rest[b]).length(),
                compliance,
            });
        };
        for r in 0..rows.len() {
            for c in 0..cols {
                let a = id(r, c);
                let b = id(r, c + 1);
                edge(a, b, props.stretch);
                edge(a, id(r, c + 2), props.bend);
                if r + 1 < rows.len() {
                    let d = id(r + 1, c);
                    let e = id(r + 1, c + 1);
                    let area = (rest[b].sub(rest[a]).cross(rest[d].sub(rest[a])).length()
                        + rest[b].sub(rest[e]).cross(rest[d].sub(rest[e])).length())
                        * 0.5;
                    for i in [a, b, d, e] {
                        mass[i] += area * props.density * 0.25;
                    }
                    edge(a, d, props.stretch);
                    edge(a, e, props.stretch * 3.);
                    edge(b, d, props.stretch * 3.);
                }
                if r + 2 < rows.len() {
                    edge(a, id(r + 2, c), props.bend);
                }
            }
        }
        let inv: Vec<_> = mass
            .into_iter()
            .enumerate()
            .map(|(i, m)| if i < cols { 0. } else { 1. / m })
            .collect();
        let particles = Particles::new(rest.clone(), inv, links)?;
        Ok(Self {
            particles,
            fine_cols,
            fine_rows,
            cols,
            rows,
            rest,
            fine_rest: fine.to_vec(),
        })
    }

    pub(super) fn step(
        &mut self,
        posed: &mut [Vec3],
        cfg: DynamicsConfig,
        dt: f32,
        project: impl Fn(Vec3, f32) -> Vec3,
    ) -> Result<(), &'static str> {
        let mut pins = Vec::with_capacity(self.rows.len() * self.cols);
        for &r in &self.rows {
            for c in 0..self.cols {
                pins.push(posed[r * self.fine_cols + c * self.fine_cols / self.cols]);
            }
        }
        let fabric = cfg.fabric.properties();
        self.particles.advance(
            dt,
            cfg.substeps,
            cfg.iterations,
            cfg.gravity,
            fabric.drag,
            fabric.friction,
            &pins,
            project,
        )?;
        // Transport the fine cut detail in the instantaneous material frame. This preserves
        // gathers during a turn instead of leaving a world-aligned residual on a rotated dress.
        let basis = |vertices: &[Vec3], row: usize, col: usize| {
            let i = row * self.cols + col;
            let u = vertices[row * self.cols + (col + 1) % self.cols]
                .sub(vertices[i])
                .try_normalize()
                .unwrap_or(Vec3::new(1., 0., 0.));
            let other = if row + 1 < self.rows.len() {
                vertices[i + self.cols].sub(vertices[i])
            } else {
                vertices[i].sub(vertices[i - self.cols])
            };
            let n = u
                .cross(other)
                .try_normalize()
                .unwrap_or(Vec3::new(0., 1., 0.));
            [u, n.cross(u), n]
        };
        let dynamic = self.particles.positions();
        let frames: Vec<_> = (0..self.rest.len())
            .map(|i| {
                (
                    basis(&self.rest, i / self.cols, i % self.cols),
                    basis(dynamic, i / self.cols, i % self.cols),
                )
            })
            .collect();
        let mut r = 0;
        for fr in 1..self.fine_rows {
            while r + 2 < self.rows.len() && fr > self.rows[r + 1] {
                r += 1;
            }
            let v = (fr - self.rows[r]) as f32 / (self.rows[r + 1] - self.rows[r]) as f32;
            for c in 0..self.fine_cols {
                let ci = c * self.cols / self.fine_cols;
                let u = (c * self.cols % self.fine_cols) as f32 / self.fine_cols as f32;
                let ids = [
                    r * self.cols + ci,
                    r * self.cols + (ci + 1) % self.cols,
                    (r + 1) * self.cols + ci,
                    (r + 1) * self.cols + (ci + 1) % self.cols,
                ];
                let weights = [(1. - v) * (1. - u), (1. - v) * u, v * (1. - u), v * u];
                let i = fr * self.fine_cols + c;
                let mut xyz = Vec3::ZERO;
                for (j, w) in ids.into_iter().zip(weights) {
                    let d = self.fine_rest[i].sub(self.rest[j]);
                    let (old, new) = frames[j];
                    let detail = new[0]
                        .scale(d.dot(old[0]))
                        .add(new[1].scale(d.dot(old[1])))
                        .add(new[2].scale(d.dot(old[2])));
                    xyz = xyz.add(dynamic[j].add(detail).scale(w));
                }
                posed[i] = xyz;
            }
        }
        Ok(())
    }
}
