//! Refraction of the actual harness surface, followed by an RGB tunnel.
//! Pure inverse sampling: time is an input, never advanced by painting.
use gibson::cell::{Color, Style};
use gibson::raster::{Rgb, RgbRaster};
use gibson::raster_fx::RasterFx;
use gibson::surface::Surface;
use gibson::ColorDepth;

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let c = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).clamp(0.0, 255.0) as u8;
    (c(a.0, b.0), c(a.1, b.1), c(a.2, b.2))
}
/// Actual cell contents are lens-refracted early; later, their luminous
/// fragments orbit the tunnel. Text sampling uses print_str to preserve glyph
/// invariants. All RGB blending is inside an opaque software raster.
pub fn render(source: &Surface, seconds: f32, depth: ColorDepth) -> Surface {
    let w = source.width;
    let h = source.height;
    if w == 0 || h == 0 {
        return source.clone();
    }
    let t = (seconds - 20.0).clamp(0.0, 8.0);
    if t == 0.0 {
        return source.clone();
    }
    let collapse = smooth(1.8, 6.5, t);
    let emerge = smooth(6.0, 8.0, t);
    let mut rgb = RgbRaster::new(w, h.saturating_mul(2));
    let wf = w as f32;
    let hf = h as f32 * 2.0;
    let city = if emerge > 0.0 {
        Some(super::world::raster(w, h, 28.0 + (t - 8.0).max(-0.5)))
    } else {
        None
    };
    for y in 0..rgb.height() {
        for x in 0..rgb.width() {
            let nx = (x as f32 - wf * 0.5) / hf;
            let ny = (y as f32 - hf * 0.5) / hf;
            let r = (nx * nx + ny * ny).sqrt().max(0.009);
            let angle = ny.atan2(nx) + collapse * (2.5 - r * 4.0);
            let z = 1.0 / (r + 0.055);
            let rings = (z * 3.5 - t * 7.0).sin().max(0.0).powf(14.0);
            let spokes = (angle * 12.0 + t * 0.4 + z * 0.12).sin().abs().powf(42.0);
            let fade = (r * 3.0).min(1.0);
            let light = (rings * 0.62 + spokes * 0.5) * fade;
            let hue = 0.5 + 0.5 * (angle * 2.0 - z * 0.13 + t * 0.6).sin();
            let tint = mix((35, 200, 245), (225, 163, 255), hue);
            let mut color = mix((3, 7, 17), tint, light * collapse);
            if let Some(city) = &city {
                color = mix(
                    color,
                    city.get(x as i32, y as i32).unwrap_or((0, 0, 0)),
                    emerge,
                );
            }
            rgb.set(x as i32, y as i32, color);
        }
    }
    RasterFx::apply_chain(
        &mut rgb,
        &[
            RasterFx::Glow {
                radius: 1,
                threshold: 130,
                strength: 0.7,
            },
            RasterFx::Vignette { strength: 0.25 },
        ],
    );
    let mut out = if depth == ColorDepth::Mono {
        rgb.to_mono_surface()
    } else {
        rgb.to_surface()
    };
    let text_strength = 1.0 - smooth(3.2, 6.8, t);
    if text_strength > 0.0 {
        for y in 0..h {
            for x in 0..w {
                let nx = (x as f32 - wf * 0.5) / hf;
                let ny = (y as f32 * 2.0 - hf * 0.5) / hf;
                let r = (nx * nx + ny * ny).sqrt();
                let mut dx = 0.0;
                let mut dy = 0.0;
                let mut rim = 0.0f32;
                // A coherent membrane: expanding lenses share a propagating onset.
                for i in 0..9 {
                    let f = i as f32;
                    let bx = (f * 2.399).sin() * 0.48;
                    let by = (f * 1.731).cos() * 0.38;
                    let radius = (t * 0.10 - f * 0.014).clamp(0.0, 0.24);
                    let vx = nx - bx;
                    let vy = ny - by;
                    let d = (vx * vx + vy * vy).sqrt();
                    if radius > 0.0 && d < radius * 1.25 {
                        let q = d / radius;
                        let lens = (1.0 - q * q).max(0.0) * smooth(0.0, 1.4, t);
                        dx += vx * lens * 0.9;
                        dy += vy * lens * 0.9;
                        rim = rim.max((1.0 - (q - 1.0).abs() * 12.0).max(0.0));
                    }
                }
                let a = ny.atan2(nx) + collapse * 3.0 * (1.0 - r).max(0.0);
                let zoom = 1.0 + collapse * 5.0;
                let sx = (((a.cos() * r * zoom + dx) * hf) + wf * 0.5).round() as i32;
                let sy = (((a.sin() * r * zoom + dy) * hf + hf * 0.5) / 2.0).round() as i32;
                if sx >= 0 && sy >= 0 {
                    if let Some(c) = source.get(sx as u16, sy as u16) {
                        // Wide glyphs cannot be independently resampled; preserve
                        // only complete one-cell samples, leaving RGB beneath.
                        if c.glyph.display_width == 1 && !c.glyph.is_empty() {
                            let bg = c.style.bg.unwrap_or(Color::Rgb(7, 12, 23)).to_rgb();
                            let fg = c.style.fg.unwrap_or(Color::Rgb(200, 215, 230)).to_rgb();
                            let under = rgb.get(x as i32, y as i32 * 2).unwrap_or((0, 0, 0));
                            let b = mix(
                                mix(under, bg, text_strength * (1.0 - collapse * 0.5)),
                                (25, 81, 100),
                                rim * 0.8,
                            );
                            let f = mix(mix(under, fg, text_strength), (147, 232, 255), rim * 0.7);
                            out.print_str(
                                x,
                                y,
                                c.glyph.grapheme.as_str(),
                                Style::new()
                                    .fg(Color::Rgb(f.0, f.1, f.2))
                                    .bg(Color::Rgb(b.0, b.1, b.2)),
                                Some(1),
                            );
                        }
                    }
                }
            }
        }
    }
    // Exit through a growing aperture into the exact next realization, so the
    // terminal changes frequency band continuously rather than switching modes.
    if t > 6.7 {
        let city = super::world::render(w, h, 28.0, depth);
        let radius = smooth(6.7, 8.0, t) * (wf / hf).hypot(1.0);
        for y in 0..h {
            for x in 0..w {
                let dx = (x as f32 - wf * 0.5) / hf;
                let dy = (y as f32 * 2.0 - hf * 0.5) / hf;
                if dx.hypot(dy) < radius {
                    if let Some(cell) = city.get(x, y) {
                        out.set_cell(x, y, cell.clone());
                    }
                }
            }
        }
    }
    out
}
