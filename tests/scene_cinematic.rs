//! Laws at the Scene → ordinary Node → framebuffer boundary.
use gibson::scene::{Effect, Presentation, Scene, SceneEntity, SceneTarget};
use gibson::{
    compute_layout, paint, Color, Node, RenderMode, Renderer, Style, Surface, SurfaceFx,
    TerminalSession,
};
use std::time::Duration;

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

fn fixture() -> (Scene, gibson::SceneId) {
    let mut scene = Scene::new();
    let id = scene.add(
        SceneEntity::new(
            "panel",
            Node::text("ordinary content", Style::default())
                .width(16.0)
                .height(3.0),
        )
        .offset(10, 5)
        .tag("ui"),
    );
    (scene, id)
}

fn evaluate(scene: &Scene, effect: &Effect, t: Duration) -> Presentation {
    let mut p = Presentation::new();
    effect.eval(t, scene, &mut p);
    p
}

fn framebuffer(mut node: Node) -> Surface {
    compute_layout(&mut node, 48, 16).unwrap();
    let mut surface = Surface::new(48, 16);
    paint(&node, &mut surface);
    surface
}

#[test]
fn additive_displacement_composes_with_baseline_and_absolute_trajectory() {
    let (scene, id) = fixture();
    let target = SceneTarget::Id(id);
    let perturbation = Effect::displace(target, (2.0, -1.0), (2.0, -1.0), ms(100));
    assert_eq!(
        evaluate(&scene, &perturbation, ms(50)).offset_of(&scene, id),
        (12, 4)
    );
    // Translate remains the existing absolute placement channel.
    let trajectory = Effect::translate(target, (10.0, 5.0), (15.0, 5.0), ms(100));
    let first = evaluate(
        &scene,
        &trajectory.clone().with(perturbation.clone()),
        ms(100),
    );
    let second = evaluate(&scene, &perturbation.with(trajectory), ms(100));
    assert_eq!(first, second);
    assert_eq!(first.offset_of(&scene, id), (17, 4));
}

#[test]
fn displacement_sums_before_clamping_and_commutes_even_at_integer_edges() {
    let (scene, id) = fixture();
    let target = SceneTarget::Id(id);
    let a = Effect::displace(target, (f32::MAX, 0.0), (f32::MAX, 0.0), ms(1));
    let b = a.clone();
    let c = Effect::displace(target, (f32::MIN, 0.0), (f32::MIN, 0.0), ms(1));
    for effects in [
        vec![a.clone(), b.clone(), c.clone()],
        vec![c.clone(), a.clone(), b.clone()],
        vec![a, c, b],
    ] {
        let p = evaluate(&scene, &Effect::parallel(effects), ms(1));
        assert_eq!(p.displacement_of(id), (i128::from(i32::MAX) - 1, 0));
        assert_eq!(p.offset_of(&scene, id), (i32::MAX, 5));
    }
}

#[test]
fn jitter_preserves_placement_and_uses_exact_modulo_at_large_times() {
    let (scene, id) = fixture();
    let target = SceneTarget::Id(id);
    let effect = Effect::jitter(target, 2.0, ms(400), ms(400));
    assert_eq!(
        evaluate(&scene, &effect, ms(100)).offset_of(&scene, id),
        (12, 5)
    );
    let large = Duration::MAX;
    let rem = Duration::from_nanos((large.as_nanos() % ms(400).as_nanos()) as u64);
    assert_eq!(
        evaluate(&scene, &effect, large),
        evaluate(&scene, &effect, rem)
    );
    assert_eq!(
        evaluate(
            &scene,
            &Effect::jitter(target, 2.0, Duration::ZERO, ms(10)),
            ms(5)
        ),
        Presentation::new()
    );
}

#[test]
fn loops_are_exact_at_boundaries_and_zero_duration_is_noop() {
    let (scene, id) = fixture();
    let target = SceneTarget::Id(id);
    let inner = Effect::translate(target, (0.0, 0.0), (10.0, 0.0), ms(100));
    let looping = inner.clone().looping();
    assert_eq!(looping.duration(), Duration::MAX);
    assert_eq!(
        evaluate(&scene, &looping, ms(100)),
        evaluate(&scene, &inner, Duration::ZERO)
    );
    assert_eq!(
        evaluate(&scene, &looping, Duration::MAX),
        evaluate(
            &scene,
            &inner,
            Duration::from_nanos((Duration::MAX.as_nanos() % ms(100).as_nanos()) as u64)
        )
    );
    assert_eq!(
        evaluate(
            &scene,
            &Effect::set_custom(target, 7, 1.0).looping(),
            Duration::MAX
        ),
        Presentation::new()
    );
}

#[test]
fn long_loop_and_repeat_remainders_are_not_truncated_to_u64_nanoseconds() {
    let (scene, id) = fixture();
    let inner = Effect::custom_ramp(
        SceneTarget::Id(id),
        5,
        0.0,
        1.0,
        Duration::from_secs(60_000_000_000),
    );
    let time = Duration::from_secs(100_000_000_000);
    let expected = evaluate(&scene, &inner, Duration::from_secs(40_000_000_000));
    assert_eq!(evaluate(&scene, &inner.clone().looping(), time), expected);
    assert_eq!(
        evaluate(&scene, &Effect::Repeat(Box::new(inner), 2), time),
        expected
    );
}

#[test]
fn duration_arithmetic_saturates_without_usize_repeat_truncation() {
    let (scene, id) = fixture();
    let effect = Effect::translate(SceneTarget::Id(id), (0.0, 0.0), (1.0, 0.0), Duration::MAX);
    assert_eq!(
        effect.clone().then(effect.clone()).duration(),
        Duration::MAX
    );
    assert_eq!(
        Effect::Delay(ms(1), Box::new(effect.clone())).duration(),
        Duration::MAX
    );
    assert_eq!(
        Effect::Repeat(Box::new(effect), usize::MAX).duration(),
        Duration::MAX
    );
    if usize::BITS > 32 {
        let repeat = Effect::Repeat(
            Box::new(Effect::custom_ramp(
                SceneTarget::Id(id),
                1,
                0.0,
                1.0,
                Duration::from_secs(1),
            )),
            (u32::MAX as usize) + 1,
        );
        assert_eq!(
            repeat.duration(),
            Duration::from_secs(u64::from(u32::MAX) + 1)
        );
        assert_eq!(evaluate(&scene, &repeat, ms(500)).custom(id, 1), Some(0.5));
    }
}

#[test]
fn cinematic_dissolve_preserves_legacy_visibility_and_exact_endpoints() {
    let (scene, id) = fixture();
    let effect = Effect::dissolve(SceneTarget::Id(id), 0.0, 1.0, 81, ms(100));
    let start = evaluate(&scene, &effect, Duration::ZERO);
    let end = evaluate(&scene, &effect, ms(100));
    assert_eq!(start.visibility_of(&scene, id), 1.0);
    assert_eq!(start.raw_visibility(id), None);
    assert_eq!(
        framebuffer(scene.to_node(&start, 48.0, 16.0)),
        Surface::new(48, 16)
    );
    assert_eq!(
        framebuffer(scene.to_node(&end, 48.0, 16.0)),
        framebuffer(scene.to_node(&Presentation::new(), 48.0, 16.0))
    );
}

#[test]
fn dissolve_masks_are_stable_and_entity_specific_for_tag_targets() {
    let (mut scene, first) = fixture();
    let second = scene.add(
        SceneEntity::new("second", Node::text("ordinary content", Style::default())).tag("ui"),
    );
    let target = SceneTarget::Tag(scene.tag("ui"));
    let fx = Effect::dissolve(target, 0.0, 1.0, 71, ms(100));
    let p = evaluate(&scene, &fx, ms(50));
    assert_eq!(p, evaluate(&scene, &fx, ms(50)));
    assert_ne!(p.surface_fx(first), p.surface_fx(second));
    let static_fx = Effect::post_process(
        target,
        SurfaceFx::Dissolve {
            seed: 71,
            fraction: 0.5,
        },
    );
    assert_eq!(p, evaluate(&scene, &static_fx, ms(50)));
}

#[test]
fn surface_chain_preserves_order_and_removal_restores_underlying_node() {
    let (scene, id) = fixture();
    let target = SceneTarget::Id(id);
    let tint = SurfaceFx::StyleOverlay(Style::new().fg(Color::Rgb(250, 40, 200)));
    let tear = SurfaceFx::Tear {
        row: 0,
        height: 1,
        amount: 2,
    };
    let effect =
        Effect::post_process(target, tint.clone()).with(Effect::post_process(target, tear.clone()));
    let p = evaluate(&scene, &effect, ms(10));
    assert_eq!(p.surface_fx(id), &[tint, tear]);
    let original = framebuffer(scene.to_node(&Presentation::new(), 48.0, 16.0));
    let infected = framebuffer(scene.to_node(&p, 48.0, 16.0));
    assert_ne!(infected, original);
    let clean = framebuffer(scene.to_node(&Presentation::new(), 48.0, 16.0));
    assert_eq!(clean, original);
    // Entity source data never acquires its presentation chain.
    assert_eq!(
        framebuffer(scene.entity(id).unwrap().node.clone()),
        framebuffer(fixture().0.entity(id).unwrap().node.clone())
    );
}

#[test]
fn one_entity_infection_is_local_and_a_frozen_frame_costs_zero_bytes() {
    let (scene, id) = fixture();
    let effect = Effect::post_process(SceneTarget::Id(id), SurfaceFx::Reverse);
    let presentation = evaluate(&scene, &effect, Duration::ZERO);
    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut session = TerminalSession::headless(48, 16);
    let mut output = Vec::new();
    renderer
        .render(
            &mut scene.to_node(&Presentation::new(), 48.0, 16.0),
            &mut session,
            &mut output,
        )
        .unwrap();
    output.clear();
    let (dirty, _, bytes, _, _) = renderer
        .render(
            &mut scene.to_node(&presentation, 48.0, 16.0),
            &mut session,
            &mut output,
        )
        .unwrap();
    assert!(dirty > 0 && dirty <= 16 * 3, "affected footprint = {dirty}");
    assert!(bytes > 0);
    output.clear();
    let (_, _, bytes, _, _) = renderer
        .render(
            &mut scene.to_node(&presentation, 48.0, 16.0),
            &mut session,
            &mut output,
        )
        .unwrap();
    assert_eq!(bytes, 0);
    assert!(output.is_empty());
    output.clear();
    let (dirty, _, _, _, _) = renderer
        .render(
            &mut scene.to_node(&Presentation::new(), 48.0, 16.0),
            &mut session,
            &mut output,
        )
        .unwrap();
    assert!(dirty > 0 && dirty <= 16 * 3);
}

#[test]
fn sequence_at_duration_max_starts_next_child_instead_of_skipping_it() {
    let (scene, id) = fixture();
    let target = SceneTarget::Id(id);
    let first = Effect::custom_ramp(target, 1, 0.0, 1.0, Duration::MAX);
    let second = Effect::custom_ramp(target, 2, 0.0, 1.0, ms(10));
    let p = evaluate(&scene, &first.then(second), Duration::MAX);
    assert_eq!(p.custom(id, 1), Some(1.0));
    assert_eq!(p.custom(id, 2), Some(0.0));
}

#[test]
fn overflowing_repeat_at_duration_max_starts_next_cycle() {
    let (scene, id) = fixture();
    let inner = Effect::custom_ramp(SceneTarget::Id(id), 1, 0.0, 1.0, Duration::MAX);
    let p = evaluate(&scene, &Effect::Repeat(Box::new(inner), 2), Duration::MAX);
    assert_eq!(p.custom(id, 1), Some(0.0));
}

#[test]
fn style_mask_progression_preserves_glyphs_and_restores_without_undo() {
    let (scene, id) = fixture();
    let style = Style::new().reverse();
    let effect = Effect::style_mask(SceneTarget::Id(id), style, 0.0, 1.0, 91, ms(100));
    let clean = framebuffer(scene.to_node(&Presentation::new(), 48.0, 16.0));
    let start = evaluate(&scene, &effect, Duration::ZERO);
    assert_eq!(framebuffer(scene.to_node(&start, 48.0, 16.0)), clean);
    let mut counts = Vec::new();
    for at in [10, 30, 60, 100] {
        let p = evaluate(&scene, &effect, ms(at));
        let surface = framebuffer(scene.to_node(&p, 48.0, 16.0));
        assert!(surface
            .cells
            .iter()
            .zip(&clean.cells)
            .all(|(a, b)| a.glyph == b.glyph));
        counts.push(
            surface
                .cells
                .iter()
                .zip(&clean.cells)
                .filter(|(a, b)| a.style != b.style)
                .count(),
        );
    }
    assert!(counts.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(counts[3] > counts[0]);
    let end = evaluate(&scene, &effect, ms(100));
    let overlay = evaluate(
        &scene,
        &Effect::post_process(SceneTarget::Id(id), SurfaceFx::StyleOverlay(style)),
        Duration::ZERO,
    );
    assert_eq!(
        framebuffer(scene.to_node(&end, 48.0, 16.0)),
        framebuffer(scene.to_node(&overlay, 48.0, 16.0))
    );
    assert_eq!(
        framebuffer(scene.to_node(&Presentation::new(), 48.0, 16.0)),
        clean
    );
}

#[test]
fn style_mask_tag_seeds_match_static_and_dynamic_forms() {
    let (mut scene, first) = fixture();
    let second = scene.add(
        SceneEntity::new("second", Node::text("ordinary content", Style::default())).tag("ui"),
    );
    let target = SceneTarget::Tag(scene.tag("ui"));
    let style = Style::new().reverse();
    let p = evaluate(
        &scene,
        &Effect::style_mask(target, style, 0.0, 1.0, 71, ms(100)),
        ms(50),
    );
    assert_ne!(p.surface_fx(first), p.surface_fx(second));
    let static_fx = Effect::post_process(
        target,
        SurfaceFx::StyleMask {
            style,
            seed: 71,
            fraction: 0.5,
        },
    );
    assert_eq!(p, evaluate(&scene, &static_fx, ms(50)));
}
