//! Cover fidelity: how much of a song's identity a cover keeps, chosen deliberately.
//!
//! A preset ([`CoverFidelityPreset`]) is only a convenient way to build a
//! [`CoverFidelityProfile`]: one **exact equivalence relation per axis**. A stronger relation
//! keeps more of the source; none of them is a similarity score. Extraction reports the
//! observational ceiling: an axis the source never observed stays Unknown at every setting,
//! and a relation the source cannot support is lowered to the strongest one it can — with the
//! reason — never filled in.
//!
//! The binary [`CoverSpec`] is the compatibility profile ([`CoverFidelityProfile::from_spec`]):
//! a map extracted at it is byte-for-byte the v1 map, with the v1 hash.
use super::*;

/// The global "how recognizably THIS song" dial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoverFidelityPreset {
    /// The source's opening statement becomes the cover's theme; everything else is the band's.
    Loose,
    /// The whole melody, its harmony by chord family, the pocket skeleton.
    Interpretive,
    /// The melody with its observed rests, exact harmony, the full kick/snare part, the form.
    Faithful,
    /// Everything observed, exactly: melody, bass line, harmony, groove, form, seating.
    Strict,
}

impl CoverFidelityPreset {
    pub const ALL: [CoverFidelityPreset; 4] = [
        CoverFidelityPreset::Loose,
        CoverFidelityPreset::Interpretive,
        CoverFidelityPreset::Faithful,
        CoverFidelityPreset::Strict,
    ];
    pub fn label(self) -> &'static str {
        match self {
            CoverFidelityPreset::Loose => "loose",
            CoverFidelityPreset::Interpretive => "interpretive",
            CoverFidelityPreset::Faithful => "faithful",
            CoverFidelityPreset::Strict => "strict",
        }
    }
}

/// Exact relations on a pinned line (Motif, Riff, BassFigure), weakest to strongest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LineRelation {
    Free,
    /// Motif only: the line's first [`THEME_BEATS`] beats — exact relative pitches and exact
    /// inter-onset rhythm — become the cover song's identity motif, and the cover performs at least
    /// one statement the song derives from it. How the band's own composer places, develops and
    /// voices that material is free: material from the source, not the source line.
    Theme,
    /// The whole line: exact relative pitches (global octave free) at exact canonical onsets.
    /// Note lengths and rests are the band's.
    Metric,
    /// [`Self::Metric`] plus every observed note/rest boundary: a gate may shorten, never fill a
    /// rest. Only where boundaries were observed.
    Faithful,
}

/// Exact relations on the harmony (HarmonicContour/HarmonicLoop share the chord spans).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HarmonyRelation {
    Free,
    /// The ordered sequence of relative roots and exact qualities, without durations (the only
    /// relation an unmetered chord chart supports).
    Ordered,
    /// Exact spans and relative roots; the quality only up to its [`QualityFamily`] (the target
    /// vocabulary chooses the member: a pinned Dom7 may sound as a major triad).
    QualityFamily,
    /// Exact spans, relative roots and qualities.
    Exact,
}

/// Exact relations on the kick/snare part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GrooveRelation {
    Free,
    /// The kick and snare strokes on the quarter-note beat grid; between beats is the band's.
    PocketSkeleton,
    /// Every kick and snare stroke at its canonical position.
    KickSnare,
}

/// Exact relations on the form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormRelation {
    Free,
    /// The ordered section-family sequence without bar counts (unmetered chart only).
    Topology,
    /// Exact phrase spans and families.
    Exact,
}

/// Exact relations on the seating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrchestrationRelation {
    Free,
    /// Every bar's arrangement role per seat.
    Exact,
}

/// A chord's family by its triad: what a quality-family relation preserves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QualityFamily {
    Major,
    Minor,
    Diminished,
    Augmented,
    Suspended,
}

impl QualityFamily {
    pub fn of(q: Quality) -> Self {
        match q {
            Quality::Maj
            | Quality::Maj7
            | Quality::Dom7
            | Quality::Maj9
            | Quality::Dom9
            | Quality::Add9
            | Quality::Maj6 => QualityFamily::Major,
            Quality::Min | Quality::Min7 | Quality::MinMaj7 | Quality::Min9 | Quality::Min6 => {
                QualityFamily::Minor
            }
            Quality::Dim | Quality::Min7b5 | Quality::Dim7 => QualityFamily::Diminished,
            Quality::Aug => QualityFamily::Augmented,
            Quality::Sus4 | Quality::Sus2 => QualityFamily::Suspended,
        }
    }
    /// The family's members, simplest first (the order a target vocabulary is searched).
    pub fn members(self) -> &'static [Quality] {
        match self {
            QualityFamily::Major => &[
                Quality::Maj,
                Quality::Dom7,
                Quality::Maj7,
                Quality::Maj6,
                Quality::Add9,
                Quality::Dom9,
                Quality::Maj9,
            ],
            QualityFamily::Minor => &[
                Quality::Min,
                Quality::Min7,
                Quality::Min6,
                Quality::Min9,
                Quality::MinMaj7,
            ],
            QualityFamily::Diminished => &[Quality::Dim, Quality::Min7b5, Quality::Dim7],
            QualityFamily::Augmented => &[Quality::Aug],
            QualityFamily::Suspended => &[Quality::Sus4, Quality::Sus2],
        }
    }
}

/// The theme window of [`LineRelation::Theme`]: two 4/4 bars from the line's first attack.
pub const THEME_BEATS: f64 = 8.0;

/// Which harmonic identity label a pinned harmony carries (the same chord spans either way).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HarmonyAxis {
    Contour,
    Loop,
    Both,
}

/// One exact relation per axis. Build it from a preset, from a binary spec, or field by field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverFidelityProfile {
    pub motif: LineRelation,
    pub riff: LineRelation,
    /// The riff's lane (Lead or Bass). `None` keeps the documented default.
    pub riff_lane: Option<Role>,
    pub bass: LineRelation,
    pub harmony: HarmonyRelation,
    /// The label a pinned harmony carries (v1 specs may pin it as a loop).
    pub harmony_axis: HarmonyAxis,
    pub groove: GrooveRelation,
    pub form: FormRelation,
    pub orchestration: OrchestrationRelation,
}

impl CoverFidelityProfile {
    pub const FREE: Self = Self {
        motif: LineRelation::Free,
        riff: LineRelation::Free,
        riff_lane: None,
        bass: LineRelation::Free,
        harmony: HarmonyRelation::Free,
        harmony_axis: HarmonyAxis::Contour,
        groove: GrooveRelation::Free,
        form: FormRelation::Free,
        orchestration: OrchestrationRelation::Free,
    };

    /// The preset, compiled into explicit relations.
    pub const fn preset(preset: CoverFidelityPreset) -> Self {
        use CoverFidelityPreset as P;
        match preset {
            P::Loose => Self {
                motif: LineRelation::Theme,
                ..Self::FREE
            },
            P::Interpretive => Self {
                motif: LineRelation::Metric,
                riff: LineRelation::Metric,
                harmony: HarmonyRelation::QualityFamily,
                groove: GrooveRelation::PocketSkeleton,
                ..Self::FREE
            },
            P::Faithful => Self {
                motif: LineRelation::Faithful,
                riff: LineRelation::Faithful,
                bass: LineRelation::Metric,
                harmony: HarmonyRelation::Exact,
                groove: GrooveRelation::KickSnare,
                form: FormRelation::Exact,
                ..Self::FREE
            },
            P::Strict => Self {
                motif: LineRelation::Faithful,
                riff: LineRelation::Faithful,
                riff_lane: None,
                bass: LineRelation::Faithful,
                harmony: HarmonyRelation::Exact,
                harmony_axis: HarmonyAxis::Contour,
                groove: GrooveRelation::KickSnare,
                form: FormRelation::Exact,
                orchestration: OrchestrationRelation::Exact,
            },
        }
    }

    /// The binary v1 spec as a profile: each pinned axis at the relation the v1 map used.
    pub fn from_spec(spec: &CoverSpec) -> Self {
        let line = |axis| {
            if spec.contains(axis) {
                LineRelation::Faithful
            } else {
                LineRelation::Free
            }
        };
        Self {
            motif: line(CoverAxis::Motif),
            riff: line(CoverAxis::Riff),
            riff_lane: None,
            bass: line(CoverAxis::BassFigure),
            harmony: if spec.contains(CoverAxis::HarmonicContour)
                || spec.contains(CoverAxis::HarmonicLoop)
            {
                HarmonyRelation::Exact
            } else {
                HarmonyRelation::Free
            },
            harmony_axis: match (
                spec.contains(CoverAxis::HarmonicContour),
                spec.contains(CoverAxis::HarmonicLoop),
            ) {
                (true, true) => HarmonyAxis::Both,
                (false, true) => HarmonyAxis::Loop,
                _ => HarmonyAxis::Contour,
            },
            groove: if spec.contains(CoverAxis::Groove) {
                GrooveRelation::KickSnare
            } else {
                GrooveRelation::Free
            },
            form: if spec.contains(CoverAxis::Form) {
                FormRelation::Exact
            } else {
                FormRelation::Free
            },
            orchestration: if spec.contains(CoverAxis::Orchestration) {
                OrchestrationRelation::Exact
            } else {
                OrchestrationRelation::Free
            },
        }
    }

    /// The axes this profile pins (a non-Free relation).
    pub fn spec(&self) -> CoverSpec {
        let mut axes = Vec::new();
        for (axis, pinned) in [
            (CoverAxis::Motif, self.motif != LineRelation::Free),
            (CoverAxis::Riff, self.riff != LineRelation::Free),
            (CoverAxis::BassFigure, self.bass != LineRelation::Free),
            (
                CoverAxis::HarmonicContour,
                self.harmony != HarmonyRelation::Free && self.harmony_axis != HarmonyAxis::Loop,
            ),
            (
                CoverAxis::HarmonicLoop,
                self.harmony != HarmonyRelation::Free && self.harmony_axis != HarmonyAxis::Contour,
            ),
            (CoverAxis::Groove, self.groove != GrooveRelation::Free),
            (CoverAxis::Form, self.form != FormRelation::Free),
            (
                CoverAxis::Orchestration,
                self.orchestration != OrchestrationRelation::Free,
            ),
        ] {
            if pinned {
                axes.push(axis);
            }
        }
        CoverSpec::new(axes)
    }

    fn validate(&self) -> Result<(), CoverError> {
        if self.riff == LineRelation::Theme || self.bass == LineRelation::Theme {
            return Err(CoverError::Invalid("a theme relation is the melody's"));
        }
        if self
            .riff_lane
            .is_some_and(|r| !matches!(r, Role::Lead | Role::Bass))
        {
            return Err(CoverError::Invalid("riff lane is Lead or Bass"));
        }
        if self.harmony == HarmonyRelation::Ordered || self.form == FormRelation::Topology {
            return Err(CoverError::Invalid(
                "ordered harmony and form topology are what an unmetered chart supports; request exact or family relations",
            ));
        }
        Ok(())
    }

    /// Relation labels, for receipts.
    pub fn line_label(r: LineRelation) -> &'static str {
        match r {
            LineRelation::Free => "free",
            LineRelation::Theme => "theme",
            LineRelation::Metric => "metric",
            LineRelation::Faithful => "faithful-line",
        }
    }
}

/// Where an axis's pinned value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisEvidence {
    /// Read from the source's own events (a generated score, or observed symbolic notes).
    Observed,
    /// Our analyzer's reading of observed events — never presented as source metadata.
    DerivedAnalysis(&'static str),
    /// Not observed: stays Unknown at every fidelity.
    Unknown,
    /// Not requested.
    NotPinned,
}

/// One axis of the observational ceiling: what was asked, what the source supports, why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxisFidelity {
    pub axis: CoverAxis,
    pub requested: &'static str,
    pub effective: &'static str,
    pub evidence: AxisEvidence,
    pub why: &'static str,
}

/// The dial's receipt: the requested profile, the effective one, and each axis's ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FidelityReport {
    pub preset: Option<CoverFidelityPreset>,
    pub requested: CoverFidelityProfile,
    pub effective: CoverFidelityProfile,
    pub axes: Vec<AxisFidelity>,
}

impl FidelityReport {
    /// Whether the source supported everything that was asked.
    pub fn at_ceiling(&self) -> bool {
        self.requested == self.effective
    }
    pub fn report(&self) -> String {
        let mut text = format!(
            "Fidelity: {} requested; {}\n",
            self.preset.map_or("custom profile", |p| p.label()),
            if self.at_ceiling() {
                "every requested relation is supported".to_string()
            } else {
                let lowered: Vec<_> = self
                    .axes
                    .iter()
                    .filter(|a| a.requested != a.effective)
                    .map(|a| format!("{} {}->{}", a.axis.label(), a.requested, a.effective))
                    .collect();
                format!("observational ceiling: {}", lowered.join(", "))
            }
        );
        for a in &self.axes {
            text.push_str(&format!(
                "  {}: requested {}, effective {} ({:?}) — {}\n",
                a.axis.label(),
                a.requested,
                a.effective,
                a.evidence,
                a.why
            ));
        }
        text
    }
}

fn harmony_label(r: HarmonyRelation) -> &'static str {
    match r {
        HarmonyRelation::Free => "free",
        HarmonyRelation::Ordered => "ordered",
        HarmonyRelation::QualityFamily => "quality-family",
        HarmonyRelation::Exact => "exact",
    }
}
fn groove_label(r: GrooveRelation) -> &'static str {
    match r {
        GrooveRelation::Free => "free",
        GrooveRelation::PocketSkeleton => "pocket-skeleton",
        GrooveRelation::KickSnare => "kick-snare",
    }
}
fn form_label(r: FormRelation) -> &'static str {
    match r {
        FormRelation::Free => "free",
        FormRelation::Topology => "topology",
        FormRelation::Exact => "exact",
    }
}
fn orchestration_label(r: OrchestrationRelation) -> &'static str {
    match r {
        OrchestrationRelation::Free => "free",
        OrchestrationRelation::Exact => "exact",
    }
}

/// What a source can support per axis, and the evidence behind it. `None` = Unknown.
pub(crate) struct Availability {
    pub line: [(Option<LineRelation>, &'static str); 3],
    pub harmony: (Option<HarmonyRelation>, AxisEvidence, &'static str),
    pub groove: (Option<GrooveRelation>, &'static str),
    pub form: (Option<FormRelation>, &'static str),
    pub orchestration: (Option<OrchestrationRelation>, &'static str),
}

fn lower_line(requested: LineRelation, ceiling: Option<LineRelation>) -> Option<LineRelation> {
    let ceiling = ceiling?;
    Some(if requested == LineRelation::Theme {
        if ceiling >= LineRelation::Theme {
            LineRelation::Theme
        } else {
            LineRelation::Free
        }
    } else {
        requested.min(ceiling)
    })
}

/// Apply a source's availability to a requested profile: the effective profile, the Unknown
/// axes, and the per-axis report. Unknown is never lowered to Free or raised to a pin.
pub(crate) fn ceiling(
    requested: &CoverFidelityProfile,
    preset: Option<CoverFidelityPreset>,
    available: &Availability,
) -> (CoverFidelityProfile, Vec<CoverAxis>, FidelityReport) {
    let mut effective = *requested;
    let mut unknown = Vec::new();
    let mut axes = Vec::new();
    let lines = [
        (CoverAxis::Motif, requested.motif),
        (CoverAxis::Riff, requested.riff),
        (CoverAxis::BassFigure, requested.bass),
    ];
    for (i, (axis, want)) in lines.into_iter().enumerate() {
        let (ceiling, why) = available.line[i];
        let got = if want == LineRelation::Free {
            Some(LineRelation::Free)
        } else {
            lower_line(want, ceiling)
        };
        let (eff, evidence, why) = match got {
            None => {
                unknown.push(axis);
                (LineRelation::Free, AxisEvidence::Unknown, why)
            }
            Some(LineRelation::Free) if want != LineRelation::Free => {
                (LineRelation::Free, AxisEvidence::Observed, why)
            }
            Some(r) if r == LineRelation::Free => (r, AxisEvidence::NotPinned, "not requested"),
            Some(r) => (
                r,
                AxisEvidence::Observed,
                if r == want { "observed" } else { why },
            ),
        };
        match axis {
            CoverAxis::Motif => effective.motif = eff,
            CoverAxis::Riff => effective.riff = eff,
            _ => effective.bass = eff,
        }
        axes.push(AxisFidelity {
            axis,
            requested: CoverFidelityProfile::line_label(want),
            effective: if evidence == AxisEvidence::Unknown {
                "unknown"
            } else {
                CoverFidelityProfile::line_label(eff)
            },
            evidence,
            why,
        });
    }
    // Harmony.
    {
        let want = requested.harmony;
        let (ceiling, evidence, why) = available.harmony;
        let (eff, ev, why) = match (want, ceiling) {
            (HarmonyRelation::Free, _) => (
                HarmonyRelation::Free,
                AxisEvidence::NotPinned,
                "not requested",
            ),
            (_, None) => {
                unknown.push(CoverAxis::HarmonicContour);
                (HarmonyRelation::Free, AxisEvidence::Unknown, why)
            }
            (_, Some(HarmonyRelation::Free)) => (HarmonyRelation::Free, evidence, why),
            (_, Some(HarmonyRelation::Ordered)) => (HarmonyRelation::Ordered, evidence, why),
            (HarmonyRelation::Exact, Some(HarmonyRelation::QualityFamily)) => {
                (HarmonyRelation::QualityFamily, evidence, why)
            }
            (w, Some(_)) => (w, evidence, if w == want { "available" } else { why }),
        };
        effective.harmony = eff;
        axes.push(AxisFidelity {
            axis: CoverAxis::HarmonicContour,
            requested: harmony_label(want),
            effective: if ev == AxisEvidence::Unknown {
                "unknown"
            } else {
                harmony_label(eff)
            },
            evidence: ev,
            why,
        });
    }
    // Groove.
    {
        let want = requested.groove;
        let (ceiling, why) = available.groove;
        let (eff, ev, why) = match (want, ceiling) {
            (GrooveRelation::Free, _) => (
                GrooveRelation::Free,
                AxisEvidence::NotPinned,
                "not requested",
            ),
            (_, None) => {
                unknown.push(CoverAxis::Groove);
                (GrooveRelation::Free, AxisEvidence::Unknown, why)
            }
            (_, Some(GrooveRelation::Free)) => (GrooveRelation::Free, AxisEvidence::Observed, why),
            (w, Some(_)) => (w, AxisEvidence::Observed, "observed"),
        };
        effective.groove = eff;
        axes.push(AxisFidelity {
            axis: CoverAxis::Groove,
            requested: groove_label(want),
            effective: if ev == AxisEvidence::Unknown {
                "unknown"
            } else {
                groove_label(eff)
            },
            evidence: ev,
            why,
        });
    }
    // Form.
    {
        let want = requested.form;
        let (ceiling, why) = available.form;
        let (eff, ev, why) = match (want, ceiling) {
            (FormRelation::Free, _) => {
                (FormRelation::Free, AxisEvidence::NotPinned, "not requested")
            }
            (_, None) => {
                unknown.push(CoverAxis::Form);
                (FormRelation::Free, AxisEvidence::Unknown, why)
            }
            (_, Some(FormRelation::Topology)) => {
                (FormRelation::Topology, AxisEvidence::Observed, why)
            }
            (w, Some(_)) => (w, AxisEvidence::Observed, "observed"),
        };
        effective.form = eff;
        axes.push(AxisFidelity {
            axis: CoverAxis::Form,
            requested: form_label(want),
            effective: if ev == AxisEvidence::Unknown {
                "unknown"
            } else {
                form_label(eff)
            },
            evidence: ev,
            why,
        });
    }
    // Orchestration.
    {
        let want = requested.orchestration;
        let (ceiling, why) = available.orchestration;
        let (eff, ev, why) = match (want, ceiling) {
            (OrchestrationRelation::Free, _) => (
                OrchestrationRelation::Free,
                AxisEvidence::NotPinned,
                "not requested",
            ),
            (_, None) => {
                unknown.push(CoverAxis::Orchestration);
                (OrchestrationRelation::Free, AxisEvidence::Unknown, why)
            }
            (w, Some(_)) => (w, AxisEvidence::Observed, "observed"),
        };
        effective.orchestration = eff;
        axes.push(AxisFidelity {
            axis: CoverAxis::Orchestration,
            requested: orchestration_label(want),
            effective: if ev == AxisEvidence::Unknown {
                "unknown"
            } else {
                orchestration_label(eff)
            },
            evidence: ev,
            why,
        });
    }
    // An axis Unknown at the source stays Unknown whether or not it was requested.
    for (axis, known) in [
        (CoverAxis::Motif, available.line[0].0.is_some()),
        (CoverAxis::Riff, available.line[1].0.is_some()),
        (CoverAxis::BassFigure, available.line[2].0.is_some()),
        (CoverAxis::HarmonicContour, available.harmony.0.is_some()),
        (CoverAxis::HarmonicLoop, available.harmony.0.is_some()),
        (CoverAxis::Groove, available.groove.0.is_some()),
        (CoverAxis::Form, available.form.0.is_some()),
        (
            CoverAxis::Orchestration,
            available.orchestration.0.is_some(),
        ),
    ] {
        if !known && !unknown.contains(&axis) {
            unknown.push(axis);
        }
    }
    let unknown: Vec<CoverAxis> = CoverAxis::ALL
        .into_iter()
        .filter(|a| unknown.contains(a))
        .collect();
    let report = FidelityReport {
        preset,
        requested: *requested,
        effective,
        axes,
    };
    (effective, unknown, report)
}

/// Project a pinned line to its relation's quotient.
pub(crate) fn project_line(line: &CoverLine, relation: LineRelation) -> CoverLine {
    let notes = match relation {
        LineRelation::Faithful => line.notes.clone(),
        LineRelation::Metric => line
            .notes
            .iter()
            .map(|n| CoverNote {
                reserved_until: None,
                ..n.clone()
            })
            .collect(),
        LineRelation::Theme => {
            let start = line.notes.first().map_or(0.0, |n| n.at.beats());
            line.notes
                .iter()
                .filter(|n| n.at.beats() < start + THEME_BEATS - 1e-9)
                .map(|n| CoverNote {
                    reserved_until: None,
                    ..n.clone()
                })
                .collect()
        }
        LineRelation::Free => Vec::new(),
    };
    CoverLine {
        role: line.role,
        notes,
    }
}

/// The theme as a motif the cover's own composer states: semitone units relative to the target
/// tonic (its first note's octave free) and its exact inter-onset rhythm (the last note keeps
/// the IOI to the theme window's end).
pub(crate) fn theme_motif(theme: &CoverLine) -> super::super::motif::Motif {
    let start = theme.notes.first().map_or(0.0, |n| n.at.beats());
    super::super::motif::Motif {
        pitch_basis: super::super::theory::PitchBasis::Semitones,
        id: 0,
        degrees: theme.notes.iter().map(|n| n.relative_pitch).collect(),
        rhythm: theme
            .notes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                (theme
                    .notes
                    .get(i + 1)
                    .map_or(start + THEME_BEATS, |x| x.at.beats())
                    - n.at.beats()) as f32
            })
            .collect(),
    }
}

/// Whether a cover keeps the pinned theme: its song declares the theme (exact relative pitches and
/// inter-onset rhythm) as its identity motif, and at least one of the song's theme sites is
/// performed — its statement planned and its material sounding in the lead.
pub(crate) fn states_theme(theme: &CoverLine, actual: &Composition) -> bool {
    if theme.notes.is_empty() || actual.song.thematic.bank.identity != theme_motif(theme) {
        return false;
    }
    actual.song.thematic.sites.iter().any(|site| {
        actual.perf.statements.iter().any(|st| {
            st.phrase == site.phrase
                && st.motif == site.motif
                && actual
                    .score
                    .role_notes(Role::Lead)
                    .any(|n| n.prov.material == Some(st.material))
        })
    })
}

/// Harmony at a relation: the same spans and roots; the quality only up to its family.
pub(crate) fn harmony_by_family(
    h: &[CoverChord],
) -> Vec<(MetricPosition, MetricPosition, i32, QualityFamily)> {
    h.iter()
        .map(|c| (c.at, c.end, c.relative_root, QualityFamily::of(c.quality)))
        .collect()
}

/// The quarter-grid skeleton of a kick/snare part.
pub(crate) fn skeleton(strokes: &[CoverStroke]) -> Vec<CoverStroke> {
    strokes
        .iter()
        .filter(|s| {
            let b = s.at.beats();
            (b - b.round()).abs() < 1e-9
        })
        .cloned()
        .collect()
}

impl CoverMap {
    /// Extract a generated (or any performed) source at a fidelity profile. The report names
    /// every relation the source could not support. At [`CoverFidelityProfile::from_spec`] the map
    /// is exactly the v1 [`CoverMap::extract`] map.
    pub fn extract_fidelity(
        reference: &Composition,
        world: &MusicWorld,
        requested: &CoverFidelityProfile,
        preset: Option<CoverFidelityPreset>,
    ) -> Result<(CoverMap, FidelityReport), CoverError> {
        requested.validate()?;
        let lane = requested.riff_lane;
        // Only a missing axis is observed absence. Any other extraction error (unprojectable
        // timing, an invalid transport) is an error, never quietly read as "the source has none".
        let probe = |axis: CoverAxis| -> Result<Option<CoverMap>, CoverError> {
            match CoverMap::extract_on_lane(reference, world, CoverSpec::new([axis]), lane) {
                Ok(m) => Ok(Some(m)),
                Err(CoverError::MissingAxis(_)) => Ok(None),
                Err(e) => Err(e),
            }
        };
        let line_avail = |axis: CoverAxis,
                          want: LineRelation|
         -> Result<(Option<LineRelation>, &'static str), CoverError> {
            if want == LineRelation::Free {
                return Ok((Some(LineRelation::Faithful), "not requested"));
            }
            Ok(match probe(axis)? {
                Some(m) => {
                    let line = [m.motif, m.riff, m.bass]
                        .into_iter()
                        .flatten()
                        .next()
                        .expect("an extracted line axis holds its line");
                    if line.notes.iter().all(|n| n.reserved_until.is_some()) {
                        (Some(LineRelation::Faithful), "observed rest boundaries")
                    } else {
                        (
                            Some(LineRelation::Metric),
                            "no observed note/rest boundaries: rests are the band's",
                        )
                    }
                }
                None => (Some(LineRelation::Free), "the source sounds no such line"),
            })
        };
        let harmony = if requested.harmony == HarmonyRelation::Free {
            (
                Some(HarmonyRelation::Exact),
                AxisEvidence::Observed,
                "not requested",
            )
        } else if probe(CoverAxis::HarmonicContour)?.is_some() {
            (
                Some(HarmonyRelation::Exact),
                AxisEvidence::Observed,
                "observed chord spans",
            )
        } else {
            (
                Some(HarmonyRelation::Free),
                AxisEvidence::Observed,
                "the source sounds no chord",
            )
        };
        let groove = if requested.groove == GrooveRelation::Free {
            (Some(GrooveRelation::KickSnare), "not requested")
        } else if probe(CoverAxis::Groove)?.is_some() {
            (Some(GrooveRelation::KickSnare), "observed")
        } else {
            (
                Some(GrooveRelation::Free),
                "the source sounds no kick or snare",
            )
        };
        let available = Availability {
            line: [
                line_avail(CoverAxis::Motif, requested.motif)?,
                line_avail(CoverAxis::Riff, requested.riff)?,
                line_avail(CoverAxis::BassFigure, requested.bass)?,
            ],
            harmony,
            groove,
            form: (Some(FormRelation::Exact), "observed"),
            orchestration: (Some(OrchestrationRelation::Exact), "observed"),
        };
        let (mut effective, _unknown, mut report) = ceiling(requested, preset, &available);
        // A requested axis the source does not sound at all is not pinned (not Unknown: the
        // absence is observed).
        let absent = |r: &(Option<LineRelation>, &str)| r.0 == Some(LineRelation::Free);
        if absent(&available.line[0]) {
            effective.motif = LineRelation::Free;
        }
        if absent(&available.line[1]) {
            effective.riff = LineRelation::Free;
        }
        if absent(&available.line[2]) {
            effective.bass = LineRelation::Free;
        }
        if available.harmony.0 == Some(HarmonyRelation::Free) {
            effective.harmony = HarmonyRelation::Free;
        }
        if available.groove.0 == Some(GrooveRelation::Free) {
            effective.groove = GrooveRelation::Free;
        }
        report.effective = effective;
        for a in &mut report.axes {
            let label = match a.axis {
                CoverAxis::Motif => CoverFidelityProfile::line_label(effective.motif),
                CoverAxis::Riff => CoverFidelityProfile::line_label(effective.riff),
                CoverAxis::BassFigure => CoverFidelityProfile::line_label(effective.bass),
                CoverAxis::HarmonicContour | CoverAxis::HarmonicLoop => {
                    harmony_label(effective.harmony)
                }
                CoverAxis::Groove => groove_label(effective.groove),
                CoverAxis::Form => form_label(effective.form),
                CoverAxis::Orchestration => orchestration_label(effective.orchestration),
            };
            if a.evidence != AxisEvidence::Unknown {
                a.effective = label;
            }
        }
        let mut map = CoverMap::extract_on_lane(reference, world, effective.spec(), lane)?;
        map.apply_fidelity(&effective);
        Ok((map, report))
    }

    /// Project every pinned axis to its relation and record the profile, unless it is exactly
    /// the v1 relation set (then the map stays the v1 map, hash included).
    pub(crate) fn apply_fidelity(&mut self, effective: &CoverFidelityProfile) {
        // A Faithful line whose boundaries were never observed is the v1 line: v1 kept them only
        // where observed.
        let unobserved = |l: &Option<CoverLine>| {
            l.as_ref()
                .is_none_or(|l| l.notes.iter().all(|n| n.reserved_until.is_none()))
        };
        let mut as_v1 = *effective;
        for (relation, line) in [
            (&mut as_v1.motif, &self.motif),
            (&mut as_v1.riff, &self.riff),
            (&mut as_v1.bass, &self.bass),
        ] {
            if *relation == LineRelation::Metric && unobserved(line) {
                *relation = LineRelation::Faithful;
            }
        }
        as_v1.riff_lane = None;
        if as_v1 == CoverFidelityProfile::from_spec(&self.spec) && effective.riff_lane.is_none() {
            self.fidelity = None;
            return;
        }
        for (line, relation) in [
            (&mut self.motif, effective.motif),
            (&mut self.riff, effective.riff),
            (&mut self.bass, effective.bass),
        ] {
            if let Some(l) = line.as_mut() {
                *l = project_line(l, relation);
            }
        }
        if effective.groove == GrooveRelation::PocketSkeleton {
            if let Some(g) = self.groove.as_mut() {
                *g = skeleton(g);
            }
        }
        self.fidelity = Some(*effective);
    }

    /// The relation this map holds for `axis` (v1 relations when no profile is recorded).
    pub fn relations(&self) -> CoverFidelityProfile {
        self.fidelity
            .unwrap_or_else(|| CoverFidelityProfile::from_spec(&self.spec))
    }
}

impl CanonicalFingerprint for CoverFidelityProfile {
    fn encode(&self, w: &mut FingerprintWriter) {
        let Self {
            motif,
            riff,
            riff_lane,
            bass,
            harmony,
            harmony_axis,
            groove,
            form,
            orchestration,
        } = self;
        w.tag("CoverFidelityProfile/v1");
        for (tag, r) in [("motif", motif), ("riff", riff), ("bass", bass)] {
            w.tag(tag);
            w.tag(CoverFidelityProfile::line_label(*r));
        }
        w.tag("riff_lane");
        w.tag(match riff_lane {
            None => "default",
            Some(Role::Lead) => "lead",
            Some(_) => "bass",
        });
        w.tag(harmony_label(*harmony));
        w.tag(match harmony_axis {
            HarmonyAxis::Contour => "contour",
            HarmonyAxis::Loop => "loop",
            HarmonyAxis::Both => "contour+loop",
        });
        w.tag(groove_label(*groove));
        w.tag(form_label(*form));
        w.tag(orchestration_label(*orchestration));
    }
}

/// The exact relation checked for `axis` under `rel`, for receipts.
pub(crate) fn relation_text(axis: CoverAxis, rel: &CoverFidelityProfile) -> &'static str {
    match axis {
        CoverAxis::Motif | CoverAxis::Riff | CoverAxis::BassFigure => {
            match match axis {
                CoverAxis::Motif => rel.motif,
                CoverAxis::Riff => rel.riff,
                _ => rel.bass,
            } {
                LineRelation::Theme => "the source's opening statement (exact relative pitches and rhythm) is the song's identity motif, and a statement derived from it sounds",
                LineRelation::Metric => "exact relative pitches at exact canonical onsets; lengths and rests free",
                LineRelation::Faithful | LineRelation::Free => axis.relation(),
            }
        }
        CoverAxis::HarmonicContour | CoverAxis::HarmonicLoop => match rel.harmony {
            HarmonyRelation::QualityFamily => {
                "exact chord spans and relative roots; quality up to its triad family"
            }
            _ => axis.relation(),
        },
        CoverAxis::Groove => match rel.groove {
            GrooveRelation::PocketSkeleton => {
                "exact kick/snare strokes on the quarter-note grid; between beats free"
            }
            _ => axis.relation(),
        },
        _ => axis.relation(),
    }
}

/// A symbolic reference's availability: observed lines (`None` = Unknown), harmony only from a
/// named derived analysis, groove/form/seating unobserved.
pub(crate) fn fidelity_availability(
    lines: [(Option<LineRelation>, &'static str); 3],
    derived_method: Option<&'static str>,
) -> Availability {
    Availability {
        line: lines,
        harmony: match derived_method {
            Some(method) => (
                Some(HarmonyRelation::Exact),
                AxisEvidence::DerivedAnalysis(method),
                "derived analysis of the observed simultaneities, not score metadata",
            ),
            None => (
                None,
                AxisEvidence::Unknown,
                "no chord symbols observed and no derived analysis supplied",
            ),
        },
        groove: (None, "no percussion observed"),
        form: (None, "no phrase or section boundaries observed"),
        orchestration: (None, "no band seating observed"),
    }
}

/// [`ceiling`] for callers outside the cover module, validating the request first.
pub(crate) fn fidelity_ceiling(
    requested: &CoverFidelityProfile,
    preset: Option<CoverFidelityPreset>,
    available: &Availability,
) -> Result<(CoverFidelityProfile, Vec<CoverAxis>, FidelityReport), CoverError> {
    requested.validate()?;
    Ok(ceiling(requested, preset, available))
}

impl CoverMap {
    /// An unmetered chord chart at a fidelity profile. The chart observes an ordered sequence of
    /// relative roots and qualities inside an ordered section topology — nothing else. Its harmony
    /// is ordered within its sections, so it is pinned only together with that topology (both
    /// requested), and then exactly as [`CoverMap::from_ordered_chart`]. Melody, bass, groove and
    /// seating stay Unknown at every setting; asking for more reports the ceiling instead.
    /// `None`: nothing observed survives the requested profile, so there is nothing to cover.
    pub fn from_ordered_chart_fidelity(
        chart: OrderedChart,
        requested: &CoverFidelityProfile,
        preset: Option<CoverFidelityPreset>,
    ) -> Result<(Option<CoverMap>, FidelityReport), CoverError> {
        requested.validate()?;
        let unobserved = (
            None,
            "the supplied chart has no notes: no melody, riff or bass observed",
        );
        let available = Availability {
            line: [unobserved, unobserved, unobserved],
            harmony: (
                Some(HarmonyRelation::Ordered),
                AxisEvidence::Observed,
                "an ordered chord chart: relative roots and qualities, no durations",
            ),
            groove: (None, "no percussion observed"),
            form: (Some(FormRelation::Topology), "section order, no bar counts"),
            orchestration: (None, "no band seating observed"),
        };
        let (mut effective, _unknown, mut report) = ceiling(requested, preset, &available);
        let chart_pinned =
            requested.harmony != HarmonyRelation::Free && requested.form != FormRelation::Free;
        if !chart_pinned {
            effective.harmony = HarmonyRelation::Free;
            effective.form = FormRelation::Free;
            report.effective = effective;
            for a in &mut report.axes {
                if matches!(a.axis, CoverAxis::HarmonicContour | CoverAxis::Form)
                    && a.evidence != AxisEvidence::NotPinned
                {
                    a.effective = "free";
                    a.why = "the chart's harmony is ordered within its sections: pinned only together with its section topology";
                }
            }
            return Ok((None, report));
        }
        Ok((Some(CoverMap::from_ordered_chart(chart)?), report))
    }
}
