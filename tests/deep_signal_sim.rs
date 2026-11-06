//! Deep Signal (v0.7 node E) — the simulation's laws, checked without rendering.
#![allow(dead_code)]
#[path = "../examples/deep_signal/world.rs"]
mod world;

use std::collections::VecDeque;
use world::{
    cos_q16, sin_q16, Intent, Phase, Rng, Tile, World, LOST_TICS, ONE, TRANSITION_TICS, TURN,
};

#[test]
fn the_sine_table_is_a_sine_table() {
    assert_eq!(sin_q16(0), 0);
    assert_eq!(sin_q16(TURN / 4), 65536);
    assert_eq!(sin_q16(TURN / 2), 0);
    assert_eq!(sin_q16(3 * TURN / 4), -65536);
    for a in 0..TURN {
        let (s, c) = (i64::from(sin_q16(a)), i64::from(cos_q16(a)));
        let norm = s * s + c * c;
        let one = 65536i64 * 65536;
        assert!(
            (norm - one).abs() < one / 20_000,
            "angle {a}: |v|^2 = {norm}"
        );
        assert_eq!(sin_q16(a), -sin_q16(-a), "odd at {a}");
    }
    for a in 1..=TURN / 4 {
        assert!(
            sin_q16(a) >= sin_q16(a - 1),
            "monotone in the first quadrant at {a}"
        );
    }
}

/// Independent BFS (not the generator's helper): tiles reachable from `from` while the uplink is
/// SHUT, i.e. treating `Tile::Exit` like a wall.
fn reachable(map: &world::Map, from: (i32, i32)) -> Vec<bool> {
    let mut seen = vec![false; (map.w * map.h) as usize];
    let mut q = VecDeque::from([from]);
    seen[(from.1 * map.w + from.0) as usize] = true;
    while let Some((x, y)) = q.pop_front() {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            let walkable = matches!(map.at(nx, ny), Tile::Floor | Tile::Hazard);
            if walkable && !seen[(ny * map.w + nx) as usize] {
                seen[(ny * map.w + nx) as usize] = true;
                q.push_back((nx, ny));
            }
        }
    }
    seen
}

/// Collects every shard and enters the uplink by placing the player on each tile in turn,
/// then runs the descent; returns once the next level is live.
fn clear_level(w: &mut World) {
    for (sx, sy) in w.shards.clone() {
        w.x = sx * ONE + ONE / 2;
        w.y = sy * ONE + ONE / 2;
        w.step(Intent::default());
    }
    w.x = (w.map.w - 2) * ONE + ONE / 2;
    w.y = (w.map.h - 2) * ONE + ONE / 2;
    w.step(Intent::default());
    for _ in 0..TRANSITION_TICS {
        w.step(Intent::default());
    }
}

#[test]
fn every_level_is_solvable_from_the_start() {
    for seed in (0..400u64).chain([7, 2112]) {
        let mut w = World::new(seed, 3);
        for level in 1..=3 {
            assert_eq!((w.level, w.phase), (level, Phase::Playing), "seed {seed}");
            let seen = reachable(&w.map, (1, 1));
            for &(sx, sy) in &w.shards {
                let i = (sy * w.map.w + sx) as usize;
                assert!(
                    seen[i],
                    "seed {seed} level {level}: shard ({sx},{sy}) unreachable"
                );
            }
            let (ex, ey) = (w.map.w - 2, w.map.h - 2);
            assert_eq!(w.map.at(ex, ey), Tile::Exit);
            // The (shut) exit must border a tile reachable while it is shut.
            let beside = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|&(dx, dy)| seen[((ey + dy) * w.map.w + ex + dx) as usize]);
            assert!(beside, "seed {seed} level {level}: exit unreachable");
            if level < 3 {
                clear_level(&mut w);
            }
        }
    }
}

fn corners_clear(w: &World) -> bool {
    let r = 220;
    [(-r, -r), (r, -r), (-r, r), (r, r)]
        .iter()
        .all(|&(dx, dy)| {
            !w.map
                .at((w.x + dx).div_euclid(ONE), (w.y + dy).div_euclid(ONE))
                .blocks(w.exit_open())
        })
}

#[test]
fn collision_never_lets_the_player_into_a_wall() {
    for seed in 0..12u64 {
        let mut w = World::new(seed, 3);
        let mut rng = Rng::new(seed + 99);
        let mut intent = Intent::default();
        for t in 0..4000 {
            if t % 17 == 0 {
                let pick = |r: &mut Rng| r.below(3) as i32 - 1;
                intent = Intent {
                    forward: pick(&mut rng),
                    strafe: pick(&mut rng),
                    turn: pick(&mut rng),
                };
            }
            w.step(intent);
            if w.phase == Phase::Playing {
                assert!(
                    corners_clear(&w),
                    "seed {seed} tic {}: inside a wall at ({}, {})",
                    w.tic,
                    w.x,
                    w.y
                );
            }
        }
    }
}

#[test]
fn the_same_intents_replay_to_the_same_state() {
    let run = |seed: u64| {
        let mut w = World::new(seed, 3);
        let mut rng = Rng::new(7);
        for _ in 0..3000 {
            let pick = |r: &mut Rng| r.below(3) as i32 - 1;
            let i = Intent {
                forward: pick(&mut rng),
                strafe: pick(&mut rng),
                turn: pick(&mut rng),
            };
            w.step(i);
        }
        w.digest()
    };
    assert_eq!(run(5), run(5));
    assert_ne!(run(5), run(6), "different seeds build different worlds");
}

#[test]
fn the_uplink_stays_shut_until_every_shard_is_recovered() {
    let mut w = World::new(11, 2);
    let (ex, ey) = (w.map.w - 2, w.map.h - 2);
    assert!(w.map.at(ex, ey).blocks(w.exit_open()));
    let shards = w.shards.clone();
    for (i, &(sx, sy)) in shards.iter().enumerate() {
        w.x = sx * ONE + ONE / 2;
        w.y = sy * ONE + ONE / 2;
        let happened = w.step(Intent::default());
        assert_eq!(w.shards.len(), shards.len() - i - 1, "{happened:?}");
    }
    assert!(w.exit_open());
    assert!(!w.map.at(ex, ey).blocks(true));
    // Standing in the open uplink starts the descent; the level advances after the transition.
    w.x = ex * ONE + ONE / 2;
    w.y = ey * ONE + ONE / 2;
    w.step(Intent::default());
    let since = match w.phase {
        Phase::Descending { since } => since,
        p => panic!("expected a descent, got {p:?}"),
    };
    for _ in 0..TRANSITION_TICS {
        w.step(Intent {
            forward: 1,
            strafe: 0,
            turn: 1,
        });
    }
    assert_eq!(w.level, 2, "descent began at {since}");
    assert_eq!(w.phase, Phase::Playing);
    assert_eq!(w.shards.len() as u32, w.shards_total);
}

#[test]
fn live_hazards_drain_integrity_and_a_lost_signal_reboots_the_sector() {
    let mut w = World::new(3, 2);
    let (hx, hy) = (0..w.map.h)
        .flat_map(|y| (0..w.map.w).map(move |x| (x, y)))
        .find(|&(x, y)| w.map.at(x, y) == Tile::Hazard)
        .expect("levels carry hazards");
    w.x = hx * ONE + ONE / 2;
    w.y = hy * ONE + ONE / 2;
    let mut drained = 0;
    let mut steps = 0;
    while w.phase == Phase::Playing && steps < 20_000 {
        let before = w.integrity;
        let live = {
            let mut next = w.clone();
            next.tic += 1;
            next.hazard_live()
        };
        w.step(Intent::default());
        if live {
            assert!(w.integrity < before || w.integrity == 0);
            drained += 1;
        } else {
            assert_eq!(w.integrity, before, "dormant hazards do no damage");
        }
        steps += 1;
    }
    assert!(
        matches!(w.phase, Phase::SignalLost { .. }),
        "integrity ran out"
    );
    assert!(drained > 0);
    let level = w.level;
    for _ in 0..LOST_TICS {
        w.step(Intent::default());
    }
    assert_eq!(w.phase, Phase::Playing);
    assert_eq!(w.level, level, "a lost signal restarts the same sector");
    assert_eq!(w.integrity, World::max_integrity());
}
