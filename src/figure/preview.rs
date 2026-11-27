//! Offscreen software-raster preview for real meshes. EXPERIMENTAL (feature `figure`).
//!
//! A decoupled, deterministic, dependency-free renderer: z-buffered triangle raster with
//! perspective-correct attributes, smooth (area-weighted when not supplied) normals, Blinn-Phong
//! key/fill/rim lights evaluated in linear light with an sRGB encode, optional 2x2 SSAA, a
//! background gradient, and a ground plane at the lowest point of the subject along the camera
//! `up` axis carrying a planar-projected soft shadow of the first shadow-casting light.
//!
//! World convention: `camera.up` is the world up axis (the helpers [`frame_bounds`] and
//! [`frame_bounds_view`] assume +Z up and put the camera on the -Y side, so +X is screen right at
//! azimuth 0). Inputs are plain slices; nothing here depends on any studio type.
//!
//! Limits (by design, for a preview): triangles with any vertex behind the near plane are
//! skipped (not clipped); shadows are planar projections onto the ground only (no self
//! shadowing); two-sided shading; materials outside the table fall back to
//! [`PreviewMaterial::DEFAULT`]. Non-finite vertices and out-of-range indices are skipped, never
//! a panic. Pixels are viewed by the session after a heavy run; nothing here claims appearance.

type V3 = [f32; 3];

const NEAR: f32 = 1.0e-3;
const SHADOW_N: usize = 512;
const SHADOW_STRENGTH: f32 = 0.85;
const AMB_SKY: V3 = [0.22, 0.25, 0.30];
const AMB_GROUND: V3 = [0.10, 0.09, 0.08];
const BG_TOP: V3 = [0.10, 0.12, 0.16];
const BG_BOTTOM: V3 = [0.30, 0.31, 0.33];
const GROUND_MATERIAL: PreviewMaterial = PreviewMaterial {
    albedo_linear: [0.42, 0.42, 0.44],
    roughness: 1.0,
    metalness: 0.0,
    emissive: [0.0, 0.0, 0.0],
};

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn mulc(a: V3, b: V3) -> V3 {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn len(a: V3) -> f32 {
    dot(a, a).sqrt()
}
fn finite3(a: V3) -> bool {
    a[0].is_finite() && a[1].is_finite() && a[2].is_finite()
}
fn normalize_or(a: V3, fallback: V3) -> V3 {
    let l = len(a);
    if l.is_finite() && l > 1.0e-20 {
        mul(a, 1.0 / l)
    } else {
        fallback
    }
}
fn lerp3(a: V3, b: V3, t: f32) -> V3 {
    add(mul(a, 1.0 - t), mul(b, t))
}
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// One triangle soup to draw. `slot_per_triangle[i]` selects the material of `triangles[i]`
/// (missing entries use slot 0). `normals`, when `Some` with one finite entry per position, are
/// used as given; otherwise smooth normals are computed area-weighted from the triangles.
#[derive(Clone, Copy, Debug)]
pub struct PreviewMesh<'a> {
    pub positions: &'a [[f32; 3]],
    pub normals: Option<&'a [[f32; 3]]>,
    pub triangles: &'a [[u32; 3]],
    pub slot_per_triangle: &'a [u8],
}

/// Linear-light material.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewMaterial {
    pub albedo_linear: [f32; 3],
    pub roughness: f32,
    pub metalness: f32,
    pub emissive: [f32; 3],
}

impl PreviewMaterial {
    /// Neutral grey, used for slots outside the material table.
    pub const DEFAULT: PreviewMaterial = PreviewMaterial {
        albedo_linear: [0.5, 0.5, 0.5],
        roughness: 0.6,
        metalness: 0.0,
        emissive: [0.0, 0.0, 0.0],
    };
}

/// Directional light. `direction_to_light` points from the scene toward the light (need not be
/// normalised). `color_linear` is linear radiance scale. The first light with
/// `casts_shadow == true` casts the ground shadow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewLight {
    pub direction_to_light: [f32; 3],
    pub color_linear: [f32; 3],
    pub casts_shadow: bool,
}

impl PreviewLight {
    /// Key (front-left-high, casts the shadow), fill and rim for a +Z-up world seen from the -Y
    /// side.
    pub fn default_rig() -> [PreviewLight; 3] {
        [
            PreviewLight {
                direction_to_light: [-0.5, -0.6, 0.7],
                color_linear: [1.0, 0.96, 0.90],
                casts_shadow: true,
            },
            PreviewLight {
                direction_to_light: [0.7, -0.4, 0.3],
                color_linear: [0.30, 0.34, 0.42],
                casts_shadow: false,
            },
            PreviewLight {
                direction_to_light: [0.2, 0.9, 0.5],
                color_linear: [0.45, 0.45, 0.50],
                casts_shadow: false,
            },
        ]
    }
}

/// Pinhole camera. `up` is also the world up axis used for the ground plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewCamera {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub vfov_deg: f32,
}

struct Basis {
    eye: V3,
    f: V3,
    r: V3,
    u: V3,
    tx: f32,
    ty: f32,
}

fn basis(cam: &PreviewCamera, aspect: f32) -> Basis {
    let f = normalize_or(sub(cam.target, cam.eye), [0.0, 1.0, 0.0]);
    let up = normalize_or(cam.up, [0.0, 0.0, 1.0]);
    let mut r = cross(f, up);
    if len(r) < 1.0e-6 {
        r = cross(f, [0.0, 0.0, 1.0]);
        if len(r) < 1.0e-6 {
            r = cross(f, [0.0, 1.0, 0.0]);
        }
    }
    let r = normalize_or(r, [1.0, 0.0, 0.0]);
    let u = cross(r, f);
    let vf = if cam.vfov_deg.is_finite() {
        cam.vfov_deg.clamp(1.0, 170.0)
    } else {
        40.0
    };
    let ty = (vf.to_radians() * 0.5).tan();
    Basis {
        eye: cam.eye,
        f,
        r,
        u,
        tx: ty * aspect,
        ty,
    }
}

impl PreviewCamera {
    /// Projects a world point to pixel coordinates (x right, y down) of a `width` x `height`
    /// image; `None` when behind the camera or non-finite.
    pub fn project(&self, p: [f32; 3], width: u32, height: u32) -> Option<(f32, f32)> {
        if width == 0 || height == 0 || !finite3(p) {
            return None;
        }
        let b = basis(self, width as f32 / height as f32);
        let v = sub(p, b.eye);
        let zc = dot(v, b.f);
        if zc.is_nan() || zc <= NEAR {
            return None;
        }
        let sx = (0.5 + 0.5 * dot(v, b.r) / (zc * b.tx)) * width as f32;
        let sy = (0.5 - 0.5 * dot(v, b.u) / (zc * b.ty)) * height as f32;
        Some((sx, sy))
    }
}

fn grow(lo: &mut V3, hi: &mut V3, p: V3) {
    for ((l, h), v) in lo.iter_mut().zip(hi.iter_mut()).zip(p) {
        *l = l.min(v);
        *h = h.max(v);
    }
}

/// Axis-aligned bounds `(min, max)` of the finite vertices referenced by the meshes' triangles.
pub fn bounds_of(meshes: &[PreviewMesh<'_>]) -> Option<([f32; 3], [f32; 3])> {
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    let mut any = false;
    for m in meshes {
        for t in m.triangles {
            for &i in t {
                if let Some(&p) = m.positions.get(i as usize) {
                    if finite3(p) {
                        any = true;
                        grow(&mut lo, &mut hi, p);
                    }
                }
            }
        }
    }
    if any {
        Some((lo, hi))
    } else {
        None
    }
}

/// Fits a camera (azimuth 0, elevation 12 degrees, vfov 30, +Z up, from the -Y side) so the
/// bounds fill `fill` (default 0.8) of the frame height. See [`frame_bounds_view`].
pub fn frame_bounds(bounds: ([f32; 3], [f32; 3]), aspect: f32, fill: f32) -> PreviewCamera {
    frame_bounds_view(bounds, aspect, fill, 0.0, 12.0)
}

/// As [`frame_bounds`] with explicit azimuth and elevation (degrees; azimuth 0 looks along +Y).
/// The eye distance is bisected so the projected bounding-box corners span `fill` of the frame
/// height (or of the width, if that is the larger fraction).
pub fn frame_bounds_view(
    bounds: ([f32; 3], [f32; 3]),
    aspect: f32,
    fill: f32,
    azimuth_deg: f32,
    elevation_deg: f32,
) -> PreviewCamera {
    let (lo, hi) = bounds;
    let fill = if fill.is_finite() {
        fill.clamp(0.05, 1.0)
    } else {
        0.8
    };
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        16.0 / 9.0
    };
    let vfov = 30.0f32;
    let up = [0.0, 0.0, 1.0];
    let center = mul(add(lo, hi), 0.5);
    if !finite3(center) || !finite3(lo) || !finite3(hi) {
        return PreviewCamera {
            eye: [0.0, -3.0, 0.5],
            target: [0.0, 0.0, 0.0],
            up,
            vfov_deg: vfov,
        };
    }
    let half = mul(sub(hi, lo), 0.5);
    let rad = len(half).max(1.0e-3);
    let (az, el) = (azimuth_deg.to_radians(), elevation_deg.to_radians());
    let d = [az.sin() * el.cos(), -az.cos() * el.cos(), el.sin()];
    let mut corners = [[0.0f32; 3]; 8];
    for (i, c) in corners.iter_mut().enumerate() {
        *c = [
            if i & 1 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 4 == 0 { lo[2] } else { hi[2] },
        ];
    }
    let frac = |dist: f32| -> f32 {
        let cam = PreviewCamera {
            eye: add(center, mul(d, dist)),
            target: center,
            up,
            vfov_deg: vfov,
        };
        let b = basis(&cam, aspect);
        let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for c in &corners {
            let v = sub(*c, b.eye);
            let zc = dot(v, b.f);
            if zc <= NEAR {
                return 1.0e9;
            }
            let nx = dot(v, b.r) / (zc * b.tx);
            let ny = dot(v, b.u) / (zc * b.ty);
            x0 = x0.min(nx);
            x1 = x1.max(nx);
            y0 = y0.min(ny);
            y1 = y1.max(ny);
        }
        ((x1 - x0) * 0.5).max((y1 - y0) * 0.5)
    };
    let mut lo_d = rad * 1.001;
    let mut hi_d = rad * 1.0e4;
    for _ in 0..80 {
        let mid = 0.5 * (lo_d + hi_d);
        if frac(mid) > fill {
            lo_d = mid;
        } else {
            hi_d = mid;
        }
    }
    PreviewCamera {
        eye: add(center, mul(d, hi_d)),
        target: center,
        up,
        vfov_deg: vfov,
    }
}

/// Render options. `ssaa` is clamped to 1 or 2 (2x2 supersampling); sizes clamp to 1..=8192.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewOptions {
    pub width: u32,
    pub height: u32,
    pub ssaa: u8,
    pub ground: bool,
    pub shadow: bool,
}

impl Default for PreviewOptions {
    fn default() -> Self {
        PreviewOptions {
            width: 960,
            height: 540,
            ssaa: 2,
            ground: true,
            shadow: true,
        }
    }
}

/// Packed 8-bit sRGB image, row-major, top row first, 3 bytes per pixel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewImage {
    pub width: u32,
    pub height: u32,
    pub rgb8: Vec<u8>,
}

const fn crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        t[n] = c;
        n += 1;
    }
    t
}
const CRC_TABLE: [u32; 256] = crc_table();

fn crc32(bytes: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in bytes {
        c = CRC_TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in bytes {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

impl PreviewImage {
    /// RGB of pixel `(x, y)`; panics if out of range.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        let i = (y as usize * self.width as usize + x as usize) * 3;
        [self.rgb8[i], self.rgb8[i + 1], self.rgb8[i + 2]]
    }

    /// PNG file bytes: 8-bit truecolor, filter 0, zlib with stored (uncompressed) deflate blocks,
    /// hand-written CRC-32 and Adler-32.
    pub fn to_png_bytes(&self) -> Vec<u8> {
        let w = self.width as usize;
        let h = self.height as usize;
        let mut raw = Vec::with_capacity(h * (w * 3 + 1));
        for y in 0..h {
            raw.push(0u8);
            raw.extend_from_slice(&self.rgb8[y * w * 3..(y + 1) * w * 3]);
        }
        let mut z = vec![0x78u8, 0x01];
        let n = raw.len();
        let mut pos = 0usize;
        loop {
            let l = (n - pos).min(65535);
            let fin = pos + l >= n;
            z.push(u8::from(fin));
            z.extend_from_slice(&(l as u16).to_le_bytes());
            z.extend_from_slice(&(!(l as u16)).to_le_bytes());
            z.extend_from_slice(&raw[pos..pos + l]);
            pos += l;
            if fin {
                break;
            }
        }
        z.extend_from_slice(&adler32(&raw).to_be_bytes());
        let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&self.width.to_be_bytes());
        ihdr.extend_from_slice(&self.height.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        push_chunk(&mut out, b"IHDR", &ihdr);
        push_chunk(&mut out, b"IDAT", &z);
        push_chunk(&mut out, b"IEND", &[]);
        out
    }
}

// ---------------------------------------------------------------------------------------------
// Scene preparation
// ---------------------------------------------------------------------------------------------

struct Flat {
    pos: Vec<V3>,
    nrm: Vec<V3>,
    tris: Vec<[u32; 3]>,
    slot: Vec<u8>,
}

fn flatten(meshes: &[PreviewMesh<'_>], up: V3) -> Flat {
    let mut f = Flat {
        pos: Vec::new(),
        nrm: Vec::new(),
        tris: Vec::new(),
        slot: Vec::new(),
    };
    for m in meshes {
        let base = f.pos.len() as u32;
        let n = m.positions.len();
        let given = match m.normals {
            Some(ns) if ns.len() == n && ns.iter().all(|v| finite3(*v)) => Some(ns),
            _ => None,
        };
        let mut acc = vec![[0.0f32; 3]; n];
        let ok = |t: &[u32; 3]| t.iter().all(|&i| (i as usize) < n);
        for t in m.triangles.iter().filter(|t| ok(t)) {
            let (a, b, c) = (
                m.positions[t[0] as usize],
                m.positions[t[1] as usize],
                m.positions[t[2] as usize],
            );
            let fnorm = cross(sub(b, a), sub(c, a));
            if finite3(fnorm) {
                for &i in t {
                    acc[i as usize] = add(acc[i as usize], fnorm);
                }
            }
        }
        f.pos.extend_from_slice(m.positions);
        for (i, a) in acc.iter().enumerate() {
            let v = match given {
                Some(g) => normalize_or(g[i], up),
                None => normalize_or(*a, up),
            };
            f.nrm.push(v);
        }
        for (ti, t) in m.triangles.iter().enumerate() {
            if !ok(t) {
                continue;
            }
            f.tris.push([t[0] + base, t[1] + base, t[2] + base]);
            f.slot
                .push(m.slot_per_triangle.get(ti).copied().unwrap_or(0));
        }
    }
    f
}

fn edge(p: [f32; 2], q: [f32; 2], r: [f32; 2]) -> f32 {
    (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
}

/// Rasterises one triangle over pixel centres of a `w` x `h` grid, calling `f(x, y, b0, b1, b2)`
/// with screen-space barycentrics for every covered centre. Two-sided; degenerate skipped.
fn raster_tri(
    a: [f32; 2],
    b: [f32; 2],
    c: [f32; 2],
    w: usize,
    h: usize,
    mut f: impl FnMut(usize, usize, f32, f32, f32),
) {
    let area = edge(a, b, c);
    if !area.is_finite() || area.abs() <= 1.0e-12 {
        return;
    }
    let inv = 1.0 / area;
    let x0 = (a[0].min(b[0]).min(c[0]) - 0.5).ceil().max(0.0);
    let x1 = (a[0].max(b[0]).max(c[0]) - 0.5).floor().min(w as f32 - 1.0);
    let y0 = (a[1].min(b[1]).min(c[1]) - 0.5).ceil().max(0.0);
    let y1 = (a[1].max(b[1]).max(c[1]) - 0.5).floor().min(h as f32 - 1.0);
    if x0 > x1 || y0 > y1 {
        return;
    }
    for y in (y0 as usize)..=(y1 as usize) {
        let py = y as f32 + 0.5;
        for x in (x0 as usize)..=(x1 as usize) {
            let p = [x as f32 + 0.5, py];
            let b0 = edge(p, b, c) * inv;
            let b1 = edge(a, p, c) * inv;
            let b2 = edge(a, b, p) * inv;
            if b0 >= 0.0 && b1 >= 0.0 && b2 >= 0.0 {
                f(x, y, b0, b1, b2);
            }
        }
    }
}

struct Light {
    dir: V3,
    color: V3,
    casts: bool,
}

fn shin_of(roughness: f32) -> f32 {
    let r = if roughness.is_finite() {
        roughness.clamp(0.05, 1.0)
    } else {
        0.6
    };
    (2.0 / (r * r * r * r + 1.0e-4) - 2.0).clamp(4.0, 512.0)
}

/// Blinn-Phong in linear light. `shadow` in 0..1 attenuates shadow-casting lights.
fn shade_surface(
    p: V3,
    n: V3,
    eye: V3,
    mat: &PreviewMaterial,
    lights: &[Light],
    up: V3,
    shadow: f32,
) -> V3 {
    let v = normalize_or(sub(eye, p), [0.0, 0.0, 1.0]);
    let n = if dot(n, v) < 0.0 { mul(n, -1.0) } else { n };
    let metal = if mat.metalness.is_finite() {
        mat.metalness.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let alb = mat.albedo_linear;
    let diff = mul(alb, 1.0 - metal);
    let f0 = lerp3([0.04; 3], alb, metal);
    let amb = lerp3(AMB_GROUND, AMB_SKY, 0.5 + 0.5 * dot(n, up));
    let shin = shin_of(mat.roughness);
    let norm = (shin + 2.0) / (8.0 * std::f32::consts::PI);
    let mut c = add(mulc(diff, amb), mat.emissive);
    for l in lights {
        let ndl = dot(n, l.dir);
        if ndl <= 0.0 {
            continue;
        }
        let vis = if l.casts {
            1.0 - SHADOW_STRENGTH * shadow
        } else {
            1.0
        };
        let h = normalize_or(add(l.dir, v), n);
        let ndh = dot(n, h).max(0.0);
        let spec = ndh.powf(shin) * norm * ndl;
        let lc = mul(l.color, vis);
        c = add(c, mulc(mulc(diff, lc), [ndl; 3]));
        c = add(c, mulc(mulc(f0, lc), [spec; 3]));
    }
    c
}

struct Ground {
    h: f32,
    up: V3,
    e1: V3,
    e2: V3,
    ca: f32,
    cb: f32,
    radius: f32,
    half: f32,
    shadow: Vec<f32>,
}

fn shadow_mark(cov: &mut [f32], x: f32, y: f32) {
    if x >= 0.0 && y >= 0.0 {
        let (ix, iy) = (x as usize, y as usize);
        if ix < SHADOW_N && iy < SHADOW_N {
            cov[iy * SHADOW_N + ix] = 1.0;
        }
    }
}

fn box_blur(src: &[f32], r: usize, horizontal: bool) -> Vec<f32> {
    let n = SHADOW_N;
    let mut out = vec![0.0f32; n * n];
    let norm = 1.0 / (2 * r + 1) as f32;
    for y in 0..n {
        for x in 0..n {
            let mut s = 0.0f32;
            for k in 0..=2 * r {
                let o = k as isize - r as isize;
                let (xx, yy) = if horizontal {
                    (x as isize + o, y as isize)
                } else {
                    (x as isize, y as isize + o)
                };
                if xx >= 0 && yy >= 0 && (xx as usize) < n && (yy as usize) < n {
                    s += src[yy as usize * n + xx as usize];
                }
            }
            out[y * n + x] = s * norm;
        }
    }
    out
}

fn build_shadow(flat: &Flat, g: &Ground, l: V3) -> Vec<f32> {
    let n = SHADOW_N;
    let nf = n as f32;
    let ldu = dot(l, g.up);
    let mut cov = vec![0.0f32; n * n];
    let proj = |p: V3| -> Option<[f32; 2]> {
        if !finite3(p) {
            return None;
        }
        let k = (dot(p, g.up) - g.h) / ldu;
        let q = sub(p, mul(l, k));
        let a = dot(q, g.e1) - g.ca;
        let b = dot(q, g.e2) - g.cb;
        let s = nf / (2.0 * g.half);
        let (gx, gy) = ((a + g.half) * s, (b + g.half) * s);
        if gx.is_finite() && gy.is_finite() {
            Some([gx, gy])
        } else {
            None
        }
    };
    let pts: Vec<Option<[f32; 2]>> = flat.pos.iter().map(|&p| proj(p)).collect();
    for t in &flat.tris {
        let (Some(a), Some(b), Some(c)) =
            (pts[t[0] as usize], pts[t[1] as usize], pts[t[2] as usize])
        else {
            continue;
        };
        raster_tri(a, b, c, n, n, |x, y, _, _, _| cov[y * n + x] = 1.0);
        // Sub-cell triangles can miss every cell centre: also mark vertices, midpoints, centroid.
        for (p, q) in [(a, b), (b, c), (c, a)] {
            shadow_mark(&mut cov, p[0], p[1]);
            shadow_mark(&mut cov, 0.5 * (p[0] + q[0]), 0.5 * (p[1] + q[1]));
        }
        shadow_mark(
            &mut cov,
            (a[0] + b[0] + c[0]) / 3.0,
            (a[1] + b[1] + c[1]) / 3.0,
        );
    }
    for _ in 0..2 {
        cov = box_blur(&cov, 4, true);
        cov = box_blur(&cov, 4, false);
    }
    for v in &mut cov {
        *v = (*v * 1.4).min(1.0);
    }
    cov
}

fn shadow_at(g: &Ground, a: f32, b: f32) -> f32 {
    if g.shadow.is_empty() {
        return 0.0;
    }
    let n = SHADOW_N;
    let s = n as f32 / (2.0 * g.half);
    let gx = (a + g.half) * s - 0.5;
    let gy = (b + g.half) * s - 0.5;
    let range = -1.0..=n as f32;
    if !(gx.is_finite() && gy.is_finite()) || !range.contains(&gx) || !range.contains(&gy) {
        return 0.0;
    }
    let (fx, fy) = (gx.floor(), gy.floor());
    let (tx, ty) = (gx - fx, gy - fy);
    let at = |ix: f32, iy: f32| -> f32 {
        let r = 0.0..n as f32;
        if !r.contains(&ix) || !r.contains(&iy) {
            0.0
        } else {
            g.shadow[iy as usize * n + ix as usize]
        }
    };
    let top = at(fx, fy) * (1.0 - tx) + at(fx + 1.0, fy) * tx;
    let bot = at(fx, fy + 1.0) * (1.0 - tx) + at(fx + 1.0, fy + 1.0) * tx;
    top * (1.0 - ty) + bot * ty
}

fn srgb_encode(x: f32) -> u8 {
    let x = if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) };
    let e = if x <= 0.003_130_8 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    };
    (e * 255.0 + 0.5) as u8
}

fn render_inner(
    meshes: &[PreviewMesh<'_>],
    materials: &[PreviewMaterial],
    camera: &PreviewCamera,
    lights_in: &[PreviewLight],
    opts: PreviewOptions,
) -> PreviewImage {
    let ssaa = opts.ssaa.clamp(1, 2) as usize;
    let ow = opts.width.clamp(1, 8192) as usize;
    let oh = opts.height.clamp(1, 8192) as usize;
    let (ww, hh) = (ow * ssaa, oh * ssaa);
    let bas = basis(camera, ow as f32 / oh as f32);
    let up = normalize_or(camera.up, [0.0, 0.0, 1.0]);
    let flat = flatten(meshes, up);
    let lights: Vec<Light> = lights_in
        .iter()
        .filter(|l| finite3(l.direction_to_light) && finite3(l.color_linear))
        .map(|l| Light {
            dir: normalize_or(l.direction_to_light, up),
            color: l.color_linear,
            casts: l.casts_shadow,
        })
        .collect();

    // Project vertices: (sx, sy, 1/z); 1/z <= 0 marks "unusable".
    let sp: Vec<V3> = flat
        .pos
        .iter()
        .map(|&p| {
            let v = sub(p, bas.eye);
            let zc = dot(v, bas.f);
            if !finite3(p) || zc.is_nan() || zc <= NEAR {
                return [0.0, 0.0, 0.0];
            }
            let sx = (0.5 + 0.5 * dot(v, bas.r) / (zc * bas.tx)) * ww as f32;
            let sy = (0.5 - 0.5 * dot(v, bas.u) / (zc * bas.ty)) * hh as f32;
            if sx.is_finite() && sy.is_finite() {
                [sx, sy, 1.0 / zc]
            } else {
                [0.0, 0.0, 0.0]
            }
        })
        .collect();

    let mut depth = vec![f32::INFINITY; ww * hh];
    let mut tri_id = vec![u32::MAX; ww * hh];
    let mut lam = vec![[0.0f32; 2]; ww * hh];
    for (ti, t) in flat.tris.iter().enumerate() {
        let (a, b, c) = (sp[t[0] as usize], sp[t[1] as usize], sp[t[2] as usize]);
        if a[2] <= 0.0 || b[2] <= 0.0 || c[2] <= 0.0 {
            continue;
        }
        raster_tri(
            [a[0], a[1]],
            [b[0], b[1]],
            [c[0], c[1]],
            ww,
            hh,
            |x, y, b0, b1, b2| {
                let iz = b0 * a[2] + b1 * b[2] + b2 * c[2];
                let d = 1.0 / iz;
                let idx = y * ww + x;
                if d < depth[idx] {
                    depth[idx] = d;
                    tri_id[idx] = ti as u32;
                    lam[idx] = [b1 * b[2] / iz, b2 * c[2] / iz];
                }
            },
        );
    }

    // Ground plane at the lowest referenced vertex along `up`.
    let mut ground: Option<Ground> = None;
    if opts.ground {
        if let Some((lo, hi)) = bounds_of_flat(&flat) {
            let h = flat
                .tris
                .iter()
                .flat_map(|t| t.iter())
                .map(|&i| dot(flat.pos[i as usize], up))
                .filter(|v| v.is_finite())
                .fold(f32::INFINITY, f32::min);
            let center = mul(add(lo, hi), 0.5);
            let radius = (0.5 * len(sub(hi, lo))).max(1.0e-3);
            let a0 = if up[0].abs() < 0.9 {
                [1.0, 0.0, 0.0]
            } else {
                [0.0, 1.0, 0.0]
            };
            let e1 = normalize_or(cross(up, a0), [1.0, 0.0, 0.0]);
            let e2 = cross(up, e1);
            let mut g = Ground {
                h,
                up,
                e1,
                e2,
                ca: dot(center, e1),
                cb: dot(center, e2),
                radius,
                half: 4.0 * radius,
                shadow: Vec::new(),
            };
            if opts.shadow {
                if let Some(l) = lights.iter().find(|l| l.casts) {
                    if dot(l.dir, up) > 0.05 {
                        g.shadow = build_shadow(&flat, &g, l.dir);
                    }
                }
            }
            if h.is_finite() && dot(bas.eye, up) - h > 1.0e-6 {
                ground = Some(g);
            }
        }
    }

    // Deferred shading in linear light.
    let mut lin = vec![0.0f32; ww * hh * 3];
    for y in 0..hh {
        let ty = (y as f32 + 0.5) / hh as f32;
        let bg = lerp3(BG_TOP, BG_BOTTOM, ty);
        let ny = (1.0 - 2.0 * ty) * bas.ty;
        for x in 0..ww {
            let idx = y * ww + x;
            let mut col = bg;
            let tid = tri_id[idx];
            if tid != u32::MAX {
                let t = flat.tris[tid as usize];
                let (l1, l2) = (lam[idx][0], lam[idx][1]);
                let l0 = 1.0 - l1 - l2;
                let (pa, pb, pc) = (
                    flat.pos[t[0] as usize],
                    flat.pos[t[1] as usize],
                    flat.pos[t[2] as usize],
                );
                let p = add(add(mul(pa, l0), mul(pb, l1)), mul(pc, l2));
                let nsm = add(
                    add(
                        mul(flat.nrm[t[0] as usize], l0),
                        mul(flat.nrm[t[1] as usize], l1),
                    ),
                    mul(flat.nrm[t[2] as usize], l2),
                );
                let face = normalize_or(cross(sub(pb, pa), sub(pc, pa)), up);
                let n = normalize_or(nsm, face);
                let mat = materials
                    .get(flat.slot[tid as usize] as usize)
                    .unwrap_or(&PreviewMaterial::DEFAULT);
                col = shade_surface(p, n, bas.eye, mat, &lights, up, 0.0);
            } else if let Some(g) = &ground {
                let nx = ((x as f32 + 0.5) / ww as f32 * 2.0 - 1.0) * bas.tx;
                let dir = add(bas.f, add(mul(bas.r, nx), mul(bas.u, ny)));
                let denom = dot(dir, g.up);
                if denom < -1.0e-6 {
                    let tt = (g.h - dot(bas.eye, g.up)) / denom;
                    if tt.is_finite() && tt > 0.0 && tt < depth[idx] {
                        let p = add(bas.eye, mul(dir, tt));
                        let a = dot(p, g.e1) - g.ca;
                        let b = dot(p, g.e2) - g.cb;
                        let dist = (a * a + b * b).sqrt();
                        let fade = smoothstep(2.0 * g.radius, g.half, dist);
                        if fade < 1.0 {
                            let s = shadow_at(g, a, b);
                            let gc =
                                shade_surface(p, g.up, bas.eye, &GROUND_MATERIAL, &lights, g.up, s);
                            col = lerp3(gc, bg, fade);
                        }
                    }
                }
            }
            let o = idx * 3;
            lin[o] = col[0];
            lin[o + 1] = col[1];
            lin[o + 2] = col[2];
        }
    }

    // Box downsample in linear light, then sRGB encode.
    let mut rgb8 = vec![0u8; ow * oh * 3];
    let inv = 1.0 / (ssaa * ssaa) as f32;
    for y in 0..oh {
        for x in 0..ow {
            let mut s = [0.0f32; 3];
            for sy in 0..ssaa {
                for sx in 0..ssaa {
                    let o = ((y * ssaa + sy) * ww + x * ssaa + sx) * 3;
                    s[0] += lin[o].clamp(0.0, 1.0);
                    s[1] += lin[o + 1].clamp(0.0, 1.0);
                    s[2] += lin[o + 2].clamp(0.0, 1.0);
                }
            }
            let o = (y * ow + x) * 3;
            rgb8[o] = srgb_encode(s[0] * inv);
            rgb8[o + 1] = srgb_encode(s[1] * inv);
            rgb8[o + 2] = srgb_encode(s[2] * inv);
        }
    }
    PreviewImage {
        width: ow as u32,
        height: oh as u32,
        rgb8,
    }
}

fn bounds_of_flat(flat: &Flat) -> Option<(V3, V3)> {
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    let mut any = false;
    for t in &flat.tris {
        for &i in t {
            let p = flat.pos[i as usize];
            if finite3(p) {
                any = true;
                grow(&mut lo, &mut hi, p);
            }
        }
    }
    if any {
        Some((lo, hi))
    } else {
        None
    }
}

/// Renders the meshes. Deterministic: identical inputs give byte-identical output.
#[cfg(not(figure_mutant = "preview_stale"))]
pub fn render_preview(
    meshes: &[PreviewMesh<'_>],
    materials: &[PreviewMaterial],
    camera: &PreviewCamera,
    lights: &[PreviewLight],
    opts: PreviewOptions,
) -> PreviewImage {
    render_inner(meshes, materials, camera, lights, opts)
}

#[cfg(figure_mutant = "preview_stale")]
type StaleKey = (usize, usize, usize, PreviewOptions);
#[cfg(figure_mutant = "preview_stale")]
type StaleSlot = Option<(StaleKey, PreviewImage)>;
#[cfg(figure_mutant = "preview_stale")]
thread_local! {
    static STALE: std::cell::RefCell<StaleSlot> = const { std::cell::RefCell::new(None) };
}

/// MUTANT (`figure_mutant = "preview_stale"`): caches by mesh/vertex/triangle counts and options
/// and ignores the vertex values, so a control change that only moves vertices is not drawn.
#[cfg(figure_mutant = "preview_stale")]
pub fn render_preview(
    meshes: &[PreviewMesh<'_>],
    materials: &[PreviewMaterial],
    camera: &PreviewCamera,
    lights: &[PreviewLight],
    opts: PreviewOptions,
) -> PreviewImage {
    let key: StaleKey = (
        meshes.len(),
        meshes.iter().map(|m| m.positions.len()).sum(),
        meshes.iter().map(|m| m.triangles.len()).sum(),
        opts,
    );
    let hit = STALE.with(|c| match c.borrow().as_ref() {
        Some((k, img)) if *k == key => Some(img.clone()),
        _ => None,
    });
    if let Some(img) = hit {
        return img;
    }
    let img = render_inner(meshes, materials, camera, lights, opts);
    STALE.with(|c| *c.borrow_mut() = Some((key, img.clone())));
    img
}
