//! avatar_frame_bench — frame and idle cost of a persistent animated avatar (ASCENSION D2).
//!
//! A synthetic avatar (a face over a gradient, with eye, gaze and mouth features) sits in a
//! left pane beside a chat or Blades pane. It is drawn by three arms that must produce
//! **byte-identical terminal output** (`--selfcheck` proves it), so their costs compare
//! like for like:
//!
//! | arm   | how the avatar becomes cells each frame |
//! |-------|------------------------------------------|
//! | rgb   | clone the base `RgbRaster`, paint the feature pixels directly, `to_surface` |
//! | rgba  | clone the base, `composite_over` full-size RGBA feature layers (cached per state), `to_surface` |
//! | cells | no raster work: a cached base `Surface` plus cached eye/mouth patch surfaces in a `Node::stack` |
//!
//! Scenarios, `--frames` frames each, 33 ms apart in presentation time:
//!
//! | scenario  | what changes |
//! |-----------|--------------|
//! | static    | nothing (the full pipeline still runs every frame) |
//! | blink     | eyes close for 3 of every 60 frames; gaze shifts every 45 |
//! | talk      | mouth shape changes every frame and one reply token streams into the chat per frame |
//! | blades    | a Blades destination switch animates in the right pane while the avatar blinks |
//! | pressure  | `talk` while one extra thread spins on a core |
//!
//! Stages per frame: `avatar` (compose + lower), `build` (node tree), `render`
//! (`Context::render`: layout, paint, diff, ANSI; headless) and `write` (into an OS pipe that
//! a thread drains: **a pipe, not a terminal**). `cpu/f` is process CPU (`getrusage`,
//! user+sys) divided by frames, so it includes the drain thread and, under `pressure`, the
//! spinner. The `idle` modes need a real TTY and are run separately:
//!
//! ```text
//! cargo run --release --example avatar_frame_bench -- [--frames N] [--grids 80x24,120x40,40x15]
//! cargo run --example avatar_frame_bench -- --selfcheck
//! script -qec "target/release/examples/avatar_frame_bench --idle app --for-ms 5000" /dev/null
//! script -qec "target/release/examples/avatar_frame_bench --idle run-once --for-ms 5000" /dev/null
//! ```
//!
//! Numbers are for the host and SHA they were measured on, never a universal claim.
//! Debug-build timings are not meaningful.

use gibson::capability::ColorDepth;
use gibson::cell::{Color, Style};
use gibson::context::{Context, RenderMode};
use gibson::node::{Node, WrapMode};
use gibson::raster::{composite_over, Rgb, RgbRaster, RgbaRaster};
use gibson::surface::Surface;
use gibson::ui::experience::{
    Blades, Content, Destination, Experience, Grammar, PresentationState,
};
use gibson::ui::skin::UiEnvironment;
use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Counts allocations made by this bench binary (the library is unchanged).
struct Counting;
static ALLOCS: AtomicU64 = AtomicU64::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: forwards the caller's layout to the system allocator unchanged.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `System.alloc` with this `layout`.
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static GLOBAL: Counting = Counting;

const FRAME: Duration = Duration::from_millis(33);

/// Process CPU time (user + system) in microseconds.
fn cpu_us() -> u64 {
    // SAFETY: `ru` is a valid out-pointer for getrusage(2).
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    // SAFETY: RUSAGE_SELF with a valid pointer.
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
    let tv = |t: libc::timeval| t.tv_sec as u64 * 1_000_000 + t.tv_usec as u64;
    tv(ru.ru_utime) + tv(ru.ru_stime)
}

fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ---------------------------------------------------------------------------------------
// The avatar
// ---------------------------------------------------------------------------------------

/// Expression state: eyes open?, gaze in {-1, 0, 1}, mouth viseme 0..5.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct Expr {
    open: bool,
    gaze: i8,
    viseme: u8,
}

/// A feature band: pixel rows `[y0, y0 + h)`, both even, so it maps to whole cell rows.
#[derive(Clone, Copy)]
struct Band {
    y0: i32,
    h: i32,
}

struct Avatar {
    cols: u16,
    rows: u16,
    base: RgbRaster,
    eyes: Band,
    mouth: Band,
    cx: i32,
    r: i32,
}

fn even(v: i32) -> i32 {
    v & !1
}

impl Avatar {
    fn new(cols: u16, rows: u16) -> Self {
        let (w, h) = (cols, rows * 2);
        let mut base = RgbRaster::new(w, h);
        for y in 0..i32::from(h) {
            for x in 0..i32::from(w) {
                base.set(x, y, (12 + (x * 3 % 40) as u8, 18, 40 + (y % 90) as u8));
            }
        }
        let cx = i32::from(w) / 2;
        let r = (i32::from(w).min(i32::from(h)) * 38 / 100).max(4);
        let cy = i32::from(h) * 45 / 100;
        base.disc(cx as f32, cy as f32, r as f32, (200, 170, 150));
        let eyes = Band {
            y0: even(cy - r / 3 - 2),
            h: 6,
        };
        // The bands must not overlap: the `cells` arm swaps them independently.
        let mouth = Band {
            y0: even(cy + r / 3).max(eyes.y0 + eyes.h),
            h: 6,
        };
        assert!(
            mouth.y0 + mouth.h <= i32::from(h),
            "avatar too small for its bands"
        );
        Avatar {
            cols,
            rows,
            base,
            eyes,
            mouth,
            cx,
            r,
        }
    }

    /// Every feature pixel for `e`, painted in this order on top of the base.
    fn eye_pixels(&self, e: Expr) -> Vec<(i32, i32, Rgb)> {
        let mut v = Vec::new();
        let y = self.eyes.y0 + 3;
        for side in [-1, 1] {
            let ex = self.cx + side * self.r * 35 / 100;
            if e.open {
                for dy in -2..=1 {
                    for dx in -2..=2 {
                        v.push((ex + dx, y + dy, (240, 240, 240)));
                    }
                }
                let px = ex + i32::from(e.gaze);
                for dy in -1..=0 {
                    for dx in 0..=1 {
                        v.push((px + dx - 1, y + dy, (20, 30, 60)));
                    }
                }
            } else {
                for dx in -2..=2 {
                    v.push((ex + dx, y, (60, 40, 40)));
                }
            }
        }
        v
    }

    fn mouth_pixels(&self, e: Expr) -> Vec<(i32, i32, Rgb)> {
        let mut v = Vec::new();
        let half_w = 2 + i32::from(e.viseme % 3);
        let open_h = 1 + i32::from(e.viseme);
        let top = self.mouth.y0 + (self.mouth.h - open_h.min(self.mouth.h)) / 2;
        for dy in 0..open_h.min(self.mouth.h) {
            for dx in -half_w..=half_w {
                v.push((self.cx + dx, top + dy, (120, 30, 40)));
            }
        }
        v
    }

    /// Arm `rgb`: one raster, features painted directly.
    fn compose_rgb(&self, e: Expr) -> Surface {
        let mut r = self.base.clone();
        for (x, y, c) in self.eye_pixels(e).into_iter().chain(self.mouth_pixels(e)) {
            r.set(x, y, c);
        }
        r.to_surface()
    }

    fn layer(&self, px: Vec<(i32, i32, Rgb)>) -> RgbaRaster {
        let mut l = RgbaRaster::new(self.base.width(), self.base.height());
        for (x, y, (r, g, b)) in px {
            l.set(x, y, [r, g, b, 255]);
        }
        l
    }

    /// The patch for one band: base + all features cropped to the band, lowered to cells.
    fn patch(&self, band: Band, e: Expr) -> Arc<Surface> {
        let w = self.base.width();
        let mut full = self.base.clone();
        for (x, y, c) in self.eye_pixels(e).into_iter().chain(self.mouth_pixels(e)) {
            full.set(x, y, c);
        }
        let mut crop = RgbRaster::new(w, band.h as u16);
        for y in 0..band.h {
            for x in 0..i32::from(w) {
                if let Some(c) = full.get(x, band.y0 + y) {
                    crop.set(x, y, c);
                }
            }
        }
        Arc::new(crop.to_surface())
    }
}

/// Per-arm caches. Building them is setup, not a per-frame cost.
enum Arm {
    Rgb,
    Rgba {
        eyes: std::collections::HashMap<(bool, i8), RgbaRaster>,
        mouth: Vec<RgbaRaster>,
    },
    Cells {
        base: Arc<Surface>,
        eyes: std::collections::HashMap<(bool, i8), Arc<Surface>>,
        mouth: Vec<Arc<Surface>>,
    },
}

const ARMS: [&str; 3] = ["rgb", "rgba", "cells"];

fn eye_states() -> Vec<(bool, i8)> {
    let mut v = vec![(false, 0)];
    for g in -1..=1 {
        v.push((true, g));
    }
    v
}

impl Arm {
    fn build(name: &str, a: &Avatar) -> Arm {
        let neutral = |open, gaze, viseme| Expr { open, gaze, viseme };
        match name {
            "rgb" => Arm::Rgb,
            "rgba" => Arm::Rgba {
                eyes: eye_states()
                    .into_iter()
                    .map(|(o, g)| ((o, g), a.layer(a.eye_pixels(neutral(o, g, 0)))))
                    .collect(),
                mouth: (0..6)
                    .map(|v| a.layer(a.mouth_pixels(neutral(true, 0, v))))
                    .collect(),
            },
            "cells" => Arm::Cells {
                base: Arc::new(a.base.to_surface()),
                // Eye and mouth bands never overlap, so each patch only needs its own
                // feature; the other feature's pixels fall outside the crop.
                eyes: eye_states()
                    .into_iter()
                    .map(|(o, g)| ((o, g), a.patch(a.eyes, neutral(o, g, 0))))
                    .collect(),
                mouth: (0..6)
                    .map(|v| a.patch(a.mouth, neutral(true, 0, v)))
                    .collect(),
            },
            other => panic!("unknown arm {other}"),
        }
    }

    /// The avatar node for `e`: the per-frame compose and lower work of this arm.
    fn node(&self, a: &Avatar, e: Expr) -> Node {
        match self {
            Arm::Rgb => Node::raster(a.compose_rgb(e)),
            Arm::Rgba { eyes, mouth } => {
                let mut r = a.base.clone();
                composite_over(&mut r, &eyes[&(e.open, e.gaze)], 0, 0);
                composite_over(&mut r, &mouth[usize::from(e.viseme)], 0, 0);
                Node::raster(r.to_surface())
            }
            Arm::Cells { base, eyes, mouth } => Node::stack()
                .width(f32::from(a.cols))
                .height(f32::from(a.rows))
                .child(Node::surface(Arc::clone(base)))
                .child(
                    Node::surface(Arc::clone(&eyes[&(e.open, e.gaze)]))
                        .offset(0.0, (a.eyes.y0 / 2) as f32),
                )
                .child(
                    Node::surface(Arc::clone(&mouth[usize::from(e.viseme)]))
                        .offset(0.0, (a.mouth.y0 / 2) as f32),
                ),
        }
    }
}

// ---------------------------------------------------------------------------------------
// The right pane
// ---------------------------------------------------------------------------------------

const WORDS: [&str; 12] = [
    "the", "signal", "arrives", "before", "the", "frame,", "and", "nothing", "waits", "longer",
    "than", "needed.",
];

fn chat(reply_tokens: usize) -> Node {
    let mut col = Node::col().flex_grow(1.0);
    for i in 0..6 {
        col = col.child(Node::text_wrapped(
            format!("[{i}] earlier turn {i}: a settled message that does not change."),
            Style::default(),
            WrapMode::WordWrap,
        ));
    }
    let reply: Vec<&str> = (0..reply_tokens).map(|i| WORDS[i % WORDS.len()]).collect();
    col.child(Node::text_wrapped(
        reply.join(" "),
        Style::default().fg(Color::Rgb(200, 220, 255)),
        WrapMode::WordWrap,
    ))
}

#[derive(Clone, Debug, PartialEq)]
struct Msg;

fn experience() -> Experience<Msg> {
    let prose = |s: &str| Content::Prose(vec![s.to_string()]);
    Experience::new("BENCH")
        .destination(Destination::new("chat", "Chat", prose("conversation")))
        .destination(Destination::new("tasks", "Tasks", prose("tasks")))
        .destination(Destination::new("memory", "Memory", prose("memory")))
}

struct BladesPane {
    grammar: Blades,
    exp: Experience<Msg>,
    state: PresentationState,
    env: UiEnvironment,
}

impl BladesPane {
    fn new(width: u16, height: u16) -> Self {
        let exp = experience();
        let mut state = PresentationState::new(&exp);
        let env = UiEnvironment {
            width,
            height,
            color_depth: ColorDepth::TrueColor,
            ..UiEnvironment::default()
        };
        let mut grammar = Blades::new();
        // Settle on the first destination, then switch: frames animate the transition.
        let _ = grammar.present(&exp, &state, &env, Duration::ZERO);
        let _ = grammar.present(&exp, &state, &env, Duration::from_secs(5));
        let key = gibson::ui::element::Key::named("tasks");
        assert!(state.activate(&exp, &key), "activate tasks");
        BladesPane {
            grammar,
            exp,
            state,
            env,
        }
    }

    fn node(&mut self, t: Duration) -> Node {
        let presented = self.grammar.present(&self.exp, &self.state, &self.env, t);
        let cx = gibson::ui::BuildCx::new(gibson::ui::skins::BLACK_ICE, self.env);
        gibson::ui::compile(&presented.element, &cx)
            .expect("blades lowers")
            .node
            .width(f32::from(self.env.width))
            .height(f32::from(self.env.height))
    }
}

// ---------------------------------------------------------------------------------------
// Runs
// ---------------------------------------------------------------------------------------

const SCENARIOS: [&str; 5] = ["static", "blink", "talk", "blades", "pressure"];

fn expr_at(scenario: &str, n: u32) -> Expr {
    match scenario {
        "blink" | "blades" => Expr {
            open: n % 60 >= 3,
            gaze: [0, 1, 0, -1][(n / 45 % 4) as usize],
            viseme: 0,
        },
        "talk" | "pressure" => Expr {
            open: true,
            gaze: 0,
            viseme: [0, 3, 1, 5, 2, 4][(n % 6) as usize],
        },
        _ => Expr {
            open: true,
            gaze: 0,
            viseme: 0,
        },
    }
}

#[derive(Default)]
struct Samples {
    avatar: Vec<u64>,
    build: Vec<u64>,
    render: Vec<u64>,
    write: Vec<u64>,
    total: Vec<u64>,
    bytes: Vec<u64>,
    allocs: Vec<u64>,
    cpu: u64,
    digest: u64,
    /// fnv of every frame's output, for the cross-arm identity check.
    frames: Vec<u64>,
}

fn pct(v: &[u64], p: f64) -> u64 {
    if v.is_empty() {
        return 0;
    }
    let mut s = v.to_vec();
    s.sort_unstable();
    s[((s.len() - 1) as f64 * p).round() as usize]
}

fn mean(v: &[u64]) -> u64 {
    if v.is_empty() {
        0
    } else {
        v.iter().sum::<u64>() / v.len() as u64
    }
}

fn run(
    cols: u16,
    rows: u16,
    arm_name: &str,
    scenario: &str,
    frames: u32,
    pipe: &mut std::fs::File,
) -> Samples {
    // Grids are at least 30x8 (`parse_grids`), so this leaves the right pane >= 18 columns.
    let av_cols = (cols * 3 / 10).max(12);
    let av_rows = rows - 2;
    let avatar = Avatar::new(av_cols, av_rows);
    let arm = Arm::build(arm_name, &avatar);
    let mut blades = (scenario == "blades").then(|| BladesPane::new(cols - av_cols, rows));
    let mut ctx = Context::headless(RenderMode::Fullscreen, cols, rows);
    ctx.set_color_depth(ColorDepth::TrueColor);

    let stop = Arc::new(AtomicBool::new(false));
    let spinner = (scenario == "pressure").then(|| {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut x = 0u64;
            while !stop.load(Ordering::Relaxed) {
                x = std::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1));
            }
        })
    });

    let mut s = Samples::default();
    let mut digest = 0xcbf2_9ce4_8422_2325u64;
    let cpu0 = cpu_us();
    for n in 0..frames {
        let a0 = ALLOCS.load(Ordering::Relaxed);
        let t0 = Instant::now();
        let av = arm.node(&avatar, expr_at(scenario, n));
        let t1 = Instant::now();
        let right = match blades.as_mut() {
            Some(b) => b.node(FRAME * n),
            None => {
                let tokens = if matches!(scenario, "talk" | "pressure") {
                    n as usize + 1
                } else {
                    24
                };
                chat(tokens)
            }
        };
        let root = Node::row()
            .width(f32::from(cols))
            .height(f32::from(rows))
            .child(av)
            .child(right);
        let t2 = Instant::now();
        ctx.set_root(root);
        ctx.render().expect("headless render");
        let out = ctx.take_output();
        let t3 = Instant::now();
        pipe.write_all(out.as_bytes()).expect("pipe write");
        pipe.flush().expect("pipe flush");
        let t4 = Instant::now();
        let us = |a: Instant, b: Instant| (b - a).as_micros() as u64;
        s.avatar.push(us(t0, t1));
        s.build.push(us(t1, t2));
        s.render.push(us(t2, t3));
        s.write.push(us(t3, t4));
        s.total.push(us(t0, t4));
        s.bytes.push(out.len() as u64);
        s.allocs.push(ALLOCS.load(Ordering::Relaxed) - a0);
        let f = fnv(out.as_bytes());
        s.frames.push(f);
        digest = (digest ^ f).wrapping_mul(0x0100_0000_01b3);
    }
    s.cpu = (cpu_us() - cpu0) / u64::from(frames.max(1));
    stop.store(true, Ordering::Relaxed);
    if let Some(h) = spinner {
        let _ = h.join();
    }
    s.digest = digest;
    s
}

/// An OS pipe whose read end is drained by a thread.
fn drained_pipe() -> (std::fs::File, std::thread::JoinHandle<u64>) {
    let mut fds = [0; 2];
    // SAFETY: `fds` is a valid 2-element array for pipe(2).
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0, "pipe(2)");
    // SAFETY: both fds were just created by pipe(2) and are owned exactly once here.
    let (r, w) = unsafe {
        (
            std::fs::File::from_raw_fd(fds[0]),
            std::fs::File::from_raw_fd(fds[1]),
        )
    };
    let drain = std::thread::spawn(move || {
        let mut r = r;
        let mut buf = vec![0u8; 1 << 16];
        let mut total = 0u64;
        while let Ok(n) = r.read(&mut buf) {
            if n == 0 {
                break;
            }
            total += n as u64;
        }
        total
    });
    (w, drain)
}

fn selfcheck() -> Result<(), String> {
    for &(cols, rows) in &[(80u16, 24u16), (40, 15)] {
        for scenario in ["static", "blink", "talk", "blades"] {
            let mut per_arm = Vec::new();
            for arm in ARMS {
                let (mut p, d) = drained_pipe();
                let s = run(cols, rows, arm, scenario, 70, &mut p);
                drop(p);
                let drained = d.join().unwrap_or(0);
                if drained != s.bytes.iter().sum::<u64>() {
                    return Err(format!("{cols}x{rows} {arm} {scenario}: pipe lost bytes"));
                }
                per_arm.push((arm, s.frames));
            }
            let (ref_arm, ref_frames) = &per_arm[0];
            for (arm, frames) in &per_arm[1..] {
                if let Some(i) = frames.iter().zip(ref_frames).position(|(a, b)| a != b) {
                    return Err(format!(
                        "{cols}x{rows} {scenario}: arm {arm} differs from {ref_arm} at frame {i}"
                    ));
                }
            }
            // Something must actually change, or the scenario measures nothing.
            if scenario != "static" && ref_frames.windows(2).all(|w| w[0] == w[1]) {
                return Err(format!("{cols}x{rows} {scenario}: no frame differs"));
            }
        }
    }
    // Determinism across runs.
    let (mut p1, d1) = drained_pipe();
    let a = run(80, 24, "rgba", "talk", 30, &mut p1);
    drop(p1);
    let (mut p2, d2) = drained_pipe();
    let b = run(80, 24, "rgba", "talk", 30, &mut p2);
    drop(p2);
    let _ = (d1.join(), d2.join());
    if a.digest != b.digest {
        return Err(format!(
            "non-deterministic: {:x} vs {:x}",
            a.digest, b.digest
        ));
    }
    println!(
        "avatar_frame_bench selfcheck: OK arms={} identical; digest={:016x}",
        ARMS.len(),
        a.digest
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Idle (needs a TTY)
// ---------------------------------------------------------------------------------------

fn idle(mode: &str, for_ms: u64) -> std::io::Result<()> {
    let avatar = Avatar::new(24, 20);
    let face = avatar.base.clone();
    let deadline = Duration::from_millis(for_ms);
    match mode {
        "app" => {
            use gibson::ui::{App, AppEvent, Control};
            let mut ticks = 0u64;
            let mut cpu_start = None;
            let mut t_start = None;
            App::inline().fps(30).run(
                (),
                |_, ev: AppEvent<()>| {
                    if let AppEvent::Tick(_) = ev {
                        ticks += 1;
                        let now = Instant::now();
                        // Measure from the first tick, after the first frame is on screen.
                        let t = *t_start.get_or_insert(now);
                        cpu_start.get_or_insert_with(cpu_us);
                        if now - t >= deadline {
                            return Control::Quit;
                        }
                    }
                    Control::Continue
                },
                |_, _| {
                    gibson::ui::column()
                        .child(gibson::ui::text("idle avatar"))
                        .child(gibson::ui::image(&face, (24, 10), "avatar"))
                },
            )?;
            let cpu = cpu_us() - cpu_start.unwrap_or_else(cpu_us);
            let wall = t_start.map(|t| t.elapsed()).unwrap_or_default();
            eprintln!(
                "RESULT idle=app for_ms={for_ms} wall_ms={} loop_passes={ticks} cpu_us={cpu} cpu_pct={:.2}",
                wall.as_millis(),
                cpu as f64 / wall.as_micros().max(1) as f64 * 100.0
            );
        }
        "run-once" => {
            let mut ctx = Context::new(RenderMode::Inline)?;
            ctx.set_root(
                Node::col()
                    .child(Node::text("idle avatar", Style::default()))
                    .child(Node::raster(face.to_surface())),
            );
            ctx.render_now()?;
            let (t0, c0) = (Instant::now(), cpu_us());
            let mut returns = 0u64;
            while t0.elapsed() < deadline {
                let _ = ctx.run_once(Duration::from_millis(1000))?;
                returns += 1;
            }
            let (wall, cpu) = (t0.elapsed(), cpu_us() - c0);
            ctx.restore()?;
            eprintln!(
                "RESULT idle=run-once for_ms={for_ms} wall_ms={} returns={returns} cpu_us={cpu} cpu_pct={:.2}",
                wall.as_millis(),
                cpu as f64 / wall.as_micros().max(1) as f64 * 100.0
            );
        }
        other => {
            eprintln!("unknown idle mode {other} (want app | run-once)");
            std::process::exit(2);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------

fn parse_grids(s: &str) -> Result<Vec<(u16, u16)>, String> {
    s.split(',')
        .map(|g| {
            let bad = || format!("bad grid {g:?} (want COLSxROWS, at least 30x8)");
            let (c, r) = g.split_once('x').ok_or_else(bad)?;
            let (c, r): (u16, u16) = (
                c.trim().parse().map_err(|_| bad())?,
                r.trim().parse().map_err(|_| bad())?,
            );
            if c < 30 || r < 8 {
                return Err(bad());
            }
            Ok((c, r))
        })
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--selfcheck") {
        if let Err(e) = selfcheck() {
            eprintln!("avatar_frame_bench selfcheck: FAIL {e}");
            std::process::exit(1);
        }
        return;
    }
    let mut frames = 300u32;
    let mut grids = vec![(80u16, 24u16), (120, 40), (40, 15)];
    let mut idle_mode: Option<String> = None;
    let mut for_ms = 5000u64;
    let mut it = args.iter();
    let need = |v: Option<&String>, flag: &str| -> String {
        v.cloned().unwrap_or_else(|| {
            eprintln!("{flag} needs a value");
            std::process::exit(2)
        })
    };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--frames" => frames = need(it.next(), "--frames").parse().unwrap_or(frames),
            "--grids" => match parse_grids(&need(it.next(), "--grids")) {
                Ok(g) => grids = g,
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(2);
                }
            },
            "--idle" => idle_mode = Some(need(it.next(), "--idle")),
            "--for-ms" => for_ms = need(it.next(), "--for-ms").parse().unwrap_or(for_ms),
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    if let Some(mode) = idle_mode {
        if let Err(e) = idle(&mode, for_ms) {
            eprintln!("idle {mode}: {e}");
            std::process::exit(1);
        }
        return;
    }
    let profile = if cfg!(debug_assertions) {
        "debug (timings not meaningful)"
    } else {
        "release"
    };
    println!(
        "# avatar_frame_bench frames={frames} profile={profile} write=pipe-not-terminal times=us"
    );
    println!("grid\tarm\tscenario\tavatar_p50\tavatar_p95\tbuild_p50\trender_p50\trender_p95\twrite_p50\ttotal_p50\ttotal_p95\tcpu_per_frame\tbytes_mean\tallocs_mean\tdigest");
    let (mut pipe, drain) = drained_pipe();
    for &(cols, rows) in &grids {
        for scenario in SCENARIOS {
            for arm in ARMS {
                let s = run(cols, rows, arm, scenario, frames, &mut pipe);
                println!(
                    "{cols}x{rows}\t{arm}\t{scenario}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:016x}",
                    pct(&s.avatar, 0.5),
                    pct(&s.avatar, 0.95),
                    pct(&s.build, 0.5),
                    pct(&s.render, 0.5),
                    pct(&s.render, 0.95),
                    pct(&s.write, 0.5),
                    pct(&s.total, 0.5),
                    pct(&s.total, 0.95),
                    s.cpu,
                    mean(&s.bytes),
                    mean(&s.allocs),
                    s.digest
                );
            }
        }
    }
    drop(pipe);
    let drained = drain.join().unwrap_or(0);
    println!("# pipe drained {drained} bytes");
}
