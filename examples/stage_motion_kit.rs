//! M5-A visual checkpoint kit for the experimental `gibson::stage::motion` (G1-G4).
//!
//! One fixed, synthetic frame per run (no external assets), presented through
//! `PresentationDriver::resolve_frame` into a real alt-screen `Context` at a fixed 80x30
//! viewport, then held on screen for `--hold-ms` and restored. A stderr JSON line reports
//! the sampled state. See docs/evidence/m5a/kit/README.md.
//!
//! `cargo run --example stage_motion_kit -- --scenario=g1 --at-ms=300 --hold-ms=2000`
use gibson::raster::{Rgb, RgbRaster, RgbaRaster};
use gibson::stage::motion::*;
use gibson::stage::*;
use gibson::ui::experience::FrameDemand;
use gibson::ui::prelude::*;
use gibson::{Context, RenderMode};
use std::sync::Arc;
use std::time::Duration;

const COLS: u16 = 80;
const ROWS: u16 = 30;
const SEED: u64 = 0x5eed;
const BLINK_MS: u64 = 150;
type Px = [u8; 4];

#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn opt(d: Option<Duration>) -> String {
    d.map_or("null".to_string(), |d| d.as_millis().to_string())
}
/// Like `opt`, but with the unit attached only when there is a value (`300ms` or `none`).
fn opt_ms(d: Option<Duration>) -> String {
    d.map_or("none".to_string(), |d| format!("{}ms", d.as_millis()))
}
/// Fill a block in logical units (1 logical unit = 2x2 raster px).
fn blk(r: &mut RgbaRaster, x: i32, y: i32, w: i32, h: i32, c: Px) {
    px(r, 2 * x, 2 * y, 2 * w, 2 * h, c);
}
fn px(r: &mut RgbaRaster, x: i32, y: i32, w: i32, h: i32, c: Px) {
    for yy in y..y + h {
        for xx in x..x + w {
            r.set(xx, yy, c);
        }
    }
}
fn solid(w: u16, h: u16, c: Px) -> Arc<RgbaRaster> {
    let mut r = RgbaRaster::new(w, h);
    px(&mut r, 0, 0, i32::from(w), i32::from(h), c);
    Arc::new(r)
}
/// 16x24 logical (32x48 px) blocky figure. `arms` 0 = down, 1 = left up, 2 = both up,
/// 3 = right up. `band` is a 1-row colour strip along the top that names the frame.
fn sprite(arms: u8, band: Px, face: bool) -> Arc<RgbaRaster> {
    let mut r = RgbaRaster::new(32, 48);
    let (skin, shirt, legs) = ([230, 190, 150, 255], [60, 90, 200, 255], [40, 40, 60, 255]);
    blk(&mut r, 0, 0, 16, 1, band);
    blk(&mut r, 5, 18, 2, 6, legs);
    blk(&mut r, 9, 18, 2, 6, legs);
    blk(&mut r, 4, 10, 8, 8, shirt);
    blk(&mut r, 4, 2, 8, 8, skin);
    blk(&mut r, 4, 2, 8, 2, [90, 50, 30, 255]);
    let (left_up, right_up) = (matches!(arms, 1 | 2), matches!(arms, 2 | 3));
    blk(&mut r, 2, if left_up { 4 } else { 10 }, 2, 7, skin);
    blk(&mut r, 12, if right_up { 4 } else { 10 }, 2, 7, skin);
    if face {
        blk(&mut r, 6, 6, 1, 1, [0, 0, 0, 255]);
        blk(&mut r, 9, 6, 1, 1, [0, 0, 0, 255]);
        blk(&mut r, 7, 8, 2, 1, [150, 40, 40, 255]);
    }
    Arc::new(r)
}
fn sequence() -> FrameSequence {
    let band = [
        [230, 230, 230, 255],
        [230, 60, 60, 255],
        [60, 200, 60, 255],
        [230, 200, 40, 255],
    ];
    let holds = [300, 120, 500, 200]; // starts 0,300,420,920; total 1120
    let frames = (0..4)
        .map(|i| SequenceFrame::new(sprite(i as u8, band[i], true), ms(holds[i])))
        .collect();
    FrameSequence::new(frames, 0).expect("valid sequence")
}
fn slots() -> LayerSlots {
    // eyes (y 10..16) and mouth (y 14..18) overlap on y 14..16: mouth z is higher, so it wins.
    let r = |x, y, width, height| PixelRect {
        x,
        y,
        width,
        height,
    };
    LayerSlots::new(
        (32, 48),
        vec![
            LayerSlot::new("eyes", r(10, 10, 12, 6)),
            LayerSlot::new("mouth", r(12, 14, 8, 4)),
        ],
    )
    .expect("valid layout")
}
fn cues() -> CueSchedule {
    CueSchedule::new(SEED, 1, ms(300), ms(1200), ms(500)).expect("valid cue")
}
fn eyes(open: bool) -> Arc<RgbaRaster> {
    let mut r = RgbaRaster::new(12, 6);
    if open {
        px(&mut r, 0, 0, 12, 6, [255, 255, 255, 255]);
        px(&mut r, 2, 1, 2, 4, [0, 0, 0, 255]);
        px(&mut r, 8, 1, 2, 4, [0, 0, 0, 255]);
    } else {
        px(&mut r, 0, 3, 12, 1, [20, 20, 20, 255]);
    }
    Arc::new(r)
}
fn tree(lines: &[String]) -> gibson::ui::Element<Msg> {
    let mut root = column()
        .key("root")
        .width(COLS)
        .height(ROWS)
        .padding(0)
        .gap(0);
    root = root.child(text(lines[0].clone()).height(1));
    root = root.child(column().key("art").width(32).height(24).padding(0).gap(0));
    for l in &lines[1..] {
        root = root.child(text(l.clone()).height(1));
    }
    root
}
fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_owned))
}
fn num(args: &[String], name: &str, default: u64) -> u64 {
    flag(args, name).map_or(default, |v| {
        v.parse().unwrap_or_else(|_| die(&format!("bad --{name}")))
    })
}
fn die(msg: &str) -> ! {
    eprintln!("stage_motion_kit: {msg}");
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let scenario = flag(&args, "scenario").unwrap_or_else(|| "g1".into());
    let (at, hold) = (num(&args, "at-ms", 0), num(&args, "hold-ms", 1500));
    let known = [
        "g1",
        "g1-reduced",
        "g2-slots",
        "g2-unfilled",
        "g3-timeline",
        "g4-hidden",
    ];
    if !known.contains(&scenario.as_str()) {
        die(&format!("unknown scenario {scenario:?}; one of {known:?}"));
    }
    let motion = if scenario == "g1-reduced" {
        MotionPreference::Reduced
    } else {
        MotionPreference::Full
    };
    let now = ms(at);
    let key = || Key::named("art");

    // Underlay (1 cell = 1x2 px) and, for g3, the time ruler drawn into it.
    let mut underlay = RgbRaster::new(COLS, ROWS * 2);
    underlay.clear((12, 14, 24));
    let cue = cues();
    let times: Vec<u64> = (0..8)
        .map(|k| cue.nth(k).expect("cue").as_millis() as u64)
        .collect();
    let nearest = (0..8usize)
        .min_by_key(|&k| times[k].abs_diff(at))
        .unwrap_or(0);
    let blinking = times.iter().any(|&t| t <= at && at < t + BLINK_MS);

    // Sample the behaviour under test.
    let mut frame_idx: Option<usize> = None;
    let mut next_in: Option<Duration> = None;
    let mut info = String::new();
    let planes: Vec<RasterPlane> = match scenario.as_str() {
        "g1" | "g1-reduced" | "g4-hidden" => {
            let s = sequence().sample(if scenario == "g4-hidden" { ms(0) } else { now }, motion);
            (frame_idx, next_in) = (Some(s.index), s.next_in);
            info = format!("frame idx={} next_in={}", s.index, opt_ms(s.next_in));
            vec![RasterPlane {
                anchor: RasterAnchor::Key(key()),
                source: s.source,
                offset_px: (0, 0),
                z: 0,
            }]
        }
        "g2-unfilled" => slots()
            .planes(key(), sprite(0, [230, 230, 230, 255], false), &[], 0)
            .unwrap(),
        "g2-slots" => {
            let fills = [
                ("eyes", solid(12, 6, [0, 200, 220, 255])),
                ("mouth", solid(8, 4, [220, 40, 200, 255])),
            ];
            slots()
                .planes(key(), sprite(0, [230, 230, 230, 255], false), &fills, 0)
                .unwrap()
        }
        _ => {
            let fills = [
                ("eyes", eyes(!blinking)),
                ("mouth", solid(8, 4, [150, 40, 40, 255])),
            ];
            let (_, nxt) = cue.next_after(now).expect("next cue");
            next_in = Some(nxt - now);
            info = format!(
                "nearest cue k={nearest} t={}ms blinking={blinking}",
                times[nearest]
            );
            let w = i32::from(COLS) - 1;
            for (k, &t) in times.iter().enumerate() {
                let x = (t as i32 * w / 10_000).min(w - 1);
                let c: Rgb = if k == nearest {
                    (255, 160, 0)
                } else {
                    (90, 170, 255)
                };
                for y in 56..60 {
                    underlay.set(x, y, c);
                    underlay.set(x + 1, y, c);
                }
            }
            let cx = (at.min(10_000) as i32 * w / 10_000).min(w);
            for y in 54..60 {
                underlay.set(cx, y, (255, 255, 255));
            }
            slots()
                .planes(key(), sprite(0, [230, 230, 230, 255], false), &fills, 0)
                .unwrap()
        }
    };
    let mut lines = vec![format!("M5-A {scenario} at={at}ms hold={hold}ms {info}")];
    if scenario == "g3-timeline" {
        let list: Vec<String> = times
            .iter()
            .enumerate()
            .map(|(k, t)| format!("k{k}={t}"))
            .collect();
        lines.push(format!("seed={SEED:#x} cues(ms): {}", list[..4].join(" ")));
        lines.push(format!("                  {}", list[4..].join(" ")));
        lines.push("ruler 0..10000ms, yellow=nearest cue, white=--at-ms".into());
    }
    lines.resize(lines.len().max(2), String::new());

    let mut ui: UiRuntime<Msg> = UiRuntime::new(skins::SWISS_SIGNAL);
    let mut compile = |t: Duration, lines: &[String]| {
        let env = UiEnvironment {
            width: COLS,
            height: ROWS,
            motion,
            ..Default::default()
        };
        ui.frame(&tree(lines), env, t).expect("ui frame")
    };
    let mut driver = PresentationDriver::new(PresentationOptions {
        transition: ms(0),
        cadence: ms(16),
    })
    .unwrap();
    let limits = RasterLimits {
        planes: 16,
        nodes: 256,
        pixels: 1_000_000,
    };
    let demand = next_in
        .map(|d| {
            vec![SourceDemand {
                key: key(),
                demand: FrameDemand::After(d),
            }]
        })
        .unwrap_or_default();
    let mut resolve = |driver: &mut PresentationDriver,
                       t: Duration,
                       lines: &[String],
                       p: &[RasterPlane],
                       d: &[SourceDemand]| {
        let parts = FrameParts {
            compiled: compile(t, lines),
            viewport: (COLS, ROWS),
            underlay: &underlay,
            planes: p,
            focus: None,
            demands: d,
            omissions: &[],
            motion,
        };
        driver
            .resolve_frame(t, parts, limits)
            .expect("resolve_frame")
    };
    let shown_t = if scenario == "g4-hidden" { ms(0) } else { now };
    let shown = resolve(&mut driver, shown_t, &lines, &planes, &demand);
    let mut deadline = driver.next_deadline();
    let mut extra = String::new();

    let mut ctx =
        Context::new(RenderMode::Fullscreen).unwrap_or_else(|e| die(&format!("terminal: {e}")));
    ctx.set_root(shown.to_node());
    ctx.render_now().expect("render");
    if scenario == "g4-hidden" {
        // Headless twin counts bytes: the visible frame, then the hidden frame, then an idle tick.
        let mut h = Context::headless(RenderMode::Fullscreen, COLS, ROWS);
        h.set_root(shown.to_node());
        h.render_now().unwrap();
        let visible_bytes = h.take_rendered_bytes().len();
        std::thread::sleep(ms(400));
        let mut l2 = lines.clone();
        l2[0] = format!("M5-A g4-hidden: plane omitted at={at}ms (no sprite, no deadline)");
        let hidden = resolve(&mut driver, now.max(shown_t), &l2, &[], &[]);
        let after_hide = driver.next_deadline();
        h.set_root(hidden.to_node());
        h.render_now().unwrap();
        let hidden_bytes = h.take_rendered_bytes().len();
        let idle_none = h.run_once(ms(1)).map(|e| e.is_none()).unwrap_or(false);
        let idle_bytes = h.take_rendered_bytes().len();
        ctx.set_root(hidden.to_node());
        ctx.render_now().expect("render hidden");
        eprintln!(
            "g4: visible_bytes={visible_bytes} next_deadline_visible={} hidden_bytes={hidden_bytes} \
             next_deadline_hidden={} idle_run_once_none={idle_none} idle_bytes={idle_bytes}",
            opt(deadline),
            opt(after_hide)
        );
        extra = format!(",\"hidden_bytes\":{hidden_bytes},\"idle_bytes\":{idle_bytes}");
        deadline = after_hide;
    }
    eprintln!(
        "{{\"scenario\":\"{scenario}\",\"at_ms\":{at},\"frame\":{},\"next_in\":{},\"next_deadline\":{}{extra}}}",
        frame_idx.map_or("null".into(), |i| i.to_string()),
        opt(next_in),
        opt(deadline)
    );
    if scenario == "g3-timeline" {
        eprintln!("{{\"cues_ms\":{times:?},\"seed\":{SEED},\"nearest\":{nearest},\"blinking\":{blinking}}}");
    }
    std::thread::sleep(ms(hold));
    ctx.restore().ok();
}
