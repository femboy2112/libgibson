//! One deliberately overcommitted bitmap title, not a font or text-morph API.
//! The whole object approaches from depth; bevel, extrusion and highlight are
//! sampled in software RGB before the terminal sees any cells.
use gibson::raster::{Rgb, RgbRaster};

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}
fn shade(c: Rgb, gain: f32) -> Rgb {
    (
        (c.0 as f32 * gain).clamp(0., 255.) as u8,
        (c.1 as f32 * gain).clamp(0., 255.) as u8,
        (c.2 as f32 * gain).clamp(0., 255.) as u8,
    )
}
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0., 1.);
    let c = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
    (c(a.0, b.0), c(a.1, b.1), c(a.2, b.2))
}
// Compact mixed-case original lettering: l/i have a narrower advance, giving
// the wide G and the lower-case bowls enough room even at 56 terminal columns.
const LETTERS: [(u8, [u8; 7]); 9] = [
    (3, [6, 2, 2, 2, 2, 2, 7]),
    (3, [2, 0, 6, 2, 2, 2, 7]),
    (5, [16, 16, 22, 25, 17, 17, 30]),
    (5, [14, 17, 16, 23, 17, 17, 14]),
    (3, [2, 0, 6, 2, 2, 2, 7]),
    (5, [16, 16, 22, 25, 17, 17, 30]),
    (5, [0, 0, 15, 16, 14, 1, 30]),
    (5, [0, 0, 14, 17, 17, 17, 14]),
    (5, [0, 0, 22, 25, 17, 17, 17]),
];
const WIDTH: f32 = 47.;
const fn title_bitmap() -> [u64; 7] {
    let mut rows = [0u64; 7];
    let mut letter = 0;
    let mut offset = 0;
    while letter < LETTERS.len() {
        let (width, pixels) = LETTERS[letter];
        let mut row = 0;
        while row < 7 {
            let mut x = 0;
            while x < width {
                if pixels[row] & (1 << (width - 1 - x)) != 0 {
                    rows[row] |= 1 << (offset + x);
                }
                x += 1;
            }
            row += 1;
        }
        offset += width + 1;
        letter += 1;
    }
    rows
}
const TITLE_BITMAP: [u64; 7] = title_bitmap();
fn glyph_at(u: f32, v: f32) -> bool {
    (0.0..WIDTH).contains(&u)
        && (0.0..7.0).contains(&v)
        && TITLE_BITMAP[v.floor() as usize] & (1 << u.floor() as u64) != 0
}

#[derive(Clone, Copy)]
struct Layout {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    arch: f32,
    skew: f32,
    depth: f32,
    reveal: f32,
    sharp: bool,
}
impl Layout {
    fn at(width: u16, height: u16, seconds: f32) -> Self {
        let w = width as f32;
        let h = height as f32;
        let entrance = smooth((seconds - 62.7) / 2.8);
        let scale = 0.31 + entrance * 0.69;
        let narrow = width < 74;
        let face_h = h * 0.20;
        let depth = (h * if narrow { 0.032 } else { 0.050 }).max(1.) * scale;
        let face_w = w * if narrow { 0.88 } else { 0.86 } * scale;
        Self {
            x: (w - face_w) * 0.5 - depth * 0.36,
            y: h * (0.063 + (1. - entrance) * 0.28),
            width: face_w,
            height: face_h * scale,
            arch: h * if narrow { 0. } else { 0.012 } * scale,
            skew: face_h * if narrow { 0.04 } else { 0.14 } * scale,
            depth,
            reveal: smooth((seconds - 62.7) / 0.65),
            sharp: narrow,
        }
    }
    fn uv(self, x: f32, y: f32) -> (f32, f32) {
        // Invert a shallow arch + italic skew. Two fixed substitutions avoid a
        // per-pixel iterative solver while retaining a smooth continuous outline.
        let mut u = (x - self.x) / self.width;
        let mut v = 0.;
        for _ in 0..2 {
            v = (y - self.y - self.arch * (u * 2. - 1.).powi(2)) / self.height;
            u = (x - self.x - self.skew * (1. - v)) / self.width;
        }
        (u * WIDTH, v * 7.)
    }
    fn contains(self, x: f32, y: f32) -> bool {
        let (u, v) = self.uv(x, y);
        glyph_at(u, v)
    }
    fn coverage(self, x: f32, y: f32) -> f32 {
        if self.sharp {
            return self.contains(x + 0.5, y + 0.5) as u8 as f32;
        }
        let mut hits = 0;
        for dy in [0.25, 0.75] {
            for dx in [0.25, 0.75] {
                hits += self.contains(x + dx, y + dy) as u8;
            }
        }
        hits as f32 * 0.25
    }
    fn extent(self) -> (i32, i32, i32, i32) {
        (
            (self.x - 1.).floor() as i32,
            (self.y - 1.).floor() as i32,
            (self.x + self.width + self.skew + (self.depth.ceil() + 1.) * 0.8 + 1.).ceil() as i32,
            (self.y + self.height + self.arch + self.depth.ceil() + 2.).ceil() as i32,
        )
    }
}

pub fn bounds(width: u16, height: u16, seconds: f32) -> Option<(i32, i32, i32, i32)> {
    if width < 12 || height < 14 || seconds <= 62.7 {
        return None;
    }
    let layout = Layout::at(width, height, seconds);
    let (mut x0, mut y0, mut x1, mut y1) = layout.extent();
    if seconds >= 65.4 {
        let subtitle = Subtitle::at(width, height, seconds);
        let (sx0, sy0, sx1, sy1) = subtitle.extent();
        x0 = x0.min(sx0);
        y0 = y0.min(sy0);
        x1 = x1.max(sx1);
        y1 = y1.max(sy1);
    }
    if (65.67..66.63).contains(&seconds) {
        let radius = sparkle_radius(height);
        let x = (layout.x + layout.width * 0.80) as i32;
        let y = (layout.y + layout.height * 0.19) as i32;
        x0 = x0.min(x - radius);
        y0 = y0.min(y - radius);
        x1 = x1.max(x + radius + 1);
        y1 = y1.max(y + radius + 1);
    }
    Some((x0, y0, x1, y1))
}
fn chrome(t: f32) -> Rgb {
    const STOPS: [(f32, Rgb); 7] = [
        (0., (250, 255, 252)),
        (0.25, (150, 229, 249)),
        (0.44, (100, 159, 195)),
        (0.50, (76, 130, 179)),
        (0.54, (255, 255, 234)),
        (0.78, (220, 230, 218)),
        (1., (136, 196, 226)),
    ];
    for pair in STOPS.windows(2) {
        if t <= pair[1].0 {
            return mix(
                pair[0].1,
                pair[1].1,
                (t - pair[0].0) / (pair[1].0 - pair[0].0),
            );
        }
    }
    STOPS[6].1
}
fn small_glyph(c: char) -> [u8; 5] {
    match c {
        'H' => [5, 5, 7, 5, 5],
        'A' => [2, 5, 7, 5, 5],
        'C' => [3, 4, 4, 4, 3],
        'K' => [5, 5, 6, 5, 5],
        'T' => [7, 2, 2, 2, 2],
        'E' => [7, 4, 6, 4, 7],
        'P' => [6, 5, 6, 4, 4],
        'L' => [4, 4, 4, 4, 7],
        'N' => [5, 7, 7, 7, 5],
        '!' => [2, 2, 2, 0, 2],
        _ => [0; 5],
    }
}
fn wide_glyph(c: char) -> [u8; 7] {
    match c {
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'N' => [17, 25, 25, 21, 19, 19, 17],
        '!' => [1, 1, 1, 1, 1, 0, 1],
        _ => [0; 7],
    }
}
#[derive(Clone, Copy)]
struct Subtitle {
    x: f32,
    y: f32,
    scale: f32,
    width: f32,
    height: f32,
    reveal: f32,
    narrow: bool,
}
impl Subtitle {
    fn at(width: u16, height: u16, seconds: f32) -> Self {
        let narrow = width < 74;
        let logical_width = if narrow { 55. } else { 85. };
        let scale = if narrow {
            (width as f32 - 2.) / logical_width
        } else {
            (width as f32 * 0.74 / logical_width).min(1.40)
        };
        let reveal = smooth((seconds - 65.4) / 1.4);
        let text_width = logical_width * scale;
        Self {
            x: (width as f32 - text_width) * 0.5,
            y: height as f32 * 0.81 + (1. - reveal),
            scale,
            width: text_width,
            height: (if narrow { 5. } else { 7. }) * scale,
            reveal,
            narrow,
        }
    }
    fn extent(self) -> (i32, i32, i32, i32) {
        (
            self.x.floor() as i32,
            self.y.floor() as i32,
            (self.x + self.width + if self.narrow { 0. } else { 1.5 }).ceil() as i32,
            (self.y + self.height + 2.5).ceil() as i32,
        )
    }
    fn contains(self, x: f32, y: f32) -> bool {
        let v = (y - self.y) / self.scale;
        let rows = if self.narrow { 5. } else { 7. };
        if !(0.0..rows).contains(&v) {
            return false;
        }
        let skew = if self.narrow { 0. } else { (rows - v) * 0.10 };
        let u = (x - self.x - skew) / self.scale;
        if u < 0. {
            return false;
        }
        let mut offset = 0.;
        for c in "HACK THE PLANET!".chars() {
            if c == ' ' {
                offset += if self.narrow { 1. } else { 3. };
                continue;
            }
            let glyph_width = if c == '!' {
                1
            } else if self.narrow {
                3
            } else {
                5
            };
            if u >= offset && u < offset + glyph_width as f32 {
                let bits = if self.narrow {
                    if c == '!' {
                        [1, 1, 1, 0, 1][v as usize]
                    } else {
                        small_glyph(c)[v as usize]
                    }
                } else {
                    wide_glyph(c)[v as usize]
                };
                return bits & (1 << (glyph_width - 1 - (u - offset).floor() as i32)) != 0;
            }
            offset += glyph_width as f32 + 1.;
        }
        false
    }
}
fn subtitle(raster: &mut RgbRaster, seconds: f32) {
    let display = Subtitle::at(raster.width(), raster.height(), seconds);
    if display.reveal <= 0. {
        return;
    }
    let (x0, y0, x1, y1) = display.extent();
    for y in y0.max(0)..y1.min(i32::from(raster.height())) {
        for x in x0.max(0)..x1.min(i32::from(raster.width())) {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            if display.contains(px, py - 1.) {
                raster.blend(x, y, (10, 18, 30), display.reveal);
            }
            if display.contains(px, py) {
                let row = (py - display.y) / display.height;
                let color = if row < 0.32 {
                    (255, 240, 181)
                } else if row < 0.65 {
                    (255, 216, 130)
                } else {
                    (243, 184, 82)
                };
                raster.blend(x, y, color, display.reveal);
            }
        }
    }
    let yy = (display.y + display.height + 1.5).floor() as i32;
    let half = (display.width * 0.34) as i32;
    raster.line(
        raster.width() as i32 / 2 - half,
        yy,
        raster.width() as i32 / 2 + half,
        yy,
        shade((181, 130, 62), display.reveal * 0.7),
    );
}
fn sparkle_radius(height: u16) -> i32 {
    (height as f32 * 0.075).floor().clamp(2., 6.) as i32
}

pub fn paint(raster: &mut RgbRaster, seconds: f32) {
    if raster.width() < 12 || raster.height() < 14 || seconds <= 62.7 {
        return;
    }
    let layout = Layout::at(raster.width(), raster.height(), seconds);
    let (x0, y0, x1, y1) = layout.extent();
    let xrange = x0.max(0)..x1.min(i32::from(raster.width()));
    let yrange = y0.max(0)..y1.min(i32::from(raster.height()));
    let depth_steps = layout.depth.ceil() as i32;
    let width = usize::from(raster.width());
    let height = usize::from(raster.height());
    let mut mask = vec![0u8; width * height];
    for y in yrange.clone() {
        for x in xrange.clone() {
            mask[y as usize * width + x as usize] =
                (layout.coverage(x as f32, y as f32) * 4.) as u8;
        }
    }
    let coverage_at = |x: i32, y: i32| -> f32 {
        if x < 0 || y < 0 || x as usize >= width || y as usize >= height {
            0.
        } else {
            mask[y as usize * width + x as usize] as f32 * 0.25
        }
    };
    // Dark silhouette separates the title from the atmospheric limb. Side faces
    // are opaque RGB layers, not terminal-cell alpha or a terminal image protocol.
    for depth in (0..=depth_steps + 1).rev() {
        for y in yrange.clone() {
            for x in xrange.clone() {
                let dx = depth as f32 * 0.8;
                let dy = depth as f32;
                let source_x = x as f32 - dx;
                let fraction = source_x - source_x.floor();
                let coverage = coverage_at(source_x.floor() as i32, y - depth) * (1. - fraction)
                    + coverage_at(source_x.floor() as i32 + 1, y - depth) * fraction;
                if coverage == 0. {
                    continue;
                }
                let color = if depth > depth_steps {
                    (6, 10, 22)
                } else {
                    let q = depth as f32 / layout.depth.max(1.);
                    let (_, v) = layout.uv(x as f32 - dx, y as f32 - dy);
                    let side = mix((23, 65, 111), (45, 20, 76), q);
                    if v > 3.2 && v < 4.1 {
                        mix(side, (96, 76, 46), 0.5)
                    } else {
                        side
                    }
                };
                raster.blend(x, y, color, coverage * layout.reveal);
            }
        }
    }
    // A narrow ink keyline separates the chrome front from its extrusion and
    // keeps counters open. Reflections must not erase the letter silhouettes.
    for y in yrange.clone() {
        for x in xrange.clone() {
            let neighbors = coverage_at(x - 1, y)
                .max(coverage_at(x + 1, y))
                .max(coverage_at(x, y - 1))
                .max(coverage_at(x, y + 1));
            let edge = (neighbors - coverage_at(x, y)).max(0.);
            if edge > 0. {
                raster.blend(x, y, (6, 13, 27), edge * layout.reveal * 0.85);
            }
        }
    }
    let sweep = smooth((seconds - 65.7) / 1.8);
    for y in yrange {
        for x in xrange.clone() {
            let coverage = coverage_at(x, y);
            if coverage == 0. {
                continue;
            }
            let (u, v) = layout.uv(x as f32 + 0.5, y as f32 + 0.5);
            let mut color = chrome(v / 7.);
            let upper_edge = !layout.contains(x as f32 - 0.45, y as f32 - 0.65);
            let lower_edge = !layout.contains(x as f32 + 1.0, y as f32 + 1.0);
            if upper_edge {
                color = mix(color, (248, 255, 238), 0.72);
            } else if lower_edge {
                color = mix(color, (89, 165, 212), 0.30);
            }
            let streak = if sweep > 0. && sweep < 1. {
                (1. - ((u / WIDTH - sweep) * 13. + v * 0.075).abs()).clamp(0., 1.)
            } else {
                0.
            };
            color = mix(color, (255, 255, 255), streak * 0.90);
            raster.blend(x, y, color, coverage * layout.reveal);
        }
    }
    // A single brief four-point sparkle crosses the right bevel at the lock-in.
    let flash = (1. - ((seconds - 66.15) / 0.48).abs()).clamp(0., 1.);
    if flash > 0. {
        let x = (layout.x + layout.width * 0.80) as i32;
        let y = (layout.y + layout.height * 0.19) as i32;
        let radius = sparkle_radius(raster.height());
        for offset in -radius..=radius {
            let gain = flash * (1. - offset.unsigned_abs() as f32 / (radius + 1) as f32);
            raster.blend(x + offset, y, (223, 253, 255), gain);
            raster.blend(x, y + offset, (249, 249, 224), gain * 0.8);
        }
    }
    subtitle(raster, seconds);
}
