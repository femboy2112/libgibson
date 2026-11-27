//! Renders a synthetic mannequin-ish scene (spheres and cylinders, two material slots) with the
//! `gibson::figure::preview` software rasteriser and writes a PNG.
//!
//! ```text
//! figure_preview_png <out.png> [width height]
//! ```
//! Run with `--features figure`. Pixels are for a human to view; nothing here judges them.
#![forbid(unsafe_code)]

use gibson::figure::preview::{
    bounds_of, frame_bounds, render_preview, PreviewLight, PreviewMaterial, PreviewMesh,
    PreviewOptions,
};

struct Soup {
    pos: Vec<[f32; 3]>,
    tris: Vec<[u32; 3]>,
    slots: Vec<u8>,
}

/// UV sphere (Z up) as an ellipsoid with radii `r`, centred at `c`.
fn ellipsoid(c: [f32; 3], r: [f32; 3], slot: u8, s: &mut Soup) {
    let (rings, sides) = (24u32, 48u32);
    let base = s.pos.len() as u32;
    for i in 0..=rings {
        let phi = std::f32::consts::PI * i as f32 / rings as f32;
        for j in 0..sides {
            let th = std::f32::consts::TAU * j as f32 / sides as f32;
            s.pos.push([
                c[0] + r[0] * phi.sin() * th.cos(),
                c[1] + r[1] * phi.sin() * th.sin(),
                c[2] + r[2] * phi.cos(),
            ]);
        }
    }
    for i in 0..rings {
        for j in 0..sides {
            let (a, b) = (base + i * sides + j, base + i * sides + (j + 1) % sides);
            let (c2, d) = (a + sides, b + sides);
            s.tris.push([a, c2, b]);
            s.tris.push([b, c2, d]);
            s.slots.push(slot);
            s.slots.push(slot);
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(out) = args.next() else {
        eprintln!("usage: figure_preview_png <out.png> [width height]");
        std::process::exit(2);
    };
    let w: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(1280);
    let h: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(720);
    let mut s = Soup {
        pos: Vec::new(),
        tris: Vec::new(),
        slots: Vec::new(),
    };
    ellipsoid([0.0, 0.0, 1.55], [0.11, 0.12, 0.14], 1, &mut s); // head
    ellipsoid([0.0, 0.0, 1.20], [0.20, 0.12, 0.30], 0, &mut s); // torso
    ellipsoid([0.0, 0.0, 0.85], [0.18, 0.11, 0.14], 0, &mut s); // pelvis
    ellipsoid([-0.10, 0.0, 0.42], [0.07, 0.07, 0.42], 0, &mut s); // legs
    ellipsoid([0.10, 0.0, 0.42], [0.07, 0.07, 0.42], 0, &mut s);
    ellipsoid([-0.30, 0.0, 1.15], [0.05, 0.05, 0.30], 1, &mut s); // arms
    ellipsoid([0.30, 0.0, 1.15], [0.05, 0.05, 0.30], 1, &mut s);
    let mesh = PreviewMesh {
        positions: &s.pos,
        normals: None,
        triangles: &s.tris,
        slot_per_triangle: &s.slots,
    };
    let mats = [
        PreviewMaterial {
            albedo_linear: [0.10, 0.22, 0.55],
            roughness: 0.55,
            metalness: 0.0,
            emissive: [0.0; 3],
        },
        PreviewMaterial {
            albedo_linear: [0.62, 0.38, 0.28],
            roughness: 0.45,
            metalness: 0.0,
            emissive: [0.0; 3],
        },
    ];
    let cam = frame_bounds(
        bounds_of(&[mesh]).expect("non-empty scene"),
        w as f32 / h as f32,
        0.8,
    );
    let opts = PreviewOptions {
        width: w,
        height: h,
        ssaa: 2,
        ground: true,
        shadow: true,
    };
    let t0 = std::time::Instant::now();
    let img = render_preview(&[mesh], &mats, &cam, &PreviewLight::default_rig(), opts);
    let dt = t0.elapsed();
    std::fs::write(&out, img.to_png_bytes()).expect("write png");
    println!(
        "wrote {out} {}x{} tris={} render_ms={}",
        img.width,
        img.height,
        s.tris.len(),
        dt.as_millis()
    );
}
