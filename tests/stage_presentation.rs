use gibson::raster::{Rgb, RgbRaster, RgbaRaster};
use gibson::stage::*;
use gibson::ui::experience::FrameDemand;
use gibson::ui::prelude::*;
use gibson::{Context, Node, Rect, RenderMode, Surface, TextInputState};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    Edit(String),
    Go,
}
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn key(s: &str) -> Key {
    Key::named(s)
}
fn limits() -> RasterLimits {
    RasterLimits {
        planes: 16,
        nodes: 256,
        pixels: 100_000,
    }
}
fn options() -> PresentationOptions {
    PresentationOptions {
        transition: ms(200),
        cadence: ms(25),
    }
}
fn source(k: &str, demand: FrameDemand) -> SourceDemand {
    SourceDemand {
        key: key(k),
        demand,
    }
}
struct Fixture {
    ui: UiRuntime<Msg>,
    image: Arc<RgbaRaster>,
    size: (u16, u16),
    rgb: Rgb,
    reading: String,
    paints: Arc<AtomicUsize>,
    right: bool,
    swap: bool,
}
impl Fixture {
    fn new() -> Self {
        let mut image = RgbaRaster::new(12, 12);
        image.pixels_mut().fill([240, 0, 0, 255]);
        image.set(0, 0, [255, 255, 255, 255]);
        image.set(1, 1, [200, 100, 0, 128]);
        Self {
            ui: UiRuntime::new(skins::SWISS_SIGNAL),
            image: Arc::new(image),
            size: (40, 16),
            rgb: (10, 20, 30),
            reading: "reading".into(),
            paints: Arc::new(AtomicUsize::new(0)),
            right: false,
            swap: false,
        }
    }
    fn compile(&mut self, now: Duration, geometry: (u16, u16, u16)) -> Compiled<Msg> {
        let (x, w, h) = geometry;
        let before = if self.right {
            spacer().grow(1.)
        } else {
            spacer().width(x).grow(0.)
        };
        let art = column().key("art").width(w).height(h).padding(0).gap(0);
        let sibling = if self.right {
            spacer().key("sibling").width(0).grow(0.)
        } else {
            spacer().key("sibling").grow(1.)
        };
        let children = if self.swap {
            vec![art, sibling, before]
        } else {
            vec![before, art, sibling]
        };
        let count = Arc::clone(&self.paints);
        let tree = column()
            .key("root")
            .width(self.size.0)
            .height(self.size.1)
            .padding(0)
            .gap(0)
            .child(row().height(4).padding(0).gap(0).children(children))
            .child(label(self.reading.clone()).key("reading"))
            .child(
                text_input(&TextInputState::with_text("draft"))
                    .key("editor")
                    .on_edit(|s| Msg::Edit(s.text)),
            )
            .child(button("Go").key("go").on_press(Msg::Go))
            .child(
                raw(Node::canvas(move |r| {
                    count.fetch_add(1, Ordering::SeqCst);
                    Surface::new(r.width, r.height)
                })
                .height(1.))
                .key("paint-probe"),
            );
        self.ui
            .frame(
                &tree,
                UiEnvironment {
                    width: self.size.0,
                    height: self.size.1,
                    motion: MotionPreference::None,
                    ..Default::default()
                },
                now,
            )
            .unwrap()
    }
    fn resolve(
        &mut self,
        driver: &mut PresentationDriver,
        now: Duration,
        geometry: (u16, u16, u16),
        demands: &[SourceDemand],
        motion: MotionPreference,
    ) -> Result<StageFrame<Msg>, StageError> {
        let compiled = self.compile(now, geometry);
        let focus = self.ui.focus().cloned();
        let mut underlay = RgbRaster::new(self.size.0, 2 * self.size.1);
        underlay.clear(self.rgb);
        let plane = RasterPlane {
            anchor: RasterAnchor::Key(key("art")),
            source: Arc::clone(&self.image),
            offset_px: (0, 0),
            z: 2,
        };
        driver.resolve_frame(
            now,
            FrameParts {
                compiled,
                viewport: self.size,
                underlay: &underlay,
                planes: &[plane],
                focus: focus.as_ref(),
                demands,
                omissions: &[],
                motion,
            },
            limits(),
        )
    }
}

#[test]
fn retarget_samples_current_rect_and_pixels_before_reversing() {
    let mut f = Fixture::new();
    let mut d = PresentationDriver::new(options()).unwrap();
    f.resolve(&mut d, ms(0), (0, 8, 4), &[], MotionPreference::Full)
        .unwrap();
    let start = f
        .resolve(&mut d, ms(0), (8, 4, 2), &[], MotionPreference::Full)
        .unwrap();
    assert_eq!(
        start.snapshot.planes[0].presented,
        PixelRect {
            x: 0,
            y: 0,
            width: 8,
            height: 8
        }
    );
    assert_eq!(
        start.snapshot.keys[&key("sibling")].layout_rect.unwrap().x,
        12,
        "new flow reserves room immediately"
    );
    let before = f
        .resolve(&mut d, ms(100), (8, 4, 2), &[], MotionPreference::Full)
        .unwrap();
    let mid = PixelRect {
        x: 4,
        y: 0,
        width: 6,
        height: 6,
    };
    assert_eq!(before.snapshot.planes[0].presented, mid);
    assert_eq!(before.snapshot.pixels.get(4, 0), Some((255, 255, 255)));
    let after = f
        .resolve(&mut d, ms(100), (2, 2, 1), &[], MotionPreference::Full)
        .unwrap();
    assert_eq!(
        after.snapshot.pixels.get(4, 0),
        before.snapshot.pixels.get(4, 0),
        "retarget preserves the actually presented marker pixel"
    );
    assert_eq!(
        after.snapshot.planes[0].presented, mid,
        "all four current rect coordinates survive retarget"
    );
    assert_eq!(
        after.snapshot.keys[&key("sibling")].layout_rect.unwrap().x,
        4
    );
    let end = f
        .resolve(&mut d, ms(300), (2, 2, 1), &[], MotionPreference::Full)
        .unwrap();
    assert_eq!(
        end.snapshot.planes[0].presented,
        PixelRect {
            x: 2,
            y: 0,
            width: 2,
            height: 2
        }
    );
    assert_eq!(end.snapshot.next_deadline, None);
}

#[test]
fn deadline_one_hundred_survives_an_unrelated_event_at_ninety() {
    let mut f = Fixture::new();
    let mut d = PresentationDriver::new(options()).unwrap();
    let demands = [
        source("art", FrameDemand::After(ms(100))),
        source("reading", FrameDemand::OnChange),
    ];
    f.resolve(&mut d, ms(0), (0, 8, 4), &demands, MotionPreference::Full)
        .unwrap();
    assert_eq!(d.next_deadline(), Some(ms(100)));
    let demands = [
        source("art", FrameDemand::After(ms(100))),
        source("reading", FrameDemand::After(ms(200))),
    ];
    let unrelated = f
        .resolve(&mut d, ms(90), (0, 8, 4), &demands, MotionPreference::Full)
        .unwrap();
    assert_eq!(
        unrelated.snapshot.next_deadline,
        Some(ms(100)),
        "re-observation cannot renew the original absolute request"
    );
    let demands = [
        source("art", FrameDemand::OnChange),
        source("reading", FrameDemand::After(ms(200))),
    ];
    let due = f
        .resolve(&mut d, ms(100), (0, 8, 4), &demands, MotionPreference::Full)
        .unwrap();
    assert_eq!(due.snapshot.due_sources, [key("art")]);
    assert_eq!(due.snapshot.next_deadline, Some(ms(290)));
    let settled = [
        source("art", FrameDemand::OnChange),
        source("reading", FrameDemand::OnChange),
    ];
    assert_eq!(
        f.resolve(&mut d, ms(150), (0, 8, 4), &settled, MotionPreference::Full)
            .unwrap()
            .snapshot
            .next_deadline,
        None
    );
}

#[test]
fn late_demands_coalesce_once_and_roster_removal_retires_them() {
    let mut f = Fixture::new();
    let mut d = PresentationDriver::new(options()).unwrap();
    f.resolve(
        &mut d,
        ms(0),
        (0, 8, 4),
        &[source("art", FrameDemand::After(ms(100)))],
        MotionPreference::Full,
    )
    .unwrap();
    let late = f
        .resolve(
            &mut d,
            ms(500),
            (0, 8, 4),
            &[source("art", FrameDemand::After(ms(50)))],
            MotionPreference::Full,
        )
        .unwrap();
    assert_eq!(late.snapshot.due_sources, [key("art")]);
    assert_eq!(late.snapshot.next_deadline, Some(ms(550)));
    assert_eq!(
        f.resolve(&mut d, ms(600), (0, 8, 4), &[], MotionPreference::Full)
            .unwrap()
            .snapshot
            .next_deadline,
        None
    );
}

#[test]
fn strict_reduced_and_none_snap_without_future_animation_but_keep_external_timers() {
    let mut f = Fixture::new();
    let mut d = PresentationDriver::new(options()).unwrap();
    f.resolve(&mut d, ms(0), (0, 8, 4), &[], MotionPreference::Full)
        .unwrap();
    assert!(f
        .resolve(&mut d, ms(0), (8, 8, 4), &[], MotionPreference::Full)
        .unwrap()
        .snapshot
        .next_deadline
        .is_some());
    let reduced = f
        .resolve(&mut d, ms(50), (8, 8, 4), &[], MotionPreference::Reduced)
        .unwrap();
    assert_eq!(reduced.snapshot.planes[0].presented.x, 8);
    assert_eq!(reduced.snapshot.next_deadline, None);
    assert_eq!(
        f.ui.active_animation_count(),
        0,
        "host normalized UI motion to None"
    );
    let none = f
        .resolve(&mut d, ms(60), (2, 4, 2), &[], MotionPreference::None)
        .unwrap();
    assert_eq!(
        none.snapshot.planes[0].presented,
        PixelRect {
            x: 2,
            y: 0,
            width: 4,
            height: 4
        }
    );
    assert_eq!(none.snapshot.next_deadline, None);
    let timed = f
        .resolve(
            &mut d,
            ms(70),
            (2, 4, 2),
            &[source("reading", FrameDemand::After(ms(100)))],
            MotionPreference::Reduced,
        )
        .unwrap();
    assert_eq!(
        timed.snapshot.next_deadline,
        Some(ms(170)),
        "explicit informational timer is not silently suppressed"
    );
}

#[test]
fn settled_scene_has_no_demand_generation_or_output_even_when_context_is_due() {
    let mut f = Fixture::new();
    let mut d = PresentationDriver::new(options()).unwrap();
    let mut ctx = Context::headless(RenderMode::Fullscreen, f.size.0, f.size.1);
    ctx.scheduler.set_max_fps(u32::MAX);
    let first = f
        .resolve(&mut d, ms(0), (0, 8, 4), &[], MotionPreference::Full)
        .unwrap();
    ctx.set_root(first.to_node());
    ctx.render_now().unwrap();
    assert!(!ctx.take_rendered_bytes().is_empty());
    f.resolve(&mut d, ms(0), (8, 8, 4), &[], MotionPreference::Full)
        .unwrap();
    let mut settled = f
        .resolve(&mut d, ms(200), (8, 8, 4), &[], MotionPreference::Full)
        .unwrap();
    assert_eq!(
        d.next_deadline(),
        None,
        "settling must remove the animation pulse"
    );
    ctx.set_root(settled.to_node());
    ctx.render_now().unwrap();
    settled.snapshot.frame_report = Some(ctx.last_frame_report());
    ctx.take_rendered_bytes();
    let paints = f.paints.load(Ordering::SeqCst);
    assert!(paints > 0);
    for now in [ms(250), ms(1000), ms(5000)] {
        assert!(
            ctx.scheduler.frame_due(),
            "negative output assertion is not a throttle artifact"
        );
        if d.next_deadline().is_some_and(|at| at <= now) {
            let frame = f
                .resolve(&mut d, now, (8, 8, 4), &[], MotionPreference::Full)
                .unwrap();
            ctx.set_root(frame.to_node());
        }
        assert!(!ctx.render_if_due().unwrap());
        assert!(ctx.take_rendered_bytes().is_empty());
        assert_eq!(
            f.paints.load(Ordering::SeqCst),
            paints,
            "idle must not generate a new Stage/Context paint"
        );
    }
    f.reading = "changed".into();
    let changed = f
        .resolve(&mut d, ms(5000), (8, 8, 4), &[], MotionPreference::Full)
        .unwrap();
    ctx.set_root(changed.to_node());
    assert!(ctx.scheduler.frame_due());
    assert!(ctx.render_if_due().unwrap());
    assert!(
        !ctx.take_rendered_bytes().is_empty(),
        "explicit-change positive control emits bytes"
    );
}

#[test]
fn snapshot_joins_key_bounds_focus_targets_cells_and_editor_cursor() {
    let mut f = Fixture::new();
    let mut d = PresentationDriver::new(options()).unwrap();
    let frame = f
        .resolve(&mut d, ms(0), (3, 8, 4), &[], MotionPreference::Full)
        .unwrap();
    assert_eq!(frame.snapshot.focus, Some(key("editor")));
    assert!(frame.snapshot.cursor_position.is_some());
    assert!(frame.snapshot.frame_report.is_none());
    let target = frame
        .snapshot
        .targets
        .iter()
        .find(|t| t.key == key("go"))
        .unwrap();
    assert_eq!(
        target.rect,
        frame.snapshot.keys[&key("go")]
            .painted
            .as_ref()
            .unwrap()
            .visible
    );
    assert_eq!(
        frame
            .compiled
            .interactions
            .get(&key("go"))
            .unwrap()
            .on_press,
        Some(Msg::Go)
    );
    let mut node = frame.to_node();
    gibson::compute_layout(&mut node, f.size.0, f.size.1).unwrap();
    let mut replay = Surface::new(f.size.0, f.size.1);
    let paint = gibson::paint(&node, &mut replay);
    assert_eq!(
        replay, frame.snapshot.cells,
        "ordinary Context node reproduces resolved cells"
    );
    assert_eq!(
        paint.cursor_position, frame.snapshot.cursor_position,
        "surface bridge must retain the editor cursor"
    );
    let mut ctx = Context::headless(RenderMode::Fullscreen, f.size.0, f.size.1);
    ctx.set_root(frame.to_node());
    let actual = ctx.render_now().unwrap();
    assert_eq!(actual.cursor_position, frame.snapshot.cursor_position);
}

#[test]
fn resize_and_reorder_use_current_keyed_layout_not_cached_paths() {
    let mut f = Fixture::new();
    f.right = true;
    let mut d = PresentationDriver::new(options()).unwrap();
    let a = f
        .resolve(&mut d, ms(0), (0, 8, 4), &[], MotionPreference::Full)
        .unwrap();
    assert_eq!(a.snapshot.planes[0].presented.x, 32);
    f.size.0 = 80;
    let moved = f
        .resolve(&mut d, ms(0), (0, 8, 4), &[], MotionPreference::Full)
        .unwrap();
    assert_eq!(moved.snapshot.planes[0].layout_rect.x, 72);
    assert_eq!(moved.snapshot.planes[0].presented.x, 32);
    let mid = f
        .resolve(&mut d, ms(100), (0, 8, 4), &[], MotionPreference::Full)
        .unwrap();
    assert_eq!(mid.snapshot.planes[0].presented.x, 52);
    f.swap = true;
    let reordered = f
        .resolve(&mut d, ms(100), (0, 8, 4), &[], MotionPreference::Full)
        .unwrap();
    assert_eq!(reordered.snapshot.planes[0].layout_rect.x, 0);
    assert_eq!(reordered.snapshot.planes[0].presented.x, 52);
    assert_eq!(
        f.resolve(&mut d, ms(300), (0, 8, 4), &[], MotionPreference::Full)
            .unwrap()
            .snapshot
            .planes[0]
            .presented
            .x,
        0
    );
}

#[test]
fn transition_limit_zero_duration_and_late_samples_are_explicit() {
    assert!(matches!(
        PresentationDriver::new(PresentationOptions {
            transition: ms(401),
            ..options()
        }),
        Err(StageError::Options)
    ));
    assert!(matches!(
        PresentationDriver::new(PresentationOptions {
            cadence: Duration::ZERO,
            ..options()
        }),
        Err(StageError::Options)
    ));
    let mut f = Fixture::new();
    let mut d = PresentationDriver::new(PresentationOptions {
        transition: ms(400),
        ..options()
    })
    .unwrap();
    f.resolve(&mut d, ms(0), (0, 8, 4), &[], MotionPreference::Full)
        .unwrap();
    f.resolve(&mut d, ms(0), (8, 8, 4), &[], MotionPreference::Full)
        .unwrap();
    let late = f
        .resolve(&mut d, ms(1000), (8, 8, 4), &[], MotionPreference::Full)
        .unwrap();
    assert_eq!(late.snapshot.planes[0].presented.x, 8);
    assert_eq!(late.snapshot.next_deadline, None);
    let mut immediate = PresentationDriver::new(PresentationOptions {
        transition: Duration::ZERO,
        ..options()
    })
    .unwrap();
    f.resolve(
        &mut immediate,
        ms(1000),
        (0, 8, 4),
        &[],
        MotionPreference::Full,
    )
    .unwrap();
    let snap = f
        .resolve(
            &mut immediate,
            ms(1000),
            (8, 8, 4),
            &[],
            MotionPreference::Full,
        )
        .unwrap();
    assert_eq!(snap.snapshot.planes[0].presented.x, 8);
    assert_eq!(snap.snapshot.next_deadline, None);
}

#[test]
fn backward_time_and_overflow_refuse_without_publishing_driver_state() {
    let mut f = Fixture::new();
    let mut d = PresentationDriver::new(options()).unwrap();
    f.resolve(
        &mut d,
        ms(100),
        (0, 8, 4),
        &[source("art", FrameDemand::After(ms(100)))],
        MotionPreference::Full,
    )
    .unwrap();
    let count = f.paints.load(Ordering::SeqCst);
    assert!(matches!(
        f.resolve(&mut d, ms(99), (8, 8, 4), &[], MotionPreference::Full),
        Err(StageError::BackwardTime)
    ));
    assert_eq!(d.next_deadline(), Some(ms(200)));
    assert_eq!(f.paints.load(Ordering::SeqCst), count);
    let mut fresh = PresentationDriver::new(options()).unwrap();
    assert!(matches!(
        f.resolve(
            &mut fresh,
            Duration::MAX,
            (0, 8, 4),
            &[source("art", FrameDemand::After(ms(1)))],
            MotionPreference::Full
        ),
        Err(StageError::TimeOverflow)
    ));
    assert_eq!(fresh.next_deadline(), None);
}

#[test]
fn duplicate_missing_sources_and_unstable_anchor_are_refused() {
    let mut f = Fixture::new();
    let mut d = PresentationDriver::new(options()).unwrap();
    let duplicate = [
        source("art", FrameDemand::OnChange),
        source("art", FrameDemand::OnChange),
    ];
    assert!(
        matches!(f.resolve(&mut d,ms(0),(0,8,4),&duplicate,MotionPreference::Full),Err(StageError::DuplicateSource(k)) if k==key("art"))
    );
    assert!(
        matches!(f.resolve(&mut d,ms(0),(0,8,4),&[source("missing",FrameDemand::OnChange)],MotionPreference::Full),Err(StageError::Source(k)) if k==key("missing"))
    );
    let c = f.compile(ms(0), (0, 8, 4));
    let bg = RgbRaster::new(40, 32);
    let plane = RasterPlane {
        anchor: RasterAnchor::FramePath(vec![]),
        source: Arc::clone(&f.image),
        offset_px: (0, 0),
        z: 0,
    };
    assert!(matches!(
        d.resolve_frame(
            ms(0),
            FrameParts {
                compiled: c,
                viewport: f.size,
                underlay: &bg,
                planes: &[plane],
                focus: None,
                demands: &[],
                omissions: &[],
                motion: MotionPreference::Full
            },
            limits()
        ),
        Err(StageError::UnstableAnchor(0))
    ));
}

#[test]
fn clipping_gaps_and_implicit_key_geometry_are_disclosed() {
    let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
    let tree = column::<Msg>()
        .key("root")
        .padding(0)
        .gap(0)
        .width(40)
        .height(16)
        .child(label("implicit"))
        .child(
            column()
                .key("hidden")
                .height(0)
                .child(button("Gone").key("gone").on_press(Msg::Go)),
        )
        .child(column().key("art").width(8).height(2));
    let c = ui
        .frame(
            &tree,
            UiEnvironment {
                width: 40,
                height: 16,
                motion: MotionPreference::None,
                ..Default::default()
            },
            Duration::ZERO,
        )
        .unwrap();
    let bg = RgbRaster::new(40, 32);
    let mut d = PresentationDriver::new(options()).unwrap();
    let frame = d
        .resolve_frame(
            ms(0),
            FrameParts {
                compiled: c,
                viewport: (40, 16),
                underlay: &bg,
                planes: &[],
                focus: ui.focus(),
                demands: &[],
                omissions: &[key("declared-omission")],
                motion: MotionPreference::Full,
            },
            limits(),
        )
        .unwrap();
    assert_eq!(
        frame.snapshot.keys[&key("hidden")]
            .layout_rect
            .unwrap()
            .height,
        0
    );
    assert!(
        frame.snapshot.keys[&key("gone")].painted.is_none(),
        "pruned descendants get no fabricated painted rectangle"
    );
    assert!(frame
        .snapshot
        .keys
        .iter()
        .any(|(k, s)| matches!(k, Key::Path(_)) && s.layout_rect.is_none()));
    assert_eq!(frame.snapshot.omissions, [key("declared-omission")]);
}

#[test]
fn current_underlay_is_used_during_motion_and_output_owns_no_source_cache() {
    let mut f = Fixture::new();
    let image = Arc::clone(&f.image);
    let mut d = PresentationDriver::new(options()).unwrap();
    f.resolve(&mut d, ms(0), (0, 8, 4), &[], MotionPreference::Full)
        .unwrap();
    f.resolve(&mut d, ms(0), (8, 8, 4), &[], MotionPreference::Full)
        .unwrap();
    f.rgb = (100, 120, 140);
    let frame = f
        .resolve(&mut d, ms(100), (8, 8, 4), &[], MotionPreference::Full)
        .unwrap();
    let mix = |s: u32, b: u32| ((s * 128 + b * 127 + 127) / 255) as u8;
    assert_eq!(
        frame.snapshot.pixels.get(5, 1),
        Some((mix(200, 100), mix(100, 120), mix(0, 140)))
    );
    assert!(Arc::ptr_eq(&image, &f.image));
    assert_eq!(
        Arc::strong_count(&image),
        2,
        "driver/output must not retain a plane-source cache"
    );
    assert_eq!(image.get(1, 1), Some([200, 100, 0, 128]));
    assert_eq!(frame.snapshot.planes[0].source_size, (12, 12));
    assert_eq!(frame.snapshot.planes[0].z, 2);
}
