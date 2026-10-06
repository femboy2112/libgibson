//! Recipe: **a scientific plot is one representation, not the truth.**
//!
//! The one law: the *observable* (here a signal the application computes) is owned
//! by you; [`gibson::plot`] only **realizes** it faithfully into a `Surface`. The
//! same data could be a ring, a field, a number — a plot is just the representation
//! you chose for this scene. A [`Timeline`] directs the framing: it decides whether
//! we are surveying a wide window or zoomed into detail, and the plot's `PlotView`
//! follows.
//!
//! Teaches: `plot::plot(spec, view, area, mode)`, app-owned data vs library-owned
//! realization, and a timeline driving a plot's view window.
//!
//!   cargo run --example show_observable            # live, scrolling scope
//!   cargo run --example show_observable -- at 9.0  # one framed window at t=9 s

use gibson::input::{Event, KeyCode, KeyModifiers};
use gibson::plot::{self, AxisScale, AxisSpec, FiniteRange, PlotSpec, PlotView, Series};
use gibson::timeline::Timeline;
use gibson::ui::{raster, screen, skins, App, AppEvent, BuildCx, Control, Element};
use gibson::{Rect, SubcellGlyphMode, Surface};

#[derive(Clone)]
enum Msg {}

/// The APPLICATION owns the science. This is a two-tone damped oscillation — a
/// stand-in for whatever real observable your program produces. The plot never
/// sees this function; it only ever receives sampled points.
fn signal(t: f64) -> f64 {
    let env = (-(t * 0.06)).exp();
    ((t * 2.0).sin() + 0.4 * (t * 5.3).sin() + 0.15 * (t * 11.0).sin()) * env
}

/// Which framing the director wants. Opaque timeline payload.
#[derive(Clone, Copy)]
enum Framing {
    /// Wide survey: a long window pans past.
    Survey,
    /// Detail: a short window, the same signal read closely.
    Detail,
}

impl Framing {
    /// The visible time window (seconds) for this framing.
    fn window(self) -> f64 {
        match self {
            Framing::Survey => 14.0,
            Framing::Detail => 4.0,
        }
    }
}

fn film() -> Timeline<Framing> {
    Timeline::new()
        .cut(6.0, Framing::Survey)
        .cut(6.0, Framing::Detail)
        .cut(4.0, Framing::Survey)
}

fn frame(w: u16, h: u16, edit: f32, mode: SubcellGlyphMode) -> Surface {
    let framing = film()
        .top(edit)
        .map(|a| *a.payload)
        .unwrap_or(Framing::Survey);
    let window = framing.window();

    // The view scrolls so the newest sample sits at the right edge.
    let x_hi = edit as f64;
    let x_lo = x_hi - window;

    // Sample the app's observable across the visible window. ~2 samples/column.
    let cols = (w as usize).max(2) * 2;
    let pts: Vec<(f64, f64)> = (0..=cols)
        .map(|i| {
            let x = x_lo + (x_hi - x_lo) * (i as f64 / cols as f64);
            (x, signal(x))
        })
        .collect();

    let spec = PlotSpec::new(
        AxisSpec::new(AxisScale::Linear, "time").unit("s"),
        AxisSpec::new(AxisScale::Linear, "amplitude"),
    )
    .title(match framing {
        Framing::Survey => "OBSERVABLE · survey",
        Framing::Detail => "OBSERVABLE · detail",
    })
    .series(Series::line(pts).color((120, 220, 255)));

    let (x0, x1) = (x_lo.min(x_hi - 1e-3), x_hi);
    let view = match (FiniteRange::new(x0, x1), FiniteRange::new(-1.2, 1.2)) {
        (Some(x), Some(y)) => PlotView::new(x, y),
        _ => return blank(w, h),
    };

    match plot::plot(&spec, &view, Rect::new(0, 0, w.max(1), h.max(1)), mode) {
        Ok((surface, _report)) => surface,
        Err(_) => blank(w, h),
    }
}

fn blank(w: u16, h: u16) -> Surface {
    gibson::raster::RgbRaster::new(w.max(1), h.saturating_mul(2).max(2)).to_surface()
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("at") {
        let t: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        for line in frame(100, 30, t, SubcellGlyphMode::Braille2x4).to_visible_lines() {
            println!("{line}");
        }
        return Ok(());
    }
    App::fullscreen()
        .skin(skins::VAPOR95)
        .fps(30)
        .run((), update, view)?;
    Ok(())
}

fn view(_m: &(), cx: &BuildCx) -> Element<Msg> {
    let (w, h) = (cx.environment.width, cx.environment.height);
    let edit = cx.time.as_secs_f32() % film().duration().max(0.001);
    screen::<Msg>().child(raster::<Msg>(frame(w, h, edit, SubcellGlyphMode::Braille2x4)).grow(1.0))
}

fn update(_m: &mut (), event: AppEvent<Msg>) -> Control {
    if let AppEvent::Input(Event::Key(key)) = event {
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            return Control::Quit;
        }
    }
    Control::Continue
}
