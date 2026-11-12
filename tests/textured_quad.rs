//! RED-first contracts for the earned v0.5 spatial primitive: a perspective-correct,
//! depth-tested textured quad on [`Rasterizer`]. The texture source is an existing
//! `RgbRaster` — no image-loader dependency. Corner→UV order is
//! TL(0,0) → TR(1,0) → BR(1,1) → BL(0,1); u grows left→right, v grows top→bottom,
//! matching image rows. These pin the §19 contracts of the v0.5 "Observable
//! Instruments" spatial axis BEFORE implementation exists.

use gibson::geom::Vec3;
use gibson::raster::{Rgb, RgbRaster};
use gibson::raster3d::{Camera, Material, Rasterizer};

/// A solid-colour texture.
fn solid(w: u16, h: u16, c: Rgb) -> RgbRaster {
    let mut t = RgbRaster::new(w, h);
    t.clear(c);
    t
}

/// A camera-facing quad at depth `z`, half-extents `hx`/`hy`, in TL→TR→BR→BL order.
fn frontal(z: f32, hx: f32, hy: f32) -> [Vec3; 4] {
    [
        Vec3::new(-hx, hy, z),  // TL uv(0,0)
        Vec3::new(hx, hy, z),   // TR uv(1,0)
        Vec3::new(hx, -hy, z),  // BR uv(1,1)
        Vec3::new(-hx, -hy, z), // BL uv(0,1)
    ]
}

#[test]
fn front_facing_quad_draws_both_triangles_and_fills_centre() {
    let mut r = Rasterizer::new(120, 80);
    r.clear((0, 0, 0));
    let tex = solid(8, 8, (220, 40, 40));
    let drew = r.textured_quad(frontal(3.0, 1.0, 0.7), &tex, &Camera::default());
    assert!(drew, "a frontal quad in view must draw");
    assert_eq!(
        r.stats.triangles_submitted, 2,
        "a quad submits two triangles"
    );
    assert_eq!(r.stats.triangles_drawn, 2);
    // Screen centre (60,40) is the projection of world (0,0,z): dead centre of the quad.
    assert_eq!(r.raster.get(60, 40).unwrap(), (220, 40, 40));
}

#[test]
fn all_four_uv_corners_map_to_their_texels() {
    // 2x2 texture: each texel is one UV quadrant.
    let mut tex = RgbRaster::new(2, 2);
    tex.set(0, 0, (255, 0, 0)); // col0,row0 -> u<.5,v<.5 -> TL red
    tex.set(1, 0, (0, 255, 0)); // col1,row0 -> u>=.5,v<.5 -> TR green
    tex.set(1, 1, (0, 0, 255)); // col1,row1 -> u>=.5,v>=.5 -> BR blue
    tex.set(0, 1, (255, 255, 255)); // col0,row1 -> u<.5,v>=.5 -> BL white
    let cam = Camera::default();
    let mut r = Rasterizer::new(120, 80);
    r.clear((0, 0, 0));
    r.textured_quad(frontal(3.0, 1.5, 1.0), &tex, &cam);
    // Probe the centre of each UV quadrant via its world position on the plane.
    let probe = |wx: f32, wy: f32| {
        let (px, py, _) = cam.project(Vec3::new(wx, wy, 3.0), 120, 80).unwrap();
        r.raster.get(px as i32, py as i32).unwrap()
    };
    assert_eq!(probe(-0.75, 0.5), (255, 0, 0), "TL quadrant -> red texel");
    assert_eq!(probe(0.75, 0.5), (0, 255, 0), "TR quadrant -> green texel");
    assert_eq!(probe(0.75, -0.5), (0, 0, 255), "BR quadrant -> blue texel");
    assert_eq!(
        probe(-0.75, -0.5),
        (255, 255, 255),
        "BL quadrant -> white texel"
    );
}

#[test]
fn texture_mapping_is_perspective_correct_not_affine() {
    // Left edge near (z=2), right edge far (z=5): a quad rotated about the vertical axis.
    // u<0.5 red, u>=0.5 blue. Perspective-correct mapping puts the u=0.5 boundary toward
    // the FAR edge on screen (the near half occupies more columns); affine would put it
    // at the geometric midpoint of the projected edges.
    let mut tex = RgbRaster::new(2, 1);
    tex.set(0, 0, (255, 0, 0));
    tex.set(1, 0, (0, 0, 255));
    let cam = Camera::default();
    let quad = [
        Vec3::new(-1.0, 1.0, 2.0),
        Vec3::new(1.0, 1.0, 5.0),
        Vec3::new(1.0, -1.0, 5.0),
        Vec3::new(-1.0, -1.0, 2.0),
    ];
    let mut r = Rasterizer::new(120, 80);
    r.clear((0, 0, 0));
    r.textured_quad(quad, &tex, &cam);
    // Scan the vertical-centre row for the red->blue transition column.
    let row = 40;
    let mut boundary = None;
    let mut seen_red = false;
    for x in 0..120 {
        match r.raster.get(x, row) {
            Some((255, 0, 0)) => seen_red = true,
            Some((0, 0, 255)) if seen_red => {
                boundary = Some(x);
                break;
            }
            _ => {}
        }
    }
    let boundary = boundary.expect("row must contain a red->blue transition") as f32;
    let left = cam.project(Vec3::new(-1.0, 0.0, 2.0), 120, 80).unwrap().0;
    let right = cam.project(Vec3::new(1.0, 0.0, 5.0), 120, 80).unwrap().0;
    let affine_mid = (left + right) / 2.0;
    assert!(
        boundary > affine_mid + 4.0,
        "perspective must push the u=0.5 boundary ({boundary}) past the affine midpoint ({affine_mid}) toward the far edge"
    );
    assert!(
        boundary < right,
        "boundary must stay left of the far edge ({right})"
    );
}

#[test]
fn nearer_textured_quad_occludes_farther_regardless_of_draw_order() {
    let cam = Camera::default();
    let near = solid(4, 4, (255, 0, 0));
    let far = solid(4, 4, (0, 255, 0));
    for order in 0..2 {
        let mut r = Rasterizer::new(120, 80);
        r.clear((0, 0, 0));
        if order == 0 {
            r.textured_quad(frontal(2.0, 1.0, 0.7), &near, &cam);
            r.textured_quad(frontal(4.0, 1.0, 0.7), &far, &cam);
        } else {
            r.textured_quad(frontal(4.0, 1.0, 0.7), &far, &cam);
            r.textured_quad(frontal(2.0, 1.0, 0.7), &near, &cam);
        }
        assert_eq!(
            r.raster.get(60, 40).unwrap(),
            (255, 0, 0),
            "near quad wins the depth test (order={order})"
        );
    }
}

#[test]
fn draw_is_deterministic() {
    let cam = Camera::default();
    let tex = solid(6, 6, (30, 180, 90));
    let quad = [
        Vec3::new(-1.0, 1.0, 2.0),
        Vec3::new(1.0, 1.0, 4.0),
        Vec3::new(1.0, -1.0, 4.0),
        Vec3::new(-1.0, -1.0, 2.0),
    ];
    let build = || {
        let mut r = Rasterizer::new(120, 80);
        r.clear((0, 0, 0));
        r.textured_quad(quad, &tex, &cam);
        r
    };
    assert_eq!(build().raster.pixels(), build().raster.pixels());
}

#[test]
fn partially_offscreen_quad_clips_without_panic() {
    let cam = Camera::default();
    let tex = solid(4, 4, (10, 200, 10));
    let mut r = Rasterizer::new(120, 80);
    r.clear((0, 0, 0));
    // Left edge far off-screen to the left; right edge on-screen.
    let q = [
        Vec3::new(-6.0, 1.0, 3.0),
        Vec3::new(0.5, 1.0, 3.0),
        Vec3::new(0.5, -1.0, 3.0),
        Vec3::new(-6.0, -1.0, 3.0),
    ];
    assert!(r.textured_quad(q, &tex, &cam), "visible portion must draw");
    assert_eq!(
        r.raster.get(40, 40).unwrap(),
        (10, 200, 10),
        "the on-screen part is textured"
    );
    // Every pixel is either background or the texture colour — no garbage, no OOB.
    assert!(r
        .raster
        .pixels()
        .iter()
        .all(|&px| px == (0, 0, 0) || px == (10, 200, 10)));
}

#[test]
fn quad_behind_camera_draws_nothing() {
    let cam = Camera::default();
    let tex = solid(4, 4, (1, 2, 3));
    let mut r = Rasterizer::new(80, 60);
    r.clear((0, 0, 0));
    let behind = [
        Vec3::new(-1.0, 1.0, -3.0),
        Vec3::new(1.0, 1.0, -3.0),
        Vec3::new(1.0, -1.0, -3.0),
        Vec3::new(-1.0, -1.0, -3.0),
    ];
    assert!(!r.textured_quad(behind, &tex, &cam));
    assert_eq!(r.stats.triangles_drawn, 0);
    assert!(r.raster.pixels().iter().all(|&px| px == (0, 0, 0)));
}

#[test]
fn quad_straddling_near_plane_clips_cleanly() {
    let cam = Camera::default();
    let tex = solid(4, 4, (1, 2, 3));
    let mut r = Rasterizer::new(80, 60);
    r.clear((0, 0, 0));
    // Two corners behind the camera, two in front: must clip, never emit garbage.
    let straddle = [
        Vec3::new(-1.0, 1.0, -1.0),
        Vec3::new(1.0, 1.0, 2.0),
        Vec3::new(1.0, -1.0, 2.0),
        Vec3::new(-1.0, -1.0, -1.0),
    ];
    let _ = r.textured_quad(straddle, &tex, &cam); // must not panic
    assert!(
        r.raster
            .pixels()
            .iter()
            .all(|&px| px == (0, 0, 0) || px == (1, 2, 3)),
        "only background or the texture colour may appear"
    );
}

#[test]
fn degenerate_quads_are_refused() {
    let cam = Camera::default();
    let tex = solid(4, 4, (9, 9, 9));
    let mut r = Rasterizer::new(80, 60);
    r.clear((0, 0, 0));
    // Four collinear corners.
    let line = [
        Vec3::new(-1.0, 0.0, 3.0),
        Vec3::new(-0.3, 0.0, 3.0),
        Vec3::new(0.3, 0.0, 3.0),
        Vec3::new(1.0, 0.0, 3.0),
    ];
    assert!(
        !r.textured_quad(line, &tex, &cam),
        "collinear corners draw nothing"
    );
    // Four coincident corners.
    let point = [Vec3::new(0.0, 0.0, 3.0); 4];
    assert!(
        !r.textured_quad(point, &tex, &cam),
        "zero-area quad draws nothing"
    );
    assert!(r.raster.pixels().iter().all(|&px| px == (0, 0, 0)));
}

#[test]
fn zero_size_texture_is_refused() {
    let cam = Camera::default();
    let tex = RgbRaster::new(0, 0);
    let mut r = Rasterizer::new(80, 60);
    r.clear((0, 0, 0));
    assert!(!r.textured_quad(frontal(3.0, 1.0, 0.7), &tex, &cam));
    assert_eq!(r.stats.triangles_drawn, 0);
    assert!(r.raster.pixels().iter().all(|&px| px == (0, 0, 0)));
}

#[test]
fn hostile_finite_coordinates_stay_bounded() {
    let cam = Camera::default();
    let tex = solid(4, 4, (50, 60, 70));
    let mut r = Rasterizer::new(80, 60);
    r.clear((0, 0, 0));
    let huge = [
        Vec3::new(-1e18, 1e18, 3.0),
        Vec3::new(1e18, 1e18, 3.0),
        Vec3::new(1e18, -1e18, 3.0),
        Vec3::new(-1e18, -1e18, 3.0),
    ];
    let _ = r.textured_quad(huge, &tex, &cam); // must not panic / write OOB
    assert!(r
        .raster
        .pixels()
        .iter()
        .all(|&px| px == (0, 0, 0) || px == (50, 60, 70)));
}

#[test]
fn non_finite_corners_are_refused() {
    let cam = Camera::default();
    let tex = solid(4, 4, (1, 1, 1));
    let mut r = Rasterizer::new(80, 60);
    r.clear((0, 0, 0));
    let nan = [
        Vec3::new(f32::NAN, 1.0, 3.0),
        Vec3::new(1.0, 1.0, 3.0),
        Vec3::new(1.0, -1.0, 3.0),
        Vec3::new(-1.0, -1.0, 3.0),
    ];
    assert!(!r.textured_quad(nan, &tex, &cam));
    let inf = [
        Vec3::new(f32::INFINITY, 1.0, 3.0),
        Vec3::new(1.0, 1.0, 3.0),
        Vec3::new(1.0, -1.0, 3.0),
        Vec3::new(-1.0, -1.0, 3.0),
    ];
    assert!(!r.textured_quad(inf, &tex, &cam));
    assert!(r.raster.pixels().iter().all(|&px| px == (0, 0, 0)));
}

#[test]
fn textured_quad_shares_the_depth_buffer_with_solid_triangles() {
    let cam = Camera::default();
    let tex = solid(4, 4, (200, 0, 200));
    let mut r = Rasterizer::new(120, 80);
    r.clear((0, 0, 0));
    // A solid triangle behind (z=5), then a textured quad in front (z=2) over the centre.
    r.draw_triangle(
        [
            Vec3::new(-2.0, 2.0, 5.0),
            Vec3::new(2.0, 2.0, 5.0),
            Vec3::new(0.0, -2.0, 5.0),
        ],
        &cam,
        Material {
            color: (0, 255, 0),
            ..Default::default()
        },
    );
    assert!(r.textured_quad(frontal(2.0, 1.0, 0.7), &tex, &cam));
    assert_eq!(
        r.raster.get(60, 40).unwrap(),
        (200, 0, 200),
        "the nearer textured quad occludes the farther solid triangle"
    );
}

/// The facade flagship's contract: wireframe edges and a textured facade share one
/// z-buffer, so an edge in front of the facade draws over it and an edge behind it is
/// hidden by it, independent of draw order.
#[test]
fn wireframe_lines_and_textured_facades_occlude_each_other_both_ways() {
    let cam = Camera::default();
    let texture = solid(8, 8, (200, 40, 40));
    let facade = frontal(6.0, 1.5, 1.5);
    let near_edge = (Vec3::new(-2.0, 0.0, 4.0), Vec3::new(2.0, 0.0, 4.0));
    let far_edge = (Vec3::new(-2.0, 0.3, 9.0), Vec3::new(2.0, 0.3, 9.0));
    let line_colour = (10, 230, 230);
    for facade_first in [true, false] {
        let mut r = Rasterizer::new(64, 64);
        r.clear((0, 0, 0));
        let draw_facade = |r: &mut Rasterizer| assert!(r.textured_quad(facade, &texture, &cam));
        if facade_first {
            draw_facade(&mut r);
        }
        r.line(near_edge.0, near_edge.1, &cam, line_colour);
        r.line(far_edge.0, far_edge.1, &cam, line_colour);
        if !facade_first {
            draw_facade(&mut r);
        }
        // Screen positions of the two edges at the facade's centre column.
        let (cx, near_y, _) = cam.project(Vec3::new(0.0, 0.0, 4.0), 64, 64).unwrap();
        let (_, far_y, _) = cam.project(Vec3::new(0.0, 0.3, 9.0), 64, 64).unwrap();
        let near_px = r.raster.get(cx as i32, near_y as i32).unwrap();
        let far_px = r.raster.get(cx as i32, far_y as i32).unwrap();
        assert_ne!(
            near_px,
            (0, 0, 0),
            "near edge painted (facade_first={facade_first})"
        );
        assert!(
            near_px.1 > near_px.0,
            "near edge is over the facade: {near_px:?}"
        );
        assert!(
            far_px.0 > far_px.1,
            "far edge hidden by the facade: {far_px:?}"
        );
    }
}

// ---- Consumer acceptance cases (docs/FACADE_SIGNAL_ACCEPTANCE.md) ----

fn drawn(r: &Rasterizer, background: Rgb) -> Vec<(i32, i32)> {
    let w = r.raster.width() as i32;
    r.raster
        .pixels()
        .iter()
        .enumerate()
        .filter(|(_, p)| **p != background)
        .map(|(i, _)| (i as i32 % w, i as i32 / w))
        .collect()
}

/// Case 1: the quad lands where `Camera::project` says its corners are.
#[test]
fn case1_vertex_projection_matches_camera_project() {
    let cam = Camera::default();
    let corners = [
        Vec3::new(-1.7, 1.1, 6.0),
        Vec3::new(1.3, 1.4, 6.5),
        Vec3::new(1.5, -1.2, 7.0),
        Vec3::new(-1.4, -0.9, 6.2),
    ];
    let mut r = Rasterizer::new(96, 96);
    r.clear((0, 0, 0));
    assert!(r.textured_quad(corners, &solid(4, 4, (255, 255, 255)), &cam));
    let px = drawn(&r, (0, 0, 0));
    let (min_x, max_x) = (
        px.iter().map(|p| p.0).min().unwrap(),
        px.iter().map(|p| p.0).max().unwrap(),
    );
    let (min_y, max_y) = (
        px.iter().map(|p| p.1).min().unwrap(),
        px.iter().map(|p| p.1).max().unwrap(),
    );
    let proj: Vec<_> = corners
        .iter()
        .map(|c| cam.project(*c, 96, 96).unwrap())
        .collect();
    let f = |sel: fn(&(f32, f32, f32)) -> f32, max: bool| {
        proj.iter()
            .map(sel)
            .fold(if max { f32::MIN } else { f32::MAX }, |a, b| {
                if max {
                    a.max(b)
                } else {
                    a.min(b)
                }
            })
    };
    // Pixel centres are covered, so a drawn extent lies within one pixel of the projection.
    assert!((min_x as f32 - f(|p| p.0, false)).abs() <= 1.0, "left");
    assert!(
        (max_x as f32 + 1.0 - f(|p| p.0, true)).abs() <= 1.0,
        "right"
    );
    assert!((min_y as f32 - f(|p| p.1, false)).abs() <= 1.0, "top");
    assert!(
        (max_y as f32 + 1.0 - f(|p| p.1, true)).abs() <= 1.0,
        "bottom"
    );
}

/// Case 2: on a quad yawed 60°, the world-space centre samples uv (0.5, 0.5) within one
/// texel, where screen-space affine interpolation misses by more than 0.05.
#[test]
fn case2_oblique_quad_is_perspective_correct_and_affine_would_miss() {
    let cam = Camera::default();
    let (n, size) = (64u16, 128u16);
    let mut tex = RgbRaster::new(n, n);
    for y in 0..n {
        for x in 0..n {
            tex.set(x as i32, y as i32, ((x * 4) as u8, (y * 4) as u8, 64));
        }
    }
    let yaw = 60f32.to_radians();
    let centre = Vec3::new(0.0, 0.0, 5.0);
    let ax = Vec3::new(yaw.cos(), 0.0, yaw.sin()).scale(2.0);
    let up = Vec3::new(0.0, 1.5, 0.0);
    let corners = [
        centre.minus(ax).plus(up),
        centre.plus(ax).plus(up),
        centre.plus(ax).minus(up),
        centre.minus(ax).minus(up),
    ];
    let mut r = Rasterizer::new(size, size);
    r.clear((0, 0, 0));
    assert!(r.textured_quad(corners, &tex, &cam));
    let (cx, cy, _) = cam.project(centre, size, size).unwrap();
    let (px, py) = (cx.floor() as i32, cy.floor() as i32);
    let c = r.raster.get(px, py).unwrap();
    let (tx, ty) = (i32::from(c.0 / 4), i32::from(c.1 / 4));
    assert!(
        (tx - 32).abs() <= 1 && (ty - 32).abs() <= 1,
        "sampled texel ({tx},{ty})"
    );

    // Affine control: screen-space barycentrics of the same pixel centre.
    let p: Vec<_> = corners
        .iter()
        .map(|c| cam.project(*c, size, size).unwrap())
        .collect();
    let uv = [[0.0f32, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let (qx, qy) = (px as f32 + 0.5, py as f32 + 0.5);
    let edge =
        |a: usize, b: usize| (p[b].0 - p[a].0) * (qy - p[a].1) - (p[b].1 - p[a].1) * (qx - p[a].0);
    let mut affine = None;
    for [i, j, k] in [[0usize, 1, 2], [0, 2, 3]] {
        let area = (p[j].0 - p[i].0) * (p[k].1 - p[i].1) - (p[j].1 - p[i].1) * (p[k].0 - p[i].0);
        let (a, b) = (edge(j, k) / area, edge(k, i) / area);
        let c = 1.0 - a - b;
        if a >= 0.0 && b >= 0.0 && c >= 0.0 {
            affine = Some([0, 1].map(|d| a * uv[i][d] + b * uv[j][d] + c * uv[k][d]));
        }
    }
    let affine = affine.expect("centre pixel lies inside the projected quad");
    assert!(
        (affine[0] - 0.5).abs() > 0.05 || (affine[1] - 0.5).abs() > 0.05,
        "control: affine uv {affine:?} would be indistinguishable"
    );
}

/// Case 3: a partly off-screen quad draws no more than its clipped analytic area plus
/// perimeter, and no less than area minus perimeter.
#[test]
fn case3_clipped_coverage_matches_analytic_area() {
    let cam = Camera::default();
    let (w, h) = (80u16, 60u16);
    let corners = [
        Vec3::new(-6.0, 1.5, 6.0),
        Vec3::new(1.0, 1.5, 6.0),
        Vec3::new(1.0, -4.0, 6.0),
        Vec3::new(-6.0, -4.0, 6.0),
    ];
    let mut r = Rasterizer::new(w, h);
    r.clear((0, 0, 0));
    r.textured_quad(corners, &solid(2, 2, (9, 9, 9)), &cam);
    let count = drawn(&r, (0, 0, 0)).len() as f32;
    // Frontal quad: its projection is an axis-aligned rectangle, linear in world x and y at
    // fixed depth. Corners outside the frustum have no `project`, so derive the mapping from
    // in-frustum points, extend it to the corners, then clip to the screen.
    let o = cam.project(Vec3::new(0.0, 0.0, 6.0), w, h).unwrap();
    let ux = cam.project(Vec3::new(1.0, 0.0, 6.0), w, h).unwrap().0 - o.0;
    let uy = cam.project(Vec3::new(0.0, 1.0, 6.0), w, h).unwrap().1 - o.1;
    let sx: Vec<f32> = corners.iter().map(|c| o.0 + c.x * ux).collect();
    let sy: Vec<f32> = corners.iter().map(|c| o.1 + c.y * uy).collect();
    let x0 = sx.iter().copied().fold(f32::MAX, f32::min).max(0.0);
    let x1 = sx.iter().copied().fold(f32::MIN, f32::max).min(w as f32);
    let y0 = sy.iter().copied().fold(f32::MAX, f32::min).max(0.0);
    let y1 = sy.iter().copied().fold(f32::MIN, f32::max).min(h as f32);
    let (area, perimeter) = ((x1 - x0) * (y1 - y0), 2.0 * ((x1 - x0) + (y1 - y0)));
    assert!(
        x0 == 0.0 && y1 == h as f32,
        "the quad really is partly off-screen"
    );
    assert!(count <= area + perimeter, "{count} > {area} + {perimeter}");
    assert!(count >= area - perimeter, "{count} < {area} - {perimeter}");
}

/// Case 4: a huge texture costs nothing proportional to its size.
#[test]
fn case4_huge_texture_work_is_bounded_by_the_destination() {
    let cam = Camera::default();
    let huge = solid(2048, 2048, (1, 2, 3));
    let mut r = Rasterizer::new(40, 30);
    r.clear((0, 0, 0));
    assert!(r.textured_quad(frontal(5.0, 1.0, 1.0), &huge, &cam));
    assert!(
        r.stats.z_tests <= 2 * 40 * 30,
        "z_tests {}",
        r.stats.z_tests
    );
}

/// Case 7: a quad covering the whole screen stays within the documented bound.
#[test]
fn case7_screen_covering_quad_is_bounded() {
    let cam = Camera::default();
    let (w, h) = (64u16, 48u16);
    let mut r = Rasterizer::new(w, h);
    r.clear((0, 0, 0));
    assert!(r.textured_quad(frontal(0.5, 50.0, 50.0), &solid(8, 8, (200, 1, 1)), &cam));
    let touched = drawn(&r, (0, 0, 0)).len() as u64;
    assert_eq!(touched, u64::from(w) * u64::from(h), "fills the screen");
    assert!(
        r.stats.z_tests <= 2 * u64::from(w) * u64::from(h),
        "z_tests {}",
        r.stats.z_tests
    );
}
