//! Recipe: **the edit clock is yours — drive it however you like.**
//!
//! The one law: a show is `f(edit)`, and the [`Timeline`] holds *no clock of its
//! own*. So the clock is a plain number in *your* model, and you can do anything to
//! it — let it play, pause it, step one frame, scrub it, or jump between acts — and
//! the timeline resolves *identically* for every one of those, because it is pure
//! `f(edit)`. There is no "player" object to fight: play/pause/seek is just
//! arithmetic on your own `f32`.
//!
//! Teaches: a model-owned edit clock (`AppEvent::Tick` → clamped `dt`), seeking and
//! single-stepping, and **act jumping computed straight from the public `cues()`** —
//! the timeline needs no `seek`/`jump`/`cue_at` method, because the cue starts are
//! already public and the clock is yours to move.
//!
//!   cargo run --example show_interactive        # live; Space=play/pause, ←/→ seek,
//!                                                #  , . step, n/p act, Home start, q quit
//!   cargo run --example show_interactive -- at 5.5   # headless: resolve one instant

use std::io;

use gibson::input::{Event, KeyCode, KeyModifiers};
use gibson::raster::RgbRaster;
use gibson::timeline::Timeline;
use gibson::ui::{raster, screen, skins, App, AppEvent, BuildCx, Control, Element};
use gibson::Surface;

#[derive(Clone)]
enum Msg {}

/// One act: a name and a base wash. The payload is entirely the author's — the
/// timeline never looks inside it.
#[derive(Clone, Copy)]
struct Act {
    name: &'static str,
    base: (u8, u8, u8),
}

/// A four-act show, 12 s end to end. Ordinary hard cuts; nothing here knows about a
/// clock — `film()` is pure data.
fn film() -> Timeline<Act> {
    Timeline::new()
        .cut(
            4.0,
            Act {
                name: "INGEST",
                base: (46, 20, 60),
            },
        )
        .cut(
            3.0,
            Act {
                name: "ALIGN",
                base: (18, 40, 72),
            },
        )
        .cut(
            3.0,
            Act {
                name: "RESOLVE",
                base: (20, 58, 40),
            },
        )
        .cut(
            2.0,
            Act {
                name: "REVEAL",
                base: (70, 40, 18),
            },
        )
}

/// The whole application state: an edit clock, whether it is advancing, and a little
/// transport bookkeeping. The clock — `edit` — is the single authoritative number.
struct Show {
    edit: f32,
    playing: bool,
    last_tick: f32,
    note: String,
}

impl Default for Show {
    fn default() -> Self {
        Self {
            edit: 0.0,
            playing: true,
            last_tick: 0.0,
            note: "playing".to_string(),
        }
    }
}

/// The act-start times, straight from the public cue list. **This is the whole
/// point of the recipe:** a seekable show does not need a library "jump to cue"
/// method — the cue starts are public, so act navigation is a few lines of the
/// author's own code. Sorted + deduped so it is correct even if cues were placed
/// out of order with `Timeline::at`.
fn act_starts(f: &Timeline<Act>) -> Vec<f32> {
    let mut s: Vec<f32> = f.cues().iter().map(|c| c.start).collect();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    s.dedup();
    s
}

/// The next act boundary strictly after `t`, or the final frame if `t` is already in
/// the last act.
fn next_act(f: &Timeline<Act>, t: f32) -> f32 {
    act_starts(f)
        .into_iter()
        .find(|&s| s > t + 1e-3)
        .unwrap_or(f.duration() - 1e-3)
}

/// The act boundary strictly before `t` (so a second press steps back act by act),
/// or the very start.
fn prev_act(f: &Timeline<Act>, t: f32) -> f32 {
    act_starts(f)
        .into_iter()
        .rev()
        .find(|&s| s < t - 1e-3)
        .unwrap_or(0.0)
}

/// Clamp a proposed clock value into the show's playable range. The window is
/// half-open `[0, dur)`, so the latest *resolvable* instant is `dur - epsilon`;
/// seeking to exactly `dur` would land past the final shot and resolve to nothing.
fn clamp_edit(f: &Timeline<Act>, t: f32) -> f32 {
    t.clamp(0.0, (f.duration() - 1e-3).max(0.0))
}

/// Draw the frame for a given edit instant. Pure `f(edit)` — identical whether the
/// clock got here by playing, seeking, or stepping.
fn frame(w: u16, h: u16, edit: f32) -> Surface {
    let f = film();
    let dur = f.duration();
    let ph = h.saturating_mul(2).max(2);

    // Resolve who is on screen. (The caller always hands us a clamped `edit`, but a
    // headless `at` past the end could still miss — fall back to a dim idle wash.)
    let (act, local, progress) = match f.top(edit) {
        Some(a) => (*a.payload, a.local, a.progress),
        None => (
            Act {
                name: "—",
                base: (10, 10, 18),
            },
            0.0,
            0.0,
        ),
    };

    // A drifting band field keyed to the act, so motion is legible frame to frame and
    // seeking within an act visibly moves the picture.
    let mut r = RgbRaster::new(w.max(1), ph);
    for x in 0..w as i32 {
        let t = ((x as f32 / w as f32) * 6.0 + local * 2.0).sin() * 0.5 + 0.5;
        let c = (
            act.base.0.saturating_add((t * 90.0) as u8),
            act.base.1.saturating_add((t * 70.0) as u8),
            act.base.2.saturating_add((t * 110.0) as u8),
        );
        for y in 0..ph as i32 {
            r.set(x, y, c);
        }
    }
    let mut surface = r.to_surface();

    if w >= 24 && h >= 6 {
        surface.bake_text(2, 1, act.name, (245, 245, 255), true);
        let clock = format!(
            "edit {edit:5.2}s / {dur:4.1}s   local {local:4.2}s   {:3.0}%",
            progress * 100.0
        );
        surface.bake_text(2, 2, &clock, (210, 210, 235), false);

        // A scrub bar: the playhead across the whole show, act boundaries marked.
        // Everything the bar draws is read off the public timeline — the position of
        // the caret is just `edit / dur`.
        let barw = (w as usize).saturating_sub(4).max(8);
        let mut bar: Vec<char> = vec!['─'; barw];
        for s in act_starts(&f) {
            let i = ((s / dur) * barw as f32) as usize;
            if let Some(cell) = bar.get_mut(i.min(barw - 1)) {
                *cell = '┆';
            }
        }
        let head = ((edit / dur) * barw as f32) as usize;
        if let Some(cell) = bar.get_mut(head.min(barw - 1)) {
            *cell = '▮';
        }
        let bar: String = bar.into_iter().collect();
        surface.bake_text(2, h.saturating_sub(3), &bar, (150, 170, 210), false);
        surface.bake_text(
            2,
            h.saturating_sub(2),
            "space play/pause   </> seek   ,/. step   n/p act   home start   q quit",
            (150, 150, 180),
            false,
        );
    }
    surface
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("at") {
        let t: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let f = film();
        match f.top(clamp_edit(&f, t)) {
            Some(a) => println!(
                "at {t:.2}s -> act {:?} (index {}), local {:.2}s, {:.0}%",
                a.payload.name,
                a.index,
                a.local,
                a.progress * 100.0
            ),
            None => println!(
                "at {t:.2}s -> (nothing scheduled; past the {:.1}s end)",
                f.duration()
            ),
        }
        return Ok(());
    }
    App::fullscreen()
        .skin(skins::VAPOR95)
        .fps(30)
        .run(Show::default(), update, view)?;
    Ok(())
}

fn view(m: &Show, cx: &BuildCx) -> Element<Msg> {
    let (w, h) = (cx.environment.width, cx.environment.height);
    screen::<Msg>().child(raster::<Msg>(frame(w, h, m.edit)).grow(1.0))
}

fn update(m: &mut Show, event: AppEvent<Msg>) -> Control {
    let f = film();
    match event {
        // The ONLY place the clock advances on its own. `Tick` carries elapsed since
        // app start; take a clamped delta so a stall never jumps the show. When
        // playing, wrap at the end so the live preview loops.
        AppEvent::Tick(elapsed) => {
            let now = elapsed.as_secs_f32();
            let dt = (now - m.last_tick).clamp(0.0, 0.25);
            m.last_tick = now;
            if m.playing {
                m.edit += dt;
                if m.edit >= f.duration() {
                    m.edit = 0.0;
                }
            }
            Control::Continue
        }
        AppEvent::Input(Event::Key(key)) => {
            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
            match key.code {
                KeyCode::Char('q') | KeyCode::Char('Q') => return Control::Quit,
                KeyCode::Char('c') if ctrl => return Control::Quit,
                KeyCode::Char(' ') => {
                    m.playing = !m.playing;
                    m.note = if m.playing { "playing" } else { "paused" }.to_string();
                }
                // Seek. Pausing first makes the scrub land where you leave it.
                KeyCode::Left => {
                    m.playing = false;
                    m.edit = clamp_edit(&f, m.edit - 0.5);
                }
                KeyCode::Right => {
                    m.playing = false;
                    m.edit = clamp_edit(&f, m.edit + 0.5);
                }
                // Single-frame step at 30 fps — the finest seek there is.
                KeyCode::Char(',') => {
                    m.playing = false;
                    m.edit = clamp_edit(&f, m.edit - 1.0 / 30.0);
                }
                KeyCode::Char('.') => {
                    m.playing = false;
                    m.edit = clamp_edit(&f, m.edit + 1.0 / 30.0);
                }
                // Jump act to act — using the author-side `next_act`/`prev_act` built
                // entirely from the public `cues()`.
                KeyCode::Char('n') => {
                    m.playing = false;
                    m.edit = clamp_edit(&f, next_act(&f, m.edit));
                }
                KeyCode::Char('p') => {
                    m.playing = false;
                    m.edit = clamp_edit(&f, prev_act(&f, m.edit));
                }
                KeyCode::Home | KeyCode::Char('0') => {
                    m.edit = 0.0;
                }
                _ => {}
            }
            Control::Continue
        }
        _ => Control::Continue,
    }
}
