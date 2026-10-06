//! Integration tests for the `cinematic_showcase` flagship's reusable core: the
//! `ShowTimeline` edit-clock primitive. Pure `f(edit)` math — no TTY, no render,
//! hermetic and fast — same discipline as `tests/intro_reaction.rs`. The film's
//! demo-local cue sheet and scene content are art direction and are exercised by
//! running the example, not pinned here; what is pinned is the content-agnostic
//! timeline law every shot rides on.

#[path = "../examples/cinematic_showcase/shot.rs"]
mod shot;
#[path = "../examples/cinematic_showcase/show_timeline.rs"]
mod show_timeline;

use shot::{dim_toward_black, title_card, GrammarId, SceneId, Shot, TitleCard};
use show_timeline::ShowTimeline;

/// A tiny float comparison for weights/progress (f32 ramp arithmetic).
fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn empty_timeline_has_zero_duration_and_resolves_to_nothing() {
    let tl: ShowTimeline<&str> = ShowTimeline::new();
    assert_eq!(tl.duration(), 0.0);
    assert!(tl.resolve(0.0).is_empty());
    assert!(tl.resolve(5.0).is_empty());
    assert!(tl.top(0.0).is_none());
}

#[test]
fn contiguous_cuts_lay_end_to_end_and_report_local_time() {
    let tl = ShowTimeline::new()
        .cut(3.0, "a")
        .cut(2.0, "b")
        .cut(4.0, "c");
    assert_eq!(tl.duration(), 9.0);

    // Inside the first cue: exactly one active, hard-cut weight 1.
    let a = tl.resolve(1.5);
    assert_eq!(a.len(), 1);
    assert_eq!(*a[0].payload, "a");
    assert!(close(a[0].local, 1.5));
    assert!(close(a[0].progress, 0.5));
    assert!(close(a[0].weight, 1.0));

    // Inside the middle cue: local is measured from THAT cue's start.
    let b = tl.resolve(4.0); // 1s into "b" (which starts at 3.0)
    assert_eq!(b.len(), 1);
    assert_eq!(*b[0].payload, "b");
    assert!(close(b[0].local, 1.0));
    assert!(close(b[0].progress, 0.5));

    let c = tl.resolve(6.5); // "c" starts at 5.0, so 1.5s in
    assert_eq!(*c[0].payload, "c");
    assert!(close(c[0].local, 1.5));
}

#[test]
fn cut_boundaries_are_half_open() {
    let tl = ShowTimeline::new().cut(2.0, "a").cut(2.0, "b");
    // At t == 2.0 the first cue has ended (half-open) and the second begins.
    let at = tl.resolve(2.0);
    assert_eq!(at.len(), 1);
    assert_eq!(*at[0].payload, "b");
    assert!(close(at[0].local, 0.0));
    // The very start of the whole timeline shows the first cue.
    assert_eq!(*tl.resolve(0.0)[0].payload, "a");
    // At/after the final end nothing is live.
    assert!(tl.resolve(4.0).is_empty());
    assert!(tl.resolve(4.5).is_empty());
}

#[test]
fn crossfade_overlaps_two_cues_with_complementary_weights() {
    // "a": [0,4) fade 1 ; "b" crossfades, starting at 4 - 1 = 3, so [3,7) fade 1.
    let tl = ShowTimeline::new().cut(4.0, "a").crossfade(4.0, 1.0, "b");
    assert_eq!(tl.duration(), 7.0);

    // Midway through the overlap (t = 3.5): both live, a falling, b rising, and
    // because the ramps are symmetric their weights sum to ~1.
    let mid = tl.resolve(3.5);
    assert_eq!(mid.len(), 2);
    let wa = mid.iter().find(|x| *x.payload == "a").unwrap().weight;
    let wb = mid.iter().find(|x| *x.payload == "b").unwrap().weight;
    assert!(close(wa, 0.5), "a weight {wa}");
    assert!(close(wb, 0.5), "b weight {wb}");
    assert!(close(wa + wb, 1.0));

    // Before the overlap only "a" is live at full weight.
    let before = tl.resolve(1.0);
    assert_eq!(before.len(), 1);
    assert_eq!(*before[0].payload, "a");
    assert!(close(before[0].weight, 1.0));

    // After the overlap only "b", full weight.
    let after = tl.resolve(5.5);
    assert_eq!(after.len(), 1);
    assert_eq!(*after[0].payload, "b");
    assert!(close(after[0].weight, 1.0));
}

#[test]
fn top_follows_the_dominant_cue_through_a_crossfade() {
    let tl = ShowTimeline::new().cut(4.0, "a").crossfade(4.0, 2.0, "b"); // overlap [2,4)

    // Early overlap: "a" still dominant.
    assert_eq!(*tl.top(2.4).unwrap().payload, "a");
    // Exact midpoint t=3.0: weights equal (0.5 each) -> tie goes to the later
    // (incoming) cue "b".
    assert_eq!(*tl.top(3.0).unwrap().payload, "b");
    // Late overlap: "b" dominant.
    assert_eq!(*tl.top(3.6).unwrap().payload, "b");
    // Gap after the end: nothing.
    assert!(tl.top(99.0).is_none());
}

#[test]
fn progress_saturates_at_the_end_of_a_cue() {
    let tl = ShowTimeline::new().cut(2.0, "a");
    // Just before the exclusive end, progress is near 1 and local near dur.
    let near = tl.resolve(1.999);
    assert_eq!(near.len(), 1);
    assert!(near[0].progress > 0.99 && near[0].progress <= 1.0);
    assert!(near[0].local > 1.99 && near[0].local < 2.0);
}

#[test]
fn degenerate_and_hostile_inputs_are_refused_without_panicking() {
    // Non-positive / non-finite durations never land a cue.
    let tl = ShowTimeline::new()
        .cut(0.0, "zero")
        .cut(-3.0, "neg")
        .cut(f32::NAN, "nan")
        .cut(f32::INFINITY, "inf")
        .cut(2.0, "real");
    assert_eq!(tl.cues().len(), 1, "only the one real cue survives");
    assert_eq!(tl.duration(), 2.0);
    assert_eq!(*tl.resolve(1.0)[0].payload, "real");

    // Hostile query times are empty, not panics.
    assert!(tl.resolve(f32::NAN).is_empty());
    assert!(tl.resolve(f32::INFINITY).is_empty());
    assert!(tl.resolve(f32::NEG_INFINITY).is_empty());
    assert!(tl.top(f32::NAN).is_none());
}

#[test]
fn fade_ramps_are_clamped_to_the_window() {
    // A fade longer than the whole window is clamped to `dur`; it cannot run
    // past the cue it belongs to.
    let tl = ShowTimeline::new()
        .at(0.0, 4.0, 10.0, 0.0, "fin")
        .at(10.0, 4.0, 0.0, 99.0, "fout");
    assert!(close(tl.cues()[0].fade_in, 4.0));
    assert!(close(tl.cues()[0].fade_out, 0.0));
    assert!(close(tl.cues()[1].fade_out, 4.0));
    // The fade-in cue ramps linearly across its whole window (0.5 at halfway).
    assert!(close(tl.resolve(2.0)[0].weight, 0.5));
    // The fade-out cue is full at its start and ramps down (0.5 at halfway).
    assert!(close(tl.resolve(10.0)[0].weight, 1.0));
    assert!(close(tl.resolve(12.0)[0].weight, 0.5));
}

#[test]
fn shot_chrome_renders_bounded_surfaces() {
    // Title card fills its viewport and does not panic at a sane size.
    let card = title_card(
        &TitleCard::new("GIBSON", "one experience, many grammars"),
        100,
        30,
    );
    assert_eq!((card.width, card.height), (100, 30));

    // The dip effect is a pure, bounded transform (0 = black, 1 = untouched).
    let mut c = title_card(&TitleCard::new("a", "b"), 40, 12);
    dim_toward_black(&mut c, 0.5);
    dim_toward_black(&mut c, 0.0);
    dim_toward_black(&mut c, 1.0); // no-op
    assert_eq!((c.width, c.height), (40, 12));

    // The shot payloads are stable value types the reel builds cues from.
    let reel_shots = [
        Shot::Title(TitleCard::new("x", "y")),
        Shot::Grammar(GrammarId::Orbital),
        Shot::Observatory(SceneId::Spectrum),
    ];
    assert_eq!(reel_shots.len(), 3);
    assert_eq!(GrammarId::REEL_ORDER.len(), 6);
    assert_eq!(SceneId::ALL.len(), 3);
}

#[test]
fn resolve_is_deterministic_across_repeated_queries() {
    let tl = ShowTimeline::new()
        .cut(3.0, 10usize)
        .crossfade(3.0, 1.0, 20usize);
    for t in [0.0f32, 1.0, 2.5, 2.75, 4.0, 5.9] {
        let a = tl.resolve(t);
        let b = tl.resolve(t);
        assert_eq!(a, b, "resolve({t}) must be pure");
    }
}
