//! Experience-grammar freeze contracts (milestone §52): the semantic model,
//! identity-keyed navigation, the two-tier preservation law (bookkeeping +
//! rendered, and that both *bite*), per-destination selection retention, identity
//! stability under dynamic rebuilds, live style-switch preservation, reachability,
//! determinism, and capability/responsive naturality.

use gibson::capability::ColorDepth;
use gibson::input::{KeyCode, KeyEvent, KeyModifiers};
use gibson::ui::experience::{
    apply_intent, handle_key, required_semantics, Action, Content, Destination, Experience, Facet,
    Grammar, Intent, Item, LawViolation, Media, MediaShelf, PresentationReceipt, PresentationState,
    Presented, Priority, SemanticInput, Standard,
};
use gibson::ui::skin::UiEnvironment;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    Play(String),
    Toggle(String),
    Open,
}

fn key(name: &str) -> gibson::ui::element::Key {
    gibson::ui::element::Key::named(name)
}

/// A mixed-domain fixture: a media collection, a detail page, a settings
/// collection (settings as actionable items — not a separate node kind), and
/// prose. Exercises every content kind and essential/tertiary priority.
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

/// A bare single-collection experience whose item order is the given key list.
fn library(order: &[&str]) -> Experience<Msg> {
    let items: Vec<Item<Msg>> = order
        .iter()
        .map(|k| {
            Item::new(*k, *k).action(Action::new(
                format!("play-{k}"),
                "Play",
                Msg::Play((*k).to_string()),
            ))
        })
        .collect();
    Experience::new("LIB").destination(Destination::new(
        "library",
        "Library",
        Content::Collection(items),
    ))
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

fn sel(state: &PresentationState, experience: &Experience<Msg>) -> usize {
    state.selected_index(experience).unwrap_or(0)
}

fn act(state: &PresentationState, experience: &Experience<Msg>) -> usize {
    state.active_index(experience)
}

// ---------------------------------------------------------------------------
// Test grammars.
// ---------------------------------------------------------------------------

/// A faithful minimal grammar: renders a keyed node for every id it claims and
/// navigates with Left/Right (opposite axis from STANDARD, same intents). Used to
/// prove style switching preserves semantics with a genuinely different grammar.
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
        let active_idx = state.active_index(experience);
        let active = &experience.destinations[active_idx];
        let mut receipt = PresentationReceipt::new("MINIMAL", active.key.clone());
        let mut root = gibson::ui::element::column::<A>();
        for destination in &experience.destinations {
            receipt.destinations.push(destination.key.clone());
            root = root.child(
                gibson::ui::element::text::<A>(destination.title.clone())
                    .key(destination.key.to_string()),
            );
        }
        match &active.content {
            Content::Collection(items) => {
                let selected = state.selected_index(experience).unwrap_or(0);
                for (index, item) in items.iter().enumerate() {
                    receipt.items.push(item.key.clone());
                    root = root.child(
                        gibson::ui::element::text::<A>(item.title.clone())
                            .key(item.key.to_string()),
                    );
                    if index == selected {
                        receipt.selected = Some(item.key.clone());
                        if let Some(primary) = item.primary() {
                            receipt.actions.push(primary.key.clone());
                            root = root.child(
                                gibson::ui::element::text::<A>(primary.label.clone())
                                    .key(primary.key.to_string()),
                            );
                        }
                    }
                }
            }
            Content::Detail { actions, .. } => {
                if let Some(primary) = actions.first() {
                    receipt.actions.push(primary.key.clone());
                    root = root.child(
                        gibson::ui::element::text::<A>(primary.label.clone())
                            .key(primary.key.to_string()),
                    );
                }
            }
            _ => {}
        }
        Presented {
            element: root,
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

/// A DISHONEST grammar: its receipt claims everything, but it renders only the
/// experience title (no keyed nodes, nothing rastered). Used to prove the
/// rendered tier of the law catches a receipt describing a render never made.
struct TitleOnly;

impl<A: Clone> Grammar<A> for TitleOnly {
    fn name(&self) -> &'static str {
        "TITLE_ONLY"
    }
    fn present(
        &mut self,
        experience: &Experience<A>,
        state: &PresentationState,
        _env: &UiEnvironment,
        _now: Duration,
    ) -> Presented<A> {
        let active_idx = state.active_index(experience);
        let active = &experience.destinations[active_idx];
        let mut receipt = PresentationReceipt::new("TITLE_ONLY", active.key.clone());
        for destination in &experience.destinations {
            receipt.destinations.push(destination.key.clone());
        }
        if let Content::Collection(items) = &active.content {
            let selected = state.selected_index(experience).unwrap_or(0);
            for item in items {
                receipt.items.push(item.key.clone());
            }
            if let Some(item) = items.get(selected) {
                receipt.selected = Some(item.key.clone());
                if let Some(primary) = item.primary() {
                    receipt.actions.push(primary.key.clone());
                }
            }
        }
        Presented {
            element: gibson::ui::element::text::<A>(experience.title.clone()),
            receipt,
        }
    }
    fn interpret(
        &self,
        _key: &KeyEvent,
        _experience: &Experience<A>,
        _state: &PresentationState,
    ) -> Option<SemanticInput<A>> {
        None
    }
}

// ---------------------------------------------------------------------------
// The preservation law — both tiers, and that they bite.
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
        let violations = presented.check(&required);
        assert!(
            violations.is_empty(),
            "destination {destination} violated the law: {violations:?}"
        );
    }
}

#[test]
fn required_action_of_the_selected_item_is_rendered() {
    let experience = fixture();
    let mut grammar = Standard::new();
    let e = env(120, 40, ColorDepth::TrueColor);
    for index in 0..5 {
        let mut state = PresentationState::new(&experience);
        for _ in 0..index {
            apply_intent(&experience, &mut state, Intent::Next);
        }
        let presented = grammar.present(&experience, &state, &e, Duration::ZERO);
        let expected = key(&format!("play-{index}"));
        assert!(
            presented.receipt.actions.contains(&expected),
            "selected item {index} did not surface its play action; actions={:?}",
            presented.receipt.actions
        );
        let required = required_semantics(&experience, &state);
        assert!(presented.check(&required).is_empty());
    }
}

#[test]
fn the_law_catches_a_silently_dropped_item() {
    let experience = fixture();
    let state = PresentationState::new(&experience);
    let required = required_semantics(&experience, &state);
    let mut receipt = PresentationReceipt::new("DISHONEST", key("library"));
    receipt.destinations = experience
        .destinations
        .iter()
        .map(|d| d.key.clone())
        .collect();
    receipt.items = vec![key("alb-0"), key("alb-1")]; // dropped alb-2..4, undeclared
    receipt.selected = Some(key("alb-0"));
    receipt.actions = vec![key("play-0")];
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
    let mut receipt = PresentationReceipt::new("DISHONEST", key("library"));
    receipt.destinations = experience
        .destinations
        .iter()
        .map(|d| d.key.clone())
        .collect();
    receipt.items = vec![key("alb-1"), key("alb-2"), key("alb-3"), key("alb-4")];
    receipt.omitted = vec![key("alb-0")]; // alb-0 is Essential — may never be omitted
    receipt.selected = Some(key("alb-1"));
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
    let mut receipt = PresentationReceipt::new("DISHONEST", key("library"));
    receipt.destinations = experience
        .destinations
        .iter()
        .map(|d| d.key.clone())
        .collect();
    receipt.items = vec![
        key("alb-0"),
        key("alb-1"),
        key("alb-2"),
        key("alb-3"),
        key("alb-4"),
        key("ghost-item"), // invented
    ];
    receipt.selected = Some(key("alb-0"));
    receipt.actions = vec![key("play-0")];
    let violations = receipt.check(&required);
    assert!(
        violations
            .iter()
            .any(|v| matches!(v, LawViolation::Invented(_))),
        "expected Invented, got {violations:?}"
    );
}

#[test]
fn the_rendered_law_catches_a_grammar_that_draws_only_a_title() {
    let experience = fixture();
    let state = PresentationState::new(&experience);
    let required = required_semantics(&experience, &state);
    let mut liar = TitleOnly;
    let presented = liar.present(
        &experience,
        &state,
        &env(120, 40, ColorDepth::TrueColor),
        Duration::ZERO,
    );
    // Its bookkeeping is internally consistent — the ledger looks honest...
    assert!(
        presented.receipt.check(&required).is_empty(),
        "bookkeeping tier should pass for a self-consistent ledger"
    );
    // ...but the RENDERED tier catches it: it claims ids it never drew.
    let violations = presented.check(&required);
    assert!(
        violations
            .iter()
            .any(|v| matches!(v, LawViolation::UnrenderedClaim(_))),
        "expected UnrenderedClaim from the rendered law, got {violations:?}"
    );
}

// ---------------------------------------------------------------------------
// Navigation semantics.
// ---------------------------------------------------------------------------

#[test]
fn item_navigation_clamps_and_never_teleports() {
    let experience = fixture();
    let mut state = PresentationState::new(&experience); // library, 5 items
    assert_eq!(sel(&state, &experience), 0);
    apply_intent(&experience, &mut state, Intent::Previous); // clamp at 0
    assert_eq!(sel(&state, &experience), 0);
    for _ in 0..10 {
        apply_intent(&experience, &mut state, Intent::Next); // clamp at 4
    }
    assert_eq!(sel(&state, &experience), 4);
    apply_intent(&experience, &mut state, Intent::Home);
    assert_eq!(sel(&state, &experience), 0);
    apply_intent(&experience, &mut state, Intent::End);
    assert_eq!(sel(&state, &experience), 4);
}

#[test]
fn rapid_reversal_stays_coherent() {
    let experience = fixture();
    let mut state = PresentationState::new(&experience);
    for intent in [
        Intent::Next,
        Intent::Next,
        Intent::Next,
        Intent::Previous,
        Intent::Next,
        Intent::Previous,
        Intent::Previous,
    ] {
        apply_intent(&experience, &mut state, intent);
    }
    // +1 +1 +1 -1 +1 -1 -1 = 1
    assert_eq!(sel(&state, &experience), 1);
}

#[test]
fn selection_is_retained_per_destination() {
    let experience = fixture();
    let mut state = PresentationState::new(&experience);
    for _ in 0..3 {
        apply_intent(&experience, &mut state, Intent::Next); // library item 3
    }
    assert_eq!(sel(&state, &experience), 3);
    apply_intent(&experience, &mut state, Intent::NextGroup); // now
    apply_intent(&experience, &mut state, Intent::NextGroup); // settings
    apply_intent(&experience, &mut state, Intent::Next);
    assert_eq!(sel(&state, &experience), 1);
    apply_intent(&experience, &mut state, Intent::PreviousGroup); // now
    apply_intent(&experience, &mut state, Intent::PreviousGroup); // library
    assert_eq!(act(&state, &experience), 0);
    assert_eq!(sel(&state, &experience), 3); // retained
}

#[test]
fn enter_activates_the_selected_items_primary_action() {
    let experience = fixture();
    let mut state = PresentationState::new(&experience);
    apply_intent(&experience, &mut state, Intent::Next); // select alb-1
    let action = apply_intent(&experience, &mut state, Intent::Enter);
    assert_eq!(action, Some(Msg::Play("alb-1".into())));
    apply_intent(&experience, &mut state, Intent::NextGroup); // now playing
    let action = apply_intent(&experience, &mut state, Intent::Enter);
    assert_eq!(action, Some(Msg::Open));
}

#[test]
fn every_destination_and_item_is_reachable_by_walking() {
    let experience = fixture();
    let mut state = PresentationState::new(&experience);
    let mut seen = vec![act(&state, &experience)];
    for _ in 0..10 {
        apply_intent(&experience, &mut state, Intent::NextGroup);
        let a = act(&state, &experience);
        if !seen.contains(&a) {
            seen.push(a);
        }
    }
    assert_eq!(seen.len(), experience.destinations.len());

    let mut state = PresentationState::new(&experience);
    let mut visited = vec![sel(&state, &experience)];
    for _ in 0..10 {
        apply_intent(&experience, &mut state, Intent::Next);
        let s = sel(&state, &experience);
        if !visited.contains(&s) {
            visited.push(s);
        }
    }
    assert_eq!(visited.len(), 5);
}

// ---------------------------------------------------------------------------
// Identity stability under dynamic rebuilds (the F2/F3 adversary findings).
// ---------------------------------------------------------------------------

#[test]
fn selection_resolves_consistently_after_a_collection_shrinks() {
    let full = library(&["a", "b", "c", "d", "e"]);
    let mut state = PresentationState::new(&full);
    apply_intent(&full, &mut state, Intent::End); // select "e"
    assert_eq!(state.selected_key(&full), Some(key("e")));

    let shrunk = library(&["a", "b"]); // "e" no longer exists
    let required = required_semantics(&shrunk, &state);
    let e = env(120, 40, ColorDepth::TrueColor);

    // required_semantics and BOTH grammars read the one shared clamping rule, so
    // they agree on the selection — no divergence, no silent None, no honest
    // grammar flagged a liar (the F2 failure is gone).
    let mut standard = Standard::new();
    let p_std = standard.present(&shrunk, &state, &e, Duration::ZERO);
    let mut shelf = MediaShelf::new();
    let p_shelf = shelf.present(&shrunk, &state, &e, Duration::from_millis(500));
    assert_eq!(p_std.receipt.selected, required.selected);
    assert_eq!(p_shelf.receipt.selected, required.selected);
    assert!(
        p_std.check(&required).is_empty(),
        "{:?}",
        p_std.check(&required)
    );
    assert!(
        p_shelf.check(&required).is_empty(),
        "{:?}",
        p_shelf.check(&required)
    );
}

#[test]
fn selection_tracks_identity_across_a_reorder() {
    let before = library(&["a", "b", "c", "d", "e"]);
    let mut state = PresentationState::new(&before);
    for _ in 0..2 {
        apply_intent(&before, &mut state, Intent::Next); // select "c"
    }
    assert_eq!(state.selected_key(&before), Some(key("c")));
    let reordered = library(&["c", "a", "b", "d", "e"]);
    assert_eq!(state.selected_key(&reordered), Some(key("c")));
    assert_eq!(sel(&state, &reordered), 0); // "c" is now at index 0
}

#[test]
fn active_destination_tracks_identity_across_an_insert() {
    let base = Experience::<Msg>::new("X")
        .destination(Destination::new("library", "L", Content::Prose(vec![])))
        .destination(Destination::new("settings", "S", Content::Prose(vec![])));
    let mut state = PresentationState::new(&base);
    apply_intent(&base, &mut state, Intent::NextGroup); // active = settings
    assert_eq!(state.active_key(&base), Some(key("settings")));
    let shifted = Experience::<Msg>::new("X")
        .destination(Destination::new("ghost", "G", Content::Prose(vec![])))
        .destination(Destination::new("library", "L", Content::Prose(vec![])))
        .destination(Destination::new("settings", "S", Content::Prose(vec![])));
    assert_eq!(state.active_key(&shifted), Some(key("settings")));
    assert_eq!(act(&state, &shifted), 2); // settings is now at index 2
}

// ---------------------------------------------------------------------------
// Live style switching preserves semantic state (the central criterion, §24).
// ---------------------------------------------------------------------------

#[test]
fn switching_grammar_preserves_active_destination_and_selection() {
    let experience = fixture();
    let e = env(120, 40, ColorDepth::TrueColor);
    let mut state = PresentationState::new(&experience);
    for _ in 0..2 {
        apply_intent(&experience, &mut state, Intent::Next); // select alb-2
    }
    let mut standard = Standard::new();
    let before = standard.present(&experience, &state, &e, Duration::ZERO);
    assert_eq!(before.receipt.selected, Some(key("alb-2")));

    let mut minimal = Minimal;
    let after = minimal.present(&experience, &state, &e, Duration::ZERO);
    assert_eq!(
        after.receipt.active_destination,
        before.receipt.active_destination
    );
    assert_eq!(after.receipt.selected, before.receipt.selected);

    let back = standard.present(&experience, &state, &e, Duration::ZERO);
    assert_eq!(
        back.receipt.active_destination,
        before.receipt.active_destination
    );
    assert_eq!(back.receipt.selected, before.receipt.selected);

    let required = required_semantics(&experience, &state);
    assert!(after.preserves(&required), "{:?}", after.check(&required));
    assert!(back.preserves(&required), "{:?}", back.check(&required));
}

#[test]
fn different_grammars_bind_different_keys_to_the_same_intent() {
    let experience = fixture();
    let standard = Standard::new();
    let minimal = Minimal;

    let mut s1 = PresentationState::new(&experience);
    let msg = handle_key(&standard, &experience, &mut s1, &press(KeyCode::Down));
    assert_eq!(msg, None);
    assert_eq!(sel(&s1, &experience), 1);

    let mut s2 = PresentationState::new(&experience);
    let msg = handle_key(&minimal, &experience, &mut s2, &press(KeyCode::Right));
    assert_eq!(msg, None);
    assert_eq!(sel(&s2, &experience), 1);

    assert_eq!(sel(&s1, &experience), sel(&s2, &experience));

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
            for _ in 0..experience.destinations.len() {
                let presented = grammar.present(&experience, &state, &e, Duration::ZERO);
                let required = required_semantics(&experience, &state);
                let violations = presented.check(&required);
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
// MEDIA_SHELF — the first cinematic grammar.
// ---------------------------------------------------------------------------

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
        let violations = presented.check(&required);
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
    let e = env(40, 8, ColorDepth::TrueColor); // too short for the caption
    let state = PresentationState::new(&experience);
    let presented = shelf.present(&experience, &state, &e, Duration::from_millis(500));
    assert!(
        presented.receipt.omitted.contains(&key("play-0")),
        "expected the caption's action to be a declared omission; omitted={:?}",
        presented.receipt.omitted
    );
    let required = required_semantics(&experience, &state);
    assert!(
        presented.check(&required).is_empty(),
        "a declared omission must still satisfy the law: {:?}",
        presented.check(&required)
    );
}

#[test]
fn media_shelf_renders_the_action_when_there_is_room() {
    let experience = fixture();
    let mut shelf = MediaShelf::new();
    let e = env(120, 40, ColorDepth::TrueColor);
    let state = PresentationState::new(&experience);
    let presented = shelf.present(&experience, &state, &e, Duration::from_millis(500));
    assert!(presented.receipt.actions.contains(&key("play-0")));
    assert!(presented.receipt.omitted.is_empty());
    let required = required_semantics(&experience, &state);
    assert!(presented.check(&required).is_empty());
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
    assert_eq!(after.receipt.selected, Some(key("alb-3")));
}

#[test]
fn media_shelf_binds_left_right_to_item_motion() {
    let experience = fixture();
    let shelf = MediaShelf::new();
    let mut state = PresentationState::new(&experience);
    let msg = handle_key(&shelf, &experience, &mut state, &press(KeyCode::Right));
    assert_eq!(msg, None);
    assert_eq!(sel(&state, &experience), 1);
    let msg = handle_key(&shelf, &experience, &mut state, &press(KeyCode::Left));
    assert_eq!(msg, None);
    assert_eq!(sel(&state, &experience), 0);
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
    let mut t = 0u64;
    for _ in 0..150 {
        t += 16;
        shelf.present(&experience, &state, &e, Duration::from_millis(t));
    }
    assert!(
        shelf.is_settled(sel(&state, &experience)),
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
                let violations = presented.check(&required);
                assert!(
                    violations.is_empty(),
                    "MEDIA_SHELF law violated at {w}x{h} {depth:?}: {violations:?}"
                );
                apply_intent(&experience, &mut state, Intent::NextGroup);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CROSS_MEDIA and the all-grammars parametric contracts.
// ---------------------------------------------------------------------------

use gibson::ui::experience::{Blades, CrossMedia, Orbital, Panorama};

fn all_grammars() -> Vec<(&'static str, Box<dyn Grammar<Msg>>)> {
    vec![
        ("STANDARD", Box::new(Standard::new())),
        ("MEDIA_SHELF", Box::new(MediaShelf::new())),
        ("CROSS_MEDIA", Box::new(CrossMedia::new())),
        ("ORBITAL", Box::new(Orbital::new())),
        ("PANORAMA", Box::new(Panorama::new())),
        ("BLADES", Box::new(Blades::new())),
    ]
}

#[test]
fn every_grammar_preserves_semantics_across_destinations_and_the_matrix() {
    let experience = fixture();
    let sizes = [(160, 50), (120, 40), (80, 24), (60, 20), (42, 15)];
    let depths = [
        ColorDepth::TrueColor,
        ColorDepth::Ansi256,
        ColorDepth::Ansi16,
        ColorDepth::Mono,
    ];
    for (name, mut grammar) in all_grammars() {
        for (w, h) in sizes {
            for depth in depths {
                let e = env(w, h, depth);
                let mut state = PresentationState::new(&experience);
                for _ in 0..experience.destinations.len() {
                    let presented =
                        grammar.present(&experience, &state, &e, Duration::from_millis(500));
                    let required = required_semantics(&experience, &state);
                    let violations = presented.check(&required);
                    assert!(
                        violations.is_empty(),
                        "{name} violated the law at {w}x{h} {depth:?}: {violations:?}"
                    );
                    apply_intent(&experience, &mut state, Intent::NextGroup);
                }
            }
        }
    }
}

#[test]
fn every_grammar_preserves_selection_on_a_live_switch() {
    let experience = fixture();
    let e = env(120, 40, ColorDepth::TrueColor);
    let mut state = PresentationState::new(&experience);
    for _ in 0..2 {
        apply_intent(&experience, &mut state, Intent::Next); // select alb-2
    }
    // The same semantic state rendered by every grammar keeps identity.
    for (name, mut grammar) in all_grammars() {
        let presented = grammar.present(&experience, &state, &e, Duration::from_millis(16));
        assert_eq!(
            presented.receipt.selected,
            Some(key("alb-2")),
            "{name} lost the selection"
        );
        assert_eq!(
            presented.receipt.active_destination,
            key("library"),
            "{name} moved the active destination"
        );
    }
}

#[test]
fn cross_media_binds_the_cross_axes() {
    let experience = fixture();
    let cm = CrossMedia::new();
    let mut state = PresentationState::new(&experience);
    // Right/Left move along the destination bar.
    handle_key(&cm, &experience, &mut state, &press(KeyCode::Right));
    assert_eq!(act(&state, &experience), 1);
    handle_key(&cm, &experience, &mut state, &press(KeyCode::Left));
    assert_eq!(act(&state, &experience), 0);
    // Up/Down move along the item column.
    handle_key(&cm, &experience, &mut state, &press(KeyCode::Down));
    assert_eq!(sel(&state, &experience), 1);
    handle_key(&cm, &experience, &mut state, &press(KeyCode::Up));
    assert_eq!(sel(&state, &experience), 0);
}

// ---------------------------------------------------------------------------
// PANORAMA — a rendered world (GL sky + focal cover + rasterised menu) with the
// UI text baked into the surface; every semantic id is attested rastered.
// ---------------------------------------------------------------------------

#[test]
fn panorama_preserves_semantics_across_destinations_and_matrix() {
    let experience = fixture();
    let sizes = [(160, 50), (120, 40), (80, 24), (60, 20), (42, 15)];
    let depths = [
        ColorDepth::TrueColor,
        ColorDepth::Ansi256,
        ColorDepth::Ansi16,
        ColorDepth::Mono,
    ];
    let mut grammar = Panorama::new();
    assert_eq!(Grammar::<Msg>::name(&grammar), "PANORAMA");
    for (w, h) in sizes {
        for depth in depths {
            let e = env(w, h, depth);
            let mut state = PresentationState::new(&experience);
            // Walk every destination, and a non-default selection inside the library.
            for _ in 0..experience.destinations.len() {
                for _ in 0..2 {
                    let presented =
                        grammar.present(&experience, &state, &e, Duration::from_millis(500));
                    let required = required_semantics(&experience, &state);
                    let violations = presented.check(&required);
                    assert!(
                        violations.is_empty(),
                        "PANORAMA violated the law at {w}x{h} {depth:?}: {violations:?}"
                    );
                    // PANORAMA renders its UI into one raster surface, so every
                    // destination it represents is attested rastered (the law above
                    // already accepts keyed-node OR rastered; this pins the design).
                    assert!(
                        experience
                            .destinations
                            .iter()
                            .all(|d| presented.receipt.rastered.contains(&d.key)),
                        "every destination must be attested rastered at {w}x{h} {depth:?}"
                    );
                    apply_intent(&experience, &mut state, Intent::Next);
                }
                apply_intent(&experience, &mut state, Intent::NextGroup);
            }
        }
    }
}

#[test]
fn panorama_binds_horizontal_pan_and_vertical_items() {
    let experience = fixture();
    let pano = Panorama::new();
    let mut state = PresentationState::new(&experience);
    // Left/Right pan between sections (destinations).
    handle_key(&pano, &experience, &mut state, &press(KeyCode::Right));
    assert_eq!(act(&state, &experience), 1);
    handle_key(&pano, &experience, &mut state, &press(KeyCode::Left));
    assert_eq!(act(&state, &experience), 0);
    // Up/Down move within the item column.
    handle_key(&pano, &experience, &mut state, &press(KeyCode::Down));
    assert_eq!(sel(&state, &experience), 1);
    handle_key(&pano, &experience, &mut state, &press(KeyCode::Up));
    assert_eq!(sel(&state, &experience), 0);
    // End/Home jump within the column; Enter activates the primary action.
    handle_key(&pano, &experience, &mut state, &press(KeyCode::End));
    assert_eq!(sel(&state, &experience), 4);
    let msg = handle_key(&pano, &experience, &mut state, &press(KeyCode::Enter));
    assert_eq!(msg, Some(Msg::Play("alb-4".into())));
    handle_key(&pano, &experience, &mut state, &press(KeyCode::Home));
    assert_eq!(sel(&state, &experience), 0);
    // Esc/Backspace are Back; an unbound key is not interpreted.
    assert!(matches!(
        pano.interpret(&press(KeyCode::Esc), &experience, &state),
        Some(SemanticInput::Navigate(Intent::Back))
    ));
    assert!(matches!(
        pano.interpret(&press(KeyCode::Backspace), &experience, &state),
        Some(SemanticInput::Navigate(Intent::Back))
    ));
    assert!(pano
        .interpret(&press(KeyCode::Char('z')), &experience, &state)
        .is_none());
}

// ---------------------------------------------------------------------------
// ORBITAL — the focal, radial raster grammar.
// ---------------------------------------------------------------------------

#[test]
fn orbital_preserves_semantics_across_destinations_and_matrix() {
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
            let mut orbital = Orbital::new();
            let e = env(w, h, depth);
            let mut state = PresentationState::new(&experience);
            for _ in 0..experience.destinations.len() {
                let presented =
                    orbital.present(&experience, &state, &e, Duration::from_millis(500));
                let required = required_semantics(&experience, &state);
                let violations = presented.check(&required);
                assert!(
                    violations.is_empty(),
                    "ORBITAL law violated at {w}x{h} {depth:?}: {violations:?}"
                );
                apply_intent(&experience, &mut state, Intent::NextGroup);
            }
        }
    }
}

#[test]
fn orbital_attests_its_orbs_and_declares_the_caption_omission_when_short() {
    let experience = fixture();
    let mut orbital = Orbital::new();
    let state = PresentationState::new(&experience);
    let required = required_semantics(&experience, &state);

    // Roomy: every item is attested rastered; the primary action is a keyed node.
    let roomy = orbital.present(
        &experience,
        &state,
        &env(80, 24, ColorDepth::TrueColor),
        Duration::ZERO,
    );
    for item in &required.all_items {
        assert!(
            roomy.receipt.rastered.contains(item),
            "{item:?} not attested"
        );
    }
    assert!(roomy.receipt.actions.contains(&key("play-0")));
    assert!(roomy.receipt.omitted.is_empty());
    assert!(roomy.check(&required).is_empty());

    // Too short for the caption: the action is *declared* omitted — lawful, never silent.
    let short = orbital.present(
        &experience,
        &state,
        &env(40, 8, ColorDepth::Mono),
        Duration::ZERO,
    );
    assert!(short.receipt.omitted.contains(&key("play-0")));
    assert!(short.check(&required).is_empty());
}

#[test]
fn orbital_navigation_rotates_the_ring() {
    let experience = fixture();
    let orbital = Orbital::new();
    let mut state = PresentationState::new(&experience);
    // Left/Right turn the ring; an orbit has no end, so the ring WRAPS (cyclic),
    // unlike a shelf or a list.
    handle_key(&orbital, &experience, &mut state, &press(KeyCode::Right));
    assert_eq!(sel(&state, &experience), 1);
    handle_key(&orbital, &experience, &mut state, &press(KeyCode::Right));
    assert_eq!(sel(&state, &experience), 2);
    handle_key(&orbital, &experience, &mut state, &press(KeyCode::Left));
    assert_eq!(sel(&state, &experience), 1);
    handle_key(&orbital, &experience, &mut state, &press(KeyCode::Home));
    assert_eq!(sel(&state, &experience), 0);
    handle_key(&orbital, &experience, &mut state, &press(KeyCode::Left));
    assert_eq!(
        sel(&state, &experience),
        4,
        "an orbit wraps: Left past the first lands on the last"
    );
    handle_key(&orbital, &experience, &mut state, &press(KeyCode::Right));
    assert_eq!(
        sel(&state, &experience),
        0,
        "and Right past the last wraps back to the first"
    );
    handle_key(&orbital, &experience, &mut state, &press(KeyCode::End));
    assert_eq!(sel(&state, &experience), 4);
    // Up/Down change destination; Enter activates the primary action; Esc is Back.
    handle_key(&orbital, &experience, &mut state, &press(KeyCode::Down));
    assert_eq!(act(&state, &experience), 1);
    handle_key(&orbital, &experience, &mut state, &press(KeyCode::Up));
    assert_eq!(act(&state, &experience), 0);
    let msg = handle_key(&orbital, &experience, &mut state, &press(KeyCode::Enter));
    assert_eq!(msg, Some(Msg::Play("alb-4".into())));

    // The rotation is observable in the painted ring: the world at selection 0 and
    // at selection 3 differ, and each is deterministic at a fixed state and time.
    let e = env(80, 24, ColorDepth::TrueColor);
    let first = PresentationState::new(&experience);
    let mut third = PresentationState::new(&experience);
    for _ in 0..3 {
        apply_intent(&experience, &mut third, Intent::Next);
    }
    let t = Duration::from_millis(250);
    let at0 = paint_orbital(&experience, &first, &e, t);
    let at3 = paint_orbital(&experience, &third, &e, t);
    assert_ne!(at0, at3, "turning the ring must change the picture");
    assert_eq!(
        at0,
        paint_orbital(&experience, &first, &e, t),
        "ORBITAL is deterministic at a fixed state and time"
    );
}

/// The clamp-vs-wrap contract lives in `apply_intent`, grammar-independent: the
/// plain item intents clamp at the ends, the cyclic ones wrap. A grammar chooses
/// which pair to emit (a shelf clamps, an orbit wraps); the law is the same here.
#[test]
fn cyclic_intents_wrap_where_plain_intents_clamp() {
    let experience = fixture();
    let mut state = PresentationState::new(&experience);
    for _ in 0..10 {
        apply_intent(&experience, &mut state, Intent::Next);
    }
    assert_eq!(sel(&state, &experience), 4, "walked to the last item");
    apply_intent(&experience, &mut state, Intent::Next);
    assert_eq!(sel(&state, &experience), 4, "Next clamps at the last item");
    apply_intent(&experience, &mut state, Intent::NextCyclic);
    assert_eq!(
        sel(&state, &experience),
        0,
        "NextCyclic wraps past the last to the first"
    );
    apply_intent(&experience, &mut state, Intent::PreviousCyclic);
    assert_eq!(
        sel(&state, &experience),
        4,
        "PreviousCyclic wraps past the first to the last"
    );
}

/// Run a fresh ORBITAL frame through the real compile -> layout -> paint path (which
/// also rejects duplicate keys), returning the painted surface.
fn paint_orbital(
    experience: &Experience<Msg>,
    state: &PresentationState,
    e: &UiEnvironment,
    now: Duration,
) -> gibson::Surface {
    let presented = Orbital::new().present(experience, state, e, now);
    let cx = gibson::ui::BuildCx::new(gibson::ui::skins::VAPOR95, *e);
    let mut node = gibson::ui::compile(&presented.element, &cx)
        .expect("ORBITAL lowers to a valid element tree")
        .node;
    gibson::compute_layout(&mut node, e.width, e.height).unwrap();
    let mut surface = gibson::Surface::new(e.width, e.height);
    gibson::paint(&node, &mut surface);
    surface
}

// ---------------------------------------------------------------------------
// BLADES — the depth-plane grammar.
// ---------------------------------------------------------------------------

#[test]
fn blades_preserves_semantics_across_destinations_and_matrix() {
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
            let mut blades = Blades::new();
            let e = env(w, h, depth);
            let mut state = PresentationState::new(&experience);
            // Walk every destination; at each, walk every item, so the rendered law
            // is checked for every (destination, selection) pair — including the
            // occluded blades, which must stay represented while hidden.
            let mut now = 0u64;
            for destination in 0..experience.destinations.len() {
                let items = experience.destinations[destination]
                    .content
                    .selectable_len();
                for step in 0..items.max(1) {
                    // Mid-glide frames (the stack still sliding) must be faithful too.
                    for _ in 0..3 {
                        now += 40;
                        let presented =
                            blades.present(&experience, &state, &e, Duration::from_millis(now));
                        let required = required_semantics(&experience, &state);
                        let violations = presented.check(&required);
                        assert!(
                            violations.is_empty(),
                            "BLADES law violated at {w}x{h} {depth:?} dest {destination} \
                             item {step}: {violations:?}"
                        );
                        assert_eq!(presented.receipt.style, "BLADES");
                    }
                    apply_intent(&experience, &mut state, Intent::Next);
                }
                apply_intent(&experience, &mut state, Intent::NextGroup);
            }
        }
    }
}

#[test]
fn blades_navigation_moves_the_stack_and_items() {
    let experience = fixture();
    let mut blades = Blades::new();
    let mut state = PresentationState::new(&experience);
    let e = env(120, 40, ColorDepth::TrueColor);

    // The stack is vertical: Down/Up change blade (destination)...
    handle_key(&blades, &experience, &mut state, &press(KeyCode::Down));
    assert_eq!(act(&state, &experience), 1);
    handle_key(&blades, &experience, &mut state, &press(KeyCode::Up));
    assert_eq!(act(&state, &experience), 0);
    // ...and Right/Left move within the foremost blade's items.
    handle_key(&blades, &experience, &mut state, &press(KeyCode::Right));
    assert_eq!(sel(&state, &experience), 1);
    handle_key(&blades, &experience, &mut state, &press(KeyCode::Left));
    assert_eq!(sel(&state, &experience), 0);
    handle_key(&blades, &experience, &mut state, &press(KeyCode::End));
    assert_eq!(sel(&state, &experience), 4);
    handle_key(&blades, &experience, &mut state, &press(KeyCode::Home));
    assert_eq!(sel(&state, &experience), 0);
    // Enter hands back the primary action; Esc/Backspace are Back (a no-op at top).
    let msg = handle_key(&blades, &experience, &mut state, &press(KeyCode::Enter));
    assert_eq!(msg, Some(Msg::Play("alb-0".into())));
    assert_eq!(
        handle_key(&blades, &experience, &mut state, &press(KeyCode::Esc)),
        None
    );
    assert_eq!(
        handle_key(&blades, &experience, &mut state, &press(KeyCode::Backspace)),
        None
    );
    assert!(blades
        .interpret(&press(KeyCode::Char('x')), &experience, &state)
        .is_none());

    // The stack genuinely moves: settled at the start, mid-glide right after a
    // destination change, settled again on the new destination once time passes.
    let mut now = 0u64;
    blades.present(&experience, &state, &e, Duration::from_millis(now));
    assert!(
        blades.is_settled(0),
        "first frame starts on the active blade"
    );
    handle_key(&blades, &experience, &mut state, &press(KeyCode::Down));
    handle_key(&blades, &experience, &mut state, &press(KeyCode::Down));
    assert_eq!(act(&state, &experience), 2);
    now += 16;
    blades.present(&experience, &state, &e, Duration::from_millis(now));
    assert!(!blades.is_settled(2), "the stack should be mid-glide");
    for _ in 0..200 {
        now += 16;
        blades.present(&experience, &state, &e, Duration::from_millis(now));
    }
    assert!(blades.is_settled(2), "the stack should settle on the blade");

    // Reversal mid-glide stays coherent: the semantic state, not the animation,
    // decides what is active.
    handle_key(&blades, &experience, &mut state, &press(KeyCode::Up));
    now += 16;
    let presented = blades.present(&experience, &state, &e, Duration::from_millis(now));
    assert_eq!(presented.receipt.active_destination, key("now"));
    assert!(presented
        .check(&required_semantics(&experience, &state))
        .is_empty());
}

#[test]
fn blades_declares_the_action_omitted_when_the_viewport_is_too_short() {
    let experience = fixture();
    let mut blades = Blades::new();
    let state = PresentationState::new(&experience);
    let e = env(80, 8, ColorDepth::TrueColor);
    let presented = blades.present(&experience, &state, &e, Duration::from_millis(16));
    let required = required_semantics(&experience, &state);
    assert!(presented.check(&required).is_empty());
    assert!(presented.receipt.omitted.contains(&key("play-0")));
    assert!(!presented.receipt.actions.contains(&key("play-0")));
    // Every destination and item is still attested while the action is declared.
    for destination in &experience.destinations {
        assert!(presented.receipt.rastered.contains(&destination.key));
    }
}

// ---------------------------------------------------------------------------
// Domain independence (§46): a SECOND fixture from an entirely different
// domain — a system console, with NO Media whatsoever — run through every
// grammar. If any grammar secretly assumed "albums" (cover art, media seeds,
// media-shaped collections), it would either misrender or trip the law here.
// The raster grammars must fall back to seeding their procedural art from the
// item key, and all four Content kinds must survive.
// ---------------------------------------------------------------------------

/// A system-operations console: services (collection, no media), a metrics
/// inspector (detail), live logs (prose), and config toggles (collection).
fn console_fixture() -> Experience<Msg> {
    let services = Content::Collection(vec![
        Item::new("svc-gateway", "api-gateway")
            .subtitle("healthy · 3 replicas")
            .priority(Priority::Essential)
            .action(Action::new(
                "restart-gw",
                "Restart",
                Msg::Play("svc-gateway".into()),
            )),
        Item::new("svc-ledger", "ledger")
            .subtitle("degraded · 1/2 replicas")
            .priority(Priority::Essential)
            .action(Action::new(
                "restart-ld",
                "Restart",
                Msg::Play("svc-ledger".into()),
            )),
        Item::new("svc-cache", "cache")
            .subtitle("healthy")
            .action(Action::new(
                "restart-ca",
                "Restart",
                Msg::Play("svc-cache".into()),
            )),
        Item::new("svc-worker", "batch-worker")
            .subtitle("idle")
            .priority(Priority::Tertiary)
            .action(Action::new(
                "restart-wk",
                "Restart",
                Msg::Play("svc-worker".into()),
            )),
    ]);

    let metrics = Content::Detail {
        facets: vec![
            Facet::new("rps", "Requests/s", "12,480"),
            Facet::new("p99", "p99 latency", "84ms"),
            Facet::new("err", "Error rate", "0.02%"),
        ],
        actions: vec![Action::new("open-dash", "Dashboard", Msg::Open)],
    };

    let logs = Content::Prose(vec![
        "12:04:01 gateway: upstream reconnect ok".into(),
        "12:04:03 ledger: replica lag 1.2s".into(),
        "12:04:05 cache: evicted 2k keys".into(),
    ]);

    let config = Content::Collection(vec![
        Item::new("cfg-tracing", "Tracing")
            .subtitle("sampled 10%")
            .action(Action::new("t-trace", "Cycle", Msg::Toggle("trace".into()))),
        Item::new("cfg-region", "Primary region")
            .subtitle("us-east-1")
            .action(Action::new(
                "t-region",
                "Cycle",
                Msg::Toggle("region".into()),
            )),
    ]);

    Experience::new("OPS CONSOLE")
        .destination(Destination::new("services", "Services", services))
        .destination(Destination::new("metrics", "Metrics", metrics))
        .destination(Destination::new("logs", "Logs", logs))
        .destination(Destination::new("config", "Config", config))
}

#[test]
fn every_grammar_preserves_a_non_media_domain_across_the_matrix() {
    let experience = console_fixture();
    // No item carries Media — prove it, so a later edit can't quietly reintroduce
    // the album assumption this fixture exists to rule out.
    for destination in &experience.destinations {
        if let Content::Collection(items) = &destination.content {
            for item in items {
                assert!(
                    item.media.is_none(),
                    "console fixture must stay media-free: {:?}",
                    item.key
                );
            }
        }
    }

    let sizes = [(160, 50), (120, 40), (80, 24), (60, 20), (42, 15)];
    let depths = [
        ColorDepth::TrueColor,
        ColorDepth::Ansi256,
        ColorDepth::Ansi16,
        ColorDepth::Mono,
    ];
    for (name, mut grammar) in all_grammars() {
        for (w, h) in sizes {
            for depth in depths {
                let e = env(w, h, depth);
                let mut state = PresentationState::new(&experience);
                for _ in 0..experience.destinations.len() {
                    let presented =
                        grammar.present(&experience, &state, &e, Duration::from_millis(300));
                    let required = required_semantics(&experience, &state);
                    let violations = presented.check(&required);
                    assert!(
                        violations.is_empty(),
                        "{name} violated the law on the console domain at {w}x{h} {depth:?}: \
                         {violations:?}"
                    );
                    apply_intent(&experience, &mut state, Intent::NextGroup);
                }
            }
        }
    }
}

#[test]
fn a_non_media_collection_still_resolves_and_acts() {
    // The services list navigates and its primary action is reachable by Enter
    // under every grammar, with no Media to lean on.
    let experience = console_fixture();
    for (name, grammar) in all_grammars() {
        let mut state = PresentationState::new(&experience);
        // Walk to the second service (grammar-independent) and activate it through
        // the grammar's OWN Enter binding via handle_key — so each grammar's
        // interpret path is exercised on a media-free collection.
        apply_intent(&experience, &mut state, Intent::Next);
        let selected = state.selected_key(&experience);
        assert_eq!(selected, Some(key("svc-ledger")), "{name}");
        let msg = handle_key(&*grammar, &experience, &mut state, &press(KeyCode::Enter));
        assert_eq!(
            msg,
            Some(Msg::Play("svc-ledger".into())),
            "{name} could not reach the primary action without media"
        );
    }
}

// ---------------------------------------------------------------------------
// Temporal contract (§29-39): what each grammar's frame depends on in TIME,
// so a terminal runtime knows when it may stop repainting. Verified by
// painting through the real compile -> layout -> paint path and comparing the
// resulting Surfaces (coalescing obsolete frames is only sound if a settled
// frame is byte-identical).
// ---------------------------------------------------------------------------

/// Paint one grammar frame through the real pipeline, for Surface-level equality.
fn paint_frame(
    grammar: &mut dyn Grammar<Msg>,
    experience: &Experience<Msg>,
    state: &PresentationState,
    e: &UiEnvironment,
    now: Duration,
) -> gibson::Surface {
    let presented = grammar.present(experience, state, e, now);
    let cx = gibson::ui::BuildCx::new(gibson::ui::skins::VAPOR95, *e);
    let mut node = gibson::ui::compile(&presented.element, &cx)
        .expect("grammar lowers to a valid element tree")
        .node;
    gibson::compute_layout(&mut node, e.width, e.height).unwrap();
    let mut surface = gibson::Surface::new(e.width, e.height);
    gibson::paint(&node, &mut surface);
    surface
}

#[test]
fn time_invariant_grammars_never_shimmer() {
    // STANDARD, CROSS_MEDIA and PANORAMA ignore `now` entirely: the same semantic
    // state yields a byte-identical frame no matter the clock, so a runtime may
    // paint once and coalesce every later frame until the state changes.
    let experience = fixture();
    let e = env(120, 40, ColorDepth::TrueColor);
    let state = PresentationState::new(&experience);
    let stateless: Vec<(&str, Box<dyn Grammar<Msg>>)> = vec![
        ("STANDARD", Box::new(Standard::new())),
        ("CROSS_MEDIA", Box::new(CrossMedia::new())),
        ("PANORAMA", Box::new(Panorama::new())),
    ];
    for (name, mut grammar) in stateless {
        let a = paint_frame(&mut *grammar, &experience, &state, &e, Duration::ZERO);
        let b = paint_frame(
            &mut *grammar,
            &experience,
            &state,
            &e,
            Duration::from_secs(9),
        );
        assert_eq!(a, b, "{name} shimmered despite no state change");
    }
}

#[test]
fn springy_grammars_settle_to_a_still_frame() {
    // MEDIA_SHELF and BLADES animate a damped spring, then settle. Once settled on
    // a target, further frames at later times are byte-identical: the settle point
    // is a true 0-diff, so the runtime can stop repainting.
    let experience = fixture();
    let e = env(120, 40, ColorDepth::TrueColor);
    let state = PresentationState::new(&experience); // active destination 0

    let mut shelf = MediaShelf::new();
    let mut blades = Blades::new();
    // Drive enough frames to let both springs settle on destination/selection 0.
    let mut now = 0u64;
    for _ in 0..400 {
        now += 16;
        shelf.present(&experience, &state, &e, Duration::from_millis(now));
        blades.present(&experience, &state, &e, Duration::from_millis(now));
    }
    assert!(blades.is_settled(0), "BLADES spring did not settle");
    assert!(shelf.is_settled(0), "MEDIA_SHELF spring did not settle");

    // Two further frames at different times must be identical now.
    let t1 = Duration::from_millis(now + 16);
    let t2 = Duration::from_millis(now + 2000);
    assert_eq!(
        paint_frame(&mut shelf, &experience, &state, &e, t1),
        paint_frame(&mut shelf, &experience, &state, &e, t2),
        "MEDIA_SHELF shimmered after settling"
    );
    assert_eq!(
        paint_frame(&mut blades, &experience, &state, &e, t1),
        paint_frame(&mut blades, &experience, &state, &e, t2),
        "BLADES shimmered after settling"
    );
}

#[test]
fn orbital_is_the_one_continuously_animated_grammar() {
    // ORBITAL's ambient field (swirl, drifting motes, halo breathe) is a function
    // of `now` with no settle point — so its frame genuinely differs over time
    // even at a fixed selection. This is a declared boundary: a runtime must keep
    // repainting ORBITAL while it is on screen. (Still deterministic at a fixed
    // time, which the orbital determinism test already pins.)
    let experience = fixture();
    let e = env(120, 40, ColorDepth::TrueColor);
    let state = PresentationState::new(&experience);
    let mut orbital = Orbital::new();
    // Prime the spring so the difference is the ambient field, not the entry glide.
    let mut now = 0u64;
    for _ in 0..200 {
        now += 16;
        orbital.present(&experience, &state, &e, Duration::from_millis(now));
    }
    let a = paint_frame(
        &mut orbital,
        &experience,
        &state,
        &e,
        Duration::from_millis(now + 16),
    );
    let b = paint_frame(
        &mut orbital,
        &experience,
        &state,
        &e,
        Duration::from_millis(now + 700),
    );
    assert_ne!(
        a, b,
        "ORBITAL is expected to animate continuously over time"
    );
}
