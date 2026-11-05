//! RXIII Checkpoint A / Gate A — the executable algebra-law suite for `gibson::scene`.
//!
//! This is the companion to the existing `scene_algebra.rs` (which proves identity-through-functor,
//! sequence associativity at EQUAL durations, determinism, bounded damage and the story graph). It
//! adds the *discriminating* cases that suite does not reach (Gate A3) and, above all, the headline
//! result Gate A2 demands: the **monoidal interchange law is false** under the timed duration
//! interpretation — proven not by comparing `duration()` fields alone but by an **observable**
//! divergence of the rendered presentation at a distinguishing timestamp.
//!
//! Each test carries its ledger verdict against the semantics in `src/scene.rs` on this head:
//!   * VALID                — holds for all inputs the operation accepts.
//!   * VALID UNDER CONDITIONS — holds only under a stated synchronization/domain condition.
//!   * COUNTEREXAMPLE       — a concrete input where a claimed law fails, with the mechanism named.
//!
//! The source of truth for the duration arithmetic is `Effect::duration` (scene.rs): `Sequence`
//! folds with `saturating_add` (⇒ **sum**), `Parallel` takes `max`. Those two disagree, and that
//! disagreement is the whole story of Gate A.

use gibson::scene::{Effect, Presentation, Scene, SceneEntity, SceneId, SceneTarget};
use gibson::{Node, Style};
use std::time::Duration;

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

fn dot(label: &str) -> Node {
    Node::text(label, Style::default()).width(1.0).height(1.0)
}

/// A scene of four independent entities a,b,c,d (one per interchange operand), each on its own
/// target so parallel/sequence composition never collides on a channel.
fn quad() -> (Scene, [SceneId; 4]) {
    let mut s = Scene::new();
    let a = s.add(SceneEntity::new("a", dot("a")));
    let b = s.add(SceneEntity::new("b", dot("b")));
    let c = s.add(SceneEntity::new("c", dot("c")));
    let d = s.add(SceneEntity::new("d", dot("d")));
    (s, [a, b, c, d])
}

fn slide(id: SceneId, d: Duration) -> Effect {
    // A 1-D translate 0→10 cells over `d`, so a sample at local fraction t reads `round(10t)` cells.
    Effect::translate(SceneTarget::Id(id), (0.0, 0.0), (10.0, 0.0), d)
}

// ---------------------------------------------------------------------------------------------
// A2 — the monoidal interchange obstruction.
// ---------------------------------------------------------------------------------------------

/// **COUNTEREXAMPLE (strict monoidal interchange).**
///
/// The interchange law `(f ⊗ g) ; (h ⊗ k) = (f ; h) ⊗ (g ; k)` is the equation that would make
/// `(⊗, ;)` a strict monoidal/duoidal structure. With `⊗ = Parallel` (duration = max) and
/// `; = Sequence` (duration = sum), take the operand durations Gate A2 names:
///   f = 10s, g = 1s, h = 1s, k = 10s   (each a slide 0→10 on its own target a,b,c,d)
///
/// Left  = sequence(parallel(f,g), parallel(h,k))  — duration = max(10,1) + max(1,10) = **20s**.
/// Right = parallel(sequence(f,h), sequence(g,k))  — duration = max(10+1, 1+10)       = **11s**.
///
/// The durations differ (20 ≠ 11), and — the part Gate A2 insists on — the *observable
/// presentation* differs too. Sample at t = 5s: on the Right, `sequence(g,k)` has finished g (1s)
/// and is 4s into k, so d has slid to `round(10·0.4) = 4`. On the Left the second tile
/// `parallel(h,k)` has not begun, so d is still at baseline 0. A real, visible divergence.
#[test]
fn strict_monoidal_interchange_fails_under_the_timed_interpretation() {
    let (scene, [a, b, c, d]) = quad();
    let (f, g, h, k) = (
        slide(a, secs(10)),
        slide(b, secs(1)),
        slide(c, secs(1)),
        slide(d, secs(10)),
    );

    let left = Effect::sequence([
        Effect::parallel([f.clone(), g.clone()]),
        Effect::parallel([h.clone(), k.clone()]),
    ]);
    let right = Effect::parallel([Effect::sequence([f, h]), Effect::sequence([g, k])]);

    // 1. The duration fields diverge exactly as the max/sum arithmetic predicts.
    assert_eq!(left.duration(), secs(20), "sequence of parallels: 10 + 10");
    assert_eq!(
        right.duration(),
        secs(11),
        "parallel of sequences: max(11, 11)"
    );
    assert_ne!(left.duration(), right.duration());

    // 2. The OBSERVABLE presentation diverges at a distinguishing timestamp (not just the field).
    let mut pl = Presentation::new();
    let mut pr = Presentation::new();
    left.eval(secs(5), &scene, &mut pl);
    right.eval(secs(5), &scene, &mut pr);

    assert_eq!(
        pl.offset_of(&scene, d),
        (0, 0),
        "left: tile (h⊗k) has not started, d at baseline"
    );
    assert_eq!(
        pr.offset_of(&scene, d),
        (4, 0),
        "right: seq(g,k) is 4s into k, d slid to round(10·0.4)"
    );
    assert_ne!(
        pl.offset_of(&scene, d),
        pr.offset_of(&scene, d),
        "interchange is observably false, not merely a duration-field artifact"
    );

    // 3. And the fully rendered Node trees disagree — the user would see two different films.
    assert_ne!(
        format!("{:?}", scene.to_node(&pl, 16.0, 4.0)),
        format!("{:?}", scene.to_node(&pr, 16.0, 4.0)),
        "the render functor lowers the two compositions to visibly different frames"
    );
}

/// **VALID UNDER CONDITIONS (interchange under duration-aligned tiles).**
///
/// The positive control Gate A2 asks for: when the parallel tiles are *synchronized* — here, all
/// four operands share one duration — `max` and `sum` no longer disagree about the tile widths, and
/// interchange is restored both in duration AND observably at every sampled instant. This is the
/// honest shape of the structure: a **timed scheduling algebra** in which interchange is a
/// conditional law gated on branch-duration alignment, not an unconditional monoidal axiom.
#[test]
fn interchange_holds_when_the_parallel_branches_are_duration_aligned() {
    let (scene, [a, b, c, d]) = quad();
    let span = secs(10);
    let (f, g, h, k) = (
        slide(a, span),
        slide(b, span),
        slide(c, span),
        slide(d, span),
    );

    let left = Effect::sequence([
        Effect::parallel([f.clone(), g.clone()]),
        Effect::parallel([h.clone(), k.clone()]),
    ]);
    let right = Effect::parallel([Effect::sequence([f, h]), Effect::sequence([g, k])]);

    assert_eq!(
        left.duration(),
        right.duration(),
        "aligned tiles: 10+10 == max(20,20)"
    );

    // Observationally identical across the whole 0..=20s span, phase boundary included.
    for t in [0u64, 3, 5, 9, 10, 11, 15, 20] {
        let (mut pl, mut pr) = (Presentation::new(), Presentation::new());
        left.eval(secs(t), &scene, &mut pl);
        right.eval(secs(t), &scene, &mut pr);
        for id in [a, b, c, d] {
            assert_eq!(
                pl.offset_of(&scene, id),
                pr.offset_of(&scene, id),
                "at {t}s entity differs — interchange should hold under alignment"
            );
        }
        assert_eq!(
            format!("{:?}", scene.to_node(&pl, 16.0, 4.0)),
            format!("{:?}", scene.to_node(&pr, 16.0, 4.0)),
            "rendered frames agree under the synchronization condition at {t}s"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// A3 — discriminating laws the equal-duration suite does not reach.
// ---------------------------------------------------------------------------------------------

/// **VALID (sequential associativity over UNEQUAL durations).**
///
/// `scene_algebra.rs` proves `(e1;e2);e3 == e1;(e2;e3)` only when every leg is 100ms. The eval loop
/// subtracts each child's *exact* duration (scene.rs: `elapsed -= ed.as_nanos()`), so associativity
/// should not depend on the legs being equal. Prove it with 2s / 5s / 1s legs, sampled across the
/// seam of every grouping.
#[test]
fn sequence_is_associative_even_over_unequal_durations() {
    let (mut scene, _) = (Scene::new(), ());
    let a = scene.add(SceneEntity::new("a", dot("a")));
    let t = SceneTarget::Id(a);
    let e1 = Effect::translate(t, (0.0, 0.0), (2.0, 0.0), secs(2));
    let e2 = Effect::translate(t, (2.0, 0.0), (2.0, 5.0), secs(5));
    let e3 = Effect::translate(t, (2.0, 5.0), (9.0, 1.0), secs(1));

    let left = Effect::sequence([Effect::sequence([e1.clone(), e2.clone()]), e3.clone()]);
    let right = Effect::sequence([e1, Effect::sequence([e2, e3])]);
    assert_eq!(left.duration(), secs(8));
    assert_eq!(right.duration(), secs(8));

    for t_s in [0u64, 1, 2, 4, 7, 8, 9] {
        let (mut pl, mut pr) = (Presentation::new(), Presentation::new());
        left.eval(secs(t_s), &scene, &mut pl);
        right.eval(secs(t_s), &scene, &mut pr);
        assert_eq!(
            pl.offset_of(&scene, a),
            pr.offset_of(&scene, a),
            "assoc at {t_s}s"
        );
    }
}

/// **VALID (Displace commutes) / COUNTEREXAMPLE (Position does not).**
///
/// Displacement contributions `add` as integer vectors (scene.rs `add_displacement`), so parallel
/// `Displace`s commute regardless of order. Absolute `Position` writes (`Translate`) are overrides
/// resolved by write order, so they do NOT commute. One test pins both halves of the documented
/// "independent channels commute; same-channel writes resolve by order" contract.
#[test]
fn parallel_displacements_commute_but_position_writes_do_not() {
    let (mut scene, _) = (Scene::new(), ());
    let a = scene.add(SceneEntity::new("a", dot("a")));
    let t = SceneTarget::Id(a);

    // Displace: additive, commutative. Order of the parallel operands is irrelevant.
    let dx = Effect::displace(t, (0.0, 0.0), (3.0, 0.0), secs(1));
    let dy = Effect::displace(t, (0.0, 0.0), (0.0, 2.0), secs(1));
    let (mut p1, mut p2) = (Presentation::new(), Presentation::new());
    Effect::parallel([dx.clone(), dy.clone()]).eval(secs(1), &scene, &mut p1);
    Effect::parallel([dy, dx]).eval(secs(1), &scene, &mut p2);
    assert_eq!(p1.offset_of(&scene, a), (3, 2));
    assert_eq!(
        p1.offset_of(&scene, a),
        p2.offset_of(&scene, a),
        "displacements commute"
    );

    // Position: override, order-dependent. Swapping the operands changes the result.
    let tx = Effect::translate(t, (0.0, 0.0), (3.0, 0.0), secs(1));
    let ty = Effect::translate(t, (0.0, 0.0), (0.0, 2.0), secs(1));
    let (mut q1, mut q2) = (Presentation::new(), Presentation::new());
    Effect::parallel([tx.clone(), ty.clone()]).eval(secs(1), &scene, &mut q1);
    Effect::parallel([ty, tx]).eval(secs(1), &scene, &mut q2);
    assert_eq!(q1.offset_of(&scene, a), (0, 2), "last write (ty) wins");
    assert_eq!(q2.offset_of(&scene, a), (3, 0), "last write (tx) wins");
    assert_ne!(
        q1.offset_of(&scene, a),
        q2.offset_of(&scene, a),
        "position writes do not commute"
    );
}

/// **VALID (zero-duration ramp is an instantaneous set).**
///
/// `local_t` returns 1.0 when the duration is zero (scene.rs), so a zero-duration ramp reads its
/// endpoint at every t, including t=0. The failure mode a naive `t/d` would hit (NaN from 0/0) does
/// not occur. A boundary case the equal-duration suite never exercises.
#[test]
fn zero_duration_ramp_jumps_to_its_endpoint() {
    let (mut scene, _) = (Scene::new(), ());
    let a = scene.add(SceneEntity::new("a", dot("a")));
    let e = Effect::translate(SceneTarget::Id(a), (0.0, 0.0), (7.0, 3.0), Duration::ZERO);
    assert_eq!(e.duration(), Duration::ZERO);
    for t in [0u64, 1, 1000] {
        let mut p = Presentation::new();
        e.eval(Duration::from_millis(t), &scene, &mut p);
        assert_eq!(
            p.offset_of(&scene, a),
            (7, 3),
            "zero-duration ramp holds its endpoint at {t}ms"
        );
    }
}

/// **VALID (retired target preserves its failure identity).**
///
/// An effect aimed at a removed entity resolves to the empty set (scene.rs `resolve`), so it writes
/// no channel — it is a silent no-op, not a panic and not a write to a dead id. The failure class
/// ("target absent") is preserved rather than collapsed into a fake zero.
#[test]
fn an_effect_targeting_a_retired_entity_writes_nothing() {
    let (mut scene, _) = (Scene::new(), ());
    let a = scene.add(SceneEntity::new("a", dot("a")));
    let ghost = scene.add(SceneEntity::new("ghost", dot("g")));
    scene.remove(ghost);

    let e = Effect::translate(SceneTarget::Id(ghost), (0.0, 0.0), (9.0, 9.0), secs(1));
    let mut p = Presentation::new();
    e.eval(secs(1), &scene, &mut p);

    assert!(
        scene.resolve(SceneTarget::Id(ghost)).is_empty(),
        "the retired id resolves to nothing"
    );
    assert_eq!(
        p.raw_offset(ghost),
        None,
        "no channel was written for the dead target"
    );
    // The live entity is untouched; the scene still renders exactly one entity.
    assert_eq!(scene.to_node(&p, 8.0, 4.0).children.len(), 1);
    let _ = a;
}

/// **VALID (a non-finite visibility omits the entity, no panic — Finding A, #77).**
///
/// A NaN visibility fails the positive predicate `v > 0.0`, so `to_node` omits the entity exactly as
/// `evaluate` marks it invisible — the two public methods agree, and a hostile non-finite parameter
/// is handled by its documented failure identity rather than crashing or painting garbage.
#[test]
fn a_non_finite_visibility_omits_the_entity_without_panicking() {
    let (mut scene, _) = (Scene::new(), ());
    let a = scene.add(SceneEntity::new("a", dot("a")));
    let e = Effect::reveal(SceneTarget::Id(a), 1.0, f32::NAN, secs(1));
    let mut p = Presentation::new();
    e.eval(secs(1), &scene, &mut p);

    assert!(
        p.visibility_of(&scene, a).is_nan(),
        "the hostile parameter is carried, not normalized"
    );
    let node = scene.to_node(&p, 8.0, 4.0);
    assert_eq!(
        node.children.len(),
        0,
        "a NaN-visible entity is omitted, matching evaluate()"
    );
}

/// **VALID (the camera channel composes through sequence).**
///
/// Two camera pans in sequence land on the second pan's endpoint, and a non-zero camera lowers to a
/// `Viewport` wrapper (scene.rs `to_node`). The camera is a single global channel, so this also
/// confirms sequence drives it to the later write's final value rather than summing.
#[test]
fn camera_pan_composes_through_sequence() {
    let (mut scene, _) = (Scene::new(), ());
    scene.add(SceneEntity::new("a", dot("a")));
    let pans = Effect::sequence([
        Effect::camera_pan((0.0, 0.0), (3.0, 0.0), secs(1)),
        Effect::camera_pan((3.0, 0.0), (3.0, 4.0), secs(1)),
    ]);
    assert_eq!(pans.duration(), secs(2));
    let mut p = Presentation::new();
    pans.eval(secs(2), &scene, &mut p);
    assert_eq!(
        p.camera(),
        (3, 4),
        "sequence lands on the second pan's endpoint"
    );
    assert!(
        matches!(
            scene.to_node(&p, 8.0, 4.0).kind,
            gibson::NodeKind::Viewport { .. }
        ),
        "a non-zero camera lowers to a Viewport"
    );
}
