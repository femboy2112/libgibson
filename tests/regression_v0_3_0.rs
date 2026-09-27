use gibson::context::{Context, RenderMode};
use gibson::node::Node;
use gibson::{
    FramePacing, PresentationProfile, Style, SubcellGlyphMode, TemporalBrailleField, TemporalGate,
    TemporalSafetyPolicy,
};

#[test]
fn context_frame_pacing_is_compatibility_default_and_phase_locked_is_opt_in() {
    let mut ctx = Context::headless(RenderMode::Inline, 40, 10);
    assert_eq!(ctx.frame_pacing(), FramePacing::CompletionRelative);
    ctx.set_frame_pacing(FramePacing::PhaseLocked);
    assert_eq!(ctx.frame_pacing(), FramePacing::PhaseLocked);
}

#[test]
fn frame_report_exposes_exact_change_and_settled_zero_write() {
    let mut ctx = Context::headless(RenderMode::Inline, 40, 10);
    ctx.set_sync_updates(false);
    ctx.set_root(Node::text("FRAME REPORT", Style::default()));
    ctx.render_now().unwrap();
    let first = ctx.last_frame_report();
    assert!(first.exact_changed_cells > 0);
    assert!(first.total_cells > 0);
    assert!(first.bytes_emitted > 0);
    assert!(first.total_duration() >= first.generation_duration);

    ctx.render_now().unwrap();
    let settled = ctx.last_frame_report();
    assert_eq!(settled.exact_changed_cells, 0);
    assert_eq!(settled.affected_cells, 0);
    assert_eq!(settled.bytes_emitted, 0);
    assert_eq!(settled.write_duration, std::time::Duration::ZERO);
}

#[test]
fn temporal_field_composes_as_an_ordinary_surface() {
    let mut field = TemporalBrailleField::new(3, 2, 0xA11CE);
    for y in 0..field.height() {
        for x in 0..field.width() {
            field.set_cell_duty(x, y, [0.25, 0.5, 0.75, 1.0, 0.0, 0.25, 0.5, 0.75]);
        }
    }
    let surface = field.advance(Style::default(), SubcellGlyphMode::Braille2x4);
    assert_eq!((surface.width, surface.height), (3, 2));

    let mut ctx = Context::headless(RenderMode::Inline, 20, 5);
    ctx.set_sync_updates(false);
    ctx.set_root(Node::surface(std::sync::Arc::new(surface)));
    ctx.render_now().unwrap();
    assert!(ctx.last_frame_report().bytes_emitted > 0);
}

#[test]
fn temporal_luminance_gate_defaults_to_static_for_unmeasured_and_60hz_paths() {
    let policy = TemporalSafetyPolicy::default();
    assert_eq!(
        policy.gate_luminance(PresentationProfile::unmeasured(), 0.02, false),
        TemporalGate::Unmeasured
    );
    assert_eq!(
        policy.gate_luminance(PresentationProfile::measured(60.0, 1.0, 0.0), 0.02, false),
        TemporalGate::CadenceTooLow
    );
    assert_eq!(
        policy.gate_luminance(PresentationProfile::measured(120.0, 0.98, 0.2), 0.02, false),
        TemporalGate::Enabled
    );
}
