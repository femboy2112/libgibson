//! Glyph Capability Lab — a visual probe for the glyph-realization axis.
//!
//! Terminal *protocols* (color, synchronized updates, CSI L, ...) can be
//! detected. A font's *glyph repertoire* cannot: there is no reliable universal
//! query for whether the loaded font actually contains a given glyph family.
//! This probe renders representative families and the same sub-cell field in
//! every [`SubcellGlyphMode`], so a human can SEE which ones their terminal
//! realizes. It prints to stdout in the primary buffer (no alternate screen,
//! no raw mode), so it works verbatim on a Linux kernel virtual console and
//! through a pipe.
//!
//! Observed platform evidence (documented in `docs/GLYPHS.md`): the Linux kernel
//! VT (TERM=linux, Ctrl+Alt+F1) accepts UTF-8 but renders U+2800..U+28FF Braille
//! as unrelated glyphs, because its default console bitmap font has no Braille
//! repertoire. Block elements (U+2580..U+259F, classic VGA/CP437) render fine.
//!
//! Run: cargo run --example glyph_capability_lab
//!      cargo run --example glyph_capability_lab -- --glyphs=block
//!      TERM=linux cargo run --example glyph_capability_lab   # see Auto's pick

use gibson::{detect_glyph_mode, BrailleCanvas, SubcellGlyphMode};

/// Builds one recognizable sub-cell figure: a framed panel, a circle, a sine
/// wave, a full diagonal and a solid block. Deterministic; no randomness.
fn sample_field(cols: u16, rows: u16) -> BrailleCanvas {
    let mut c = BrailleCanvas::new(cols, rows);
    let (pw, ph) = (c.pixel_width() as i32, c.pixel_height() as i32);

    // Border.
    c.rect(0, 0, pw, ph);
    // Circle centered in the panel.
    let (ccx, ccy) = (pw / 2, ph / 2);
    c.circle(ccx, ccy, (ph / 2 - 3).max(2));
    // Full diagonal (tests line connectivity under fallback).
    c.line(1, 1, pw - 2, ph - 2);
    // Sine wave across the width.
    let mut prev: Option<(i32, i32)> = None;
    for x in 1..pw - 1 {
        let phase = x as f32 / pw as f32 * std::f32::consts::TAU * 2.0;
        let y = (ph as f32 / 2.0 + phase.sin() * (ph as f32 / 2.5)) as i32;
        if let Some((px, py)) = prev {
            c.line(px, py, x, y);
        }
        prev = Some((x, y));
    }
    // A solid block in the corner (tests full-coverage realization: ⣿ / █ / @).
    c.filled_rect(2, 2, 6, 8);
    c
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let has = |s: &str| args.iter().any(|a| a == s);
    let value = |p: &str| args.iter().find_map(|a| a.strip_prefix(p));

    if has("--help") {
        println!(
            "glyph_capability_lab — visual probe for the glyph-realization axis\n\n\
             Prints representative glyph families and one sub-cell field realized\n\
             in every mode, for VISUAL inspection (protocols cannot prove a font).\n\n\
             --glyphs=auto|braille|halfblock|block|ascii  (or LIBGIBSON_GLYPHS)\n\
             Prints and exits; safe on a raw Linux VT and through a pipe."
        );
        return;
    }

    let term = std::env::var("TERM").unwrap_or_else(|_| "(unset)".into());
    let colorterm = std::env::var("COLORTERM").unwrap_or_else(|_| "(unset)".into());
    let resolved = match detect_glyph_mode(value("--glyphs="), |k| std::env::var(k).ok()) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };

    println!("LibGibson — Glyph Capability Lab");
    println!("================================\n");
    println!(
        "Terminal protocols cannot report a font's glyph repertoire. The families\n\
         below must be inspected BY EYE. If a panel is garbage but a lower one is\n\
         clean, your font lacks that family — select a lower --glyphs mode.\n"
    );
    println!("  TERM={term}   COLORTERM={colorterm}");
    println!(
        "  Auto-resolved mode for this terminal: {}  (override with --glyphs=)\n",
        resolved.as_str()
    );

    println!("1. ASCII  (universally realizable)");
    println!("     .:-=+*#@   ABCabc012  |/\\_-");
    println!("2. Box drawing  (U+2500..)");
    println!("     ┌─┬─┐  ├─┼─┤  └─┴─┘  ╭──╮  ═ ║ ╔ ╗");
    println!("3. Block elements  (U+2580.., classic CP437 — reliable on the Linux VT)");
    println!("     ░▒▓█  ▀▄  ▌▐  ▖▗▘▙▚▛▜▝▞▟");
    println!("4. Half blocks  (the subset this engine uses for HalfBlock1x2)");
    println!("     ▀ upper   ▄ lower   █ full");
    println!("5. Braille  (U+2800..U+28FF — UTF-8 support does NOT guarantee this)");
    println!("     ⠁⠃⠇⠏⠟⠿⡿⣿   ⣿⣿⣿   (and blank ⠀)\n");

    // The same field, four ways, side by side.
    let (cols, rows) = (22u16, 9u16);
    let field = sample_field(cols, rows);
    let modes = [
        SubcellGlyphMode::Braille2x4,
        SubcellGlyphMode::HalfBlock1x2,
        SubcellGlyphMode::Block,
        SubcellGlyphMode::Ascii,
    ];
    let panels: Vec<(&str, Vec<String>)> = modes
        .iter()
        .map(|&m| (m.as_str(), field.to_lines_mode(m)))
        .collect();

    println!("The same sub-cell field, realized four ways");
    println!("--------------------------------------------");
    let colw = cols as usize;
    let gap = "   ";
    // Header row.
    let header: Vec<String> = panels
        .iter()
        .map(|(name, _)| format!("{name:<colw$}"))
        .collect();
    println!("  {}", header.join(gap));
    for r in 0..rows as usize {
        let row: Vec<String> = panels
            .iter()
            .map(|(_, lines)| format!("{:<colw$}", lines[r]))
            .collect();
        println!("  {}", row.join(gap));
    }

    println!(
        "\nVisual inspection required: terminal protocols do not prove font\n\
         repertoire. On TERM=linux, Auto avoids Braille by policy (a custom\n\
         console font could still contain it; --glyphs=braille forces it)."
    );
}
