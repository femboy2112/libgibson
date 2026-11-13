//! Six service states as status rows, rendered headlessly at 40×15 mono and
//! 80×24 ANSI-16 for each built-in skin, plus a report of states that share a colour.
//!
//! The pattern: the state's *word* always carries the meaning (so monochrome and
//! screen readers lose nothing), its *marker* differs by tone, and its *colour*
//! reinforces both. No new API: `status(word).tone(..).emphasis(..)`.
//!
//! ```text
//! cargo run --example status_states            # print every frame + report
//! ```
//! The report lists pairs of states painted with identical colours (foreground
//! and background, for the exact tone and emphasis each is drawn with). The light
//! built-in skins have such pairs at ANSI-16, because their warning and error both
//! quantize to red; there the words and markers still differ. An application skin
//! can separate them with its own `ansi16` [`DepthPalette`], and check that with
//! the same comparison.
use gibson::ui::prelude::*;
use gibson::{compute_layout, paint, ColorDepth, Surface};

/// State word, tone, emphasis. `loading` and `busy` share a style by design: they
/// are told apart by their words.
const STATES: [(&str, Tone, Emphasis); 6] = [
    ("loading", Tone::Accent, Emphasis::Normal),
    ("ready", Tone::Success, Emphasis::Normal),
    ("busy", Tone::Accent, Emphasis::Normal),
    ("degraded", Tone::Warning, Emphasis::Normal),
    ("failed", Tone::Danger, Emphasis::Strong),
    ("stopping", Tone::Neutral, Emphasis::Muted),
];

fn view() -> Element<()> {
    let mut col = column();
    for (word, tone, emphasis) in STATES {
        col = col.child(status(word).tone(tone).emphasis(emphasis));
    }
    screen().child(col)
}

fn environment(width: u16, height: u16, color_depth: ColorDepth) -> UiEnvironment {
    UiEnvironment {
        width,
        height,
        color_depth,
        motion: MotionPreference::None,
        ..UiEnvironment::default()
    }
}

fn render(skin: Skin, env: UiEnvironment) -> Surface {
    let mut node = compile(&view(), &BuildCx::new(skin, env))
        .expect("compile")
        .node;
    compute_layout(&mut node, env.width, env.height).expect("layout");
    let mut surface = Surface::new(env.width, env.height);
    paint(&node, &mut surface);
    surface
}

fn rows(s: &Surface) -> Vec<String> {
    (0..s.height)
        .map(|y| {
            (0..s.width)
                .filter_map(|x| {
                    let c = s.get(x, y).unwrap();
                    (!c.is_continuation).then(|| c.glyph.grapheme.to_string())
                })
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

/// Pairs of states painted with the same foreground and background at `depth`,
/// using the exact (tone, emphasis) each state is drawn with. `loading`/`busy`
/// share a style on purpose and are not reported.
fn style_collisions(skin: Skin, depth: ColorDepth) -> Vec<(&'static str, &'static str)> {
    let resolved = skin.resolve(&environment(80, 24, depth));
    let colours = |tone, emphasis| {
        let st = resolved.style(tone, emphasis);
        (st.fg, st.bg)
    };
    let mut out = Vec::new();
    for (i, &(a, ta, ea)) in STATES.iter().enumerate() {
        for &(b, tb, eb) in &STATES[i + 1..] {
            if (a, b) != ("loading", "busy") && colours(ta, ea) == colours(tb, eb) {
                out.push((a, b));
            }
        }
    }
    out
}

const SKINS: [(&str, Skin); 3] = [
    ("SWISS_SIGNAL", skins::SWISS_SIGNAL),
    ("BLACK_ICE", skins::BLACK_ICE),
    ("VAPOR95", skins::VAPOR95),
];

fn main() {
    for (name, skin) in SKINS {
        for (w, h, depth) in [(40, 15, ColorDepth::Mono), (80, 24, ColorDepth::Ansi16)] {
            println!("── {name} {w}×{h} {depth:?}");
            for row in rows(&render(skin, environment(w, h, depth))) {
                if !row.trim().is_empty() {
                    println!("{row}");
                }
            }
        }
        for depth in [
            ColorDepth::Ansi16,
            ColorDepth::Ansi256,
            ColorDepth::TrueColor,
        ] {
            println!(
                "   {depth:?}: states sharing a colour: {:?}",
                style_collisions(skin, depth)
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_word_is_painted_once_in_mono_at_40x15() {
        for (_, skin) in SKINS {
            let text = rows(&render(skin, environment(40, 15, ColorDepth::Mono))).join("\n");
            for (word, ..) in STATES {
                let hits = text
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|w| *w == word)
                    .count();
                assert_eq!(hits, 1, "{word} in\n{text}");
            }
        }
    }

    #[test]
    fn every_tone_has_its_own_marker_at_every_depth() {
        // Where colour cannot separate two states, the marker before the word does.
        for (name, skin) in SKINS {
            for depth in [ColorDepth::Mono, ColorDepth::Ansi16, ColorDepth::TrueColor] {
                let text = rows(&render(skin, environment(80, 24, depth)));
                let marker = |word: &str| {
                    let line = text
                        .iter()
                        .find(|l| l.split_whitespace().any(|w| w == word))
                        .unwrap_or_else(|| panic!("{name} {depth:?}: {word} missing"));
                    line.split_whitespace().next().unwrap().to_string()
                };
                let markers: Vec<String> = ["loading", "ready", "degraded", "failed", "stopping"]
                    .iter()
                    .map(|w| marker(w))
                    .collect();
                for i in 0..markers.len() {
                    for j in i + 1..markers.len() {
                        assert_ne!(markers[i], markers[j], "{name} {depth:?}: {markers:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn truecolor_paints_every_state_style_distinctly() {
        for (name, skin) in SKINS {
            assert_eq!(
                style_collisions(skin, ColorDepth::TrueColor),
                vec![],
                "{name}"
            );
        }
    }

    #[test]
    fn the_collision_report_sees_emphasis_not_just_tone() {
        // `stopping` is Neutral+Muted: make muted text the success colour and the
        // report must pair it with `ready`, which a Normal-only check would miss.
        let mut skin = skins::BLACK_ICE;
        skin.palette.text_muted = skin.palette.success;
        let found = style_collisions(skin, ColorDepth::TrueColor);
        assert!(found.contains(&("ready", "stopping")), "{found:?}");
    }

    #[test]
    fn mono_frame_carries_no_colour() {
        let s = render(skins::SWISS_SIGNAL, environment(40, 15, ColorDepth::Mono));
        for y in 0..s.height {
            for x in 0..s.width {
                let st = s.get(x, y).unwrap().style;
                assert!(
                    matches!(st.fg, None | Some(gibson::Color::Reset))
                        && matches!(st.bg, None | Some(gibson::Color::Reset)),
                    "colour at {x},{y}: {st:?}"
                );
            }
        }
    }
}
