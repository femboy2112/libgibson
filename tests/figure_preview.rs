//! Tests for `gibson::figure::preview` on synthetic meshes only. Pixels are not judged for
//! appearance here; the session views the rendered PNG after a heavy run.
#![cfg(feature = "figure")]

use std::collections::HashMap;

use gibson::figure::preview::{
    bounds_of, frame_bounds, render_preview, PreviewCamera, PreviewImage, PreviewLight,
    PreviewMaterial, PreviewMesh, PreviewOptions,
};

struct M {
    pos: Vec<[f32; 3]>,
    tris: Vec<[u32; 3]>,
    slots: Vec<u8>,
}

impl M {
    fn view(&self) -> PreviewMesh<'_> {
        PreviewMesh {
            positions: &self.pos,
            normals: None,
            triangles: &self.tris,
            slot_per_triangle: &self.slots,
        }
    }
}

fn icosphere(subdiv: u32, radius: f32, center: [f32; 3]) -> M {
    let t = (1.0 + 5.0f32.sqrt()) * 0.5;
    let mut pos: Vec<[f32; 3]> = vec![
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ];
    let mut tris: Vec<[u32; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    for _ in 0..subdiv {
        let mut cache: HashMap<(u32, u32), u32> = HashMap::new();
        let mut next = Vec::with_capacity(tris.len() * 4);
        for tri in &tris {
            let mut mid = [0u32; 3];
            for (k, m) in mid.iter_mut().enumerate() {
                let (a, b) = (tri[k], tri[(k + 1) % 3]);
                let key = (a.min(b), a.max(b));
                *m = *cache.entry(key).or_insert_with(|| {
                    let (p, q) = (pos[a as usize], pos[b as usize]);
                    pos.push([
                        (p[0] + q[0]) * 0.5,
                        (p[1] + q[1]) * 0.5,
                        (p[2] + q[2]) * 0.5,
                    ]);
                    (pos.len() - 1) as u32
                });
            }
            next.push([tri[0], mid[0], mid[2]]);
            next.push([tri[1], mid[1], mid[0]]);
            next.push([tri[2], mid[2], mid[1]]);
            next.push([mid[0], mid[1], mid[2]]);
        }
        tris = next;
    }
    for p in &mut pos {
        let l = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        for (v, c) in p.iter_mut().zip(center) {
            *v = *v / l * radius + c;
        }
    }
    let n = tris.len();
    M {
        pos,
        tris,
        slots: vec![0; n],
    }
}

/// Z-up cylinder from z = 0 to `height`, with caps.
fn cylinder(sides: u32, radius: f32, height: f32, rings: u32) -> M {
    let mut pos = Vec::new();
    let mut tris = Vec::new();
    for r in 0..=rings {
        let z = height * r as f32 / rings as f32;
        for s in 0..sides {
            let a = std::f32::consts::TAU * s as f32 / sides as f32;
            pos.push([radius * a.cos(), radius * a.sin(), z]);
        }
    }
    for r in 0..rings {
        for s in 0..sides {
            let (a, b) = (r * sides + s, r * sides + (s + 1) % sides);
            let (c, d) = (a + sides, b + sides);
            tris.push([a, b, c]);
            tris.push([b, d, c]);
        }
    }
    let (bc, tc) = (pos.len() as u32, pos.len() as u32 + 1);
    pos.push([0.0, 0.0, 0.0]);
    pos.push([0.0, 0.0, height]);
    for s in 0..sides {
        let s1 = (s + 1) % sides;
        tris.push([bc, s1, s]);
        tris.push([tc, rings * sides + s, rings * sides + s1]);
    }
    let n = tris.len();
    M {
        pos,
        tris,
        slots: vec![0; n],
    }
}

fn mats() -> Vec<PreviewMaterial> {
    vec![
        PreviewMaterial {
            albedo_linear: [0.6, 0.15, 0.1],
            roughness: 0.5,
            metalness: 0.0,
            emissive: [0.0; 3],
        },
        PreviewMaterial {
            albedo_linear: [0.1, 0.6, 0.15],
            roughness: 0.5,
            metalness: 0.0,
            emissive: [0.0; 3],
        },
    ]
}

fn small_opts() -> PreviewOptions {
    PreviewOptions {
        width: 320,
        height: 180,
        ssaa: 2,
        ground: true,
        shadow: true,
        contact_shadow: false,
    }
}

fn camera_for(m: &M) -> PreviewCamera {
    frame_bounds(bounds_of(&[m.view()]).unwrap(), 320.0 / 180.0, 0.8)
}

fn lum(p: [u8; 3]) -> f32 {
    0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32
}

fn diff_count(a: &PreviewImage, b: &PreviewImage) -> usize {
    assert_eq!((a.width, a.height), (b.width, b.height));
    a.rgb8
        .chunks(3)
        .zip(b.rgb8.chunks(3))
        .filter(|(x, y)| x != y)
        .count()
}

#[test]
fn deterministic_byte_identical() {
    let m = icosphere(3, 0.5, [0.0, 0.0, 0.5]);
    let cam = camera_for(&m);
    let l = PreviewLight::default_rig();
    let a = render_preview(&[m.view()], &mats(), &cam, &l, small_opts());
    let b = render_preview(&[m.view()], &mats(), &cam, &l, small_opts());
    assert_eq!(a.rgb8.len(), 320 * 180 * 3);
    assert_eq!(a, b);
    assert_eq!(a.to_png_bytes(), b.to_png_bytes());
    // Non-vacuous: the image is not a flat colour.
    assert!(a.rgb8.chunks(3).any(|p| p != &a.rgb8[0..3]));
}

fn crc32_ref(bytes: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in bytes {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

fn adler_ref(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in bytes {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

#[test]
fn png_bytes_decode_valid() {
    let m = icosphere(2, 0.5, [0.0, 0.0, 0.5]);
    let cam = camera_for(&m);
    let img = render_preview(
        &[m.view()],
        &mats(),
        &cam,
        &PreviewLight::default_rig(),
        small_opts(),
    );
    let png = img.to_png_bytes();
    assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    let mut pos = 8;
    let mut idat = Vec::new();
    let mut kinds = Vec::new();
    while pos < png.len() {
        let n = u32::from_be_bytes(png[pos..pos + 4].try_into().unwrap()) as usize;
        let kind = png[pos + 4..pos + 8].to_vec();
        let data = &png[pos + 8..pos + 8 + n];
        let crc = u32::from_be_bytes(png[pos + 8 + n..pos + 12 + n].try_into().unwrap());
        assert_eq!(crc, crc32_ref(&png[pos + 4..pos + 8 + n]), "CRC {kind:?}");
        if kind == b"IHDR" {
            assert_eq!(n, 13);
            assert_eq!(u32::from_be_bytes(data[0..4].try_into().unwrap()), 320);
            assert_eq!(u32::from_be_bytes(data[4..8].try_into().unwrap()), 180);
            assert_eq!(&data[8..], &[8, 2, 0, 0, 0]);
        }
        if kind == b"IDAT" {
            idat.extend_from_slice(data);
        }
        kinds.push(kind);
        pos += 12 + n;
    }
    assert_eq!(pos, png.len());
    assert_eq!(kinds.first().unwrap(), b"IHDR");
    assert_eq!(kinds.last().unwrap(), b"IEND");
    // Inflate the stored blocks and compare to the scanlines.
    assert_eq!(&idat[..2], &[0x78, 0x01]);
    let mut raw = Vec::new();
    let mut p = 2;
    loop {
        let fin = idat[p] & 1 == 1;
        assert_eq!(idat[p] >> 1, 0, "stored block");
        let l = u16::from_le_bytes([idat[p + 1], idat[p + 2]]);
        let nl = u16::from_le_bytes([idat[p + 3], idat[p + 4]]);
        assert_eq!(l, !nl);
        raw.extend_from_slice(&idat[p + 5..p + 5 + l as usize]);
        p += 5 + l as usize;
        if fin {
            break;
        }
    }
    assert_eq!(
        u32::from_be_bytes(idat[p..p + 4].try_into().unwrap()),
        adler_ref(&raw)
    );
    assert_eq!(p + 4, idat.len());
    let mut expect = Vec::new();
    for row in img.rgb8.chunks(320 * 3) {
        expect.push(0u8);
        expect.extend_from_slice(row);
    }
    assert_eq!(raw, expect);
}

#[test]
fn control_change_changes_subject_pixels() {
    let a = icosphere(3, 0.5, [0.0, 0.0, 0.5]);
    let cam = camera_for(&a);
    let l = PreviewLight::default_rig();
    let ra = render_preview(&[a.view()], &mats(), &cam, &l, small_opts());
    // Control: identical input again gives zero differing pixels (so the diff below is real).
    let ra2 = render_preview(&[a.view()], &mats(), &cam, &l, small_opts());
    assert_eq!(diff_count(&ra, &ra2), 0);
    // Control change: same topology and counts, vertices moved.
    let mut b = icosphere(3, 0.5, [0.0, 0.0, 0.5]);
    for p in &mut b.pos {
        p[0] += 0.35;
        p[2] *= 1.3;
    }
    let rb = render_preview(&[b.view()], &mats(), &cam, &l, small_opts());
    let changed = diff_count(&ra, &rb);
    assert!(
        changed > 1500,
        "vertex update must change pixels: {changed}"
    );
}

#[test]
fn material_slot_selects_colour() {
    let mut m = icosphere(3, 0.5, [0.0, 0.0, 0.5]);
    let cam = camera_for(&m);
    let l = PreviewLight::default_rig();
    let (cx, cy) = cam.project([0.0, -0.5, 0.5], 320, 180).unwrap();
    let red = render_preview(&[m.view()], &mats(), &cam, &l, small_opts());
    m.slots.iter_mut().for_each(|s| *s = 1);
    let green = render_preview(&[m.view()], &mats(), &cam, &l, small_opts());
    let (pr, pg) = (
        red.pixel(cx as u32, cy as u32),
        green.pixel(cx as u32, cy as u32),
    );
    assert!(pr[0] > pr[1], "slot 0 red-dominant: {pr:?}");
    assert!(pg[1] > pg[0], "slot 1 green-dominant: {pg:?}");
}

fn shadow_lights() -> Vec<PreviewLight> {
    vec![
        PreviewLight {
            direction_to_light: [-0.8, 0.0, 0.6],
            color_linear: [1.0, 1.0, 1.0],
            casts_shadow: true,
        },
        PreviewLight {
            direction_to_light: [0.0, -0.8, 0.6],
            color_linear: [0.3, 0.3, 0.3],
            casts_shadow: false,
        },
    ]
}

#[test]
fn ground_shadow_present_and_control() {
    let m = cylinder(24, 0.3, 1.8, 6);
    let cam = camera_for(&m);
    let l = shadow_lights();
    let on = render_preview(&[m.view()], &mats(), &cam, &l, small_opts());
    let off = render_preview(
        &[m.view()],
        &mats(),
        &cam,
        &l,
        PreviewOptions {
            shadow: false,
            ..small_opts()
        },
    );
    // Shadow of the axis at mid height (0.9 m): ground point (0,0,0.9) - L * (0.9 / 0.6).
    let under = cam.project([1.2, 0.0, 0.0], 320, 180).unwrap();
    let far = cam.project([-1.2, 0.0, 0.0], 320, 180).unwrap();
    let px = |img: &PreviewImage, p: (f32, f32)| img.pixel(p.0 as u32, p.1 as u32);
    let (u_on, f_on) = (lum(px(&on, under)), lum(px(&on, far)));
    let (u_off, f_off) = (lum(px(&off, under)), lum(px(&off, far)));
    assert!(u_on < 0.8 * f_on, "shadowed {u_on} vs far {f_on}");
    assert!((u_off - f_off).abs() <= 3.0, "control: {u_off} vs {f_off}");
    assert!(
        (f_on - f_off).abs() <= 3.0,
        "lit side unchanged: {f_on} vs {f_off}"
    );
}

#[test]
fn framing_fills_requested_fraction() {
    let m = cylinder(24, 0.3, 1.8, 6);
    let bounds = bounds_of(&[m.view()]).unwrap();
    let cam = frame_bounds(bounds, 960.0 / 540.0, 0.8);
    let (mut y0, mut y1) = (f32::MAX, f32::MIN);
    for p in &m.pos {
        let (_, y) = cam.project(*p, 960, 540).expect("in front of camera");
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    let frac = (y1 - y0) / 540.0;
    assert!((0.6..=1.0).contains(&frac), "height fraction {frac}");
}

#[test]
fn degenerate_inputs_do_not_panic() {
    let pos = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [f32::NAN, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, f32::INFINITY],
    ];
    let tris = vec![
        [0, 0, 0],
        [0, 1, 2],
        [0, 1, 3],
        [0, 1, 99],
        [0, 1, 4],
        [0, 4, 5],
    ];
    let slots = vec![0u8, 200];
    let bad_normals = vec![[f32::NAN; 3]; 6];
    let meshes = [
        PreviewMesh {
            positions: &pos,
            normals: None,
            triangles: &tris,
            slot_per_triangle: &slots,
        },
        PreviewMesh {
            positions: &pos,
            normals: Some(&bad_normals),
            triangles: &tris,
            slot_per_triangle: &[],
        },
        PreviewMesh {
            positions: &[],
            normals: None,
            triangles: &[],
            slot_per_triangle: &[],
        },
    ];
    let cam = PreviewCamera {
        eye: [0.0, -3.0, 1.0],
        target: [0.0, 0.0, 0.0],
        up: [0.0, 0.0, 1.0],
        vfov_deg: 30.0,
    };
    for (w, h, ssaa) in [(1, 1, 1), (17, 9, 2), (64, 36, 9), (0, 0, 0)] {
        let o = PreviewOptions {
            width: w,
            height: h,
            ssaa,
            ground: true,
            shadow: true,
            contact_shadow: true,
        };
        let img = render_preview(&meshes, &[], &cam, &[], o);
        assert_eq!(img.rgb8.len(), (img.width * img.height * 3) as usize);
        let img = render_preview(&meshes, &mats(), &cam, &PreviewLight::default_rig(), o);
        assert_eq!(img.rgb8.len(), (img.width * img.height * 3) as usize);
        let _ = img.to_png_bytes();
    }
    // Empty scene, camera looking straight along up, NaN camera.
    let none = render_preview(&[], &[], &cam, &[], small_opts());
    assert_eq!(none.rgb8.len(), 320 * 180 * 3);
    let weird = PreviewCamera {
        eye: [0.0, 0.0, 5.0],
        target: [0.0, 0.0, 0.0],
        up: [0.0, 0.0, 1.0],
        vfov_deg: f32::NAN,
    };
    let m = icosphere(1, 0.5, [0.0, 0.0, 0.5]);
    let _ = render_preview(
        &[m.view()],
        &mats(),
        &weird,
        &PreviewLight::default_rig(),
        small_opts(),
    );
}

#[test]
fn large_mesh_renders() {
    // ~20k triangles, 960x540 at 1x: exercises scale (timing is reported by the gate, not asserted).
    let m = icosphere(5, 0.5, [0.0, 0.0, 0.5]);
    assert!(m.tris.len() >= 20_000);
    let cam = frame_bounds(bounds_of(&[m.view()]).unwrap(), 960.0 / 540.0, 0.8);
    let o = PreviewOptions {
        width: 960,
        height: 540,
        ssaa: 1,
        ground: true,
        shadow: true,
        contact_shadow: true,
    };
    let img = render_preview(&[m.view()], &mats(), &cam, &PreviewLight::default_rig(), o);
    assert_eq!(img.rgb8.len(), 960 * 540 * 3);
}

// ---------------------------------------------------------------------------------------------
// Contact shadow and shadow root (a standing subject must not read as floating)
// ---------------------------------------------------------------------------------------------

fn add_box(m: &mut M, lo: [f32; 3], hi: [f32; 3]) {
    let base = m.pos.len() as u32;
    for i in 0..8 {
        m.pos.push([
            if i & 1 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 4 == 0 { lo[2] } else { hi[2] },
        ]);
    }
    for q in [
        [0u32, 1, 3, 2],
        [4, 6, 7, 5],
        [0, 4, 5, 1],
        [2, 3, 7, 6],
        [0, 2, 6, 4],
        [1, 5, 7, 3],
    ] {
        m.tris.push([base + q[0], base + q[1], base + q[2]]);
        m.tris.push([base + q[0], base + q[2], base + q[3]]);
        m.slots.extend([0, 0]);
    }
}

/// Synthetic standing figure, +Z up: two feet boxes (10 x 26 x 6 cm) at x = -0.12 and +0.12,
/// thin legs, and a torso box. The whole figure is raised by `lift`. A tiny speck on the z = 0
/// plane, 1 m to the right, keeps the ground plane at z = 0 (the plane sits at the lowest vertex)
/// and is far outside the contact reach of the feet, so a lifted figure really hovers.
fn standing_figure(lift: f32) -> M {
    let mut m = M {
        pos: Vec::new(),
        tris: Vec::new(),
        slots: Vec::new(),
    };
    for fx in [-0.12f32, 0.12] {
        add_box(
            &mut m,
            [fx - 0.05, -0.13, lift],
            [fx + 0.05, 0.13, lift + 0.06],
        );
        add_box(
            &mut m,
            [fx - 0.04, -0.05, lift + 0.06],
            [fx + 0.04, 0.05, lift + 0.9],
        );
    }
    add_box(&mut m, [-0.25, -0.10, lift + 0.9], [0.25, 0.10, lift + 1.7]);
    let b = m.pos.len() as u32;
    m.pos
        .extend([[1.0, 0.0, 0.0], [1.02, 0.0, 0.0], [1.0, 0.02, 0.0]]);
    m.tris.push([b, b + 1, b + 2]);
    m.slots.push(0);
    m
}

fn no_cast_rig() -> [PreviewLight; 3] {
    let mut r = PreviewLight::default_rig();
    for l in &mut r {
        l.casts_shadow = false;
    }
    r
}

const CW: u32 = 640;
const CH: u32 = 360;

fn contact_opts(contact_shadow: bool) -> PreviewOptions {
    PreviewOptions {
        width: CW,
        height: CH,
        ssaa: 2,
        ground: true,
        shadow: true,
        contact_shadow,
    }
}

/// Camera from the front at 55 degrees elevation, framed on the grounded figure.
fn contact_camera() -> PreviewCamera {
    let g = standing_figure(0.0);
    gibson::figure::preview::frame_bounds_view(
        bounds_of(&[g.view()]).unwrap(),
        CW as f32 / CH as f32,
        0.7,
        0.0,
        55.0,
    )
}

/// Mean luminance of the 3 x 3 pixel patch around world point `p`.
fn patch_lum(img: &PreviewImage, cam: &PreviewCamera, p: [f32; 3]) -> f32 {
    let (sx, sy) = cam.project(p, CW, CH).expect("on screen");
    let (cx, cy) = (sx.round() as i64, sy.round() as i64);
    let mut s = 0.0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (x, y) = ((cx + dx) as u32, (cy + dy) as u32);
            assert!(x < CW && y < CH, "patch off screen at {p:?}");
            s += lum(img.pixel(x, y));
        }
    }
    s / 9.0
}

/// Ground point `d` metres in front of (towards the camera from) the front edge of the foot at
/// x = `fx`.
fn front_of_foot(fx: f32, d: f32) -> [f32; 3] {
    [fx, -0.13 - d, 0.0]
}

#[test]
fn contact_shadow_darkens_ground_beside_each_foot() {
    let fig = standing_figure(0.0);
    let cam = contact_camera();
    let rig = no_cast_rig();
    let on = render_preview(&[fig.view()], &mats(), &cam, &rig, contact_opts(true));
    let off = render_preview(&[fig.view()], &mats(), &cam, &rig, contact_opts(false));
    // Open ground, well away from the feet and the speck.
    let open_on = patch_lum(&on, &cam, [0.55, 0.0, 0.0]);
    let open_off = patch_lum(&off, &cam, [0.55, 0.0, 0.0]);
    assert!(
        (open_on - open_off).abs() < 0.5,
        "open ground is untouched: {open_on} vs {open_off}"
    );
    for fx in [-0.12f32, 0.12] {
        let near = patch_lum(&on, &cam, front_of_foot(fx, 0.02));
        let near_off = patch_lum(&off, &cam, front_of_foot(fx, 0.02));
        // Stated margin: next to a foot the ground is at least 8 luminance levels darker than
        // open ground, and at least 8 darker than the same pixels without the term.
        assert!(
            near < open_on - 8.0,
            "foot x={fx}: near {near} vs open {open_on}"
        );
        assert!(
            near < near_off - 8.0,
            "foot x={fx}: contact on {near} vs off {near_off}"
        );
    }
    // The sides of a foot darken too (outer side of the left foot).
    let side_on = patch_lum(&on, &cam, [-0.17 - 0.02, 0.0, 0.0]);
    let side_off = patch_lum(&off, &cam, [-0.17 - 0.02, 0.0, 0.0]);
    assert!(side_on < side_off - 8.0, "side {side_on} vs {side_off}");
}

#[test]
fn contact_darkening_falls_off_with_distance() {
    let fig = standing_figure(0.0);
    let cam = contact_camera();
    let rig = no_cast_rig();
    let on = render_preview(&[fig.view()], &mats(), &cam, &rig, contact_opts(true));
    let off = render_preview(&[fig.view()], &mats(), &cam, &rig, contact_opts(false));
    for fx in [-0.12f32, 0.12] {
        // Three radii out from the foot's front edge: 2, 6 and 10 cm.
        let drop: Vec<f32> = [0.02f32, 0.06, 0.10]
            .iter()
            .map(|&d| {
                let p = front_of_foot(fx, d);
                patch_lum(&off, &cam, p) - patch_lum(&on, &cam, p)
            })
            .collect();
        assert!(
            drop[0] >= 8.0 && drop[1] >= 3.0 && drop[2] >= 0.3,
            "foot x={fx}: still darkened at every radius: {drop:?}"
        );
        assert!(
            drop[0] > drop[1] + 1.0 && drop[1] > drop[2] + 1.0,
            "foot x={fx}: strictly monotone falloff over 3 radii: {drop:?}"
        );
    }
}

#[test]
fn a_lifted_figure_has_no_contact_darkening() {
    // Control: the same figure raised 0.2 m (well above the contact band) over the same ground
    // plane; the render with the term on is identical, pixel for pixel, around the feet.
    let fig = standing_figure(0.2);
    let cam = contact_camera();
    let rig = no_cast_rig();
    let on = render_preview(&[fig.view()], &mats(), &cam, &rig, contact_opts(true));
    let off = render_preview(&[fig.view()], &mats(), &cam, &rig, contact_opts(false));
    for fx in [-0.12f32, 0.12] {
        for d in [0.0f32, 0.02, 0.06, 0.10] {
            let p = front_of_foot(fx, d);
            let (a, b) = (patch_lum(&on, &cam, p), patch_lum(&off, &cam, p));
            assert!(
                (a - b).abs() < 0.5,
                "hovering foot x={fx} d={d}: {a} vs {b}"
            );
        }
    }
    // Non-vacuous: the grounded figure under the identical camera and options does darken there.
    let g = standing_figure(0.0);
    let gon = render_preview(&[g.view()], &mats(), &cam, &rig, contact_opts(true));
    let goff = render_preview(&[g.view()], &mats(), &cam, &rig, contact_opts(false));
    let p = front_of_foot(-0.12, 0.02);
    assert!(patch_lum(&goff, &cam, p) - patch_lum(&gon, &cam, p) > 8.0);
}

#[test]
fn contact_shadow_is_deterministic_and_never_brightens() {
    let fig = standing_figure(0.0);
    let cam = contact_camera();
    let rig = PreviewLight::default_rig();
    let a = render_preview(&[fig.view()], &mats(), &cam, &rig, contact_opts(true));
    let b = render_preview(&[fig.view()], &mats(), &cam, &rig, contact_opts(true));
    assert_eq!(a, b, "byte-identical");
    let off = render_preview(&[fig.view()], &mats(), &cam, &rig, contact_opts(false));
    // With the real key light casting a shadow too: every channel is <= the render without the
    // term (the term only darkens), and a substantial set of pixels is strictly darker.
    assert!(a.rgb8.iter().zip(&off.rgb8).all(|(x, y)| x <= y));
    let darker = a
        .rgb8
        .chunks(3)
        .zip(off.rgb8.chunks(3))
        .filter(|(x, y)| x != y)
        .count();
    assert!(darker > 300, "contact term changed {darker} pixels");
}

#[test]
fn the_shadow_of_a_vertex_on_the_ground_lands_at_its_screen_position() {
    // A pole standing on the ground; the key light is low and from -X, so the shadow falls to
    // +X. The root of the shadow must touch the pole's base on screen (the shadow of a vertex at
    // ground height is the vertex itself), and the far end must be where the projection says.
    let pole = cylinder(24, 0.06, 1.0, 8);
    let bounds = ([-0.5, -0.5, 0.0], [1.8, 0.5, 1.0]);
    let cam = frame_bounds(bounds, CW as f32 / CH as f32, 0.8);
    let l = shadow_lights();
    let on = render_preview(
        &[pole.view()],
        &mats(),
        &cam,
        &l,
        PreviewOptions {
            contact_shadow: false,
            ..contact_opts(false)
        },
    );
    let off = render_preview(
        &[pole.view()],
        &mats(),
        &cam,
        &l,
        PreviewOptions {
            shadow: false,
            contact_shadow: false,
            ..contact_opts(false)
        },
    );
    // Just right of the pole's base (5 cm clear of its 6 cm radius): inside the shadow.
    let root = patch_lum(&off, &cam, [0.11, 0.0, 0.0]) - patch_lum(&on, &cam, [0.11, 0.0, 0.0]);
    assert!(
        root >= 10.0,
        "shadow root is attached to the base: drop {root}"
    );
    // Away from the shadow side: no shadow.
    let away = patch_lum(&off, &cam, [-0.25, 0.0, 0.0]) - patch_lum(&on, &cam, [-0.25, 0.0, 0.0]);
    assert!(away.abs() <= 2.0, "no shadow on the lit side: drop {away}");
    // The shadow region (pixels darker by >= 10 levels) ends where the top of the pole projects:
    // ground x = 1.333 for a 1 m pole at -0.8/0.6 slope; allow 0.25 m for the soft blur.
    let mut right = 0u32;
    for y in 0..CH {
        for x in 0..CW {
            if lum(off.pixel(x, y)) - lum(on.pixel(x, y)) >= 10.0 {
                right = right.max(x);
            }
        }
    }
    let lo = cam.project([1.2, 0.0, 0.0], CW, CH).unwrap().0;
    let hi = cam.project([1.333 + 0.25, 0.0, 0.0], CW, CH).unwrap().0;
    assert!(
        (right as f32) >= lo && (right as f32) <= hi,
        "shadow ends at x={right}, expected within [{lo}, {hi}]"
    );
}
