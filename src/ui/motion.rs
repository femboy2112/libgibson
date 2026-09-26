//! Deterministic, finite semantic motion lowered into existing surface effects.
//!
//! A plan owns no clock and never loops implicitly. The empty effect chain is
//! the settled visual state. For Exit/ModalExit, the owner removes the outgoing
//! object at the deadline; plans do not retain trees or create exit ghosts.

use std::time::Duration;

use crate::{clock, surface_fx::FxMask, Style, SurfaceFx};

/// User preference, applied before any effect is constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MotionPreference {
    #[default]
    Full,
    /// Brief emphasis only: no displacement, noise, or hidden information.
    Reduced,
    /// Immediate settled appearance, with no effect allocation.
    None,
}

/// The presentation meaning of a transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionRole {
    Enter,
    Exit,
    Focus,
    Blur,
    Activate,
    Success,
    Warning,
    Error,
    /// An explicit finite pulse, not an implicitly repeating animation.
    Busy,
    ModalEnter,
    ModalExit,
    Change,
}

/// A skin's motion grammar; every grammar uses existing SurfaceFx operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionLanguage {
    Physical,
    Acquisition,
    Editorial,
}

/// Editable timing and motion preferences belonging to a skin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionTokens {
    pub language: MotionLanguage,
    pub enter: Duration,
    pub exit: Duration,
    pub focus: Duration,
    pub activate: Duration,
    pub signal: Duration,
    pub modal: Duration,
}

impl MotionTokens {
    pub fn resolve(
        self,
        role: MotionRole,
        preference: MotionPreference,
        style: Style,
    ) -> MotionPlan {
        let duration = match role {
            MotionRole::Enter | MotionRole::Change => self.enter,
            MotionRole::Exit | MotionRole::ModalExit => self.exit,
            MotionRole::Focus | MotionRole::Blur => self.focus,
            MotionRole::Activate => self.activate,
            MotionRole::ModalEnter => self.modal,
            _ => self.signal,
        };
        MotionPlan {
            role,
            duration: match preference {
                MotionPreference::Full => duration,
                MotionPreference::Reduced => duration.min(Duration::from_millis(80)),
                MotionPreference::None => Duration::ZERO,
            },
            language: self.language,
            preference,
            style,
        }
    }
}

/// A pure function from elapsed time to an ordinary surface-effect chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionPlan {
    pub role: MotionRole,
    pub duration: Duration,
    pub language: MotionLanguage,
    pub preference: MotionPreference,
    pub style: Style,
}

impl MotionPlan {
    pub fn is_settled(&self, elapsed: Duration) -> bool {
        self.preference == MotionPreference::None || elapsed >= self.duration
    }

    /// No wall clock, mutable RNG, allocation history, or frame counter is read.
    /// At and after the deadline this returns exactly `[]`.
    pub fn effects(&self, elapsed: Duration) -> Vec<SurfaceFx> {
        if self.is_settled(elapsed) {
            return Vec::new();
        }
        if self.preference == MotionPreference::Reduced {
            return vec![SurfaceFx::StyleOverlay(self.style)];
        }
        let t = (elapsed.as_secs_f64() / self.duration.as_secs_f64()) as f32;
        let eased = clock::ease_out(t);
        let exiting = matches!(self.role, MotionRole::Exit | MotionRole::ModalExit);
        if matches!(
            self.role,
            MotionRole::Enter | MotionRole::ModalEnter | MotionRole::Exit | MotionRole::ModalExit
        ) {
            let revealed = if exiting { 1.0 - eased } else { eased };
            return match self.language {
                MotionLanguage::Physical => vec![
                    SurfaceFx::RowShift {
                        amount: ((1.0 - revealed) * 2.0).round() as i32,
                        seed: 0,
                    },
                    hide_prefix(FxMask::HorizontalWipe {
                        fraction: 1.0 - revealed,
                    }),
                ],
                MotionLanguage::Acquisition => vec![
                    SurfaceFx::Dissolve {
                        seed: 0x0049_4345,
                        fraction: revealed,
                    },
                    SurfaceFx::Scanline {
                        position: eased,
                        style: self.style,
                    },
                ],
                MotionLanguage::Editorial => vec![hide_prefix(FxMask::VerticalWipe {
                    fraction: 1.0 - revealed,
                })],
            };
        }
        match (self.language, self.role) {
            (MotionLanguage::Physical, MotionRole::Activate) => vec![SurfaceFx::Reverse],
            (MotionLanguage::Acquisition, MotionRole::Error) => vec![
                // One short, deterministic displacement, only for an error.
                SurfaceFx::RowShift {
                    amount: if t < 0.3 { 1 } else { 0 },
                    seed: 0,
                },
                SurfaceFx::StyleOverlay(self.style),
            ],
            (MotionLanguage::Acquisition, MotionRole::Focus | MotionRole::Busy) => {
                vec![SurfaceFx::StyleMask {
                    style: self.style,
                    seed: 0x0046_4f43_5553,
                    fraction: 1.0 - eased,
                }]
            }
            (MotionLanguage::Editorial, MotionRole::Focus | MotionRole::Change) => {
                vec![SurfaceFx::StyleOverlay(self.style).scoped(FxMask::Band {
                    position: 1.0,
                    width: (1.0 - eased) * 0.25,
                })]
            }
            (_, MotionRole::Blur) => vec![SurfaceFx::Dim],
            _ => vec![SurfaceFx::StyleOverlay(self.style)],
        }
    }
}

fn hide_prefix(mask: FxMask) -> SurfaceFx {
    SurfaceFx::Dissolve {
        seed: 0,
        fraction: 0.0,
    }
    .scoped(mask)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{skin::skins, UiEnvironment};

    #[test]
    fn plans_are_deterministic_finite_and_exactly_identity_at_deadline() {
        for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
            let resolved = skin.resolve(&UiEnvironment::default());
            for role in [
                MotionRole::Enter,
                MotionRole::Exit,
                MotionRole::Focus,
                MotionRole::Blur,
                MotionRole::Activate,
                MotionRole::Success,
                MotionRole::Warning,
                MotionRole::Error,
                MotionRole::Busy,
                MotionRole::ModalEnter,
                MotionRole::ModalExit,
                MotionRole::Change,
            ] {
                let plan = resolved.motion(role);
                let half = plan.duration / 2;
                assert_eq!(plan.effects(half), plan.effects(half));
                assert!(plan.effects(plan.duration).is_empty());
                assert!(plan.effects(Duration::MAX).is_empty());
            }
        }
    }

    #[test]
    fn none_is_immediate_and_reduced_never_moves_or_hides() {
        for preference in [MotionPreference::None, MotionPreference::Reduced] {
            let skin = skins::BLACK_ICE.resolve(&UiEnvironment {
                motion: preference,
                ..UiEnvironment::default()
            });
            let plan = skin.motion(MotionRole::ModalEnter);
            if preference == MotionPreference::None {
                assert_eq!(plan.duration, Duration::ZERO);
                assert!(plan.effects(Duration::ZERO).is_empty());
            } else {
                assert!(plan.duration <= Duration::from_millis(80));
                assert!(plan
                    .effects(Duration::ZERO)
                    .iter()
                    .all(|fx| { matches!(fx, SurfaceFx::StyleOverlay(_)) }));
            }
        }
    }
}
