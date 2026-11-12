//! FACADE SIGNAL: a live grid raycaster rendered onto the faces of a 3D wireframe city.
//!
//! The raycast view (a seeded maze, [`gibson::raycast`]) renders offscreen into an
//! `RgbRaster` every frame; [`Rasterizer::textured_quad`] maps it perspective-correctly
//! onto building facades. The city is drawn as depth-tested wireframe edges, with dark
//! filled bodies where a building needs a surface. Edges, bodies and facades share one
//! z-buffer, so an edge in front of a facade draws over it and the facade hides whatever
//! stands behind it, whatever the draw order.
//!
//! Run:          cargo run --release --example facade_signal
//! Receipts:     cargo run --release --example facade_signal -- --frames 120 [--size 120x40]
//! Still image:  cargo run --release --example facade_signal -- --ppm out.ppm --t 3.5
//! Options:      --seed N, --reduced-motion (or REDUCE_MOTION=1): one settled frame, no animation.
//!
//! Fallbacks: a monochrome terminal (NO_COLOR, TERM=dumb, or a mono colour depth) and a
//! terminal smaller than 60×20 get a plain text panel instead of a dithered image.
//! Keys: q / Esc / Ctrl-C quit.
use gibson::capability::ColorDepth;
use gibson::context::{Context, RenderMode};
use gibson::geom::{Transform3, Vec3};
use gibson::raster::{Rgb, RgbRaster};
use gibson::raster3d::{Camera, Fog, Material, Rasterizer, TriangleMesh};
use gibson::raycast::{self, GridMap, RayView, Texels};
use gibson::{Event, KeyCode, KeyModifiers, Node, Style};
use std::io;
use std::time::{Duration, Instant};

const MIN_COLS: u16 = 60;
const MIN_ROWS: u16 = 20;
/// Offscreen raycast texture size (pixels). Fixed, so per-frame raycast work is bounded
/// independently of the terminal: at most 2·W·H writes + W·max_steps grid steps.
const SIGNAL_W: u16 = 64;
const SIGNAL_H: u16 = 48;
const FPS: u32 = 20;

/// One building: footprint centre (x, z), half-extents, height, and whether its
/// front face (−z side) carries the signal.
#[derive(Clone, Copy)]
struct Building {
    x: f32,
    z: f32,
    hx: f32,
    hz: f32,
    h: f32,
    signal: bool,
}

fn mix64(a: u64, b: u64) -> u64 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 32)
}

/// A deterministic city for `seed`: a 5×5 block grid; the central blocks carry signal facades.
fn city(seed: u64) -> Vec<Building> {
    let mut out = Vec::new();
    for gz in -2i32..=2 {
        for gx in -2i32..=2 {
            let r = mix64(seed, ((gx + 8) * 31 + gz + 8) as u64);
            let h = 1.5 + (r % 1000) as f32 / 1000.0 * 3.0;
            let signal = (gx == 0 && gz == 0) || (gx == -1 && gz == 1) || (gx == 1 && gz == -1);
            out.push(Building {
                x: gx as f32 * 3.0,
                z: gz as f32 * 3.0,
                hx: 0.9 + ((r >> 12) % 300) as f32 / 1000.0,
                hz: 0.9 + ((r >> 24) % 300) as f32 / 1000.0,
                // Signal towers rise above the rest so their facades read over the city.
                h: if signal {
                    6.5 + (r % 7) as f32 / 7.0
                } else {
                    h
                },
                signal,
            });
        }
    }
    out
}

struct SignalTexels;
impl Texels for SignalTexels {
    fn wall(&self, material: u8, u: f64, v: f64, d: f64) -> Rgb {
        let k = 1.0 / (1.0 + 0.04 * d * d);
        let base: Rgb = match material {
            1 => {
                if (v * 6.0).fract() < 0.12
                    || (u * 3.0 + ((v * 6.0).floor() % 2.0) * 0.5).fract() < 0.07
                {
                    (40, 30, 36)
                } else {
                    (150, 72, 64)
                }
            }
            2 => {
                if (u * 8.0).fract() < 0.12 || (v * 8.0).fract() < 0.1 {
                    (120, 240, 210)
                } else {
                    (18, 70, 76)
                }
            }
            _ => {
                if (u * 10.0).fract() < 0.3 {
                    (110, 70, 170)
                } else {
                    (64, 40, 110)
                }
            }
        };
        let f = |c: u8| (f64::from(c) * k) as u8;
        (f(base.0), f(base.1), f(base.2))
    }
    fn floor(&self, x: f64, y: f64, d: f64) -> Rgb {
        let k = 1.0 / (1.0 + 0.05 * d * d);
        let tile = ((x.floor() + y.floor()) as i64).rem_euclid(2) == 0;
        let c = if tile { 46.0 } else { 30.0 };
        ((c * k) as u8, (c * k * 0.8) as u8, (c * k * 1.4) as u8)
    }
    fn ceiling(&self, _x: f64, _y: f64, d: f64) -> Rgb {
        let k = 1.0 / (1.0 + 0.05 * d * d);
        ((10.0 * k) as u8, (12.0 * k) as u8, (34.0 * k) as u8)
    }
}

/// The raycast world: a seeded maze and the walker's patrol route through it.
struct Signal {
    map: GridMap,
    /// Cell centres from the start cell to the farthest reachable cell (BFS).
    route: Vec<(f64, f64)>,
}

impl Signal {
    fn new(seed: u64) -> Self {
        let map = GridMap::maze(31, 31, seed);
        // The maze is a tree, so BFS twice gives its longest corridor: the patrol route.
        let (far, _) = Self::bfs(&map, (1, 1));
        let (end, parent) = Self::bfs(&map, far);
        let w = i64::from(map.width());
        let idx = |(x, y): (i64, i64)| (y * w + x) as usize;
        let mut route = Vec::new();
        let mut at = idx(end);
        loop {
            route.push(((at as i64 % w) as f64 + 0.5, (at as i64 / w) as f64 + 0.5));
            if at == idx(far) {
                break;
            }
            at = parent[at];
        }
        Self { map, route }
    }

    /// Breadth-first search over open cells from `from`: the last cell reached
    /// (a farthest one) and each cell's parent index.
    fn bfs(map: &GridMap, from: (i64, i64)) -> ((i64, i64), Vec<usize>) {
        let (w, h) = (i64::from(map.width()), i64::from(map.height()));
        let idx = |x: i64, y: i64| (y * w + x) as usize;
        let mut parent = vec![usize::MAX; (w * h) as usize];
        let mut queue = std::collections::VecDeque::from([from]);
        parent[idx(from.0, from.1)] = idx(from.0, from.1);
        let mut last = from;
        while let Some((x, y)) = queue.pop_front() {
            last = (x, y);
            for (dx, dy) in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if map.get(nx, ny) == 0 && parent[idx(nx, ny)] == usize::MAX {
                    parent[idx(nx, ny)] = idx(x, y);
                    queue.push_back((nx, ny));
                }
            }
        }
        (last, parent)
    }

    /// The walker at time `t`: walks the route at 1.6 cells/s, there and back, looking
    /// along its direction of travel.
    fn walker(&self, t: f64) -> RayView {
        let n = self.route.len();
        if n < 2 {
            return RayView::default();
        }
        let span = (n - 1) as f64;
        let d = (t * 1.6).rem_euclid(2.0 * span);
        let (d, back) = if d <= span {
            (d, false)
        } else {
            (2.0 * span - d, true)
        };
        let i = (d.floor() as usize).min(n - 2);
        let f = d - i as f64;
        let (a, b) = (self.route[i], self.route[i + 1]);
        let (x, y) = (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f);
        let (mut dx, mut dy) = (b.0 - a.0, b.1 - a.1);
        if back {
            (dx, dy) = (-dx, -dy);
        }
        RayView {
            x,
            y,
            angle: dy.atan2(dx),
            ..RayView::default()
        }
    }
}

/// The orbiting city camera at time `t`.
fn camera(t: f64) -> Camera {
    let a = (t * 0.18) as f32 + 0.6;
    Camera {
        position: Vec3::new(
            a.sin() * 17.0,
            8.5 + (t as f32 * 0.21).sin() * 1.2,
            -a.cos() * 17.0,
        ),
        target: Vec3::new(0.0, 3.2, 0.0),
        ..Camera::default()
    }
}

const EDGE: Rgb = (90, 230, 255);
const EDGE_SIGNAL: Rgb = (255, 210, 90);
const GROUND: Rgb = (30, 60, 90);
const SKY: Rgb = (4, 6, 16);

/// Per-frame counters, for receipts.
#[derive(Default, Clone, Copy)]
struct FrameStats {
    raycast_pixels: u64,
    raycast_steps: u64,
    facades_drawn: u32,
    z_tests: u64,
}

/// Renders the whole scene at time `t` into `r`, reusing `signal` as the offscreen view.
fn render_scene(
    r: &mut Rasterizer,
    signal: &mut RgbRaster,
    world: &Signal,
    buildings: &[Building],
    t: f64,
    sentinel: Option<Rgb>,
) -> FrameStats {
    let mut stats = FrameStats::default();
    match sentinel {
        // Tests replace the live view with a flat colour to measure visible facade area.
        Some(c) => signal.clear(c),
        None => {
            let rs = raycast::render(&world.map, &world.walker(t), &SignalTexels, signal);
            stats.raycast_pixels = rs.pixels_written;
            stats.raycast_steps = rs.grid_steps;
        }
    }

    let cam = camera(t);
    r.clear(SKY);
    r.fog = Some(Fog {
        color: SKY,
        start: 10.0,
        end: 34.0,
    });
    // Ground grid.
    for i in -8i32..=8 {
        let c = i as f32 * 1.5;
        r.line(
            Vec3::new(c, 0.0, -12.0),
            Vec3::new(c, 0.0, 12.0),
            &cam,
            GROUND,
        );
        r.line(
            Vec3::new(-12.0, 0.0, c),
            Vec3::new(12.0, 0.0, c),
            &cam,
            GROUND,
        );
    }
    for b in buildings {
        // A dark body so a building occludes what stands behind it.
        let body = TriangleMesh::box_xyz(b.hx * 2.0, b.h, b.hz * 2.0);
        r.draw_mesh(
            &body,
            Transform3 {
                offset: Vec3::new(b.x, b.h / 2.0, b.z),
                ..Transform3::default()
            },
            &cam,
            Material {
                color: (8, 12, 22),
                ambient: 0.9,
                diffuse: 0.1,
                emissive: 0.0,
            },
        );
        // Signal towers carry the live view on every side face. Each facade sits a hair
        // outside its face, so it wins the depth test against its own body but not
        // against anything nearer, and is drawn only when the camera is outside that face
        // (a facade seen from behind would show the view mirrored).
        if b.signal {
            let (x0, x1, z0, z1) = (b.x - b.hx, b.x + b.hx, b.z - b.hz, b.z + b.hz);
            let (inset, top, bottom, eps) = (0.12, b.h - 0.3, 0.4, 0.01);
            let p = |x, y, z| Vec3::new(x, y, z);
            // TL → TR → BR → BL as seen from outside each face.
            let faces = [
                (cam.position.z < z0 - eps, {
                    let z = z0 - eps;
                    [
                        p(x0 + inset, top, z),
                        p(x1 - inset, top, z),
                        p(x1 - inset, bottom, z),
                        p(x0 + inset, bottom, z),
                    ]
                }),
                (cam.position.z > z1 + eps, {
                    let z = z1 + eps;
                    [
                        p(x1 - inset, top, z),
                        p(x0 + inset, top, z),
                        p(x0 + inset, bottom, z),
                        p(x1 - inset, bottom, z),
                    ]
                }),
                (cam.position.x < x0 - eps, {
                    let x = x0 - eps;
                    [
                        p(x, top, z1 - inset),
                        p(x, top, z0 + inset),
                        p(x, bottom, z0 + inset),
                        p(x, bottom, z1 - inset),
                    ]
                }),
                (cam.position.x > x1 + eps, {
                    let x = x1 + eps;
                    [
                        p(x, top, z0 + inset),
                        p(x, top, z1 - inset),
                        p(x, bottom, z1 - inset),
                        p(x, bottom, z0 + inset),
                    ]
                }),
            ];
            for (outside, corners) in faces {
                if outside && r.textured_quad(corners, signal, &cam) {
                    stats.facades_drawn += 1;
                }
            }
        }
        // Wireframe: the 12 box edges, depth-tested against bodies and facades.
        let (x0, x1, z0, z1) = (b.x - b.hx, b.x + b.hx, b.z - b.hz, b.z + b.hz);
        let c = if b.signal { EDGE_SIGNAL } else { EDGE };
        let p = |x, y, z| Vec3::new(x, y, z);
        for (a, bb) in [
            (p(x0, 0.0, z0), p(x1, 0.0, z0)),
            (p(x1, 0.0, z0), p(x1, 0.0, z1)),
            (p(x1, 0.0, z1), p(x0, 0.0, z1)),
            (p(x0, 0.0, z1), p(x0, 0.0, z0)),
            (p(x0, b.h, z0), p(x1, b.h, z0)),
            (p(x1, b.h, z0), p(x1, b.h, z1)),
            (p(x1, b.h, z1), p(x0, b.h, z1)),
            (p(x0, b.h, z1), p(x0, b.h, z0)),
            (p(x0, 0.0, z0), p(x0, b.h, z0)),
            (p(x1, 0.0, z0), p(x1, b.h, z0)),
            (p(x1, 0.0, z1), p(x1, b.h, z1)),
            (p(x0, 0.0, z1), p(x0, b.h, z1)),
        ] {
            r.line(a, bb, &cam, c);
        }
    }
    stats.z_tests = r.stats.z_tests;
    stats
}

fn hash(r: &RgbRaster) -> u64 {
    r.pixels().iter().fold(0xcbf2_9ce4_8422_2325u64, |h, p| {
        [p.0, p.1, p.2]
            .iter()
            .fold(h, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3))
    })
}

/// Which presentation a terminal of this size and depth gets.
#[derive(Debug, PartialEq, Eq)]
enum Look {
    Image,
    TooSmall,
    Mono,
}

/// Whether the environment asks for no colour: a non-empty `NO_COLOR` (no-color.org,
/// matching the library's own capability detection) or `TERM=dumb`.
fn mono_env() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
        || std::env::var("TERM").is_ok_and(|t| t == "dumb")
}

fn look(cols: u16, rows: u16, depth: ColorDepth, mono_env: bool) -> Look {
    if mono_env || depth == ColorDepth::Mono {
        Look::Mono
    } else if cols < MIN_COLS || rows < MIN_ROWS {
        Look::TooSmall
    } else {
        Look::Image
    }
}

fn text_panel(
    look: &Look,
    cols: u16,
    rows: u16,
    t: f64,
    buildings: &[Building],
    world: &Signal,
) -> Node {
    let signals = buildings.iter().filter(|b| b.signal).count();
    let lines = match look {
        Look::TooSmall => vec![
            "FACADE SIGNAL".to_string(),
            format!("terminal {cols}x{rows} is smaller than {MIN_COLS}x{MIN_ROWS}"),
            "enlarge the window to see the city".to_string(),
            "q / Esc to quit".to_string(),
        ],
        _ => vec![
            "FACADE SIGNAL (text mode: monochrome terminal)".to_string(),
            format!(
                "city: {} buildings, {signals} carry a live raycast facade",
                buildings.len()
            ),
            format!(
                "camera orbit angle: {:.0} deg",
                ((t * 0.18 + 0.6).to_degrees()).rem_euclid(360.0)
            ),
            format!(
                "signal walker heading: {:.0} deg",
                world.walker(t).angle.to_degrees().rem_euclid(360.0)
            ),
            "q / Esc to quit".to_string(),
        ],
    };
    lines
        .into_iter()
        .fold(Node::col(), |n, l| n.child(Node::text(l, Style::default())))
}

struct Args {
    frames: Option<u32>,
    size: (u16, u16),
    seed: u64,
    ppm: Option<String>,
    t: f64,
    reduced: bool,
}

fn parse() -> Result<Args, String> {
    let mut a = Args {
        frames: None,
        size: (100, 32),
        seed: 2112,
        ppm: None,
        t: 0.0,
        reduced: std::env::var_os("REDUCE_MOTION").is_some_and(|v| v != "0"),
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut val = || it.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--frames" => a.frames = Some(val()?.parse().map_err(|e| format!("--frames: {e}"))?),
            "--seed" => a.seed = val()?.parse().map_err(|e| format!("--seed: {e}"))?,
            "--t" => a.t = val()?.parse().map_err(|e| format!("--t: {e}"))?,
            "--ppm" => a.ppm = Some(val()?),
            "--reduced-motion" => a.reduced = true,
            "--size" => {
                let v = val()?;
                let (c, r) = v.split_once('x').ok_or("--size wants COLSxROWS")?;
                a.size = (
                    c.parse().map_err(|e| format!("--size: {e}"))?,
                    r.parse().map_err(|e| format!("--size: {e}"))?,
                );
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(a)
}

/// The scene time actually rendered: the clock when animating, otherwise the fixed
/// settled time 0, so a reduced-motion or static frame is identical for any clock.
fn scene_time(animate: bool, clock: f64) -> f64 {
    if animate {
        clock
    } else {
        0.0
    }
}

/// Raster pixels for a terminal of `cols`×`rows` (half-block: two pixel rows per cell row).
fn raster_size(cols: u16, rows: u16) -> (u16, u16) {
    (cols, rows.saturating_mul(2))
}

fn receipts(a: &Args) -> io::Result<()> {
    let (cols, rows) = a.size;
    let world = Signal::new(a.seed);
    let buildings = city(a.seed);
    let (w, h) = raster_size(cols, rows);
    let mut r = Rasterizer::new(w, h);
    let mut signal = RgbRaster::new(SIGNAL_W, SIGNAL_H);
    let frames = a.frames.unwrap_or(1);
    let start = Instant::now();
    let mut worst = Duration::ZERO;
    let mut total = FrameStats::default();
    let mut last = 0;
    for f in 0..frames {
        let t = scene_time(!a.reduced, a.t + f64::from(f) / f64::from(FPS));
        let fs = Instant::now();
        let s = render_scene(&mut r, &mut signal, &world, &buildings, t, None);
        worst = worst.max(fs.elapsed());
        total.raycast_pixels += s.raycast_pixels;
        total.raycast_steps += s.raycast_steps;
        total.facades_drawn += s.facades_drawn;
        total.z_tests += s.z_tests;
        last = hash(&r.raster);
    }
    let elapsed = start.elapsed();
    println!(
        "facade_signal receipts: size={cols}x{rows} raster={w}x{h} seed={} frames={frames} reduced={} route_cells={}",
        a.seed,
        a.reduced,
        world.route.len()
    );
    println!(
        "  mean_frame_ms={:.3} worst_frame_ms={:.3} last_frame_hash={last:016x}",
        elapsed.as_secs_f64() * 1000.0 / f64::from(frames.max(1)),
        worst.as_secs_f64() * 1000.0
    );
    println!(
        "  per_frame: raycast_pixels={} raycast_steps={} facades={:.2} z_tests={}",
        total.raycast_pixels / u64::from(frames.max(1)),
        total.raycast_steps / u64::from(frames.max(1)),
        f64::from(total.facades_drawn) / f64::from(frames.max(1)),
        total.z_tests / u64::from(frames.max(1)),
    );
    println!(
        "  walker_at_last_frame: {:?}",
        world.walker(if a.reduced {
            0.0
        } else {
            a.t + f64::from(frames.saturating_sub(1)) / f64::from(FPS)
        })
    );
    println!(
        "  bounds: raycast_pixels <= {} raycast_steps <= {}",
        2 * u64::from(SIGNAL_W) * u64::from(SIGNAL_H),
        u64::from(SIGNAL_W) * u64::from(RayView::default().max_steps)
    );
    if let Some(path) = &a.ppm {
        r.raster.write_ppm(std::fs::File::create(path)?)?;
        println!("  wrote {path}");
    }
    Ok(())
}

fn live(a: &Args) -> io::Result<()> {
    let world = Signal::new(a.seed);
    let buildings = city(a.seed);
    let mut ctx = Context::new(RenderMode::Fullscreen)?;
    if !ctx.is_interactive() {
        // Not a terminal (redirected or piped): `poll_event` cannot block here, so a
        // live loop would spin. Use the receipts mode instead.
        drop(ctx);
        eprintln!("facade_signal: not an interactive terminal; use --frames N or --ppm FILE");
        std::process::exit(2);
    }
    ctx.enter_interactive()?;
    ctx.set_max_fps(FPS);
    let mut signal = RgbRaster::new(SIGNAL_W, SIGNAL_H);
    let mut r = Rasterizer::new(1, 1);
    let start = Instant::now();
    let frame = Duration::from_secs(1) / FPS;
    let mut dirty = true;
    let mono = mono_env();
    let mut next_frame = Instant::now();
    let result = (|| -> io::Result<()> {
        loop {
            let (cols, rows) = ctx.session.terminal_size();
            let mode = look(cols, rows, ctx.capabilities().color_depth, mono);
            // Only the image animates; reduced motion and the text panels are static.
            let animate = !a.reduced && mode == Look::Image;
            let t = scene_time(animate, start.elapsed().as_secs_f64());
            // Render when something changed, or when the next animation frame is due;
            // input alone never forces an extra frame.
            if dirty || (animate && Instant::now() >= next_frame) {
                let node = if mode == Look::Image {
                    let (w, h) = raster_size(cols, rows);
                    if (r.raster.width(), r.raster.height()) != (w, h) {
                        r = Rasterizer::new(w, h);
                    }
                    render_scene(&mut r, &mut signal, &world, &buildings, t, None);
                    Node::raster(r.raster.to_surface())
                } else {
                    text_panel(&mode, cols, rows, t, &buildings, &world)
                };
                ctx.set_root(node);
                ctx.render_now()?;
                dirty = false;
                next_frame = Instant::now() + frame;
            }
            // Animating: wake once per frame. Settled (reduced motion or text mode is
            // static): block until input, so an idle terminal costs no CPU.
            let wait = if animate {
                next_frame.saturating_duration_since(Instant::now())
            } else {
                Duration::from_secs(3600)
            };
            match ctx.poll_event(wait)? {
                Some(Event::Key(k))
                    if k.code == KeyCode::Char('q')
                        || k.code == KeyCode::Esc
                        || (k.code == KeyCode::Char('c')
                            && k.modifiers.contains(KeyModifiers::CONTROL)) =>
                {
                    return Ok(());
                }
                Some(Event::Resize(..)) => dirty = true,
                _ => {}
            }
        }
    })();
    let restored = ctx.restore();
    result.and(restored)
}

fn main() {
    let args = match parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("facade_signal: {e}");
            std::process::exit(2);
        }
    };
    let run = if args.frames.is_some() || args.ppm.is_some() {
        receipts(&args)
    } else {
        live(&args)
    };
    if let Err(e) = run {
        eprintln!("facade_signal: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_deterministic_for_seed_and_t() {
        let world = Signal::new(7);
        let city = city(7);
        let render = |t| {
            let mut r = Rasterizer::new(96, 64);
            let mut s = RgbRaster::new(SIGNAL_W, SIGNAL_H);
            render_scene(&mut r, &mut s, &world, &city, t, None);
            hash(&r.raster)
        };
        assert_eq!(render(1.25), render(1.25));
        assert_ne!(render(0.0), render(1.25), "control: frames change with t");
    }

    #[test]
    fn signal_facades_are_visible_and_raycast_work_is_bounded() {
        let world = Signal::new(7);
        let city = city(7);
        let mut r = Rasterizer::new(120, 80);
        let mut s = RgbRaster::new(SIGNAL_W, SIGNAL_H);
        let st = render_scene(&mut r, &mut s, &world, &city, 0.0, None);
        assert!(
            st.facades_drawn >= 1,
            "at least one live facade faces the camera"
        );
        assert!(st.raycast_pixels <= 2 * u64::from(SIGNAL_W) * u64::from(SIGNAL_H));
        assert!(st.raycast_steps <= u64::from(SIGNAL_W) * u64::from(RayView::default().max_steps));
    }

    /// Visible facade area: the pixels that change when the live view is replaced by a
    /// flat sentinel colour. Only facade pixels depend on the signal texture, so this is
    /// the live signal actually visible after occlusion.
    fn visible_facade_pixels(city_seed: u64, t: f64) -> usize {
        let city = city(city_seed);
        let world = Signal::new(city_seed);
        let frame = |sentinel| {
            let mut r = Rasterizer::new(120, 80);
            let mut s = RgbRaster::new(SIGNAL_W, SIGNAL_H);
            render_scene(&mut r, &mut s, &world, &city, t, sentinel);
            r.raster
        };
        let (live, flat) = (frame(None), frame(Some((255, 0, 255))));
        live.pixels()
            .iter()
            .zip(flat.pixels())
            .filter(|(x, y)| x != y)
            .count()
    }

    #[test]
    fn the_walker_moves_through_open_maze_cells() {
        let world = Signal::new(7);
        assert!(world.route.len() > 20, "a long patrol, not a dead end");
        let mut cells = std::collections::BTreeSet::new();
        for step in 0..200 {
            let v = world.walker(f64::from(step) * 0.37);
            assert_eq!(
                world.map.get(v.x.floor() as i64, v.y.floor() as i64),
                0,
                "{v:?}"
            );
            cells.insert((v.x.floor() as i64, v.y.floor() as i64));
        }
        assert!(
            cells.len() > 20,
            "the walker actually travels: {} cells",
            cells.len()
        );
    }

    #[test]
    fn the_live_signal_is_a_substantial_visible_area_through_the_orbit() {
        // At least 2% of the frame (192 of 9600 pixels) at every sampled point of the
        // orbit, for several cities; a sliver seen edge-on or hidden by a neighbour fails.
        for city_seed in [7u64, 2112, 99] {
            for step in 0..12 {
                let t = f64::from(step) * 3.0;
                let px = visible_facade_pixels(city_seed, t);
                assert!(
                    px >= 192,
                    "city {city_seed}, t={t}: only {px} live facade pixels"
                );
            }
        }
    }

    #[test]
    fn reduced_motion_frame_is_identical_for_any_clock() {
        let world = Signal::new(7);
        let city = city(7);
        let render = |clock| {
            let mut r = Rasterizer::new(80, 48);
            let mut s = RgbRaster::new(SIGNAL_W, SIGNAL_H);
            render_scene(
                &mut r,
                &mut s,
                &world,
                &city,
                scene_time(false, clock),
                None,
            );
            hash(&r.raster)
        };
        let settled = render(0.0);
        for clock in [0.5, 3.0, 1234.5] {
            assert_eq!(render(clock), settled, "clock {clock}");
        }
        assert_ne!(scene_time(true, 3.0), scene_time(false, 3.0), "control");
    }

    #[test]
    fn fallbacks_follow_size_and_depth() {
        assert_eq!(look(59, 30, ColorDepth::TrueColor, false), Look::TooSmall);
        assert_eq!(look(80, 19, ColorDepth::Ansi256, false), Look::TooSmall);
        assert_eq!(look(80, 24, ColorDepth::Mono, false), Look::Mono);
        assert_eq!(look(80, 24, ColorDepth::Ansi16, false), Look::Image);
        assert_eq!(
            look(120, 40, ColorDepth::TrueColor, true),
            Look::Mono,
            "NO_COLOR / TERM=dumb"
        );
    }
}
