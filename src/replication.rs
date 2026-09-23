//! A tiny deterministic replication graph — the "rabbit" effect.
//!
//! This is deliberately **not** a particle burst. It models the thing the
//! narrative actually describes: one node that duplicates along branches,
//! generation by generation, with bounded growth. It can be frozen and then
//! neutralized (collapsed and dissolved), which is what "cookie" does.
//!
//! Layout is normalized so the graph renders into any braille canvas:
//! generation `g` sits at `y = g / max_generation`, and horizontal spread
//! halves each generation. Growth is bounded by `max_generation` and
//! `max_nodes`, so a runaway cannot allocate without limit.

use crate::canvas::BrailleCanvas;

#[derive(Debug, Clone, Copy, PartialEq)]
struct ReplicationNode {
    generation: u32,
    /// Normalized horizontal position in `[-1, 1]`.
    x: f32,
    parent: Option<usize>,
}

/// A bounded, deterministic branching replication graph.
#[derive(Debug, Clone, PartialEq)]
pub struct Replication {
    nodes: Vec<ReplicationNode>,
    max_generation: u32,
    max_nodes: usize,
    /// 0 when intact; grows to 1 as the graph collapses after neutralization.
    collapse: f32,
    active: bool,
    frozen: bool,
    tick_acc: f32,
    tick_period: f32,
}

impl Replication {
    /// Creates a stopped replication graph with the given growth bounds.
    pub fn new(max_generation: u32, max_nodes: usize) -> Self {
        Self {
            nodes: Vec::new(),
            max_generation: max_generation.max(1),
            max_nodes: max_nodes.max(1),
            collapse: 0.0,
            active: false,
            frozen: false,
            tick_acc: 0.0,
            tick_period: 0.35,
        }
    }

    /// Begins replication from a single node.
    pub fn start(&mut self) {
        self.nodes = vec![ReplicationNode {
            generation: 0,
            x: 0.0,
            parent: None,
        }];
        self.collapse = 0.0;
        self.active = true;
        self.frozen = false;
        self.tick_acc = 0.0;
    }

    /// Freezes growth; the graph stays visible until neutralized or resumed.
    pub fn freeze(&mut self) {
        self.frozen = true;
    }

    pub fn resume(&mut self) {
        self.frozen = false;
    }

    /// Neutralize ("cookie"): freeze and begin collapse. The graph dissolves over
    /// the next ~0.5s of `update` calls.
    pub fn neutralize(&mut self) {
        if self.active {
            self.frozen = true;
            if self.collapse == 0.0 {
                self.collapse = 1e-4;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    pub fn is_collapsing(&self) -> bool {
        self.collapse > 0.0
    }

    pub fn collapse(&self) -> f32 {
        self.collapse.clamp(0.0, 1.0)
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn generation(&self) -> u32 {
        self.nodes.iter().map(|n| n.generation).max().unwrap_or(0)
    }

    /// Advances growth and collapse. `dt` is in seconds.
    pub fn update(&mut self, dt: f32) {
        if !self.active || dt <= 0.0 {
            return;
        }
        if self.collapse > 0.0 {
            // Dissolve: nodes vanish from the leaves inward.
            self.collapse = (self.collapse + dt * 2.0).min(1.0);
            if self.collapse >= 1.0 {
                self.active = false;
                self.nodes.clear();
            }
            return;
        }
        if self.frozen {
            return;
        }
        self.tick_acc += dt;
        while self.tick_acc >= self.tick_period {
            self.tick_acc -= self.tick_period;
            if !self.spawn_generation() {
                self.frozen = true;
                break;
            }
        }
    }

    /// Spawns two children for every current leaf. Returns `false` when a bound
    /// stops further growth.
    fn spawn_generation(&mut self) -> bool {
        let g = self.generation();
        if g >= self.max_generation || self.nodes.len() >= self.max_nodes {
            return false;
        }
        let spread = 1.0 / (1u32 << (g + 1).min(30)) as f32;
        let parents: Vec<usize> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.generation == g)
            .map(|(i, _)| i)
            .collect();
        for p in parents {
            let px = self.nodes[p].x;
            if self.nodes.len() + 2 > self.max_nodes {
                return false;
            }
            self.nodes.push(ReplicationNode {
                generation: g + 1,
                x: (px - spread).clamp(-1.0, 1.0),
                parent: Some(p),
            });
            self.nodes.push(ReplicationNode {
                generation: g + 1,
                x: (px + spread).clamp(-1.0, 1.0),
                parent: Some(p),
            });
        }
        true
    }

    /// Draws the graph as braille edges + dots, clipped to the canvas.
    pub fn render_braille(&self, canvas: &mut BrailleCanvas) {
        if !self.active || self.nodes.is_empty() {
            return;
        }
        let pw = canvas.pixel_width() as f32;
        let ph = canvas.pixel_height() as f32;
        if pw < 2.0 || ph < 2.0 {
            return;
        }
        let map = |n: &ReplicationNode| {
            let fx = (n.x + 1.0) * 0.5;
            let fy = if self.max_generation == 0 {
                0.5
            } else {
                n.generation as f32 / self.max_generation as f32
            };
            (
                (fx * (pw - 1.0)).round() as i32,
                (fy * (ph - 1.0)).round() as i32,
            )
        };
        // Edges first so dots sit on top.
        for n in &self.nodes {
            if let Some(p) = n.parent {
                let (x0, y0) = map(&self.nodes[p]);
                let (x1, y1) = map(n);
                canvas.line(x0, y0, x1, y1);
            }
        }
        for n in &self.nodes {
            let (x, y) = map(n);
            canvas.set(x, y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn growth_is_bounded_and_deterministic() {
        let mut a = Replication::new(5, 1000);
        let mut b = Replication::new(5, 1000);
        a.start();
        b.start();
        for _ in 0..100 {
            a.update(0.1);
            b.update(0.1);
        }
        assert_eq!(a.node_count(), b.node_count(), "must be deterministic");
        // Generations 0..=5 have 2^(g+1)-1 = 63 nodes.
        assert_eq!(a.node_count(), 63);
        assert_eq!(a.generation(), 5);
        assert!(a.is_active() && a.is_frozen(), "growth stops at the bound");
    }

    #[test]
    fn max_nodes_bound_is_respected() {
        let mut r = Replication::new(20, 10);
        r.start();
        for _ in 0..100 {
            r.update(0.1);
        }
        assert!(r.node_count() <= 10);
        assert!(
            r.node_count() >= 7,
            "should still branch: {}",
            r.node_count()
        );
    }

    #[test]
    fn neutralize_collapses_and_clears() {
        let mut r = Replication::new(4, 64);
        r.start();
        for _ in 0..40 {
            r.update(0.1);
        }
        assert!(r.node_count() > 1);
        r.neutralize();
        assert!(r.is_collapsing());
        // Not immediately gone; collapse takes time.
        r.update(0.05);
        assert!(r.is_active());
        for _ in 0..100 {
            r.update(0.05);
        }
        assert!(!r.is_active(), "fully neutralized");
        assert_eq!(r.node_count(), 0);
    }

    #[test]
    fn freeze_stops_growth_until_resumed() {
        let mut r = Replication::new(5, 1000);
        r.start();
        r.update(1.0);
        let frozen_at = r.node_count();
        r.freeze();
        for _ in 0..20 {
            r.update(0.5);
        }
        assert_eq!(r.node_count(), frozen_at, "frozen graph must not grow");
        r.resume();
        r.update(1.0);
        assert!(r.node_count() > frozen_at, "resume restarts growth");
    }

    #[test]
    fn render_draws_within_the_canvas() {
        let mut r = Replication::new(4, 64);
        r.start();
        for _ in 0..40 {
            r.update(0.1);
        }
        let mut c = BrailleCanvas::new(40, 12);
        r.render_braille(&mut c);
        assert!(!c.is_empty());
    }

    #[test]
    fn inactive_render_is_a_no_op() {
        let r = Replication::new(4, 64);
        let mut c = BrailleCanvas::new(10, 5);
        r.render_braille(&mut c);
        assert!(c.is_empty());
    }
}
