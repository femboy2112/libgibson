//! The primitive relations a pinned identity holds in an actual cover, read from what the cover
//! REALIZED — never by re-running the extractor that built the expected map.
//!
//! Extraction builds the quotient value from a source (the identity projection decides which
//! events count). Verification inspects the target's realized events and checks each relation
//! against the quotient directly:
//!
//! - **line** — the events the lift realized from the pin (the lift's own [`PINNED_IDENTITY`]
//!   mark) are in bijection with the pinned notes: each at the pin's metric position carried
//!   forward by the target's declared groove transport, at the pin's relative pitch under one
//!   global octave, inside every observed rest boundary;
//! - **groove** — the strokes the lift realized from the pin, kick for kick, snare for snare, at
//!   the forward-transported positions;
//! - **harmony** — the target's chord spans at exactly the pinned spans, relative roots and
//!   qualities (or quality families);
//! - **form** — the target's phrase topology and the sections that sound;
//! - **orchestration** — the target's per-bar seated roles, and nobody sounding off stage.
//!
//! A bug in what the extractor counts as a motif cannot make this verifier agree with it: an
//! unrelated note in a pinned place is not the pin realized.

use super::*;
use crate::audio::human_music::projection::PINNED_IDENTITY;

fn transport_of(actual: &Composition) -> Result<GrooveTransport, String> {
    actual
        .perf
        .cover_constraints
        .as_ref()
        .map(|c| c.transport)
        .ok_or_else(|| "the actual performance is not a cover (no lift to verify)".to_string())
}

/// The pinned `line` holds in `actual` (a cover lifted under `world`).
pub(super) fn line(
    expected: &CoverLine,
    actual: &Composition,
    world: &MusicWorld,
) -> Result<(), String> {
    let transport = transport_of(actual)?;
    let realized: Vec<&Note> = actual
        .score
        .role_notes(expected.role)
        .filter(|n| n.prov.role_note == PINNED_IDENTITY)
        .collect();
    if realized.len() != expected.notes.len() {
        return Err(format!(
            "{} pinned {:?} events, {} realized from the pin",
            expected.notes.len(),
            expected.role,
            realized.len()
        ));
    }
    let mut octave: Option<i32> = None;
    for pin in &expected.notes {
        let at = transport.transport(pin.at).beats();
        let Some(n) = realized
            .iter()
            .find(|n| n.start_beat.to_bits() == at.to_bits())
        else {
            return Err(format!("no realized pin at {} (metric {:?})", at, pin.at));
        };
        let offset = n.pitch - world.tonic_pc - pin.relative_pitch;
        if offset.rem_euclid(12) != 0 {
            return Err(format!(
                "pin at {at}: pitch {} is not the pinned relative pitch",
                n.pitch
            ));
        }
        if *octave.get_or_insert(offset) != offset {
            return Err(format!(
                "pin at {at}: the line changes octave (one global octave is free)"
            ));
        }
        if let Some(end) = pin.reserved_until {
            let limit = transport.transport(end).beats();
            if n.start_beat + f64::from(n.dur_beats) > limit + 1e-9 {
                return Err(format!(
                    "pin at {at} sounds through its observed rest boundary"
                ));
            }
        }
    }
    Ok(())
}

/// The pinned groove holds in `actual`.
pub(super) fn groove(expected: &[CoverStroke], actual: &Composition) -> Result<(), String> {
    let transport = transport_of(actual)?;
    let mut realized: Vec<(u64, GrooveVoice)> = actual
        .score
        .drums
        .iter()
        .filter(|d| d.prov.groove_variation == Some("cover-cell"))
        .filter_map(|d| match d.voice {
            DrumVoice::Kick => Some((d.start_beat.to_bits(), GrooveVoice::Kick)),
            DrumVoice::Snare => Some((d.start_beat.to_bits(), GrooveVoice::Snare)),
            _ => None,
        })
        .collect();
    let mut pinned: Vec<(u64, GrooveVoice)> = expected
        .iter()
        .map(|s| (transport.transport(s.at).beats().to_bits(), s.voice))
        .collect();
    let key = |x: &(u64, GrooveVoice)| (f64::from_bits(x.0), matches!(x.1, GrooveVoice::Snare));
    realized.sort_by(|a, b| key(a).partial_cmp(&key(b)).unwrap());
    pinned.sort_by(|a, b| key(a).partial_cmp(&key(b)).unwrap());
    if realized != pinned {
        return Err(format!(
            "{} pinned strokes, {} realized from the pin, or at other positions",
            pinned.len(),
            realized.len()
        ));
    }
    Ok(())
}

/// The pinned harmony holds in `actual`'s sounding chord spans.
pub(super) fn harmony(
    expected: &[CoverChord],
    by_family: bool,
    actual: &Composition,
    world: &MusicWorld,
) -> Result<(), String> {
    let spans = &actual.score.chords;
    if spans.len() != expected.len() {
        return Err(format!(
            "{} pinned spans, {} sounding",
            expected.len(),
            spans.len()
        ));
    }
    for (pin, span) in expected.iter().zip(spans) {
        let end = span.start_beat + f64::from(span.dur_beats);
        if span.start_beat.to_bits() != pin.at.beats().to_bits()
            || (end - pin.end.beats()).abs() > 1e-6
        {
            return Err(format!(
                "span at {} is not the pinned span {:?}..{:?}",
                span.start_beat, pin.at, pin.end
            ));
        }
        if (span.chord.root_pc - world.tonic_pc).rem_euclid(12) != pin.relative_root {
            return Err(format!("span at {}: another root", span.start_beat));
        }
        let quality = if by_family {
            QualityFamily::of(span.chord.quality) == QualityFamily::of(pin.quality)
        } else {
            span.chord.quality == pin.quality
        };
        if !quality {
            return Err(format!("span at {}: another quality", span.start_beat));
        }
    }
    Ok(())
}

/// The pinned phrase topology holds in `actual`'s form and in the sections that sound.
pub(super) fn form(expected: &[CoverPhrase], actual: &Composition) -> Result<(), String> {
    let planned: Vec<_> = actual
        .song
        .plan
        .form
        .phrases
        .iter()
        .map(|p| (p.start_bar, p.bars, p.family))
        .collect();
    let pinned: Vec<_> = expected
        .iter()
        .map(|p| (p.start_bar, p.bars, p.family))
        .collect();
    if planned != pinned {
        return Err("the cover's phrase topology is not the pinned one".into());
    }
    let sounding: Vec<_> = actual
        .score
        .sections
        .iter()
        .map(|s| (s.start_bar, s.bars, s.kind))
        .collect();
    let expected_sections: Vec<_> = expected
        .iter()
        .map(|p| (p.start_bar, p.bars, p.family.to_section_kind()))
        .collect();
    if sounding != expected_sections {
        return Err("the sections that sound disagree with the pinned form".into());
    }
    Ok(())
}

/// The pinned per-bar seating holds on `actual`'s stage, and nobody sounds off it.
pub(super) fn orchestration(expected: &[CoverSeats], actual: &Composition) -> Result<(), String> {
    let seated: Vec<_> = actual
        .perf
        .stage
        .seats
        .iter()
        .map(|s| s.map(|x| x.role))
        .collect();
    let pinned: Vec<_> = expected.iter().map(|s| s.roles).collect();
    if seated != pinned {
        return Err("the cover seats another per-bar role topology".into());
    }
    let offstage = super::super::functor::orchestration_violations(&actual.perf, &actual.score);
    if !offstage.is_empty() {
        return Err(format!("events sound off stage: {offstage:?}"));
    }
    Ok(())
}
