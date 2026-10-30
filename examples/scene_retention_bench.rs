//! Issue #77 reproducer + retention bench for `gibson::scene`.
//!
//! Before #77 a `Scene` could only grow: there was no `remove`/`retain`/`clear`,
//! and `evaluate`/`to_node` cloned and re-scanned every entity — hidden ones
//! included — making `to_node` O(n²) in retained entities. This example measures
//! `to_node` cost against a growing pool of hidden ("retired-by-hiding") entities,
//! then shows that `Scene::remove` actually bounds the cost.
//!
//! Run (release, for real numbers):
//!   cargo run --release --example scene_retention_bench
//!
//! This is a measurement instrument, not a pass/fail gate: it prints wall-clock
//! numbers for the host it runs on and promises no particular frame rate.

use gibson::cell::Style;
use gibson::node::Node;
use gibson::scene::{Presentation, Scene, SceneEntity};
use std::hint::black_box;
use std::time::Instant;

const ITERS: u32 = 50;
const KEPT_VISIBLE: usize = 4;

fn text(label: &str) -> Node {
    Node::text(label, Style::new()).width(8.0).height(1.0)
}

/// Mean `to_node` time over `ITERS` calls on the given scene.
fn mean_to_node(scene: &Scene, p: &Presentation) -> std::time::Duration {
    let t0 = Instant::now();
    for _ in 0..ITERS {
        black_box(scene.to_node(p, 80.0, 24.0));
    }
    t0.elapsed() / ITERS
}

fn main() {
    let p = Presentation::new();

    println!("# issue #77 — to_node cost vs retained hidden entities");
    println!("# (Style::new, {KEPT_VISIBLE} visible kept, rest hidden; mean of {ITERS} calls)\n");
    println!(
        "{:>7}  {:>14}  {:>14}  {:>16}",
        "hidden", "len()", "to_node (hidden)", "to_node (retired)"
    );

    for &hidden in &[10usize, 400, 2_000, 5_000] {
        let mut scene = Scene::new();
        for k in 0..KEPT_VISIBLE {
            scene.add(SceneEntity::new(format!("keep{k}"), text("keep")));
        }
        // Add `hidden` entities and hide them — the pre-#77 "retire by hiding".
        let mut hidden_ids = Vec::with_capacity(hidden);
        for i in 0..hidden {
            let id = scene.add(SceneEntity::new(format!("h{i}"), text("hidden")));
            scene.entity_mut(id).unwrap().visible = false;
            hidden_ids.push(id);
        }

        let with_hidden = mean_to_node(&scene, &p);
        let len_before = scene.len();

        // Now actually retire them with the new lifecycle and re-measure.
        for id in &hidden_ids {
            scene.remove(*id);
        }
        let retired = mean_to_node(&scene, &p);

        println!(
            "{hidden:>7}  {len_before:>14}  {:>14?}  {:>16?}",
            with_hidden, retired
        );
    }

    println!(
        "\n# `to_node (retired)` is the cost after Scene::remove — it should stay flat\n\
         # at the {KEPT_VISIBLE}-visible baseline regardless of how many were retired."
    );
}
