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
    };
    let img = render_preview(&[m.view()], &mats(), &cam, &PreviewLight::default_rig(), o);
    assert_eq!(img.rgb8.len(), 960 * 540 * 3);
}
