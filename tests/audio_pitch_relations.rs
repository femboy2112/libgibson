//! Exhaustive finite membership equivalence and source/observer boundary controls.
use gibson::audio::human_music::{
    fingerprint::CanonicalFingerprint,
    performance::AccentGrid,
    pitch::{classify, pc_mask, source_stable_function, PitchContext},
    score::PitchFunction,
    theory::{pitch_class, Chord, Mode, PitchClassSet, Quality, Scale},
};

const QUALITIES: [Quality; 18] = [
    Quality::Maj,
    Quality::Min,
    Quality::Dim,
    Quality::Aug,
    Quality::Maj7,
    Quality::Min7,
    Quality::Dom7,
    Quality::Min7b5,
    Quality::Dim7,
    Quality::MinMaj7,
    Quality::Sus4,
    Quality::Sus2,
    Quality::Maj9,
    Quality::Min9,
    Quality::Dom9,
    Quality::Add9,
    Quality::Maj6,
    Quality::Min6,
];

#[test]
fn all_chord_qualities_roots_and_pitch_classes_match_historical_relations() {
    for quality in QUALITIES {
        for root in 0..12 {
            let chord = Chord::new(root, quality);
            let historical: Vec<_> = quality
                .intervals()
                .iter()
                .map(|interval| (root + interval).rem_euclid(12))
                .collect();
            let old_mask = historical
                .iter()
                .fold(0_u16, |mask, pc| mask | (1_u16 << pc));
            assert_eq!(
                chord.pitch_classes(),
                historical,
                "ordered tones stay ordered"
            );
            assert_eq!(chord.pitch_class_set().bits(), old_mask);
            assert_eq!(pc_mask(&historical), old_mask);
            for pc in 0..12 {
                for octave in -2..=10 {
                    let pitch = pc + 12 * octave;
                    let member = historical.contains(&pc);
                    assert_eq!(chord.contains_pc(pitch), member);
                    assert_eq!(chord.pitch_class_set().contains(pitch), member);
                    let old_inner_loop = quality
                        .intervals()
                        .iter()
                        .any(|interval| (root + interval).rem_euclid(12) == pitch_class(pitch));
                    assert_eq!(member, old_inner_loop);
                    let old_nearest = (0..=12)
                        .flat_map(|distance| [pitch - distance, pitch + distance])
                        .find(|candidate| historical.contains(&pitch_class(*candidate)))
                        .unwrap_or(pitch);
                    assert_eq!(chord.nearest_chord_tone(pitch), old_nearest);
                }
            }
        }
    }
}

#[test]
fn source_classification_preserves_chord_precedence_and_literal_palette_membership() {
    for quality in QUALITIES {
        for root in 0..12 {
            let chord = Chord::new(root, quality);
            let old_chord = chord.pitch_classes();
            for licensed_pc in -1..=13 {
                let licensed = [licensed_pc];
                for pitch in -12..=131 {
                    let pc = pitch_class(pitch);
                    let historical = if old_chord.contains(&pc) {
                        Some(PitchFunction::ChordTone)
                    } else if licensed.contains(&pc) {
                        Some(PitchFunction::LicensedExtension)
                    } else {
                        None
                    };
                    assert_eq!(source_stable_function(chord, &licensed, pitch), historical);
                }
            }
        }
    }
}

#[test]
fn static_source_membership_does_not_certify_temporal_sustain() {
    let c = Chord::new(0, Quality::Maj);
    assert_eq!(
        source_stable_function(c, &[], 64),
        Some(PitchFunction::ChordTone)
    );
    let context = PitchContext {
        pitch: 64,
        onset: 0.0,
        duration: 2.0,
        prev: None,
        next: None,
        next_onset: None,
        cur: Some(c),
        prev_chord: None,
        next_chord: Some(Chord::new(1, Quality::Maj)),
        next_boundary: Some(1.0),
        is_strong: true,
        licensed: 0,
    };
    assert_eq!(classify(&context, &Scale::new(0, Mode::Ionian)), None);
}

#[test]
fn set_identity_forgets_order_and_octaves_but_not_membership() {
    let a = PitchClassSet::from_pitches(&[60, 64, 67, 72]);
    let b = PitchClassSet::from_pitches(&[7, 4, 0]);
    assert_eq!(a, b);
    assert_eq!(a.canonical_fingerprint(), b.canonical_fingerprint());
    assert_ne!(a, PitchClassSet::from_pitches(&[0, 3, 7]));
    assert_eq!(PitchClassSet::from_bits(0xf000), PitchClassSet::EMPTY);
    assert_eq!(
        pc_mask(&[i32::MIN, i32::MAX]),
        (1_u16 << pitch_class(i32::MIN)) | (1_u16 << pitch_class(i32::MAX))
    );
}

#[test]
fn accent_grid_declares_nearest_projection_and_exact_metric_identity() {
    assert_eq!(AccentGrid::step_of(0.124), (0, 0));
    assert_eq!(AccentGrid::step_of(0.125), (0, 1));
    assert_eq!(AccentGrid::step_of(3.875), (1, 0));
    for bar in [0, 1, 37, u32::MAX] {
        for step in 0..16 {
            let metric = AccentGrid::metric_of(bar, step).unwrap();
            assert_eq!(metric.beats(), AccentGrid::beat_of(bar, step));
        }
    }
}
