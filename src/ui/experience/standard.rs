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
use super::model::{Content, CustomCx, Experience};
use super::receipt::PresentationReceipt;
use crate::input::{KeyCode, KeyEvent};
use crate::ui::element::{
    button, choice, column, heading, label, list, panel, raster, row, screen, tabs, text, Key,
};
use crate::ui::skin::UiEnvironment;
use crate::ui::style::{Emphasis, Tone};
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
        now: Duration,
    ) -> Presented<A> {
        let active_idx = state.active_index(experience);
        let active = experience.destinations.get(active_idx);
        let active_key = active
            .map(|d| d.key.clone())
            .unwrap_or_else(|| Key::named("∅"));
        let mut receipt = PresentationReceipt::new("STANDARD", active_key);

        // Destination nav bar — a styled tab strip (every destination a keyed,
        // reachable choice), not a hand-rolled row of bold words.
        let mut nav = tabs::<A>();
        for (index, destination) in experience.destinations.iter().enumerate() {
            receipt.destinations.push(destination.key.clone());
            nav = nav.child(
                choice::<A>(destination.title.clone(), index == active_idx)
                    .key(destination.key.to_string()),
            );
        }

        let content = match active.map(|d| &d.content) {
            Some(Content::Collection(items)) => {
                let selected = state.selected_index(experience).unwrap_or(0);
                // Pad titles to a common column so subtitles align into a second
                // column — a clean list, not title and subtitle mashed together.
                let title_w = items
                    .iter()
                    .map(|item| item.title.chars().count())
                    .max()
                    .unwrap_or(0);
                let mut listing = list::<A>().gap(0);
                for (index, item) in items.iter().enumerate() {
                    receipt.items.push(item.key.clone());
                    let line = match &item.subtitle {
                        Some(subtitle) => {
                            format!("{:<title_w$}    {}", item.title, subtitle)
                        }
                        None => item.title.clone(),
                    };
                    // One clean row, keyed so the receipt's claimed item is the node
                    // actually on screen. The selected row carries a single selection
                    // bar (no per-row button brackets).
                    let mut entry = text::<A>(line).key(item.key.to_string());
                    if index == selected {
                        entry = entry
                            .selected(true)
                            .emphasis(Emphasis::Strong)
                            .tone(Tone::Accent);
                        receipt.selected = Some(item.key.clone());
                    }
                    listing = listing.child(entry);
                }
                // Render the selected item's primary action as a keyed button, so
                // the action the receipt claims is the action actually on screen
                // (and reachable via Enter).
                let mut column = column::<A>().gap(1).child(listing.grow(1.0));
                if let Some(primary) = items.get(selected).and_then(|item| item.primary()) {
                    receipt.actions.push(primary.key.clone());
                    column =
                        column.child(button(primary.label.clone()).key(primary.key.to_string()));
                }
                column
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
                raster::<A>(custom.render(&CustomCx::new(width, height, now, env.color_depth)))
            }
            None => text::<A>("(no destinations)"),
        };

        // Frame the active destination's content on a raised panel surface (filled
        // body + accent title bar), the way a composed app would — depth and
        // hierarchy, not flat text on the bare screen. Plain, but not bare-metal.
        let active_title = active
            .map(|d| d.title.to_uppercase())
            .unwrap_or_else(|| "—".to_string());
        let root = screen::<A>().child(
            column::<A>()
                .padding(1)
                .gap(1)
                .child(heading(experience.title.clone()))
                .child(nav)
                .child(panel::<A>(active_title).grow(1.0).child(content)),
        );

        Presented::new(root, receipt)
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
