//! HumanMusic merge holdout v4: predeclared, one contact. This ignored test is never a tuning
//! loop. Configuration and harness are committed before execution; the output directory must be
//! new. v3 stays immutable known evidence; nothing here was fitted to it.
//!
//! The laws are the consolidated ones, applied to every row:
//!
//! - every generated BAND source holds its own declared contract — the general
//!   [`PerformanceReceipt`] under its profile (what `perform_checked` admits); a red source still
//!   proceeds to extraction and lift, so its evidence is not lost;
//! - the song records the composer that was asked to compose it, and the grammar it declared;
//! - the identity projection is well defined under overlapping material: adding a non-identity
//!   event at an identity onset of a pinned role does not change the extracted map;
//! - every refusal is one of the declared lawful refusals, at the stage that may make it;
//! - every lift is admitted: `CoverAdmission = PerformanceReceipt + CoverConformance` under the
//!   target's profile, and the cover is not a replay of its source;
//! - a swung Groove pinned together with another song-identity axis across a swing change: the
//!   canonical groove QUOTIENT (its metric positions) survives the target's swing transport,
//!   while the performed float onsets are not required to match across worlds.
use gibson::audio::human_music::{
    composer::Composer,
    contract::CompositionGrammar,
    cover::{
        cover_candidate, cover_skeleton, AxisEvidence, CoverAxis, CoverConformance, CoverError,
        CoverFidelityPreset, CoverFidelityProfile, CoverFreedom, CoverKnowledge, CoverMap,
        CoverSpec, CoverTarget, FidelityReport, GrooveVoice, OrderedChart, SkeletonSchedule,
    },
    fingerprint::CanonicalFingerprint,
    functor::{perform_candidate, Composition},
    ids::MaterialId,
    language::MusicalLanguage,
    percussion::{DrumRestraint, PercussionPolicy},
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    projection::{identity_notes, IdentityMaterial, STATEMENT_NOTE},
    receipt::PerformanceReceipt,
    reference_song::{ReferenceSong, DERIVED_HARMONY_METHOD},
    rhythm::MetricPosition,
    score::{DrumVoice, Note, Provenance, Role, StrokeOrigin},
    semantic::{deflected_lift_trace, demo_trace},
    song::{AnchorReport, SongMap},
    vocabulary::VOCABULARY_REFUSAL,
    MusicWorld,
};
use std::{
    any::Any,
    fs::{self, File, OpenOptions},
    io::Write,
    panic::{catch_unwind, AssertUnwindSafe},
    path::Path,
};

const CONFIG: &str =
    include_str!("../docs/fixtures/humanmusic-consolidation2/holdout-v4/config.tsv");
const ODE: &str = include_str!("../docs/fixtures/humanmusic-cover/ode-import/reference.tsv");
const SWING: &str =
    include_str!("../docs/fixtures/humanmusic-cover/swing-partial/ordered-chart.tsv");
const EXPECTED_CASES: usize = 28;
const COLUMNS: usize = 20;

struct Case<'a> {
    id: &'a str,
    source: &'a str,
    story: &'a str,
    source_seed: &'a str,
    source_world: &'a str,
    source_grammar: &'a str,
    composer: &'a str,
    beats: &'a str,
    source_tempo: &'a str,
    source_language: &'a str,
    source_profile: &'a str,
    selection: &'a str,
    expect: &'a str,
    target_seed: &'a str,
    target_world: &'a str,
    target_grammar: &'a str,
    target_tempo: &'a str,
    target_tonic: &'a str,
    target_language: &'a str,
    target_profile: &'a str,
}

fn cases() -> Vec<Case<'static>> {
    CONFIG
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .skip(1)
        .map(|line| {
            let f: Vec<_> = line.split('\t').collect();
            assert_eq!(f.len(), COLUMNS, "invalid configuration row: {line}");
            assert!(
                matches!(f[12], "lift" | "refusal" | "lawful" | "nothing"),
                "undeclared expectation in {line}"
            );
            Case {
                id: f[0],
                source: f[1],
                story: f[2],
                source_seed: f[3],
                source_world: f[4],
                source_grammar: f[5],
                composer: f[6],
                beats: f[7],
                source_tempo: f[8],
                source_language: f[9],
                source_profile: f[10],
                selection: f[11],
                expect: f[12],
                target_seed: f[13],
                target_world: f[14],
                target_grammar: f[15],
                target_tempo: f[16],
                target_tonic: f[17],
                target_language: f[18],
                target_profile: f[19],
            }
        })
        .collect()
}

fn world(name: &str, tempo: &str) -> MusicWorld {
    let mut world = match name {
        "BLACK_ICE" => MusicWorld::black_ice(),
        "SWISS_SIGNAL" => MusicWorld::swiss_signal(),
        "VAPOR95" => MusicWorld::vapor95(),
        _ => panic!("unknown declared world {name}"),
    };
    if tempo != "nominal" {
        world.tempo_bpm = tempo.parse().expect("declared tempo");
    }
    world
}

fn grammar(name: &str) -> CompositionGrammar {
    match name {
        "HookArc" => CompositionGrammar::HookArc,
        "LoopEvolution" => CompositionGrammar::LoopEvolution,
        "RiffDrive" => CompositionGrammar::RiffDrive,
        "WorldSwitch" => CompositionGrammar::WorldSwitch,
        "DeflectedLift" => CompositionGrammar::DeflectedLift,
        "PropulsiveReturn" => CompositionGrammar::PropulsiveReturn,
        _ => panic!("unknown declared grammar {name}"),
    }
}

fn composer(name: &str) -> Composer {
    match name {
        "StructuralR9" => Composer::StructuralR9,
        "MeaningDirected" => Composer::MeaningDirected,
        "StablePropulsion" => Composer::StablePropulsion,
        _ => panic!("unknown declared composer {name}"),
    }
}

fn options(language: &str) -> PerformanceOptions {
    PerformanceOptions {
        language: match language {
            "fusion" => MusicalLanguage::fusion_conversation(),
            "simple" => MusicalLanguage::simple(),
            _ => panic!("unknown declared language {language}"),
        },
        ..PerformanceOptions::default()
    }
}

fn profile(name: &str) -> PerformanceProfile {
    let restraint = match name {
        "band-foundation" => DrumRestraint::Foundation,
        "band-balanced" => DrumRestraint::Balanced,
        "band-expressive" => DrumRestraint::Expressive,
        "band-busy" => DrumRestraint::Busy,
        _ => panic!("undeclared profile {name}: v3 declares BAND at a drum restraint only"),
    };
    PerformanceProfile::BAND.with_drum_restraint(restraint)
}

fn preset(name: &str) -> Option<CoverFidelityPreset> {
    match name.trim_end_matches("-derived") {
        "preset-loose" => Some(CoverFidelityPreset::Loose),
        "preset-interpretive" => Some(CoverFidelityPreset::Interpretive),
        "preset-faithful" => Some(CoverFidelityPreset::Faithful),
        "preset-strict" => Some(CoverFidelityPreset::Strict),
        _ => None,
    }
}

/// Where a refusal was made.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Extraction,
    Lift,
}

/// The declared lawful refusal family (see the configuration header), at the stage that may make
/// each refusal. `established` is the generated source's established spec: an axis it never
/// established may lawfully be missing from it.
fn lawful_refusal(error: &CoverError, stage: Stage, established: Option<&CoverSpec>) -> bool {
    match error {
        CoverError::ConflictingPins => stage == Stage::Extraction,
        CoverError::MissingAxis(axis) => {
            stage == Stage::Extraction && established.is_some_and(|spec| !spec.contains(*axis))
        }
        CoverError::Invalid(why) => {
            why.contains("outside target vocabulary")
                || why.contains("no lawful harmony contains the pinned simultaneous attacks")
                || why.contains("no lawful pitch function in the target harmony")
                || why.contains(VOCABULARY_REFUSAL)
        }
        _ => false,
    }
}

fn escaped(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn panic_text(payload: Box<dyn Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).into()))
        .unwrap_or_else(|| "non-string panic payload".into())
}

/// Append and flush every observation before the next potentially failing operation.
struct Receipt<'a> {
    id: &'a str,
    file: File,
    checks: usize,
    failures: usize,
}

impl Receipt<'_> {
    fn emit(&mut self, phase: &str, law: &str, status: &str, detail: &str) {
        self.checks += 1;
        self.failures += usize::from(matches!(status, "FAIL" | "PANIC"));
        let row = format!(
            "{}\t{phase}\t{law}\t{status}\t{}\n",
            self.id,
            escaped(detail)
        );
        self.file.write_all(row.as_bytes()).expect("append receipt");
        self.file.flush().expect("flush receipt");
        print!("{row}");
    }

    fn check(&mut self, phase: &str, law: &str, f: impl FnOnce() -> (bool, String)) {
        match catch_unwind(AssertUnwindSafe(f)) {
            Ok((pass, detail)) => {
                self.emit(phase, law, if pass { "PASS" } else { "FAIL" }, &detail)
            }
            Err(panic) => self.emit(phase, law, "PANIC", &panic_text(panic)),
        }
    }
}

fn copy(c: &Composition) -> Composition {
    Composition {
        score: c.score.clone(),
        song: c.song.clone(),
        perf: c.perf.clone(),
    }
}

/// The consolidated general law, one receipt row per law so no failure hides another.
fn performance_laws(
    c: &Composition,
    world: &MusicWorld,
    profile: PerformanceProfile,
    phase: &str,
    receipt: &mut Receipt<'_>,
) {
    receipt.check(phase, "canonical-inventory", || {
        (
            true,
            format!(
                "song={:016x}; plan={:016x}; score={:016x}; notes={}; drums={}; length={}; tempo={}",
                c.song.canonical_fingerprint(),
                c.perf.canonical_fingerprint(),
                c.score.canonical_fingerprint(),
                c.score.notes.len(),
                c.score.drums.len(),
                c.score.total_beats,
                c.score.tempo_bpm
            ),
        )
    });
    receipt.check(phase, "performance-receipt", || {
        let law = PerformanceReceipt::measure_under(c, world, profile);
        (
            law.passes(),
            format!("failures={:?}; receipt={law:?}", law.failures()),
        )
    });
    receipt.check(phase, "guard:percussion-law", || {
        match (profile.percussion, &c.score.percussion) {
            (PercussionPolicy::Arbitrated(restraint), Some(report)) => {
                let over: Vec<_> = report
                    .bars
                    .iter()
                    .filter(|b| b.admitted > b.band.allowance())
                    .collect();
                (
                    report.restraint == restraint && over.is_empty(),
                    format!("{}bars over allowance={over:?}", report.report()),
                )
            }
            (declared, report) => (
                false,
                format!("declared {declared:?}; report present={}", report.is_some()),
            ),
        }
    });
    if let Some(trace) = &c.perf.rehearsal {
        receipt.check(phase, "evidence:rehearsal", || (true, format!("{trace:?}")));
    }
}

/// The roles whose identity `map` pins.
fn pinned_identity_roles(map: &CoverMap) -> Vec<Role> {
    let mut roles = Vec::new();
    if map.spec.contains(CoverAxis::Motif) || map.spec.contains(CoverAxis::Riff) {
        roles.push(Role::Lead);
    }
    if map.spec.contains(CoverAxis::BassFigure) || map.spec.contains(CoverAxis::Riff) {
        roles.push(Role::Bass);
    }
    roles
}

/// Identity under overlapping material: the source's natural overlaps are reported, and a
/// non-identity event added at an identity onset of every pinned role leaves the map unchanged.
fn identity_overlap(
    source: &Composition,
    map: &CoverMap,
    extract: &dyn Fn(&Composition) -> Result<CoverMap, CoverError>,
    receipt: &mut Receipt<'_>,
) {
    let roles = pinned_identity_roles(map);
    if roles.is_empty() {
        return;
    }
    receipt.check("extraction", "evidence:natural-overlap", || {
        let material = IdentityMaterial::of(&source.song, &source.perf);
        let mut shared = Vec::new();
        for &role in &roles {
            let identity = identity_notes(source, role);
            let at = |n: &Note| n.start_beat.to_bits();
            let count = source
                .score
                .role_notes(role)
                .filter(|n| !material.carries(n))
                .filter(|n| identity.iter().any(|i| at(i) == at(n)))
                .count();
            shared.push((role, identity.len(), count));
        }
        (
            true,
            format!("(role, identity events, non-identity events at an identity onset)={shared:?}"),
        )
    });
    receipt.check("extraction", "identity-under-overlap", || {
        let material = IdentityMaterial::of(&source.song, &source.perf);
        // The hostile event shares the identity event's instrument and onset and is tagged as
        // that instrument's own kind of note - a lead statement note of a material that is not
        // the song's theme, a bass approach - so only the identity projection tells them apart.
        let foreign = source
            .perf
            .materials
            .iter()
            .map(|m| m.id)
            .find(|id| !material.theme.contains(id))
            .unwrap_or(MaterialId(u32::MAX));
        let mut hostile = copy(source);
        let mut added = Vec::new();
        for &role in &roles {
            let Some(anchor) = identity_notes(source, role).first().copied().copied() else {
                continue;
            };
            let mut n = Note::new(
                anchor.start_beat,
                0.25,
                anchor.pitch + 5,
                0.5,
                role,
                Provenance {
                    material: if role == Role::Lead {
                        Some(foreign)
                    } else {
                        anchor.prov.material
                    },
                    role_note: if role == Role::Lead {
                        STATEMENT_NOTE
                    } else {
                        "approach"
                    },
                    ..anchor.prov
                },
            );
            n.function = anchor.function;
            if material.carries(&n) {
                return (
                    false,
                    format!("harness: the hostile {role:?} event carries identity"),
                );
            }
            hostile.score.notes.push(n);
            added.push((role, anchor.start_beat));
        }
        let again = extract(&hostile);
        (
            again.as_ref() == Ok(map),
            format!(
                "added non-identity events {added:?}; re-extraction {}",
                match &again {
                    Ok(m) if m == map => "identical".to_string(),
                    Ok(m) => format!("CHANGED to spec {:?}", m.spec.axes()),
                    Err(e) => format!("refused: {e:?}"),
                }
            ),
        )
    });
}

/// The observational ceiling is honest: an axis the report calls Unknown is Unknown in the map
/// and not pinned; every pinned axis has observed (or labelled derived) evidence.
fn check_ceiling(map: &CoverMap, report: &FidelityReport, receipt: &mut Receipt<'_>) {
    receipt.check("extraction", "guard:ceiling-honest", || {
        let mut problems = Vec::new();
        for a in &report.axes {
            let pinned = map.spec.contains(a.axis)
                || (a.axis == CoverAxis::HarmonicContour
                    && map.spec.contains(CoverAxis::HarmonicLoop));
            if a.evidence == AxisEvidence::Unknown {
                if pinned || map.knowledge(a.axis) != CoverKnowledge::Unknown {
                    problems.push(format!(
                        "{:?} reported unknown but {:?}",
                        a.axis,
                        map.knowledge(a.axis)
                    ));
                }
            } else if pinned && matches!(a.effective, "unknown" | "free") {
                problems.push(format!(
                    "{:?} pinned with effective {}",
                    a.axis, a.effective
                ));
            }
        }
        if map.relations().spec() != map.spec {
            problems.push("recorded relations disagree with the pins".into());
        }
        (
            problems.is_empty(),
            format!("problems={problems:?}\n{}", report.report()),
        )
    });
}

/// The cross-swing Groove law, restated independently of [`CoverConformance`]: a pinned Groove is
/// a canonical METRIC quotient, so when the cover is admitted every pinned stroke's metric position,
/// carried through the target's declared swing transport, must sound as a realized pinned stroke of
/// the same voice. The performed float onsets are NOT required to match across worlds; the harness
/// reports how many moved (the transport actually applied) and never asserts equality of the floats.
fn cross_swing_groove_quotient(
    candidate: &Composition,
    map: &CoverMap,
    receipt: &mut Receipt<'_>,
) {
    let Some(expected) = &map.groove else {
        return;
    };
    let Some(transport) = candidate
        .perf
        .cover_constraints
        .as_ref()
        .map(|c| c.transport)
    else {
        return;
    };
    receipt.check("cover", "cross-swing-groove-quotient", || {
        let realized: Vec<(f64, bool)> = candidate
            .score
            .drums
            .iter()
            .filter(|d| d.prov.groove_variation == Some("cover-cell"))
            .filter_map(|d| match d.voice {
                DrumVoice::Kick => Some((d.start_beat, false)),
                DrumVoice::Snare => Some((d.start_beat, true)),
                _ => None,
            })
            .collect();
        let mut missing = Vec::new();
        let mut moved = 0usize;
        for pin in expected {
            let at = transport.transport(pin.at);
            let is_snare = matches!(pin.voice, GrooveVoice::Snare);
            let found = realized
                .iter()
                .find(|(b, s)| b.to_bits() == at.beats().to_bits() && *s == is_snare);
            if found.is_none() {
                missing.push(format!(
                    "{:?} at metric {:?} -> performed {}",
                    pin.voice,
                    at.metric(),
                    at.beats()
                ));
            }
            // The transport preserves the metric source: the quotient is the metric position.
            if at.metric() != pin.at {
                missing.push(format!(
                    "{:?} at {:?}: transport changed the metric source",
                    pin.voice, pin.at
                ));
            }
            if (pin.at.beats() - at.beats()).abs() > 1e-9 {
                moved += 1;
            }
        }
        (
            missing.is_empty(),
            format!(
                "{} pinned strokes; {} carried to another performed float by the transport; missing={missing:?}",
                expected.len(),
                moved
            ),
        )
    });
}

fn run_case(case: &Case<'_>, receipt: &mut Receipt<'_>) {
    // --- Source and map. ---
    let mut source: Option<Composition> = None;
    let mut established: Option<CoverSpec> = None;
    let extracted: Result<(CoverMap, Option<FidelityReport>), CoverError> = match case.source {
        "generated" => {
            let world = world(case.source_world, case.source_tempo);
            let source_profile = profile(case.source_profile);
            let beats: f64 = case.beats.parse().expect("declared source length");
            let trace = match case.story {
                "demo" => demo_trace(beats),
                "deflected" => deflected_lift_trace(beats),
                _ => panic!("unknown story {}", case.story),
            };
            let declared_composer = composer(case.composer);
            let song = SongMap::compose(
                &trace,
                case.source_seed.parse().expect("source seed"),
                Some(grammar(case.source_grammar)),
                declared_composer,
            );
            let generated = match catch_unwind(AssertUnwindSafe(|| {
                perform_candidate(&song, &world, options(case.source_language), source_profile)
            })) {
                Err(panic) => {
                    receipt.emit("source", "generation", "PANIC", &panic_text(panic));
                    return;
                }
                Ok(Err(refusal)) => {
                    let lawful = refusal.to_string().contains(VOCABULARY_REFUSAL)
                        && matches!(case.expect, "refusal" | "lawful");
                    receipt.emit(
                        "source",
                        "generation",
                        if lawful { "PASS" } else { "FAIL" },
                        &format!("explicit refusal: {refusal}; expect={}", case.expect),
                    );
                    return;
                }
                Ok(Ok(c)) => c,
            };
            receipt.emit(
                "source",
                "generation",
                "PASS",
                "performed; its laws are judged separately",
            );
            receipt.check("source", "declared-grammar", || {
                (
                    generated.song.plan.contract.grammar == grammar(case.source_grammar),
                    format!("grammar={:?}", generated.song.plan.contract.grammar),
                )
            });
            receipt.check("source", "declared-composer", || {
                (
                    generated.song.composer() == declared_composer,
                    format!(
                        "declared={declared_composer:?}; recorded={:?}",
                        generated.song.composer()
                    ),
                )
            });
            performance_laws(&generated, &world, source_profile, "source", receipt);
            let spec = CoverSpec::established(&AnchorReport::check(
                &generated.song,
                &generated.perf,
                &generated.score,
            ));
            receipt.check("source", "evidence:established", || {
                (true, format!("established={:?}", spec.axes()))
            });
            let selection = case.selection;
            let extract = |c: &Composition| -> Result<CoverMap, CoverError> {
                match selection {
                    "established" => CoverMap::extract(c, &world, spec.clone()),
                    "groove-free-established" => {
                        CoverMap::extract(c, &world, spec.clone().with(CoverAxis::Groove, false))
                    }
                    "motif-only" => {
                        CoverMap::extract(c, &world, CoverSpec::new([CoverAxis::Motif]))
                    }
                    "bass-only" => {
                        CoverMap::extract(c, &world, CoverSpec::new([CoverAxis::BassFigure]))
                    }
                    "groove-only" => {
                        CoverMap::extract(c, &world, CoverSpec::new([CoverAxis::Groove]))
                    }
                    "groove+motif" => CoverMap::extract(
                        c,
                        &world,
                        CoverSpec::new([CoverAxis::Groove, CoverAxis::Motif]),
                    ),
                    "groove+bass" => CoverMap::extract(
                        c,
                        &world,
                        CoverSpec::new([CoverAxis::Groove, CoverAxis::BassFigure]),
                    ),
                    "groove+harmonic" => CoverMap::extract(
                        c,
                        &world,
                        CoverSpec::new([CoverAxis::Groove, CoverAxis::HarmonicContour]),
                    ),
                    other => {
                        let p = preset(other).expect("declared selection");
                        CoverMap::extract_fidelity(
                            c,
                            &world,
                            &CoverFidelityProfile::preset(p),
                            Some(p),
                        )
                        .map(|(m, _)| m)
                    }
                }
            };
            let result = match catch_unwind(AssertUnwindSafe(|| match preset(selection) {
                Some(p) => CoverMap::extract_fidelity(
                    &generated,
                    &world,
                    &CoverFidelityProfile::preset(p),
                    Some(p),
                )
                .map(|(m, r)| (m, Some(r))),
                None => extract(&generated).map(|m| (m, None)),
            })) {
                Ok(result) => result,
                Err(panic) => {
                    receipt.emit("extraction", "projection", "PANIC", &panic_text(panic));
                    return;
                }
            };
            if let Ok((map, _)) = &result {
                identity_overlap(&generated, map, &extract, receipt);
            }
            established = Some(spec);
            source = Some(generated);
            result
        }
        "ode" => {
            let reference = match ReferenceSong::from_tsv(ODE, "sop") {
                Ok(r) => r,
                Err(e) => {
                    receipt.emit("reference", "symbolic-ingestion", "FAIL", &format!("{e:?}"));
                    return;
                }
            };
            let derived = if case.selection.ends_with("-derived") {
                match reference.derive_harmony(MetricPosition::new(2, 1).expect("two beats")) {
                    Ok(d) => {
                        receipt.check("reference", "guard:derived-harmony-labelled", || {
                            (
                                d.method == DERIVED_HARMONY_METHOD && !d.chords.is_empty(),
                                format!(
                                    "DERIVED ANALYSIS method={}; spans={}",
                                    d.method,
                                    d.chords.len()
                                ),
                            )
                        });
                        Some(d)
                    }
                    Err(e) => {
                        receipt.emit("reference", "derived-harmony", "FAIL", &format!("{e:?}"));
                        return;
                    }
                }
            } else {
                None
            };
            match case.selection {
                "v1-motif-only" => reference
                    .extract(CoverSpec::new([CoverAxis::Motif]))
                    .map(|m| (m, None)),
                selection => {
                    let p = preset(selection).expect("declared preset");
                    reference
                        .extract_fidelity(
                            &CoverFidelityProfile::preset(p),
                            Some(p),
                            derived.as_ref(),
                        )
                        .map(|(m, r)| (m, Some(r)))
                }
            }
        }
        "swing" => {
            let p = preset(case.selection).expect("declared preset");
            let chart = match OrderedChart::from_tsv(SWING) {
                Ok(c) => c,
                Err(e) => {
                    receipt.emit(
                        "reference",
                        "ordered-chart-ingestion",
                        "FAIL",
                        &format!("{e:?}"),
                    );
                    return;
                }
            };
            match CoverMap::from_ordered_chart_fidelity(
                chart,
                &CoverFidelityProfile::preset(p),
                Some(p),
            ) {
                Ok((None, report)) => {
                    receipt.emit(
                        "extraction",
                        "nothing-to-cover",
                        if case.expect == "nothing" {
                            "PASS"
                        } else {
                            "FAIL"
                        },
                        &format!(
                            "no observed axis survives {}; expect={}\n{}",
                            p.label(),
                            case.expect,
                            report.report()
                        ),
                    );
                    return;
                }
                Ok((Some(map), report)) => Ok((map, Some(report))),
                Err(e) => Err(e),
            }
        }
        _ => panic!("unknown source {}", case.source),
    };
    let (map, report) = match extracted {
        Ok(x) => x,
        Err(e) => {
            let pass = matches!(case.expect, "refusal" | "lawful")
                && lawful_refusal(&e, Stage::Extraction, established.as_ref());
            receipt.emit(
                "extraction",
                "projection",
                if pass { "PASS" } else { "FAIL" },
                &format!(
                    "explicit refusal at extraction: {e:?}; expect={}",
                    case.expect
                ),
            );
            return;
        }
    };
    // A map that pins no song identity (the form scaffold alone) leaves nothing to cover: lawful
    // where declared `lawful` or `nothing`, never a lift.
    if !map.spec.has_song_identity() {
        receipt.emit(
            "extraction",
            "nothing-to-cover",
            if matches!(case.expect, "lawful" | "nothing") {
                "PASS"
            } else {
                "FAIL"
            },
            &format!(
                "no song identity survives the selection: spec={:?}; expect={}\n{}",
                map.spec.axes(),
                case.expect,
                report.as_ref().map(|r| r.report()).unwrap_or_default()
            ),
        );
        return;
    }
    receipt.emit(
        "extraction",
        "projection",
        if case.expect == "nothing" {
            "FAIL"
        } else {
            "PASS"
        },
        &format!(
            "map={:016x}; spec={:?}; knowledge={:?}; expect={}",
            map.canonical_fingerprint(),
            map.spec.axes(),
            CoverAxis::ALL.map(|a| (a, map.knowledge(a))),
            case.expect
        ),
    );
    receipt.check("extraction", "guard:map-validation", || {
        let result = map.validate();
        (result.is_ok(), format!("{result:?}"))
    });
    if let Some(report) = &report {
        check_ceiling(&map, report, receipt);
    }
    if case.source != "generated" {
        receipt.check("reference", "unobserved-axes-remain-unknown", || {
            // Never observed by either external source: groove and seating. The chart observes
            // an ordered section topology (Form) and harmony; the Ode observes voices but no form,
            // and harmony only through an explicitly supplied derived analysis.
            let mut absent = vec![CoverAxis::Groove, CoverAxis::Orchestration];
            if case.source == "swing" {
                absent.extend([
                    CoverAxis::Motif,
                    CoverAxis::Riff,
                    CoverAxis::BassFigure,
                    CoverAxis::HarmonicLoop,
                ]);
            } else {
                absent.push(CoverAxis::Form);
                if !case.selection.ends_with("-derived") {
                    absent.extend([CoverAxis::HarmonicContour, CoverAxis::HarmonicLoop]);
                }
            }
            let wrong: Vec<_> = absent
                .into_iter()
                .filter(|a| map.knowledge(*a) != CoverKnowledge::Unknown)
                .collect();
            (wrong.is_empty(), format!("wrongly known axes={wrong:?}"))
        });
    }
    if case.selection.ends_with("-derived") && map.harmony.is_some() {
        receipt.check("extraction", "guard:derived-harmony-provenance", || {
            let labelled = report.as_ref().is_some_and(|r| {
                r.axes.iter().any(|a| {
                    a.axis == CoverAxis::HarmonicContour
                        && matches!(a.evidence, AxisEvidence::DerivedAnalysis(_))
                })
            });
            (
                labelled,
                "pinned harmony is labelled derived analysis, never score metadata".into(),
            )
        });
    }
    // --- Lift. ---
    let mut target_world = world(case.target_world, case.target_tempo);
    target_world.tonic_pc = case.target_tonic.parse().expect("declared target tonic");
    let target_profile = profile(case.target_profile);
    let target = CoverTarget {
        world: &target_world,
        seed: case.target_seed.parse().expect("declared target seed"),
        grammar: grammar(case.target_grammar),
        options: options(case.target_language),
        profile: target_profile,
    };
    let lifted = catch_unwind(AssertUnwindSafe(|| {
        if case.source == "swing" {
            cover_skeleton(&map, target, SkeletonSchedule { bars_per_chord: 1 })
        } else {
            cover_candidate(&map, target)
        }
    }));
    let candidate = match lifted {
        Err(panic) => {
            receipt.emit("cover", "candidate-lift", "PANIC", &panic_text(panic));
            return;
        }
        Ok(Err(e)) => {
            let pass = matches!(case.expect, "refusal" | "lawful")
                && lawful_refusal(&e, Stage::Lift, established.as_ref());
            receipt.emit(
                "cover",
                "candidate-lift",
                if pass { "PASS" } else { "FAIL" },
                &format!("explicit refusal: {e:?}; expect={}", case.expect),
            );
            return;
        }
        Ok(Ok(c)) => c,
    };
    receipt.emit(
        "cover",
        "candidate-lift",
        if case.expect == "refusal" {
            "FAIL"
        } else {
            "PASS"
        },
        &format!("lifted; expect={}", case.expect),
    );
    performance_laws(&candidate, &target_world, target_profile, "cover", receipt);
    match catch_unwind(AssertUnwindSafe(|| {
        CoverConformance::check(&map, &candidate, &target_world)
    })) {
        Err(panic) => receipt.emit("cover", "conformance", "PANIC", &panic_text(panic)),
        Ok(law) => {
            receipt.emit(
                "cover",
                "metric-length",
                if law.length_passed { "PASS" } else { "FAIL" },
                "canonical metric extent",
            );
            for check in &law.checks {
                receipt.emit(
                    "cover",
                    check.axis.label(),
                    if check.passed { "PASS" } else { "FAIL" },
                    &format!("{}; {}", check.relation, check.detail),
                );
            }
            for axis in map.spec.axes() {
                if !law.checks.iter().any(|c| c.axis == *axis) {
                    receipt.emit(
                        "cover",
                        axis.label(),
                        "FAIL",
                        "pinned axis has no conformance receipt",
                    );
                }
            }
        }
    }
    if let Some(source) = source {
        receipt.check("cover", "guard:anti-replay-vs-source", || {
            let freedom = CoverFreedom::compare(&source, &candidate);
            (
                freedom.has_freedom(),
                format!("{freedom:?}; machine variation only, no listening claim"),
            )
        });
    }
    cross_swing_groove_quotient(&candidate, &map, receipt);
}

#[test]
#[ignore = "HumanMusic merge holdout v4: configuration and harness are committed before one-time execution"]
fn merge_holdout_v4() {
    let cases = cases();
    assert_eq!(
        cases.len(),
        EXPECTED_CASES,
        "configuration cannot silently shrink"
    );
    // Declared profile: release. Refuse before any output exists, so a wrong invocation cannot
    // burn the contact.
    if cfg!(debug_assertions) {
        panic!(
            "declared execution profile is --release (debug assertions off); nothing was written"
        );
    }
    // The executed source must be a commit anyone can reconstruct: refuse a dirty tree before any
    // output exists.
    let git = |args: &[&str]| -> String {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("git is available");
        assert!(out.status.success(), "git {args:?} failed");
        String::from_utf8(out.stdout).expect("utf-8")
    };
    let head = git(&["rev-parse", "HEAD"]).trim().to_string();
    let dirty = git(&["status", "--porcelain", "--untracked-files=all"]);
    assert!(
        dirty.trim().is_empty(),
        "refusing to execute on a dirty tree (nothing was written):\n{dirty}"
    );
    let output = std::env::var("HUMANMUSIC_HOLDOUT_V4_OUT")
        .expect("set a NEW receipt directory after the configuration commit");
    let output = Path::new(&output);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).expect("holdout parent directory");
    }
    fs::create_dir(output)
        .expect("holdout output must not already exist; never overwrite first-contact evidence");
    fs::write(output.join("config.tsv"), CONFIG).expect("preserve exact executed configuration");
    fs::write(
        output.join("provenance.tsv"),
        format!(
            "field\tvalue\nsource_commit\t{head}\ntree\tclean\ndebug_assertions\t{}\npackage_version\t{}\nconfig_bytes\t{}\nconfig_fingerprint\t{:016x}\node_fixture_fingerprint\t{:016x}\nswing_fixture_fingerprint\t{:016x}\nprofile_band\t{:016x}\nexpected_cases\t{EXPECTED_CASES}\n",
            cfg!(debug_assertions),
            env!("CARGO_PKG_VERSION"),
            CONFIG.len(),
            CONFIG.canonical_fingerprint(),
            ODE.canonical_fingerprint(),
            SWING.canonical_fingerprint(),
            PerformanceProfile::BAND.canonical_fingerprint(),
        ),
    )
    .expect("preserve execution provenance");
    let mut summary = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("summary.tsv"))
        .expect("create summary");
    writeln!(summary, "case\tstatus\tchecks\tfailures").unwrap();
    let mut failed_cases = 0;
    let mut all_failures = 0;
    let mut all_checks = 0;
    for case in &cases {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output.join(format!("{}.tsv", case.id)))
            .expect("unique case receipt");
        let mut receipt = Receipt {
            id: case.id,
            file,
            checks: 0,
            failures: 0,
        };
        receipt
            .file
            .write_all(b"case\tphase\tlaw\tstatus\tdetail\n")
            .unwrap();
        if let Err(panic) = catch_unwind(AssertUnwindSafe(|| run_case(case, &mut receipt))) {
            receipt.emit(
                "case",
                "unhandled-case-failure",
                "PANIC",
                &panic_text(panic),
            );
        }
        let failed = receipt.failures != 0;
        failed_cases += usize::from(failed);
        all_failures += receipt.failures;
        all_checks += receipt.checks;
        writeln!(
            summary,
            "{}\t{}\t{}\t{}",
            case.id,
            if failed { "FAIL" } else { "PASS" },
            receipt.checks,
            receipt.failures
        )
        .unwrap();
        summary.flush().unwrap();
    }
    writeln!(
        summary,
        "TOTAL\t{}\t{}\t{}",
        if failed_cases == 0 { "PASS" } else { "FAIL" },
        all_checks,
        all_failures
    )
    .unwrap();
    summary.flush().unwrap();
    assert_eq!(failed_cases, 0, "{failed_cases}/{EXPECTED_CASES} cases failed with {all_failures} independent failing receipts; preserve {} and do not fit after contact", output.display());
}

/// The configuration parses, every declared name resolves, and no seed repeats — checked without
/// executing a single row.
#[test]
fn holdout_v4_configuration_is_well_formed() {
    let cases = cases();
    assert_eq!(cases.len(), EXPECTED_CASES);
    let mut seeds = Vec::new();
    for case in &cases {
        if case.source == "generated" {
            let _ = (
                world(case.source_world, case.source_tempo),
                grammar(case.source_grammar),
                composer(case.composer),
                options(case.source_language),
                profile(case.source_profile),
            );
            assert!(case.beats.parse::<f64>().is_ok_and(|b| b > 0.0));
            assert!(matches!(case.story, "demo" | "deflected"));
            seeds.push(case.source_seed);
        }
        let _ = (
            world(case.target_world, case.target_tempo),
            grammar(case.target_grammar),
            options(case.target_language),
            profile(case.target_profile),
        );
        assert!(case
            .target_tonic
            .parse::<i32>()
            .is_ok_and(|t| (0..12).contains(&t)));
        seeds.push(case.target_seed);
        assert!(
            matches!(
                case.selection,
                "established"
                    | "groove-free-established"
                    | "motif-only"
                    | "bass-only"
                    | "groove-only"
                    | "groove+motif"
                    | "groove+bass"
                    | "groove+harmonic"
                    | "v1-motif-only"
            ) || preset(case.selection).is_some(),
            "{}: undeclared selection {}",
            case.id,
            case.selection
        );
    }
    let mut sorted = seeds.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), seeds.len(), "a seed repeats");
}
