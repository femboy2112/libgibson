//! Recipe: **put a flat surface in a moving world.**
//!
//! The one law: a 2-D [`RgbRaster`] becomes a textured quad in 3-D via
//! [`Rasterizer::textured_quad`], and the *camera* has the opinion. The content is
//! an object fixed in the world; moving the eye is what makes it cinematic — the
//! difference between a slideshow and a picture. A [`Timeline`] drives which leg of
//! the camera move is playing, and the leg's `progress` eases the eye.
//!
//! This is the compressed form of the flagship's "facade": perspective-map any
//! surface you can draw, then fly past it. Only public `raster3d` is used.
//!
//! Teaches: `Rasterizer`/`textured_quad`, an explicit `Camera`, and a timeline
//! sequencing camera legs.
//!
//!   cargo run --example show_spatial_plane            # live, loops the fly-by
//!   cargo run --example show_spatial_plane -- shot 5.0 out.ppm   # one frame -> PPM

use gibson::geom::Vec3;
use gibson::input::{Event, KeyCode, KeyModifiers};
use gibson::raster::RgbRaster;
use gibson::raster3d::{Camera, Rasterizer};
use gibson::timeline::Timeline;
use gibson::ui::{raster, screen, skins, App, AppEvent, BuildCx, Control, Element};
use gibson::Surface;

#[derive(Clone)]
enum Msg {}

/// Which leg of the fly-by. Opaque timeline payload.
#[derive(Clone, Copy)]
enum Leg {
    Approach,
    Orbit,
    Retreat,
}

fn film() -> Timeline<Leg> {
    Timeline::new()
        .cut(3.0, Leg::Approach)
        .cut(4.0, Leg::Orbit)
        .cut(3.0, Leg::Retreat)
}

/// The one textured object: a checker-and-gradient panel (16:9-ish). Any RgbRaster
/// works — this one just has enough structure to read perspective foreshortening.
fn panel_texture() -> RgbRaster {
    let (tw, th) = (96u16, 54u16);
    let mut r = RgbRaster::new(tw, th);
    for y in 0..th as i32 {
        for x in 0..tw as i32 {
            let checker = ((x / 8) + (y / 8)) % 2 == 0;
            let gx = (x as f32 / tw as f32 * 180.0) as u8;
            let base = if checker { (30, 20, 60) } else { (18, 12, 40) };
            r.set(x, y, (base.0 + gx / 3, base.1 + gx / 4, base.2 + gx / 2));
        }
    }
    r
}

/// Smoothstep for eased eye motion.
fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// The camera for a leg at `progress` in `0..1`. The plane sits in the xy-plane at
/// z = 0; the eye lives at negative z and looks at the origin.
fn camera_for(leg: Leg, progress: f32) -> Camera {
    let e = ease(progress);
    let (eye, fov) = match leg {
        // Dolly straight in from far to near.
        Leg::Approach => (Vec3::new(0.0, 0.3, lerp(-9.0, -3.6, e)), 0.9),
        // Quarter-orbit at the near radius.
        Leg::Orbit => {
            let a = e * std::f32::consts::FRAC_PI_2;
            let r = 4.2;
            (Vec3::new(r * a.sin(), 0.8, -r * a.cos()), 0.9)
        }
        // Dolly back out.
        Leg::Retreat => (Vec3::new(0.0, 0.3, lerp(-3.6, -9.0, e)), 0.9),
    };
    Camera {
        position: eye,
        target: Vec3::new(0.0, 0.0, 0.0),
        up: Vec3::new(0.0, 1.0, 0.0),
        fov_y: fov,
        near: 0.1,
        far: 60.0,
    }
}

fn frame(tex: &RgbRaster, w: u16, h: u16, edit: f32) -> Surface {
    let ph = h.saturating_mul(2).max(2);
    let mut rz = Rasterizer::new(w.max(1), ph);
    rz.clear((6, 5, 14));

    let cam = match film().top(edit) {
        Some(a) => camera_for(*a.payload, a.progress),
        None => camera_for(Leg::Retreat, 1.0),
    };

    // The plane: half-width 1.6, half-height 0.9, wound TL -> TR -> BR -> BL to
    // match the texture's UV corners.
    let (hw, hh) = (1.6, 0.9);
    let corners = [
        Vec3::new(-hw, hh, 0.0),
        Vec3::new(hw, hh, 0.0),
        Vec3::new(hw, -hh, 0.0),
        Vec3::new(-hw, -hh, 0.0),
    ];
    rz.textured_quad(corners, tex, &cam);

    let mut surface = rz.raster.to_surface();
    if w >= 12 && h >= 2 {
        let leg = match film().top(edit).map(|a| *a.payload) {
            Some(Leg::Approach) => "APPROACH",
            Some(Leg::Orbit) => "ORBIT",
            _ => "RETREAT",
        };
        surface.bake_text(2, 1, leg, (236, 236, 250), true);
    }
    surface
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let tex = panel_texture();

    // One raw frame to PPM (full pixel resolution) for geometry/look sanity.
    if args.get(1).map(String::as_str) == Some("shot") {
        let t: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5.0);
        let path = args
            .get(3)
            .cloned()
            .unwrap_or_else(|| "plane.ppm".to_string());
        let mut rz = Rasterizer::new(480, 270);
        rz.clear((6, 5, 14));
        let cam = match film().top(t) {
            Some(a) => camera_for(*a.payload, a.progress),
            None => camera_for(Leg::Retreat, 1.0),
        };
        let (hw, hh) = (1.6, 0.9);
        rz.textured_quad(
            [
                Vec3::new(-hw, hh, 0.0),
                Vec3::new(hw, hh, 0.0),
                Vec3::new(hw, -hh, 0.0),
                Vec3::new(-hw, -hh, 0.0),
            ],
            &tex,
            &cam,
        );
        let file = std::fs::File::create(&path)?;
        rz.raster
            .write_ppm(std::io::BufWriter::new(file))
            .expect("write ppm");
        println!("show_spatial_plane: frame at {t:.1}s -> {path}");
        return Ok(());
    }

    App::fullscreen()
        .skin(skins::VAPOR95)
        .fps(30)
        .run(tex, update, view)?;
    Ok(())
}

fn view(tex: &RgbRaster, cx: &BuildCx) -> Element<Msg> {
    let (w, h) = (cx.environment.width, cx.environment.height);
    let edit = cx.time.as_secs_f32() % film().duration().max(0.001);
    screen::<Msg>().child(raster::<Msg>(frame(tex, w, h, edit)).grow(1.0))
}

fn update(_tex: &mut RgbRaster, event: AppEvent<Msg>) -> Control {
    if let AppEvent::Input(Event::Key(key)) = event {
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            return Control::Quit;
        }
    }
    Control::Continue
}
