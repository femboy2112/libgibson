//! `STANDARD` — the reference grammar.
//!
//! Ordinary [`crate::ui`] chrome: a destination nav bar and the active
//! destination's content as a plain list / field set / prose / instrument. It is
//! deliberately unremarkable to look at. Its job is to be the **oracle**: it
//! represents *everything* the application defines — every destination, every
//! item, every reachable action — so its receipt is the semantic ground truth a
//! cinematic grammar's receipt is compared against. It omits nothing, and it is
//! the accessible fallback path.

use super::grammar::{Grammar, Presented};
use super::intent::{Intent, PresentationState, SemanticInput};
use super::model::{Content, Experience};
use super::receipt::PresentationReceipt;
use crate::input::{KeyCode, KeyEvent};
use crate::ui::element::{
    button, column, divider, heading, label, list, raster, row, screen, text, Key,
};
use crate::ui::skin::UiEnvironment;
use crate::ui::style::Emphasis;
use std::time::Duration;

/// The reference presentation grammar. Stateless.
#[derive(Debug, Clone, Copy, Default)]
pub struct Standard;

impl Standard {
    pub fn new() -> Self {
        Self
    }
}

impl<A: Clone> Grammar<A> for Standard {
    fn name(&self) -> &'static str {
        "STANDARD"
    }

    fn present(
        &mut self,
        experience: &Experience<A>,
        state: &PresentationState,
        env: &UiEnvironment,
        _now: Duration,
    ) -> Presented<A> {
        let active_idx = state.active();
        let active = experience.destinations.get(active_idx);
        let active_key = active
            .map(|d| d.key.clone())
            .unwrap_or_else(|| Key::named("∅"));
        let mut receipt = PresentationReceipt::new("STANDARD", active_key);

        // Destination nav bar — every destination represented and reachable.
        let mut nav = row().gap(2);
        for (index, destination) in experience.destinations.iter().enumerate() {
            receipt.destinations.push(destination.key.clone());
            let mut tab = text::<A>(destination.title.clone()).key(destination.key.to_string());
            if index == active_idx {
                tab = tab.selected(true).emphasis(Emphasis::Strong);
            }
            nav = nav.child(tab);
        }

        let content = match active.map(|d| &d.content) {
            Some(Content::Collection(items)) => {
                let selected = state.selection();
                let mut listing = list::<A>().gap(0);
                for (index, item) in items.iter().enumerate() {
                    receipt.items.push(item.key.clone());
                    let mut entry = row::<A>().gap(1).key(item.key.to_string());
                    entry = entry.child(text(item.title.clone()));
                    if let Some(subtitle) = &item.subtitle {
                        entry = entry.child(label(subtitle.clone()));
                    }
                    if index == selected {
                        entry = entry.selected(true).emphasis(Emphasis::Strong);
                        receipt.selected = Some(item.key.clone());
                        for action in &item.actions {
                            receipt.actions.push(action.key.clone());
                        }
                    }
                    listing = listing.child(entry);
                }
                listing
            }
            Some(Content::Detail { facets, actions }) => {
                let mut fields = column::<A>().gap(0);
                for facet in facets {
                    fields = fields.child(
                        row::<A>()
                            .gap(1)
                            .child(label(format!("{}:", facet.label)))
                            .child(text(facet.value.clone())),
                    );
                }
                let mut action_row = row::<A>().gap(2);
                for action in actions {
                    receipt.actions.push(action.key.clone());
                    action_row =
                        action_row.child(button(action.label.clone()).key(action.key.to_string()));
                }
                fields.child(action_row)
            }
            Some(Content::Prose(lines)) => {
                let mut body = column::<A>().gap(0);
                for line in lines {
                    body = body.child(text(line.clone()));
                }
                body
            }
            Some(Content::Custom(custom)) => {
                let width = env.width.max(1);
                let height = env.height.saturating_sub(5).max(1);
                receipt.degraded.push(format!(
                    "custom instrument '{}' composited at {width}x{height}",
                    custom.label
                ));
                raster::<A>(custom.render(width, height))
            }
            None => text::<A>("(no destinations)"),
        };

        let root = screen::<A>().child(
            column::<A>()
                .padding(1)
                .gap(1)
                .child(heading(experience.title.clone()))
                .child(nav)
                .child(divider())
                .child(content.grow(1.0)),
        );

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
            KeyCode::Up | KeyCode::Char('k') => Intent::Previous,
            KeyCode::Down | KeyCode::Char('j') => Intent::Next,
            KeyCode::Left | KeyCode::Char('h') => Intent::PreviousGroup,
            KeyCode::Right | KeyCode::Char('l') => Intent::NextGroup,
            KeyCode::Enter => Intent::Enter,
            KeyCode::Esc | KeyCode::Backspace => Intent::Back,
            KeyCode::Home => Intent::Home,
            KeyCode::End => Intent::End,
            _ => return None,
        };
        Some(SemanticInput::Navigate(intent))
    }
}
