//! Predeclared, one-contact consolidated holdout. This ignored test is never a tuning loop.
//! Compile before configuration commit; execute only after root freezes the architecture.
//! Each observer is caught separately: a red source still proceeds to cover extraction/lift.
//! No R16 pitch equality or all-onset hat-lattice rule is asserted here.
use gibson::audio::human_music::{
    action::Agent,
    composer::Composer,
    contract::CompositionGrammar,
    cover::{
        cover_candidate, CoverAxis, CoverConformance, CoverFreedom, CoverKnowledge, CoverMap,
        CoverSpec, CoverTarget,
    },
    fingerprint::CanonicalFingerprint,
    functor::{orchestration_violations, perform_with_profile, Composition},
    identity::IdentityDiagnostics,
    language::MusicalLanguage,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    reference_song::ReferenceSong,
    score::Role,
    semantic::{deflected_lift_trace, demo_trace},
    song::{SongMap, SongMapConformance},
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

const CONFIG: &str = include_str!("../docs/fixtures/humanmusic-consolidation/fresh/config.tsv");
const ODE: &str = include_str!("../docs/fixtures/humanmusic-cover/ode-import/reference.tsv");
const EXPECTED_CASES: usize = 36;

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
    spec: &'a str,
    target_seed: &'a str,
    target_world: &'a str,
    target_grammar: &'a str,
    target_tempo: &'a str,
    target_tonic: &'a str,
    target_language: &'a str,
}
fn cases() -> Vec<Case<'static>> {
    CONFIG
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .skip(1)
        .map(|line| {
            let f: Vec<_> = line.split('\t').collect();
            assert_eq!(f.len(), 17, "invalid configuration row: {line}");
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
                spec: f[10],
                target_seed: f[11],
                target_world: f[12],
                target_grammar: f[13],
                target_tempo: f[14],
                target_tonic: f[15],
                target_language: f[16],
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
fn selection(name: &str, song: Option<&SongMap>) -> CoverSpec {
    match name {
        "default" => CoverSpec::from_contract(&song.expect("generated source").plan.contract),
        "groove-free" => CoverSpec::from_contract(&song.expect("generated source").plan.contract)
            .with(CoverAxis::Groove, false),
        "all" => CoverSpec::all(),
        "motif-only" => CoverSpec::new([CoverAxis::Motif]),
        "riff-bass" => CoverSpec::new([
            CoverAxis::Riff,
            CoverAxis::BassFigure,
            CoverAxis::Groove,
            CoverAxis::Form,
        ]),
        _ => panic!("unknown declared selection {name}"),
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

fn run_case(case: &Case<'_>, receipt: &mut Receipt<'_>) {
    let (source, map, requested) = match case.source {
        "generated" => {
            let world = world(case.source_world, case.source_tempo);
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
            let Some(source) = receipt.attempt("source", "generation", || {
                let song = SongMap::compose(
                    &trace,
                    case.source_seed.parse().expect("source seed"),
                    Some(grammar(case.source_grammar)),
                    composer,
                );
                perform_with_profile(
                    &song,
                    &world,
                    options(case.source_language),
                    PerformanceProfile::POCKET,
                )
                .map_err(|e| e.to_string())
            }) else {
                return;
            };
            // `SongMap::composer()` infers StablePropulsion from any PropulsiveReturn phenomenal
            // target, so it cannot tell an R9/MeaningDirected call on that grammar apart. The
            // declared call is the authority; the self-report is retained as detail only.
            receipt.check("source", "declared-grammar", || {
                (
                    source.song.plan.contract.grammar == grammar(case.source_grammar),
                    format!(
                        "grammar={:?}; declared_composer={composer:?}; self_reported_composer={:?}",
                        source.song.plan.contract.grammar,
                        source.song.composer()
                    ),
                )
            });
            inspect(&source, &world, "source", receipt);
            let requested = selection(case.spec, Some(&source.song));
            let Some(map) = receipt.attempt("extraction", "generated-projection", || {
                CoverMap::extract(&source, &world, requested.clone()).map_err(|e| e.to_string())
            }) else {
                return;
            };
            (Some(source), map, requested)
        }
        "ode" => {
            let Some(reference) = receipt.attempt("reference", "symbolic-ingestion", || {
                ReferenceSong::from_tsv(ODE, "sop").map_err(|e| e.to_string())
            }) else {
                return;
            };
            receipt.check("reference", "observed-source-domain", || {
                let result = reference.melody_score().map_err(|e| e.to_string()).and_then(|s| s.validate());
                (result.is_ok(), format!("validation={result:?}; voice={}; key={}; tempo={:?}; length={:?}; fixture={:016x}", reference.selected_voice, reference.tonic, reference.tempo, reference.length, ODE.canonical_fingerprint()))
            });
            let requested = selection(case.spec, None);
            let Some(map) = receipt.attempt("extraction", "external-projection", || {
                reference
                    .extract(requested.clone())
                    .map_err(|e| e.to_string())
            }) else {
                return;
            };
            receipt.check("reference", "unobserved-axes-remain-unknown", || {
                let absent = [
                    CoverAxis::Groove,
                    CoverAxis::HarmonicContour,
                    CoverAxis::HarmonicLoop,
                    CoverAxis::Form,
                    CoverAxis::Orchestration,
                    CoverAxis::BassFigure,
                ];
                let wrong: Vec<_> = absent
                    .into_iter()
                    .filter(|a| map.knowledge(*a) != CoverKnowledge::Unknown)
                    .collect();
                (
                    wrong.is_empty(),
                    format!("wrongly inferred axes={wrong:?}; observed selected melody only"),
                )
            });
            (None, map, requested)
        }
        _ => panic!("unknown source {}", case.source),
    };
    receipt.check("extraction", "requested-axes-not-downgraded", || {
        let missing: Vec<_> = requested
            .axes()
            .iter()
            .copied()
            .filter(|a| map.knowledge(*a) != CoverKnowledge::Invariant)
            .collect();
        (
            missing.is_empty(),
            format!(
                "requested={:?}; unavailable={missing:?}; map={:016x}; knowledge={:?}",
                requested.axes(),
                map.canonical_fingerprint(),
                CoverAxis::ALL.map(|a| (a, map.knowledge(a)))
            ),
        )
    });
    receipt.check("extraction", "guard:map-validation", || {
        let result = map.validate();
        (result.is_ok(), format!("{result:?}"))
    });
    let mut world = world(case.target_world, case.target_tempo);
    world.tonic_pc = case.target_tonic.parse().expect("declared target tonic");
    let Some(candidate) = receipt.attempt("cover", "candidate-lift", || {
        cover_candidate(
            &map,
            CoverTarget {
                world: &world,
                seed: case.target_seed.parse().expect("declared target seed"),
                grammar: grammar(case.target_grammar),
                options: options(case.target_language),
                profile: PerformanceProfile::POCKET,
            },
        )
        .map_err(|e| e.to_string())
    }) else {
        return;
    };
    inspect(&candidate, &world, "cover", receipt);
    if let Some(law) = receipt.attempt("cover", "projection-observer", || {
        Ok(CoverConformance::check(&map, &candidate, &world))
    }) {
        receipt.emit(
            "cover",
            "metric-length",
            if law.length_passed { "PASS" } else { "FAIL" },
            "canonical metric extent",
        );
        for axis in requested.axes() {
            if let Some(check) = law.checks.iter().find(|c| c.axis == *axis) {
                receipt.emit(
                    "cover",
                    axis.label(),
                    if check.passed { "PASS" } else { "FAIL" },
                    &format!("{}; {}", check.relation, check.detail),
                );
            } else {
                receipt.emit(
                    "cover",
                    axis.label(),
                    "FAIL",
                    "requested axis has no conformance receipt",
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
#[ignore = "fresh declared holdout: root must commit configuration after architectural freeze before one-time execution"]
fn consolidated_fresh_holdout() {
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
            "field\tvalue\ndebug_assertions\t{}\npackage_version\t{}\nconfig_bytes\t{}\nconfig_fingerprint\t{:016x}\node_fixture_bytes\t{}\node_fixture_fingerprint\t{:016x}\nexpected_cases\t{EXPECTED_CASES}\n",
            cfg!(debug_assertions),
            env!("CARGO_PKG_VERSION"),
            CONFIG.len(),
            CONFIG.canonical_fingerprint(),
            ODE.len(),
            ODE.canonical_fingerprint(),
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
