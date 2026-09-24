//! Unix terminal event oscilloscope. The supervised child owns a different PTY;
//! the viewer never competes with it for input or renders into its output stream.
#[cfg(unix)]
#[path = "event_pressure_lab/child.rs"]
mod child;
#[cfg(unix)]
#[path = "event_pressure_lab/host.rs"]
mod host;
#[cfg(unix)]
#[path = "event_pressure_lab/probe.rs"]
mod probe;
#[cfg(unix)]
#[path = "event_pressure_lab/trace.rs"]
mod trace;
#[cfg(unix)]
#[path = "event_pressure_lab/visual.rs"]
mod visual;
#[cfg(unix)]
fn main() -> std::io::Result<()> {
    host::main()
}
#[cfg(not(unix))]
fn main() {
    eprintln!("Event Pressure Lab currently requires Unix PTYs; no Windows claim.");
}
