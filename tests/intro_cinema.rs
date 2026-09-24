//! Cinematic correctness witnesses, not substitutes for human art direction.
#[allow(dead_code)]
#[path = "../examples/libgibson_intro.rs"]
mod intro;

use gibson::{compute_diff, AnsiCompiler, ColorDepth, Surface};
use intro::director::{Act, Director, CUES};
use intro::model::{JobStatus, AGENTS, MESSAGES};
use std::collections::HashSet;

fn at(seconds: f32) -> Director {
    let mut d = Director::default();
    d.seek(seconds);
    d
}
fn text(surface: &Surface) -> String {
    (0..surface.height)
        .map(|y| {
            (0..surface.width)
                .map(|x| surface.get(x, y).unwrap().glyph.grapheme.as_str())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn wide_valid(surface: &Surface) {
    assert_eq!(
        surface.cells.len(),
        usize::from(surface.width) * usize::from(surface.height)
    );
    for y in 0..surface.height {
        for x in 0..surface.width {
            let cell = surface.get(x, y).unwrap();
            if cell.is_continuation {
                assert!(x > 0);
                let lead = surface.get(x - 1, y).unwrap();
                assert!(!lead.is_continuation && lead.glyph.display_width == 2);
            } else if cell.glyph.display_width == 2 {
                assert!(x + 1 < surface.width);
                assert!(surface.get(x + 1, y).unwrap().is_continuation);
            }
        }
    }
}

#[test]
fn cue_sheet_is_contiguous_and_direct_entry_has_the_right_act() {
    assert_eq!(CUES[0].start, 0.);
    assert_eq!(CUES[0].end, 20.);
    for pair in CUES.windows(2) {
        assert_eq!(pair[0].end, pair[1].start);
    }
    for cue in CUES {
        assert!(cue.end > cue.start);
        let mut d = Director::default();
        assert!(d.stage(cue.name));
        assert_eq!(d.seconds, cue.start);
        assert_eq!(d.cue().act, cue.act);
    }
    let mut d = at(40.);
    let before = d.clone();
    assert!(!d.stage("not-an-act"));
    assert_eq!(d, before);
}

#[test]
fn pause_seek_skip_and_terminal_hold_are_bounded() {
    let mut d = at(39.);
    d.paused = true;
    d.advance(100.);
    assert_eq!(d.seconds, 39.);
    d.skip(true);
    assert_eq!(d.cue().act, Act::Couriers);
    d.skip(false);
    assert_eq!(d.cue().act, Act::Facades);
    d.paused = false;
    for invalid in [f32::NAN, f32::INFINITY, -1., 0.] {
        d.advance(invalid);
    }
    assert_eq!(d.seconds, 36.);
    d.advance(1000.);
    let final_state = d.clone();
    for _ in 0..10_000 {
        d.advance(1. / 60.);
    }
    assert_eq!(d, final_state);
    assert_eq!(d.seconds, 72.);
    d.seek(-3.);
    d.skip(false);
    assert_eq!(d.seconds, 0.);
}

#[test]
fn semantic_jobs_respect_dependencies_and_messages_carry_completed_outputs() {
    for agent in AGENTS {
        assert!(agent.finish_ms > agent.start_ms);
        assert!(!agent.result.is_empty());
        if let Some(parent) = agent.depends_on {
            assert!(parent < AGENTS.len());
            assert!(AGENTS[parent].finish_ms <= agent.start_ms);
        }
        assert_eq!(
            agent.status(agent.finish_ms as f32 / 1000.),
            JobStatus::Complete
        );
        assert_eq!(agent.progress(agent.finish_ms as f32 / 1000.), 1.);
    }
    for message in MESSAGES {
        assert!(message.from < AGENTS.len() && message.to < AGENTS.len());
        assert_ne!(message.from, message.to);
        assert!(!message.payload.is_empty());
        assert!(message.arrive > message.depart);
        assert_eq!(
            AGENTS[message.from].status(message.depart),
            JobStatus::Complete
        );
        assert!(message.depart >= CUES[4].start && message.arrive <= CUES[4].end);
        // Forward contracts follow the job dependency chain; the final receipt
        // returns from validation to the architect who requested it.
        assert!(
            AGENTS[message.to].depends_on == Some(message.from)
                || (message.from == 3 && message.to == 0)
        );
    }
}

#[test]
fn exact_update_replay_and_seek_produce_identical_final_projection() {
    let steps = [3.125, 8., 0.25, 10., 6.625, 4., 7.];
    let mut first = Director::default();
    let mut shots = Vec::new();
    for dt in steps {
        first.advance(dt);
        shots.push(first.cue().act);
    }
    let mut replay = Director::default();
    let replay_shots: Vec<_> = steps
        .into_iter()
        .map(|dt| {
            replay.advance(dt);
            replay.cue().act
        })
        .collect();
    assert_eq!(shots, replay_shots);
    assert_eq!(first, replay);
    let seek = at(first.seconds);
    assert_eq!(seek, first);
    let before = first.clone();
    let a = intro::frame(&first, 120, 32, ColorDepth::TrueColor, true);
    assert_eq!(
        a,
        intro::frame(&replay, 120, 32, ColorDepth::TrueColor, true)
    );
    assert_eq!(a, intro::frame(&seek, 120, 32, ColorDepth::TrueColor, true));
    assert_eq!(
        first, before,
        "painting must not advance direction or semantic time"
    );
}

#[test]
fn frozen_frames_have_zero_exact_affected_and_wire_cost() {
    for seconds in [12., 23., 31., 39., 47., 58., 68.] {
        let d = at(seconds);
        let a = intro::frame(&d, 56, 24, ColorDepth::TrueColor, true);
        let b = intro::frame(&d, 56, 24, ColorDepth::TrueColor, true);
        let delta = compute_diff(Some(&a), &b);
        assert_eq!(delta.exact_changed_cell_count(), 0, "at {seconds}");
        assert_eq!(delta.affected_cell_count(), 0, "at {seconds}");
        assert!(AnsiCompiler::new().compile(&delta).is_empty());
    }
}

#[test]
fn every_act_is_responsive_wide_safe_and_hints_retreat_during_the_film() {
    for (w, h) in [(56, 24), (120, 32), (160, 40)] {
        for seconds in [12., 23., 31., 39., 47., 58., 68.] {
            for depth in [ColorDepth::TrueColor, ColorDepth::Mono] {
                let d = at(seconds);
                let snapshot = d.clone();
                let surface = intro::frame(&d, w, h, depth, true);
                assert_eq!((surface.width, surface.height), (w, h));
                wide_valid(&surface);
                assert!(
                    !text(&surface).contains("space pause"),
                    "at {seconds}, {w}x{h}"
                );
                assert_eq!(d, snapshot);
                let occupied = surface
                    .cells
                    .iter()
                    .filter(|c| !matches!(c.glyph.grapheme.as_str(), " " | "" | "⠀"))
                    .count();
                assert!(occupied > 50, "empty composition at {seconds}, {w}x{h}");
            }
        }
    }
}

#[test]
fn semantic_labels_and_rgb_diversity_survive_the_visual_transformation() {
    let harness = intro::frame(&at(12.), 120, 32, ColorDepth::TrueColor, false);
    assert!(text(&harness).contains("LIBGIBSON"));
    assert!(text(&harness).contains("BUILDER"));
    let facade = intro::frame(&at(39.), 120, 32, ColorDepth::TrueColor, false);
    assert!(text(&facade).contains("SCOUT"));
    let finale = intro::frame(&at(68.), 120, 32, ColorDepth::TrueColor, false);
    wide_valid(&finale); // The subtitle is now graphical lettering, not text cells.
    for seconds in [31., 39., 47., 58., 68.] {
        let rgb = intro::world::raster(120, 31, seconds);
        let palette: HashSet<_> = rgb.pixels().iter().copied().collect();
        assert!(
            palette.len() > 32,
            "flat raster palette at {seconds}: {}",
            palette.len()
        );
        // Held hero shots need a bright focal element; the ascent crossfade
        // deliberately darkens between scales and is checked for diversity.
        if seconds != 58. {
            assert!(
                rgb.pixels().iter().any(|&(r, g, b)| r.max(g).max(b) > 160),
                "missing focal light at {seconds}"
            );
        }
    }
}

#[test]
fn empty_small_and_capped_viewports_are_safe_and_resize_is_only_projection() {
    let d = at(39.);
    let original = intro::frame(&d, 56, 24, ColorDepth::TrueColor, false);
    for (w, h) in [
        (0, 0),
        (0, 24),
        (56, 0),
        (1, 1),
        (2, 2),
        (320, 100),
        (u16::MAX, u16::MAX),
    ] {
        let surface = intro::frame(&d, w, h, ColorDepth::TrueColor, false);
        assert_eq!((surface.width, surface.height), (w.min(320), h.min(100)));
        wide_valid(&surface);
    }
    assert_eq!(
        original,
        intro::frame(&d, 56, 24, ColorDepth::TrueColor, false)
    );
    assert_eq!(d, at(39.));
}

#[test]
fn interaction_hints_have_explicit_seekable_visibility_without_paint_timers() {
    let mut d = at(0.0);
    assert!(d.hints_visible());
    d.seek(4.0);
    assert!(!d.hints_visible());
    d.seek(39.0);
    assert!(!d.hints_visible());
    d.interacted();
    assert!(d.hints_visible());
    d.advance(3.0);
    assert!(!d.hints_visible());
    d.paused = true;
    assert!(d.hints_visible());
    let paused = intro::frame(&d, 56, 24, ColorDepth::Mono, true);
    assert!(text(&paused).contains("PAUSED"));
    assert!(!text(&intro::frame(&d, 56, 24, ColorDepth::Mono, false)).contains("PAUSED"));
    d.paused = false;
    d.seek(70.0);
    assert!(!d.hints_visible());
    d.seek(71.0);
    assert!(d.hints_visible());
    let held = intro::frame(&d, 120, 32, ColorDepth::TrueColor, true);
    assert!(text(&held).contains("replay / R"));
    assert!(!text(&held).contains("space pause"));
}

#[test]
fn four_identities_are_unique_cell_safe_and_survive_every_readable_facade() {
    use intro::identity::IDENTITIES;
    assert_eq!(IDENTITIES.len(), AGENTS.len());
    let mut colors = HashSet::new();
    let mut marks = HashSet::new();
    for (index, identity) in IDENTITIES.iter().enumerate() {
        assert!(colors.insert(identity.accent));
        assert!(marks.insert(identity.signature));
        assert_eq!(gibson::Glyph::new(identity.signature).display_width, 1);
        for depth in [ColorDepth::TrueColor, ColorDepth::Mono] {
            for (w, h) in [(56, 24), (120, 32), (160, 40)] {
                let facade = intro::frame(&at(37.0 + index as f32 * 2.0), w, h, depth, false);
                let labels = text(&facade);
                assert!(
                    labels.contains(AGENTS[index].name),
                    "missing {} at {w}x{h} {depth:?}",
                    AGENTS[index].name
                );
                assert!(
                    labels.contains(identity.signature),
                    "missing identity mark at {w}x{h}"
                );
            }
        }
    }
}

#[test]
fn named_city_shots_cover_each_facade_and_preserve_continuous_camera_poses() {
    use intro::shots::{facade, SHOTS};
    assert_eq!(SHOTS[0].start, 28.0);
    assert_eq!(SHOTS.last().unwrap().end, 60.0);
    for pair in SHOTS.windows(2) {
        assert_eq!(pair[0].end, pair[1].start);
    }
    for (index, t) in [37.0, 39.0, 41.0, 43.0].into_iter().enumerate() {
        assert_eq!(facade(t), Some(index));
    }
    for (w, h) in [(56, 24), (120, 32), (160, 40)] {
        for t in (270..610).map(|t| t as f32 / 10.0) {
            let camera = intro::world::camera(t, w, h);
            assert!(camera.position.is_finite() && camera.target.is_finite());
            assert!(camera.fov_y > 0.0 && camera.fov_y < std::f32::consts::PI);
        }
        let boundaries = SHOTS
            .iter()
            .skip(1)
            .map(|s| s.start)
            .chain(MESSAGES.iter().flat_map(|m| [m.depart, m.arrive]));
        for boundary in boundaries {
            let a = intro::world::camera(boundary - 0.0001, w, h);
            let b = intro::world::camera(boundary + 0.0001, w, h);
            let distance = |p: gibson::geom::Vec3, q: gibson::geom::Vec3| {
                ((p.x - q.x).powi(2) + (p.y - q.y).powi(2) + (p.z - q.z).powi(2)).sqrt()
            };
            assert!(
                distance(a.position, b.position) < 0.03,
                "camera cut at {}: {:?} -> {:?}",
                boundary,
                a.position,
                b.position
            );
            assert!(
                distance(a.target, b.target) < 0.03,
                "target cut at {}",
                boundary
            );
        }
    }
}

#[test]
fn graphical_title_and_companion_fit_the_supported_compositions() {
    for (w, h) in [(56, 24), (120, 32), (160, 40)] {
        assert!(intro::planet::title_bounds(w, h, 61.0).is_none());
        for tick in 631..=720 {
            let t = tick as f32 / 10.0;
            let (left, top, right, bottom) = intro::planet::title_bounds(w, h, t).unwrap();
            assert!(
                left >= 0 && top >= 0 && right <= i32::from(w) && bottom <= i32::from(h) * 2,
                "title clipped at {w}x{h} {t}s: {left},{top}..{right},{bottom}"
            );
            assert!(right > left && bottom > top);
        }
        let d = at(72.0);
        let a = intro::frame(&d, w, h, ColorDepth::TrueColor, true);
        let b = intro::frame(&d, w, h, ColorDepth::TrueColor, true);
        let diff = compute_diff(Some(&a), &b);
        assert_eq!(
            (diff.exact_changed_cell_count(), diff.affected_cell_count()),
            (0, 0)
        );
        assert!(AnsiCompiler::new().compile(&diff).is_empty());
    }
}
