//! Consumer acceptance for the experience tier (ProjectGHOST/MINERVA, E1–E8):
//! Blades with a Standard fallback, unavailable destinations that are never
//! live, actions as labels only, identity-keyed selection, keys handed back to
//! the application, reduced motion and determinism. E7 (camera-rig poses) lives
//! with `gibson::rig`'s unit tests. See `docs/EXPERIENCE_ACCEPTANCE.md`.

use gibson::capability::ColorDepth;
use gibson::input::{KeyCode, KeyEvent, KeyModifiers};
use gibson::ui::experience::{
    handle_key_outcome, required_semantics, Action, Blades, Content, CrossMedia, Destination,
    Experience, Fallback, FrameDemand, Grammar, Item, KeyOutcome, MediaShelf, Orbital, Panorama,
    PresentationState, Standard,
};
use gibson::ui::skin::UiEnvironment;
use gibson::ui::MotionPreference;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Request(u64);

fn key(name: &str) -> gibson::ui::element::Key {
    gibson::ui::element::Key::named(name)
}

/// The Citadel strip: six categories; Beings unavailable; Projects holds
/// journal actions keyed by their action id, status word first.
fn citadel(order: &[u64]) -> Experience<Request> {
    let status = |id: u64| match id % 3 {
        0 => "AWAITING DECISION",
        1 => "UNDERWAY",
        _ => "VERIFIED",
    };
    let projects = Content::Collection(
        order
            .iter()
            .map(|&id| {
                Item::new(format!("a-{id}"), format!("{} · job {id}", status(id)))
                    .action(Action::new(format!("open-{id}"), "Open", Request(id)))
            })
            .collect(),
    );
    let prose = |s: &str| Content::Prose(vec![s.to_string()]);
    Experience::new("CITADEL")
        .destination(Destination::new("chat", "Chat", prose("conversation")))
        .destination(Destination::new("projects", "Projects", projects))
        .destination(Destination::new("agents", "Agents", prose("agents")))
        .destination(
            Destination::new("beings", "Beings", prose("would be live")).unavailable("not built"),
        )
        .destination(Destination::new("memory", "Memory", prose("memory")))
        .destination(Destination::new(
            "instruments",
            "Instruments",
            prose("instruments"),
        ))
}

fn env(width: u16, height: u16, depth: ColorDepth, motion: MotionPreference) -> UiEnvironment {
    UiEnvironment {
        width,
        height,
        color_depth: depth,
        motion,
        ..UiEnvironment::default()
    }
}

fn wide() -> UiEnvironment {
    env(120, 40, ColorDepth::TrueColor, MotionPreference::Full)
}

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::empty(),
    }
}

fn blades() -> Fallback<Request> {
    Fallback::new(Box::new(Blades::new()))
}

fn paint(
    grammar: &mut dyn Grammar<Request>,
    experience: &Experience<Request>,
    state: &PresentationState,
    e: &UiEnvironment,
    now: Duration,
) -> gibson::Surface {
    let presented = grammar.present(experience, state, e, now);
    let cx = gibson::ui::BuildCx::new(gibson::ui::skins::BLACK_ICE, *e);
    let mut node = gibson::ui::compile(&presented.element, &cx)
        .expect("lowers")
        .node;
    gibson::compute_layout(&mut node, e.width, e.height).unwrap();
    let mut surface = gibson::Surface::new(e.width, e.height);
    gibson::paint(&node, &mut surface);
    surface
}

fn text(s: &gibson::Surface) -> String {
    (0..s.height)
        .map(|y| {
            (0..s.width)
                .filter_map(|x| {
                    let c = s.get(x, y).unwrap();
                    (!c.is_continuation).then(|| c.glyph.grapheme.to_string())
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Projects active, `a-<id>` selected.
fn state_at(experience: &Experience<Request>, id: u64) -> PresentationState {
    let mut state = PresentationState::new(experience);
    assert!(state.activate(experience, &key("projects")));
    assert!(state.select(experience, &key(&format!("a-{id}"))));
    state
}

#[test]
fn e1_switching_standard_and_blades_keeps_selection_actions_and_reasons() {
    let x = citadel(&[101, 102, 103]);
    let state = state_at(&x, 102);
    let required = required_semantics(&x, &state);
    assert_eq!(required.selected, Some(key("a-102")));
    assert_eq!(
        required.unavailable,
        vec![(key("beings"), "not built".to_string())]
    );
    let mut grammars: Vec<Box<dyn Grammar<Request>>> =
        vec![Box::new(Standard::new()), Box::new(blades())];
    let receipts: Vec<_> = grammars
        .iter_mut()
        .map(|g| g.present(&x, &state, &wide(), Duration::ZERO).receipt)
        .collect();
    assert_eq!(receipts[0].style, "STANDARD");
    assert_eq!(receipts[1].style, "BLADES");
    for r in &receipts {
        assert_eq!(r.check(&required), vec![], "{}", r.style);
        assert_eq!(r.selected, Some(key("a-102")), "{}", r.style);
    }
    let actions = |r: &gibson::ui::experience::PresentationReceipt| {
        let mut a: Vec<_> = r
            .actions
            .iter()
            .filter(|k| required.actions.contains(k))
            .cloned()
            .collect();
        a.sort();
        a
    };
    assert_eq!(actions(&receipts[0]), actions(&receipts[1]));
}

fn all_grammars() -> Vec<Box<dyn Grammar<Request>>> {
    vec![
        Box::new(Standard::new()),
        Box::new(Blades::new()),
        Box::new(MediaShelf::new()),
        Box::new(CrossMedia::new()),
        Box::new(Orbital::new()),
        Box::new(Panorama::new()),
        Box::new(blades()),
    ]
}

#[test]
fn e2_an_unavailable_destination_is_never_live_in_any_grammar() {
    let x = citadel(&[101, 102]);
    let mut state = PresentationState::new(&x);
    assert!(state.activate(&x, &key("beings")));
    let required = required_semantics(&x, &state);
    assert_eq!(required.active_destination, Some(key("beings")));
    assert!(required.all_items.is_empty() && required.actions.is_empty());
    for mut g in all_grammars() {
        let presented = g.present(&x, &state, &wide(), Duration::from_secs(5));
        assert!(
            presented.receipt.items.is_empty(),
            "{}",
            presented.receipt.style
        );
        let painted = text(&paint(&mut *g, &x, &state, &wide(), Duration::from_secs(5)));
        assert!(painted.contains("not built"), "{}:\n{painted}", g.name());
        // Activating it yields nothing, whatever key the grammar binds.
        let mut s = state.clone();
        for code in [KeyCode::Enter, KeyCode::Char(' ')] {
            assert!(
                !matches!(
                    handle_key_outcome(&*g, &x, &mut s, &press(code)),
                    KeyOutcome::Action(_)
                ),
                "{}",
                g.name()
            );
        }
    }
}

#[test]
fn e3_selection_follows_the_action_id_across_a_reorder() {
    let x = citadel(&[101, 102, 103, 104]);
    let state = state_at(&x, 103);
    let reordered = citadel(&[104, 103, 102, 101]);
    for mut g in [
        Box::new(Standard::new()) as Box<dyn Grammar<Request>>,
        Box::new(blades()),
    ] {
        let r = g
            .present(&reordered, &state, &wide(), Duration::ZERO)
            .receipt;
        assert_eq!(r.selected, Some(key("a-103")), "{}", g.name());
    }
    let mut s = state.clone();
    assert_eq!(
        handle_key_outcome(&Standard::new(), &reordered, &mut s, &press(KeyCode::Enter)),
        KeyOutcome::Action(Request(103))
    );
}

#[test]
fn e4_blades_steps_down_to_standard_and_status_words_survive() {
    let x = citadel(&[101, 102, 103]);
    let state = state_at(&x, 101);
    let cases = [
        env(40, 24, ColorDepth::TrueColor, MotionPreference::Full),
        env(80, 24, ColorDepth::Mono, MotionPreference::Full),
        env(80, 24, ColorDepth::TrueColor, MotionPreference::Reduced),
        env(80, 24, ColorDepth::TrueColor, MotionPreference::None),
    ];
    for e in cases {
        let mut g = blades();
        assert_eq!(
            g.present(&x, &state, &e, Duration::ZERO).receipt.style,
            "STANDARD",
            "{e:?}"
        );
        let painted = text(&paint(&mut g, &x, &state, &e, Duration::ZERO));
        for word in ["UNDERWAY", "VERIFIED", "AWAITING DECISION"] {
            assert!(
                painted.contains(word),
                "{word} at {}×{} {:?}:\n{painted}",
                e.width,
                e.height,
                e.color_depth
            );
        }
    }
    let mut g = blades();
    assert_eq!(
        g.present(&x, &state, &wide(), Duration::ZERO).receipt.style,
        "BLADES"
    );
}

#[test]
fn e5_reduced_motion_frame_is_settled_at_time_zero() {
    // Switch blade (Projects → Agents) and paint the very next frame. With
    // reduced motion it must already be the settled frame; the full-motion
    // control must still be gliding, or this test could not tell them apart.
    let x = citadel(&[101, 102]);
    let first = state_at(&x, 102);
    let mut next = first.clone();
    assert!(next.activate(&x, &key("agents")));
    let frame_after_switch = |motion: MotionPreference| {
        let e = env(100, 30, ColorDepth::TrueColor, motion);
        let mut g = blades();
        g.present(&x, &first, &e, Duration::ZERO);
        let demand = g.present(&x, &next, &e, Duration::from_millis(16)).demand;
        let soon = paint(&mut g, &x, &next, &e, Duration::from_millis(16));
        let settled = paint(&mut g, &x, &next, &e, Duration::from_secs(10));
        (demand, soon == settled)
    };
    for motion in [MotionPreference::Reduced, MotionPreference::None] {
        let (demand, settled) = frame_after_switch(motion);
        assert!(matches!(demand, FrameDemand::OnChange), "{motion:?}");
        assert!(
            settled,
            "{motion:?}: first frame after a switch is the settled one"
        );
    }
    let (demand, settled) = frame_after_switch(MotionPreference::Full);
    assert!(
        !matches!(demand, FrameDemand::OnChange) && !settled,
        "full motion glides"
    );
}

#[test]
fn e6_same_scene_size_and_time_paint_identically() {
    let x = citadel(&[101, 102, 103]);
    let state = state_at(&x, 102);
    for t in [0u64, 250, 1_000] {
        let a = paint(&mut blades(), &x, &state, &wide(), Duration::from_millis(t));
        let b = paint(&mut blades(), &x, &state, &wide(), Duration::from_millis(t));
        assert!(a == b, "t={t}");
    }
}

#[test]
fn e8_keys_the_experience_does_not_own_go_back_to_the_application() {
    let x = citadel(&[101, 102]);
    for e in [
        wide(),
        env(40, 20, ColorDepth::Mono, MotionPreference::Full),
    ] {
        let mut g = blades();
        let mut state = state_at(&x, 101);
        g.present(&x, &state, &e, Duration::ZERO);
        for code in [
            KeyCode::Esc,
            KeyCode::Backspace,
            KeyCode::Char('q'),
            KeyCode::Tab,
            KeyCode::Delete,
        ] {
            let before = state.clone();
            assert_eq!(
                handle_key_outcome(&g, &x, &mut state, &press(code)),
                KeyOutcome::Unhandled,
                "{code:?} at {}",
                e.width
            );
            assert_eq!(state, before, "{code:?} changed nothing");
        }
        assert_eq!(
            handle_key_outcome(&Standard::new(), &x, &mut state, &press(KeyCode::Down)),
            KeyOutcome::Consumed
        );
    }
}
