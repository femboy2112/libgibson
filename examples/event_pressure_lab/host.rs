use super::{
    child,
    probe::{self, Config, Report, Trial},
    trace::{Record, MAX_RECORDS, MAX_TRACE_BYTES},
    visual::{self, Entry, Pulse, View},
};
use gibson::{ColorDepth, Context, Event, KeyCode, KeyModifiers, Node};
use std::{
    fs, io,
    path::PathBuf,
    time::{Duration, Instant},
};

fn option(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .find_map(|s| s.strip_prefix(&format!("{name}=")).map(str::to_owned))
        .or_else(|| args.windows(2).find(|p| p[0] == name).map(|p| p[1].clone()))
}
fn number(args: &[String], name: &str, default: u64) -> io::Result<u64> {
    option(args, name).map_or(Ok(default), |s| {
        s.parse()
            .map_err(|_| io::Error::other(format!("invalid {name}")))
    })
}
pub fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help") {
        println!("Event Pressure Lab (Unix)\n  --mode raw|crossterm|context --scenario normal|resize-key|key-resize|coincident|slow-drain|burst|storm\n  --workload silent|graphics --pause-ms 0..500 --deadline-ms 100..3000\n  --auto (repeat live trials)\n  --headless [--trace FILE]   one real trial; exit 2 on delivery failure\n  --matrix --repeat 1..100  CSV observations; failure rows remain failures\n  --replay-trace FILE [--freeze-at MICROSECONDS] [--dump] [--deterministic]\n  --illustrative --dump --width 120 --height 32 --color truecolor|ansi16|mono\nLive: 1..7 scenario, M input mode, [ ] drain pause, R rerun, A repeat, Space freeze view, Esc/Ctrl-C exit. Ordinary lowercase keys go to child.\nReal PTY timings are measured, never deterministic. A frozen trace projection is deterministic.");
        return Ok(());
    }
    if let Some(mode) = option(&args, "--child") {
        let dir =
            option(&args, "--trace-dir").ok_or_else(|| io::Error::other("missing trace dir"))?;
        return child::run(
            &mode,
            &PathBuf::from(dir),
            number(&args, "--epoch", 0)?,
            args.iter().any(|s| s == "--graphics"),
            args.iter().any(|s| s == "--burst"),
        );
    }
    let c = Config {
        mode: option(&args, "--mode").unwrap_or("context".into()),
        scenario: option(&args, "--scenario").unwrap_or("normal".into()),
        graphics: option(&args, "--workload").as_deref() != Some("silent"),
        pause_ms: number(&args, "--pause-ms", 60)?,
        deadline_ms: number(&args, "--deadline-ms", 700)?,
    };
    c.validate()?;
    let exe = std::env::current_exe()?;
    if args.iter().any(|s| s == "--matrix") {
        let n = number(&args, "--repeat", 1)?;
        if !(1..=100).contains(&n) {
            return Err(io::Error::other("repeat must be 1..100"));
        }
        println!("mode,scenario,graphics,pause_ms,trials,delivered,readable_stalls,other_failures,restored,key_samples,p50_us,p95_us,max_us,drained_bytes,frames");
        for mode in probe::MODES {
            for scenario in probe::SCENARIOS {
                let (mut pass, mut stalls, mut other, mut restored, mut drained, mut frames) =
                    (0, 0, 0, 0, 0, 0);
                let mut latencies = vec![];
                for _ in 0..n {
                    let r = probe::run(
                        &exe,
                        Config {
                            mode: mode.into(),
                            scenario: scenario.into(),
                            ..c.clone()
                        },
                    )?;
                    if r.passed() {
                        pass += 1;
                    } else if r.verdict == "READABLE / NOT DELIVERED" {
                        stalls += 1;
                    } else {
                        other += 1;
                    }
                    restored += u64::from(r.restored);
                    drained += r.drained;
                    frames += r.frames;
                    latencies.extend(r.latencies_us);
                }
                latencies.sort_unstable();
                let p = |percent: usize| {
                    if latencies.is_empty() {
                        0
                    } else {
                        latencies[(percent * latencies.len()).div_ceil(100).saturating_sub(1)]
                    }
                };
                println!("{mode},{scenario},{},{},{n},{pass},{stalls},{other},{restored},{},{},{},{},{drained},{frames}",c.graphics,c.pause_ms,latencies.len(),p(50),p(95),p(100));
            }
        }
        return Ok(());
    }
    if args.iter().any(|s| s == "--headless") {
        let r = probe::run(&exe, c)?;
        if let Some(path) = option(&args, "--trace") {
            save(&path, &r)?;
        }
        println!(
            "{} sent={:?} received={:?} restored={} frames={} drained={} latency_us={:?}",
            r.verdict, r.sent, r.received, r.restored, r.frames, r.drained, r.latencies_us
        );
        if !r.passed() {
            for e in r.records.iter().rev().take(24).rev() {
                print!("{}", e.line());
            }
            std::process::exit(2);
        }
        return Ok(());
    }
    let depth = match option(&args, "--color").as_deref() {
        Some("mono") => ColorDepth::Mono,
        Some("ansi16") => ColorDepth::Ansi16,
        Some("ansi256") => ColorDepth::Ansi256,
        _ => ColorDepth::TrueColor,
    };
    let replay = option(&args, "--replay-trace")
        .map(|p| load(&p))
        .transpose()?;
    if args.iter().any(|s| s == "--deterministic")
        && replay.is_none()
        && !args.iter().any(|s| s == "--illustrative")
    {
        return Err(io::Error::other("real PTY timing is measured; deterministic inspection requires a trace or illustrative fixture"));
    }
    if args.iter().any(|s| s == "--dump") {
        let view = if let Some(records) = replay.as_ref() {
            let end = number(&args, "--freeze-at", records.last().map_or(0, |r| r.us))?;
            view(&c, records, 0, end, false)
        } else if args.iter().any(|s| s == "--illustrative") {
            visual::fixture()
        } else {
            return Err(io::Error::other(
                "dump requires measured --replay-trace or explicitly --illustrative",
            ));
        };
        let w = number(&args, "--width", 120)?.clamp(1, 240) as u16;
        let h = number(&args, "--height", 32)?.clamp(1, 80) as u16;
        let surface = visual::frame(&view, w, h, depth);
        if let Some(path) = option(&args, "--cells") {
            // Development inspection through ordinary framebuffer ANSI, not an
            // image protocol or hand-written control stream.
            let mut compiler = gibson::AnsiCompiler::new();
            let bytes = compiler.compile(&gibson::compute_diff(None, &surface));
            fs::write(path, bytes)?;
        }
        for y in 0..h {
            let line: String = (0..w)
                .map(|x| surface.get(x, y).unwrap().glyph.grapheme.as_str())
                .collect();
            println!("{line}");
        }
        return Ok(());
    }
    let freeze = option(&args, "--freeze-at")
        .map(|s| {
            s.parse::<u64>()
                .map_err(|_| io::Error::other("invalid freeze time"))
        })
        .transpose()?;
    live(c, depth, replay, freeze, args.iter().any(|s| s == "--auto"))
}
fn save(path: &str, r: &Report) -> io::Result<()> {
    let text: String = r.records.iter().map(Record::line).collect();
    fs::write(path, text)
}
fn load(path: &str) -> io::Result<Vec<Record>> {
    let f = fs::File::open(path)?;
    if f.metadata()?.len() > MAX_TRACE_BYTES {
        return Err(io::Error::other("trace file too large"));
    }
    use io::Read;
    let mut text = String::new();
    f.take(MAX_TRACE_BYTES + 1).read_to_string(&mut text)?;
    let records: Vec<_> = text
        .lines()
        .map(|s| Record::parse(s).ok_or_else(|| io::Error::other("invalid trace line")))
        .collect::<io::Result<_>>()?;
    if records.len() > MAX_RECORDS
        || records
            .windows(2)
            .any(|p| p[0].us > p[1].us || p[0].seq >= p[1].seq)
    {
        return Err(io::Error::other("unordered/oversized trace"));
    }
    Ok(records)
}
pub fn view(c: &Config, records: &[Record], drained: u64, at: u64, running: bool) -> View {
    let rows: Vec<_> = records.iter().filter(|r| r.us <= at).cloned().collect();
    let config = rows
        .iter()
        .find(|r| r.kind == "Config")
        .map(|r| r.value.split(',').collect::<Vec<_>>());
    let drained = rows
        .iter()
        .rev()
        .find(|r| r.kind == "Drained")
        .and_then(|r| r.value.parse().ok())
        .unwrap_or(drained);
    let result = probe::analyze(&rows, drained, false);
    let writes: Vec<_> = rows
        .iter()
        .filter(|r| r.source == "PTY" && r.kind == "Write")
        .collect();
    let mut pulses = vec![];
    for (i, r) in writes.iter().enumerate().rev().take(8).rev() {
        let mut observed_us = [None; 6];
        observed_us[0] = Some(r.us);
        // Sole outstanding byte only: FIONREAD is not a per-key tracer.
        if writes.len() == 1 {
            observed_us[1] = rows
                .iter()
                .find(|e| e.kind == "Readable" && e.us >= r.us && e.value == "1:true")
                .map(|e| e.us);
        }
        for (stage, source, kind) in [
            (3, "CROSSTERM", "DecodedKey"),
            (4, "CONTEXT", "ReturnedKey"),
            (5, "APP", "Key"),
        ] {
            observed_us[stage] = rows
                .iter()
                .filter(|e| e.source == source && e.kind == kind)
                .nth(i)
                .filter(|e| e.value == r.value)
                .map(|e| e.us);
        }
        pulses.push(Pulse {
            id: i as u64,
            key: r
                .value
                .parse()
                .ok()
                .and_then(char::from_u32)
                .unwrap_or('?')
                .to_string(),
            observed_us,
        });
    }
    let mode = if let Some(p) = &config {
        p[0]
    } else if rows.iter().any(|r| r.source == "RAW") {
        "raw"
    } else if rows.iter().any(|r| r.source == "CONTEXT") {
        "context"
    } else if rows.iter().any(|r| r.source == "CROSSTERM") {
        "crossterm"
    } else {
        c.mode.as_str()
    };
    View {
        micros: at,
        scenario: config
            .as_ref()
            .and_then(|p| p.get(1))
            .unwrap_or(&c.scenario.as_str())
            .to_string(),
        mode: mode.into(),
        running,
        result: if running || !rows.iter().any(|r| r.kind == "VerdictWindow") {
            "measuring; deadline pending".into()
        } else {
            result.verdict
        },
        pause_ms: config
            .as_ref()
            .and_then(|p| p.get(3))
            .and_then(|s| s.parse().ok())
            .unwrap_or(c.pause_ms),
        bytes_generated: result.generated,
        bytes_drained: drained,
        frames: result.frames,
        entries: rows
            .iter()
            .rev()
            .filter(|r| !matches!(r.kind.as_str(), "PollBegin" | "Empty" | "Readable"))
            .take(512)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|r| Entry {
                micros: r.us,
                source: r.source.clone(),
                kind: r.kind.clone(),
                value: r.value.clone(),
            })
            .collect(),
        pulses,
        latencies_us: result.latencies_us,
        dropped: 0,
        illustrative: false,
        observed_stages: [
            true,
            true,
            false,
            mode == "crossterm",
            mode == "context",
            true,
        ],
        controls: "1..7 scenario  M mode  [ ] pressure  R run  A repeat  Space view  Esc exit"
            .into(),
    }
}
fn live(
    mut c: Config,
    depth: ColorDepth,
    replay: Option<Vec<Record>>,
    freeze: Option<u64>,
    mut auto: bool,
) -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let mut ctx = Context::fullscreen()?;
    ctx.session.enter_interactive()?;
    ctx.set_color_depth(depth);
    let mut trial = if replay.is_none() {
        Some(Trial::start(&exe, c.clone())?)
    } else {
        None
    };
    let mut start = Instant::now();
    let mut held: Option<View> = None;
    let mut done_at = None;
    loop {
        if let Some(t) = &mut trial {
            if t.tick()? && done_at.is_none() {
                done_at = Some(Instant::now());
            }
        }
        let current = if let Some(records) = &replay {
            // Quarter-speed display makes subsecond measured gaps inspectable;
            // receipts retain their real timestamps, not playback timestamps.
            view(
                &c,
                records,
                0,
                freeze.unwrap_or(start.elapsed().as_micros() as u64 / 4),
                false,
            )
        } else {
            let t = trial.as_ref().unwrap();
            view(
                &c,
                &t.records,
                t.drained,
                t.elapsed_us(),
                t.report.is_none(),
            )
        };
        let size = crossterm::terminal::size()?;
        ctx.set_root(Node::raster(visual::frame(
            held.as_ref().unwrap_or(&current),
            size.0.clamp(1, 240),
            size.1.clamp(1, 80),
            depth,
        )));
        let mut rerun = false;
        if let Some(Event::Key(k)) = ctx.run_once(Duration::from_millis(8))? {
            match k.code {
                KeyCode::Esc => break,
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Char(' ') => {
                    held = if held.is_some() {
                        None
                    } else {
                        Some(current.clone())
                    };
                }
                KeyCode::Char('R') => rerun = true,
                KeyCode::Char('A') => auto = !auto,
                KeyCode::Char('M') => {
                    let i = probe::MODES.iter().position(|m| *m == c.mode).unwrap();
                    c.mode = probe::MODES[(i + 1) % 3].into();
                    rerun = true;
                }
                KeyCode::Char(ch @ '1'..='7') => {
                    c.scenario = probe::SCENARIOS[ch as usize - '1' as usize].into();
                    rerun = true;
                }
                KeyCode::Char('[') => {
                    c.pause_ms = c.pause_ms.saturating_sub(20);
                    rerun = true;
                }
                KeyCode::Char(']') => {
                    c.pause_ms = (c.pause_ms + 20).min(500);
                    rerun = true;
                }
                KeyCode::Char(ch) if ch.is_ascii_graphic() => {
                    if let Some(t) = &mut trial {
                        if t.report.is_none() {
                            t.send_key(ch as u8)?;
                        }
                    }
                }
                _ => {}
            }
        }
        if auto && done_at.is_some_and(|t| t.elapsed() > Duration::from_secs(2)) {
            rerun = true;
        }
        if rerun {
            held = None;
            start = Instant::now();
            done_at = None;
            if replay.is_none() {
                drop(trial.take());
                trial = Some(Trial::start(&exe, c.clone())?);
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    // Drop supervised trial before restoring the viewer's terminal.
    drop(trial);
    ctx.restore()
}
