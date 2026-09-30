//! A source-authority boundary for structured reference songs.
//!
//! The generic TSV adapter imports explicit symbolic events. It does not transcribe
//! audio, infer missing melody, guess harmony, or treat a chord chart as a lead sheet.
//! Provenance stays with the reference/receipt; generation receives only `CoverMap`.
use super::cover::{
    CoverAxis, CoverChord, CoverError, CoverFidelityPreset, CoverFidelityProfile, CoverLine,
    CoverMap, CoverNote, CoverSpec, FidelityReport, HarmonyRelation, LineRelation,
};
use super::rhythm::MetricPosition;
use super::score::{Note, Provenance, Role, Score};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceNote {
    pub at: MetricPosition,
    pub duration: MetricPosition,
    pub pitch: i32,
}
/// All notes of a named source voice. Retained observations never enter the cover
/// generator unless extraction explicitly promotes them into an invariant axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceVoice {
    pub label: String,
    pub notes: Vec<ReferenceNote>,
}
/// Explicit source observations. `None` means unobserved, never a free variation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceMode {
    Major,
    Minor,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceSong {
    pub length: MetricPosition,
    pub tonic: i32,
    pub mode: ReferenceMode,
    pub meter: (u32, u32),
    pub tempo: Option<f32>,
    pub selected_voice: String,
    pub voices: Vec<ReferenceVoice>,
}
fn fraction(text: &str) -> Result<MetricPosition, CoverError> {
    let (a, b) = text.split_once('/').ok_or(CoverError::Invalid(
        "expected rational numerator/denominator",
    ))?;
    MetricPosition::new(
        a.parse()
            .map_err(|_| CoverError::Invalid("rational numerator"))?,
        b.parse()
            .map_err(|_| CoverError::Invalid("rational denominator"))?,
    )
    .ok_or(CoverError::Invalid("zero denominator"))
}
impl ReferenceSong {
    /// Format: meter N D; key PC major|minor; tempo BPM; length N/D;
    /// note VOICE ONSET_N/D DURATION_N/D MIDI. Fields are whitespace-separated.
    /// Every note is validated; the caller explicitly selects one monophonic voice.
    pub fn from_tsv(text: &str, voice: &str) -> Result<Self, CoverError> {
        let (mut meter, mut tonic, mut tempo, mut length, mut mode) =
            (None, None, None, None, None);
        let mut selected = Vec::new();
        let mut all = Vec::new();
        let mut voices = Vec::<ReferenceVoice>::new();
        for line in text
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        {
            let fields: Vec<_> = line.split_whitespace().collect();
            match fields.as_slice() {
                ["meter", n, d] if meter.is_none() => {
                    meter = Some((
                        n.parse().map_err(|_| CoverError::Invalid("meter"))?,
                        d.parse().map_err(|_| CoverError::Invalid("meter"))?,
                    ));
                }
                ["key", pc, kind] if tonic.is_none() && matches!(*kind, "major" | "minor") => {
                    let pc: i32 = pc.parse().map_err(|_| CoverError::Invalid("key"))?;
                    if !(0..12).contains(&pc) {
                        return Err(CoverError::Invalid("key pitch class"));
                    }
                    tonic = Some(pc);
                    mode = Some(if *kind == "major" {
                        ReferenceMode::Major
                    } else {
                        ReferenceMode::Minor
                    });
                }
                ["tempo", value] if tempo.is_none() => {
                    let bpm: f32 = value.parse().map_err(|_| CoverError::Invalid("tempo"))?;
                    if !bpm.is_finite() || bpm <= 0.0 {
                        return Err(CoverError::Invalid("tempo"));
                    }
                    tempo = Some(bpm);
                }
                ["length", value] if length.is_none() => length = Some(fraction(value)?),
                ["note", part, at, duration, pitch] => {
                    let pitch: i32 = pitch
                        .parse()
                        .map_err(|_| CoverError::Invalid("MIDI pitch"))?;
                    let note = ReferenceNote {
                        at: fraction(at)?,
                        duration: fraction(duration)?,
                        pitch,
                    };
                    if !(0..128).contains(&pitch)
                        || note.at.beats() < 0.0
                        || note.duration.beats() <= 0.0
                    {
                        return Err(CoverError::Invalid("note domain"));
                    }
                    all.push(note.clone());
                    if let Some(part_notes) = voices.iter_mut().find(|v| v.label == *part) {
                        part_notes.notes.push(note.clone());
                    } else {
                        voices.push(ReferenceVoice {
                            label: (*part).into(),
                            notes: vec![note.clone()],
                        });
                    }
                    if *part == voice {
                        selected.push(note);
                    }
                }
                _ => {
                    return Err(CoverError::Invalid(
                        "unknown, duplicate or malformed reference record",
                    ))
                }
            }
        }
        let length = length.ok_or(CoverError::Invalid("missing length"))?;
        let meter = meter.ok_or(CoverError::Invalid("missing meter"))?;
        let tonic = tonic.ok_or(CoverError::Invalid("missing key"))?;
        if meter != (4, 4) {
            return Err(CoverError::Invalid("current realization supports 4/4 only"));
        }
        if length.beats() <= 0.0
            || all
                .iter()
                .any(|n| add(n.at, n.duration).map_or(true, |end| end > length))
        {
            return Err(CoverError::Invalid("note outside reference extent"));
        }
        selected.sort_by_key(|n| n.at);
        for part in &mut voices {
            part.notes.sort_by_key(|n| (n.at, n.pitch, n.duration));
        }
        voices.sort_by(|a, b| a.label.cmp(&b.label));
        if selected.is_empty() {
            return Err(CoverError::Invalid("selected voice absent"));
        }
        if selected
            .windows(2)
            .any(|p| add(p[0].at, p[0].duration).map_or(true, |end| end > p[1].at))
        {
            return Err(CoverError::Invalid(
                "selected melody must be explicitly monophonic",
            ));
        }
        Ok(Self {
            length,
            tonic,
            mode: mode.ok_or(CoverError::Invalid("missing mode"))?,
            meter,
            tempo,
            selected_voice: voice.into(),
            voices,
        })
    }
    /// The explicitly selected source voice; no independent melody cache.
    pub fn melody(&self) -> Option<&[ReferenceNote]> {
        self.voices
            .iter()
            .find(|v| v.label == self.selected_voice)
            .map(|v| v.notes.as_slice())
    }
    /// Preserve observed selected axes; missing source axes remain Unknown. Free
    /// observed melody is deliberately discarded, and never retained as a fallback.
    pub fn extract(&self, spec: CoverSpec) -> Result<CoverMap, CoverError> {
        if !(0..12).contains(&self.tonic)
            || self.meter != (4, 4)
            || self.length.beats() <= 0.0
            || self.tempo.is_some_and(|t| !t.is_finite() || t <= 0.0)
        {
            return Err(CoverError::Invalid("reference metadata domain"));
        }
        if let Some(notes) = self.melody() {
            for n in notes {
                let end = add(n.at, n.duration)?;
                if n.at.beats() < 0.0
                    || n.duration.beats() <= 0.0
                    || end > self.length
                    || !(0..128).contains(&n.pitch)
                {
                    return Err(CoverError::Invalid("reference note bounds"));
                }
            }
        }
        let observed = self.melody().filter(|m| !m.is_empty());
        let known = [CoverAxis::Motif, CoverAxis::Riff];
        let effective = CoverSpec::new(
            spec.axes()
                .iter()
                .copied()
                .filter(|a| observed.is_some() && known.contains(a)),
        );
        let line = observed.map(|notes| {
            let p = notes[0].pitch - self.tonic;
            let origin = p - p.rem_euclid(12);
            CoverLine {
                role: Role::Lead,
                notes: notes
                    .iter()
                    .map(|n| CoverNote {
                        at: n.at,
                        relative_pitch: n.pitch - self.tonic - origin,
                        reserved_until: Some(
                            add(n.at, n.duration)
                                .expect("reference rational sums validated before extraction"),
                        ),
                    })
                    .collect(),
            }
        });
        let map = CoverMap {
            unknown_axes: CoverAxis::ALL
                .into_iter()
                .filter(|a| !known.contains(a) || observed.is_none())
                .collect(),
            ordered_chart: None,
            length: Some(self.length),
            form: None,
            motif: effective
                .contains(CoverAxis::Motif)
                .then(|| line.clone())
                .flatten(),
            riff: effective
                .contains(CoverAxis::Riff)
                .then_some(line)
                .flatten(),
            harmony: None,
            groove: None,
            bass: None,
            orchestration: None,
            fidelity: None,
            projection: super::cover::CoverProjection::Lane,
            spec: effective,
        };
        map.validate()?;
        Ok(map)
    }
    /// Renderable isolated observed melody, with literal source gates. This is a
    /// source-melody audition, not a claimed reconstruction of unknown accompaniment.
    pub fn melody_score(&self) -> Result<Score, CoverError> {
        self.extract(CoverSpec::none())?;
        let melody = self
            .melody()
            .ok_or(CoverError::MissingAxis(CoverAxis::Motif))?;
        let mut score = Score::new(
            self.tempo
                .ok_or(CoverError::Invalid("source tempo unobserved"))?,
            4.0,
            self.length.beats(),
        );
        score.notes = melody
            .iter()
            .map(|n| {
                Note::new(
                    n.at.beats(),
                    n.duration.beats() as f32,
                    n.pitch,
                    0.75,
                    Role::Lead,
                    Provenance::new(super::form::SectionKind::A),
                )
            })
            .collect();
        Ok(score)
    }
}

/// A harmonic reading of the reference's simultaneities. **Derived analysis, not source
/// metadata**: the symbolic source states pitches and onsets, never chord symbols; these spans are
/// what our analyzer reads from them, with the method named. They enter a `CoverMap` only at an
/// explicitly requested harmony relation, and every receipt labels them derived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedHarmony {
    pub method: &'static str,
    pub window: MetricPosition,
    pub chords: Vec<CoverChord>,
}

/// The declared analyzer: fixed windows; per window, the triad whose members cover the most
/// sounding note-duration (every voice; ties: the lowest sounding pitch as root, then the lower
/// root); a seventh is added only when that pitch class sounds for at least half the window (a
/// passing tone is not a chord member); equal neighbours merge; a window with no sounding note
/// continues the previous chord.
pub const DERIVED_HARMONY_METHOD: &str =
    "satb-window-triad/v2: max covered duration triad; structural seventh >= half window; bass-root ties; merge equal";

impl ReferenceSong {
    /// Derive a chord-span reading from every voice (see [`DERIVED_HARMONY_METHOD`]).
    pub fn derive_harmony(&self, window: MetricPosition) -> Result<DerivedHarmony, CoverError> {
        use super::theory::Quality as Q;
        if window.beats() <= 0.0 {
            return Err(CoverError::Invalid("analysis window"));
        }
        let triads = [Q::Maj, Q::Min, Q::Dim, Q::Aug];
        let mut spans: Vec<CoverChord> = Vec::new();
        let mut at = super::rhythm::MetricPosition::new(0, 1).expect("zero");
        while at < self.length {
            let end = add(at, window)?.min(self.length);
            let (a, e) = (at.beats(), end.beats());
            // (pitch class, sounding duration inside the window, summed over voices) and the
            // lowest pitch sounding at the window's start.
            let mut weight = [0.0f64; 12];
            let mut lowest: Option<i32> = None;
            for voice in &self.voices {
                for n in &voice.notes {
                    let (s, t) = (n.at.beats(), add(n.at, n.duration)?.beats());
                    let overlap = t.min(e) - s.max(a);
                    if overlap > 1e-9 {
                        weight[n.pitch.rem_euclid(12) as usize] += overlap;
                        if s <= a + 1e-9 || lowest.is_none() {
                            lowest = Some(lowest.map_or(n.pitch, |l| l.min(n.pitch)));
                        }
                    }
                }
            }
            let chord = if weight.iter().all(|&w| w <= 0.0) {
                spans.last().map(|c| (c.relative_root, c.quality))
            } else {
                let bass_pc = lowest.map(|p| p.rem_euclid(12));
                let w = |pc: i32| weight[pc.rem_euclid(12) as usize];
                // 1. The triad covering the most sounding duration; ties: the lowest sounding
                //    pitch as root, then the lower root.
                let mut best: Option<(f64, bool, i32, Q)> = None;
                for root in 0..12 {
                    for &q in &triads {
                        let covered: f64 = q.intervals().iter().map(|o| w(root + o)).sum();
                        let key = (covered, bass_pc == Some(root), -root, q);
                        let better = best.is_none_or(|b| {
                            (key.0 - b.0).abs() > 1e-9 && key.0 > b.0
                                || (key.0 - b.0).abs() <= 1e-9 && (key.1, key.2) > (b.1, b.2)
                        });
                        if better {
                            best = Some(key);
                        }
                    }
                }
                best.map(|(_, _, neg_root, triad)| {
                    let root = -neg_root;
                    // 2. A seventh only when it is structural: sounding for at least half the
                    //    window (a passing tone is not a chord member).
                    let structural = |offset: i32| w(root + offset) >= (e - a) * 0.5 - 1e-9;
                    let quality = match triad {
                        Q::Maj if structural(10) => Q::Dom7,
                        Q::Maj if structural(11) => Q::Maj7,
                        Q::Min if structural(10) => Q::Min7,
                        Q::Dim if structural(10) => Q::Min7b5,
                        q => q,
                    };
                    ((root - self.tonic).rem_euclid(12), quality)
                })
            };
            let Some((relative_root, quality)) = chord else {
                return Err(CoverError::MissingAxis(CoverAxis::HarmonicContour));
            };
            match spans.last_mut() {
                Some(prev) if prev.relative_root == relative_root && prev.quality == quality => {
                    prev.end = end;
                }
                _ => spans.push(CoverChord {
                    at,
                    end,
                    relative_root,
                    quality,
                }),
            }
            at = end;
        }
        Ok(DerivedHarmony {
            method: DERIVED_HARMONY_METHOD,
            window,
            chords: spans,
        })
    }

    /// Extract at a fidelity profile. Observed: the selected voice (melody) and, when present, a
    /// voice labelled `bass` (bass line), both with their notated note/rest boundaries.
    /// Harmony only from an explicitly supplied [`DerivedHarmony`] (labelled derived in the
    /// report). Groove, form and seating are unobserved and stay Unknown at every setting.
    pub fn extract_fidelity(
        &self,
        requested: &CoverFidelityProfile,
        preset: Option<CoverFidelityPreset>,
        derived: Option<&DerivedHarmony>,
    ) -> Result<(CoverMap, FidelityReport), CoverError> {
        let base = self.extract(CoverSpec::none())?;
        let line = |notes: &[ReferenceNote], role: Role| -> CoverLine {
            let p = notes[0].pitch - self.tonic;
            let origin = p - p.rem_euclid(12);
            CoverLine {
                role,
                notes: notes
                    .iter()
                    .map(|n| CoverNote {
                        at: n.at,
                        relative_pitch: n.pitch - self.tonic - origin,
                        reserved_until: Some(
                            add(n.at, n.duration)
                                .expect("reference rational sums validated before extraction"),
                        ),
                    })
                    .collect(),
            }
        };
        let melody = self.melody().filter(|m| !m.is_empty());
        let bass_voice = self
            .voices
            .iter()
            .find(|v| v.label == "bass" && v.label != self.selected_voice)
            .filter(|v| {
                !v.notes.is_empty()
                    && v.notes
                        .windows(2)
                        .all(|p| add(p[0].at, p[0].duration).is_ok_and(|end| end <= p[1].at))
            });
        let observed = |present: bool| -> (Option<LineRelation>, &'static str) {
            if present {
                (
                    Some(LineRelation::Faithful),
                    "observed notes with notated rests",
                )
            } else {
                (None, "no such voice observed")
            }
        };
        let available = super::cover::fidelity_availability(
            [
                observed(melody.is_some()),
                observed(melody.is_some()),
                observed(bass_voice.is_some()),
            ],
            derived.map(|d| d.method),
        );
        let (effective, unknown, report) =
            super::cover::fidelity_ceiling(requested, preset, &available)?;
        let mut map = CoverMap {
            unknown_axes: unknown,
            ordered_chart: None,
            length: base.length,
            form: None,
            motif: (effective.motif != LineRelation::Free)
                .then(|| melody.map(|m| line(m, Role::Lead)))
                .flatten(),
            riff: (effective.riff != LineRelation::Free)
                .then(|| melody.map(|m| line(m, Role::Lead)))
                .flatten(),
            harmony: (effective.harmony != HarmonyRelation::Free)
                .then(|| derived.map(|d| d.chords.clone()))
                .flatten(),
            groove: None,
            bass: (effective.bass != LineRelation::Free)
                .then(|| bass_voice.map(|v| line(&v.notes, Role::Bass)))
                .flatten(),
            orchestration: None,
            fidelity: None,
            projection: super::cover::CoverProjection::Lane,
            spec: effective.spec(),
        };
        map.apply_fidelity(&effective);
        map.validate()?;
        Ok((map, report))
    }
}

fn add(a: MetricPosition, b: MetricPosition) -> Result<MetricPosition, CoverError> {
    let denominator = u64::from(a.subdivision()) * u64::from(b.subdivision());
    let numerator = i128::from(a.ticks()) * i128::from(b.subdivision())
        + i128::from(b.ticks()) * i128::from(a.subdivision());
    let mut x = numerator.unsigned_abs();
    let mut y = u128::from(denominator);
    while y != 0 {
        (x, y) = (y, x % y);
    }
    let ticks = i64::try_from(numerator / x as i128)
        .map_err(|_| CoverError::Invalid("reference rational sum overflow"))?;
    let division = u32::try_from(u128::from(denominator) / x)
        .map_err(|_| CoverError::Invalid("reference rational denominator overflow"))?;
    MetricPosition::new(ticks, division).ok_or(CoverError::Invalid("reference rational sum"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn omitted_harmony_is_unknown_and_freed_melody_is_free() {
        let r=ReferenceSong::from_tsv("meter 4 4\nkey 0 major\ntempo 100\nlength 4/1\nnote tune 0/1 1/1 60\nnote tune 1/1 1/1 64\n","tune").unwrap();
        let map = r.extract(CoverSpec::none()).unwrap();
        assert_eq!(
            map.knowledge(CoverAxis::Motif),
            super::super::cover::CoverKnowledge::Free
        );
        assert_eq!(
            map.knowledge(CoverAxis::HarmonicContour),
            super::super::cover::CoverKnowledge::Unknown
        );
        assert!(map.motif.is_none());
    }
    #[test]
    fn excluded_voices_remain_observed_without_entering_the_quotient() {
        let text =
            "meter 4 4\nkey 0 minor\nlength 4/1\nnote tune 0/1 1/1 60\nnote bass 0/1 4/1 36\n";
        let a = ReferenceSong::from_tsv(text, "tune").unwrap();
        let b = ReferenceSong::from_tsv(&text.replace("4/1 36", "4/1 43"), "tune").unwrap();
        assert_ne!(a.voices, b.voices);
        assert_eq!(a.mode, ReferenceMode::Minor);
        assert_eq!(
            a.extract(CoverSpec::new([CoverAxis::Motif])).unwrap(),
            b.extract(CoverSpec::new([CoverAxis::Motif])).unwrap()
        );
        assert!(add(
            MetricPosition::new(i64::MAX, 1).unwrap(),
            MetricPosition::new(1, 1).unwrap()
        )
        .is_err());
        assert!(add(
            MetricPosition::new(1, 4294967291).unwrap(),
            MetricPosition::new(1, 4294967279).unwrap()
        )
        .is_err());
    }
    #[test]
    fn malformed_and_polyphonic_sources_are_rejected() {
        let base = "meter 4 4\nkey 0 major\nlength 4/1\nnote tune 0/1 2/1 60\n";
        assert!(ReferenceSong::from_tsv(&format!("{base}note tune 1/1 1/1 64"), "tune").is_err());
        assert!(ReferenceSong::from_tsv(&format!("{base}note other 2/0 1/1 64"), "tune").is_err());
        assert!(ReferenceSong::from_tsv(base, "guessed-voice").is_err());
    }
}
