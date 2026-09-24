//! Borderless software graphics probes. Feedback advances only on explicit ticks;
//! asking for the same frozen frame never deposits another sample into its trail.

use std::time::Duration;

use gibson::cell::{Color, Style};
use gibson::raster::{Rgb, RgbRaster};
use gibson::raster3d::{Camera, Fog, Material, Rasterizer, TriangleMesh};
use gibson::raster_fx::{
    metaballs, palette, radial_glow, ring, vortex, FeedbackBuffer, RasterFx, RasterFxWorkspace,
};
use gibson::{BrailleCanvas, Surface, Transform3, Vec3};

const CYAN: Rgb = (35, 220, 255);
const MAGENTA: Rgb = (255, 36, 155);
const INK: Rgb = (2, 3, 12);

pub struct GraphicalLab {
    renderer: Rasterizer,
    emission: RgbRaster,
    feedback: FeedbackBuffer,
    effects: RasterFxWorkspace,
    scene: usize,
    field_samples: usize,
    feedback_passes: u64,
    time: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feedback_is_ticked_once_and_frozen_reads_are_exact() {
        let mut a = GraphicalLab::new();
        let mut b = GraphicalLab::new();
        for step in 1..=90 {
            let t = step as f32 / 60.0;
            let dt = Duration::from_secs_f64(1.0 / 60.0);
            a.advance(2, 120, 30, t, dt);
            b.advance(2, 120, 30, t, dt);
        }
        assert_eq!(a.feedback_passes, 90);
        let frozen = a.surface(2, 120, 30, 1.5, false);
        assert_eq!(frozen, a.surface(2, 120, 30, 1.5, false));
        assert_eq!(frozen, b.surface(2, 120, 30, 1.5, false));
        assert_eq!(a.feedback_passes, 90);
        assert!(
            a.feedback
                .raster()
                .pixels()
                .iter()
                .filter(|&&rgb| rgb != (0, 0, 0))
                .count()
                > 150
        );
    }

    #[test]
    fn shaded_core_has_volume_and_rgb_diversity() {
        let mut lab = GraphicalLab::new();
        lab.advance(0, 120, 30, 1.1, Duration::from_millis(16));
        assert!(lab.renderer.stats.triangles_drawn > 8);
        assert!(lab.renderer.stats.z_tests > 100);
        let colors: std::collections::HashSet<_> = lab.renderer.raster.pixels().iter().collect();
        assert!(colors.len() > 64);
        assert_eq!(lab.surface(0, 120, 30, 1.1, false).width, 120);
    }

    #[test]
    fn all_graphical_scenes_resize_and_have_mono_density() {
        let mut lab = GraphicalLab::new();
        for scene in 0..7 {
            for (width, height) in [(56, 22), (80, 22), (120, 30), (160, 38)] {
                lab.advance(scene, width, height, 1.0, Duration::from_millis(50));
                let rgb = lab.surface(scene, width, height, 1.0, false);
                assert_eq!((rgb.width, rgb.height), (width, height));
                let mono = lab.surface(scene, width, height, 1.0, true);
                assert_eq!((mono.width, mono.height), (width, height));
                assert!(mono
                    .cells
                    .iter()
                    .all(|cell| !matches!(cell.style.fg, Some(Color::Rgb(..)))));
                // Frozen resize regenerates geometry without adding trail time.
                let changed = lab.surface(scene, width - 2, height - 2, 1.0, false);
                assert_eq!((changed.width, changed.height), (width - 2, height - 2));
            }
        }
    }
}

impl GraphicalLab {
    pub fn new() -> Self {
        Self {
            renderer: Rasterizer::new(1, 2),
            emission: RgbRaster::new(1, 2),
            feedback: FeedbackBuffer::new(1, 2, Duration::from_millis(650)),
            effects: RasterFxWorkspace::default(),
            scene: usize::MAX,
            field_samples: 0,
            feedback_passes: 0,
            time: 0.0,
        }
    }

    fn prepare(&mut self, scene: usize, cols: u16, rows: u16) {
        // Work remains bounded even under hostile PTY dimensions.
        let width = cols.clamp(1, 320);
        let height = rows.clamp(1, 120) * 2;
        if self.renderer.raster.width() != width || self.renderer.raster.height() != height {
            self.renderer = Rasterizer::new(width, height);
            self.emission = RgbRaster::new(width, height);
            self.feedback.resize(width, height);
        }
        if self.scene != scene {
            self.feedback.reset();
            self.feedback_passes = 0;
            self.scene = scene;
        }
    }

    pub fn advance(&mut self, scene: usize, cols: u16, rows: u16, time: f32, dt: Duration) {
        self.prepare(scene, cols, rows);
        self.time = time;
        self.paint(dt);
    }

    pub fn surface(
        &mut self,
        scene: usize,
        cols: u16,
        rows: u16,
        time: f32,
        mono: bool,
    ) -> Surface {
        let resized = self.renderer.raster.width() != cols.clamp(1, 320)
            || self.renderer.raster.height() != rows.clamp(1, 120) * 2
            || self.scene != scene;
        self.prepare(scene, cols, rows);
        if resized {
            self.time = time;
            self.paint(Duration::ZERO);
        }
        let mut surface = if mono {
            self.renderer.raster.to_mono_surface()
        } else {
            self.renderer.raster.to_surface()
        };
        if scene == 6 {
            // Braille geometry retains crisp high-frequency outlines over the
            // lower-resolution RGB light field. Both use the same 3D camera.
            let mut wire = BrailleCanvas::new(surface.width, surface.height);
            let camera = self.camera();
            let transform = Transform3::rotation(0.4, self.time * 0.35, 0.15);
            let mesh = gibson::Mesh::octahedron(1.6);
            for &(a, b) in &mesh.edges {
                if let (Some(a), Some(b)) = (
                    camera.project(
                        transform.apply(mesh.vertices[a]),
                        surface.width,
                        surface.height * 2,
                    ),
                    camera.project(
                        transform.apply(mesh.vertices[b]),
                        surface.width,
                        surface.height * 2,
                    ),
                ) {
                    wire.line(
                        (a.0 * 2.0) as i32,
                        (a.1 * 2.0) as i32,
                        (b.0 * 2.0) as i32,
                        (b.1 * 2.0) as i32,
                    );
                }
            }
            // Preserve the underlying RGB mass as the outline cell's background.
            for y in 0..surface.height {
                for x in 0..surface.width {
                    if let Some(glyph) = wire.glyph_at(x, y).filter(|glyph| *glyph != '\u{2800}') {
                        let mut style = Style::new().bold();
                        if !mono {
                            let rgb = self
                                .renderer
                                .raster
                                .get(i32::from(x), i32::from(y) * 2)
                                .unwrap_or(INK);
                            style = style
                                .fg(Color::Rgb(175, 245, 255))
                                .bg(Color::Rgb(rgb.0, rgb.1, rgb.2));
                        }
                        surface.print_str(x, y, &glyph.to_string(), style, Some(1));
                    }
                }
            }
        }
        surface
    }

    pub fn caption(&self, debug: bool) -> String {
        if debug {
            format!(
                "{} px · triangles {}/{} · z-tests {} · field {} · feedback {} · ←/→ scenes",
                usize::from(self.renderer.raster.width())
                    * usize::from(self.renderer.raster.height()),
                self.renderer.stats.triangles_drawn,
                self.renderer.stats.triangles_submitted,
                self.renderer.stats.z_tests,
                self.field_samples,
                self.feedback_passes,
            )
        } else {
            let detail = match self.scene {
                0 => "Lambert faces / real depth / distance fog",
                1 => "Near cyan / far magenta · z-buffer resolves overlap",
                2 => "Crossing paths / 650 ms feedback half-life / bounded glow",
                3 => "Polar interference / radial waves / RGB palette",
                4 => "Continuous potential / cyan-magenta collision",
                5 => "RGB-only sine warp / chromatic split",
                _ => "RGB volume + Braille edges + ordinary text",
            };
            format!("{detail} · ←/→ scenes · q exit")
        }
    }

    fn camera(&self) -> Camera {
        Camera {
            position: Vec3::new(0.0, 0.7, -5.4),
            target: Vec3::new(0.0, 0.0, 0.0),
            ..Camera::default()
        }
    }

    fn paint(&mut self, dt: Duration) {
        self.renderer.clear(INK);
        self.field_samples = 0;
        match self.scene {
            0 | 1 | 6 => self.geometry(),
            2 => self.beams(dt),
            _ => self.field(),
        }
        let mut fx = vec![RasterFx::Glow {
            radius: 2,
            threshold: if self.scene == 2 { 30 } else { 145 },
            strength: if self.scene == 2 { 1.2 } else { 0.8 },
        }];
        if self.scene == 5 {
            fx.push(RasterFx::SineWarp {
                amplitude: 3.0,
                frequency: 0.12,
                phase: self.time,
            });
            fx.push(RasterFx::ChromaticSplit { offset: 2 });
        }
        fx.push(RasterFx::Vignette { strength: 0.5 });
        self.effects.apply(&mut self.renderer.raster, &fx);
    }

    fn geometry(&mut self) {
        let camera = self.camera();
        self.renderer.fog = Some(Fog {
            color: INK,
            start: 5.0,
            end: 13.0,
        });
        self.renderer.light = Vec3::new(-0.6, 0.9, -1.0);
        let t = self.time;
        let material = |color| Material {
            color,
            ambient: 0.2,
            diffuse: 0.8,
            emissive: 0.03,
        };
        // A perspective floor supplies depth anchors without a dashboard frame.
        for i in -8..=8 {
            let x = i as f32 * 0.7;
            self.renderer.line(
                Vec3::new(x, -1.5, -0.5),
                Vec3::new(x, -1.5, 9.0),
                &camera,
                (8, 40, 58),
            );
            self.renderer.line(
                Vec3::new(-5.6, -1.5, x + 4.0),
                Vec3::new(5.6, -1.5, x + 4.0),
                &camera,
                (8, 40, 58),
            );
        }
        if self.scene == 1 {
            // Alternate submission order every second; geometry/depth semantics
            // stay independent of order (the orbit is continuous).
            let order = if (t as u64).is_multiple_of(2) {
                [0, 1]
            } else {
                [1, 0]
            };
            for i in order {
                let offset = if i == 0 {
                    Vec3::new(-0.35, 0.0, -0.4)
                } else {
                    Vec3::new(0.65, 0.3, 1.2)
                };
                self.renderer.draw_mesh(
                    &TriangleMesh::cube(2.0),
                    Transform3 {
                        ry: t * 0.35,
                        rx: 0.25,
                        offset,
                        ..Transform3::default()
                    },
                    &camera,
                    material(if i == 0 { CYAN } else { MAGENTA }),
                );
            }
        } else {
            // Orbiting satellites expose occlusion, not just shaded polygons.
            for i in 0..8 {
                let a = t * 0.28 + i as f32 * std::f32::consts::TAU / 8.0;
                self.renderer.draw_mesh(
                    &TriangleMesh::box_xyz(0.42, 0.7 + 0.2 * a.sin(), 0.42),
                    Transform3 {
                        ry: -a,
                        offset: Vec3::new(a.cos() * 2.6, -0.5, a.sin() * 2.6),
                        ..Transform3::default()
                    },
                    &camera,
                    material(if i % 2 == 0 { CYAN } else { MAGENTA }),
                );
            }
            self.renderer.draw_mesh(
                &TriangleMesh::octahedron(1.65),
                Transform3::rotation(0.4, t * 0.35, 0.15),
                &camera,
                material((90, 170, 255)),
            );
            self.renderer.draw_mesh(
                &TriangleMesh::cube(0.68),
                Transform3::rotation(-t * 0.4, t * 0.7, 0.2),
                &camera,
                material((255, 120, 200)),
            );
        }
    }

    fn beams(&mut self, dt: Duration) {
        self.emission.clear((0, 0, 0));
        let width = f32::from(self.emission.width());
        let height = f32::from(self.emission.height());
        for side in 0..2 {
            let phase = self.time * 0.85 + side as f32 * 2.1;
            let point = |t: f32| {
                (
                    (0.5 + 0.39 * t.sin()) * width,
                    (0.5 + 0.3 * (t * 2.0 + side as f32).sin()) * height,
                )
            };
            // White heads stay out of history so the trail retains its identity.
            let color = if side == 0 { (5, 60, 80) } else { (80, 5, 50) };
            let mut previous = point(phase - 0.06);
            for step in 0..8 {
                let p = point(phase - 0.06 + step as f32 * 0.01);
                self.emission.line(
                    previous.0 as i32,
                    previous.1 as i32,
                    p.0 as i32,
                    p.1 as i32,
                    color,
                );
                previous = p;
            }
        }
        if !dt.is_zero() {
            self.feedback.update(dt, &self.emission);
            self.feedback_passes += 1;
        }
        self.renderer.raster = self.feedback.raster().clone();
        for side in 0..2 {
            let phase = self.time * 0.85 + side as f32 * 2.1;
            let x = (0.5 + 0.39 * phase.sin()) * width;
            let y = (0.5 + 0.3 * (phase * 2.0 + side as f32).sin()) * height;
            self.renderer
                .raster
                .set(x as i32, y as i32, (230, 255, 255));
        }
    }

    fn field(&mut self) {
        let width = self.renderer.raster.width();
        let height = self.renderer.raster.height();
        let aspect = f32::from(width) / f32::from(height);
        let t = self.time;
        let cyan = [
            (-0.45 + 0.18 * t.sin(), 0.2 * (t * 0.7).cos(), 0.08),
            (-0.2, -0.2, 0.04),
        ];
        let acid = [
            (0.45 - 0.18 * t.sin(), 0.2 * (t * 0.9).sin(), 0.08),
            (0.3, -0.25, 0.04),
        ];
        for y in 0..height {
            for x in 0..width {
                let nx = (f32::from(x) / f32::from(width) - 0.5) * 2.0 * aspect;
                let ny = (f32::from(y) / f32::from(height) - 0.5) * 2.0;
                let rgb = if self.scene == 4 {
                    let c = metaballs(nx, ny, &cyan);
                    let a = metaballs(nx, ny, &acid);
                    let energy = ((c + a - 0.3) * 0.75).clamp(0.0, 1.0);
                    let tint = palette((a / (a + c + 0.001)).clamp(0.0, 1.0), CYAN, MAGENTA);
                    let rim = ring(c + a, 0.0, 1.3, 0.11);
                    palette((energy + rim * 0.35).clamp(0.0, 1.0), INK, tint)
                } else {
                    let radial = radial_glow(nx, ny, 1.4);
                    let wave = vortex(nx, ny, t * 0.4, 5.0);
                    let tint = palette(wave, (10, 100, 200), MAGENTA);
                    let pulse = ((nx.hypot(ny) * 15.0 - t * 2.0).sin() * 0.5 + 0.5).powi(4);
                    palette(
                        (radial * (0.2 + 0.7 * wave) + pulse * 0.16).clamp(0.0, 1.0),
                        INK,
                        tint,
                    )
                };
                self.renderer.raster.set(i32::from(x), i32::from(y), rgb);
                self.field_samples += 1;
            }
        }
    }
}
