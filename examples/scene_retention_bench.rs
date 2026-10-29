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
use gibson::scene::{Presentation, Scene, SceneEntity, SceneId};
use std::collections::VecDeque;
use std::hint::black_box;
use std::time::{Duration, Instant};

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

    bench_removal_churn();
    bench_constant_pop_churn();
}

const CHURN_SIZES: &[usize] = &[1_000, 2_000, 5_000, 10_000];

/// A scene of `n` labelled entities, plus their ids in insertion order.
fn build_scene(n: usize) -> (Scene, Vec<SceneId>) {
    let mut scene = Scene::new();
    let mut ids = Vec::with_capacity(n);
    for i in 0..n {
        ids.push(scene.add(SceneEntity::new(format!("e{i}"), text("e"))));
    }
    (scene, ids)
}

/// Phase 1C (Renaissance IV): the cost of RETIRING many entities, which the original
/// `to_node` bench did not measure. `Scene::remove` is `position()` scan + `Vec::remove`
/// shift + `by_label` purge — O(n) per call — so removing all N one-by-one is O(N²).
/// `Scene::retain` drops any number in a single O(n) pass. This measures both on one
/// host so the quadratic signature (or its absence at realistic N) is visible, and
/// shows the batch path as the remedy that needs no new index.
fn bench_removal_churn() {
    println!("\n# issue #77 — lifecycle operation cost (Phase 1C, one host, one build)");
    println!("# remove-all = N one-by-one removes in insertion order (front-biased shift);");
    println!("# retain-drop = the same N dropped in a single retain(|_| false) pass.\n");
    println!(
        "{:>7}  {:>18}  {:>14}  {:>18}  {:>14}",
        "N", "remove-all total", "per remove", "retain-drop total", "id_of (mean)"
    );
    let mut prev: Option<(usize, Duration)> = None;
    for &n in CHURN_SIZES {
        // (a) One-by-one remove of all N.
        let (mut s1, ids) = build_scene(n);
        let t0 = Instant::now();
        for id in &ids {
            black_box(s1.remove(*id));
        }
        let remove_total = t0.elapsed();
        let per = remove_total / n as u32;

        // (b) Batch retain-drop of all N (fresh scene, identical population).
        let (mut s2, _) = build_scene(n);
        let t1 = Instant::now();
        s2.retain(|_| false);
        let retain_total = t1.elapsed();

        // (c) Pure label-lookup throughput on a full scene (labels pre-built so the
        //     format! cost is outside the timed region).
        let (s3, _) = build_scene(n);
        let labels: Vec<String> = (0..n).map(|i| format!("e{i}")).collect();
        let t2 = Instant::now();
        for l in &labels {
            black_box(s3.id_of(l));
        }
        let id_of_mean = t2.elapsed() / n as u32;

        println!("{n:>7}  {remove_total:>18?}  {per:>14?}  {retain_total:>18?}  {id_of_mean:>14?}");
        if let Some((pn, pt)) = prev {
            if n == pn * 2 && pt.as_secs_f64() > 0.0 {
                let ratio = remove_total.as_secs_f64() / pt.as_secs_f64();
                println!(
                    "#   remove-all doubling {pn}→{n}: {ratio:.2}x  (~4x = quadratic, ~2x = linear)"
                );
            }
        }
        prev = Some((n, remove_total));
    }
    println!(
        "# NOTE: remove() is O(n)/call by design; bulk retirement should use retain()\n\
         # (one O(n) pass). An index is justified only if interactive single-remove is hot."
    );
}

/// Steady-state "spawn one / retire the oldest" churn at a roughly constant live
/// population — the interactive pattern (particles, transient overlays). Per-cycle cost
/// reflects one add + one front remove at that population.
fn bench_constant_pop_churn() {
    const POP: usize = 2_000;
    const CYCLES: usize = 5_000;
    let mut scene = Scene::new();
    let mut live: VecDeque<SceneId> = VecDeque::with_capacity(POP);
    for i in 0..POP {
        live.push_back(scene.add(SceneEntity::new(format!("p{i}"), text("p"))));
    }
    let mut next = POP;
    let t0 = Instant::now();
    for _ in 0..CYCLES {
        let id = scene.add(SceneEntity::new(format!("p{next}"), text("p")));
        live.push_back(id);
        next += 1;
        if let Some(old) = live.pop_front() {
            black_box(scene.remove(old));
        }
    }
    let per = t0.elapsed() / CYCLES as u32;
    println!(
        "\n# constant-population churn: pop~{POP}, {CYCLES} (add + oldest-remove) cycles \
         -> {per:?}/cycle, final len {}",
        scene.len()
    );
}
