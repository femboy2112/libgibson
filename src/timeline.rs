//! A content-agnostic edit-clock timeline: cue resolution for authored, seekable,
//! deterministic shows.
//!
//! Given an **edit-clock** position (seconds), a [`Timeline`] resolves which cues
//! are active, each cue's local **narrative-clock** time, its progress through its
//! window, and a crossfade weight. It is pure `f(edit)` math — no rendering, no
//! effects, no scene content, no wall clock, no knowledge of cameras, experiences
//! or audio. The thing that *uses* it (which shot plays when, the captions, the
//! camera, a future soundtrack) stays the author's — a timeline, not a director.
//!
//! The type is generic over an opaque payload `T` (a shot id, a closure handle,
//! whatever the show chooses); the timeline never inspects it. That is what keeps
//! it honest and reusable across unrelated shows: a projection
//! `M : Timeline<T> -> Whatever` (visuals, a semantic score, reaction cues) is the
//! caller's, exactly as *timeline semantics* are distinct from *visual realization*.
//!
//! # Two-clock discipline
//!
//! The edit clock is the only stored time. Each cue's narrative ("local") clock is
//! a *derived read* of it, so a cue's content is rebuilt and seeked every frame and
//! is never mutated across frames — the original content cannot drift or be
//! corrupted. A cue reports `local` seconds since its own start; what a shot does
//! once its content runs shorter than the cue window (freeze the final frame, loop,
//! idle) is the shot's business, never the timeline's.
//!
//! ```
//! use gibson::timeline::Timeline;
//!
//! // Three shots, end to end: a 3 s title, a 5 s body that dissolves in over 1 s,
//! // a 2 s outro.
//! let tl = Timeline::new()
//!     .cut(3.0, "title")
//!     .crossfade(5.0, 1.0, "body")
//!     .cut(2.0, "outro");
//!
//! assert_eq!(tl.duration(), 9.0);
//!
//! // Who is on screen 4 s in, and how far into their own shot?
//! let now = tl.top(4.0).unwrap();
//! assert_eq!(*now.payload, "body");
//! assert!((now.local - 2.0).abs() < 1e-6); // body started at 2.0 (3.0 - 1.0 fade)
//! ```

/// One cue: a half-open window `[start, start + dur)` on the edit clock carrying an
/// opaque payload, plus independent fade-in and fade-out ramps.
///
/// The two ramps are separate (not one symmetric `fade`) on purpose: an interior
/// shot fades *in* over its overlap with the previous shot and *out* over its
/// overlap with the next, and those overlaps need not be equal — and, crucially,
/// the first shot must be able to cut in hard (`fade_in == 0`) while still
/// dissolving *out* into the second. Coupling the two ends would force the opening
/// shot to fade up from black.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cue<T> {
    /// Edit-clock second at which this cue begins.
    pub start: f32,
    /// Visible duration on the edit clock. Always `> 0` for a stored cue.
    pub dur: f32,
    /// Fade-in ramp length (seconds) at the cue's start. `0.0` is a hard cut-in.
    /// Clamped to `[0, dur]`.
    pub fade_in: f32,
    /// Fade-out ramp length (seconds) at the cue's end. `0.0` is a hard cut-out.
    /// Clamped to `[0, dur]`.
    pub fade_out: f32,
    /// Opaque per-cue content handle. The timeline never inspects it.
    pub payload: T,
}

impl<T> Cue<T> {
    /// Exclusive end of the cue's edit-clock window.
    #[inline]
    pub fn end(&self) -> f32 {
        self.start + self.dur
    }

    /// Crossfade weight at edit time `t`: `0.0` outside `[start, end)`, ramping
    /// `0 -> 1` over the first `fade_in` seconds and `1 -> 0` over the last
    /// `fade_out` seconds, flat `1.0` in between. A hard edge (ramp `0.0`) is full
    /// weight right up to the half-open boundary. If `fade_in + fade_out` exceeds
    /// `dur` the two ramps meet below `1.0` — a very short shot that is all dissolve
    /// — which is well defined (the `min` never goes negative or above one), just
    /// never fully opaque.
    #[inline]
    pub fn weight(&self, t: f32) -> f32 {
        if !t.is_finite() || t < self.start || t >= self.end() {
            return 0.0;
        }
        let rising = if self.fade_in > 0.0 {
            (t - self.start) / self.fade_in
        } else {
            1.0
        };
        let falling = if self.fade_out > 0.0 {
            (self.end() - t) / self.fade_out
        } else {
            1.0
        };
        rising.min(falling).clamp(0.0, 1.0)
    }
}

/// The resolved state of one active cue at a given edit time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Active<'a, T> {
    /// The active cue's payload (borrowed from the timeline).
    pub payload: &'a T,
    /// The cue's index in insertion order.
    pub index: usize,
    /// Narrative-clock seconds since this cue's own start (`>= 0`). This is the
    /// "local time" a shot is seeked to; the shot decides what to do if it exceeds
    /// the shot's own content length.
    pub local: f32,
    /// `local / dur`, clamped to `[0, 1]` — how far through the cue window.
    pub progress: f32,
    /// Crossfade weight in `[0, 1]` (see [`Cue::weight`]).
    pub weight: f32,
}

/// An ordered set of cues on a single edit clock. Built fluently; queried by
/// [`resolve`](Timeline::resolve) / [`top`](Timeline::top). Holds no clock state of
/// its own — time is always supplied by the caller, so the same timeline resolves
/// identically for a live clock, a seek, or a deterministic capture.
#[derive(Debug, Clone)]
pub struct Timeline<T> {
    cues: Vec<Cue<T>>,
}

impl<T> Default for Timeline<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Timeline<T> {
    /// An empty timeline (zero duration, no cues).
    pub fn new() -> Self {
        Self { cues: Vec::new() }
    }

    /// Append a **hard cut** immediately after the current last cue ends (or at
    /// `0.0` for the first cue). A non-finite or non-positive `dur` is ignored, so a
    /// degenerate cue can never land on the timeline.
    pub fn cut(self, dur: f32, payload: T) -> Self {
        let start = self.duration();
        self.at(start, dur, 0.0, 0.0, payload)
    }

    /// Append a cue that **overlaps** the current end by `fade` seconds and
    /// cross-dissolves with it: the previous cue's *tail* is ramped out over the
    /// overlap (so an outgoing shot made with [`cut`](Self::cut) still dissolves)
    /// and the new cue fades *in* over the same window. The new cue starts at
    /// `prev_end - fade`; both cues are live during the overlap with complementary
    /// weights that sum to `1.0`. The new cue's own tail stays a hard cut until a
    /// *later* `crossfade` ramps it in turn.
    pub fn crossfade(mut self, dur: f32, fade: f32, payload: T) -> Self {
        let fade = if fade.is_finite() { fade.max(0.0) } else { 0.0 };
        if let Some(prev) = self.cues.last_mut() {
            let f = fade.min(prev.dur);
            prev.fade_out = prev.fade_out.max(f);
        }
        let start = (self.duration() - fade).max(0.0);
        self.at(start, dur, fade, 0.0, payload)
    }

    /// Place a cue at an explicit edit-clock `start` with explicit fade ramps. Cues
    /// may overlap; the resolver reports every one that is live at a queried time.
    /// Non-finite `start`/`dur` or `dur <= 0` are rejected (the call is a no-op),
    /// because a timeline with an unplaceable cue is a corpse that only panics
    /// later. Each ramp is clamped to `[0, dur]`.
    pub fn at(mut self, start: f32, dur: f32, fade_in: f32, fade_out: f32, payload: T) -> Self {
        if start.is_finite() && dur.is_finite() && dur > 0.0 {
            let clamp_ramp = |f: f32| {
                if f.is_finite() {
                    f.clamp(0.0, dur)
                } else {
                    0.0
                }
            };
            self.cues.push(Cue {
                start,
                dur,
                fade_in: clamp_ramp(fade_in),
                fade_out: clamp_ramp(fade_out),
                payload,
            });
        }
        self
    }

    /// The cues, in insertion order.
    pub fn cues(&self) -> &[Cue<T>] {
        &self.cues
    }

    /// Total edit-clock length: the largest cue end. Empty timelines are `0.0`.
    pub fn duration(&self) -> f32 {
        self.cues
            .iter()
            .map(Cue::end)
            .fold(0.0_f32, |acc, e| acc.max(e))
    }

    /// Every cue live at edit time `t`, in insertion order, each with its local
    /// narrative time, progress and crossfade weight. Empty if `t` is in a gap or
    /// non-finite — the "nothing scheduled here" case.
    pub fn resolve(&self, t: f32) -> Vec<Active<'_, T>> {
        if !t.is_finite() {
            return Vec::new();
        }
        self.cues
            .iter()
            .enumerate()
            .filter_map(|(index, cue)| {
                if t < cue.start || t >= cue.end() {
                    return None;
                }
                let local = (t - cue.start).max(0.0);
                let progress = (local / cue.dur).clamp(0.0, 1.0);
                Some(Active {
                    payload: &cue.payload,
                    index,
                    local,
                    progress,
                    weight: cue.weight(t),
                })
            })
            .collect()
    }

    /// The single dominant cue at edit time `t`: the live cue with the greatest
    /// crossfade weight (ties broken by the later cue, so an incoming shot takes
    /// over the instant it is at least as present as the outgoing one). `None` in a
    /// gap. This is the "which shot is the audience actually watching now" query.
    ///
    /// Windows are half-open `[start, end)`, so `top(self.duration())` is `None` —
    /// seeking to exactly the end lands *past* the final shot. To hold the last
    /// frame when seeking, clamp: `tl.top(t.min(tl.duration() - 1e-3))`.
    pub fn top(&self, t: f32) -> Option<Active<'_, T>> {
        self.resolve(t)
            .into_iter()
            .reduce(|best, cur| if cur.weight >= best.weight { cur } else { best })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny float comparison for weights/progress (f32 ramp arithmetic).
    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn empty_timeline_has_zero_duration_and_resolves_to_nothing() {
        let tl: Timeline<&str> = Timeline::new();
        assert_eq!(tl.duration(), 0.0);
        assert!(tl.resolve(0.0).is_empty());
        assert!(tl.resolve(5.0).is_empty());
        assert!(tl.top(0.0).is_none());
    }

    #[test]
    fn contiguous_cuts_lay_end_to_end_and_report_local_time() {
        let tl = Timeline::new().cut(3.0, "a").cut(2.0, "b").cut(4.0, "c");
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
        let tl = Timeline::new().cut(2.0, "a").cut(2.0, "b");
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
        let tl = Timeline::new().cut(4.0, "a").crossfade(4.0, 1.0, "b");
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
        let tl = Timeline::new().cut(4.0, "a").crossfade(4.0, 2.0, "b"); // overlap [2,4)

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
        let tl = Timeline::new().cut(2.0, "a");
        // Just before the exclusive end, progress is near 1 and local near dur.
        let near = tl.resolve(1.999);
        assert_eq!(near.len(), 1);
        assert!(near[0].progress > 0.99 && near[0].progress <= 1.0);
        assert!(near[0].local > 1.99 && near[0].local < 2.0);
    }

    #[test]
    fn degenerate_and_hostile_inputs_are_refused_without_panicking() {
        // Non-positive / non-finite durations never land a cue.
        let tl = Timeline::new()
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
        let tl = Timeline::new()
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
    fn resolve_is_deterministic_across_repeated_queries() {
        let tl = Timeline::new()
            .cut(3.0, 10usize)
            .crossfade(3.0, 1.0, 20usize);
        for t in [0.0f32, 1.0, 2.5, 2.75, 4.0, 5.9] {
            let a = tl.resolve(t);
            let b = tl.resolve(t);
            assert_eq!(a, b, "resolve({t}) must be pure");
        }
    }
}
