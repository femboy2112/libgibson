//! HumanMusic proving ground.
//!
//! Renders ONE frozen semantic event fixture through three MusicWorlds — BLACK_ICE,
//! VAPOR95, SWISS_SIGNAL — and writes an offline WAV per world plus a structural dump
//! (form, harmony, motif development, groove summary, provenance) and render receipts.
//!
//! The point: the three tracks are recognizably the *same music* in FORM and meaning
//! (same section skeleton, same motif identity, same resolutions) yet clearly different
//! musical dialects. Machines can certify the structure and safety; taste is for your ears
//! — the WAVs are the artifact to listen to.
//!
//! Run:
//!   cargo run --release --example human_music_lab
//!   cargo run --release --example human_music_lab -- --world=vapor95 --out=/tmp/hm
//!   cargo run --release --example human_music_lab -- --beats=96 --seed=2112
//!   cargo run --release --example human_music_lab -- --language=simple      # same song, plain speech
//!   cargo run --release --example human_music_lab -- --actions=off          # mood without action
//!   cargo run --release --example human_music_lab -- --responses=clockwork  # fixed-slot answers
//!   cargo run --release --example human_music_lab -- --calls=every          # every statement calls
//!   cargo run --release --example human_music_lab -- --manifest=fixed       # one choreography
//!   cargo run --release --example human_music_lab -- --ab                   # six A/Bs (BLACK_ICE) +
//!                                                                          # independent vs coupled x3
//!   cargo run --release --example human_music_lab -- --coupling=independent # the R7b realization
//!   cargo run --release --example human_music_lab -- --sonority-detail=unowned  # every offending slice
//!   cargo run --release --example human_music_lab -- --sonority-detail=nonroot  # every non-root bass
//!   cargo run --release --example human_music_lab -- --harmonic-reference   # same Score, clean timbre
//!   cargo run --release --example human_music_lab -- --production=nosat,dry  # remove single factors
//!   cargo run --release --example human_music_lab -- --stems --pair-stems    # solo + pitched pairs
//!   cargo run --release --example human_music_lab -- --dump-notes           # <world>.notes.tsv
//!   cargo run --release --example human_music_lab -- --acceptance           # Round IX: ONE SongMap,
//!                                          # 4 performances (3 rooms fusion + BLACK_ICE simple)
//!   cargo run --release --example human_music_lab -- --meaning              # Round X: ONE story, two
//!                                          # composers (A = R9, B = meaning-directed), same band
//!
//! Every mode below `--calibrate`/`--ab`/`--stems` builds ONE [`SongMap`] and performs it in each
//! room (Round IX): the song is never re-composed per world, and each performance's
//! `SongMapConformance` is printed next to its render.

use std::path::PathBuf;

use gibson::audio::buffer::StereoBlock;
use gibson::audio::human_music::action::ManifestationPolicy;
use gibson::audio::human_music::backbone::lead_sheet;
use gibson::audio::human_music::budget::ComplexityReport;
use gibson::audio::human_music::composer::{
    compose_meaning, Composer, CompositionReport, CompositionalPrior,
};
use gibson::audio::human_music::contract::CompositionGrammar;
use gibson::audio::human_music::diagnostics::{
    ActionDiagnostics, CoherenceDiagnostics, DiscourseDiagnostics, HarmonyContextDiagnostics,
    LeadOutlineDiagnostics, RealizationDiagnostics, RigidityDiagnostics,
};
use gibson::audio::human_music::functor::{compose_full, compose_with_grammar, perform, SfxAudit};
use gibson::audio::human_music::harmony::ChordSpan;
use gibson::audio::human_music::language::MusicalLanguage;
use gibson::audio::human_music::meaning::Commutation;
use gibson::audio::human_music::performance::{
    CallPolicy, EnsembleCoupling, PerformanceOptions, ResponseMode,
};
use gibson::audio::human_music::phenomenal::PhenomenalTrajectory;
use gibson::audio::human_music::semantic::{
    calm_loop, deflected_lift_trace, rise_unresolved, SemanticTrace,
};
use gibson::audio::human_music::song::{SongMap, SongMapConformance};
use gibson::audio::human_music::sonority::{ColorPolicy, EnsembleSonorityDiagnostics};
use gibson::audio::human_music::surgical::{ledger, residual, Perturbation};
use gibson::audio::human_music::synth::{HumanMusicSynth, ProductionControl, StemMask};
use gibson::audio::human_music::theory::note_name;
use gibson::audio::human_music::timeline::IntentTimeline;
use gibson::audio::human_music::voicing::{HarmonicStability, VoicingDiagnostics};
use gibson::audio::human_music::witness;
use gibson::audio::human_music::{demo_trace, MusicWorld, WorldId};
use gibson::audio::render::{OfflineRenderer, RenderResult};
use gibson::audio::wav::write_wav_i16;
use gibson::audio::SampleRate;

fn arg(flag: &str) -> Option<String> {
    std::env::args()
        .skip(1)
        .find_map(|a| a.strip_prefix(flag).map(|s| s.to_string()))
}

/// Select the semantic story fixture by name. The flagship default is the DeflectedLift *bounce*
/// (`deflected_lift_trace`), so the default listen finally matches the default grammar instead of
/// fighting the old single-arc `demo_trace` — pass `--story=cinematic` for that stress test.
fn story_trace(name: &str, beats: f64) -> SemanticTrace {
    match name {
        "cinematic" | "demo" | "arc" => demo_trace(beats),
        "calm" => calm_loop(beats),
        "rise" | "unresolved" => rise_unresolved(beats),
        _ => deflected_lift_trace(beats),
    }
}

/// Print the DeflectedLift harmonic spine grouped by the backbone's REAL cycles and slots (Round
/// VII chunked the tagged chords in fours, which misattributed chords to cycles once a slot held
/// one to four chords).
fn print_spine(plan: &gibson::audio::human_music::plan::CompositionPlan, chords: &[ChordSpan]) {
    let Some(bb) = plan.backbone.as_ref() else {
        return;
    };
    println!("harmonic spine (DeflectedLift cell — reach -> miss -> open -> reset), by cycle:");
    for cyc in 0..=bb.slots.iter().map(|s| s.cycle).max().unwrap_or(0) {
        let parts: Vec<String> = bb
            .slots
            .iter()
            .filter(|s| s.cycle == cyc)
            .map(|s| {
                let (a, b) = (s.start_beat(), s.end_beat());
                let names: Vec<String> = chords
                    .iter()
                    .filter(|c| c.start_beat >= a - 1e-6 && c.start_beat < b - 1e-6)
                    .map(|c| c.chord.label())
                    .collect();
                format!("{}=[{}]", s.gesture.label(), names.join(" "))
            })
            .collect();
        println!("  cycle {cyc}: {}", parts.join("  "));
    }
}

/// The performance options selected on the command line (the calibration A/B knobs).
fn perf_options() -> PerformanceOptions {
    PerformanceOptions {
        language: match arg("--language=").as_deref() {
            Some("simple") => MusicalLanguage::simple(),
            _ => MusicalLanguage::fusion_conversation(),
        },
        // The default is the R7b band (the listen preferred it to Round VIII's coupled bed);
        // `r8` renders the rejected negative control, `surgical` R7b with its real hard vertical
        // defects repaired note by note.
        coupling: match arg("--coupling=").as_deref() {
            Some("r8") | Some("coupled") => EnsembleCoupling::CoupledR8,
            Some("surgical") => EnsembleCoupling::Surgical,
            _ => EnsembleCoupling::Independent,
        },
        actions: arg("--actions=").as_deref() != Some("off"),
        responses: match arg("--responses=").as_deref() {
            Some("clockwork") => ResponseMode::Clockwork,
            _ => ResponseMode::Free,
        },
        calls: match arg("--calls=").as_deref() {
            Some("every") | Some("saturated") => CallPolicy::EveryStatement,
            _ => CallPolicy::Selective,
        },
        manifestations: match arg("--manifest=").as_deref() {
            Some("fixed") => ManifestationPolicy::Fixed,
            _ => ManifestationPolicy::Varied,
        },
    }
}

/// The debug production controls selected on the command line: `--harmonic-reference` (every
/// production factor removed, pitched buses only) or `--production=<toggles>`.
fn production() -> ProductionControl {
    if std::env::args().any(|a| a == "--harmonic-reference") {
        return ProductionControl::HARMONIC_REFERENCE;
    }
    match arg("--production=") {
        Some(spec) => ProductionControl::parse(&spec).unwrap_or_else(|e| {
            eprintln!(
                "--production: {e} (toggles: {:?})",
                ProductionControl::NAMES
            );
            std::process::exit(2)
        }),
        None => ProductionControl::NORMAL,
    }
}

/// The file-name suffix of a production control: empty for the normal mix, `.harmonic_reference`
/// for the neutral reference, else `.prod_<toggles>`.
fn production_suffix(p: ProductionControl) -> String {
    if p == ProductionControl::NORMAL {
        String::new()
    } else if p == ProductionControl::HARMONIC_REFERENCE {
        ".harmonic_reference".into()
    } else {
        format!(".prod_{}", p.label())
    }
}

/// A render with production factors removed is quieter (no saturation loudness, no bus comp):
/// scale it — one pure gain, pitch and timing untouched — to peak 0.8 so an A/B against the normal
/// mix compares harmony, not level. The normal mix is written as rendered.
fn normalize_for_comparison(out: &mut RenderResult, prod: ProductionControl) {
    if prod == ProductionControl::NORMAL || out.peak <= 1e-6 {
        return;
    }
    let g = 0.8 / out.peak;
    for x in out.audio.left.iter_mut().chain(out.audio.right.iter_mut()) {
        *x *= g;
    }
    // The receipts describe the file that is written, not the pre-normalization render.
    out.peak *= g;
    out.rms *= g;
}

/// The realized Score, one note per line (TSV): the exact pitches the diagnostics and the ear argue
/// about, with their role, function, provenance tag and the harmony they sound over.
fn write_notes_tsv(
    path: &std::path::Path,
    score: &gibson::audio::human_music::score::Score,
) -> std::io::Result<()> {
    use std::fmt::Write;
    let mut s = String::from(
        "start\tdur\tend\trole\tmidi\tname\tfunction\trole_note\txform\tchord\tactions\n",
    );
    let mut notes: Vec<_> = score.notes.iter().collect();
    notes.sort_by(|a, b| {
        a.start_beat
            .total_cmp(&b.start_beat)
            .then(a.role.label().cmp(b.role.label()))
            .then(a.pitch.cmp(&b.pitch))
    });
    let chord_at = |beat: f64| {
        score
            .chords
            .iter()
            .rfind(|c| c.start_beat <= beat + 1e-6)
            .map(|c| c.chord.label())
            .unwrap_or_default()
    };
    for n in notes {
        let chord = chord_at(n.start_beat);
        let actions: Vec<String> = n.prov.actions.iter().map(|a| a.to_string()).collect();
        let _ = writeln!(
            s,
            "{:.4}\t{:.4}\t{:.4}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            n.start_beat,
            n.dur_beats,
            n.start_beat + n.dur_beats as f64,
            n.role.label(),
            n.pitch,
            note_name(n.pitch),
            n.function.map(|f| f.label()).unwrap_or("NONE"),
            n.prov.role_note,
            n.prov.motif_xform.unwrap_or("-"),
            chord,
            actions.join(","),
        );
    }
    // The stings the vertical audit counts: one row per pitch, role `sfx`, their gated length.
    let bps = score.tempo_bpm.max(1.0) as f64 / 60.0;
    for e in score.sfx.iter().filter(|e| e.is_pitched()) {
        let (a, d, _, _) = e.kind.envelope();
        let dur = (a + d + e.kind.hold_secs()) as f64 * bps;
        for (k, &p) in e.pitches.iter().enumerate() {
            let _ = writeln!(
                s,
                "{:.4}\t{:.4}\t{:.4}\tsfx\t{}\t{}\t{}\t{:?}\t{}\t{}\t{}",
                e.start_beat,
                dur,
                e.start_beat + dur,
                p,
                note_name(p),
                e.function[k].map(|f| f.label()).unwrap_or("NONE"),
                e.kind,
                if e.owned_by.is_some() { "owned" } else { "-" },
                chord_at(e.start_beat),
                e.owned_by.map(|a| a.to_string()).unwrap_or_default(),
            );
        }
    }
    std::fs::write(path, s)
}

fn main() -> std::io::Result<()> {
    let seed: u64 = arg("--seed=").and_then(|s| s.parse().ok()).unwrap_or(2112);
    let beats: f64 = arg("--beats=")
        .and_then(|s| s.parse().ok())
        .unwrap_or(120.0);
    let which = arg("--world=").unwrap_or_else(|| "all".into());
    // The canonical audition now orbits the DeflectedLift bounce (a small cyclic harmonic identity
    // and a recurring hook). Pass --grammar=hookarc (or loop / riff) to A/B against the earlier
    // cinematic-arc song.
    let grammar = match arg("--grammar=").as_deref() {
        Some("hookarc") => CompositionGrammar::HookArc,
        Some("loop") | Some("loop_evolution") => CompositionGrammar::LoopEvolution,
        Some("riff") | Some("riff_drive") => CompositionGrammar::RiffDrive,
        Some("deflected") | Some("deflected_lift") | Some("bounce") => {
            CompositionGrammar::DeflectedLift
        }
        _ => CompositionGrammar::DeflectedLift,
    };
    // The semantic story fixture (default: the DeflectedLift bounce, so the flagship listen matches
    // the flagship grammar). Pass --story=cinematic to render the old single-arc demo as a stress test.
    let story = arg("--story=").unwrap_or_else(|| "bounce".into());
    let out_dir = arg("--out=")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("libgibson_human_music"));
    std::fs::create_dir_all(&out_dir)?;

    let sr = SampleRate::STUDIO;
    let block = 512;
    let prod = production();

    // Calibration mode: render the four grammar probes, each exercising a different definition of
    // musical direction, and dump their plan + coherence + discourse diagnostics.
    if std::env::args().any(|a| a == "--calibrate") {
        return calibrate(&out_dir, sr, block, seed);
    }
    // The four flagship A/Bs on ONE composition, so the ear can isolate backbone, language,
    // action and interaction timing instead of judging everything at once.
    if std::env::args().any(|a| a == "--ab") {
        return ab(
            &out_dir,
            sr,
            block,
            seed,
            &story_trace(&arg("--story=").unwrap_or_default(), beats),
        );
    }

    // Stem-isolation mode: render each bus of ONE world alone (plus the full mix) so a bad tone
    // can be pinned to a specific voice family — "the wrong pitch is in bass at bar N" instead of
    // "something sounds fucky". Rust-only debug surface; defaults to BLACK_ICE.
    // Pair stems (Round VIII): the six pitched-role pairs, to hear WHICH TWO players collide.
    let want_stems = std::env::args().any(|a| a == "--stems");
    let want_pairs = std::env::args().any(|a| a == "--pair-stems");
    if want_stems || want_pairs {
        let wid = match which.as_str() {
            "vapor95" => WorldId::Vapor95,
            "swiss_signal" => WorldId::SwissSignal,
            _ => WorldId::BlackIce,
        };
        return stems(
            &out_dir,
            sr,
            block,
            seed,
            wid,
            grammar,
            &story,
            (want_stems, want_pairs),
            prod,
        );
    }

    let trace = story_trace(&story, beats);
    if std::env::args().any(|a| a == "--phenomenal") {
        return phenomenal_ab(&out_dir, sr, block, &trace, seed, prod);
    }
    // Round X: the same story, seed, grammar and band, composed twice — the page is the variable.
    if std::env::args().any(|a| a == "--meaning") {
        return meaning_ab(&out_dir, sr, block, &trace, seed, grammar, prod);
    }
    // Round IX: the song is composed ONCE, before any room or idiom is known; every render below
    // is a performance of this one map.
    let song = SongMap::build(&trace, seed, Some(grammar));
    if std::env::args().any(|a| a == "--acceptance") {
        return acceptance(&out_dir, sr, block, &song, prod);
    }

    let worlds: Vec<WorldId> = match which.as_str() {
        "black_ice" => vec![WorldId::BlackIce],
        "vapor95" => vec![WorldId::Vapor95],
        "swiss_signal" => vec![WorldId::SwissSignal],
        _ => vec![WorldId::BlackIce, WorldId::Vapor95, WorldId::SwissSignal],
    };

    println!(
        "HumanMusic lab — {grammar:?} / story={story} — {} events, {beats:.0} beats",
        trace.events.len()
    );
    println!("seed={seed}  sr={} Hz  out={}", sr.get(), out_dir.display());
    print_song(&song);
    println!();

    for id in worlds {
        let world = MusicWorld::from_id(id);
        let file_stem = world.name.to_lowercase();
        let path = out_dir.join(format!("{file_stem}{}.wav", production_suffix(prod)));

        let comp = perform(&song, &world, perf_options());
        let conformance = SongMapConformance::check(&song, &comp.perf, &comp.score);
        let (score, plan, perf) = (comp.score, comp.song.plan, comp.perf);
        score.validate().expect("score invariants");
        if std::env::args().any(|a| a == "--dump-notes") {
            let tsv = out_dir.join(format!("{file_stem}.notes.tsv"));
            write_notes_tsv(&tsv, &score)?;
            println!("notes: {}", tsv.display());
        }

        let mut synth = HumanMusicSynth::with_production(&score, &world, sr, prod);
        // The harmonic reference is a HARMONIC control: every voice the vertical audit counts (the
        // four pitched buses and the pitched stings), no drums.
        if prod == ProductionControl::HARMONIC_REFERENCE {
            synth.set_stem_mask(StemMask::harmonic());
        }
        let frames = synth.total_samples();
        let t0 = std::time::Instant::now();
        let out = OfflineRenderer::new(sr, block).render(&mut synth, frames);
        let render_wall = t0.elapsed();

        let mut out = out;
        normalize_for_comparison(&mut out, prod);
        write_wav_i16(&path, &out.audio, sr)?;

        let real_secs = out.duration().as_secs_f64();
        let (ldc, rdc) = out.audio.dc_offset();
        println!(
            "=== {} =========================================",
            world.name
        );
        println!(
            "performance {:#018x}  score {:#018x}",
            perf.fingerprint(),
            score.fingerprint()
        );
        println!("{}", conformance.report());
        print!("{}", score.summary());
        print_spine(&plan, &score.chords);
        print!("{}", plan.dump());
        print!("{}", perf.actions.dump());
        print!("{}", perf.dump());
        print!("{}", CoherenceDiagnostics::measure(&plan, &score).report());
        print!("{}", DiscourseDiagnostics::measure(&plan, &score).report());
        println!(
            "{}",
            RealizationDiagnostics::measure(&plan, &score).report()
        );
        print!("{}", LeadOutlineDiagnostics::measure(&score).report());
        // Round VII: the verbs, the bar-to-bar rigidity, and the harmony as relations — against
        // the song's own timeline (the spine the actions were lifted from).
        print!(
            "{}",
            ActionDiagnostics::measure(&song.timeline, &plan, &perf, &score).report()
        );
        print!("{}", RigidityDiagnostics::measure(&score).report());
        print!("{}", HarmonyContextDiagnostics::measure(&perf).report());
        // Round VIIb: the exact causal receipts, the material relations, the shared budget, the
        // voicing path, and the stage's admission ledger.
        print!("{}", witness::audit(&perf, &score).report());
        for r in witness::interaction_receipts(&perf, &score) {
            println!(
                "  receipt {} {}->{} {:<8} call_ev={} answer_ev={} to_caller={:.2} to_other={:.2} margin={:+.2}{}",
                r.interaction,
                r.initiator.label(),
                r.responder.label(),
                r.transform.label(),
                r.call_events,
                r.answer_events,
                r.to_caller,
                r.to_other,
                r.margin(),
                if r.informative() { "" } else { " (uninformative)" }
            );
        }
        for o in &perf.opportunities {
            println!(
                "  opportunity {:?} {} @{:.2} open={:.2} space={:.2} head={:.2} redund={:.2} score={:+.2} -> {:?}",
                o.source,
                o.initiator.label(),
                o.start_beat,
                o.openness,
                o.space,
                o.headroom,
                o.redundancy,
                o.score,
                o.verdict
            );
        }
        for a in &perf.admissions {
            println!(
                "  admission {:?} {:?} @{:.2}: {:?}",
                a.action, a.kind, a.start_beat, a.outcome
            );
        }
        print!("{}", ComplexityReport::measure(&perf, &score).report());
        println!("{}", perf.regions.dump());
        // The debts, as settled IN THIS PERFORMANCE: each settlement bound to the action that
        // discharges it (the plan dump above prints the plan's unbound ledger).
        for o in &perf.obligations.obligations {
            println!(
                "  obligation {} {} opened@phrase{} due@{:?} -> {}",
                o.id,
                o.kind.label(),
                o.source_phrase,
                o.deadline,
                match o.settlement {
                    Some(st) => format!(
                        "{:?} by phrase{} witness={}",
                        st.how,
                        st.by_phrase,
                        st.witness
                            .map(|w| w.to_string())
                            .unwrap_or_else(|| "NONE".into())
                    ),
                    None => "unsettled".into(),
                }
            );
        }
        print!("{}", SfxAudit::measure(&perf, &score).report());
        print!(
            "{}",
            VoicingDiagnostics::measure(&score, &perf.contexts).report()
        );
        // Round VIIIb: how far the bed moves — and, off the R7b default, how much of it moved away
        // from the R7b realization of the SAME composition (the prior the ear preferred).
        let opts = perf_options();
        let r7b = (opts.coupling != EnsembleCoupling::Independent).then(|| {
            perform(
                &song,
                &world,
                PerformanceOptions {
                    coupling: EnsembleCoupling::Independent,
                    ..opts
                },
            )
            .score
        });
        print!(
            "{}",
            HarmonicStability::measure(&score, &perf.contexts, r7b.as_ref()).report()
        );
        if opts.coupling == EnsembleCoupling::Surgical {
            print!("{}", ledger(&score.vertical_repairs));
            if let Some(reference) = &r7b {
                print!("{}", Perturbation::measure(reference, &score).report());
            }
        }
        {
            // The hard vertical defects this realization still sounds (audible lifetimes at the
            // masking floor, the surgical pass's own detector) — for every arm, so the three compare.
            let policy = ColorPolicy::for_world(world.id, &perf.language);
            let left = residual(&score, &perf.contexts, &world, &policy);
            println!(
                "hard vertical defects sounding (audible, -20 dB; clash >= 0.125 b, floor, held flip, unresolved): {}",
                left.len()
            );
            for d in &left {
                println!("  {d}");
            }
        }
        // Round VIII: the UNION the band sounds — nominal durations, then the audible lifetimes on
        // this world's envelopes (a pad tail under the next chord, a stab already silent).
        let policy = ColorPolicy::for_world(world.id, &perf.language);
        let sonority = EnsembleSonorityDiagnostics::measure(&score, &perf.contexts, &policy, &[]);
        print!("{}", sonority.report());
        let audible = EnsembleSonorityDiagnostics::measure_audible(
            &score,
            &perf.contexts,
            &world,
            &policy,
            &[],
        );
        if let Some(r) = &score.support_report {
            print!("{}", r.report());
        }
        // --sonority-detail=unowned|missing|flip|bass|all: every problem slice of that class, spelled.
        // --sonority-detail=nonroot: every non-root bass note with its function and its reason.
        if arg("--sonority-detail=").as_deref() == Some("nonroot") {
            for (beat, pitch, chord, f, why) in &sonority.non_root_bass {
                println!(
                    "  non-root bass {beat:>7.2} {:4} over {chord:7} {f:9} {why}",
                    note_name(*pitch)
                );
            }
        }
        if let Some(which) = arg("--sonority-detail=") {
            use gibson::audio::human_music::sonority::{
                audible_voices, describe, slices, voices_of, Problem,
            };
            // --audible-floor=<dB>: slice the AUDIBLE lifetimes (the audible measure's own list).
            let voices = match arg("--audible-floor=").and_then(|f| f.parse::<f64>().ok()) {
                Some(floor) => audible_voices(&score, &perf.contexts, &world, floor),
                None => voices_of(&score, &perf.contexts),
            };
            for sl in slices(&voices, &perf.contexts, &policy, &[]) {
                let hit = sl.problems.iter().any(|p| match (which.as_str(), p) {
                    ("unowned", Problem::Unowned { .. }) => true,
                    ("missing", Problem::MissingCore { .. }) => sl.end - sl.start >= 0.25,
                    ("flip", Problem::IdentityFlip { .. }) => true,
                    ("bass", Problem::BassFunction { .. }) => true,
                    ("all", _) => true,
                    _ => false,
                });
                if hit {
                    print!("{}", describe(&sl, &voices, &perf.contexts));
                }
            }
        }
        println!(
            "vertical decisions (coupled realization, explained): {}",
            score.vertical_decisions.len()
        );
        for d in &score.vertical_decisions {
            println!(
                "  {:>7.2} {:5} {:11} {}",
                d.beat,
                d.role.label(),
                d.what,
                d.reason
            );
        }
        let masked = EnsembleSonorityDiagnostics::measure_audible_at(
            &score,
            &perf.contexts,
            &world,
            &policy,
            &[],
            20.0,
        );
        for (floor, a) in [(30, &audible), (20, &masked)] {
            println!(
                "ensemble sonority (AUDIBLE lifetimes, -{floor} dB): unowned m2={} m9={} ({:.2} beats) identity_flips={} bass_function={} crowded={} mean_pcs={:.2} owned[{}]",
                a.unowned_m2,
                a.unowned_m9,
                a.unowned_beats,
                a.identity_flips,
                a.bass_function_violations,
                a.crowded_slices,
                a.mean_distinct_pcs,
                a.owned
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
        println!(
            "lead: melody_repairs={} rejudged_at_release={}",
            score.melody_repairs, score.melody_rejudged
        );
        println!(
            "render: {real_secs:.1}s audio in {:.0}ms  ({:.1}x realtime)  peak={:.3} rms={:.3} dc=({:.4},{:.4})",
            render_wall.as_secs_f64() * 1000.0,
            real_secs / render_wall.as_secs_f64().max(1e-6),
            out.peak,
            out.rms,
            ldc,
            rdc,
        );
        println!(
            "safety: nonfinite={}  clipped_blocks={}  max_voices={}  worst_block={:.2}ms",
            out.had_nonfinite,
            out.reports.iter().filter(|r| r.clipped()).count(),
            out.max_active_voices,
            out.worst_block_time().as_secs_f64() * 1000.0,
        );
        println!("production: {}", prod.label());
        println!("wav: {}\n", path.display());
    }

    println!("Listen to the WAVs above. Same form + motif + resolutions, three dialects.");
    Ok(())
}

/// The song's receipt: its fingerprint and its thematic and harmonic maps' fingerprints.
fn print_song(song: &SongMap) {
    println!(
        "SongMap {:#018x}  ThematicMap {:#018x}  HarmonicMap {}",
        song.fingerprint(),
        song.thematic.fingerprint(),
        song.harmonic
            .map(|h| format!("{:#018x}", h.fingerprint()))
            .unwrap_or_else(|| "none (this grammar charts no harmony yet)".into())
    );
}

/// Round IX acceptance: ONE SongMap, four performances — BLACK_ICE, VAPOR95 and SWISS_SIGNAL in
/// the flagship idiom, and BLACK_ICE in the plain one — each rendered, fingerprinted and checked
/// for conformance. The question for the ear: different bands, the same composition?
fn acceptance(
    out_dir: &std::path::Path,
    sr: SampleRate,
    block: usize,
    song: &SongMap,
    prod: ProductionControl,
) -> std::io::Result<()> {
    let base = perf_options();
    let fusion = PerformanceOptions {
        language: MusicalLanguage::fusion_conversation(),
        ..base
    };
    let simple = PerformanceOptions {
        language: MusicalLanguage::simple(),
        ..base
    };
    println!("HumanMusic Round IX acceptance — one SongMap, four performances");
    print_song(song);
    let set = [
        (WorldId::BlackIce, fusion),
        (WorldId::Vapor95, fusion),
        (WorldId::SwissSignal, fusion),
        (WorldId::BlackIce, simple),
    ];
    for (id, opts) in set {
        let world = MusicWorld::from_id(id);
        let c = perform(song, &world, opts);
        let r = SongMapConformance::check(song, &c.perf, &c.score);
        let mut synth = HumanMusicSynth::with_production(&c.score, &world, sr, prod);
        let frames = synth.total_samples();
        let mut out = OfflineRenderer::new(sr, block).render(&mut synth, frames);
        normalize_for_comparison(&mut out, prod);
        let path = out_dir.join(format!(
            "acceptance_{}_{}{}.wav",
            world.name.to_lowercase(),
            opts.language.id.label(),
            production_suffix(prod)
        ));
        write_wav_i16(&path, &out.audio, sr)?;
        println!(
            "\n=== {} / {} ({:?} rhythm, {:?} coupling) ===",
            world.name,
            opts.language.id.label(),
            opts.language.harmonic_rhythm,
            opts.coupling
        );
        println!(
            "SongMap {:#018x} (performed {:#018x})  performance {:#018x}  score {:#018x}",
            c.song.fingerprint(),
            c.perf.song_fingerprint,
            c.perf.fingerprint(),
            c.score.fingerprint()
        );
        println!(
            "home {}{:?}  chords {}  statements {}  interactions {}  actions {}  notes {}  drums {}",
            note_name(c.perf.region.tonic_pc + 60).trim_end_matches(char::is_numeric),
            c.perf.region.mode,
            c.score.chords.len(),
            c.perf.statements.len(),
            c.perf.interactions.len(),
            c.perf.actions.actions.len(),
            c.score.notes.len(),
            c.score.drums.len()
        );
        print_spine(&c.song.plan, &c.score.chords);
        println!("{}", r.report());
        println!("wav: {}", path.display());
    }
    Ok(())
}

/// Print what the meaning-directed composer considered and why it chose what it chose.
fn print_composition(r: &CompositionReport) {
    println!("CompositionalPrior {:?}", r.prior);
    println!(
        "MeaningPlan (F): arc {:?}, resolution {:?}, {} events",
        r.target.arc,
        r.target.resolution,
        r.target.events.len()
    );
    println!("chart filters (survivors): {:?}", r.chart_stages);
    if let Some(i) = r.chart {
        println!("chart chosen: {:?}", r.charts[i].cell);
    }
    println!("theme filters (survivors): {:?}", r.theme_stages);
    let t = &r.themes[r.theme];
    println!(
        "thesis {:?} rhythm {:?} ({:?}, fit {:.2})",
        t.thesis.degrees, t.thesis.rhythm, t.params, t.fit
    );
    println!("answer {:?}", t.answer.degrees);
    println!("thesis profile {:?}", t.profile);
    let mut near: Vec<_> = r
        .themes
        .iter()
        .filter(|c| c.dropped_by.is_some() && c.profile.reach == r.target.arc)
        .collect();
    near.sort_by(|a, b| b.fit.total_cmp(&a.fit));
    for c in near.iter().take(4) {
        println!(
            "  runner-up {:?} fit {:.2} dropped by {:?} unusual {:?}",
            c.thesis.degrees,
            c.fit,
            c.dropped_by.unwrap_or(""),
            c.profile.unusual(&r.prior)
        );
    }
}

/// A song's content, readable: its theme sites and its lead sheet (the chart as charted, in C).
fn print_page(song: &SongMap) {
    println!(
        "SongMap {:#018x} ({:?})  thesis {:?} {:?}",
        song.fingerprint(),
        song.composer(),
        song.thematic.bank.identity.degrees,
        song.thematic.bank.identity.rhythm
    );
    for s in &song.thematic.sites {
        println!(
            "  p{} {:<9} {:<12} {:?}",
            s.phrase,
            format!("{:?}", s.role),
            format!("{:?}", s.handoff),
            s.motif.degrees
        );
    }
    if let (Some(bb), Some(hm)) = (&song.plan.backbone, &song.harmonic) {
        let end = song.plan.form.total_beats;
        let sheet: Vec<String> = lead_sheet(bb, hm, song.frame)
            .iter()
            .filter(|s| s.start_beat < end - 1e-9)
            .map(|s| format!("{:.0}:{}", s.start_beat, s.chord.label()))
            .collect();
        println!("  lead sheet (C): {}", sheet.join(" "));
    }
}

/// Round X acceptance A/B: ONE story, ONE seed, ONE grammar, ONE band — two composers. A is the
/// Round IX structural song (the control, `SongMap::build`); B is the meaning-directed song
/// (`Composer::MeaningDirected`). Same form, same performance options, coupling and production:
/// only the page differs. BLACK_ICE decides; VAPOR95 checks it is the SONG that changed, not a
/// BLACK_ICE accident. The questions for the ear: which song do you want to hear again, and can
/// you tell what B is doing sooner than A?
fn meaning_ab(
    out_dir: &std::path::Path,
    sr: SampleRate,
    block: usize,
    trace: &SemanticTrace,
    seed: u64,
    grammar: CompositionGrammar,
    prod: ProductionControl,
) -> std::io::Result<()> {
    let a = SongMap::compose(trace, seed, Some(grammar), Composer::StructuralR9);
    let (b, report) = compose_meaning(a.clone(), &CompositionalPrior::HOOKY_FUSION);
    let opts = perf_options();
    println!(
        "HumanMusic Round X acceptance — one story, two composers (seed {seed}, {:?}, {:?} coupling)\n",
        opts.language.id, opts.coupling
    );
    print_composition(&report);
    for (label, song) in [("A structural", &a), ("B meaning", &b)] {
        println!("\n=== {label} ===");
        print_page(song);
        println!("{}", Commutation::check(song).report());
    }
    for id in [WorldId::BlackIce, WorldId::Vapor95] {
        let world = MusicWorld::from_id(id);
        for (label, song) in [("structural", &a), ("meaning", &b)] {
            let c = perform(song, &world, opts);
            let r = SongMapConformance::check(song, &c.perf, &c.score);
            let d = RealizationDiagnostics::measure(&song.plan, &c.score);
            let mut synth = HumanMusicSynth::with_production(&c.score, &world, sr, prod);
            let frames = synth.total_samples();
            let mut out = OfflineRenderer::new(sr, block).render(&mut synth, frames);
            normalize_for_comparison(&mut out, prod);
            let path = out_dir.join(format!(
                "{}_{label}{}.wav",
                world.name.to_lowercase(),
                production_suffix(prod)
            ));
            write_wav_i16(&path, &out.audio, sr)?;
            println!(
                "{} / {label}: SongMap {:#018x}  performance {:#018x}  score {:#018x}  conformance {}  \
                 repairs {} rejudged {}  unjustified {:?}  lead notes {}  beats {}  peak {:.3} rms {:.4}",
                world.name,
                song.fingerprint(),
                c.perf.fingerprint(),
                c.score.fingerprint(),
                if r.passes() { "PASS" } else { "FAIL" },
                c.score.melody_repairs,
                c.score.melody_rejudged,
                d.unjustified_by_role,
                c.score
                    .notes
                    .iter()
                    .filter(|n| n.role == gibson::audio::human_music::score::Role::Lead)
                    .count(),
                c.score.total_beats,
                out.peak,
                out.rms
            );
            if !r.passes() {
                println!("{}", r.report());
            }
            println!("wav: {}", path.display());
        }
    }
    Ok(())
}

/// Render the four grammar calibration probes. Each is a deterministic composition constructed to
/// exercise a *different* definition of musical direction:
///   - hook_arc: a directed arc with functional cadences (the resolved demo).
///   - loop_evolution: a calm loop — meaning comes from evolution, not cadences (cyclic harmony).
///   - riff_drive: a rising, unresolved trace over a modal pedal (harmonic stasis is allowed).
///   - world_switch: two locally coherent regimes (two sonic dialects) of the SAME composition
///     plan, joined — the transported identity is the form + discourse skeleton and the seed.
fn calibrate(
    out_dir: &std::path::Path,
    sr: SampleRate,
    block: usize,
    seed: u64,
) -> std::io::Result<()> {
    println!(
        "HumanMusic calibration — four grammar probes\nseed={seed}  out={}\n",
        out_dir.display()
    );

    let cases = [
        (
            "hook_arc",
            CompositionGrammar::HookArc,
            demo_trace(120.0),
            WorldId::BlackIce,
        ),
        (
            "loop_evolution",
            CompositionGrammar::LoopEvolution,
            calm_loop(120.0),
            WorldId::Vapor95,
        ),
        (
            "riff_drive",
            CompositionGrammar::RiffDrive,
            rise_unresolved(120.0),
            WorldId::SwissSignal,
        ),
    ];
    for (name, grammar, trace, wid) in cases {
        let world = MusicWorld::from_id(wid);
        let (score, plan) = compose_with_grammar(&trace, &world, seed, grammar);
        score.validate().expect("calibration score invariants");
        let mut synth = HumanMusicSynth::new(&score, &world, sr);
        let frames = synth.total_samples();
        let out = OfflineRenderer::new(sr, block).render(&mut synth, frames);
        let path = out_dir.join(format!("calib_{name}.wav"));
        write_wav_i16(&path, &out.audio, sr)?;

        println!("=== {name}  ({grammar:?}, {}) ===", world.name);
        print!("{}", plan.dump());
        print!("{}", CoherenceDiagnostics::measure(&plan, &score).report());
        print!("{}", DiscourseDiagnostics::measure(&plan, &score).report());
        println!(
            "{}",
            RealizationDiagnostics::measure(&plan, &score).report()
        );
        print!("{}", LeadOutlineDiagnostics::measure(&score).report());
        println!(
            "safety: nonfinite={}  peak={:.3}  max_voices={}\nwav: {}\n",
            out.audio.has_nonfinite(),
            out.peak,
            out.max_active_voices,
            path.display(),
        );
    }

    // world_switch: the same composition plan realized in TWO sonic dialects, joined. The
    // transported identity across the switch is the form + discourse skeleton (world-independent)
    // and the seed; regime A is BLACK_ICE, regime B is VAPOR95.
    let trace = demo_trace(120.0);
    let (a_world, b_world) = (MusicWorld::black_ice(), MusicWorld::vapor95());
    let (a_score, plan) =
        compose_with_grammar(&trace, &a_world, seed, CompositionGrammar::WorldSwitch);
    let (b_score, _) =
        compose_with_grammar(&trace, &b_world, seed, CompositionGrammar::WorldSwitch);
    a_score.validate().expect("world-switch A invariants");
    b_score.validate().expect("world-switch B invariants");

    let mut sa = HumanMusicSynth::new(&a_score, &a_world, sr);
    let fa = sa.total_samples();
    let ra = OfflineRenderer::new(sr, block).render(&mut sa, fa);
    let mut sb = HumanMusicSynth::new(&b_score, &b_world, sr);
    let fb = sb.total_samples();
    let rb = OfflineRenderer::new(sr, block).render(&mut sb, fb);

    let mut combined = StereoBlock::new(0);
    ra.audio.append_to(&mut combined);
    rb.audio.append_to(&mut combined);
    let path = out_dir.join("calib_world_switch.wav");
    write_wav_i16(&path, &combined, sr)?;

    println!("=== world_switch  (BLACK_ICE → VAPOR95, transported plan) ===");
    print!("{}", plan.dump());
    println!(
        "transport: same discourse plan + seed, two dialects joined at the switch\nnonfinite={}  wav: {}\n",
        combined.has_nonfinite(),
        path.display(),
    );

    println!("Four probes rendered. Each exercises a different definition of musical direction.");
    Ok(())
}

/// Stem isolation: render one world's six buses (pad/keys/bass/lead/drums/sfx) each in isolation,
/// plus the full mix, to `<world>.stem_<bus>.wav`; and/or (Round VIII) the six pitched-role PAIRS
/// to `<world>.pair_<a>+<b>.wav` — a vertical collision lives between two players, which a solo
/// stem cannot hear. Each render uses a fresh synth (clean DSP state) with the corresponding
/// [`StemMask`] and the selected [`ProductionControl`] (suffix in the file name when not normal).
#[allow(clippy::too_many_arguments)]
fn stems(
    out_dir: &std::path::Path,
    sr: SampleRate,
    block: usize,
    seed: u64,
    wid: WorldId,
    grammar: CompositionGrammar,
    story: &str,
    (solo, pairs): (bool, bool),
    prod: ProductionControl,
) -> std::io::Result<()> {
    let world = MusicWorld::from_id(wid);
    let file_stem = world.name.to_lowercase();
    let trace = story_trace(story, 120.0);
    let score = compose_full(&trace, &world, seed, Some(grammar), perf_options()).score;
    score.validate().expect("score invariants");

    println!(
        "HumanMusic stems — {} @ seed {seed}  ({} sfx, {} notes)\nout={}\n",
        world.name,
        score.sfx.len(),
        score.notes.len(),
        out_dir.display()
    );

    let mut masks: Vec<(String, StemMask)> = Vec::new();
    if solo {
        masks.extend(
            StemMask::NAMES
                .iter()
                .map(|n| (format!("stem_{n}"), StemMask::solo(n))),
        );
        // Under the harmonic reference "full" is the reference mix itself (no drums).
        let full = if prod == ProductionControl::HARMONIC_REFERENCE {
            StemMask::harmonic()
        } else {
            StemMask::full()
        };
        masks.push(("stem_full".to_string(), full));
    }
    if pairs {
        masks.extend(
            StemMask::PITCHED_PAIRS
                .iter()
                .map(|(a, b)| (format!("pair_{a}+{b}"), StemMask::only(&[a, b]))),
        );
    }
    let suffix = production_suffix(prod);

    for (name, mask) in masks {
        let mut synth = HumanMusicSynth::with_production(&score, &world, sr, prod);
        synth.set_stem_mask(mask);
        let frames = synth.total_samples();
        let mut out = OfflineRenderer::new(sr, block).render(&mut synth, frames);
        normalize_for_comparison(&mut out, prod);
        let path = out_dir.join(format!("{file_stem}.{name}{suffix}.wav"));
        write_wav_i16(&path, &out.audio, sr)?;
        println!(
            "  {name:16}  peak={:.3} rms={:.3} nonfinite={}  {}",
            out.peak,
            out.rms,
            out.audio.has_nonfinite(),
            path.display()
        );
    }

    println!(
        "\nStems rendered. Solo each bus to localize a bad tone; `full` is the reference mix."
    );
    Ok(())
}

/// The flagship A/Bs of ONE composition (BLACK_ICE, DeflectedLift): the conversational-fusion
/// language, the simple language, the same composition with the action plan disabled (mood without
/// action), clockwork fixed-slot responses, every statement forced to call (saturation), and one
/// fixed gesture choreography every cycle (rigidity). Writes `ab_<case>.wav`.
fn ab(
    out_dir: &std::path::Path,
    sr: SampleRate,
    block: usize,
    seed: u64,
    trace: &SemanticTrace,
) -> std::io::Result<()> {
    let world = MusicWorld::black_ice();
    let fusion = PerformanceOptions::default();
    let cases = [
        ("fusion", fusion),
        (
            "simple",
            PerformanceOptions {
                language: MusicalLanguage::simple(),
                ..fusion
            },
        ),
        (
            "actions_off",
            PerformanceOptions {
                actions: false,
                ..fusion
            },
        ),
        (
            "clockwork",
            PerformanceOptions {
                responses: ResponseMode::Clockwork,
                ..fusion
            },
        ),
        (
            "saturated",
            PerformanceOptions {
                calls: CallPolicy::EveryStatement,
                ..fusion
            },
        ),
        (
            "fixed_gestures",
            PerformanceOptions {
                manifestations: ManifestationPolicy::Fixed,
                ..fusion
            },
        ),
    ];
    println!(
        "HumanMusic A/B — {} DeflectedLift, seed={seed}\n",
        world.name
    );
    for (name, opts) in cases {
        let c = compose_full(
            trace,
            &world,
            seed,
            Some(CompositionGrammar::DeflectedLift),
            opts,
        );
        c.score.validate().expect("A/B score invariants");
        let mut synth = HumanMusicSynth::new(&c.score, &world, sr);
        let frames = synth.total_samples();
        let out = OfflineRenderer::new(sr, block).render(&mut synth, frames);
        let path = out_dir.join(format!("ab_{name}.wav"));
        write_wav_i16(&path, &out.audio, sr)?;
        println!(
            "  {name:12} actions={:<3} interactions={:<3} notes={:<4} drums={:<4} peak={:.3} rms={:.3} nonfinite={}  {}",
            c.perf.actions.actions.len(),
            c.perf.interactions.len(),
            c.score.notes.len(),
            c.score.drums.len(),
            out.peak,
            out.rms,
            out.audio.has_nonfinite(),
            path.display()
        );
        // The three R7 instruments on every case, so the A/B numbers sit next to the A/B ears.
        let a = ActionDiagnostics::measure(
            &IntentTimeline::walk(trace),
            &c.song.plan,
            &c.perf,
            &c.score,
        );
        let r = RigidityDiagnostics::measure(&c.score);
        let h = HarmonyContextDiagnostics::measure(&c.perf);
        let budget = ComplexityReport::measure(&c.perf, &c.score);
        let rec: Vec<String> = a
            .manifestation_recurrence
            .iter()
            .map(|(g, x)| format!("{g}={x:.2}"))
            .collect();
        println!(
            "  {:12} causal={}/{} calls={}/{} answered={:.2} figure_calls={} drum_calls={} receipts={}/{} min_margin={:+.2} stasis={:.1}b undeclared_idle={:.1}b budget_violations={} manifest[{}] repairs={} rejudged={}",
            "",
            a.causal_witnessed,
            a.causal_total,
            a.statement_calls,
            a.statements,
            a.answered_statement_rate,
            a.figure_calls,
            a.drum_calls,
            a.caller_related_receipts,
            a.informative_receipts,
            a.min_caller_margin,
            a.declared_stasis_beats,
            a.longest_undeclared_idle_beats,
            budget.violations.len(),
            rec.join(" "),
            c.score.melody_repairs,
            c.score.melody_rejudged,
        );
        println!(
            "  {:12} unwitnessed={} witness_cov={:.2} longest_idle={:.1}b responders={} placement_entropy={:.2}b same_slot={:.2} | recurrence keys={:.2} bass={:.2} drums={:.2} | deflects={} prepared={} global_scale_share={:.2}",
            "",
            a.unwitnessed_morphisms,
            a.semantic_witness_coverage,
            a.longest_actionless_span_beats,
            a.responders.len(),
            a.response_placement_entropy,
            a.same_slot_response_recurrence,
            r.keys_onset_recurrence,
            r.bass_onset_recurrence,
            r.drums_onset_recurrence,
            h.deflects,
            h.deflects_prepared,
            h.global_scale_only_share,
        );
    }
    println!(
        "\nSame song four ways: language, action and interaction timing isolated for the ear."
    );
    ab_coupling(out_dir, sr, block, seed, trace)
}

/// The coupling A/B/C: the SAME composition realized by the R7b band (each player projecting the
/// shared material alone — the default the listen preferred), by Round VIII's coupled bed (the
/// rejected negative control) and by the surgical pass (R7b with only its real hard vertical defects
/// repaired), in every world, with the vertical numbers beside each file.
fn ab_coupling(
    out_dir: &std::path::Path,
    sr: SampleRate,
    block: usize,
    seed: u64,
    trace: &SemanticTrace,
) -> std::io::Result<()> {
    println!(
        "\nHumanMusic A/B — R7b (independent) vs R8 (coupled, rejected) vs surgical realization, seed={seed}\n"
    );
    for world in MusicWorld::all() {
        for (name, coupling) in [
            ("r7b", EnsembleCoupling::Independent),
            ("r8", EnsembleCoupling::CoupledR8),
            ("surgical", EnsembleCoupling::Surgical),
        ] {
            let c = compose_full(
                trace,
                &world,
                seed,
                Some(CompositionGrammar::DeflectedLift),
                PerformanceOptions {
                    coupling,
                    ..PerformanceOptions::default()
                },
            );
            c.score.validate().expect("A/B score invariants");
            let mut synth = HumanMusicSynth::new(&c.score, &world, sr);
            let frames = synth.total_samples();
            let out = OfflineRenderer::new(sr, block).render(&mut synth, frames);
            let path = out_dir.join(format!(
                "ab_coupling_{}_{name}.wav",
                world.name.to_lowercase()
            ));
            write_wav_i16(&path, &out.audio, sr)?;
            let policy = ColorPolicy::for_world(world.id, &c.perf.language);
            let d = EnsembleSonorityDiagnostics::measure(&c.score, &c.perf.contexts, &policy, &[]);
            let a = EnsembleSonorityDiagnostics::measure_audible_at(
                &c.score,
                &c.perf.contexts,
                &world,
                &policy,
                &[],
                gibson::audio::human_music::sonority::MASKING_FLOOR_DB,
            );
            let w = witness::audit(&c.perf, &c.score);
            println!(
                "  {:13} {:12} notes={:<4} unowned m2/m9 {}/{} ({:.2} b)  audible -20 dB {}/{} ({:.2} b)  bass_function={} missing_core={} ({:.2} b)  witnessed {}/{} repairs={} peak={:.3} rms={:.3}  {}",
                world.name,
                name,
                c.score.notes.len(),
                d.unowned_m2,
                d.unowned_m9,
                d.unowned_beats,
                a.unowned_m2,
                a.unowned_m9,
                a.unowned_beats,
                d.bass_function_violations,
                d.missing_core_slices,
                d.missing_core_beats,
                w.witnessed(),
                w.total(),
                c.score.melody_repairs,
                out.peak,
                out.rms,
                path.display()
            );
        }
    }
    println!("\nSame composition, same synth: only the realization's coupling differs.");
    Ok(())
}

/// Round XI: same trace/seed/options/production, two song targets. Confirm the compositional
/// distinction before the first render; take each immutable score through normal and reference
/// production. SWISS_SIGNAL precedes BLACK_ICE so the first listen has no dark-world confound.
fn phenomenal_ab(
    out_dir: &std::path::Path,
    sr: SampleRate,
    block: usize,
    trace: &SemanticTrace,
    seed: u64,
    prod: ProductionControl,
) -> std::io::Result<()> {
    let a = SongMap::compose(
        trace,
        seed,
        Some(CompositionGrammar::DeflectedLift),
        Composer::MeaningDirected,
    );
    let b = SongMap::compose(trace, seed, None, Composer::StablePropulsion);
    let opts = perf_options();
    println!("Round XI: seed={seed} beats={} options={opts:?} production={prod:?} sr={sr:?} block={block}", trace.total_beats);
    if format!("{:?}", a.plan.arrangement) != format!("{:?}", b.plan.arrangement) {
        return Err(std::io::Error::other("A/B orchestration envelopes differ"));
    }
    println!("orchestration envelope: identical");
    let legacy = Commutation::check(&a);
    println!("Round X control: {}", legacy.report());
    // Keep inherited form failures visible on short/custom stories; the flagship must commute.
    if !legacy.commutes() {
        return Err(std::io::Error::other(
            "Round X control does not commute on this input",
        ));
    }
    for (label, song) in [("tension_deflection", &a), ("stable_propulsion", &b)] {
        println!("\n=== {label} ===");
        print_page(song);
        let trajectory = PhenomenalTrajectory::observe(song);
        let report = trajectory.report();
        println!("{report}");
        std::fs::write(out_dir.join(format!("{label}.trajectory.txt")), report)?;
        if let Some(target) = &song.phenomenal {
            let failures = trajectory.divergences(target);
            println!("F target {target:?}\nphenomenal commutation: {failures:?}");
            if !failures.is_empty() {
                return Err(std::io::Error::other(format!(
                    "phenomenal target failed: {failures:?}"
                )));
            }
        }
    }
    for id in [WorldId::SwissSignal, WorldId::BlackIce] {
        let world = MusicWorld::from_id(id);
        let dir = out_dir.join(world.name.to_lowercase());
        std::fs::create_dir_all(&dir)?;
        println!("\nworld={world:?}");
        for (label, song) in [("tension_deflection", &a), ("stable_propulsion", &b)] {
            let c = perform(song, &world, opts);
            let law = SongMapConformance::check(song, &c.perf, &c.score);
            println!("{label}: {}", law.report());
            if !law.passes() {
                return Err(std::io::Error::other(law.report()));
            }
            let d = RealizationDiagnostics::measure(&song.plan, &c.score);
            println!("{label}: song={:#018x} performance={:#018x} score={:#018x} repairs={} rejudged={} unjustified={:?}",
                song.fingerprint(), c.perf.fingerprint(), c.score.fingerprint(),
                c.score.melody_repairs, c.score.melody_rejudged, d.unjustified_by_role);
            write_notes_tsv(&dir.join(format!("{label}.notes.tsv")), &c.score)?;
            for (suffix, production) in [
                ("", prod),
                (".harmonic_reference", ProductionControl::HARMONIC_REFERENCE),
            ] {
                let mut synth = HumanMusicSynth::with_production(&c.score, &world, sr, production);
                if production == ProductionControl::HARMONIC_REFERENCE {
                    synth.set_stem_mask(StemMask::harmonic());
                }
                let frames = synth.total_samples();
                let mut out = OfflineRenderer::new(sr, block).render(&mut synth, frames);
                let gain = if production == ProductionControl::NORMAL || out.peak <= 1e-6 {
                    1.0
                } else {
                    0.8 / out.peak
                };
                normalize_for_comparison(&mut out, production);
                if out.had_nonfinite {
                    return Err(std::io::Error::other("nonfinite render"));
                }
                let path = dir.join(format!("{label}{suffix}.wav"));
                write_wav_i16(&path, &out.audio, sr)?;
                println!(
                    "wav={} frames={frames} peak={:.6} rms={:.6} gain={gain:.6} nonfinite={}",
                    path.display(),
                    out.peak,
                    out.rms,
                    out.had_nonfinite
                );
            }
        }
    }
    Ok(())
}
