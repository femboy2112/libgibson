//! The DYNAMIC complement to `tests/scene_retention.rs` (#205): over a sustained
//! insert/retire churn session, the PEAK retained entity count stays bounded by the
//! working set — and only because the scene is actually retired, not because of any
//! canned number.
//!
//! #205 proves the STATIC claim: at fixed snapshots (0/10/400/2000/5000 hidden),
//! paint is proportional to VISIBLE and retirement shrinks the live set. This file
//! proves the OVER-TIME claim that a long-lived consumer actually cares about: if you
//! churn 5000 transient entities through a scene one at a time, retiring the oldest as
//! you go, the scene never retains more than the working set — while the identical add
//! timeline WITHOUT retirement grows unbounded to the full 5000. That is the #77 defect
//! (unbounded `Scene` growth) made impossible, witnessed as a high-water mark the way
//! the wire transport's `RetentionEnvelope` (src/wire.rs) witnesses bounded retained
//! bytes.
//!
//! This deliberately does NOT reuse `wire::RetentionEnvelope`: its contract and every
//! accessor are typed in BYTES, and reading an entity COUNT back through a
//! `*_bytes()` accessor would assert a count is a byte figure — conflating two
//! quantities (laws L7 truthful telemetry / L10 claim scoping). Entities are a count,
//! so the accumulator here is count-typed.
//!
//! Honesty boundary (ceded in writing): this bounds the retained ENTITY COUNT, never
//! literal heap bytes — `Scene` exposes no per-entity byte accessor and `Node` payloads
//! vary, so a count is the honest quantity (exactly as #205 cedes that `children.len()`
//! is a clone-count proxy, not an allocation meter). Pure integer counting: no clock
//! (L10-safe — a retention count is not a latency value), no byte fixtures, no libm.

use gibson::cell::Style;
use gibson::node::Node;
use gibson::scene::{Scene, SceneEntity, SceneId};

const VISIBLE: usize = 4; // a stable painted baseline that is never retired
const WORKING_SET: usize = 8; // how many transients may be live at once
const FRAMES: usize = 5_000; // issue #77's reproducer scale, as a per-tic churn

/// The declared retention budget: base paint + working set, with one slot of headroom.
const CEILING: usize = VISIBLE + WORKING_SET + 1;

fn text(label: &str) -> Node {
    Node::text(label, Style::new()).width(8.0).height(1.0)
}

/// A count-typed high-water gate, mirroring `wire::RetentionEnvelope`'s discipline but
/// for a retained ENTITY COUNT rather than bytes. It records the running peak of the
/// live population and whether a declared ceiling was ever exceeded. Its worth is that
/// it can FAIL: the never-retire run below drives `holds()` false.
struct PeakEntities {
    ceiling: usize,
    high_water: usize,
    samples: u64,
    breaches: u64,
}

impl PeakEntities {
    fn new(ceiling: usize) -> Self {
        Self {
            ceiling,
            high_water: 0,
            samples: 0,
            breaches: 0,
        }
    }

    /// Observe the current live entity count (never bytes).
    fn observe(&mut self, live: usize) {
        self.high_water = self.high_water.max(live);
        self.samples += 1;
        if live > self.ceiling {
            self.breaches += 1;
        }
    }

    fn high_water(&self) -> usize {
        self.high_water
    }
    fn breaches(&self) -> u64 {
        self.breaches
    }
    fn holds(&self) -> bool {
        self.high_water <= self.ceiling
    }
    fn headroom(&self) -> usize {
        self.ceiling.saturating_sub(self.high_water)
    }
}

/// Run the IDENTICAL add timeline — `VISIBLE` stable entities, then `FRAMES` transients
/// added one per tic — sampling `scene.len()` into a `PeakEntities` after each tic's
/// housekeeping. When `retire` is true, the oldest transient is removed once the live
/// transient set would exceed `WORKING_SET`. Returns the witness and the total number of
/// entities EVER added (the churn volume), so total-ever can be contrasted with the peak.
fn run_churn(retire: bool) -> (PeakEntities, usize) {
    let mut scene = Scene::new();
    let mut total_added = 0usize;

    for k in 0..VISIBLE {
        scene.add(SceneEntity::new(format!("v{k}"), text("v")));
        total_added += 1;
    }

    let mut live_transients: Vec<SceneId> = Vec::with_capacity(WORKING_SET + 1);
    let mut witness = PeakEntities::new(CEILING);

    for i in 0..FRAMES {
        // Fresh, never-recycled label every tic (SceneIds are not reused).
        let id = scene.add(SceneEntity::new(format!("t{i}"), text("t")));
        total_added += 1;
        live_transients.push(id);

        if retire && live_transients.len() > WORKING_SET {
            // Retire the oldest transient — the bounded-working-set discipline.
            let oldest = live_transients.remove(0);
            assert!(
                scene.remove(oldest).is_some(),
                "each transient retires once"
            );
        }

        // Sample the settled retained population for this tic.
        witness.observe(scene.len());
    }

    (witness, total_added)
}

/// BOUNDED: retiring the oldest transient holds the peak retained count under the
/// declared budget forever, no matter how long the session runs. The churn volume
/// (5004 entities ever added) dwarfs the peak retention (`VISIBLE + WORKING_SET`), which
/// is the entire point of the #77 lifecycle.
#[test]
fn retiring_churn_keeps_peak_entities_under_budget() {
    let (witness, total_added) = run_churn(true);

    assert_eq!(
        total_added,
        VISIBLE + FRAMES,
        "the full churn volume was issued"
    );
    assert_eq!(
        witness.high_water(),
        VISIBLE + WORKING_SET,
        "peak retained is the stable baseline plus the working set, nothing more"
    );
    assert!(
        witness.holds(),
        "the declared retention budget is never breached"
    );
    assert_eq!(witness.breaches(), 0);
    assert!(
        witness.headroom() > 0,
        "the peak sits strictly under the ceiling"
    );
    assert_eq!(witness.samples, FRAMES as u64, "every tic was sampled");
    assert!(
        total_added > witness.high_water() * 100,
        "total-ever churned ({total_added}) dwarfs peak retained ({}) — bounded over time",
        witness.high_water()
    );
}

/// THE TEETH: the SAME add timeline WITHOUT retirement grows the retained set to the
/// full churn volume and breaches the SAME budget the bounded run held. Because both
/// runs issue an identical workload, a hard-coded peak number could not tell them
/// apart — only genuinely sampling the live scene across the churn separates the
/// bounded peak from the growing one. This is the #77 defect, reproduced as a failing
/// gate (the calibration that makes the bounded run's pass mean something).
#[test]
fn without_retirement_the_same_workload_breaches_the_budget() {
    let (witness, total_added) = run_churn(false);

    assert_eq!(
        total_added,
        VISIBLE + FRAMES,
        "identical churn volume to the bounded run"
    );
    assert_eq!(
        witness.high_water(),
        VISIBLE + FRAMES,
        "with nothing retired, every entity ever added is still retained"
    );
    assert!(
        !witness.holds(),
        "the unbounded accumulator breaches the budget"
    );
    assert!(
        witness.breaches() > 0,
        "the ceiling is exceeded on the tics past the working set"
    );
    assert_eq!(
        witness.headroom(),
        0,
        "no headroom — the peak blew past the ceiling"
    );
}

/// The bounded and unbounded runs issue the EXACT same number of adds, yet their peak
/// retention differs by orders of magnitude. This is what defeats a canned certificate:
/// the separation lives entirely in whether the scene was retired, which only a real
/// per-tic sample of the live population can detect.
#[test]
fn identical_volume_wildly_different_peak() {
    let (bounded, bounded_added) = run_churn(true);
    let (unbounded, unbounded_added) = run_churn(false);

    assert_eq!(
        bounded_added, unbounded_added,
        "same workload — the only difference is retirement"
    );
    assert!(
        unbounded.high_water() > bounded.high_water() * 100,
        "unbounded peak ({}) is orders of magnitude over bounded peak ({})",
        unbounded.high_water(),
        bounded.high_water()
    );
}
