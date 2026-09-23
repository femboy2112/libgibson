//! Scene Algebra — a tiny compositional semantics of visual change.
//!
//! ## Why this exists
//!
//! LibGibson already has a stable renderer (`Node` → `Surface` → `Layout` →
//! compositing → diff → ANSI). What it did **not** have was a way for visual
//! dynamism to act on UI objects abstractly. Demos had to hand-wire
//! `if plague_frames > 0 { … }` into application state, forcing the effect, the
//! scene object and the application phase to know too much about each other.
//!
//! This module introduces the smallest abstraction that removes that coupling:
//!
//! * a [`Scene`] is a set of semantically identified [`SceneEntity`]s, each
//!   wrapping an ordinary [`Node`];
//! * an [`Effect`] is a deterministic transformation of *presentation* channels
//!   (placement, additive displacement, visibility, camera, custom, surface FX),
//!   targeting entities by [`SceneId`]
//!   or [`TagId`];
//! * [`Effect::parallel`] and [`Effect::sequence`] compose effects, and
//!   [`Effect::identity`] is the no-op.
//!
//! ## Category-theoretic reading (optional)
//!
//! The category-theoretic language is *not required* to use the API. It simply
//! names the laws the API is designed to preserve:
//!
//! * **Objects** are scene states; **morphisms** are effects. Composition
//!   `f ; g` is [`Effect::sequence`]; the identity is [`Effect::identity`].
//! * **Monoidal product** `f ⊗ g` is [`Effect::parallel`]: run both over the
//!   same interval. Parallel effects on *independent* channels/targets commute;
//!   placement/scalar writes resolve by explicit write order. Displacements
//!   add as integer vectors; surface operations append in deterministic order.
//! * **Functor** `Render : SCENE → UI`: [`Scene::to_node`] maps a scene
//!   presentation into ordinary `Node`s (a `Stack` of offset layers inside an
//!   optional camera `Viewport`). It is **not** a second renderer; it produces
//!   the same declarative render objects everything else uses.
//!
//! Everything here is a pure function of its inputs and of time, so a scene
//! frame is reproducible under [`crate::FixedStepClock`] and suitable for
//! goldens, replay and tests.

use crate::node::Node;
use crate::{Style, SurfaceFx};
use std::collections::BTreeMap;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

/// Stable identity for a scene entity, assigned by [`Scene::add`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SceneId(pub u64);

/// Stable identity for a semantic tag, interned by [`Scene::tag`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TagId(pub u64);

/// An effect target: one entity by id, or every entity carrying a tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SceneTarget {
    Id(SceneId),
    Tag(TagId),
}

/// A presentation channel. Effects write channels; they never mutate a `Node`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Channel {
    /// Integer cell translation (`(dx, dy)`), applied as an absolute-layer offset.
    Position,
    /// Additive integer cell perturbations, summed after absolute placement.
    Displacement,
    /// Ordered post-process endomorphisms of a realized entity surface.
    SurfaceFx,
    /// Visibility in `[0, 1]`. Terminals have no alpha, so the render functor
    /// thresholds at `> 0.0`; the precise value is available to raster effects.
    Visibility,
    /// Global camera offset in cells (one camera per scene).
    Camera,
    /// Application-defined scalar channel (e.g. glitch intensity, scan reveal).
    Custom(u64),
}

// ---------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------

/// A semantically identified wrapper around an ordinary [`Node`].
///
/// A `SceneEntity` deliberately **does not** duplicate `Node` functionality: it
/// adds identity, tags and a z-order. All presentation changes come from effects.
#[derive(Debug, Clone)]
pub struct SceneEntity {
    /// Filled in by [`Scene::add`]; a placeholder `SceneId(0)` before that.
    pub id: SceneId,
    /// Human-readable label, unique within a scene, used for ergonomic lookup.
    pub label: String,
    /// Tags interned at add time.
    pub tags: Vec<TagId>,
    tag_labels: Vec<String>,
    /// The declarative render object this entity represents.
    pub node: Node,
    /// Sort key for compositing; higher paints later (on top).
    pub z: i32,
    /// Baseline visibility when no effect overrides it.
    pub visible: bool,
    /// Baseline cell offset when no effect overrides it.
    pub offset: (i32, i32),
}

impl SceneEntity {
    /// Creates an entity from a label and an ordinary node.
    pub fn new(label: impl Into<String>, node: Node) -> Self {
        Self {
            id: SceneId(0),
            label: label.into(),
            tags: Vec::new(),
            tag_labels: Vec::new(),
            node,
            z: 0,
            visible: true,
            offset: (0, 0),
        }
    }

    /// Adds a tag. Interned into a [`TagId`] when added to a [`Scene`].
    pub fn tag(mut self, name: impl Into<String>) -> Self {
        self.tag_labels.push(name.into());
        self
    }

    /// Sets the compositing z-order (higher is on top).
    pub fn z(mut self, z: i32) -> Self {
        self.z = z;
        self
    }

    /// Starts hidden; an effect can reveal it.
    pub fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }

    /// Sets a baseline cell offset.
    pub fn offset(mut self, x: i32, y: i32) -> Self {
        self.offset = (x, y);
        self
    }

    /// The tag labels requested on this entity (before interning).
    pub fn tag_labels(&self) -> &[String] {
        &self.tag_labels
    }
}

// ---------------------------------------------------------------------------
// Presentation
// ---------------------------------------------------------------------------

/// The evaluated presentation delta produced by effects.
///
/// Placement/scalars override in write order; displacement sums and surface
/// effects append in write order. Build a fresh presentation each frame: `eval`
/// accumulates contributions and does not reset a previously populated value.
/// Fields are populated only for channels an effect actually wrote, so untouched channels fall
/// back to the entity's baseline in [`Scene::to_node`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Presentation {
    offsets: BTreeMap<SceneId, (i32, i32)>,
    displacements: BTreeMap<SceneId, (i128, i128)>,
    surface_fx: BTreeMap<SceneId, Vec<SurfaceFx>>,
    visibility: BTreeMap<SceneId, f32>,
    custom: BTreeMap<(SceneId, u64), f32>,
    camera: (i32, i32),
}

impl Presentation {
    pub fn new() -> Self {
        Self::default()
    }

    /// Absolute placement (effect override or baseline) plus the sum of rounded
    /// displacement contributions, clamped to `i32` only after summation.
    pub fn offset_of(&self, scene: &Scene, id: SceneId) -> (i32, i32) {
        let placement = self
            .offsets
            .get(&id)
            .copied()
            .unwrap_or_else(|| scene.baseline_offset(id));
        let delta = self.displacement_of(id);
        (
            (i128::from(placement.0) + delta.0).clamp(i32::MIN as i128, i32::MAX as i128) as i32,
            (i128::from(placement.1) + delta.1).clamp(i32::MIN as i128, i32::MAX as i128) as i32,
        )
    }

    /// Summed additive displacement, before the final coordinate clamp.
    pub fn displacement_of(&self, id: SceneId) -> (i128, i128) {
        self.displacements.get(&id).copied().unwrap_or_default()
    }

    /// Ordered post-process chain. An empty chain adds no rasterization.
    pub fn surface_fx(&self, id: SceneId) -> &[SurfaceFx] {
        self.surface_fx.get(&id).map(Vec::as_slice).unwrap_or(&[])
    }

    fn add_displacement(&mut self, id: SceneId, delta: (i32, i32)) {
        let sum = self.displacements.entry(id).or_default();
        sum.0 += i128::from(delta.0);
        sum.1 += i128::from(delta.1);
    }

    fn append_fx(&mut self, id: SceneId, fx: SurfaceFx) {
        self.surface_fx.entry(id).or_default().push(fx);
    }

    /// Effective visibility for an entity (effect override, else baseline).
    pub fn visibility_of(&self, scene: &Scene, id: SceneId) -> f32 {
        self.visibility.get(&id).copied().unwrap_or_else(|| {
            if scene.baseline_visible(id) {
                1.0
            } else {
                0.0
            }
        })
    }

    /// Raw visibility override, if any.
    pub fn raw_visibility(&self, id: SceneId) -> Option<f32> {
        self.visibility.get(&id).copied()
    }

    /// Raw absolute placement override, excluding additive displacement.
    pub fn raw_offset(&self, id: SceneId) -> Option<(i32, i32)> {
        self.offsets.get(&id).copied()
    }

    /// A custom scalar channel value, if written.
    pub fn custom(&self, id: SceneId, key: u64) -> Option<f32> {
        self.custom.get(&(id, key)).copied()
    }

    /// Global camera offset in cells.
    pub fn camera(&self) -> (i32, i32) {
        self.camera
    }

    fn set_offset(&mut self, id: SceneId, v: (i32, i32)) {
        self.offsets.insert(id, v);
    }

    fn set_visibility(&mut self, id: SceneId, v: f32) {
        self.visibility.insert(id, v);
    }

    fn set_custom(&mut self, id: SceneId, key: u64, v: f32) {
        self.custom.insert((id, key), v);
    }
}

// ---------------------------------------------------------------------------
// Easing
// ---------------------------------------------------------------------------

/// Interpolation curve for ramp effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Easing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Spring,
}

impl Easing {
    fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseIn => crate::clock::ease_in(t),
            Easing::EaseOut => crate::clock::ease_out(t),
            Easing::EaseInOut => crate::clock::ease_in_out(t),
            Easing::Spring => crate::clock::spring(t),
        }
    }
}

// ---------------------------------------------------------------------------
// Effects
// ---------------------------------------------------------------------------

/// A deterministic transformation of presentation channels.
///
/// Morphisms compose with [`Effect::sequence`]; the monoidal product is
/// [`Effect::parallel`]; [`Effect::identity`] is the no-op. See the module docs.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Do nothing. Composition stays regular under conditionals.
    Identity,
    /// Translate the [`Channel::Position`] from `from` to `to` cells.
    Translate {
        target: SceneTarget,
        from: (f32, f32),
        to: (f32, f32),
        duration: Duration,
        easing: Easing,
    },
    /// Add a rounded displacement without overwriting absolute placement.
    Displace {
        target: SceneTarget,
        from: (f32, f32),
        to: (f32, f32),
        duration: Duration,
        easing: Easing,
    },
    /// Add deterministic jitter without overwriting absolute placement.
    Jitter {
        target: SceneTarget,
        amplitude: f32,
        period: Duration,
        duration: Duration,
    },
    /// Append a static post-process operation to every matching entity.
    PostProcess { target: SceneTarget, fx: SurfaceFx },
    /// Terminal-native reveal by a stable cell mask, independent of visibility.
    Dissolve {
        target: SceneTarget,
        from: f32,
        to: f32,
        seed: u64,
        duration: Duration,
        easing: Easing,
    },
    /// Progressively apply a style through a stable cell mask without erasure.
    StyleMask {
        target: SceneTarget,
        style: Style,
        from: f32,
        to: f32,
        seed: u64,
        duration: Duration,
        easing: Easing,
    },
    /// Move one styled scanline down an entity's realized surface.
    Scanline {
        target: SceneTarget,
        style: Style,
        duration: Duration,
    },
    /// Ramp [`Channel::Visibility`] from `from` to `to` in `[0, 1]`.
    Reveal {
        target: SceneTarget,
        from: f32,
        to: f32,
        duration: Duration,
        easing: Easing,
    },
    /// Pan the global camera from `from` to `to` cells.
    CameraPan {
        from: (f32, f32),
        to: (f32, f32),
        duration: Duration,
        easing: Easing,
    },
    /// Deterministic shake around the origin, overriding position for `duration`.
    Shake {
        target: SceneTarget,
        amplitude: f32,
        period: Duration,
        duration: Duration,
    },
    /// Instantly set a custom scalar channel.
    SetCustom {
        target: SceneTarget,
        key: u64,
        value: f32,
    },
    /// Ramp a custom scalar channel from `from` to `to`.
    CustomRamp {
        target: SceneTarget,
        key: u64,
        from: f32,
        to: f32,
        duration: Duration,
        easing: Easing,
    },
    /// Run all children over the same interval (monoidal product).
    Parallel(Vec<Effect>),
    /// Run children one after another (categorical composition).
    Sequence(Vec<Effect>),
    /// Run `inner` after `delay`.
    Delay(Duration, Box<Effect>),
    /// Run `inner` `times` times in sequence.
    Repeat(Box<Effect>, usize),
    /// Repeat using exact integer-nanosecond modulo; zero duration is a no-op.
    /// `duration()` reports `Duration::MAX`, the representable time horizon.
    /// Reverse a finite child before looping it; reversing a loop instead uses
    /// that horizon as its phase origin. Cycles reset, never accumulate motion.
    Loop(Box<Effect>),
    /// Run `inner` backwards in time.
    Reverse(Box<Effect>),
}

impl Effect {
    /// The no-op morphism.
    pub fn identity() -> Self {
        Effect::Identity
    }

    /// Translate `target` from `from` to `to` over `duration`.
    pub fn translate(
        target: SceneTarget,
        from: (f32, f32),
        to: (f32, f32),
        duration: Duration,
    ) -> Self {
        Effect::Translate {
            target,
            from,
            to,
            duration,
            easing: Easing::Linear,
        }
    }

    /// Add a rounded displacement to placement. Multiple displacements commute.
    pub fn displace(
        target: SceneTarget,
        from: (f32, f32),
        to: (f32, f32),
        duration: Duration,
    ) -> Self {
        Self::Displace {
            target,
            from,
            to,
            duration,
            easing: Easing::Linear,
        }
    }

    /// Add jitter around placement; unlike legacy `shake`, this composes with motion.
    pub fn jitter(
        target: SceneTarget,
        amplitude: f32,
        period: Duration,
        duration: Duration,
    ) -> Self {
        Self::Jitter {
            target,
            amplitude,
            period,
            duration,
        }
    }

    /// Append an operation after normal entity painting. Static operations persist
    /// while mounted, even though their interpolation duration is zero.
    pub fn post_process(target: SceneTarget, fx: SurfaceFx) -> Self {
        Self::PostProcess { target, fx }
    }

    /// Reveal cells with a stable threshold mask. Seeds are mixed with entity ids
    /// for tag targets so separate entities do not share identical masks.
    pub fn dissolve(
        target: SceneTarget,
        from: f32,
        to: f32,
        seed: u64,
        duration: Duration,
    ) -> Self {
        Self::Dissolve {
            target,
            from,
            to,
            seed,
            duration,
            easing: Easing::Linear,
        }
    }

    /// Spread a style over a stable cell mask, preserving every source glyph.
    /// Tag targets mix the explicit seed with each entity's stable identity.
    pub fn style_mask(
        target: SceneTarget,
        style: Style,
        from: f32,
        to: f32,
        seed: u64,
        duration: Duration,
    ) -> Self {
        Self::StyleMask {
            target,
            style,
            from,
            to,
            seed,
            duration,
            easing: Easing::Linear,
        }
    }

    /// Sweep a scanline from the top to bottom of a realized entity.
    pub fn scanline(target: SceneTarget, style: Style, duration: Duration) -> Self {
        Self::Scanline {
            target,
            style,
            duration,
        }
    }

    /// Keep a finite effect running until its owning bundle is removed.
    /// Each cycle restarts at zero; it does not accumulate previous cycles.
    /// A zero-duration child contributes nothing. For backwards cycles, wrap
    /// the finite child in `Reverse` first, then call `looping`.
    pub fn looping(self) -> Self {
        Self::Loop(Box::new(self))
    }

    /// Reveal/ramp `target`'s visibility.
    pub fn reveal(target: SceneTarget, from: f32, to: f32, duration: Duration) -> Self {
        Effect::Reveal {
            target,
            from,
            to,
            duration,
            easing: Easing::Linear,
        }
    }

    /// Pan the camera.
    pub fn camera_pan(from: (f32, f32), to: (f32, f32), duration: Duration) -> Self {
        Effect::CameraPan {
            from,
            to,
            duration,
            easing: Easing::Linear,
        }
    }

    /// Shake `target` around the origin.
    pub fn shake(
        target: SceneTarget,
        amplitude: f32,
        period: Duration,
        duration: Duration,
    ) -> Self {
        Effect::Shake {
            target,
            amplitude,
            period,
            duration,
        }
    }

    /// Set a custom scalar channel instantly.
    pub fn set_custom(target: SceneTarget, key: u64, value: f32) -> Self {
        Effect::SetCustom { target, key, value }
    }

    /// Ramp a custom scalar channel.
    pub fn custom_ramp(
        target: SceneTarget,
        key: u64,
        from: f32,
        to: f32,
        duration: Duration,
    ) -> Self {
        Effect::CustomRamp {
            target,
            key,
            from,
            to,
            duration,
            easing: Easing::Linear,
        }
    }

    /// Composition: run `self`, then `other`.
    pub fn then(self, other: Effect) -> Self {
        Effect::Sequence(vec![self, other])
    }

    /// Monoidal product: run `self` and `other` in parallel.
    pub fn with(self, other: Effect) -> Self {
        Effect::Parallel(vec![self, other])
    }

    /// Builds a parallel composition (empty ⇒ identity).
    pub fn parallel(effects: impl IntoIterator<Item = Effect>) -> Self {
        let v: Vec<Effect> = effects.into_iter().collect();
        match v.len() {
            0 => Effect::Identity,
            1 => v.into_iter().next().unwrap(),
            _ => Effect::Parallel(v),
        }
    }

    /// Builds a sequential composition (empty ⇒ identity).
    pub fn sequence(effects: impl IntoIterator<Item = Effect>) -> Self {
        let v: Vec<Effect> = effects.into_iter().collect();
        match v.len() {
            0 => Effect::Identity,
            1 => v.into_iter().next().unwrap(),
            _ => Effect::Sequence(v),
        }
    }

    /// Sets the easing curve on a ramp variant (no-op for composites).
    pub fn eased(mut self, e: Easing) -> Self {
        match &mut self {
            Effect::Translate { easing, .. }
            | Effect::Displace { easing, .. }
            | Effect::Dissolve { easing, .. }
            | Effect::StyleMask { easing, .. }
            | Effect::Reveal { easing, .. }
            | Effect::CameraPan { easing, .. }
            | Effect::CustomRamp { easing, .. } => *easing = e,
            _ => {}
        }
        self
    }

    /// Total duration of the effect.
    pub fn duration(&self) -> Duration {
        match self {
            Effect::Identity | Effect::SetCustom { .. } | Effect::PostProcess { .. } => {
                Duration::ZERO
            }
            Effect::Translate { duration, .. }
            | Effect::Displace { duration, .. }
            | Effect::Jitter { duration, .. }
            | Effect::Dissolve { duration, .. }
            | Effect::StyleMask { duration, .. }
            | Effect::Scanline { duration, .. }
            | Effect::Reveal { duration, .. }
            | Effect::CameraPan { duration, .. }
            | Effect::Shake { duration, .. }
            | Effect::CustomRamp { duration, .. } => *duration,
            Effect::Parallel(v) => v
                .iter()
                .map(|e| e.duration())
                .max()
                .unwrap_or(Duration::ZERO),
            Effect::Sequence(v) => v
                .iter()
                .fold(Duration::ZERO, |d, e| d.saturating_add(e.duration())),
            Effect::Delay(d, inner) => d.saturating_add(inner.duration()),
            Effect::Repeat(inner, n) => repeated_duration(inner.duration(), *n),
            Effect::Loop(_) => Duration::MAX,
            Effect::Reverse(inner) => inner.duration(),
        }
    }

    fn local_t(&self, t: Duration, d: Duration) -> f32 {
        if d.is_zero() {
            1.0
        } else {
            (t.as_secs_f32() / d.as_secs_f32()).clamp(0.0, 1.0)
        }
    }

    /// Evaluates the effect at local time `t`, writing presentation deltas.
    ///
    /// Deterministically accumulates into `out`; start each frame with a fresh
    /// presentation. Effects never touch the terminal, renderer or source `Node`.
    /// Sequences retain elapsed children's final contributions, including
    /// displacement and surface effects.
    pub fn eval(&self, t: Duration, scene: &Scene, out: &mut Presentation) {
        let d = self.duration();
        let k = self.local_t(t, d);
        match self {
            Effect::Identity => {}
            Effect::Translate {
                target,
                from,
                to,
                easing,
                ..
            } => {
                let e = easing.apply(k);
                let v = (
                    crate::clock::lerp(from.0, to.0, e).round() as i32,
                    crate::clock::lerp(from.1, to.1, e).round() as i32,
                );
                for id in scene.resolve(*target) {
                    out.set_offset(id, v);
                }
            }
            Effect::Displace {
                target,
                from,
                to,
                easing,
                ..
            } => {
                let e = easing.apply(k);
                let delta = (
                    crate::clock::lerp(from.0, to.0, e).round() as i32,
                    crate::clock::lerp(from.1, to.1, e).round() as i32,
                );
                for id in scene.resolve(*target) {
                    out.add_displacement(id, delta);
                }
            }
            Effect::Jitter {
                target,
                amplitude,
                period,
                ..
            } => {
                if period.is_zero() {
                    return;
                }
                let phase = std::f64::consts::TAU
                    * ((t.as_nanos() % period.as_nanos()) as f64 / period.as_nanos() as f64);
                let delta = (
                    (phase.sin() * f64::from(*amplitude)).round() as i32,
                    (phase.cos() * f64::from(*amplitude)).round() as i32,
                );
                for id in scene.resolve(*target) {
                    out.add_displacement(id, delta);
                }
            }
            Effect::PostProcess { target, fx } => {
                for id in scene.resolve(*target) {
                    let mut fx = fx.clone();
                    if let (
                        SceneTarget::Tag(_),
                        SurfaceFx::Dissolve { seed, .. } | SurfaceFx::StyleMask { seed, .. },
                    ) = (target, &mut fx)
                    {
                        *seed ^= id.0.wrapping_mul(0x9e3779b97f4a7c15);
                    }
                    out.append_fx(id, fx);
                }
            }
            Effect::Dissolve {
                target,
                from,
                to,
                seed,
                easing,
                ..
            } => {
                let fraction = crate::clock::lerp(*from, *to, easing.apply(k));
                for id in scene.resolve(*target) {
                    let seed = match target {
                        SceneTarget::Tag(_) => seed ^ id.0.wrapping_mul(0x9e3779b97f4a7c15),
                        SceneTarget::Id(_) => *seed,
                    };
                    out.append_fx(id, SurfaceFx::Dissolve { seed, fraction });
                }
            }
            Effect::StyleMask {
                target,
                style,
                from,
                to,
                seed,
                easing,
                ..
            } => {
                let fraction = crate::clock::lerp(*from, *to, easing.apply(k));
                for id in scene.resolve(*target) {
                    let seed = match target {
                        SceneTarget::Tag(_) => seed ^ id.0.wrapping_mul(0x9e3779b97f4a7c15),
                        SceneTarget::Id(_) => *seed,
                    };
                    out.append_fx(
                        id,
                        SurfaceFx::StyleMask {
                            style: *style,
                            seed,
                            fraction,
                        },
                    );
                }
            }
            Effect::Scanline { target, style, .. } => {
                for id in scene.resolve(*target) {
                    out.append_fx(
                        id,
                        SurfaceFx::Scanline {
                            position: k,
                            style: *style,
                        },
                    );
                }
            }
            Effect::Reveal {
                target,
                from,
                to,
                easing,
                ..
            } => {
                let e = easing.apply(k);
                let v = crate::clock::lerp(*from, *to, e);
                for id in scene.resolve(*target) {
                    out.set_visibility(id, v);
                }
            }
            Effect::CameraPan {
                from, to, easing, ..
            } => {
                let e = easing.apply(k);
                out.camera = (
                    crate::clock::lerp(from.0, to.0, e).round() as i32,
                    crate::clock::lerp(from.1, to.1, e).round() as i32,
                );
            }
            Effect::Shake {
                target,
                amplitude,
                period,
                ..
            } => {
                let p = period.as_secs_f32().max(1e-6);
                let phase = std::f32::consts::TAU * (t.as_secs_f32() / p);
                let v = (
                    (phase.sin() * amplitude).round() as i32,
                    (phase.cos() * amplitude).round() as i32,
                );
                for id in scene.resolve(*target) {
                    out.set_offset(id, v);
                }
            }
            Effect::SetCustom { target, key, value } => {
                for id in scene.resolve(*target) {
                    out.set_custom(id, *key, *value);
                }
            }
            Effect::CustomRamp {
                target,
                key,
                from,
                to,
                easing,
                ..
            } => {
                let e = easing.apply(k);
                let v = crate::clock::lerp(*from, *to, e);
                for id in scene.resolve(*target) {
                    out.set_custom(id, *key, v);
                }
            }
            Effect::Parallel(v) => {
                // Explicit order for scalar/placement writes and surface chains;
                // displacement contributions sum independently of that order.
                for e in v {
                    e.eval(t, scene, out);
                }
            }
            Effect::Sequence(v) => {
                let mut elapsed = t.as_nanos();
                for e in v {
                    let ed = e.duration();
                    if elapsed >= ed.as_nanos() {
                        // Subtract exact elapsed time, rather than saturating a
                        // cumulative endpoint and accidentally skipping children.
                        e.eval(ed, scene, out);
                        elapsed -= ed.as_nanos();
                    } else {
                        e.eval(duration_from_nanos(elapsed), scene, out);
                        break;
                    }
                }
            }
            Effect::Delay(delay, inner) => {
                if t >= *delay {
                    inner.eval(t.saturating_sub(*delay), scene, out);
                }
            }
            Effect::Repeat(inner, n) => {
                // Zero-duration or zero-count inner is a no-op (documented).
                let id = inner.duration();
                if id.is_zero() || *n == 0 {
                    return;
                }
                let total_nanos = id.as_nanos().saturating_mul(*n as u128);
                if t.as_nanos() >= total_nanos {
                    inner.eval(id, scene, out);
                } else {
                    inner.eval(
                        duration_from_nanos(t.as_nanos() % id.as_nanos()),
                        scene,
                        out,
                    );
                }
            }
            Effect::Loop(inner) => {
                let period = inner.duration();
                if !period.is_zero() {
                    inner.eval(
                        duration_from_nanos(t.as_nanos() % period.as_nanos()),
                        scene,
                        out,
                    );
                }
            }
            Effect::Reverse(inner) => {
                inner.eval(d.saturating_sub(t.min(d)), scene, out);
            }
        }
    }
}

// Convert in seconds plus nanoseconds instead of truncating at u64 nanoseconds.
fn duration_from_nanos(nanos: u128) -> Duration {
    let nanos = nanos.min(Duration::MAX.as_nanos());
    Duration::new(
        (nanos / 1_000_000_000) as u64,
        (nanos % 1_000_000_000) as u32,
    )
}

fn repeated_duration(duration: Duration, times: usize) -> Duration {
    duration_from_nanos(duration.as_nanos().saturating_mul(times as u128))
}

// ---------------------------------------------------------------------------
// Scene
// ---------------------------------------------------------------------------

/// An error from [`Scene::try_add`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneError {
    /// An entity with this label already exists in the scene.
    DuplicateLabel(String),
}

impl std::fmt::Display for SceneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SceneError::DuplicateLabel(s) => write!(f, "duplicate scene entity label: `{s}`"),
        }
    }
}

/// A set of semantically identified entities, plus the functor to ordinary `Node`s.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    entities: Vec<SceneEntity>,
    by_label: BTreeMap<String, SceneId>,
    tag_ids: BTreeMap<String, TagId>,
    tag_names: BTreeMap<TagId, String>,
    next_id: u64,
    next_tag: u64,
}

/// The result of evaluating a scene at a time: an entity with effective styling.
#[derive(Debug, Clone)]
pub struct ResolvedEntity {
    pub id: SceneId,
    pub label: String,
    pub z: i32,
    pub offset: (i32, i32),
    pub visible: bool,
    pub visibility: f32,
    pub node: Node,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an entity, assigning it a stable [`SceneId`] and interning its tags.
    ///
    /// Labels are unique within a scene. **Panics** on a duplicate label — use
    /// [`Scene::try_add`] to handle the collision explicitly. Silent overwrite
    /// would make `Scene::target(label)` ambiguous.
    pub fn add(&mut self, entity: SceneEntity) -> SceneId {
        let label = entity.label.clone();
        if self.by_label.contains_key(&label) {
            panic!("duplicate scene entity label: `{label}`");
        }
        self.add_unchecked(entity)
    }

    /// Fallible add; returns [`SceneError::DuplicateLabel`] instead of panicking.
    pub fn try_add(&mut self, entity: SceneEntity) -> Result<SceneId, SceneError> {
        if self.by_label.contains_key(&entity.label) {
            return Err(SceneError::DuplicateLabel(entity.label));
        }
        Ok(self.add_unchecked(entity))
    }

    fn add_unchecked(&mut self, mut entity: SceneEntity) -> SceneId {
        let id = SceneId(self.next_id);
        self.next_id += 1;
        entity.id = id;
        entity.tags = entity
            .tag_labels
            .iter()
            .map(|t| self.intern_tag(t))
            .collect();
        self.by_label.insert(entity.label.clone(), id);
        self.entities.push(entity);
        id
    }

    /// Interns a tag name into a [`TagId`].
    pub fn tag(&mut self, name: impl Into<String>) -> TagId {
        self.intern_tag(&name.into())
    }

    fn intern_tag(&mut self, name: &str) -> TagId {
        if let Some(t) = self.tag_ids.get(name) {
            return *t;
        }
        let t = TagId(self.next_tag);
        self.next_tag += 1;
        self.tag_ids.insert(name.to_string(), t);
        self.tag_names.insert(t, name.to_string());
        t
    }

    /// Looks up an entity id by its label.
    pub fn id_of(&self, label: &str) -> Option<SceneId> {
        self.by_label.get(label).copied()
    }

    /// An ergonomic target by label.
    pub fn target(&self, label: &str) -> Option<SceneTarget> {
        self.id_of(label).map(SceneTarget::Id)
    }

    pub fn entities(&self) -> &[SceneEntity] {
        &self.entities
    }

    /// Mutable access to an entity (e.g. to refresh a dynamic overlay's node).
    pub fn entity_mut(&mut self, id: SceneId) -> Option<&mut SceneEntity> {
        self.entities.iter_mut().find(|e| e.id == id)
    }

    pub fn entity(&self, id: SceneId) -> Option<&SceneEntity> {
        self.entities.iter().find(|e| e.id == id)
    }

    /// Replaces an entity's node in place. Returns `false` if the id is unknown.
    pub fn set_node(&mut self, id: SceneId, node: Node) -> bool {
        match self.entity_mut(id) {
            Some(e) => {
                e.node = node;
                true
            }
            None => false,
        }
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Entity ids matching a target (id target matches at most one).
    pub fn resolve(&self, target: SceneTarget) -> Vec<SceneId> {
        match target {
            SceneTarget::Id(id) => {
                if self.entities.iter().any(|e| e.id == id) {
                    vec![id]
                } else {
                    Vec::new()
                }
            }
            SceneTarget::Tag(tag) => self
                .entities
                .iter()
                .filter(|e| e.tags.contains(&tag))
                .map(|e| e.id)
                .collect(),
        }
    }

    fn baseline_offset(&self, id: SceneId) -> (i32, i32) {
        self.entities
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.offset)
            .unwrap_or((0, 0))
    }

    fn baseline_visible(&self, id: SceneId) -> bool {
        self.entities
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.visible)
            .unwrap_or(false)
    }

    /// Resolves every entity to its effective presentation, sorted by z then
    /// insertion order (stable, deterministic).
    pub fn evaluate(&self, p: &Presentation) -> Vec<ResolvedEntity> {
        let mut out: Vec<ResolvedEntity> = self
            .entities
            .iter()
            .map(|e| {
                let visibility = p.visibility_of(self, e.id);
                ResolvedEntity {
                    id: e.id,
                    label: e.label.clone(),
                    z: e.z,
                    offset: p.offset_of(self, e.id),
                    visible: visibility > 0.0,
                    visibility,
                    node: e.node.clone().post_process(p.surface_fx(e.id).to_vec()),
                }
            })
            .collect();
        out.sort_by_key(|r| (r.z, r.id.0));
        out
    }

    /// The functor `Render : SCENE → UI`.
    ///
    /// Produces an ordinary `Node` tree: a `Stack` of offset layers, wrapped in a
    /// camera `Viewport` when the camera is non-zero. Hidden entities are omitted.
    /// This is deliberately *not* a second renderer — it emits the same
    /// declarative nodes that flow through the existing layout/paint/diff path.
    pub fn to_node(&self, p: &Presentation, width: f32, height: f32) -> Node {
        let mut stack = Node::stack().width(width).height(height);
        for r in self.evaluate(p) {
            if !r.visible {
                continue;
            }
            stack = stack.child(r.node.offset(r.offset.0 as f32, r.offset.1 as f32));
        }
        let (cx, cy) = p.camera();
        if cx != 0 || cy != 0 {
            Node::viewport(cx, cy)
                .width(width)
                .height(height)
                .child(stack)
        } else {
            stack
        }
    }
}

/// A named group of effects that can be mounted and unmounted together.
///
/// This is the engine-level answer to "one semantic cause, many coordinated
/// presentations": mounting `"plague-presence"` starts its whole bundle with no
/// per-effect timer synchronisation.
#[derive(Debug, Clone)]
pub struct EffectBundle {
    pub name: String,
    pub effects: Vec<Effect>,
}

impl EffectBundle {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            effects: Vec::new(),
        }
    }

    pub fn effect(mut self, e: Effect) -> Self {
        self.effects.push(e);
        self
    }

    pub fn effects(mut self, es: impl IntoIterator<Item = Effect>) -> Self {
        self.effects.extend(es);
        self
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Style;

    fn text(label: &str) -> Node {
        Node::text(label, Style::default()).width(4.0).height(1.0)
    }

    #[test]
    fn add_assigns_stable_ids_and_interns_tags() {
        let mut scene = Scene::new();
        let a = scene.add(SceneEntity::new("a", text("a")).tag("crew"));
        let b = scene.add(SceneEntity::new("b", text("b")).tag("crew").tag("city"));
        assert_ne!(a, b);
        assert_eq!(scene.id_of("a"), Some(a));
        let crew = scene.tag("crew");
        let mut ids = scene.resolve(SceneTarget::Tag(crew));
        ids.sort();
        assert_eq!(ids, vec![a.min(b), a.max(b)]);
        assert_eq!(scene.len(), 2);
    }

    #[test]
    fn duplicate_labels_are_rejected() {
        let mut scene = Scene::new();
        let a = scene.add(SceneEntity::new("plague", text("1")));
        // Fallible path reports the collision instead of overwriting.
        let err = scene.try_add(SceneEntity::new("plague", text("2")));
        assert_eq!(err, Err(SceneError::DuplicateLabel("plague".into())));
        // The original mapping and entity count are unchanged.
        assert_eq!(scene.id_of("plague"), Some(a));
        assert_eq!(scene.len(), 1);
        assert_eq!(scene.resolve(SceneTarget::Id(a)), vec![a]);
    }

    #[test]
    #[should_panic(expected = "duplicate scene entity label")]
    fn add_panics_loudly_on_duplicate_label() {
        let mut scene = Scene::new();
        scene.add(SceneEntity::new("plague", text("1")));
        scene.add(SceneEntity::new("plague", text("2")));
    }

    #[test]
    fn identity_and_interning_are_deterministic() {
        let build = || {
            let mut scene = Scene::new();
            let x = scene.add(SceneEntity::new("x", text("x")).tag("t"));
            let y = scene.add(SceneEntity::new("y", text("y")).tag("t"));
            (scene.tag("t"), x, y, scene.id_of("y"))
        };
        let (t1, x1, y1, yl1) = build();
        let (t2, x2, y2, yl2) = build();
        assert_eq!((t1, x1, y1, yl1), (t2, x2, y2, yl2));
    }

    #[test]
    fn unknown_targets_resolve_to_empty_without_panicking() {
        let scene = Scene::new();
        assert!(scene.resolve(SceneTarget::Id(SceneId(999))).is_empty());
        assert!(scene.resolve(SceneTarget::Tag(TagId(999))).is_empty());
        assert_eq!(scene.id_of("nope"), None);
    }

    #[test]
    fn identity_preserves_the_scene() {
        let mut scene = Scene::new();
        let a = scene.add(SceneEntity::new("a", text("a")).offset(2, 3));
        let mut before = Presentation::new();
        Effect::identity().eval(Duration::from_millis(10), &scene, &mut before);
        assert_eq!(before.offset_of(&scene, a), (2, 3));
        assert_eq!(before.raw_offset(a), None, "identity must not write");
    }

    #[test]
    fn translate_reaches_endpoints_and_rounds_to_cells() {
        let mut scene = Scene::new();
        let a = scene.add(SceneEntity::new("a", text("a")));
        let e = Effect::translate(
            SceneTarget::Id(a),
            (0.0, 0.0),
            (10.0, 5.0),
            Duration::from_millis(100),
        );
        let mut p = Presentation::new();
        e.eval(Duration::ZERO, &scene, &mut p);
        assert_eq!(p.offset_of(&scene, a), (0, 0));
        e.eval(Duration::from_millis(50), &scene, &mut p);
        assert_eq!(p.offset_of(&scene, a), (5, 3)); // 2.5 rounds to 3
        e.eval(Duration::from_millis(100), &scene, &mut p);
        assert_eq!(p.offset_of(&scene, a), (10, 5));
    }

    #[test]
    fn sequence_matches_manual_two_step_composition() {
        let mut scene = Scene::new();
        let a = scene.add(SceneEntity::new("a", text("a")));
        let t = SceneTarget::Id(a);
        let a_eff = Effect::translate(t, (0.0, 0.0), (6.0, 0.0), Duration::from_millis(100));
        let b_eff = Effect::translate(t, (6.0, 0.0), (6.0, 4.0), Duration::from_millis(100));
        let seq = Effect::sequence([a_eff.clone(), b_eff.clone()]);

        // Checkpoints across the composed timeline.
        for (ms, expect) in [
            (0u64, (0, 0)),
            (50, (3, 0)),
            (100, (6, 0)),
            (150, (6, 2)),
            (200, (6, 4)),
        ] {
            let mut p = Presentation::new();
            seq.eval(Duration::from_millis(ms), &scene, &mut p);
            assert_eq!(p.offset_of(&scene, a), expect, "at t={ms}ms");

            // Manual equivalent: apply A for the first window, B afterwards.
            let mut m = Presentation::new();
            if ms <= 100 {
                a_eff.eval(Duration::from_millis(ms), &scene, &mut m);
            } else {
                a_eff.eval(Duration::from_millis(100), &scene, &mut m);
                b_eff.eval(Duration::from_millis(ms - 100), &scene, &mut m);
            }
            assert_eq!(
                p.offset_of(&scene, a),
                m.offset_of(&scene, a),
                "sequence must equal manual composition at t={ms}ms"
            );
        }
    }

    #[test]
    fn sequence_is_associative_at_checkpoints() {
        let mut scene = Scene::new();
        let a = scene.add(SceneEntity::new("a", text("a")));
        let t = SceneTarget::Id(a);
        let e1 = Effect::translate(t, (0.0, 0.0), (3.0, 0.0), Duration::from_millis(100));
        let e2 = Effect::translate(t, (3.0, 0.0), (3.0, 3.0), Duration::from_millis(100));
        let e3 = Effect::translate(t, (3.0, 3.0), (1.0, 1.0), Duration::from_millis(100));

        let left = Effect::sequence([Effect::sequence([e1.clone(), e2.clone()]), e3.clone()]);
        let right = Effect::sequence([e1.clone(), Effect::sequence([e2.clone(), e3.clone()])]);
        for ms in (0..=300).step_by(25) {
            let mut pl = Presentation::new();
            let mut pr = Presentation::new();
            left.eval(Duration::from_millis(ms), &scene, &mut pl);
            right.eval(Duration::from_millis(ms), &scene, &mut pr);
            assert_eq!(
                pl.offset_of(&scene, a),
                pr.offset_of(&scene, a),
                "((e1;e2);e3) must equal (e1;(e2;e3)) at {ms}ms"
            );
        }
    }

    #[test]
    fn parallel_on_independent_targets_is_order_independent() {
        let mut scene = Scene::new();
        let a = scene.add(SceneEntity::new("a", text("a")));
        let b = scene.add(SceneEntity::new("b", text("b")));
        let ta = Effect::translate(
            SceneTarget::Id(a),
            (0.0, 0.0),
            (8.0, 0.0),
            Duration::from_millis(100),
        );
        let tb = Effect::translate(
            SceneTarget::Id(b),
            (0.0, 0.0),
            (0.0, 8.0),
            Duration::from_millis(100),
        );

        let ab = Effect::parallel([ta.clone(), tb.clone()]);
        let ba = Effect::parallel([tb, ta]);
        for ms in (0..=100).step_by(20) {
            let mut p1 = Presentation::new();
            let mut p2 = Presentation::new();
            ab.eval(Duration::from_millis(ms), &scene, &mut p1);
            ba.eval(Duration::from_millis(ms), &scene, &mut p2);
            assert_eq!(p1.offset_of(&scene, a), p2.offset_of(&scene, a));
            assert_eq!(p1.offset_of(&scene, b), p2.offset_of(&scene, b));
        }
    }

    #[test]
    fn parallel_same_channel_uses_explicit_write_order() {
        // Commutativity is deliberately NOT assumed. The later child wins.
        let mut scene = Scene::new();
        let a = scene.add(SceneEntity::new("a", text("a")));
        let first = Effect::set_custom(SceneTarget::Id(a), 1, 1.0);
        let second = Effect::set_custom(SceneTarget::Id(a), 1, 2.0);
        let mut p = Presentation::new();
        Effect::parallel([first.clone(), second.clone()]).eval(Duration::ZERO, &scene, &mut p);
        assert_eq!(p.custom(a, 1), Some(2.0));
        let mut q = Presentation::new();
        Effect::parallel([second, first]).eval(Duration::ZERO, &scene, &mut q);
        assert_eq!(q.custom(a, 1), Some(1.0));
    }

    #[test]
    fn delay_repeat_and_reverse_behave() {
        let mut scene = Scene::new();
        let a = scene.add(SceneEntity::new("a", text("a")));
        let t = SceneTarget::Id(a);
        let e = Effect::translate(t, (0.0, 0.0), (10.0, 0.0), Duration::from_millis(100));

        let delayed = Effect::Delay(Duration::from_millis(50), Box::new(e.clone()));
        assert_eq!(delayed.duration(), Duration::from_millis(150));
        let mut p = Presentation::new();
        delayed.eval(Duration::from_millis(50), &scene, &mut p);
        assert_eq!(p.offset_of(&scene, a), (0, 0));
        delayed.eval(Duration::from_millis(100), &scene, &mut p);
        assert_eq!(p.offset_of(&scene, a), (5, 0));

        let rep = Effect::Repeat(Box::new(e.clone()), 3);
        assert_eq!(rep.duration(), Duration::from_millis(300));
        let mut p2 = Presentation::new();
        rep.eval(Duration::from_millis(120), &scene, &mut p2);
        assert_eq!(p2.offset_of(&scene, a), (2, 0)); // 20ms into 2nd iteration

        let rev = Effect::Reverse(Box::new(e));
        let mut p3 = Presentation::new();
        rev.eval(Duration::from_millis(25), &scene, &mut p3);
        assert_eq!(p3.offset_of(&scene, a), (8, 0)); // 75ms forwards
    }

    #[test]
    fn reveal_controls_visibility_and_to_node_omits_hidden() {
        let mut scene = Scene::new();
        let a = scene.add(SceneEntity::new("a", text("a")).hidden());
        let vis = Effect::reveal(SceneTarget::Id(a), 0.0, 1.0, Duration::from_millis(100));
        let mut p = Presentation::new();
        vis.eval(Duration::ZERO, &scene, &mut p);
        let resolved = scene.evaluate(&p);
        assert!(!resolved[0].visible, "baseline hidden and ramped from 0");
        vis.eval(Duration::from_millis(100), &scene, &mut p);
        let resolved = scene.evaluate(&p);
        assert!(resolved[0].visible);
        // The functor omits hidden entities.
        let node_hidden = scene.to_node(&Presentation::new(), 20.0, 5.0);
        assert!(node_hidden.children.is_empty());
        let node_shown = scene.to_node(&p, 20.0, 5.0);
        assert_eq!(node_shown.children.len(), 1);
    }

    #[test]
    fn camera_pan_wraps_scene_in_a_viewport() {
        let mut scene = Scene::new();
        let a = scene.add(SceneEntity::new("a", text("a")));
        let e = Effect::camera_pan((0.0, 0.0), (4.0, 2.0), Duration::from_millis(100));
        let mut p = Presentation::new();
        e.eval(Duration::from_millis(100), &scene, &mut p);
        assert_eq!(p.camera(), (4, 2));
        let node = scene.to_node(&p, 20.0, 5.0);
        assert!(matches!(node.kind, crate::node::NodeKind::Viewport { .. }));
        let _ = a;
    }

    #[test]
    fn evaluation_is_deterministic_and_z_sorted() {
        let mut scene = Scene::new();
        let top = scene.add(SceneEntity::new("top", text("t")).z(10));
        let bottom = scene.add(SceneEntity::new("bottom", text("b")).z(-5));
        let resolved = scene.evaluate(&Presentation::new());
        assert_eq!(resolved[0].id, bottom);
        assert_eq!(resolved[1].id, top);
        // Same inputs → same output twice.
        let a = scene.to_node(&Presentation::new(), 10.0, 3.0);
        let b = scene.to_node(&Presentation::new(), 10.0, 3.0);
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
    }
}
