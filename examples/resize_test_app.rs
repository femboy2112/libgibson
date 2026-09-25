//! Deterministic PTY resize probe application.
//!
//! Protocol (line-oriented sentinels on stdout):
//!   - `GIBSON_RESIZE_PROBE_READY`
//!   - `GIBSON_RESIZE_PROBE_DONE anchor_resyncs=<n> fast=<n> fallback=<n>`
//!
//! Keys: printable characters edit the live input; `i` inserts an asynchronous
//! event above the live region; `q` (or Esc) exits cleanly.
//!
//! The app intentionally drives its loop through `Context::run_once`, which
//! enters raw mode / bracketed paste and polls for real input and resize events.

use gibson::cell::ThemeStyles;
use gibson::context::Context;
use gibson::input::{Event, KeyCode, KeyModifiers, TextInputState};
use gibson::node::{Node, WrapMode};
use gibson::Theme;
use std::io::Write;
use std::time::Duration;

fn build_root(text: &TextInputState, s: &ThemeStyles, tick: bool) -> Node {
    Node::col()
        .child(Node::text(
            if tick {
                "── resize probe :: live ──"
            } else {
                "── resize probe :: live ─"
            },
            s.accent,
        ))
        .child(Node::rich_text_wrapped(
            gibson::RichText::raw(
                "Committed history stays above; the live footer must remain stable.",
            ),
            WrapMode::WordWrap,
        ))
        .child(
            Node::row().gap(1.0).child(Node::text("❯", s.accent)).child(
                Node::text_input(
                    &text.text,
                    text.cursor_grapheme,
                    Some("type here..."),
                    s.text,
                )
                .flex_grow(1.0),
            ),
        )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme = Theme::default();
    let s = theme.styles();
    let mut ctx = Context::inline()?;
    ctx.set_max_fps(60);
    ctx.set_animation_interval(Duration::from_millis(90));

    let mut stdout = std::io::stdout();
    writeln!(stdout, "GIBSON_RESIZE_PROBE_READY")?;
    stdout.flush()?;

    let mut input = TextInputState::new();
    let mut tick = false;

    loop {
        let root = build_root(&input, &s, tick);
        ctx.set_root(root);

        // run_once polls for input (bounded by the frame budget) and renders if due.
        if let Some(event) = ctx.run_once(Duration::from_millis(50))? {
            match event {
                Event::Key(k) => match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    // Ctrl-G inserts an async event; printable characters type normally.
                    KeyCode::Char('g') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                        ctx.insert_raw_lines_before_live_unchecked(&[
                            "── ASYNC INSERTION (arrived above live region) ──",
                        ])?;
                    }
                    _ => {
                        if input.handle_event(&event) {
                            ctx.request_render();
                        }
                    }
                },
                Event::Resize(_, _) => {
                    ctx.request_render();
                }
                Event::Tick => {}
                // Paste — and any future `Event` kind (`Event` is
                // `#[non_exhaustive]`) — is offered to the text input; render if it
                // consumed the event.
                _ => {
                    if input.handle_event(&event) {
                        ctx.request_render();
                    }
                }
            }
        }
        tick = !tick;
    }

    // Final commit of the typed input, then clean restoration.
    let final_text = input.text.clone();
    ctx.commit_text(&format!("❯ {}", final_text))?;
    ctx.restore()?;

    let stats = ctx.stats();
    writeln!(
        stdout,
        "GIBSON_RESIZE_PROBE_DONE anchor_resyncs={} fast={} fallback={} frames={}",
        stats.anchor_resyncs, stats.fast_insertions, stats.insertion_repaints, stats.frames
    )?;
    stdout.flush()?;
    std::io::Write::flush(&mut stdout)?;
    Ok(())
}
