//! The **standing audit** of every declared number in a song's contract and its discourse goals.
//!
//! A field is exactly one of:
//!
//! - **Load-bearing** — some planner, realizer or law reads it: perturbing it changes the
//!   performance, or a law's verdict about the performance;
//! - **Diagnostic** — only a plan-level diagnostic reads it: perturbing it changes that
//!   diagnostic and neither the performance nor any law;
//! - **Descriptive** — recorded (it is in the song's fingerprints and the plan dump) and read by
//!   nothing: perturbing it changes neither the performance, any law, nor any diagnostic.
//!
//! The standing is declared once, in [`STANDING`]; each field's documentation must state the same
//! standing, and the perturbations below prove it under the historical (POCKET) and the band
//! (BAND) profiles. A field that starts being read goes red here until its documentation says so.

use super::super::cover::CoverSpec;
use super::super::diagnostics::{CoherenceDiagnostics, DiscourseDiagnostics};
use super::super::fingerprint::CanonicalFingerprint;
use super::super::functor::perform_with_profile;
use super::super::performance::PerformanceOptions;
use super::super::plan::CompositionPlan;
use super::super::policy::PerformanceProfile;
use super::super::receipt::PerformanceReceipt;
use super::super::semantic::{calm_loop, demo_trace, SemanticTrace};
use super::super::song::{AnchorReport, MotifRepetition, SongMap};
use super::super::world::MusicWorld;
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    LoadBearing,
    Diagnostic,
    Descriptive,
}

impl Standing {
    const ALL: [Standing; 3] = [
        Standing::LoadBearing,
        Standing::Diagnostic,
        Standing::Descriptive,
    ];

    fn marker(self) -> &'static str {
        match self {
            Standing::LoadBearing => "**Load-bearing**",
            Standing::Diagnostic => "**Diagnostic**",
            Standing::Descriptive => "**Descriptive**",
        }
    }
}

/// `(source, struct, field, standing)` — the one declaration of each field's standing.
const STANDING: &[(&str, &str, &str, Standing)] = &[
    (
        "contract.rs",
        "CoherenceContract",
        "grammar",
        Standing::LoadBearing,
    ),
    (
        "contract.rs",
        "CoherenceContract",
        "anchors",
        Standing::LoadBearing,
    ),
    (
        "contract.rs",
        "CoherenceContract",
        "recurrence_bars",
        Standing::LoadBearing,
    ),
    (
        "contract.rs",
        "CoherenceContract",
        "max_transform",
        Standing::Descriptive,
    ),
    (
        "contract.rs",
        "CoherenceContract",
        "phrase_bars",
        Standing::LoadBearing,
    ),
    (
        "contract.rs",
        "CoherenceContract",
        "resolution",
        Standing::LoadBearing,
    ),
    (
        "contract.rs",
        "CoherenceContract",
        "foreground_budget",
        Standing::LoadBearing,
    ),
    (
        "contract.rs",
        "CoherenceContract",
        "novelty_budget",
        Standing::Descriptive,
    ),
    (
        "discourse.rs",
        "PhraseGoal",
        "thematic_distance",
        Standing::Diagnostic,
    ),
    (
        "discourse.rs",
        "PhraseGoal",
        "harmonic_distance",
        Standing::Descriptive,
    ),
    (
        "discourse.rs",
        "PhraseGoal",
        "novelty_budget",
        Standing::Descriptive,
    ),
    (
        "discourse.rs",
        "MusicalThesis",
        "anchors",
        Standing::Descriptive,
    ),
    (
        "backbone.rs",
        "TimeScales",
        "phrase_bars",
        Standing::Descriptive,
    ),
];

const SOURCES: [(&str, &str); 3] = [
    ("contract.rs", include_str!("contract.rs")),
    ("discourse.rs", include_str!("discourse.rs")),
    ("backbone.rs", include_str!("backbone.rs")),
];

const SEED: u64 = 78_301_210;

const GRAMMARS: [CompositionGrammar; 6] = [
    CompositionGrammar::HookArc,
    CompositionGrammar::LoopEvolution,
    CompositionGrammar::RiffDrive,
    CompositionGrammar::WorldSwitch,
    CompositionGrammar::DeflectedLift,
    CompositionGrammar::PropulsiveReturn,
];

/// The documentation directly above `pub {field}:` inside `pub struct {strukt}`.
fn field_doc(file: &str, strukt: &str, field: &str) -> String {
    let src = SOURCES
        .iter()
        .find(|(f, _)| *f == file)
        .map(|(_, s)| *s)
        .expect("an audited source");
    let open = format!("pub struct {strukt} {{");
    let body = &src[src
        .find(&open)
        .unwrap_or_else(|| panic!("{strukt} in {file}"))..];
    let body = &body[..body.find("\n}\n").expect("the struct closes")];
    let lines: Vec<&str> = body.lines().collect();
    let decl = format!("pub {field}:");
    let at = lines
        .iter()
        .position(|l| l.trim_start().starts_with(&decl))
        .unwrap_or_else(|| panic!("{strukt}::{field} in {file}"));
    let mut doc: Vec<&str> = lines[..at]
        .iter()
        .rev()
        .take_while(|l| l.trim_start().starts_with("///"))
        .map(|l| l.trim_start().trim_start_matches("///").trim())
        .collect();
    doc.reverse();
    doc.join(" ")
}

/// The song [`SongMap::build`] writes for `grammar`, re-planned under an edited contract.
fn song_under(
    trace: &SemanticTrace,
    grammar: CompositionGrammar,
    edit: impl Fn(&mut CoherenceContract),
) -> SongMap {
    let timeline = IntentTimeline::walk(trace);
    let mut contract = CoherenceContract::for_grammar(grammar);
    edit(&mut contract);
    let plan =
        CompositionPlan::build_with_contract_for_beats(&timeline, trace.total_beats, contract);
    SongMap::from_plan_with(trace, SEED, timeline, plan, MotifRepetition::Develop)
}

/// Everything a reader of the song can observe of its performance.
#[derive(Debug, Clone, PartialEq)]
struct Heard {
    perf: u64,
    score: u64,
    receipt: PerformanceReceipt,
    established: CoverSpec,
    coherence: CoherenceDiagnostics,
    discourse: DiscourseDiagnostics,
}

fn hear(song: &SongMap, profile: PerformanceProfile) -> Result<Heard, String> {
    let world = MusicWorld::black_ice();
    let c = perform_with_profile(song, &world, PerformanceOptions::default(), profile)
        .map_err(|e| e.to_string())?;
    Ok(Heard {
        perf: c.perf.canonical_fingerprint(),
        score: c.score.canonical_fingerprint(),
        receipt: PerformanceReceipt::measure_under(&c, &world, profile),
        established: CoverSpec::established(&AnchorReport::check(&c.song, &c.perf, &c.score)),
        coherence: CoherenceDiagnostics::measure(&c.song.plan, &c.score),
        discourse: DiscourseDiagnostics::measure(&c.song.plan, &c.score),
    })
}

const PROFILES: [PerformanceProfile; 2] = [PerformanceProfile::POCKET, PerformanceProfile::BAND];

#[test]
fn every_audited_field_documents_its_standing() {
    let mut wrong = Vec::new();
    for &(file, strukt, field, standing) in STANDING {
        let doc = field_doc(file, strukt, field);
        let declared: Vec<Standing> = Standing::ALL
            .into_iter()
            .filter(|s| doc.contains(s.marker()))
            .collect();
        if declared != [standing] {
            wrong.push(format!(
                "{strukt}::{field} is {standing:?}; its documentation declares {declared:?}"
            ));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn replanning_under_the_grammars_own_contract_is_the_song_build_writes() {
    let trace = demo_trace(64.0);
    for g in GRAMMARS {
        assert_eq!(
            song_under(&trace, g, |_| {}).canonical_fingerprint(),
            SongMap::build(&trace, SEED, Some(g)).canonical_fingerprint(),
            "{g:?}"
        );
    }
}

/// Every Descriptive field (and `recurrence_bars` outside DeflectedLift) moved at once: the
/// song's record changes, and nothing any reader observes of its performance does.
#[test]
fn descriptive_fields_are_read_by_nothing() {
    let trace = demo_trace(64.0);
    for g in GRAMMARS {
        let base = SongMap::build(&trace, SEED, Some(g));
        let mut moved = song_under(&trace, g, |c| {
            c.max_transform = 0.99;
            c.novelty_budget = 0.01;
            if g != CompositionGrammar::DeflectedLift {
                c.recurrence_bars += 4;
            }
        });
        for goal in &mut moved.plan.discourse.goals {
            goal.harmonic_distance = 1.0 - goal.harmonic_distance;
            goal.novelty_budget = 0.99;
        }
        moved.plan.discourse.thesis.anchors.clear();
        if let Some(b) = moved.plan.backbone.as_mut() {
            b.scales.phrase_bars += 3;
        }
        assert_ne!(
            moved.canonical_fingerprint(),
            base.canonical_fingerprint(),
            "{g:?}: the perturbation is recorded"
        );
        for profile in PROFILES {
            assert_eq!(hear(&moved, profile), hear(&base, profile), "{g:?}");
        }
    }
}

/// `thematic_distance` moves the discourse diagnostic's return strength and nothing else.
#[test]
fn thematic_distance_is_read_only_by_the_discourse_diagnostic() {
    let trace = demo_trace(64.0);
    let mut moved_any = false;
    for g in GRAMMARS {
        let base = SongMap::build(&trace, SEED, Some(g));
        let mut moved = base.clone();
        for goal in &mut moved.plan.discourse.goals {
            goal.thematic_distance = 1.0 - goal.thematic_distance;
        }
        for profile in PROFILES {
            let (a, b) = (hear(&moved, profile), hear(&base, profile));
            let (Ok(a), Ok(b)) = (a, b) else {
                panic!("{g:?} {profile:?} performs");
            };
            assert_eq!(
                (a.perf, a.score, &a.receipt, &a.established, &a.coherence),
                (b.perf, b.score, &b.receipt, &b.established, &b.coherence),
                "{g:?}"
            );
            moved_any |= a.discourse != b.discourse;
        }
    }
    assert!(moved_any, "no discourse diagnostic reads thematic_distance");
}

/// Each Load-bearing field, moved alone on one witness song, changes the performance or a law's
/// verdict about it.
#[test]
fn load_bearing_fields_change_what_is_heard() {
    let demo = demo_trace(64.0);
    let hook = CompositionGrammar::HookArc;
    let witnesses: [(&str, SongMap, SongMap); 6] = [
        (
            "grammar",
            song_under(&demo, hook, |_| {}),
            song_under(&demo, hook, |c| {
                c.grammar = CompositionGrammar::DeflectedLift
            }),
        ),
        (
            "anchors",
            song_under(&demo, hook, |_| {}),
            song_under(&demo, hook, |c| {
                c.anchors.retain(|a| *a != CoherenceAnchor::Motif)
            }),
        ),
        (
            // Read only by DeflectedLift's fixed-tiling fallback: a trace too sparse to bind.
            "recurrence_bars",
            song_under(&calm_loop(64.0), CompositionGrammar::DeflectedLift, |_| {}),
            song_under(&calm_loop(64.0), CompositionGrammar::DeflectedLift, |c| {
                c.recurrence_bars = 8
            }),
        ),
        (
            "phrase_bars",
            song_under(&demo, hook, |_| {}),
            song_under(&demo, hook, |c| c.phrase_bars = 2),
        ),
        (
            "resolution",
            song_under(&demo, hook, |_| {}),
            song_under(&demo, hook, |c| c.resolution = ResolutionPolicy::Loop),
        ),
        (
            "foreground_budget",
            song_under(&demo, hook, |_| {}),
            song_under(&demo, hook, |c| c.foreground_budget = 2),
        ),
    ];
    for (field, base, moved) in &witnesses {
        let (a, b) = (
            hear(base, PerformanceProfile::POCKET),
            hear(moved, PerformanceProfile::POCKET),
        );
        let (Ok(a), Ok(b)) = (a, b) else {
            panic!("{field}: the witness performs");
        };
        assert!(
            a.perf != b.perf
                || a.score != b.score
                || a.receipt != b.receipt
                || a.established != b.established,
            "{field} is declared Load-bearing and moving it changes nothing heard"
        );
    }
}
