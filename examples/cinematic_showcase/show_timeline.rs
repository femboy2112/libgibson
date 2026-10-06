//! `ShowTimeline` — a content-agnostic edit-clock timeline (demo-local, promotable).
//!
//! This is the ONE reusable law extracted from the intro / reaction directors
//! (`examples/libgibson_intro/director.rs`, `examples/libgibson_intro_reaction/director.rs`):
//! given an **edit-clock** position, resolve which cues are active, each cue's
//! local **narrative-clock** time, its progress, and a crossfade weight. It is
//! pure `f(edit)` math — no rendering, no effects, no scene content, no wall
//! clock. The film that *uses* it (which shot plays when, the captions, the
//! camera) stays demo-local art direction, honoring the explicit injunction in
//! `docs/FRANK_REACTION_CUT_PLAN.md:84` and `docs/INTRODUCTORY_CINEMA.md:77-81`
//! that the per-film cue sheet must not harden into a general video editor.
//!
//! **Two-clock discipline** (`reaction/director.rs:176-178`): the edit clock is
//! the only stored time. Each cue's narrative clock is a *derived read* of it,
//! so a shot is rebuilt and seeked every frame and is never mutated across
//! frames — the original content cannot drift or be corrupted. A cue reports
//! `local` seconds since its own start; what a shot does once its content runs
//! shorter than the cue window (freeze the final frame, loop, idle) is the
//! shot's business, never the timeline's.
//!
//! The type is deliberately generic over an opaque payload `T` (a shot id, a
//! closure handle, whatever the film chooses) and carries none of the renderer.
//! That is what keeps it honest: a timeline, not a director.

#![allow(dead_code)] // a reusable primitive; a given film uses a subset of the API.

/// One cue: a half-open window `[start, start + dur)` on the edit clock carrying
/// an opaque payload, plus independent fade-in and fade-out ramps.
///
/// The two ramps are separate (not one symmetric `fade`) on purpose: a film's
/// interior shot fades *in* over its overlap with the previous shot and *out*
/// over its overlap with the next, and those overlaps need not be equal — and,
/// crucially, the first shot must be able to cut in hard (`fade_in == 0`) while
/// still dissolving *out* into the second. Coupling the two ends would force the
/// opening shot to fade up from black.
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
    /// `fade_out` seconds, flat `1.0` in between. A hard edge (ramp `0.0`) is
    /// full weight right up to the half-open boundary. If `fade_in + fade_out`
    /// exceeds `dur` the two ramps meet below `1.0` — a very short shot that is
    /// all dissolve — which is well defined (the `min` never goes negative or
    /// above one), just never fully opaque.
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
    /// "local time" a shot is seeked to; the shot decides what to do if it
    /// exceeds the shot's own content length.
    pub local: f32,
    /// `local / dur`, clamped to `[0, 1]` — how far through the cue window.
    pub progress: f32,
    /// Crossfade weight in `[0, 1]` (see [`Cue::weight`]).
    pub weight: f32,
}

/// An ordered set of cues on a single edit clock. Built fluently; queried by
/// [`resolve`](ShowTimeline::resolve) / [`top`](ShowTimeline::top). Holds no
/// clock state of its own — time is always supplied by the caller.
#[derive(Debug, Clone)]
pub struct ShowTimeline<T> {
    cues: Vec<Cue<T>>,
}

impl<T> Default for ShowTimeline<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> ShowTimeline<T> {
    /// An empty timeline (zero duration, no cues).
    pub fn new() -> Self {
        Self { cues: Vec::new() }
    }

    /// Append a **hard cut** immediately after the current last cue ends (or at
    /// `0.0` for the first cue). A non-finite or non-positive `dur` is ignored,
    /// so a degenerate cue can never land on the timeline.
    pub fn cut(self, dur: f32, payload: T) -> Self {
        let start = self.duration();
        self.at(start, dur, 0.0, 0.0, payload)
    }

    /// Append a cue that **overlaps** the current end by `fade` seconds and
    /// cross-dissolves with it: the previous cue's *tail* is ramped out over the
    /// overlap (so an outgoing shot made with [`cut`](Self::cut) still dissolves)
    /// and the new cue fades *in* over the same window. The new cue starts at
    /// `prev_end - fade`; both cues are live during the overlap with
    /// complementary weights that sum to `1.0`. The new cue's own tail stays a
    /// hard cut until a *later* `crossfade` ramps it in turn.
    pub fn crossfade(mut self, dur: f32, fade: f32, payload: T) -> Self {
        let fade = if fade.is_finite() { fade.max(0.0) } else { 0.0 };
        if let Some(prev) = self.cues.last_mut() {
            let f = fade.min(prev.dur);
            prev.fade_out = prev.fade_out.max(f);
        }
        let start = (self.duration() - fade).max(0.0);
        self.at(start, dur, fade, 0.0, payload)
    }

    /// Place a cue at an explicit edit-clock `start` with explicit fade ramps.
    /// Cues may overlap; the resolver reports every one that is live at a queried
    /// time. Non-finite `start`/`dur` or `dur <= 0` are rejected (the call is a
    /// no-op), because a timeline with an unplaceable cue is a corpse that only
    /// panics later, exactly the discipline `KeyedFilm::from_raw` uses for empty
    /// film. Each ramp is clamped to `[0, dur]`.
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
    /// narrative time, progress and crossfade weight. Empty if `t` is in a gap
    /// or non-finite — the "film plays alone" case of the reaction director.
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
    /// over the instant it is at least as present as the outgoing one). `None`
    /// in a gap. This is the "which shot is the audience actually watching now"
    /// query the HUD lower-third follows.
    pub fn top(&self, t: f32) -> Option<Active<'_, T>> {
        self.resolve(t)
            .into_iter()
            .reduce(|best, cur| if cur.weight >= best.weight { cur } else { best })
    }
}
