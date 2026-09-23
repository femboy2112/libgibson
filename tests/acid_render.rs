//! Whole-frame and bounded-damage evidence for the actual fictional encounter.
#[allow(dead_code)]
#[path = "../examples/acid_vs_crash.rs"]
mod encounter;

use encounter::Encounter;
use gibson::*;
use std::time::Duration;

fn surface(mut root: Node, w: u16, h: u16) -> Surface {
    compute_layout(&mut root, w, h).unwrap();
    let mut s = Surface::new(w, h);
    paint(&root, &mut s);
    s
}
fn rows(s: &Surface) -> Vec<String> {
    (0..s.height)
        .map(|y| {
            (0..s.width)
                .filter_map(|x| {
                    let c = s.get(x, y).unwrap();
                    (!c.is_continuation).then_some(c.glyph.grapheme.as_str())
                })
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}
fn valid(s: &Surface) {
    for y in 0..s.height {
        for x in 0..s.width {
            let c = s.get(x, y).unwrap();
            if c.is_continuation {
                assert!(x > 0);
                assert_eq!(s.get(x - 1, y).unwrap().glyph.display_width, 2);
            } else if c.glyph.display_width == 2 {
                assert!(s.get(x + 1, y).is_some_and(|c| c.is_continuation));
            }
        }
    }
}
#[test]
fn actual_story_frames_match_whole_renderer_vt100_across_sizes() {
    for (w, h) in [(56, 24), (80, 24), (120, 32), (160, 40)] {
        let mut renderer = Renderer::new(RenderMode::Fullscreen);
        let mut terminal = TerminalSession::headless(w, h);
        let mut parser = vt100::Parser::new(h, w, 0);
        for stage in encounter::STAGES {
            let mut e = Encounter::new(stage, true);
            e.update(Duration::from_millis(400), &[]);
            let expected = surface(e.frame(w, h), w, h);
            valid(&expected);
            let mut wire = Vec::new();
            renderer
                .render(&mut e.frame(w, h), &mut terminal, &mut wire)
                .unwrap();
            parser.process(&wire);
            let got = parser
                .screen()
                .rows(0, w)
                .map(|s| s.trim_end().to_string())
                .collect::<Vec<_>>();
            assert_eq!(got, rows(&expected), "{stage} at {w}x{h}");
            let text = got.join("\n");
            assert!(
                text.contains("crash >"),
                "control island missing in {stage} at {w}x{h}"
            );
            wire.clear();
            let (_, _, bytes, _, _) = renderer
                .render(&mut e.frame(w, h), &mut terminal, &mut wire)
                .unwrap();
            assert_eq!(bytes, 0, "frozen {stage} at {w}x{h}");
            assert!(wire.is_empty());
        }
    }
}

fn measure(label: &str, a: Node, b: Node, w: u16, h: u16) -> (usize, usize, usize) {
    let previous = surface(a.clone(), w, h);
    let next = surface(b.clone(), w, h);
    let diff = compute_diff(Some(&previous), &next);
    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut terminal = TerminalSession::headless(w, h);
    let mut wire = Vec::new();
    renderer
        .render(&mut a.clone(), &mut terminal, &mut wire)
        .unwrap();
    wire.clear();
    let (_, _, bytes, _, _) = renderer
        .render(&mut b.clone(), &mut terminal, &mut wire)
        .unwrap();
    eprintln!(
        "{label}: exact={} affected={} wire={bytes}",
        diff.exact_changed_cell_count(),
        diff.affected_cell_count()
    );
    (
        diff.exact_changed_cell_count(),
        diff.affected_cell_count(),
        bytes,
    )
}
#[test]
fn real_ghost_cursor_and_route_motion_have_bounded_damage() {
    let mut e = Encounter::new("ghost", false);
    e.update(Duration::from_millis(600), &[]);
    let a = e.frame(120, 32);
    let mut b = a.clone();
    // Hold the world fixed and move only its actual ghost entity by one cell.
    let cursor = b
        .children
        .iter_mut()
        .find(|n| matches!(&n.kind,NodeKind::Text{text,..} if text.contains("◀ AB")))
        .expect("ghost scene entity");
    cursor.layout_style.offset_x += 1.0;
    let (exact, affected, bytes) = measure("ghost one cell", a, b, 120, 32);
    assert!((1..=12).contains(&exact));
    assert!(affected <= 12);
    assert!(bytes < 400);

    let mut quiet = Encounter::new("quiet", false);
    let a = quiet.frame(120, 32);
    quiet.update(Duration::from_millis(100), &[]);
    let mut b = a.clone();
    let advanced = quiet.frame(120, 32);
    // Isolate the real map's packet advancement from the machine clock label.
    b.children[1] = advanced.children[1].clone();
    let (exact, affected, bytes) = measure("route packets 100ms", a, b, 120, 32);
    assert!(exact > 0);
    assert!(exact < 120 * 32 / 4);
    assert!(affected < 120 * 32 / 3);
    assert!(bytes < 4000);
}
#[test]
fn display_counter_removes_invasion_without_corrupting_control_island() {
    let mut e = Encounter::new("takeover", false);
    e.update(Duration::from_secs(3), &[]);
    let before = e.frame(120, 32);
    let before_surface = surface(before.clone(), 120, 32);
    assert!(e.command("hard isolate"));
    let after = e.frame(120, 32);
    let after_surface = surface(after.clone(), 120, 32);
    for y in 30..32 {
        for x in 0..120 {
            assert_eq!(
                before_surface.get(x, y),
                after_surface.get(x, y),
                "command island {x},{y}"
            );
        }
    }
    assert!(!e
        .director()
        .mounted()
        .any(|b| b == "takeover" || b == "display-presence"));
    let (exact, _, _) = measure("display removal", before, after, 120, 32);
    assert!(exact > 200);
    assert!(e.replay_matches());
}

#[test]
fn ghost_input_never_edits_crash_buffer_and_live_resize_keeps_renderer_correct() {
    let mut e = Encounter::new("takeover", true);
    e.input.insert_str("my unfinished response 界 e\u{301}");
    let original = e.input.text.clone();
    let cursor = e.input.cursor_grapheme;
    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut terminal = TerminalSession::headless(120, 32);
    let mut parser = vt100::Parser::new(32, 120, 0);
    for (w, h) in [(120, 32), (56, 24), (160, 40), (80, 24), (120, 32)] {
        terminal.set_terminal_size(w, h);
        parser.set_size(h, w);
        e.tick(Duration::from_millis(150), false);
        let expected = surface(e.frame(w, h), w, h);
        let mut wire = Vec::new();
        renderer
            .render(&mut e.frame(w, h), &mut terminal, &mut wire)
            .unwrap();
        parser.process(&wire);
        assert_eq!(e.input.text, original);
        assert_eq!(e.input.cursor_grapheme, cursor);
        let actual = parser
            .screen()
            .rows(0, w)
            .map(|s| s.trim_end().to_string())
            .collect::<Vec<_>>();
        assert_eq!(actual, rows(&expected), "live resize {w}x{h}");
        assert!(e.replay_matches());
    }
}

#[test]
fn real_auth_frontier_increment_is_panel_local_and_frozen_frame_is_free() {
    let mut e = Encounter::new("first-breach", false);
    let a = e.frame(120, 32);
    e.update(Duration::from_millis(100), &[]);
    let next = e.frame(120, 32);
    let session = |n: &Node| matches!(&n.kind,NodeKind::Border{title:Some(title),..} if title.contains("SESSION /"));
    let index = a
        .children
        .iter()
        .position(session)
        .expect("ordinary session entity");
    let mut b = a.clone();
    // Isolate the actual changed entity from graph packets and narrative data.
    b.children[index] = next.children.iter().find(|n| session(n)).unwrap().clone();
    let sa = surface(a.clone(), 120, 32);
    let sb = surface(b.clone(), 120, 32);
    let diff = compute_diff(Some(&sa), &sb);
    for (x, y) in diff.exact_changed_cells() {
        assert!(
            x >= 72 && (3..19).contains(&y),
            "frontier escaped session: {x},{y}"
        );
    }
    let (exact, affected, bytes) = measure("actual AUTH frontier 100ms", a, b.clone(), 120, 32);
    assert!(exact > 0 && exact < 48 * 16 / 2);
    assert!(affected < 48 * 16);
    assert!(bytes < 4000);
    let frozen = measure("frozen battlefield", b.clone(), b, 120, 32);
    assert_eq!(frozen, (0, 0, 0));
}

#[test]
fn instant_final_counter_does_not_freeze_a_recoil_offscreen() {
    let mut e = Encounter::new("climax", false);
    e.command("hard isolate");
    e.command("cut link");
    assert!(e.director().is_finished());
    let frame = e.frame(120, 32);
    let map = frame
        .children
        .iter()
        .find(
            |n| matches!(&n.kind,NodeKind::Border{title:Some(t),..} if t.contains("LOCAL FABRIC")),
        )
        .unwrap();
    assert_eq!(map.layout_style.offset_x, 0.0);
    assert!(!e.director().mounted().any(|name| name == "route-isolation"));
    assert!(e.replay_matches());
}

#[test]
fn packets_advance_between_world_quanta_and_replay_exactly() {
    let mut e = Encounter::new("quiet", false);
    let graph = e.battle().graph.clone();
    let before = e.frame(160, 40);
    e.update(Duration::from_millis(33), &[]);
    assert_eq!(e.battle().elapsed_ms, 0, "no simulation quantum consumed");
    assert_eq!(
        e.battle().graph,
        graph,
        "presentation cannot advance influence"
    );
    let after = e.frame(160, 40);
    let (exact, affected, bytes) = measure("sub-quantum route motion", before, after, 160, 40);
    assert!(exact > 0 && exact < 100);
    assert!(affected < 100 && bytes < 3000);
    assert_eq!(
        surface(e.frame(160, 40), 160, 40),
        surface(e.replay().frame(160, 40), 160, 40)
    );
}

#[test]
fn acid_route_strokes_do_not_fill_terminal_cell_backgrounds() {
    let e = Encounter::new("route-contested", false);
    let s = surface(e.frame(120, 32), 120, 32);
    let mut acid_dots = 0;
    for y in 0..s.height {
        for x in 0..s.width {
            let cell = s.get(x, y).unwrap();
            if cell
                .glyph
                .grapheme
                .chars()
                .any(|c| ('\u{2801}'..='\u{28ff}').contains(&c))
                && cell.style.fg == Some(Color::Rgb(255, 79, 192))
            {
                acid_dots += 1;
                assert!(!cell.style.reverse, "route became a rectangle at {x},{y}");
                assert!(cell.style.bg.is_none() || cell.style.bg == Some(Color::Reset));
            }
            assert!(!cell.glyph.grapheme.contains('▰'), "solid pressure bar");
        }
    }
    assert!(acid_dots > 0, "the test must actually see Acid's carrier");
}
