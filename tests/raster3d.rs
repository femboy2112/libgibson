use gibson::geom::{Transform3, Vec3};
use gibson::raster::{RgbRaster, MAX_RASTER_DIMENSION};
use gibson::raster3d::{Camera, Fog, Material, Rasterizer, TriangleMesh};

fn material(color: (u8, u8, u8)) -> Material {
    Material {
        color,
        ambient: 1.,
        diffuse: 0.,
        emissive: 0.,
    }
}
fn triangle(z: f32) -> [Vec3; 3] {
    [
        Vec3::new(-1., -1., z),
        Vec3::new(1., -1., z),
        Vec3::new(0., 1., z),
    ]
}

#[test]
fn rgb_software_blend_clipping_and_opaque_halfblocks() {
    let mut r = RgbRaster::new(3, 3);
    r.clear((10, 20, 30));
    r.blend(1, 0, (110, 120, 130), 0.5);
    assert_eq!(r.get(1, 0), Some((60, 70, 80)));
    r.blend(1, 0, (255, 255, 255), f32::NAN);
    assert_eq!(r.get(1, 0), Some((60, 70, 80)));
    r.set(i32::MAX, i32::MIN, (255, 0, 0));
    assert_eq!(r.get(-1, 0), None);
    r.line(i32::MIN, 1, i32::MAX, 1, (1, 2, 3));
    assert_eq!(r.get(0, 1), Some((1, 2, 3)));
    assert_eq!(r.get(2, 1), Some((1, 2, 3)));
    let s = r.to_surface();
    assert_eq!((s.width, s.height), (3, 2));
    assert_eq!(
        s.get(1, 0).unwrap().style.fg,
        Some(gibson::Color::Rgb(60, 70, 80))
    );
    assert_eq!(
        s.get(1, 0).unwrap().style.bg,
        Some(gibson::Color::Rgb(1, 2, 3))
    );
    assert_eq!(
        s.get(1, 1).unwrap().style.bg,
        Some(gibson::Color::Rgb(0, 0, 0))
    );
    assert!(s
        .cells
        .iter()
        .all(|c| !c.transparent && c.glyph.display_width == 1));
    let mut ppm = Vec::new();
    r.write_ppm(&mut ppm).unwrap();
    assert!(ppm.starts_with(b"P6\n3 3\n255\n"));
}
#[test]
fn raster_allocation_and_empty_shapes_are_bounded() {
    let r = RgbRaster::new(u16::MAX, 1);
    assert_eq!(r.width(), MAX_RASTER_DIMENSION);
    let mut empty = RgbRaster::new(0, 0);
    empty.line(i32::MIN, i32::MIN, i32::MAX, i32::MAX, (255, 0, 0));
    empty.disc(0., 0., f32::MAX, (255, 0, 0));
    assert!(empty.pixels().is_empty());
    let mut r = RgbRaster::new(8, 8);
    r.disc(4., 4., f32::MAX, (1, 2, 3));
    assert!(r.pixels().iter().all(|&c| c == (1, 2, 3)));
}
#[test]
fn monochrome_preserves_density_without_color() {
    let mut r = RgbRaster::new(3, 2);
    for y in 0..2 {
        r.set(1, y, (128, 128, 128));
        r.set(2, y, (255, 255, 255));
    }
    let s = r.to_mono_surface();
    let dots = |x| {
        s.get(x, 0)
            .unwrap()
            .glyph
            .grapheme
            .as_str()
            .chars()
            .next()
            .unwrap() as u32
            - 0x2800
    };
    assert_eq!(dots(0), 0);
    assert!(dots(1).count_ones() > 0 && dots(1).count_ones() < 8);
    assert_eq!(dots(2), 255);
    assert!(s
        .cells
        .iter()
        .all(|c| c.style.fg.is_none() && c.style.bg.is_none()));
}
#[test]
fn real_occlusion_is_independent_of_submission_order() {
    let camera = Camera::default();
    let mut a = Rasterizer::new(64, 64);
    let mut b = a.clone();
    a.draw_triangle(triangle(4.), &camera, material((0, 0, 255)));
    a.draw_triangle(triangle(2.), &camera, material((255, 0, 0)));
    b.draw_triangle(triangle(2.), &camera, material((255, 0, 0)));
    b.draw_triangle(triangle(4.), &camera, material((0, 0, 255)));
    assert_eq!(a.raster, b.raster);
    assert_eq!(a.raster.get(32, 32), Some((255, 0, 0)));
    assert!((a.depth(32, 32).unwrap() - 2.).abs() < 1e-5);
}
#[test]
fn perspective_depth_matches_independent_ray_plane_intersection() {
    let camera = Camera::default();
    let v = [
        Vec3::new(-1., -1., 2.),
        Vec3::new(2., -2., 4.),
        Vec3::new(0., 4., 8.),
    ];
    let mut r = Rasterizer::new(64, 64);
    r.draw_triangle(v, &camera, material((50, 200, 255)));
    let tangent = (camera.fov_y * 0.5).tan();
    let ray = Vec3::new((32.5 / 32. - 1.) * tangent, (1. - 32.5 / 32.) * tangent, 1.);
    let normal = v[1].minus(v[0]).cross(v[2].minus(v[0]));
    let expected = normal.dot(v[0]) / normal.dot(ray);
    assert!((r.depth(32, 32).unwrap() - expected).abs() < 1e-5);
}
#[test]
fn near_plane_crossing_clips_instead_of_dropping_triangle() {
    let camera = Camera {
        near: 1.,
        ..Camera::default()
    };
    let mut r = Rasterizer::new(40, 30);
    r.draw_triangle(
        [
            Vec3::new(-0.5, -0.5, 0.5),
            Vec3::new(1., -0.5, 2.),
            Vec3::new(0., 1., 2.),
        ],
        &camera,
        material((255, 255, 255)),
    );
    assert_eq!(r.stats.triangles_drawn, 1);
    assert!(r.stats.z_tests > 0);
    for y in 0..30 {
        for x in 0..40 {
            let z = r.depth(x, y).unwrap();
            assert!(z >= 1.);
        }
    }
    r.clear((0, 0, 0));
    r.draw_triangle(triangle(0.1), &camera, material((255, 255, 255)));
    assert_eq!(r.stats.z_tests, 0);
}
#[test]
fn viewport_and_far_plane_clip_and_work_stays_bounded() {
    let camera = Camera {
        far: 5.,
        ..Camera::default()
    };
    let mut r = Rasterizer::new(32, 24);
    r.draw_triangle(
        [
            Vec3::new(-1e30, -1e30, 2.),
            Vec3::new(1e30, -1e30, 2.),
            Vec3::new(0., 1e30, 2.),
        ],
        &camera,
        material((255, 0, 0)),
    );
    assert!(r.stats.z_tests <= 32 * 24 * 7);
    assert!(r.raster.pixels().iter().any(|&p| p != (0, 0, 0)));
    r.clear((0, 0, 0));
    r.draw_triangle(triangle(10.), &camera, material((255, 0, 0)));
    assert_eq!(r.stats.z_tests, 0);
    r.draw_triangle(
        [
            Vec3::new(100., 0., 2.),
            Vec3::new(101., 0., 2.),
            Vec3::new(100., 1., 2.),
        ],
        &camera,
        material((255, 0, 0)),
    );
    assert_eq!(r.stats.z_tests, 0);
}
#[test]
fn invalid_coordinates_degenerate_triangles_and_camera_are_rejected() {
    let camera = Camera::default();
    let mut r = Rasterizer::new(20, 20);
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut v = triangle(2.);
        v[0].x = invalid;
        r.draw_triangle(v, &camera, Material::default());
    }
    r.draw_triangle([Vec3::new(0., 0., 2.); 3], &camera, Material::default());
    r.draw_triangle(
        triangle(2.),
        &Camera { near: 0., ..camera },
        Material::default(),
    );
    r.draw_triangle(
        triangle(2.),
        &camera,
        Material {
            ambient: f32::NAN,
            ..Material::default()
        },
    );
    assert_eq!(r.stats.z_tests, 0);
    assert!(r.raster.pixels().iter().all(|&c| c == (0, 0, 0)));
}
#[test]
fn camera_look_at_and_transform_agree_with_projection() {
    let camera = Camera {
        position: Vec3::new(4., 2., -5.),
        target: Vec3::new(0., 0., 2.),
        ..Camera::default()
    };
    let projected = camera.project(camera.target, 80, 40).unwrap();
    assert!((projected.0 - 40.).abs() < 1e-4 && (projected.1 - 20.).abs() < 1e-4);
    let mut r = Rasterizer::new(80, 40);
    r.draw_mesh(
        &TriangleMesh::cube(2.),
        Transform3 {
            offset: camera.target,
            ..Transform3::default()
        },
        &camera,
        Material::default(),
    );
    assert!(r.depth(40, 20).unwrap().is_finite());
}
#[test]
fn flat_light_and_fog_provide_volume_and_depth() {
    let camera = Camera::default();
    let mut lit = Rasterizer::new(60, 60);
    lit.light = Vec3::new(0., 0., 1.);
    lit.draw_triangle(
        triangle(2.),
        &camera,
        Material {
            color: (200, 100, 50),
            ambient: 0.2,
            diffuse: 0.8,
            emissive: 0.,
        },
    );
    assert_eq!(lit.raster.get(30, 30), Some((200, 100, 50)));
    lit.clear((0, 0, 0));
    lit.light = Vec3::new(0., 0., -1.);
    lit.draw_triangle(
        triangle(2.),
        &camera,
        Material {
            color: (200, 100, 50),
            ambient: 0.2,
            diffuse: 0.8,
            emissive: 0.,
        },
    );
    assert_eq!(lit.raster.get(30, 30), Some((40, 20, 10)));
    lit.clear((0, 0, 0));
    lit.fog = Some(Fog {
        color: (0, 0, 100),
        start: 1.,
        end: 3.,
    });
    lit.draw_triangle(triangle(2.), &camera, material((200, 100, 0)));
    assert_eq!(lit.raster.get(30, 30), Some((100, 50, 50)));
}
#[test]
fn filled_constructors_and_backface_culling_show_nearest_surface() {
    let camera = Camera::default();
    for mesh in [
        TriangleMesh::cube(2.),
        TriangleMesh::box_xyz(2., 3., 2.),
        TriangleMesh::octahedron(1.5),
    ] {
        let mut r = Rasterizer::new(64, 64);
        r.cull_backfaces = true;
        r.draw_mesh(
            &mesh,
            Transform3 {
                offset: Vec3::new(0., 0., 4.),
                ..Transform3::default()
            },
            &camera,
            Material::default(),
        );
        assert!(r.depth(32, 32).unwrap() < 4.);
        assert!(r.stats.triangles_drawn > 0);
        assert!(r.stats.triangles_drawn < r.stats.triangles_submitted);
    }
}
#[test]
fn depth_tested_routes_hide_behind_geometry_and_clear_resets_depth() {
    let camera = Camera::default();
    let mut r = Rasterizer::new(64, 64);
    r.draw_triangle(triangle(2.), &camera, material((200, 0, 0)));
    r.line(
        Vec3::new(-1., 0., 4.),
        Vec3::new(1., 0., 4.),
        &camera,
        (0, 255, 255),
    );
    assert_eq!(r.raster.get(32, 32), Some((200, 0, 0)));
    r.line(
        Vec3::new(-1., 0., 1.),
        Vec3::new(1., 0., 1.),
        &camera,
        (0, 255, 255),
    );
    assert_eq!(r.raster.get(32, 32), Some((0, 255, 255)));
    r.clear((1, 2, 3));
    assert_eq!(r.depth(32, 32), Some(f32::INFINITY));
    assert_eq!(r.stats.z_tests, 0);
    assert_eq!(r.raster.get(32, 32), Some((1, 2, 3)));
}
#[test]
fn identical_frames_have_exact_surface_identity_and_color_diversity() {
    let render = || {
        let mut r = Rasterizer::new(120, 64);
        r.fog = Some(Fog {
            color: (3, 5, 14),
            start: 3.,
            end: 9.,
        });
        r.draw_mesh(
            &TriangleMesh::cube(3.),
            Transform3 {
                rx: 0.4,
                ry: 0.7,
                offset: Vec3::new(0., 0., 5.),
                ..Transform3::default()
            },
            &Camera::default(),
            Material::default(),
        );
        r
    };
    let a = render();
    let b = render();
    assert_eq!(a.raster.to_surface(), b.raster.to_surface());
    let colors: std::collections::BTreeSet<_> = a.raster.pixels().iter().copied().collect();
    assert!(colors.len() > 20, "fogged shaded volume requires gradients");
}

#[test]
fn raster_replacement_resets_depth_even_with_equal_pixel_count() {
    let camera = Camera::default();
    let mut r = Rasterizer::new(32, 32);
    r.draw_triangle(triangle(1.), &camera, material((255, 0, 0)));
    r.raster = RgbRaster::new(64, 16);
    assert_eq!(r.depth(0, 0), None);
    r.draw_triangle(triangle(4.), &camera, material((0, 255, 0)));
    assert_eq!(r.raster.get(32, 8), Some((0, 255, 0)));
    r.raster = RgbRaster::new(80, 80);
    r.clear((0, 0, 0));
    assert_eq!(r.depth(79, 79), Some(f32::INFINITY));
}

#[test]
fn hostile_finite_segments_never_escape_pixel_work_budget() {
    let camera = Camera::default();
    let mut r = Rasterizer::new(32, 24);
    for value in [f32::MAX, 1e30, 1e10] {
        r.line(
            Vec3::new(-value, 0., 2.),
            Vec3::new(value, 0., 2.),
            &camera,
            (255, 255, 255),
        );
        r.line(
            Vec3::new(value, value, 0.00001),
            Vec3::new(0., 0., 3.),
            &camera,
            (255, 255, 255),
        );
    }
    assert!(r.stats.z_tests <= 6 * 33);
}
