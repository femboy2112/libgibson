//! One coalescing surface refracts the realized harness before folding inward.
//! Pure inverse sampling: time is an input, never advanced by painting.
use super::identity::{membrane_anchors, GRAPH_POINTS, IDENTITIES};
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
#[derive(Clone, Copy)]
struct Nucleus {
    x: f32,
    y: f32,
    radius2: f32,
    accent: Rgb,
}
#[derive(Clone, Copy, Default)]
struct Film {
    coverage: f32,
    rim: f32,
    dx: f32,
    dy: f32,
    sheen: f32,
    accent: Rgb,
}
/// Summed compact inverse-distance lobes, with an analytic gradient. The
/// visible rim is an isocontour of the SUM: touching blisters really merge,
/// rather than drawing overlapping circles. No simulation history is needed.
fn field(x: f32, y: f32, nuclei: &[Nucleus; 8], gain: f32) -> Film {
    let mut f = 0.0;
    let mut gx = 0.0;
    let mut gy = 0.0;
    let mut rgb = [0.0; 3];
    for n in nuclei {
        let dx = x - n.x;
        let dy = y - n.y;
        let d = dx * dx + dy * dy + n.radius2;
        let q = n.radius2 / d;
        let value = q * q;
        f += value;
        let gradient = -4.0 * value / d;
        gx += dx * gradient;
        gy += dy * gradient;
        rgb[0] += value * n.accent.0 as f32;
        rgb[1] += value * n.accent.1 as f32;
        rgb[2] += value * n.accent.2 as f32;
    }
    let rim = (1.0 - (f - 0.58).abs() / 0.16).clamp(0.0, 1.0);
    let len = gx.hypot(gy).max(0.0001);
    let nx = gx / len;
    let ny = gy / len;
    let coverage = smooth(0.24, 0.85, f) * gain;
    let lens = (1.0 + f * f).recip() * gain * 0.0018;
    Film {
        coverage,
        rim: rim * rim * gain,
        dx: (gx * lens).clamp(-0.11, 0.11),
        dy: (gy * lens).clamp(-0.11, 0.11),
        sheen: ((-nx * 0.6 - ny * 0.8).max(0.0)).powi(8) * coverage,
        accent: (
            (rgb[0] / f.max(0.0001)) as u8,
            (rgb[1] / f.max(0.0001)) as u8,
            (rgb[2] / f.max(0.0001)) as u8,
        ),
    }
}
fn source_at(source: &Surface, x: f32, y: f32) -> Option<&gibson::cell::Cell> {
    if x >= 0.0 && y >= 0.0 && x < source.width as f32 && y < source.height as f32 {
        source.get(x as u16, y as u16)
    } else {
        None
    }
}
/// Actual cells lens-refract first. Their colored strokes then feed the well;
/// the exact upcoming city is already visible inside it before the final exit.
/// Whole one-cell glyphs are copied with print_str; wide pairs are never split.
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
    let collapse = smooth(2.3, 6.9, t);
    let emerge = smooth(5.6, 8.0, t);
    let text_strength = 1.0 - smooth(3.6, 6.6, t);
    let gain = smooth(0.0, 1.0, t);
    let wf = w as f32;
    let hf = h as f32 * 2.0;
    let anchors = membrane_anchors(w, h);
    let nuclei = std::array::from_fn(|i| {
        let (ax, ay) = if i < 4 {
            anchors[i]
        } else if w >= 110 {
            let (x, y) = GRAPH_POINTS[i - 4];
            (0.28 + x * 0.39, 0.68 + y * 0.17)
        } else if w >= 74 {
            let (x, y) = GRAPH_POINTS[i - 4];
            (0.34 + x * 0.59, 0.64 + y * 0.17)
        } else {
            (0.28 + (i - 4) as f32 * 0.14, 0.28 + (i % 2) as f32 * 0.16)
        };
        let age = (t - i as f32 * 0.075).max(0.0);
        let radius = 0.014 + age * 0.022 + age * age * 0.010;
        Nucleus {
            x: (ax * wf - wf * 0.5) / hf,
            y: ay - 0.5,
            radius2: radius * radius,
            accent: IDENTITIES[i % 4].accent,
        }
    });
    let mut rgb = RgbRaster::new(w, h.saturating_mul(2));
    let mut films = Vec::with_capacity(usize::from(w) * usize::from(h));
    let city_rgb = if emerge > 0.0 {
        Some(super::world::raster(w, h, 28.0))
    } else {
        None
    };
    for y in 0..rgb.height() {
        for x in 0..rgb.width() {
            let nx = (x as f32 - wf * 0.5) / hf;
            let ny = (y as f32 - hf * 0.5) / hf;
            let film = field(nx, ny, &nuclei, gain);
            if y % 2 == 0 {
                films.push(film);
            }
            let r = nx.hypot(ny).max(0.009);
            let angle = ny.atan2(nx) + collapse * (2.7 - r * 3.5);
            let z = (r + 0.045).recip();
            let rings = (z * 3.1 - t * 6.6).sin().max(0.0).powi(16);
            let spokes = (angle * 10.0 + t * 0.22 + z * 0.11).sin().abs().powi(48);
            let light = (rings * 0.56 + spokes * 0.55) * (r * 3.5).min(1.0);
            let tint = mix(
                IDENTITIES[0].accent,
                IDENTITIES[2].accent,
                0.5 + 0.5 * (angle * 2.0 - z * 0.1).sin(),
            );
            let mut color = mix((3, 7, 17), tint, light * collapse);
            // Even empty harness cells become the material of the lens. Rim
            // shading lives in subcells while refracted glyphs stay razor sharp.
            let src = source.get(x, y / 2);
            let bg = src
                .and_then(|c| c.style.bg)
                .unwrap_or(Color::Rgb(7, 12, 23))
                .to_rgb();
            color = mix(color, bg, text_strength * (1.0 - collapse * 0.4));
            color = mix(color, film.accent, film.coverage * 0.09 * text_strength);
            color = mix(color, film.accent, film.rim * 0.72 * text_strength);
            color = mix(
                color,
                (213, 244, 248),
                film.sheen * film.rim * 0.85 * text_strength,
            );
            if let Some(city) = &city_rgb {
                let aperture = emerge * (wf / hf).hypot(1.0) * 0.65;
                let inside = 1.0 - smooth(aperture * 0.7, aperture + 0.04, r);
                color = mix(
                    color,
                    city.get(x as i32, y as i32).unwrap_or((0, 0, 0)),
                    inside.max(emerge * emerge),
                );
            }
            rgb.set(x as i32, y as i32, color);
        }
    }
    // Actual source ink becomes radial streaks, retaining all four signatures'
    // colors. A finite sample grid bounds work, independent of elapsed time.
    let material = smooth(2.8, 4.8, t) * (1.0 - smooth(5.4, 7.0, t));
    if material > 0.0 {
        for sy in (1..h).step_by(2) {
            for sx in (1..w).step_by(3) {
                let Some(cell) = source.get(sx, sy) else {
                    continue;
                };
                if cell.glyph.grapheme.trim().is_empty() || cell.is_continuation {
                    continue;
                }
                let vx = (sx as f32 - wf * 0.5) / hf;
                let vy = (sy as f32 * 2.0 - hf * 0.5) / hf;
                let r = vx.hypot(vy) / (1.0 + collapse * 4.0);
                let a = vy.atan2(vx) - collapse * 3.0;
                let px = a.cos() * r * hf + wf * 0.5;
                let py = a.sin() * r * hf + hf * 0.5;
                let tail = 1.0 + material * 0.2;
                let qx = (a + 0.07 * material).cos() * r * tail * hf + wf * 0.5;
                let qy = (a + 0.07 * material).sin() * r * tail * hf + hf * 0.5;
                let ink = cell.style.fg.unwrap_or(Color::Rgb(150, 196, 210)).to_rgb();
                rgb.line(
                    px as i32,
                    py as i32,
                    qx as i32,
                    qy as i32,
                    mix((3, 7, 17), ink, material * 0.7),
                );
            }
        }
    }
    RasterFx::apply_chain(
        &mut rgb,
        &[
            RasterFx::Glow {
                radius: 1,
                threshold: 130,
                strength: 0.55,
            },
            RasterFx::Vignette {
                strength: 0.18 * collapse,
            },
        ],
    );
    let mut out = if depth == ColorDepth::Mono {
        rgb.to_mono_surface()
    } else {
        rgb.to_surface()
    };
    if text_strength > 0.0 {
        for y in 0..h {
            for x in 0..w {
                let film = films[usize::from(y) * usize::from(w) + usize::from(x)];
                let nx = (x as f32 - wf * 0.5) / hf;
                let ny = (y as f32 * 2.0 - hf * 0.5) / hf;
                let r = nx.hypot(ny);
                let a = ny.atan2(nx) + collapse * 3.0 * (1.0 - r).max(0.0);
                let zoom = 1.0 + collapse * 4.0;
                let sx =
                    ((a.cos() * r * zoom + film.dx * (1.0 - collapse)) * hf + wf * 0.5).round();
                let sy = ((a.sin() * r * zoom + film.dy * (1.0 - collapse)) * hf + hf * 0.5)
                    .round()
                    * 0.5;
                if let Some(c) = source_at(source, sx, sy) {
                    if c.glyph.display_width == 1
                        && !c.glyph.is_empty()
                        && !c.glyph.grapheme.trim().is_empty()
                    {
                        let bg = c.style.bg.unwrap_or(Color::Rgb(7, 12, 23)).to_rgb();
                        let fg = c.style.fg.unwrap_or(Color::Rgb(200, 215, 230)).to_rgb();
                        let under = rgb.get(x as i32, y as i32 * 2).unwrap_or((0, 0, 0));
                        let b = mix(
                            under,
                            bg,
                            text_strength * (1.0 - film.coverage) * (1.0 - collapse),
                        );
                        let f = mix(
                            mix(under, fg, text_strength),
                            (223, 245, 255),
                            film.rim * text_strength * 0.55,
                        );
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
    // Buildings are discernible inside the well before the act boundary.
    // Soft RGB depth precedes full-frequency Braille architecture at its center.
    if t > 5.8 {
        let city = super::world::render(w, h, 28.0, depth);
        let radius = smooth(5.8, 8.0, t) * (wf / hf).hypot(1.0);
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
