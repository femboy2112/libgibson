//! Typed pocket time — a small, opt-in categorical layer that keeps a MUSICAL phase displacement
//! (beats, which scale with tempo) and a PHYSICAL latency (seconds, which do not) from ever being
//! conflated in a single float again.
//!
//! This module adds no behaviour to any render. Nothing in the kernel, the synth, or the C137 path
//! calls it; it is the type system and the executable laws that a lawful pocket fix is built ON.
//! The bug it exists to make impossible is `KEYS_ATTACK_COMP` (kernel.rs:906): a 0.03-**beat**
//! constant introduced as a physical-latency correction. Because it lives in beats it scales with
//! tempo (22.5 ms at 80 BPM, 17.31 ms at 104, 12.86 ms at 140), which no fixed DSP latency does; and
//! because it is one float it cannot say whether it *means* a musical anticipation or a patch delay.
//!
//! Two pieces, each with teeth (see `tests/audio_homosapians_pocket_time.rs`):
//!
//! **[`Displacement`]** — an element of the additive group `D = Beats × Seconds`, with the tempo
//! evaluation homomorphism `E_β(b, s) = 60·b/β + s` into seconds. A musical push is a beats-only
//! displacement; a measured patch latency is a seconds-only displacement; they compose but never
//! merge. The tempo-naturality fact (`P5`) falls straight out: a pure-beats displacement's physical
//! duration scales with 1/β, a pure-seconds one is β-invariant.
//!
//! **[`PhaseGraph`]** — a finite graph whose vertices are performing [`Gesture`]s and whose edges
//! are the band's *declared, intended* relative-phase relations (`t_v − t_u = d_uv`). The elementary
//! cycle-consistency lemma makes the ethos operational: a consistent global assignment exists iff
//! the oriented displacement sum around every cycle is zero. A contradictory triangle yields a
//! concrete [`Conflict`] (the minimal cycle and its residual) instead of silent jitter repair —
//! the law can say NO. Separately, [`PhaseGraph::observed_residuals`] checks whether the actually
//! scheduled times HONOUR the declared relations, because "the observed times are mutually
//! consistent" is tautologically true and proves nothing about intent.

/// A performing gesture — a vertex in the phase graph. These are *gestures*, not just role labels:
/// the required Keys carrier and the riff-comp are the same instrument but distinct time-owners, and
/// their accidental 0.03-beat disagreement (the self-flam, P3) is exactly a relation between two of
/// these vertices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Gesture {
    KeysComp,
    KeysCarrier,
    BassFoundation,
    Kick,
    Snare,
    Hats,
    GuitarStrum,
}

/// An element of the typed displacement group `D = Beats × Seconds`. Beats are musical (they scale
/// with tempo); seconds are physical (they do not). Addition is componentwise; the identity is zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Displacement {
    /// Musical component, in beats.
    pub beats: f64,
    /// Physical component, in seconds.
    pub seconds: f64,
}

impl Displacement {
    /// The group identity (dead on, no displacement).
    pub const ZERO: Displacement = Displacement {
        beats: 0.0,
        seconds: 0.0,
    };

    /// A purely musical displacement (a push/lay-back expressed as a beat fraction).
    pub const fn beats(beats: f64) -> Displacement {
        Displacement {
            beats,
            seconds: 0.0,
        }
    }

    /// A purely physical displacement (a measured patch/chain latency in seconds).
    pub const fn seconds(seconds: f64) -> Displacement {
        Displacement {
            beats: 0.0,
            seconds,
        }
    }

    /// The evaluation homomorphism `E_β(b, s) = 60·b/β + s` into physical seconds at tempo `bpm > 0`.
    /// `E_β(d1 + d2) = E_β(d1) + E_β(d2)` (a group homomorphism into (ℝ, +)).
    pub fn eval_seconds(self, bpm: f64) -> f64 {
        debug_assert!(bpm > 0.0, "tempo must be positive");
        60.0 / bpm * self.beats + self.seconds
    }

    /// Evaluate into beats at tempo `bpm > 0` — the inverse view, for scheduling on the beat grid.
    pub fn eval_beats(self, bpm: f64) -> f64 {
        debug_assert!(bpm > 0.0, "tempo must be positive");
        self.beats + self.seconds * bpm / 60.0
    }
}

impl std::ops::Add for Displacement {
    type Output = Displacement;
    /// Group addition (componentwise).
    fn add(self, other: Displacement) -> Displacement {
        Displacement {
            beats: self.beats + other.beats,
            seconds: self.seconds + other.seconds,
        }
    }
}

impl std::ops::Neg for Displacement {
    type Output = Displacement;
    /// Group inverse (negate both components).
    fn neg(self) -> Displacement {
        Displacement {
            beats: -self.beats,
            seconds: -self.seconds,
        }
    }
}

/// A named musical relation between two gestures, carrying its intended signed displacement. These
/// are not all equalities: a `Lock` demands coincidence, a `Behind`/`Ahead` is a *licensed* lean, a
/// `Withhold`/`Independent` declares there is deliberately NO coincidence constraint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PocketRelation {
    /// `to` lands at the same time as `from` (`d = 0`).
    Lock,
    /// `to` sits later than `from` by the given displacement (a lay-back).
    Behind(Displacement),
    /// `to` sits earlier than `from` by the given displacement (an authored anticipation).
    Ahead(Displacement),
}

impl PocketRelation {
    /// The signed displacement `d_uv` (so `t_to − t_from = d_uv`) this relation prescribes.
    pub fn displacement(self) -> Displacement {
        match self {
            PocketRelation::Lock => Displacement::ZERO,
            PocketRelation::Behind(d) => d,
            PocketRelation::Ahead(d) => -d,
        }
    }
}

/// A declared, source-owned intended relation `t_to − t_from = relation` between two gestures, with a
/// short human reason. The `reason` is provenance, never proof.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseEdge {
    pub from: Gesture,
    pub to: Gesture,
    pub relation: PocketRelation,
    pub reason: &'static str,
}

impl PhaseEdge {
    pub fn lock(from: Gesture, to: Gesture, reason: &'static str) -> PhaseEdge {
        PhaseEdge {
            from,
            to,
            relation: PocketRelation::Lock,
            reason,
        }
    }
    pub fn behind(from: Gesture, to: Gesture, d: Displacement, reason: &'static str) -> PhaseEdge {
        PhaseEdge {
            from,
            to,
            relation: PocketRelation::Behind(d),
            reason,
        }
    }
    pub fn ahead(from: Gesture, to: Gesture, d: Displacement, reason: &'static str) -> PhaseEdge {
        PhaseEdge {
            from,
            to,
            relation: PocketRelation::Ahead(d),
            reason,
        }
    }

    fn delay_seconds(self, bpm: f64) -> f64 {
        self.relation.displacement().eval_seconds(bpm)
    }
}

/// The minimal conflicting cycle found by [`PhaseGraph::consistency`]: a contradictory set of
/// intended relations whose oriented displacement sum around the cycle is nonzero. This is the law's
/// NO — a specific witness, not an invitation to invent timing.
#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
    /// The gestures around the offending cycle, in order (first repeated implicitly at the end).
    pub cycle: Vec<Gesture>,
    /// The residual (seconds) by which the cycle fails to close at the evaluated tempo.
    pub residual_seconds: f64,
}

/// A consistent global assignment of relative times (seconds) to the reachable gestures, unique up to
/// one additive origin per connected component. Gestures with no declared edge are simply absent —
/// their freedom is preserved, never collapsed to a forced coincidence.
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub times: Vec<(Gesture, f64)>,
}

impl Assignment {
    pub fn get(&self, g: Gesture) -> Option<f64> {
        self.times.iter().find(|(v, _)| *v == g).map(|(_, t)| *t)
    }
}

/// A residual between the band's declared intent and what the Score actually scheduled, for one edge.
/// `residual = (observed_to − observed_from) − d_uv`. A `Lock` edge with a residual outside the bound
/// is a lock the performance does not keep (the `KEYS_ATTACK_COMP` case: a declared Lock, observed
/// 0.03 beat off).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObservedResidual {
    pub edge: PhaseEdge,
    pub residual_seconds: f64,
    pub within_bound: bool,
}

/// A finite graph of intended inter-gesture phase relations.
#[derive(Debug, Clone, Default)]
pub struct PhaseGraph {
    edges: Vec<PhaseEdge>,
}

impl PhaseGraph {
    pub fn new() -> PhaseGraph {
        PhaseGraph { edges: Vec::new() }
    }

    pub fn with_edges(edges: Vec<PhaseEdge>) -> PhaseGraph {
        PhaseGraph { edges }
    }

    pub fn push(&mut self, edge: PhaseEdge) -> &mut Self {
        self.edges.push(edge);
        self
    }

    pub fn edges(&self) -> &[PhaseEdge] {
        &self.edges
    }

    fn vertices(&self) -> Vec<Gesture> {
        let mut vs: Vec<Gesture> = Vec::new();
        for e in &self.edges {
            for g in [e.from, e.to] {
                if !vs.contains(&g) {
                    vs.push(g);
                }
            }
        }
        vs
    }

    /// Does a consistent global relative-time assignment exist for the declared intents, at tempo
    /// `bpm`? Returns the assignment (unique up to one origin per connected component) or the minimal
    /// [`Conflict`] cycle whose displacement sum is nonzero beyond `tol_seconds`.
    ///
    /// This decides whether the *intentions can coexist at all* — it is NOT a statement about the
    /// actual Score (see [`PhaseGraph::observed_residuals`]).
    pub fn consistency(&self, bpm: f64, tol_seconds: f64) -> Result<Assignment, Conflict> {
        let vs = self.vertices();
        // Undirected adjacency with the signed, tempo-evaluated delay: edge u->v carries +d, and the
        // reverse traversal v->u carries -d.
        let adj = |g: Gesture| -> Vec<(Gesture, f64)> {
            let mut out = Vec::new();
            for e in &self.edges {
                if e.from == g {
                    out.push((e.to, e.delay_seconds(bpm)));
                } else if e.to == g {
                    out.push((e.from, -e.delay_seconds(bpm)));
                }
            }
            out
        };

        let mut times: Vec<(Gesture, f64)> = Vec::new();
        let mut parent: Vec<(Gesture, Option<Gesture>)> = Vec::new();
        let t_of = |times: &[(Gesture, f64)], g: Gesture| {
            times.iter().find(|(v, _)| *v == g).map(|(_, t)| *t)
        };

        // BFS each connected component from an unassigned root (its own time origin at 0).
        for &root in &vs {
            if t_of(&times, root).is_some() {
                continue;
            }
            times.push((root, 0.0));
            parent.push((root, None));
            let mut queue = vec![root];
            while let Some(u) = queue.pop() {
                let tu = t_of(&times, u).expect("u assigned");
                for (v, d) in adj(u) {
                    if t_of(&times, v).is_none() {
                        times.push((v, tu + d));
                        parent.push((v, Some(u)));
                        queue.push(v);
                    }
                }
            }
        }

        // Every edge must now agree with the assigned times; the first that does not reveals a
        // contradictory fundamental cycle.
        for e in &self.edges {
            let (tu, tv) = (t_of(&times, e.from).unwrap(), t_of(&times, e.to).unwrap());
            let residual = (tv - tu) - e.delay_seconds(bpm);
            if residual.abs() > tol_seconds {
                let cycle = fundamental_cycle(&parent, e.from, e.to);
                return Err(Conflict {
                    cycle,
                    residual_seconds: residual,
                });
            }
        }
        Ok(Assignment { times })
    }

    /// Do the *actually scheduled* times honour the declared intents? For each edge, the residual is
    /// `(observed_to − observed_from) − d_uv` in seconds; `within_bound` is `|residual| ≤ bound`.
    /// `observed` gives each gesture's scheduled time (seconds); gestures absent from `observed` are
    /// skipped. This is the check with real teeth against a Score — a declared `Lock` with a residual
    /// outside the bound is a lie, however the prose labels it.
    pub fn observed_residuals(
        &self,
        observed: &[(Gesture, f64)],
        bpm: f64,
        bound_seconds: f64,
    ) -> Vec<ObservedResidual> {
        let obs = |g: Gesture| observed.iter().find(|(v, _)| *v == g).map(|(_, t)| *t);
        let mut out = Vec::new();
        for e in &self.edges {
            if let (Some(tu), Some(tv)) = (obs(e.from), obs(e.to)) {
                let residual = (tv - tu) - e.delay_seconds(bpm);
                out.push(ObservedResidual {
                    edge: *e,
                    residual_seconds: residual,
                    within_bound: residual.abs() <= bound_seconds,
                });
            }
        }
        out
    }
}

/// Reconstruct the cycle `from -> ... -> to -> from` through the BFS tree (used to describe a
/// [`Conflict`] concretely rather than as a bare scalar).
fn fundamental_cycle(
    parent: &[(Gesture, Option<Gesture>)],
    from: Gesture,
    to: Gesture,
) -> Vec<Gesture> {
    let up = |g: Gesture| -> Vec<Gesture> {
        let mut path = vec![g];
        let mut cur = g;
        while let Some((_, Some(p))) = parent.iter().find(|(v, _)| *v == cur) {
            path.push(*p);
            cur = *p;
        }
        path
    };
    let pu = up(from); // from -> ... -> root
    let pv = up(to); // to -> ... -> root
                     // Lowest common ancestor = first vertex of pu also in pv.
    let lca = pu.iter().copied().find(|g| pv.contains(g)).unwrap_or(from);
    let mut cycle = Vec::new();
    for g in &pu {
        cycle.push(*g);
        if *g == lca {
            break;
        }
    }
    // Append the to-side path down from the LCA (excluding the LCA, which is already present).
    let mut down: Vec<Gesture> = Vec::new();
    for g in &pv {
        if *g == lca {
            break;
        }
        down.push(*g);
    }
    down.reverse();
    cycle.extend(down);
    cycle
}

#[cfg(test)]
mod tests {
    use super::*;

    const S104: f64 = 60.0 / 104.0; // seconds per beat at 104 BPM

    fn close(x: Displacement, y: Displacement) -> bool {
        (x.beats - y.beats).abs() < 1e-12 && (x.seconds - y.seconds).abs() < 1e-12
    }

    #[test]
    fn displacement_is_an_additive_group() {
        let a = Displacement {
            beats: 0.03,
            seconds: 0.002,
        };
        let b = Displacement {
            beats: -0.01,
            seconds: 0.005,
        };
        // Componentwise sum, compared with a tolerance (never float-equality on a derived value).
        assert!(close(
            a + b,
            Displacement {
                beats: 0.02,
                seconds: 0.007
            }
        ));
        // Identity and inverse are exact (adding/subtracting the same bits).
        assert_eq!(a + Displacement::ZERO, a);
        assert_eq!(a + (-a), Displacement::ZERO);
        // Commutative.
        assert_eq!(a + b, b + a);
    }

    #[test]
    fn e_beta_is_a_homomorphism_into_seconds() {
        let a = Displacement::beats(0.03);
        let b = Displacement::seconds(0.004);
        for &bpm in &[80.0, 104.0, 140.0] {
            let lhs = (a + b).eval_seconds(bpm);
            let rhs = a.eval_seconds(bpm) + b.eval_seconds(bpm);
            assert!((lhs - rhs).abs() < 1e-12, "E_beta additive at {bpm}");
        }
    }

    #[test]
    fn tempo_naturality_separates_musical_from_physical() {
        // A pure-beats push scales in seconds with tempo: this is the KEYS_ATTACK_COMP footprint.
        let musical = Displacement::beats(0.03);
        assert!((musical.eval_seconds(80.0) - 0.0225).abs() < 1e-9);
        assert!((musical.eval_seconds(104.0) - 0.0173076923).abs() < 1e-9);
        assert!((musical.eval_seconds(140.0) - 0.0128571429).abs() < 1e-9);
        // A pure-seconds latency is tempo-invariant — what a real patch/chorus delay actually is.
        let physical = Displacement::seconds(0.005);
        for &bpm in &[80.0, 104.0, 140.0] {
            assert!((physical.eval_seconds(bpm) - 0.005).abs() < 1e-12);
        }
    }

    #[test]
    fn lawful_lock_triangle_is_consistent() {
        // Keys locks Bass, Bass locks Kick, Keys locks Kick — cycle sum 0.
        let g = PhaseGraph::with_edges(vec![
            PhaseEdge::lock(
                Gesture::KeysComp,
                Gesture::BassFoundation,
                "keys sits on the bass",
            ),
            PhaseEdge::lock(
                Gesture::BassFoundation,
                Gesture::Kick,
                "bass and kick interlock",
            ),
            PhaseEdge::lock(Gesture::KeysComp, Gesture::Kick, "keys sits on the kick"),
        ]);
        let a = g
            .consistency(104.0, 1e-9)
            .expect("lawful triangle admits an assignment");
        // All three locked together => equal relative times.
        assert_eq!(a.get(Gesture::KeysComp), a.get(Gesture::Kick));
        assert_eq!(a.get(Gesture::BassFoundation), a.get(Gesture::Kick));
    }

    #[test]
    fn contradictory_triangle_is_refused_with_a_witness() {
        // Keys locks Bass, Bass locks Kick, but Keys is ALSO declared 0.02 beat behind Kick with no
        // other difference. The triangle cannot close: the law must say NO, concretely.
        let g = PhaseGraph::with_edges(vec![
            PhaseEdge::lock(Gesture::KeysComp, Gesture::BassFoundation, "lock"),
            PhaseEdge::lock(Gesture::BassFoundation, Gesture::Kick, "lock"),
            PhaseEdge::behind(
                Gesture::KeysComp,
                Gesture::Kick,
                Displacement::beats(0.02),
                "contradiction",
            ),
        ]);
        let conflict = g
            .consistency(104.0, 1e-9)
            .expect_err("contradiction must be refused");
        assert!((conflict.residual_seconds.abs() - 0.02 * S104).abs() < 1e-9);
        assert!(conflict.cycle.contains(&Gesture::KeysComp));
        assert!(conflict.cycle.contains(&Gesture::Kick));
    }

    #[test]
    fn underconstrained_graph_preserves_freedom() {
        // Only Keys->Bass is declared; Snare is a separate isolated edge to Hats. The two components
        // are independent: consistency holds and nothing forces cross-component coincidence.
        let g = PhaseGraph::with_edges(vec![
            PhaseEdge::lock(Gesture::KeysComp, Gesture::BassFoundation, "lock"),
            PhaseEdge::behind(
                Gesture::Hats,
                Gesture::Snare,
                Displacement::beats(0.03),
                "snare lays back",
            ),
        ]);
        let a = g
            .consistency(104.0, 1e-9)
            .expect("underconstrained is still consistent");
        // Keys and Snare share no declared relation: their relative time is NOT pinned (each
        // component origins at 0, so the cross-component difference is undetermined, not forced 0).
        assert!(a.get(Gesture::KeysComp).is_some());
        assert!(a.get(Gesture::Snare).is_some());
        // The snare genuinely lays back behind its hats reference.
        let hats = a.get(Gesture::Hats).unwrap();
        let snare = a.get(Gesture::Snare).unwrap();
        assert!((snare - hats - 0.03 * S104).abs() < 1e-9);
    }

    #[test]
    fn observed_residual_catches_a_lock_the_score_does_not_keep() {
        // The band DECLARES the comp locks the kick, but the Score schedules the comp 0.03 beat early
        // (the KEYS_ATTACK_COMP footprint). Intent-vs-observed must flag the lock as violated.
        let g = PhaseGraph::with_edges(vec![PhaseEdge::lock(
            Gesture::KeysComp,
            Gesture::Kick,
            "comp is meant to lock the kit",
        )]);
        let kick_t = 1.0; // seconds (arbitrary origin)
        let comp_t = kick_t - 0.03 * S104; // comp scheduled 0.03 beat earlier than the kick
        let res = g.observed_residuals(
            &[(Gesture::KeysComp, comp_t), (Gesture::Kick, kick_t)],
            104.0,
            0.005, // a 5 ms musical bound
        );
        assert_eq!(res.len(), 1);
        assert!(
            !res[0].within_bound,
            "a 17 ms early comp is not a kept lock"
        );
        // kick − comp = +0.03 beat (the comp is earlier), intent 0 → residual +0.03 beat in seconds.
        assert!((res[0].residual_seconds - 0.03 * S104).abs() < 1e-9);
    }
}
