//! Acceptance for the experimental grid raycaster (consumer case #9 in
//! docs/FACADE_SIGNAL_ACCEPTANCE.md): determinism, bounded work, closed seeded maze.
use gibson::raster::{Rgb, RgbRaster};
use gibson::raycast::{render, GridMap, RayStats, RayView, Texels};
use std::collections::VecDeque;

struct Flat;
impl Texels for Flat {
    fn wall(&self, material: u8, u: f64, v: f64, _d: f64) -> Rgb {
        (
            material.wrapping_mul(60),
            (u * 255.0) as u8,
            (v * 255.0) as u8,
        )
    }
    fn floor(&self, _x: f64, _y: f64, _d: f64) -> Rgb {
        (0, 0, 40)
    }
    fn ceiling(&self, _x: f64, _y: f64, _d: f64) -> Rgb {
        (40, 0, 0)
    }
}

fn hash(r: &RgbRaster) -> u64 {
    r.pixels().iter().fold(0xcbf2_9ce4_8422_2325u64, |h, p| {
        [p.0, p.1, p.2]
            .iter()
            .fold(h, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3))
    })
}

fn frame(map: &GridMap, view: &RayView, w: u16, h: u16) -> (RgbRaster, RayStats) {
    let mut r = RgbRaster::new(w, h);
    let s = render(map, view, &Flat, &mut r);
    (r, s)
}

/// View for (seed, t): a deterministic walk around the start cell.
fn view_at(t: f64) -> RayView {
    RayView {
        x: 1.5,
        y: 1.5,
        angle: t * 0.7,
        ..RayView::default()
    }
}

#[test]
fn seeded_maze_is_deterministic_closed_and_fully_reachable() {
    for seed in [0u64, 1, 42, 0xDEAD_BEEF, u64::MAX] {
        let a = GridMap::maze(31, 21, seed);
        assert_eq!(a, GridMap::maze(31, 21, seed), "same seed, same maze");
        let (w, h) = (a.width() as i64, a.height() as i64);
        for x in 0..w {
            assert_ne!(a.get(x, 0), 0);
            assert_ne!(a.get(x, h - 1), 0);
        }
        for y in 0..h {
            assert_ne!(a.get(0, y), 0);
            assert_ne!(a.get(w - 1, y), 0);
        }
        assert_eq!(a.get(1, 1), 0, "start cell is open");
        // Every open cell is reachable from the start.
        let mut seen = vec![false; (w * h) as usize];
        let mut queue = VecDeque::from([(1i64, 1i64)]);
        seen[(w + 1) as usize] = true;
        while let Some((x, y)) = queue.pop_front() {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                let i = (ny * w + nx) as usize;
                if a.get(nx, ny) == 0 && !seen[i] {
                    seen[i] = true;
                    queue.push_back((nx, ny));
                }
            }
        }
        let open = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&(x, y)| a.get(x, y) == 0)
            .count();
        assert_eq!(seen.iter().filter(|s| **s).count(), open, "seed {seed}");
        assert!(open > 50, "a real maze, not a sealed start cell");
    }
    assert_ne!(GridMap::maze(31, 21, 1), GridMap::maze(31, 21, 2));
}

#[test]
fn render_is_deterministic_for_seed_and_t() {
    let map = GridMap::maze(31, 21, 7);
    for t in [0.0, 0.25, 1.0, 9.5] {
        let (a, sa) = frame(&map, &view_at(t), 96, 48);
        let (b, sb) = frame(&map, &view_at(t), 96, 48);
        assert_eq!(hash(&a), hash(&b), "t={t}");
        assert_eq!(sa, sb);
    }
    let (a, _) = frame(&map, &view_at(0.0), 96, 48);
    let (b, _) = frame(&map, &view_at(1.0), 96, 48);
    assert_ne!(
        hash(&a),
        hash(&b),
        "control: the view actually changes with t"
    );
}

#[test]
fn work_is_bounded_by_the_raster_and_max_steps_not_the_map() {
    for (w, h) in [(1u16, 1u16), (64, 32), (200, 120)] {
        for map in [GridMap::maze(255, 255, 3), GridMap::new(3, 3)] {
            let view = RayView {
                max_steps: 40,
                ..view_at(0.3)
            };
            let (_, s) = frame(&map, &view, w, h);
            let (w64, h64) = (u64::from(w), u64::from(h));
            assert_eq!(s.columns, u32::from(w));
            assert!(s.pixels_written >= w64 * h64, "every pixel written");
            assert!(s.pixels_written <= 2 * w64 * h64);
            assert!(s.grid_steps <= w64 * 40);
        }
    }
}

#[test]
fn rays_never_leave_the_grid() {
    // An all-open 3×3 map has no interior walls: every ray must stop at the
    // OUTSIDE wall within a few steps, never miss.
    let map = GridMap::new(3, 3);
    for i in 0..16 {
        let (_, s) = frame(
            &map,
            &RayView {
                angle: f64::from(i) * 0.4,
                ..view_at(0.0)
            },
            80,
            30,
        );
        assert_eq!(s.misses, 0, "angle step {i}");
        assert!(s.grid_steps <= 80 * 4);
    }
}

#[test]
fn hostile_views_render_nothing() {
    let map = GridMap::maze(15, 15, 1);
    for bad in [
        RayView {
            x: f64::NAN,
            ..view_at(0.0)
        },
        RayView {
            angle: f64::INFINITY,
            ..view_at(0.0)
        },
        RayView {
            plane: 0.0,
            ..view_at(0.0)
        },
        RayView {
            plane: -1.0,
            ..view_at(0.0)
        },
    ] {
        let (r, s) = frame(&map, &bad, 32, 16);
        assert_eq!(s, RayStats::default());
        assert!(r.pixels().iter().all(|p| *p == (0, 0, 0)));
    }
    let (_, s) = frame(&map, &view_at(0.0), 0, 0);
    assert_eq!(s, RayStats::default());
}

#[test]
fn from_rows_validates_shape() {
    assert!(GridMap::from_rows(&[]).is_none());
    assert!(GridMap::from_rows(&[&[1, 1], &[1]]).is_none());
    let m = GridMap::from_rows(&[&[1, 1, 1], &[1, 0, 1], &[1, 1, 1]]).unwrap();
    assert_eq!(
        (m.width(), m.height(), m.get(1, 1), m.get(9, 9)),
        (3, 3, 0, GridMap::OUTSIDE)
    );
}

/// Records every texture coordinate the renderer asks for.
struct Probe {
    seen: std::cell::RefCell<Vec<(f64, f64)>>,
}
impl Texels for Probe {
    fn wall(&self, _m: u8, u: f64, v: f64, _d: f64) -> Rgb {
        self.seen.borrow_mut().push((u, v));
        ((u * 255.0) as u8, 0, 0)
    }
    fn floor(&self, _x: f64, _y: f64, _d: f64) -> Rgb {
        (0, 0, 0)
    }
    fn ceiling(&self, _x: f64, _y: f64, _d: f64) -> Rgb {
        (0, 0, 0)
    }
}

#[test]
fn wall_textures_read_left_to_right_on_every_face() {
    // From the centre of an open 9×9 room, look at the facing wall: its u must grow from
    // left to right for each of the four headings.
    let map = GridMap::new(9, 9);
    for (i, angle) in [0.0, 0.5, 1.0, 1.5].iter().enumerate() {
        let view = RayView {
            x: 4.5,
            y: 4.5,
            angle: angle * std::f64::consts::PI,
            ..RayView::default()
        };
        let probe = Probe {
            seen: Default::default(),
        };
        let mut r2 = RgbRaster::new(32, 16);
        render(&map, &view, &probe, &mut r2);
        // `u` restarts at every cell boundary, so look within the single wall cell straight
        // ahead: at 4.5 cells with plane 0.66 it spans columns 14..=18 of 32.
        let row: Vec<u8> = (14..=18).map(|x| r2.get(x, 8).unwrap().0).collect();
        assert!(row.windows(2).all(|w| w[0] <= w[1]), "heading {i}: {row:?}");
        assert!(row.first() < row.last(), "heading {i}: u actually varies");
    }
}

#[test]
fn texture_coordinates_stay_in_the_half_open_unit_interval() {
    let probe = Probe {
        seen: Default::default(),
    };
    let map = GridMap::new(6, 6);
    let mut r = RgbRaster::new(1, 1);
    // The exact case that produced u == 1.0 before the fix.
    render(
        &map,
        &RayView {
            x: 2.0,
            y: 1.0,
            angle: 5.0 * std::f64::consts::PI / 4.0,
            ..RayView::default()
        },
        &probe,
        &mut r,
    );
    let maze = GridMap::maze(15, 15, 3);
    let mut r = RgbRaster::new(48, 24);
    for i in 0..400 {
        let f = f64::from(i);
        let view = RayView {
            x: 1.0 + (f * 0.37) % 13.0,
            y: 1.0 + (f * 0.53) % 13.0,
            angle: f * 0.25,
            ..RayView::default()
        };
        render(&maze, &view, &probe, &mut r);
        render(
            &map,
            &RayView {
                x: 1.0 + (i % 5) as f64,
                y: 1.0 + (i / 5 % 5) as f64,
                ..view
            },
            &probe,
            &mut r,
        );
    }
    let seen = probe.seen.borrow();
    assert!(seen.len() > 1000);
    for &(u, v) in seen.iter() {
        assert!(
            (0.0..1.0).contains(&u) && (0.0..1.0).contains(&v),
            "u={u} v={v}"
        );
    }
}

#[test]
fn extreme_but_finite_views_are_refused_not_panicking() {
    let map = GridMap::maze(15, 15, 1);
    for bad in [
        RayView {
            x: 1e19,
            ..view_at(0.0)
        },
        RayView {
            x: -1e19,
            angle: std::f64::consts::PI,
            ..view_at(0.0)
        },
        RayView {
            y: 2e9,
            ..view_at(0.0)
        },
        RayView {
            plane: 1e-310,
            ..view_at(0.0)
        },
        RayView {
            plane: 1e300,
            ..view_at(0.0)
        },
    ] {
        let (_, s) = frame(&map, &bad, 16, 8);
        assert_eq!(s, RayStats::default(), "{bad:?}");
    }
    // A viewer far outside a small map, but within MAX_COORD, still renders safely.
    let (_, s) = frame(
        &map,
        &RayView {
            x: 9.0e8,
            y: -9.0e8,
            ..view_at(0.0)
        },
        16,
        8,
    );
    assert_eq!(s.columns, 16);
}
