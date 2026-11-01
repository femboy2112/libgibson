//! Deterministic at-scale proof that the shipped #77 entity-retirement lifecycle
//! bounds `Scene` cost by the LIVE+VISIBLE population, not by total-ever.
//!
//! #192 shipped the lifecycle (`remove` / `retain` / `clear`) and the O(n) resolve
//! hot path. Its correctness is already proven IN THE SMALL (N=2) by the in-module
//! unit tests `removed_entity_leaves_no_ghost_surface`,
//! `hide_versus_remove_is_a_real_retirement`, and
//! `stale_id_after_remove_is_safely_rejected`. The one AT-SCALE witness it left —
//! `scene.rs::to_node_scaling_is_not_quadratic_in_hidden_entities` — is a wall-clock
//! ratio and is `#[ignore]`d precisely because wall-clock is machine-flaky and
//! cannot gate CI. This battery fills that gap with DETERMINISTIC COUNTING
//! assertions at issue #77's own reproducer scale (5000 hidden entities), so the
//! bound is proven on every CI run on every host — no clock, no byte fixtures, no
//! libm coupling (contrast the audio PCM golden that forces a toolchain pin).
//!
//! What is proven:
//!  * paint work is proportional to VISIBLE and INVARIANT to the hidden/retired
//!    population (`to_node().children.len()` == visible count, at every scale);
//!  * `evaluate` realizes the whole LIVE set, so a HIDDEN entity's cost is retained
//!    until it is actually retired (the mutation control, lifted to scale);
//!  * retirement truly SHRINKS the live set (`len()` / `entities()` fall), so
//!    total-ever does not accumulate — the #77 defect made impossible;
//!  * a retired id is permanently dead at scale (`resolve` yields empty).
//!
//! Honesty boundary (ceded in writing, not silently):
//!  * `children.len()` is a deterministic PROXY for the node-clone count — exactly
//!    one clone per painted child (src/scene.rs:1299) — not a raw allocation meter.
//!  * `to_node` still builds and sorts an order vec of length `len()` on EVERY call
//!    (src/scene.rs:1289), so per-call cost is O(LIVE·log LIVE), NOT O(painted). The
//!    claim is "clone/paint work is proportional to VISIBLE, and the retained set
//!    truly shrinks on retire" — never "a retired entity costs literally zero".
//!  * A HIDDEN entity still costs one visibility lookup + one sort slot; only
//!    RETIREMENT reclaims it. That distinction is exactly what the mutation control
//!    below proves at scale.

use gibson::cell::Style;
use gibson::node::Node;
use gibson::scene::{Presentation, Scene, SceneEntity, SceneId, SceneTarget};

const VISIBLE: usize = 4;
const HIDDEN: usize = 5_000; // issue #77's reproducer scale

fn text(label: &str) -> Node {
    Node::text(label, Style::new()).width(8.0).height(1.0)
}

/// A scene with `visible` painted entities followed by `hidden` retained-but-hidden
/// ones. Returns the scene and the hidden ids (for later retirement).
fn scene_with(visible: usize, hidden: usize) -> (Scene, Vec<SceneId>) {
    let mut scene = Scene::new();
    for k in 0..visible {
        scene.add(SceneEntity::new(format!("v{k}"), text("v")));
    }
    let mut hidden_ids = Vec::with_capacity(hidden);
    for i in 0..hidden {
        let id = scene.add(SceneEntity::new(format!("h{i}"), text("h")));
        scene.entity_mut(id).unwrap().visible = false;
        hidden_ids.push(id);
    }
    (scene, hidden_ids)
}

/// The painted-layer count. Default `Presentation` ⇒ camera (0,0) ⇒ `to_node`
/// returns the stack directly, so its children ARE the painted (visible) layers
/// (the in-module unit tests read `.children.len()` the same way).
fn painted(scene: &Scene) -> usize {
    scene
        .to_node(&Presentation::new(), 80.0, 24.0)
        .children
        .len()
}

fn live_evaluated(scene: &Scene) -> usize {
    scene.evaluate(&Presentation::new()).len()
}

/// THE HEADLINE: paint is bounded by VISIBLE and invariant to the hidden
/// population at every scale, while `evaluate` tracks the whole LIVE set. This is
/// the deterministic replacement for the `#[ignore]`d wall-clock ratio witness.
#[test]
fn paint_is_invariant_to_hidden_population_while_evaluate_tracks_live() {
    for &hidden in &[0usize, 10, 400, 2_000, 5_000] {
        let (scene, _) = scene_with(VISIBLE, hidden);
        assert_eq!(
            scene.len(),
            VISIBLE + hidden,
            "live count at hidden={hidden}"
        );
        assert_eq!(
            painted(&scene),
            VISIBLE,
            "paint must stay at the {VISIBLE}-visible baseline at hidden={hidden}"
        );
        assert_eq!(
            live_evaluated(&scene),
            VISIBLE + hidden,
            "evaluate realizes the whole live set (hidden cost retained) at hidden={hidden}"
        );
    }
}

/// `remove` reclaims the retained set: the #77 defect (unbounded growth, hidden
/// entities still evaluated/cloned) made impossible, proven at scale.
#[test]
fn retiring_hidden_entities_shrinks_the_live_set() {
    let (mut scene, hidden_ids) = scene_with(VISIBLE, HIDDEN);
    assert_eq!(scene.len(), VISIBLE + HIDDEN);

    for id in &hidden_ids {
        assert!(
            scene.remove(*id).is_some(),
            "each hidden id retires exactly once"
        );
    }

    assert_eq!(
        scene.len(),
        VISIBLE,
        "remove reclaims the whole retained set"
    );
    assert_eq!(
        scene.entities().len(),
        VISIBLE,
        "backing storage shrank, not just hid"
    );
    assert_eq!(
        live_evaluated(&scene),
        VISIBLE,
        "evaluate cost falls with retirement"
    );
    assert_eq!(
        painted(&scene),
        VISIBLE,
        "paint unchanged — the visible set was never touched"
    );
}

/// `retain` is the one-pass bulk retirement tool (per the remove() O(n)/call
/// caveat): keeping only the visible entities reclaims every hidden one at once.
#[test]
fn bulk_retain_reclaims_the_hidden_set_in_one_pass() {
    let (mut scene, _) = scene_with(VISIBLE, HIDDEN);
    scene.retain(|e| e.visible);
    assert_eq!(scene.len(), VISIBLE);
    assert_eq!(live_evaluated(&scene), VISIBLE);
    assert_eq!(painted(&scene), VISIBLE);
}

/// MUTATION CONTROL, lifted to scale: hiding is NOT retiring. Two scenes with the
/// same VISIBLE paint — one with 5000 entities HIDDEN, one with the same 5000
/// RETIRED — paint identically yet differ in retained cost. This proves the
/// lifecycle actually retires rather than merely re-skinning `visible = false`.
#[test]
fn hide_is_not_retire_at_scale() {
    let (hidden_scene, _) = scene_with(VISIBLE, HIDDEN);

    let (mut retired_scene, _) = scene_with(VISIBLE, HIDDEN);
    retired_scene.retain(|e| e.visible);

    // Same observable paint...
    assert_eq!(painted(&hidden_scene), VISIBLE);
    assert_eq!(painted(&retired_scene), VISIBLE);
    assert_eq!(painted(&hidden_scene), painted(&retired_scene));

    // ...but the hidden scene still carries the cost; the retired one reclaimed it.
    assert_eq!(hidden_scene.len(), VISIBLE + HIDDEN);
    assert_eq!(live_evaluated(&hidden_scene), VISIBLE + HIDDEN);
    assert_eq!(retired_scene.len(), VISIBLE);
    assert_eq!(live_evaluated(&retired_scene), VISIBLE);
    assert!(
        hidden_scene.len() > retired_scene.len(),
        "hiding retains the cost; retirement reclaims it"
    );
}

/// `clear` reclaims everything; the scene paints and evaluates nothing afterward.
#[test]
fn clear_reclaims_the_entire_scene() {
    let (mut scene, _) = scene_with(VISIBLE, HIDDEN);
    scene.clear();
    assert!(scene.is_empty());
    assert_eq!(painted(&scene), 0);
    assert_eq!(live_evaluated(&scene), 0);
}

/// Every retired id is permanently dead at scale — `resolve` yields empty for all
/// 5000, and none of the surviving visible ids were disturbed.
#[test]
fn retired_ids_are_permanently_dead_at_scale() {
    let (mut scene, hidden_ids) = scene_with(VISIBLE, HIDDEN);
    scene.retain(|e| e.visible);
    for id in &hidden_ids {
        assert!(
            scene.resolve(SceneTarget::Id(*id)).is_empty(),
            "a retired id must resolve to nothing"
        );
        assert!(scene.entity(*id).is_none());
    }
    // The visible survivors are intact and still resolvable.
    assert_eq!(scene.entities().len(), VISIBLE);
    for e in scene.entities() {
        assert_eq!(scene.resolve(SceneTarget::Id(e.id)), vec![e.id]);
    }
}
