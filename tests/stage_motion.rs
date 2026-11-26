//! M5 G1-G4 contract tests. Public synthetic fixtures only (tiny solid-colour rasters).
use gibson::raster::{composite_over, Rgb, RgbRaster, RgbaRaster};
use gibson::stage::motion::*;
use gibson::stage::*;
use gibson::ui::experience::FrameDemand;
use gibson::ui::prelude::*;
use gibson::{Context, RenderMode};
use std::sync::Arc;
use std::time::Duration;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn ns(n: u64) -> Duration {
    Duration::from_nanos(n)
}
fn solid(w: u16, h: u16, c: [u8; 4]) -> Arc<RgbaRaster> {
    let mut r = RgbaRaster::new(w, h);
    r.pixels_mut().fill(c);
    Arc::new(r)
}
fn frame(c: u8, hold_ms: u64) -> SequenceFrame {
    SequenceFrame::new(solid(2, 2, [c, 0, 0, 255]), ms(hold_ms))
}
/// Identify a frame by its red channel.
fn tag(s: &SequenceSample) -> u8 {
    s.source.get(0, 0).unwrap()[0]
}
/// frames tagged 10,20,30 holding 100,50,200 ms: starts 0,100,150, total 350; rest = 0.
fn seq() -> FrameSequence {
    FrameSequence::new(vec![frame(10, 100), frame(20, 50), frame(30, 200)], 0).unwrap()
}

// ---------------------------------------------------------------- G1

#[test]
fn g1_half_open_boundaries() {
    let s = seq();
    let full = MotionPreference::Full;
    let cases: [(u64, usize, u8, Option<u64>); 8] = [
        (0, 0, 10, Some(100)),
        (99, 0, 10, Some(1)),
        (100, 1, 20, Some(50)),
        (149, 1, 20, Some(1)),
        (150, 2, 30, Some(200)),
        (349, 2, 30, Some(1)),
        (350, 0, 10, None),
        (351, 0, 10, None),
    ];
    for (t, idx, tg, next) in cases {
        let x = s.sample(ms(t), full);
        assert_eq!((x.index, tag(&x), x.next_in), (idx, tg, next.map(ms)), "t={t}");
    }
}

#[test]
fn g1_rest_after_total_has_no_deadline() {
    let s = FrameSequence::new(vec![frame(10, 100), frame(20, 50)], 1).unwrap();
    for t in [ms(150), ms(10_000), Duration::MAX] {
        let x = s.sample(t, MotionPreference::Full);
        assert_eq!((x.index, tag(&x), x.next_in), (1, 20, None));
    }
}

#[test]
fn g1_reduced_and_none_rest_immediately() {
    let s = FrameSequence::new(vec![frame(10, 100), frame(20, 50), frame(30, 5)], 2).unwrap();
    for m in [MotionPreference::Reduced, MotionPreference::None] {
        for t in [ms(0), ms(120), ms(9999)] {
            let x = s.sample(t, m);
            assert_eq!((x.index, tag(&x), x.next_in), (2, 30, None));
        }
    }
}

#[test]
fn g1_refusals() {
    assert_eq!(FrameSequence::new(vec![], 0).unwrap_err(), SequenceError::Empty);
    let many: Vec<_> = (0..65).map(|_| frame(1, 1)).collect();
    assert!(matches!(
        FrameSequence::new(many, 0),
        Err(SequenceError::TooManyFrames { .. })
    ));
    let max: Vec<_> = (0..64).map(|_| frame(1, 1)).collect();
    assert!(FrameSequence::new(max, 63).is_ok(), "64 frames is allowed");
    let short = vec![frame(1, 5), SequenceFrame::new(solid(2, 2, [0; 4]), Duration::from_micros(999))];
    assert!(matches!(
        FrameSequence::new(short, 0),
        Err(SequenceError::HoldTooShort { index: 1 })
    ));
    let exact = vec![SequenceFrame::new(solid(2, 2, [0; 4]), ms(1))];
    assert!(FrameSequence::new(exact, 0).is_ok(), "exactly 1 ms is allowed");
    assert!(matches!(
        FrameSequence::new(vec![frame(1, 5)], 1),
        Err(SequenceError::RestOutOfRange { rest: 1, len: 1 })
    ));
    let mismatch = vec![frame(1, 5), SequenceFrame::new(solid(3, 2, [0; 4]), ms(5))];
    assert!(matches!(
        FrameSequence::new(mismatch, 0),
        Err(SequenceError::SizeMismatch { index: 1 })
    ));
    let tall = vec![frame(1, 5), SequenceFrame::new(solid(2, 3, [0; 4]), ms(5))];
    assert!(matches!(
        FrameSequence::new(tall, 0),
        Err(SequenceError::SizeMismatch { index: 1 })
    ));
}

#[test]
fn g1_sum_of_holds_overflow_is_refused() {
    let near = Duration::MAX - ms(1);
    let two = vec![
        SequenceFrame::new(solid(2, 2, [0; 4]), near),
        SequenceFrame::new(solid(2, 2, [0; 4]), near),
    ];
    assert_eq!(
        FrameSequence::new(two, 0).unwrap_err(),
        SequenceError::DurationOverflow
    );
    // One huge hold alone is fine, and samples without panicking.
    let one = vec![SequenceFrame::new(solid(2, 2, [7, 0, 0, 255]), Duration::MAX)];
    let s = FrameSequence::new(one, 0).unwrap();
    let x = s.sample(ms(5), MotionPreference::Full);
    assert_eq!(x.next_in, Some(Duration::MAX - ms(5)));
}

#[test]
fn g1_is_deterministic_and_late_sampling_coalesces() {
    let s = seq();
    for t in [0u64, 1, 99, 100, 149, 150, 349, 350, 1000] {
        let a = s.sample(ms(t), MotionPreference::Full);
        let b = s.clone().sample(ms(t), MotionPreference::Full);
        assert_eq!((a.index, a.next_in), (b.index, b.next_in));
        assert!(Arc::ptr_eq(&a.source, &b.source) || *a.source == *b.source);
    }
    // Sampling late (t=160 straight away) lands on frame 2: frame 1 is never replayed.
    let late = s.sample(ms(160), MotionPreference::Full);
    assert_eq!((late.index, late.next_in), (2, Some(ms(190))));
    // Sampling out of order is fine: no hidden state.
    let _ = s.sample(ms(300), MotionPreference::Full);
    assert_eq!(s.sample(ms(10), MotionPreference::Full).index, 0);
}

// ---------------------------------------------------------------- G2

fn rect(x: i32, y: i32, w: u32, h: u32) -> PixelRect {
    PixelRect { x, y, width: w, height: h }
}
fn slots() -> LayerSlots {
    LayerSlots::new(
        (8, 8),
        vec![
            LayerSlot::new("eyes", rect(2, 1, 4, 2)),
            LayerSlot::new("mouth", rect(3, 5, 2, 2)),
        ],
    )
    .unwrap()
}

#[test]
fn g2_plane_order_and_offsets() {
    let base = solid(8, 8, [10, 10, 10, 255]);
    let eyes = solid(4, 2, [200, 0, 0, 255]);
    let mouth = solid(2, 2, [0, 200, 0, 255]);
    // Fill order differs from declaration order; output follows declaration order.
    let planes = slots()
        .planes(
            Key::named("avatar"),
            Arc::clone(&base),
            &[("mouth", Arc::clone(&mouth)), ("eyes", Arc::clone(&eyes))],
            5,
        )
        .unwrap();
    assert_eq!(planes.len(), 3);
    for p in &planes {
        assert_eq!(p.anchor, RasterAnchor::Key(Key::named("avatar")));
    }
    assert_eq!((planes[0].offset_px, planes[0].z), ((0, 0), 5));
    assert!(Arc::ptr_eq(&planes[0].source, &base));
    assert_eq!((planes[1].offset_px, planes[1].z), ((2, 1), 6));
    assert!(Arc::ptr_eq(&planes[1].source, &eyes));
    assert_eq!((planes[2].offset_px, planes[2].z), ((3, 5), 7));
    assert!(Arc::ptr_eq(&planes[2].source, &mouth));
}

#[test]
fn g2_slot_index_not_fill_rank_sets_z() {
    // Only the second slot filled: it keeps z + 1 + 1.
    let planes = slots()
        .planes(
            Key::named("a"),
            solid(8, 8, [0; 4]),
            &[("mouth", solid(2, 2, [1, 1, 1, 255]))],
            0,
        )
        .unwrap();
    assert_eq!(planes.len(), 2);
    assert_eq!((planes[1].offset_px, planes[1].z), ((3, 5), 2));
}

#[test]
fn g2_new_refusals() {
    let s = |v| LayerSlots::new((8, 8), v);
    assert!(matches!(
        s(vec![LayerSlot::new("a", rect(-1, 0, 2, 2))]),
        Err(SlotError::OutsideBase { .. })
    ));
    assert!(matches!(
        s(vec![LayerSlot::new("a", rect(0, -1, 2, 2))]),
        Err(SlotError::OutsideBase { .. })
    ));
    assert!(matches!(
        s(vec![LayerSlot::new("a", rect(7, 0, 2, 2))]),
        Err(SlotError::OutsideBase { .. })
    ));
    assert!(matches!(
        s(vec![LayerSlot::new("a", rect(0, 7, 2, 2))]),
        Err(SlotError::OutsideBase { .. })
    ));
    assert!(s(vec![LayerSlot::new("a", rect(6, 6, 2, 2))]).is_ok(), "touching the edge is inside");
    assert!(matches!(
        s(vec![LayerSlot::new("a", rect(0, 0, 0, 2))]),
        Err(SlotError::EmptyRect { .. })
    ));
    assert!(matches!(
        s(vec![LayerSlot::new("a", rect(0, 0, 2, 0))]),
        Err(SlotError::EmptyRect { .. })
    ));
    assert!(matches!(
        s(vec![
            LayerSlot::new("a", rect(0, 0, 1, 1)),
            LayerSlot::new("a", rect(2, 2, 1, 1))
        ]),
        Err(SlotError::DuplicateName { .. })
    ));
    // Extreme coordinates must refuse, not wrap.
    assert!(matches!(
        s(vec![LayerSlot::new("a", rect(i32::MAX, 0, u32::MAX, 1))]),
        Err(SlotError::OutsideBase { .. })
    ));
}

#[test]
fn g2_planes_refusals() {
    let sl = slots();
    let k = || Key::named("a");
    let base = || solid(8, 8, [0; 4]);
    assert!(matches!(
        sl.planes(k(), solid(8, 7, [0; 4]), &[], 0),
        Err(SlotError::BaseSizeMismatch { .. })
    ));
    assert!(matches!(
        sl.planes(k(), solid(7, 8, [0; 4]), &[], 0),
        Err(SlotError::BaseSizeMismatch { .. })
    ));
    assert!(matches!(
        sl.planes(k(), base(), &[("nose", solid(1, 1, [0; 4]))], 0),
        Err(SlotError::UnknownSlot { .. })
    ));
    for (w, h) in [(3, 2), (5, 2), (4, 1), (4, 3)] {
        assert!(
            matches!(
                sl.planes(k(), base(), &[("eyes", solid(w, h, [0; 4]))], 0),
                Err(SlotError::FillSizeMismatch { .. })
            ),
            "{w}x{h}"
        );
    }
    assert!(matches!(
        sl.planes(
            k(),
            base(),
            &[("eyes", solid(4, 2, [0; 4])), ("eyes", solid(4, 2, [0; 4]))],
            0
        ),
        Err(SlotError::DuplicateFill { .. })
    ));
    assert!(sl.planes(k(), base(), &[], 0).is_ok(), "no fills is just the base");
}

fn node_root(w: u16, h: u16) -> gibson::Node {
    gibson::Node::col().width(f32::from(w)).height(f32::from(h))
}
fn under() -> RgbRaster {
    let mut r = RgbRaster::new(8, 8);
    r.clear((90, 80, 70));
    r
}
fn resolve(planes: &[RasterPlane]) -> RgbRaster {
    // FramePath(root) is the raw-resolver anchor; offsets and z are carried unchanged.
    let raw: Vec<RasterPlane> = planes
        .iter()
        .map(|p| RasterPlane {
            anchor: RasterAnchor::FramePath(vec![]),
            ..p.clone()
        })
        .collect();
    resolve_raster(
        &node_root(8, 4),
        (8, 4),
        &under(),
        &raw,
        RasterLimits { planes: 16, nodes: 100, pixels: 100_000 },
    )
    .unwrap()
    .pixels
}
fn same(a: &RgbRaster, b: &RgbRaster) {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    for y in 0..i32::from(a.height()) {
        for x in 0..i32::from(a.width()) {
            assert_eq!(a.get(x, y), b.get(x, y), "pixel {x},{y}");
        }
    }
}

#[test]
fn g2_composite_equals_hand_composite_and_unfilled_equals_base() {
    let base = solid(8, 8, [10, 200, 30, 255]);
    let eyes = solid(4, 2, [200, 0, 0, 128]);
    let mouth = solid(2, 2, [0, 0, 220, 255]);
    let sl = slots();
    let anchor = Key::named("avatar");

    let all = sl
        .planes(
            anchor.clone(),
            Arc::clone(&base),
            &[("eyes", Arc::clone(&eyes)), ("mouth", Arc::clone(&mouth))],
            0,
        )
        .unwrap();
    let mut hand = under();
    composite_over(&mut hand, &base, 0, 0);
    composite_over(&mut hand, &eyes, 2, 1);
    composite_over(&mut hand, &mouth, 3, 5);
    same(&resolve(&all), &hand);

    // Unfilled slots: byte-identical to the base plane alone.
    let none = sl.planes(anchor.clone(), Arc::clone(&base), &[], 0).unwrap();
    let alone = [RasterPlane {
        anchor: RasterAnchor::Key(anchor),
        source: Arc::clone(&base),
        offset_px: (0, 0),
        z: 0,
    }];
    same(&resolve(&none), &resolve(&alone));
    // One slot filled leaves the other region equal to the base alone.
    let part = sl
        .planes(Key::named("avatar"), Arc::clone(&base), &[("eyes", eyes)], 0)
        .unwrap();
    let (p, b) = (resolve(&part), resolve(&alone));
    assert_eq!(p.get(3, 5), b.get(3, 5));
    assert_ne!(p.get(3, 1), b.get(3, 1));
}

// ---------------------------------------------------------------- G3

const SEC: u64 = 1_000_000_000;
fn cue(seed: u64, c: u64) -> CueSchedule {
    CueSchedule::new(seed, c, Duration::from_secs(10), Duration::from_secs(4), Duration::from_secs(1))
        .unwrap()
}

/// Independent reference for the contract hash (standard SplitMix64 step: golden gamma first).
fn ref_mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}
fn ref_h(seed: u64, c: u64, k: u64) -> u64 {
    ref_mix(ref_mix(ref_mix(seed) ^ c) ^ k)
}

#[test]
fn g3_hash_golden_table() {
    // changing these values requires a minor release (M5 spec G3).
    // Derived with a separate python3 implementation. origin 10 s, period 4 s, min_gap 1 s
    // (span 3e9 ns): nth(k) = 10e9 + k*4e9 + h % (3e9 + 1).
    let table: [(u64, u64, u64, u64, Option<u64>); 4] = [
        (0, 0, 0, 0x238275bc38fcbe91, Some(11_717_340_104)),
        (1, 2, 3, 0xd0734750fde362b3, Some(24_386_420_295)),
        (0xDEADBEEF, 7, 100, 0xe5a79e8023c79bb9, Some(412_473_340_274)),
        (u64::MAX, u64::MAX, u64::MAX, 0x595b17f487c0e71b, None),
    ];
    for (seed, c, k, h, nth) in table {
        assert_eq!(ref_h(seed, c, k), h, "reference hash ({seed},{c},{k})");
        let expect_jitter = h % (3 * SEC + 1);
        if let Some(n) = nth {
            assert_eq!(cue(seed, c).nth(k), Some(ns(n)), "nth ({seed},{c},{k})");
            assert_eq!(n, 10 * SEC + k * 4 * SEC + expect_jitter);
        } else {
            assert_eq!(cue(seed, c).nth(k), None, "k = u64::MAX overflows");
        }
    }
    assert_ne!(ref_h(0, 0, 0), 0, "SplitMix64 must not have a zero fixed point");
    // The jitter of the max tuple, via a schedule whose period leaves headroom for k.
    let big = CueSchedule::new(u64::MAX, u64::MAX, Duration::ZERO, ns(1), Duration::ZERO).unwrap();
    // span 1 -> jitter = h % 2; h is odd so jitter 1.
    assert_eq!(big.nth(u64::MAX), Some(ns(u64::MAX) + ns(0x595b17f487c0e71b % 2)));
}

#[test]
fn g3_nth_matches_reference_and_differs_across_cues() {
    let (a, b) = (cue(42, 1), cue(42, 2));
    let mut differing = 0;
    for k in 0..50 {
        let expect = 10 * SEC + k * 4 * SEC + ref_h(42, 1, k) % (3 * SEC + 1);
        assert_eq!(a.nth(k), Some(ns(expect)));
        assert_eq!(a.nth(k), a.clone().nth(k), "pure");
        if a.nth(k) != b.nth(k) {
            differing += 1;
        }
    }
    assert!(differing > 40, "cues are independent: {differing}/50 differ");
    assert_ne!(cue(1, 1).nth(3), cue(2, 1).nth(3), "seed matters");
}

#[test]
fn g3_next_after_before_origin_is_k0() {
    let s = cue(9, 9);
    let n0 = s.nth(0).unwrap();
    for t in [Duration::ZERO, ms(1), Duration::from_secs(9), Duration::from_secs(10) - ns(1)] {
        assert_eq!(s.next_after(t), Some((0, n0)), "t={t:?}");
        assert!(n0 > t);
    }
    // t = 0 with a large origin.
    let far = CueSchedule::new(5, 5, Duration::from_secs(1_000_000), ms(10), ms(5)).unwrap();
    let (k, at) = far.next_after(Duration::ZERO).unwrap();
    assert_eq!((k, Some(at)), (0, far.nth(0)));
    // origin exactly == t: occurrence 0 may still be exactly origin only if jitter 0; must be > t.
    let (k, at) = s.next_after(Duration::from_secs(10)).unwrap();
    assert!(at > Duration::from_secs(10) && Some(at) == s.nth(k));
}

#[test]
fn g3_overflow_is_none() {
    let s = cue(1, 1);
    assert_eq!(s.nth(u64::MAX), None);
    let top = CueSchedule::new(1, 1, Duration::MAX, ns(1), Duration::ZERO).unwrap();
    assert_eq!(top.nth(1), None);
    assert_eq!(top.next_after(Duration::MAX), None);
    let near = CueSchedule::new(1, 1, Duration::MAX - ns(1), ns(10), Duration::ZERO).unwrap();
    assert_eq!(near.nth(1), None);
    // A t whose slot index does not fit in u64 (period 1 ns, t huge) yields None, not a wrap.
    let tiny = CueSchedule::new(1, 1, Duration::ZERO, ns(1), Duration::ZERO).unwrap();
    assert_eq!(tiny.next_after(Duration::MAX), None);
}

#[test]
fn g3_constructor_refusals() {
    let d = Duration::from_secs;
    assert_eq!(CueSchedule::new(0, 0, d(0), d(0), d(0)).unwrap_err(), CueError::ZeroPeriod);
    assert_eq!(
        CueSchedule::new(0, 0, d(0), d(1), d(2)).unwrap_err(),
        CueError::GapExceedsPeriod
    );
    assert!(CueSchedule::new(0, 0, d(0), d(1), d(1)).is_ok(), "min_gap == period");
    let too_long = ns(u64::MAX) + ns(1);
    assert_eq!(
        CueSchedule::new(0, 0, d(0), too_long, d(0)).unwrap_err(),
        CueError::PeriodTooLong
    );
    assert!(CueSchedule::new(0, 0, d(0), ns(u64::MAX), d(0)).is_ok());
    // min_gap == period: zero jitter, perfectly periodic.
    let fixed = CueSchedule::new(3, 3, d(2), d(1), d(1)).unwrap();
    for k in 0..5 {
        assert_eq!(fixed.nth(k), Some(d(2 + k)));
    }
}

#[test]
fn g3_next_after_consistent_with_nth_at_boundaries() {
    let s = cue(77, 3);
    let mut probes = vec![Duration::ZERO, Duration::from_secs(10)];
    for k in 0..40 {
        let n = s.nth(k).unwrap();
        for t in [n.saturating_sub(ns(1)), n, n + ns(1)] {
            probes.push(t);
        }
        probes.push(Duration::from_secs(10 + 4 * k));
        probes.push(Duration::from_secs(10 + 4 * k) - ns(1));
    }
    for t in probes {
        let (k, at) = s.next_after(t).unwrap();
        assert_eq!(s.nth(k), Some(at));
        assert!(at > t, "strictly after: t={t:?}");
        // Brute force: no earlier occurrence is strictly after t.
        let first = (0..).map(|j| (j, s.nth(j).unwrap())).find(|(_, x)| *x > t).unwrap();
        assert_eq!((k, at), first, "t={t:?}");
    }
}

#[test]
fn g3_next_after_is_constant_time_for_huge_t() {
    // 584-year-scale t with a 1 ms period: a loop proportional to t could not finish.
    let s = CueSchedule::new(8, 8, Duration::ZERO, ms(1), Duration::ZERO).unwrap();
    let t = Duration::from_secs(10_000_000_000);
    let start = std::time::Instant::now();
    let (k, at) = s.next_after(t).unwrap();
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(at > t && Some(at) == s.nth(k));
    assert!(k >= 10_000_000_000_000 - 1);
}

#[test]
fn g3_seeded_property_gap_bounds_over_1e5_occurrences() {
    let mut seed = 0x5EED_u64 ^ 0x1234_5678_9abc_def0;
    seed = ref_mix(seed);
    println!("g3 property seed = {seed:#x}");
    let mut state = seed;
    let mut next = || {
        state = ref_mix(state);
        state
    };
    for round in 0..4 {
        let period = 1 + next() % 5_000_000_000;
        let min_gap = next() % (period + 1);
        let origin = next() % (100 * SEC);
        let (cs, cc) = (next(), next());
        let s = CueSchedule::new(cs, cc, ns(origin), ns(period), ns(min_gap)).unwrap();
        let mut prev = s.nth(0).unwrap();
        assert!(prev >= ns(origin) && prev <= ns(origin + period - min_gap));
        for k in 1..25_000u64 {
            let cur = s.nth(k).unwrap();
            let slot = ns(origin) + ns(period) * (k as u32);
            assert!(cur >= slot && cur <= slot + ns(period - min_gap), "seed={seed:#x} round={round} k={k}");
            let gap = cur - prev;
            assert!(
                gap >= ns(min_gap) && gap <= ns(2 * period - min_gap),
                "seed={seed:#x} round={round} k={k} gap={gap:?}"
            );
            prev = cur;
            // next_after(prev - 1ns) is exactly this occurrence.
            assert_eq!(s.next_after(cur - ns(1)).map(|(i, _)| i).filter(|&i| i <= k), Some(k));
        }
    }
}

// ---------------------------------------------------------------- G4

#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {}

fn tree() -> gibson::ui::Element<Msg> {
    column()
        .key("root")
        .width(8)
        .height(4)
        .padding(0)
        .gap(0)
        .child(column().key("art").width(4).height(2).padding(0).gap(0))
}

#[test]
fn g4_hidden_means_zero_scheduled_work() {
    let mut ui: UiRuntime<Msg> = UiRuntime::new(skins::SWISS_SIGNAL);
    let mut compile = |now: Duration| {
        ui.frame(
            &tree(),
            UiEnvironment { width: 8, height: 4, motion: MotionPreference::Full, ..Default::default() },
            now,
        )
        .unwrap()
    };
    let mut driver = PresentationDriver::new(PresentationOptions {
        transition: ms(200),
        cadence: ms(25),
    })
    .unwrap();
    let limits = RasterLimits { planes: 16, nodes: 256, pixels: 100_000 };
    let mut underlay = RgbRaster::new(8, 8);
    underlay.clear((1, 2, 3) as Rgb);

    // Visible: a G1-sampled plane and its After(next_in) demand.
    let sequence = FrameSequence::new(vec![frame(10, 100), frame(20, 100)], 0).unwrap();
    let sample = sequence.sample(ms(0), MotionPreference::Full);
    let next_in = sample.next_in.expect("playing");
    let plane = RasterPlane {
        anchor: RasterAnchor::Key(Key::named("art")),
        source: sample.source,
        offset_px: (0, 0),
        z: 0,
    };
    let demand = [SourceDemand { key: Key::named("art"), demand: FrameDemand::After(next_in) }];
    let shown = driver
        .resolve_frame(
            ms(0),
            FrameParts {
                compiled: compile(ms(0)),
                viewport: (8, 4),
                underlay: &underlay,
                planes: std::slice::from_ref(&plane),
                focus: None,
                demands: &demand,
                omissions: &[],
                motion: MotionPreference::Full,
            },
            limits,
        )
        .unwrap();
    assert_eq!(driver.next_deadline(), Some(ms(100)), "visible avatar asks for its frame change");

    let mut ctx = Context::headless(RenderMode::Fullscreen, 8, 4);
    ctx.scheduler.set_max_fps(u32::MAX);
    ctx.set_root(shown.to_node());
    ctx.render_now().unwrap();
    assert!(!ctx.take_rendered_bytes().is_empty());

    // Hide: omit the source from demands and the plane from planes.
    let hidden = driver
        .resolve_frame(
            ms(50),
            FrameParts {
                compiled: compile(ms(50)),
                viewport: (8, 4),
                underlay: &underlay,
                planes: &[],
                focus: None,
                demands: &[],
                omissions: &[],
                motion: MotionPreference::Full,
            },
            limits,
        )
        .unwrap();
    assert_eq!(driver.next_deadline(), None);
    assert_eq!(hidden.snapshot.next_deadline, None);
    ctx.set_root(hidden.to_node());
    ctx.render_now().unwrap();
    ctx.take_rendered_bytes();
    assert_eq!(ctx.time_until_frame_needed(), None);
    // Idle run_once emits nothing.
    assert!(ctx.run_once(ms(1)).unwrap().is_none());
    assert!(ctx.take_rendered_bytes().is_empty(), "idle run_once emits 0 bytes");
    assert_eq!(ctx.time_until_frame_needed(), None);

    // Re-show at a later host time resumes from that time and replays nothing.
    let later = ms(900);
    let s2 = sequence.sample(later - ms(900), MotionPreference::Full);
    let demand2 = [SourceDemand { key: Key::named("art"), demand: FrameDemand::After(s2.next_in.unwrap()) }];
    driver
        .resolve_frame(
            later,
            FrameParts {
                compiled: compile(later),
                viewport: (8, 4),
                underlay: &underlay,
                planes: &[],
                focus: None,
                demands: &demand2,
                omissions: &[],
                motion: MotionPreference::Full,
            },
            limits,
        )
        .unwrap();
    assert_eq!(driver.next_deadline(), Some(later + ms(100)));
}
