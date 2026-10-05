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
