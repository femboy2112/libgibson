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
fn temporal_surface_composes_through_a_clipped_viewport() {
    let mut field = TemporalBrailleField::new(4, 1, 0xBEEF);
    for x in 0..4 {
        field.set_cell_duty(x, 0, [1.0; 8]);
    }
    let surface = field.advance(Style::default(), SubcellGlyphMode::Braille2x4);
    let root = Node::viewport(1, 0).width(2.0).height(1.0).child(
        Node::surface(std::sync::Arc::new(surface))
            .width(4.0)
            .height(1.0),
    );

    let mut ctx = Context::headless(RenderMode::Inline, 2, 2);
    ctx.set_sync_updates(false);
    ctx.set_root(root);
    ctx.render_now().unwrap();

    let lines = ctx.last_frame_lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].chars().count(), 2);
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

#[test]
fn processor_static_fallback_composes_and_clips_without_leaking() {
    use gibson::{compute_layout, paint, ResetPolicy, Surface, TemporalDisplayProcessor};

    // 4x1 processor; each cell has 4 white + 4 black dots => a non-empty static
    // mask, so every fallback cell is a visible Braille glyph.
    let mut proc = TemporalDisplayProcessor::new(4, 1, SubcellGlyphMode::Braille2x4, 0x1234);
    proc.set_target_image(
        |lx, _ly| {
            if lx % 2 == 0 {
                [255, 255, 255]
            } else {
                [0, 0, 0]
            }
        },
        ResetPolicy::Reset,
    );
    let fallback = proc.static_fallback();
    assert_eq!((fallback.width, fallback.height), (4, 1));
    assert_ne!(fallback.get(0, 0).unwrap().glyph.grapheme.as_str(), " ");

    // Compose the 4-wide surface through a 2-wide viewport panned by (1, 0) inside
    // a 4-wide root, and paint into an exact 4x1 buffer through the ordinary
    // layout/paint pipeline (no second renderer).
    let mut root = Node::stack().width(4.0).height(1.0).child(
        Node::viewport(1, 0)
            .width(2.0)
            .height(1.0)
            .child(Node::raster(proc.static_fallback()).width(4.0).height(1.0)),
    );
    compute_layout(&mut root, 4, 1).unwrap();
    let mut out = Surface::new(4, 1);
    paint(&root, &mut out);

    // The 2-wide window (x=0,1) shows the surface's columns 1 and 2 (offset 1).
    assert_eq!(
        out.get(0, 0).unwrap().glyph.grapheme,
        fallback.get(1, 0).unwrap().glyph.grapheme
    );
    assert_eq!(
        out.get(1, 0).unwrap().glyph.grapheme,
        fallback.get(2, 0).unwrap().glyph.grapheme
    );
    // Nothing leaks past the 2-wide viewport into x=2,3 of the root surface.
    assert_eq!(out.get(2, 0).unwrap().glyph.grapheme.as_str(), " ");
    assert_eq!(out.get(3, 0).unwrap().glyph.grapheme.as_str(), " ");
}

#[test]
fn processor_advance_composes_through_the_render_pipeline() {
    use gibson::{ResetPolicy, TemporalDisplayProcessor};

    let mut proc = TemporalDisplayProcessor::new(3, 2, SubcellGlyphMode::Braille2x4, 0x9);
    proc.set_profile(PresentationProfile::measured(120.0, 0.98, 0.2));
    proc.set_target_image(
        |lx, ly| {
            let v = 100u8 + 3 * ly as u8 + lx as u8;
            [v, v, 128]
        },
        ResetPolicy::Reset,
    );
    // advance() returns an ordinary Surface (static or temporal); it must flow
    // through the existing headless renderer unchanged.
    let surface = proc.advance(0);
    assert_eq!((surface.width, surface.height), (3, 2));

    let mut ctx = Context::headless(RenderMode::Inline, 20, 5);
    ctx.set_sync_updates(false);
    ctx.set_root(Node::surface(std::sync::Arc::new(surface)));
    ctx.render_now().unwrap();
    assert!(ctx.last_frame_report().bytes_emitted > 0);
}
