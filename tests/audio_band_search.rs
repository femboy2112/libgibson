//! Exploratory BAND search — NOT a holdout.
//!
//! Enumerates generated sources across worlds × languages × grammars × composers × traces ×
//! lengths × drum restraints × tempi × seeds and runs each through `perform_checked` under
//! `PerformanceProfile::BAND`. Every case is classified exactly once:
//!
//! - `admitted` — the checked performance satisfied its `PerformanceReceipt`;
//! - `refused`  — the candidate was refused before realization (`PerformanceRejection::Refused`);
//! - `rejected` — a realized candidate failed its own receipt (`PerformanceRejection::Rejected`);
//! - `panicked` — the library panicked (always an internal red).
//!
//! Configuration is read from the environment so a search can be narrowed to one family or
//! broadened without editing the harness. The search guides source repairs before a freeze; it is
//! never itself evidence for a frozen head (that is what a declared holdout is for).
//!
//! ```text
//! BAND_SEARCH_SEEDS=96800000:4            first seed : count
//! BAND_SEARCH_WORLDS=BLACK_ICE,VAPOR95     default: all three
//! BAND_SEARCH_LANGUAGES=fusion             default: simple,fusion
//! BAND_SEARCH_GRAMMARS=DeflectedLift       default: all six
//! BAND_SEARCH_COMPOSERS=StructuralR9       default: all three
//! BAND_SEARCH_TRACES=demo,deflected        default: both
//! BAND_SEARCH_LENGTHS=2.5,7.25,33.25       default: 2.5,4,7.25,13.25,16,33.25,64
//! BAND_SEARCH_RESTRAINTS=Balanced          default: all four
//! BAND_SEARCH_TEMPI=nominal,132            default: nominal
//! BAND_SEARCH_THREADS=6                    default: 6
//! BAND_SEARCH_OUT=/path/results.tsv        default: none (summary only)
//! ```
//!
//! Run: `cargo test --release --test audio_band_search -- --ignored --nocapture`.
use gibson::audio::human_music::{
    composer::Composer,
    contract::CompositionGrammar,
    functor::perform_checked,
    language::MusicalLanguage,
    percussion::DrumRestraint,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    receipt::PerformanceRejection,
    semantic::{deflected_lift_trace, demo_trace},
    MusicWorld, SongMap,
};
use std::collections::BTreeMap;
use std::io::Write;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

fn list(var: &str, default: &[&str]) -> Vec<String> {
    match std::env::var(var) {
        Ok(v) if !v.trim().is_empty() => v.split(',').map(|s| s.trim().to_string()).collect(),
        _ => default.iter().map(|s| s.to_string()).collect(),
    }
}

fn world(name: &str, tempo: &str) -> MusicWorld {
    let mut world = match name {
        "BLACK_ICE" => MusicWorld::black_ice(),
        "SWISS_SIGNAL" => MusicWorld::swiss_signal(),
        "VAPOR95" => MusicWorld::vapor95(),
        _ => panic!("unknown world {name}"),
    };
    if tempo != "nominal" {
        world.tempo_bpm = tempo.parse().expect("tempo");
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
        _ => panic!("unknown grammar {name}"),
    }
}

fn composer(name: &str) -> Composer {
    match name {
        "StructuralR9" => Composer::StructuralR9,
        "MeaningDirected" => Composer::MeaningDirected,
        "StablePropulsion" => Composer::StablePropulsion,
        _ => panic!("unknown composer {name}"),
    }
}

fn language(name: &str) -> MusicalLanguage {
    match name {
        "simple" => MusicalLanguage::simple(),
        "fusion" => MusicalLanguage::fusion_conversation(),
        _ => panic!("unknown language {name}"),
    }
}

fn restraint(name: &str) -> DrumRestraint {
    match name {
        "Foundation" => DrumRestraint::Foundation,
        "Balanced" => DrumRestraint::Balanced,
        "Expressive" => DrumRestraint::Expressive,
        "Busy" => DrumRestraint::Busy,
        _ => panic!("unknown restraint {name}"),
    }
}

/// One generated song (composition is independent of world, language, restraint and tempo).
#[derive(Clone)]
struct SongKey {
    trace: String,
    length: String,
    seed: u64,
    grammar: String,
    composer: String,
}

#[test]
#[ignore = "exploratory search; release only; configure with BAND_SEARCH_* (see module docs)"]
fn band_search() {
    let seeds = std::env::var("BAND_SEARCH_SEEDS").unwrap_or_else(|_| "96800000:1".into());
    let (first, count) = seeds
        .split_once(':')
        .expect("BAND_SEARCH_SEEDS=first:count");
    let first: u64 = first.parse().expect("first seed");
    let count: u64 = count.parse().expect("seed count");
    let worlds = list(
        "BAND_SEARCH_WORLDS",
        &["BLACK_ICE", "SWISS_SIGNAL", "VAPOR95"],
    );
    let languages = list("BAND_SEARCH_LANGUAGES", &["simple", "fusion"]);
    let grammars = list(
        "BAND_SEARCH_GRAMMARS",
        &[
            "HookArc",
            "LoopEvolution",
            "RiffDrive",
            "WorldSwitch",
            "DeflectedLift",
            "PropulsiveReturn",
        ],
    );
    let composers = list(
        "BAND_SEARCH_COMPOSERS",
        &["StructuralR9", "MeaningDirected", "StablePropulsion"],
    );
    let traces = list("BAND_SEARCH_TRACES", &["demo", "deflected"]);
    let lengths = list(
        "BAND_SEARCH_LENGTHS",
        &["2.5", "4", "7.25", "13.25", "16", "33.25", "64"],
    );
    let restraints = list(
        "BAND_SEARCH_RESTRAINTS",
        &["Foundation", "Balanced", "Expressive", "Busy"],
    );
    let tempi = list("BAND_SEARCH_TEMPI", &["nominal"]);
    let threads: usize = std::env::var("BAND_SEARCH_THREADS")
        .ok()
        .and_then(|t| t.parse().ok())
        .unwrap_or(6);

    let mut songs = Vec::new();
    for seed in first..first + count {
        for t in &traces {
            for l in &lengths {
                for g in &grammars {
                    for c in &composers {
                        songs.push(SongKey {
                            trace: t.clone(),
                            length: l.clone(),
                            seed,
                            grammar: g.clone(),
                            composer: c.clone(),
                        });
                    }
                }
            }
        }
    }
    let songs = Arc::new(songs);
    let next = Arc::new(Mutex::new(0usize));
    let rows: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let tally: Arc<Mutex<BTreeMap<String, usize>>> = Arc::new(Mutex::new(BTreeMap::new()));
    let families: Arc<Mutex<BTreeMap<String, usize>>> = Arc::new(Mutex::new(BTreeMap::new()));
    let per_song = worlds.len() * languages.len() * restraints.len() * tempi.len();

    std::thread::scope(|scope| {
        for _ in 0..threads {
            let (songs, next, rows, tally, families) = (
                songs.clone(),
                next.clone(),
                rows.clone(),
                tally.clone(),
                families.clone(),
            );
            let (worlds, languages, restraints, tempi) = (&worlds, &languages, &restraints, &tempi);
            scope.spawn(move || loop {
                let i = {
                    let mut n = next.lock().unwrap();
                    let i = *n;
                    *n += 1;
                    i
                };
                let Some(key) = songs.get(i) else { break };
                let beats: f64 = key.length.parse().expect("length");
                let trace = match key.trace.as_str() {
                    "demo" => demo_trace(beats),
                    "deflected" => deflected_lift_trace(beats),
                    other => panic!("unknown trace {other}"),
                };
                let song = catch_unwind(AssertUnwindSafe(|| {
                    SongMap::compose(
                        &trace,
                        key.seed,
                        Some(grammar(&key.grammar)),
                        composer(&key.composer),
                    )
                }));
                let mut local = Vec::with_capacity(per_song);
                for w in worlds {
                    for lang in languages {
                        for r in restraints {
                            for tempo in tempi {
                                let (status, detail) = match &song {
                                    Err(_) => ("panicked".to_string(), "compose".to_string()),
                                    Ok(song) => {
                                        let world = world(w, tempo);
                                        let opts = PerformanceOptions {
                                            language: language(lang),
                                            ..PerformanceOptions::default()
                                        };
                                        let profile = PerformanceProfile::BAND
                                            .with_drum_restraint(restraint(r));
                                        match catch_unwind(AssertUnwindSafe(|| {
                                            perform_checked(song, &world, opts, profile)
                                        })) {
                                            Err(_) => {
                                                ("panicked".to_string(), "perform".to_string())
                                            }
                                            Ok(Ok(_)) => ("admitted".to_string(), String::new()),
                                            Ok(Err(PerformanceRejection::Refused(e))) => {
                                                ("refused".to_string(), format!("{e}"))
                                            }
                                            Ok(Err(PerformanceRejection::Rejected(receipt))) => (
                                                "rejected".to_string(),
                                                receipt.failures().join("; "),
                                            ),
                                        }
                                    }
                                };
                                local.push((
                                    format!(
                                        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                                        key.seed,
                                        key.trace,
                                        key.length,
                                        key.grammar,
                                        key.composer,
                                        w,
                                        lang,
                                        r,
                                        tempo,
                                        status,
                                        detail
                                    ),
                                    status,
                                    detail,
                                ));
                            }
                        }
                    }
                }
                let mut t = tally.lock().unwrap();
                let mut f = families.lock().unwrap();
                let mut out = rows.lock().unwrap();
                for (row, status, detail) in local {
                    *t.entry(status.clone()).or_default() += 1;
                    if status != "admitted" {
                        // A family is the failure text with its counts removed.
                        let family: String = detail
                            .chars()
                            .map(|ch| if ch.is_ascii_digit() { '#' } else { ch })
                            .collect();
                        *f.entry(format!("{status}: {family}")).or_default() += 1;
                    }
                    out.push(row);
                }
            });
        }
    });

    let mut rows = rows.lock().unwrap().clone();
    rows.sort();
    if let Ok(path) = std::env::var("BAND_SEARCH_OUT") {
        let mut f = std::fs::File::create(&path).expect("output file");
        writeln!(
            f,
            "seed\ttrace\tlength\tgrammar\tcomposer\tworld\tlanguage\trestraint\ttempo\tstatus\tdetail"
        )
        .unwrap();
        for r in &rows {
            writeln!(f, "{r}").unwrap();
        }
    }
    let tally = tally.lock().unwrap();
    let total: usize = tally.values().sum();
    println!("BAND search: {total} performances");
    for (k, v) in tally.iter() {
        println!("  {k}: {v}");
    }
    for (k, v) in families.lock().unwrap().iter() {
        println!("  [{v}] {k}");
    }
    for r in rows.iter().filter(|r| !r.contains("\tadmitted\t")).take(60) {
        println!("  {r}");
    }
}
