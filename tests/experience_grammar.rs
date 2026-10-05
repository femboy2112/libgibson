//! Experience-grammar freeze contracts (milestone §52, the subset provable before
//! any cinematic style exists): the semantic model, navigation, the preservation
//! law (and that it *bites*), per-destination selection retention, live
//! style-switch state preservation, reachability, determinism, and
//! capability/responsive naturality of the reference grammar.

use gibson::capability::ColorDepth;
use gibson::input::{KeyCode, KeyEvent, KeyModifiers};
use gibson::ui::experience::{
    apply_intent, handle_key, required_semantics, Action, Content, Destination, Experience,
    Grammar, Intent, Item, LawViolation, Media, PresentationReceipt, PresentationState, Presented,
    Priority, SemanticInput, Standard,
};
use gibson::ui::skin::UiEnvironment;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    Play(String),
    Toggle(String),
    Open,
}

/// A mixed-domain fixture: a media collection, a detail page, a settings
/// collection (settings as actionable items — not a separate node kind), and
/// prose. Deliberately exercises every content kind and both essential and
/// tertiary priority.
fn fixture() -> Experience<Msg> {
    let library = Content::Collection(vec![
        Item::new("alb-0", "Neon Harbour")
            .subtitle("Vapor Cartography")
            .media(Media::new(0))
            .priority(Priority::Essential)
            .action(Action::new("play-0", "Play", Msg::Play("alb-0".into()))),
        Item::new("alb-1", "Glass Arcades")
            .media(Media::new(1))
            .action(Action::new("play-1", "Play", Msg::Play("alb-1".into()))),
        Item::new("alb-2", "Tidal Automata")
            .media(Media::new(2))
            .action(Action::new("play-2", "Play", Msg::Play("alb-2".into()))),
        Item::new("alb-3", "Low Orbit Choir")
            .media(Media::new(3))
            .action(Action::new("play-3", "Play", Msg::Play("alb-3".into()))),
        Item::new("alb-4", "Hidden Track")
            .media(Media::new(4))
            .priority(Priority::Tertiary)
            .action(Action::new("play-4", "Play", Msg::Play("alb-4".into()))),
    ]);

    let now_playing = Content::Detail {
        facets: vec![
            gibson::ui::experience::Facet::new("track", "Track", "Neon Harbour"),
            gibson::ui::experience::Facet::new("time", "Elapsed", "01:12"),
        ],
        actions: vec![Action::new("open", "Open", Msg::Open)],
    };

    let settings = Content::Collection(vec![
        Item::new("set-color", "Colour depth")
            .subtitle("TrueColor")
            .action(Action::new("t-color", "Cycle", Msg::Toggle("color".into()))),
        Item::new("set-motion", "Motion")
            .subtitle("Full")
            .action(Action::new(
                "t-motion",
                "Cycle",
                Msg::Toggle("motion".into()),
            )),
    ]);

    let about = Content::Prose(vec![
        "Gibson Library — a semantic experience.".into(),
        "One application, many grammars.".into(),
    ]);

    Experience::new("GIBSON LIBRARY")
        .destination(Destination::new("library", "Library", library))
        .destination(Destination::new("now", "Now Playing", now_playing))
        .destination(Destination::new("settings", "Settings", settings))
        .destination(Destination::new("about", "About", about))
}

fn env(width: u16, height: u16, color_depth: ColorDepth) -> UiEnvironment {
    UiEnvironment {
        width,
        height,
        color_depth,
        ..UiEnvironment::default()
    }
}

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::empty(),
    }
}

// ---------------------------------------------------------------------------
// A second, deliberately different grammar used only in tests, to prove that
// style switching preserves semantics and that different grammars bind different
// physical keys to the same semantic intents. It represents everything (a
// faithful minimal grammar), but navigates with Left/Right instead of Up/Down.
// ---------------------------------------------------------------------------
struct Minimal;

impl<A: Clone> Grammar<A> for Minimal {
    fn name(&self) -> &'static str {
        "MINIMAL"
    }
    fn present(
        &mut self,
        experience: &Experience<A>,
        state: &PresentationState,
        _env: &UiEnvironment,
        _now: Duration,
    ) -> Presented<A> {
        let active_idx = state.active();
        let active = &experience.destinations[active_idx];
        let mut receipt = PresentationReceipt::new("MINIMAL", active.key.clone());
        for destination in &experience.destinations {
            receipt.destinations.push(destination.key.clone());
        }
        match &active.content {
            Content::Collection(items) => {
                for (index, item) in items.iter().enumerate() {
                    receipt.items.push(item.key.clone());
                    if index == state.selection() {
                        receipt.selected = Some(item.key.clone());
                        for action in &item.actions {
                            receipt.actions.push(action.key.clone());
                        }
                    }
                }
            }
            Content::Detail { actions, .. } => {
                for action in actions {
                    receipt.actions.push(action.key.clone());
                }
            }
            _ => {}
        }
        Presented {
            element: gibson::ui::element::text::<A>(experience.title.clone()),
            receipt,
        }
    }
    fn interpret(
        &self,
        key: &KeyEvent,
        _experience: &Experience<A>,
        _state: &PresentationState,
    ) -> Option<SemanticInput<A>> {
        let intent = match key.code {
            KeyCode::Left => Intent::Previous,
            KeyCode::Right => Intent::Next,
            KeyCode::Up => Intent::PreviousGroup,
            KeyCode::Down => Intent::NextGroup,
            KeyCode::Enter => Intent::Enter,
            _ => return None,
        };
        Some(SemanticInput::Navigate(intent))
    }
}

// ---------------------------------------------------------------------------
// The preservation law, and that it has teeth.
// ---------------------------------------------------------------------------

#[test]
fn standard_presentation_preserves_semantics_in_every_destination() {
    let experience = fixture();
    let mut grammar = Standard::new();
    let e = env(120, 40, ColorDepth::TrueColor);
    for destination in 0..experience.destinations.len() {
        let mut state = PresentationState::new(&experience);
        for _ in 0..destination {
            apply_intent(&experience, &mut state, Intent::NextGroup);
        }
        let presented = grammar.present(&experience, &state, &e, Duration::ZERO);
        let required = required_semantics(&experience, &state);
        let violations = presented.receipt.check(&required);
        assert!(
            violations.is_empty(),
            "destination {destination} violated the law: {violations:?}"
        );
    }
}

#[test]
fn required_action_of_the_selected_item_is_always_represented() {
    let experience = fixture();
    let mut grammar = Standard::new();
    let e = env(120, 40, ColorDepth::TrueColor);
    // Walk every item in the library; the selected item's play action must appear.
    for index in 0..5 {
        let mut state = PresentationState::new(&experience);
        for _ in 0..index {
            apply_intent(&experience, &mut state, Intent::Next);
        }
        let presented = grammar.present(&experience, &state, &e, Duration::ZERO);
        let expected = gibson::ui::element::Key::named(format!("play-{index}"));
        assert!(
            presented.receipt.actions.contains(&expected),
            "selected item {index} did not surface its play action; actions={:?}",
            presented.receipt.actions
        );
    }
}

#[test]
fn the_law_catches_a_silently_dropped_item() {
    let experience = fixture();
    let state = PresentationState::new(&experience);
    let required = required_semantics(&experience, &state);
    // A dishonest receipt that drops a normal item without declaring it.
    let mut receipt =
        PresentationReceipt::new("DISHONEST", gibson::ui::element::Key::named("library"));
    receipt.destinations = experience
        .destinations
        .iter()
        .map(|d| d.key.clone())
        .collect();
    // Represent only the first two of five items; declare no omission.
    receipt.items = vec![
        gibson::ui::element::Key::named("alb-0"),
        gibson::ui::element::Key::named("alb-1"),
    ];
    receipt.selected = Some(gibson::ui::element::Key::named("alb-0"));
    receipt.actions = vec![gibson::ui::element::Key::named("play-0")];
    let violations = receipt.check(&required);
    assert!(
        violations
            .iter()
            .any(|v| matches!(v, LawViolation::SilentLoss(_))),
        "expected a SilentLoss violation, got {violations:?}"
    );
}

#[test]
fn the_law_catches_an_omitted_essential() {
    let experience = fixture();
    let state = PresentationState::new(&experience);
    let required = required_semantics(&experience, &state);
    let mut receipt =
        PresentationReceipt::new("DISHONEST", gibson::ui::element::Key::named("library"));
    receipt.destinations = experience
        .destinations
        .iter()
        .map(|d| d.key.clone())
        .collect();
    // Declare the ESSENTIAL item omitted — never allowed.
    receipt.items = vec![
        gibson::ui::element::Key::named("alb-1"),
        gibson::ui::element::Key::named("alb-2"),
        gibson::ui::element::Key::named("alb-3"),
        gibson::ui::element::Key::named("alb-4"),
    ];
    receipt.omitted = vec![gibson::ui::element::Key::named("alb-0")];
    receipt.selected = Some(gibson::ui::element::Key::named("alb-1"));
    let violations = receipt.check(&required);
    assert!(
        violations
            .iter()
            .any(|v| matches!(v, LawViolation::EssentialOmitted(_))),
        "expected EssentialOmitted, got {violations:?}"
    );
}

#[test]
fn the_law_catches_an_invented_id() {
    let experience = fixture();
    let state = PresentationState::new(&experience);
    let required = required_semantics(&experience, &state);
    let mut receipt =
        PresentationReceipt::new("DISHONEST", gibson::ui::element::Key::named("library"));
    receipt.destinations = experience
        .destinations
        .iter()
        .map(|d| d.key.clone())
        .collect();
    receipt.items = experience
        .destinations
        .iter()
        .find(|d| d.title == "Library")
        .map(|_| {
            vec![
                gibson::ui::element::Key::named("alb-0"),
                gibson::ui::element::Key::named("alb-1"),
                gibson::ui::element::Key::named("alb-2"),
                gibson::ui::element::Key::named("alb-3"),
                gibson::ui::element::Key::named("alb-4"),
                gibson::ui::element::Key::named("ghost-item"), // invented
            ]
        })
        .unwrap();
    receipt.selected = Some(gibson::ui::element::Key::named("alb-0"));
    receipt.actions = vec![gibson::ui::element::Key::named("play-0")];
    let violations = receipt.check(&required);
    assert!(
        violations
            .iter()
            .any(|v| matches!(v, LawViolation::Invented(_))),
        "expected Invented, got {violations:?}"
    );
}

// ---------------------------------------------------------------------------
// Navigation semantics.
// ---------------------------------------------------------------------------

#[test]
fn item_navigation_clamps_and_never_teleports() {
    let experience = fixture();
    let mut state = PresentationState::new(&experience); // library, 5 items
    assert_eq!(state.selection(), 0);
    apply_intent(&experience, &mut state, Intent::Previous); // clamp at 0
    assert_eq!(state.selection(), 0);
    for _ in 0..10 {
        apply_intent(&experience, &mut state, Intent::Next); // clamp at 4
    }
    assert_eq!(state.selection(), 4);
    apply_intent(&experience, &mut state, Intent::Home);
    assert_eq!(state.selection(), 0);
    apply_intent(&experience, &mut state, Intent::End);
    assert_eq!(state.selection(), 4);
}

#[test]
fn rapid_reversal_stays_coherent() {
    let experience = fixture();
    let mut state = PresentationState::new(&experience);
    let script = [
        Intent::Next,
        Intent::Next,
        Intent::Next,
        Intent::Previous,
        Intent::Next,
        Intent::Previous,
        Intent::Previous,
    ];
    for intent in script {
        apply_intent(&experience, &mut state, intent);
    }
    // +1 +1 +1 -1 +1 -1 -1 = 1
    assert_eq!(state.selection(), 1);
}

#[test]
fn selection_is_retained_per_destination() {
    let experience = fixture();
    let mut state = PresentationState::new(&experience);
    // Select item 3 in the library.
    for _ in 0..3 {
        apply_intent(&experience, &mut state, Intent::Next);
    }
    assert_eq!(state.selection(), 3);
    // Move to settings, select item 1.
    apply_intent(&experience, &mut state, Intent::NextGroup); // now
    apply_intent(&experience, &mut state, Intent::NextGroup); // settings
    apply_intent(&experience, &mut state, Intent::Next);
    assert_eq!(state.selection(), 1);
    // Back to the library — its selection must be exactly where we left it.
    apply_intent(&experience, &mut state, Intent::PreviousGroup); // now
    apply_intent(&experience, &mut state, Intent::PreviousGroup); // library
    assert_eq!(state.active(), 0);
    assert_eq!(state.selection(), 3);
}

#[test]
fn enter_activates_the_selected_items_primary_action() {
    let experience = fixture();
    let mut state = PresentationState::new(&experience);
    apply_intent(&experience, &mut state, Intent::Next); // select alb-1
    let action = apply_intent(&experience, &mut state, Intent::Enter);
    assert_eq!(action, Some(Msg::Play("alb-1".into())));
    // Detail destination's Enter activates its action.
    apply_intent(&experience, &mut state, Intent::NextGroup); // now playing
    let action = apply_intent(&experience, &mut state, Intent::Enter);
    assert_eq!(action, Some(Msg::Open));
}

#[test]
fn every_destination_and_item_is_reachable_by_walking() {
    let experience = fixture();
    // Destinations: walking NextGroup must visit all of them.
    let mut state = PresentationState::new(&experience);
    let mut seen = vec![state.active()];
    for _ in 0..10 {
        apply_intent(&experience, &mut state, Intent::NextGroup);
        if !seen.contains(&state.active()) {
            seen.push(state.active());
        }
    }
    assert_eq!(seen.len(), experience.destinations.len());
    // Items: walking Next must visit every library item.
    let mut state = PresentationState::new(&experience);
    let mut visited = vec![state.selection()];
    for _ in 0..10 {
        apply_intent(&experience, &mut state, Intent::Next);
        if !visited.contains(&state.selection()) {
            visited.push(state.selection());
        }
    }
    assert_eq!(visited.len(), 5);
}

// ---------------------------------------------------------------------------
// Live style switching preserves semantic state (the central criterion, §24).
// ---------------------------------------------------------------------------

#[test]
fn switching_grammar_preserves_active_destination_and_selection() {
    let experience = fixture();
    let e = env(120, 40, ColorDepth::TrueColor);
    let mut state = PresentationState::new(&experience);
    // Select item 2 in the library under STANDARD.
    for _ in 0..2 {
        apply_intent(&experience, &mut state, Intent::Next);
    }
    let mut standard = Standard::new();
    let before = standard.present(&experience, &state, &e, Duration::ZERO);
    assert_eq!(
        before.receipt.selected,
        Some(gibson::ui::element::Key::named("alb-2"))
    );

    // Switch to a radically different grammar WITHOUT touching state.
    let mut minimal = Minimal;
    let after = minimal.present(&experience, &state, &e, Duration::ZERO);
    assert_eq!(
        after.receipt.active_destination,
        before.receipt.active_destination
    );
    assert_eq!(after.receipt.selected, before.receipt.selected);

    // Switch back; projection identical again.
    let back = standard.present(&experience, &state, &e, Duration::ZERO);
    assert_eq!(
        back.receipt.active_destination,
        before.receipt.active_destination
    );
    assert_eq!(back.receipt.selected, before.receipt.selected);
    // Both grammars satisfy the law at this state.
    let required = required_semantics(&experience, &state);
    assert!(after.receipt.preserves(&required));
    assert!(back.receipt.preserves(&required));
}

#[test]
fn different_grammars_bind_different_keys_to_the_same_intent() {
    let experience = fixture();
    // STANDARD: Down advances the item. MINIMAL: Right advances the item.
    let standard = Standard::new();
    let minimal = Minimal;

    let mut s1 = PresentationState::new(&experience);
    let msg = handle_key(&standard, &experience, &mut s1, &press(KeyCode::Down));
    assert_eq!(msg, None);
    assert_eq!(s1.selection(), 1);

    let mut s2 = PresentationState::new(&experience);
    let msg = handle_key(&minimal, &experience, &mut s2, &press(KeyCode::Right));
    assert_eq!(msg, None);
    assert_eq!(s2.selection(), 1);

    // STANDARD's Down and MINIMAL's Right produced the identical semantic effect.
    assert_eq!(s1.selection(), s2.selection());

    // And Enter through handle_key yields the typed application action.
    let msg = handle_key(&standard, &experience, &mut s1, &press(KeyCode::Enter));
    assert_eq!(msg, Some(Msg::Play("alb-1".into())));
}

// ---------------------------------------------------------------------------
// Determinism and capability/responsive naturality.
// ---------------------------------------------------------------------------

#[test]
fn presentation_is_deterministic_at_a_fixed_state_and_time() {
    let experience = fixture();
    let mut grammar = Standard::new();
    let e = env(120, 40, ColorDepth::TrueColor);
    let state = PresentationState::new(&experience);
    let a = grammar.present(&experience, &state, &e, Duration::ZERO);
    let b = grammar.present(&experience, &state, &e, Duration::ZERO);
    assert_eq!(a.receipt, b.receipt);
}

#[test]
fn standard_preserves_semantics_across_the_responsive_and_capability_matrix() {
    let experience = fixture();
    let mut grammar = Standard::new();
    let sizes = [(160, 50), (120, 40), (80, 24), (60, 20), (42, 15)];
    let depths = [
        ColorDepth::TrueColor,
        ColorDepth::Ansi256,
        ColorDepth::Ansi16,
        ColorDepth::Mono,
    ];
    for (w, h) in sizes {
        for depth in depths {
            let e = env(w, h, depth);
            let mut state = PresentationState::new(&experience);
            // Check the law in each destination at this size/capability.
            for _ in 0..experience.destinations.len() {
                let presented = grammar.present(&experience, &state, &e, Duration::ZERO);
                let required = required_semantics(&experience, &state);
                let violations = presented.receipt.check(&required);
                assert!(
                    violations.is_empty(),
                    "law violated at {w}x{h} {depth:?}: {violations:?}"
                );
                apply_intent(&experience, &mut state, Intent::NextGroup);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// MEDIA_SHELF — the first cinematic grammar. Validates the frozen IR end to end:
// the raster escape hatch, an animated (spring) grammar, and *lawful declared
// omission*.
// ---------------------------------------------------------------------------

use gibson::ui::experience::MediaShelf;

#[test]
fn media_shelf_preserves_semantics_in_every_destination() {
    let experience = fixture();
    let mut shelf = MediaShelf::new();
    let e = env(120, 40, ColorDepth::TrueColor);
    for destination in 0..experience.destinations.len() {
        let mut state = PresentationState::new(&experience);
        for _ in 0..destination {
            apply_intent(&experience, &mut state, Intent::NextGroup);
        }
        let presented = shelf.present(&experience, &state, &e, Duration::from_millis(500));
        let required = required_semantics(&experience, &state);
        let violations = presented.receipt.check(&required);
        assert!(
            violations.is_empty(),
            "MEDIA_SHELF violated the law at destination {destination}: {violations:?}"
        );
    }
}

#[test]
fn media_shelf_declared_omission_of_the_caption_is_lawful() {
    let experience = fixture();
    let mut shelf = MediaShelf::new();
    // A viewport too short to carry the action caption.
    let e = env(40, 8, ColorDepth::TrueColor);
    let state = PresentationState::new(&experience); // library, item 0 (has play-0)
    let presented = shelf.present(&experience, &state, &e, Duration::from_millis(500));
    // The action was dropped but DECLARED, so the law still holds.
    assert!(
        presented
            .receipt
            .omitted
            .contains(&gibson::ui::element::Key::named("play-0")),
        "expected the caption's action to be a declared omission; omitted={:?}",
        presented.receipt.omitted
    );
    let required = required_semantics(&experience, &state);
    assert!(
        presented.receipt.check(&required).is_empty(),
        "a *declared* omission must still satisfy the law: {:?}",
        presented.receipt.check(&required)
    );
}

#[test]
fn media_shelf_represents_the_action_when_there_is_room() {
    let experience = fixture();
    let mut shelf = MediaShelf::new();
    let e = env(120, 40, ColorDepth::TrueColor);
    let state = PresentationState::new(&experience);
    let presented = shelf.present(&experience, &state, &e, Duration::from_millis(500));
    assert!(presented
        .receipt
        .actions
        .contains(&gibson::ui::element::Key::named("play-0")));
    assert!(presented.receipt.omitted.is_empty());
}

#[test]
fn switching_to_media_shelf_preserves_selection() {
    let experience = fixture();
    let e = env(120, 40, ColorDepth::TrueColor);
    let mut state = PresentationState::new(&experience);
    for _ in 0..3 {
        apply_intent(&experience, &mut state, Intent::Next); // select alb-3
    }
    let mut standard = Standard::new();
    let before = standard.present(&experience, &state, &e, Duration::ZERO);
    let mut shelf = MediaShelf::new();
    let after = shelf.present(&experience, &state, &e, Duration::from_millis(16));
    assert_eq!(after.receipt.selected, before.receipt.selected);
    assert_eq!(
        after.receipt.active_destination,
        before.receipt.active_destination
    );
    assert_eq!(
        after.receipt.selected,
        Some(gibson::ui::element::Key::named("alb-3"))
    );
}

#[test]
fn media_shelf_binds_left_right_to_item_motion() {
    let experience = fixture();
    let shelf = MediaShelf::new();
    let mut state = PresentationState::new(&experience);
    // Right advances the item under the shelf (STANDARD used Down for this).
    let msg = handle_key(&shelf, &experience, &mut state, &press(KeyCode::Right));
    assert_eq!(msg, None);
    assert_eq!(state.selection(), 1);
    let msg = handle_key(&shelf, &experience, &mut state, &press(KeyCode::Left));
    assert_eq!(msg, None);
    assert_eq!(state.selection(), 0);
}

#[test]
fn media_shelf_spring_settles_on_the_selection() {
    let experience = fixture();
    let mut shelf = MediaShelf::new();
    let e = env(120, 40, ColorDepth::TrueColor);
    let mut state = PresentationState::new(&experience);
    for _ in 0..4 {
        apply_intent(&experience, &mut state, Intent::Next); // target item 4
    }
    // Drive ~2.5 s of frames at 60fps toward the fixed target.
    let mut t = 0u64;
    for _ in 0..150 {
        t += 16;
        shelf.present(&experience, &state, &e, Duration::from_millis(t));
    }
    assert!(
        shelf.is_settled(state.selection()),
        "spring should have settled on the selection after 2.5s"
    );
}

#[test]
fn media_shelf_preserves_semantics_across_the_responsive_and_capability_matrix() {
    let experience = fixture();
    let sizes = [(160, 50), (120, 40), (80, 24), (60, 20), (42, 15)];
    let depths = [
        ColorDepth::TrueColor,
        ColorDepth::Ansi256,
        ColorDepth::Ansi16,
        ColorDepth::Mono,
    ];
    for (w, h) in sizes {
        for depth in depths {
            let mut shelf = MediaShelf::new();
            let e = env(w, h, depth);
            let mut state = PresentationState::new(&experience);
            for _ in 0..experience.destinations.len() {
                let presented = shelf.present(&experience, &state, &e, Duration::from_millis(500));
                let required = required_semantics(&experience, &state);
                let violations = presented.receipt.check(&required);
                assert!(
                    violations.is_empty(),
                    "MEDIA_SHELF law violated at {w}x{h} {depth:?}: {violations:?}"
                );
                apply_intent(&experience, &mut state, Intent::NextGroup);
            }
        }
    }
}
