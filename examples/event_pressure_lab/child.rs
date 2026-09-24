//! Three input paths, identical optional Surface/Context output workload.
//! The silent raw/Crossterm controls never construct a LibGibson Context.
use super::trace::{clock_us, Recorder};
use gibson::capability::ColorDepth;
use gibson::input::{Event, KeyCode};
use gibson::raster::RgbRaster;
use gibson::{Context, Node};
use std::{io, path::Path, time::Duration};

struct RawGuard;
impl Drop for RawGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

pub fn run(mode: &str, dir: &Path, epoch: u64, graphics: bool, burst: bool) -> io::Result<()> {
    let mut trace = Recorder::new(&dir.join("child.tsv"), epoch)?;
    let mut ctx = if mode == "context" || graphics {
        Some(Context::fullscreen()?)
    } else {
        None
    };
    if let Some(c) = &mut ctx {
        c.session.enter_interactive()?;
        c.set_color_depth(ColorDepth::TrueColor);
        if burst {
            c.set_max_fps(240);
        }
    }
    crossterm::terminal::enable_raw_mode()?;
    let _guard = RawGuard;
    if mode != "raw" {
        let _ = crossterm::event::poll(Duration::from_millis(1))?;
    }
    trace.emit("CHILD", "Gate", "source-initialized")?;
    let end = clock_us() + 8_000_000;
    while !dir.join("go").exists() && clock_us() < end {
        std::thread::sleep(Duration::from_millis(1));
    }
    trace.emit("CHILD", "Released", mode)?;
    let mut frame = 0u64;
    let mut last_frame = 0;
    let mut old_size = (0, 0);
    while clock_us() < end && !dir.join("stop").exists() {
        let now = clock_us();
        let mut pending: libc::c_int = 0;
        // SAFETY: stdin belongs to this child's PTY; ioctl only observes queue.
        let ok = unsafe { libc::ioctl(0, libc::FIONREAD, &mut pending) };
        let mut p = libc::pollfd {
            fd: 0,
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut p, 1, 0) };
        if ok == 0 && pending > 0 {
            trace.emit("TTY", "Readable", format!("{pending}:{}", ready > 0))?;
        }
        let size = crossterm::terminal::size()?;
        if size != old_size {
            trace.emit("TTY", "Size", format!("{}x{}", size.0, size.1))?;
            old_size = size;
        }
        if graphics && now - last_frame >= if burst { 4_167 } else { 16_667 } {
            let (w, h) = (size.0.clamp(1, 240), size.1.clamp(1, 80));
            let mut raster = RgbRaster::new(w, h * 2);
            for y in 0..h * 2 {
                for x in 0..w {
                    let phase = ((u64::from(x) * 7 + u64::from(y) * 11 + frame * 19) % 256) as u8;
                    raster.set(
                        x as i32,
                        y as i32,
                        (phase, phase.wrapping_add(85), phase.wrapping_add(170)),
                    );
                }
            }
            ctx.as_mut()
                .unwrap()
                .set_root(Node::raster(raster.to_surface()));
            trace.emit("RENDER", "FrameBegin", frame)?;
            last_frame = now;
        }
        let before = ctx.as_mut().map(|c| c.stats().frames).unwrap_or(0);
        trace.emit(mode_source(mode), "PollBegin", "")?;
        match mode {
            "raw" => {
                let mut p = libc::pollfd {
                    fd: 0,
                    events: libc::POLLIN,
                    revents: 0,
                };
                let n = unsafe { libc::poll(&mut p, 1, 5) };
                if n > 0 && p.revents & libc::POLLIN != 0 {
                    let mut bytes = [0u8; 4096];
                    let n = unsafe { libc::read(0, bytes.as_mut_ptr().cast(), bytes.len()) };
                    if n < 0 {
                        return Err(io::Error::last_os_error());
                    }
                    trace.emit("RAW", "Read", n)?;
                    for b in &bytes[..n as usize] {
                        trace.emit("APP", "Key", u32::from(*b))?;
                    }
                } else {
                    trace.emit("RAW", "Empty", "")?;
                }
            }
            "crossterm" => {
                if crossterm::event::poll(Duration::from_millis(5))? {
                    trace.emit("CROSSTERM", "PollReady", "")?;
                    match crossterm::event::read()? {
                        crossterm::event::Event::Key(k) => {
                            if let crossterm::event::KeyCode::Char(c) = k.code {
                                trace.emit("CROSSTERM", "DecodedKey", c as u32)?;
                                trace.emit("APP", "Key", c as u32)?;
                            }
                        }
                        crossterm::event::Event::Resize(w, h) => {
                            trace.emit("CROSSTERM", "Resize", format!("{w}x{h}"))?
                        }
                        _ => {}
                    }
                } else {
                    trace.emit("CROSSTERM", "Empty", "")?;
                }
            }
            "context" => {
                if let Some(ev) = ctx.as_mut().unwrap().run_once(Duration::from_millis(5))? {
                    match ev {
                        Event::Key(k) => {
                            if let KeyCode::Char(c) = k.code {
                                trace.emit("CONTEXT", "ReturnedKey", c as u32)?;
                                trace.emit("APP", "Key", c as u32)?;
                            }
                        }
                        Event::Resize(w, h) => {
                            trace.emit("CONTEXT", "Resize", format!("{w}x{h}"))?
                        }
                        _ => {}
                    }
                } else {
                    trace.emit("CONTEXT", "Empty", "")?;
                }
            }
            _ => return Err(io::Error::other("unknown child mode")),
        }
        if graphics {
            let c = ctx.as_mut().unwrap();
            if mode != "context" {
                c.render_if_due()?;
            }
            let stats = c.stats();
            if stats.frames > before {
                trace.emit(
                    "RENDER",
                    "FrameCommitted",
                    format!("{}:{}", frame, stats.frame_bytes),
                )?;
                frame += 1;
            }
        }
        // One input per Context step, with a bounded 1 ms yield: no spin when
        // the scheduler has no root/frame to advance its deadline in silent mode.
        std::thread::sleep(Duration::from_millis(1));
    }
    if let Some(c) = &mut ctx {
        c.restore()?;
    }
    crossterm::terminal::disable_raw_mode()?;
    trace.emit("CHILD", "Restored", "")?;
    Ok(())
}
fn mode_source(mode: &str) -> &str {
    match mode {
        "raw" => "RAW",
        "context" => "CONTEXT",
        _ => "CROSSTERM",
    }
}
