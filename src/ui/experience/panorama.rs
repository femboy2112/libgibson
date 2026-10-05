//! `PANORAMA` — a typographic panorama grammar (Zune/Metro-panorama inspired).
//!
//! Proof that "cinematic" is not "3D": this style uses **no raster and no
//! perspective**, only large type, negative space and one wide horizontal world.
//! Every destination is a panorama *section* announced by a very large heading
//! (letter-spaced capitals, the terminal's "large type"). The active section's
//! items form a quiet vertical reading column beneath its heading, and the
//! content deliberately **overruns the viewport edge**: the previous section's
//! title bleeds in at the left margin, the following sections' titles bleed off
//! the right. There are no boxes and no dividers — hierarchy is carried by type
//! size (spacing), emphasis and position alone, with restrained motion (none).
//!
//! Navigation follows the world's axes: Left/Right **pan** between sections
//! (`PreviousGroup`/`NextGroup`), Up/Down move within the reading column
//! (`Previous`/`Next`). A Node/`Element` grammar: every destination, item and
//! claimed action is a keyed node, and focus is expressed by emphasis plus a
//! non-colour marker, never by omission (so it survives `ColorDepth::Mono`).
//! The only raster is a `Custom` instrument, composited verbatim.

use super::grammar::{Grammar, Presented};
use super::intent::{Intent, PresentationState, SemanticInput};
use super::model::{Content, Experience};
use super::receipt::PresentationReceipt;
use crate::input::{KeyCode, KeyEvent};
use crate::ui::element::{button, column, heading, label, raster, row, screen, text, Element, Key};
use crate::ui::skin::UiEnvironment;
use crate::ui::style::{Emphasis, Tone};
use std::time::Duration;

/// Below this width the letter-spaced "large type" collapses to plain capitals —
/// a declared, purely typographic reduction (never an omitted id).
const LARGE_TYPE_MIN_WIDTH: u16 = 100;

/// The panorama grammar. Stateless: the pan is a pure function of the active
/// destination, so a live switch can never strand the camera off the selection.
#[derive(Debug, Clone, Copy, Default)]
pub struct Panorama;

impl Panorama {
    pub fn new() -> Self {
        Self
    }
}

/// Set a title in "large type": capitals, letter-spaced when there is room (words
/// separated by a wide gap), plain capitals otherwise.
fn large_type(title: &str, spaced: bool) -> String {
    let upper = title.to_uppercase();
    if !spaced {
        return upper;
    }
    upper
        .split_whitespace()
        .map(|word| {
            word.chars()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("   ")
}

/// Truncate `text` to at most `cells` characters, ending in an ellipsis when cut.
fn fit(text: &str, cells: usize) -> String {
    if text.chars().count() <= cells {
        return text.to_string();
    }
    let keep = cells.saturating_sub(1);
    let mut out: String = text.chars().take(keep).collect();
    out.push('\u{2026}');
    out
}

impl<A: Clone> Grammar<A> for Panorama {
    fn name(&self) -> &'static str {
        "PANORAMA"
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
        let mut receipt = PresentationReceipt::new("PANORAMA", active_key);

        let spaced = env.width >= LARGE_TYPE_MIN_WIDTH;
        if !spaced {
            receipt
                .degraded
                .push("large type collapsed to plain capitals".to_string());
        }
        let total = experience.destinations.len();

        // Pan geometry, budgeted in whole cells so the camera is a pure function
        // of the active destination: the sections already panned past collapse
        // into a narrow left rail of truncated titles (the previous section
        // bleeds in at the margin), the active section owns the pane, and the
        // sections still ahead run off the right edge.
        let usable = env.width.saturating_sub(2).max(1) as usize;
        let n_prev = active_idx.min(total);
        let n_next = total.saturating_sub(active_idx + 1);
        let rail_gap = 2usize;
        // Ooh yeah, divide-by-zero is pain: checked_div says "no sections behind" = 0 cells!
        let rail_share = (usable / 4)
            .max(8)
            .checked_div(n_prev)
            .map_or(0, |share| share.max(5));
        // Each panned-past title gets only the cells it needs, capped at its share.
        let slivers: Vec<String> = experience
            .destinations
            .iter()
            .take(n_prev)
            .map(|d| {
                fit(
                    &format!("\u{2039} {}", large_type(&d.title, spaced)),
                    rail_share,
                )
            })
            .collect();
        let rail_width: usize = slivers
            .iter()
            .map(|sliver| sliver.chars().count() + rail_gap)
            .sum();
        let bleed = if n_next == 0 {
            0
        } else {
            (usable / 6).clamp(4, 14)
        };
        let pane_width = usable
            .saturating_sub(rail_width)
            .saturating_sub(bleed)
            .max(16);

        // The wide world: one section heading per destination, in order. The
        // active one is the dominant large type; its siblings are faint titles.
        let mut world = row::<A>().gap(rail_gap as u16);
        for (index, destination) in experience.destinations.iter().enumerate() {
            receipt.destinations.push(destination.key.clone());
            let title = large_type(&destination.title, spaced);
            if index == active_idx {
                let heading = heading::<A>(title)
                    .key(destination.key.to_string())
                    .selected(true)
                    .emphasis(Emphasis::Strong)
                    .tone(Tone::Accent);
                let body = reading_column(active, state, experience, &mut receipt, env);
                world = world.child(
                    column::<A>()
                        .gap(1)
                        .width(pane_width.min(u16::MAX as usize) as u16)
                        .child(heading)
                        .child(body),
                );
            } else if index < active_idx {
                // Panned past: a chevron-led, truncated sliver in the left rail.
                let sliver = slivers[index].clone();
                let cells = sliver.chars().count().min(u16::MAX as usize) as u16;
                world = world.child(
                    label::<A>(sliver)
                        .key(destination.key.to_string())
                        .width(cells)
                        .emphasis(Emphasis::Faint),
                );
            } else {
                // Still ahead: a chevron-trailed title that bleeds off the edge.
                world = world.child(
                    label::<A>(format!("{title} \u{203a}"))
                        .key(destination.key.to_string())
                        .emphasis(Emphasis::Faint),
                );
            }
        }

        // A small kicker: the experience title and a positional "n / N" so the
        // section order is legible without colour.
        let position = if total == 0 {
            String::new()
        } else {
            format!("{:02} / {:02}", active_idx + 1, total)
        };
        let kicker = row::<A>()
            .gap(2)
            .child(label::<A>(experience.title.clone()).emphasis(Emphasis::Muted))
            .child(label::<A>(position).emphasis(Emphasis::Faint));

        let root = screen::<A>().child(
            column::<A>()
                .padding(1)
                .gap(2)
                .child(kicker)
                .child(world.grow(1.0)),
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
        // The panorama's axes: Left/Right pan the world, Up/Down walk the column.
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

/// The active section's body: a quiet reading column under the large heading.
fn reading_column<A: Clone>(
    active: Option<&super::model::Destination<A>>,
    state: &PresentationState,
    experience: &Experience<A>,
    receipt: &mut PresentationReceipt,
    env: &UiEnvironment,
) -> Element<A> {
    match active.map(|d| &d.content) {
        Some(Content::Collection(items)) => {
            let selected = state.selected_index(experience).unwrap_or(0);
            let mut body = column::<A>().gap(0).padding(1);
            for (index, item) in items.iter().enumerate() {
                receipt.items.push(item.key.clone());
                // Every item is a keyed node. Focus is a marker plus emphasis —
                // never omission, never colour alone.
                if index == selected {
                    receipt.selected = Some(item.key.clone());
                    body = body.child(
                        text::<A>(format!("▸ {}", item.title))
                            .key(item.key.to_string())
                            .selected(true)
                            .emphasis(Emphasis::Strong),
                    );
                    if let Some(subtitle) = &item.subtitle {
                        body = body
                            .child(label::<A>(format!("  {subtitle}")).emphasis(Emphasis::Muted));
                    }
                    if let Some(primary) = item.primary() {
                        receipt.actions.push(primary.key.clone());
                        body = body.child(
                            button::<A>(primary.label.clone())
                                .key(primary.key.to_string())
                                .tone(Tone::Accent),
                        );
                    }
                } else {
                    body = body.child(
                        text::<A>(format!("  {}", item.title))
                            .key(item.key.to_string())
                            .emphasis(Emphasis::Faint),
                    );
                }
            }
            body
        }
        Some(Content::Detail { facets, actions }) => {
            let mut body = column::<A>().gap(0).padding(1);
            for facet in facets {
                body = body
                    .child(label::<A>(facet.label.to_uppercase()).emphasis(Emphasis::Faint))
                    .child(text::<A>(facet.value.clone()).emphasis(Emphasis::Strong));
            }
            let mut action_row = row::<A>().gap(2);
            for action in actions {
                receipt.actions.push(action.key.clone());
                action_row =
                    action_row.child(button::<A>(action.label.clone()).key(action.key.to_string()));
            }
            body.child(action_row)
        }
        Some(Content::Prose(lines)) => {
            let mut body = column::<A>().gap(0).padding(1);
            for line in lines {
                body = body.child(text::<A>(line.clone()));
            }
            body
        }
        Some(Content::Custom(custom)) => {
            let width = env.width.max(1);
            let height = env.height.saturating_sub(8).max(1);
            receipt
                .degraded
                .push(format!("custom instrument '{}' composited", custom.label));
            raster::<A>(custom.render(width, height))
        }
        None => text::<A>("(no destinations)"),
    }
}
