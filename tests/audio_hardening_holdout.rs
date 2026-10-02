//! Hardening holdout v2: predeclared, one contact. This ignored test is never a tuning loop.
//! Configuration and harness are committed before execution; the output directory must be new.
//! v1's observers are inherited verbatim (`inspect`); v2 adds the profile, fidelity, anchor,
//! percussion and declared-expectation laws. A red source still proceeds to extraction and lift.
use gibson::audio::human_music::{
    action::Agent,
    composer::Composer,
    contract::CompositionGrammar,
    cover::{
        cover_candidate, cover_skeleton, AxisEvidence, CoverAxis, CoverConformance, CoverError,
        CoverFidelityPreset, CoverFidelityProfile, CoverFreedom, CoverKnowledge, CoverMap,
        CoverSpec, CoverTarget, FidelityReport, OrderedChart, SkeletonSchedule,
    },
    fingerprint::CanonicalFingerprint,
    functor::{orchestration_violations, perform_with_profile, Composition},
    identity::IdentityDiagnostics,
    language::MusicalLanguage,
    percussion::{DrumRestraint, PercussionPolicy},
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    reference_song::{ReferenceSong, DERIVED_HARMONY_METHOD},
    rhythm::MetricPosition,
    score::Role,
    semantic::{deflected_lift_trace, demo_trace},
    song::{AnchorReport, SongMap, SongMapConformance},
    temporal::TemporalPitchDiagnostics,
    voice::{self, HeardWindows, ObservedLifetimePolicy, VoiceEventId},
    witness, MusicWorld,
};
use std::{
    any::Any,
    fs::{self, File, OpenOptions},
    io::Write,
    panic::{catch_unwind, AssertUnwindSafe},
    path::Path,
};

const CONFIG: &str = include_str!("../docs/fixtures/humanmusic-hardening/holdout-v2/config.tsv");
const ODE: &str = include_str!("../docs/fixtures/humanmusic-cover/ode-import/reference.tsv");
const SWING: &str =
    include_str!("../docs/fixtures/humanmusic-cover/swing-partial/ordered-chart.tsv");
const EXPECTED_CASES: usize = 48;
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
        "DeflectedLift" => CompositionGrammar::DeflectedLift,
        "PropulsiveReturn" => CompositionGrammar::PropulsiveReturn,
        _ => panic!("unknown declared grammar {name}"),
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
    match name {
        "pocket" => PerformanceProfile::POCKET,
        "band-foundation" => {
            PerformanceProfile::BAND.with_drum_restraint(DrumRestraint::Foundation)
        }
        "band-balanced" => PerformanceProfile::BAND.with_drum_restraint(DrumRestraint::Balanced),
        "band-expressive" => {
            PerformanceProfile::BAND.with_drum_restraint(DrumRestraint::Expressive)
        }
        "band-busy" => PerformanceProfile::BAND.with_drum_restraint(DrumRestraint::Busy),
        _ => panic!("unknown declared profile {name}"),
    }
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
/// The declared lawful refusal family (see the configuration header).
fn lawful_refusal(error: &CoverError) -> bool {
    match error {
        CoverError::ConflictingPins => true,
        CoverError::Invalid(why) => {
            why.contains("outside target vocabulary")
                || why.contains("no lawful harmony contains the pinned simultaneous attacks")
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
    fn attempt<T>(
        &mut self,
        phase: &str,
        law: &str,
        f: impl FnOnce() -> Result<T, String>,
    ) -> Option<T> {
        match catch_unwind(AssertUnwindSafe(f)) {
            Ok(Ok(value)) => {
                self.emit(
                    phase,
                    law,
                    "PASS",
                    "returned a candidate; admission checked separately",
                );
                Some(value)
            }
            Ok(Err(error)) => {
                self.emit(phase, law, "FAIL", &error);
                None
            }
            Err(panic) => {
                self.emit(phase, law, "PANIC", &panic_text(panic));
                None
            }
        }
    }
}

/// Same pipeline constituents as CoverPipelineReceipt, evaluated separately so an observer
/// panic cannot conceal another observer's result. These routes share repository provenance.
fn inspect(c: &Composition, world: &MusicWorld, phase: &str, receipt: &mut Receipt<'_>) {
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
    receipt.check(phase, "score-domain", || match c.score.validate() {
        Ok(()) => (true, "all score domains valid".into()),
        Err(error) => (false, error),
    });
    receipt.check(phase, "song-projection", || {
        let law = SongMapConformance::check(&c.song, &c.perf, &c.score);
        (law.passes(), law.report())
    });
    receipt.check(phase, "pitch-function-presence", || {
        let missing: Vec<_> = c
            .score
            .notes
            .iter()
            .filter(|n| n.function.is_none())
            .collect();
        (missing.is_empty(), format!("unclassified={missing:?}"))
    });
    receipt.check(phase, "temporal-function", || {
        let law = TemporalPitchDiagnostics::measure(&c.perf, &c.score);
        (law.false_function_claims == 0, law.report())
    });
    receipt.check(phase, "held-identity", || {
        let law = IdentityDiagnostics::measure_score(&c.score, &c.perf.contexts, world);
        (
            law.flips().count() == 0,
            law.report(&c.score.notes, &c.perf.contexts),
        )
    });
    receipt.check(phase, "action-receipts", || {
        let law = witness::audit(&c.perf, &c.score);
        (law.missing().count() == 0, law.report())
    });
    receipt.check(phase, "stage-boundary", || {
        // Keep the shared stage authority used by CoverPipelineReceipt. Raw performed-onset
        // disagreements are retained separately; inherited percussion jitter has a declared
        // boundary convention in orchestration_violations, not a new rule invented here.
        let mut violations: Vec<_> = c
            .score
            .notes
            .iter()
            .filter_map(|n| {
                let agent = match n.role {
                    Role::Lead => Agent::Lead,
                    Role::Bass => Agent::Bass,
                    Role::Keys => Agent::Keys,
                    Role::Pad => Agent::Pad,
                };
                (!c.perf.on_stage(agent, n.start_beat)).then_some((agent, n.start_beat))
            })
            .collect();
        violations.extend(
            c.score
                .drums
                .iter()
                .filter(|d| !c.perf.on_stage(Agent::Drums, d.start_beat))
                .map(|d| (Agent::Drums, d.start_beat)),
        );
        let legacy = orchestration_violations(&c.perf, &c.score);
        (
            legacy.is_empty(),
            format!("authoritative violations={legacy:?}; diagnostic raw-onset disagreements={violations:?}"),
        )
    });
    receipt.check(phase, "causal-hearing", || {
        let stale = c.score.stale_hearings();
        (stale.is_empty(), format!("stale={stale:?}"))
    });
    receipt.check(phase, "guard:hearing-ledger-coverage", || {
        let required = [
            ("keys", Role::Lead),
            ("bass", Role::Lead),
            ("drums", Role::Lead),
            ("bass", Role::Keys),
            ("pad", Role::Lead),
            ("pad", Role::Keys),
            ("pad", Role::Bass),
            ("drums", Role::Bass),
        ];
        let missing: Vec<_> = required
            .into_iter()
            .filter(|(listener, source)| {
                !c.score
                    .hearings
                    .iter()
                    .any(|h| h.listener == *listener && h.source == *source)
            })
            .collect();
        (
            missing.is_empty(),
            format!("missing={missing:?}; ledgers={}", c.score.hearings.len()),
        )
    });
    receipt.check(phase, "continuation-identity", || {
        let violations = voice::continuity_violations(&c.score.notes, &c.score.voice_continuity);
        (
            violations.is_empty(),
            format!(
                "violations={violations:?}; edges={:?}",
                c.score.voice_continuity
            ),
        )
    });
    receipt.check(phase, "guard:direct-voice-lifetime", || {
        let heard = HeardWindows::of_score(&c.score, world);
        let mut failures = Vec::new();
        if heard.policy() != ObservedLifetimePolicy::ExplicitContinuity {
            failures.push(format!(
                "modern profile silently observed as {:?}",
                heard.policy()
            ));
        }
        let spb = 60.0 / f64::from(c.score.tempo_bpm.max(1.0));
        for (i, n) in c.score.notes.iter().enumerate() {
            let envelope = voice::audible_end(
                n.start_beat,
                f64::from(n.dur_beats),
                voice::patch(world, n.role),
                c.score.tempo_bpm,
            );
            let id = VoiceEventId::of(n);
            let expected = c
                .score
                .voice_continuity
                .iter()
                .filter(|edge| edge.from == id)
                .map(|edge| edge.to.onset() + voice::MONO_CHOKE_SECS / spb)
                .fold(envelope, f64::min);
            let (start, end) = heard.windows()[i];
            if start.to_bits() != n.start_beat.to_bits() || end.to_bits() != expected.to_bits() {
                failures.push(format!(
                    "note {i}: observed ({start},{end}), expected ({},{expected})",
                    n.start_beat
                ));
            }
        }
        (failures.is_empty(), format!("failures={failures:?}"))
    });
    receipt.check(phase, "occupancy-contract", || {
        let violations = gibson::audio::human_music::occupancy::violations(&c.perf, &c.score, true);
        (violations.is_empty(), format!("violations={violations:?}"))
    });
    receipt.check(phase, "guard:semantic-occupancy", || {
        let mut failures = Vec::new();
        for role in [Role::Lead, Role::Bass] {
            let owners: Vec<_> = c
                .score
                .occupancy
                .iter()
                .filter(|o| o.role == role)
                .collect();
            if owners.len() != 1 {
                failures.push(format!(
                    "{role:?}: expected one authored owner, found {}",
                    owners.len()
                ));
            }
            for owner in owners {
                // Empty acoustic input is a deliberate mutation: it must not release source intent.
                for slot in &owner.rhythm {
                    if owner.yields_at(slot.beat) || owner.allows_comp_at(slot.beat, &[]) {
                        failures.push(format!(
                            "{role:?}: vacant acoustic stream stole reservation {slot:?}"
                        ));
                    }
                }
                for span in &owner.spans {
                    if !span.start.is_finite() || !span.end.is_finite() || span.end <= span.start {
                        failures.push(format!("{role:?}: invalid ownership span {span:?}"));
                    } else if owner.yields_at((span.start + span.end) / 2.0) {
                        failures.push(format!("{role:?}: owned midpoint yielded {span:?}"));
                    }
                }
            }
        }
        (
            failures.is_empty(),
            format!("failures={failures:?}; ownership={:?}", c.score.occupancy),
        )
    });
    receipt.check(phase, "phrase-destinations", || {
        let missing: Vec<_> = c
            .score
            .phrase_plans
            .iter()
            .filter(|p| {
                p.destination.start_beat < c.score.total_beats
                    && !c.score.role_notes(p.role).any(|n| {
                        n.start_beat == p.destination.start_beat && n.pitch == p.destination.pitch
                    })
            })
            .collect();
        (missing.is_empty(), format!("missing={missing:?}"))
    });
}

/// v2 laws beyond v1's `inspect`: every declared anchor the form had room for is stated, and
/// the drummer's percussion law is the one the profile declares.
fn inspect_v2(
    c: &Composition,
    profile: PerformanceProfile,
    phase: &str,
    receipt: &mut Receipt<'_>,
) {
    receipt.check(phase, "anchor-report", || {
        let report = AnchorReport::check(&c.song, &c.perf, &c.score);
        (report.violations().is_empty(), format!("{report:?}"))
    });
    receipt.check(phase, "guard:percussion-law", || {
        match (profile.percussion, &c.score.percussion) {
            (PercussionPolicy::Unarbitrated, None) => {
                (true, "historical unarbitrated drummer".into())
            }
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

fn run_case(case: &Case<'_>, receipt: &mut Receipt<'_>) {
    // --- Source and map. ---
    let mut source: Option<Composition> = None;
    let extracted: Result<(CoverMap, Option<FidelityReport>), CoverError> = match case.source {
        "generated" => {
            let world = world(case.source_world, case.source_tempo);
            let source_profile = profile(case.source_profile);
            let beats = case.beats.parse().expect("declared source length");
            let trace = match case.story {
                "demo" => demo_trace(beats),
                "deflected" => deflected_lift_trace(beats),
                _ => panic!("unknown story {}", case.story),
            };
            let composer = match case.composer {
                "StructuralR9" => Composer::StructuralR9,
                "MeaningDirected" => Composer::MeaningDirected,
                "StablePropulsion" => Composer::StablePropulsion,
                _ => panic!("unknown composer {}", case.composer),
            };
            let Some(generated) = receipt.attempt("source", "generation", || {
                let song = SongMap::compose(
                    &trace,
                    case.source_seed.parse().expect("source seed"),
                    Some(grammar(case.source_grammar)),
                    composer,
                );
                perform_with_profile(&song, &world, options(case.source_language), source_profile)
                    .map_err(|e| e.to_string())
            }) else {
                return;
            };
            receipt.check("source", "declared-grammar", || {
                (
                    generated.song.plan.contract.grammar == grammar(case.source_grammar),
                    format!(
                        "grammar={:?}; declared_composer={composer:?}; self_reported_composer={:?}",
                        generated.song.plan.contract.grammar,
                        generated.song.composer()
                    ),
                )
            });
            inspect(&generated, &world, "source", receipt);
            inspect_v2(&generated, source_profile, "source", receipt);
            let established = || {
                CoverSpec::established(&AnchorReport::check(
                    &generated.song,
                    &generated.perf,
                    &generated.score,
                ))
            };
            let result = match case.selection {
                "established" => {
                    CoverMap::extract(&generated, &world, established()).map(|m| (m, None))
                }
                "groove-free-established" => CoverMap::extract(
                    &generated,
                    &world,
                    established().with(CoverAxis::Groove, false),
                )
                .map(|m| (m, None)),
                "motif-only" => {
                    CoverMap::extract(&generated, &world, CoverSpec::new([CoverAxis::Motif]))
                        .map(|m| (m, None))
                }
                selection => {
                    let p = preset(selection).expect("declared selection");
                    CoverMap::extract_fidelity(
                        &generated,
                        &world,
                        &CoverFidelityProfile::preset(p),
                        Some(p),
                    )
                    .map(|(m, r)| (m, Some(r)))
                }
            };
            source = Some(generated);
            result
        }
        "ode" => {
            let Some(reference) = receipt.attempt("reference", "symbolic-ingestion", || {
                ReferenceSong::from_tsv(ODE, "sop").map_err(|e| e.to_string())
            }) else {
                return;
            };
            let derived = if case.selection.ends_with("-derived") {
                let Some(d) = receipt.attempt("reference", "derived-harmony", || {
                    reference
                        .derive_harmony(MetricPosition::new(2, 1).expect("two beats"))
                        .map_err(|e| e.to_string())
                }) else {
                    return;
                };
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
            let Some(chart) = receipt.attempt("reference", "ordered-chart-ingestion", || {
                OrderedChart::from_tsv(SWING).map_err(|e| e.to_string())
            }) else {
                return;
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
            let pass = matches!(case.expect, "refusal" | "lawful") && lawful_refusal(&e);
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
    receipt.check("extraction", "identity-present", || {
        (
            !map.spec.axes().is_empty(),
            format!(
                "pinned axes={:?} (a cover with no pinned identity is not a cover)",
                map.spec.axes()
            ),
        )
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
            let pass = matches!(case.expect, "refusal" | "lawful") && lawful_refusal(&e);
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
    inspect(&candidate, &target_world, "cover", receipt);
    inspect_v2(&candidate, target_profile, "cover", receipt);
    if let Some(law) = receipt.attempt("cover", "projection-observer", || {
        Ok(CoverConformance::check(&map, &candidate, &target_world))
    }) {
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
    if let Some(source) = source {
        receipt.check("cover", "guard:anti-replay-vs-source", || {
            let freedom = CoverFreedom::compare(&source, &candidate);
            (
                freedom.has_freedom(),
                format!("{freedom:?}; machine variation only, no listening claim"),
            )
        });
    }
}

#[test]
#[ignore = "hardening holdout v2: configuration and harness are committed before one-time execution"]
fn hardening_holdout_v2() {
    let cases = cases();
    assert_eq!(
        cases.len(),
        EXPECTED_CASES,
        "configuration cannot silently shrink"
    );
    // Declared profile: release. Under debug assertions `realize_policy` asserts the stage
    // boundary itself, turning a stage failure into a generation panic that erases every sibling
    // observer. Refuse before any output exists, so a wrong invocation cannot burn the contact.
    if cfg!(debug_assertions) {
        panic!(
            "declared execution profile is --release (debug assertions off); nothing was written"
        );
    }
    let output = std::env::var("HUMANMUSIC_HOLDOUT_OUT")
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
            "field\tvalue\ndebug_assertions\t{}\npackage_version\t{}\nconfig_bytes\t{}\nconfig_fingerprint\t{:016x}\node_fixture_fingerprint\t{:016x}\nswing_fixture_fingerprint\t{:016x}\nprofile_pocket\t{:016x}\nprofile_band\t{:016x}\nexpected_cases\t{EXPECTED_CASES}\n",
            cfg!(debug_assertions),
            env!("CARGO_PKG_VERSION"),
            CONFIG.len(),
            CONFIG.canonical_fingerprint(),
            ODE.canonical_fingerprint(),
            SWING.canonical_fingerprint(),
            PerformanceProfile::POCKET.canonical_fingerprint(),
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
