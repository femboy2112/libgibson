//! Contract tests for the cinematic showcase's Observatory scenes: every scene
//! renders deterministically at exactly the requested size, lands real plot
//! geometry, and survives tiny sizes and hostile local times without panicking.

#[path = "../examples/cinematic_showcase/observatory.rs"]
mod observatory;
#[path = "../examples/cinematic_showcase/shot.rs"]
mod shot;

use gibson::capability::ColorDepth;
use gibson::Surface;
use observatory::render;
use shot::SceneId;

const DEPTHS: [ColorDepth; 2] = [ColorDepth::Mono, ColorDepth::TrueColor];

#[test]
fn surface_is_exactly_w_by_h() {
    for scene in SceneId::ALL {
        for depth in DEPTHS {
            for local in [0.0_f32, 1.5, 3.0, 6.0, 8.0] {
                for (w, h) in [(80_u16, 24_u16), (120, 40), (40, 12)] {
                    let s = render(scene, local, w, h, depth);
                    assert_eq!((s.width, s.height), (w, h), "{scene:?} @ {local}");
                    assert_eq!(s.cells.len(), w as usize * h as usize);
                }
            }
        }
    }
}

#[test]
fn render_is_deterministic() {
    for scene in SceneId::ALL {
        for depth in DEPTHS {
            for local in [0.0_f32, 0.7, 3.3, 7.9] {
                let a = render(scene, local, 100, 30, depth);
                let b = render(scene, local, 100, 30, depth);
                assert_eq!(a, b, "{scene:?} @ {local} not deterministic");
            }
        }
    }
}

#[test]
fn scenes_are_non_blank_mid_animation() {
    for scene in SceneId::ALL {
        for depth in DEPTHS {
            let s = render(scene, 4.0, 100, 30, depth);
            assert_ne!(s, Surface::new(100, 30), "{scene:?} drew nothing");
        }
    }
}

#[test]
fn scenes_actually_animate() {
    for scene in SceneId::ALL {
        let a = render(scene, 1.0, 100, 30, ColorDepth::TrueColor);
        let b = render(scene, 4.0, 100, 30, ColorDepth::TrueColor);
        assert_ne!(a, b, "{scene:?} is static between local=1 and local=4");
    }
}

#[test]
fn tiny_sizes_and_hostile_times_do_not_panic() {
    let locals = [
        0.0_f32,
        -1.0,
        -1.0e9,
        1.0e9,
        f32::MAX,
        f32::MIN,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ];
    let sizes = [(0_u16, 0_u16), (1, 1), (2, 2), (3, 30), (80, 1), (5, 5)];
    for scene in SceneId::ALL {
        for depth in DEPTHS {
            for &local in &locals {
                for &(w, h) in &sizes {
                    let s = render(scene, local, w, h, depth);
                    assert_eq!((s.width, s.height), (w, h));
                }
                // Hostile time at a normal size must still produce a full surface.
                let s = render(scene, local, 80, 24, depth);
                assert_eq!((s.width, s.height), (80, 24));
            }
        }
    }
}
