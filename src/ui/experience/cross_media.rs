//! `CROSS_MEDIA` — a cross-bar grammar (structurally inspired by XMB).
//!
//! Two orthogonal axes meeting at a focus: destinations spread **horizontally**
//! across a bar near the vertical centre, and the active destination's items
//! descend **vertically** from it. One object dominates — the selected item, in
//! full emphasis — while its neighbours and the sibling destinations fade to
//! faint markers. No bordered panels, generous negative space; the hierarchy is
//! carried by emphasis and position, not chrome. A Node/`Element` grammar (no
//! raster), so it degrades cleanly to any capability and stays cheap.
//!
//! Navigation is the cross: Left/Right move along the destination bar, Up/Down
//! along the item column. Every item is a keyed node in the tree (so all are
//! represented and reachable), with emphasis — not omission — expressing focus.

use super::grammar::{Grammar, Presented};
use super::intent::{Intent, PresentationState, SemanticInput};
use super::model::{Content, Experience};
use super::receipt::PresentationReceipt;
use crate::input::{KeyCode, KeyEvent};
use crate::ui::element::{button, column, label, raster, row, screen, spacer, text, Element, Key};
use crate::ui::skin::UiEnvironment;
use crate::ui::style::{Emphasis, Tone};
use std::time::Duration;

/// The cross-bar grammar. Stateless.
#[derive(Debug, Clone, Copy, Default)]
pub struct CrossMedia;

impl CrossMedia {
    pub fn new() -> Self {
        Self
    }
}

/// Centre a single element horizontally with flexible spacers on either side.
fn centre_row<A>(child: Element<A>) -> Element<A> {
    row::<A>()
        .child(spacer::<A>().grow(1.0))
        .child(child)
        .child(spacer::<A>().grow(1.0))
}

impl<A: Clone> Grammar<A> for CrossMedia {
    fn name(&self) -> &'static str {
        "CROSS_MEDIA"
    }

    fn present(
        &mut self,
        experience: &Experience<A>,
        state: &PresentationState,
        env: &UiEnvironment,
        _now: Duration,
    ) -> Presented<A> {
        let active_idx = state.active_index(experience);
        let active = experience.destinations.get(active_idx);
        let active_key = active
            .map(|d| d.key.clone())
            .unwrap_or_else(|| Key::named("∅"));
        let mut receipt = PresentationReceipt::new("CROSS_MEDIA", active_key);

        // The horizontal destination bar. The active destination is emphasised and
        // numbered; siblings are faint markers spread to either side.
        let mut bar = row::<A>().gap(3);
        for (index, destination) in experience.destinations.iter().enumerate() {
            receipt.destinations.push(destination.key.clone());
            let mut tab = text::<A>(destination.title.clone()).key(destination.key.to_string());
            if index == active_idx {
                tab = tab
                    .selected(true)
                    .emphasis(Emphasis::Strong)
                    .tone(Tone::Accent);
            } else {
                tab = tab.emphasis(Emphasis::Faint);
            }
            bar = bar.child(tab);
        }

        // The vertical item axis descending from the active destination.
        let axis = match active.map(|d| &d.content) {
            Some(Content::Collection(items)) if !items.is_empty() => {
                let selected = state.selected_index(experience).unwrap_or(0);
                let mut column_axis = column::<A>().gap(0);
                for (index, item) in items.iter().enumerate() {
                    receipt.items.push(item.key.clone());
                    let mut node = text::<A>(item.title.clone()).key(item.key.to_string());
                    if index == selected {
                        node = node.selected(true).emphasis(Emphasis::Strong);
                        receipt.selected = Some(item.key.clone());
                    } else {
                        node = node.emphasis(Emphasis::Faint);
                    }
                    column_axis = column_axis.child(centre_row(node));
                }
                // The dominant focus: the selected item's subtitle and its primary
                // action, drawn (and keyed) right at the crossing point.
                if let Some(item) = items.get(selected) {
                    if let Some(subtitle) = &item.subtitle {
                        column_axis = column_axis.child(centre_row(
                            label::<A>(subtitle.clone()).emphasis(Emphasis::Muted),
                        ));
                    }
                    if let Some(primary) = item.primary() {
                        receipt.actions.push(primary.key.clone());
                        column_axis = column_axis.child(centre_row(
                            button::<A>(primary.label.clone())
                                .key(primary.key.to_string())
                                .tone(Tone::Accent),
                        ));
                    }
                }
                column_axis
            }
            Some(Content::Detail { facets, actions }) => {
                let mut body = column::<A>().gap(0);
                for facet in facets {
                    body = body.child(centre_row(text::<A>(format!(
                        "{}  {}",
                        facet.label, facet.value
                    ))));
                }
                let mut action_row = row::<A>().gap(2);
                for action in actions {
                    receipt.actions.push(action.key.clone());
                    action_row = action_row
                        .child(button::<A>(action.label.clone()).key(action.key.to_string()));
                }
                body.child(centre_row(action_row))
            }
            Some(Content::Prose(lines)) => {
                let mut body = column::<A>().gap(0);
                for line in lines {
                    body = body.child(centre_row(text::<A>(line.clone())));
                }
                body
            }
            Some(Content::Custom(custom)) => {
                let width = env.width.max(1);
                let height = env.height.saturating_sub(6).max(1);
                receipt
                    .degraded
                    .push(format!("custom instrument '{}' composited", custom.label));
                centre_row(raster::<A>(custom.render(width, height)))
            }
            _ => text::<A>("(no destinations)"),
        };

        // Compose the cross: flexible space, the destination bar, the item axis,
        // flexible space — so the crossing point sits near the vertical centre.
        let root = screen::<A>().child(
            column::<A>()
                .padding(1)
                .gap(1)
                .child(spacer::<A>().grow(1.0))
                .child(centre_row(bar))
                .child(axis)
                .child(spacer::<A>().grow(1.0)),
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
        // The cross: Left/Right along the destination bar, Up/Down along the item
        // column.
        let intent = match key.code {
            KeyCode::Left => Intent::PreviousGroup,
            KeyCode::Right => Intent::NextGroup,
            KeyCode::Up => Intent::Previous,
            KeyCode::Down => Intent::Next,
            KeyCode::Enter => Intent::Enter,
            KeyCode::Esc | KeyCode::Backspace => Intent::Back,
            KeyCode::Home => Intent::Home,
            KeyCode::End => Intent::End,
            _ => return None,
        };
        Some(SemanticInput::Navigate(intent))
    }
}
