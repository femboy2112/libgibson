//! Round XVII lab: frozen controls, pocket factors, source-ledger perturbations and stems.
use gibson::audio::{
    human_music::{
        composer::Composer,
        expression::{ExpressionDiagnostics, ExpressionStrategy},
        functor::{
            perform_coherent, perform_phrased, perform_pocket_experiment, perform_pocketed,
            Composition,
        },
        identity::IdentityDiagnostics,
        mass::MassDiagnostics,
        performance::PerformanceOptions,
        phrase_diagnostics::PhraseSurfaceDiagnostics,
        pocket::PocketOptions,
        pocket_diagnostics::PocketDiagnostics,
        score::{Note, Role},
        semantic::deflected_lift_trace,
        song::{SongMap, SongMapConformance},
        synth::StemMask,
        temporal::TemporalPitchDiagnostics,
        tension::TensionDiagnostics,
        voicing_diagnostics::VoicingSurfaceDiagnostics,
        witness, HumanMusicSynth, MusicWorld,
    },
    render::OfflineRenderer,
    wav::write_wav_i16,
    SampleRate,
};
use std::{fmt::Write, path::Path};

fn render(
    dir: &Path,
    name: &str,
    c: &Composition,
    w: &MusicWorld,
    mask: StemMask,
) -> std::io::Result<String> {
    let sr = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::new(&c.score, w, sr);
    synth.set_stem_mask(mask);
    let frames = synth.total_samples();
    let audio = OfflineRenderer::new(sr, 256).render(&mut synth, frames);
    assert!(!audio.had_nonfinite);
    let path = dir.join(format!("{name}.wav"));
    write_wav_i16(&path, &audio.audio, sr)?;
    Ok(format!(
        "{} frames={} peak={} rms={}\n",
        path.display(),
        audio.audio.frames(),
        audio.peak,
        audio.rms
    ))
}
// Recover the source event identity from the source ledger; retiming never becomes an addition.
fn source_slot(c: &Composition, n: &Note) -> (f64, i32) {
    c.score
        .expression_decisions
        .iter()
        .find_map(|d| {
            let a = d.after?;
            ((a.start_beat - n.start_beat).abs() < 1e-6
                && ((a.role == n.role && a.pitch == n.pitch)
                    || (n.prov.role_note == "unison"
                        && a.role == Role::Lead
                        && (a.pitch - n.pitch).rem_euclid(12) == 0)))
                .then_some((
                    d.before.note.start_beat,
                    n.pitch + d.before.note.pitch - a.pitch,
                ))
        })
        .unwrap_or((n.start_beat, n.pitch))
}
fn perturbation(a: &Composition, b: &Composition) -> String {
    let mut out=String::from("Source-ledger identity diff; categories overlap. Octave movement is a pitch change, not new harmony.\n");
    for role in Role::ALL {
        let old: Vec<_> = a.score.role_notes(role).collect();
        let new: Vec<_> = b.score.role_notes(role).collect();
        let mut used = vec![false; new.len()];
        let (mut retimed, mut duration, mut velocity, mut pitch, mut omitted) = (0, 0, 0, 0, 0);
        let mut rows = String::new();
        for n in &old {
            let (at, pc) = source_slot(a, n);
            let found = new
                .iter()
                .enumerate()
                .filter(|(i, m)| {
                    let (bt, q) = source_slot(b, m);
                    !used[*i]
                        && (at - bt).abs() < 1e-6
                        && n.prov == m.prov
                        && (pc == q || role == Role::Pad && (pc - q).rem_euclid(12) == 0)
                })
                .min_by_key(|(_, m)| (n.pitch - m.pitch).abs());
            if let Some((i, m)) = found {
                used[i] = true;
                retimed += usize::from(n.start_beat != m.start_beat);
                duration += usize::from(n.dur_beats != m.dur_beats);
                velocity += usize::from(n.velocity != m.velocity);
                pitch += usize::from(n.pitch != m.pitch);
                if format!("{n:?}") != format!("{m:?}") {
                    writeln!(rows, "before={n:?}\nafter={m:?}").unwrap();
                }
            } else {
                omitted += 1;
                writeln!(rows, "omitted={n:?}").unwrap();
            }
        }
        let added = used.iter().filter(|u| !**u).count();
        writeln!(out,"{} old={} new={} retimed={retimed} duration={duration} velocity={velocity} pitch={pitch} omitted={omitted} added={added}",role.label(),old.len(),new.len()).unwrap();
        out.push_str(&rows);
        for (i, n) in new.iter().enumerate().filter(|(i, _)| !used[*i]) {
            let _ = i;
            writeln!(out, "added={n:?}").unwrap();
        }
    }
    writeln!(
        out,
        "drums old={} new={} exact_equal={}",
        a.score.drums.len(),
        b.score.drums.len(),
        format!("{:?}", a.score.drums) == format!("{:?}", b.score.drums)
    )
    .unwrap();
    out
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .iter()
        .find_map(|s| s.strip_prefix("--out="))
        .unwrap_or("target/humanmusic-r17/final");
    let dir = Path::new(out);
    std::fs::create_dir_all(dir)?;
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let mut summary = format!(
        "StablePropulsion seed=2112 FusionConversation default options song={:016x}\n",
        song.fingerprint()
    );
    summary.push_str("Boundary: C4 knockout accepted by maintainer; support source selection is NO-GO under the current ruler; SWISS r17 is the unchanged support-only control. Factor bits are support/mono/legato/lattice, high to low; stable_precursors is a separate A/B. Zero lattice violations is one witness, not timing acceptance.\n");
    let mut renders = String::new();
    for (name, w) in [
        ("black_ice", MusicWorld::black_ice()),
        ("swiss", MusicWorld::swiss_signal()),
    ] {
        let r14 = perform_coherent(&song, &w, PerformanceOptions::default());
        let r16 = perform_phrased(&song, &w, PerformanceOptions::default());
        let r17 = if name == "swiss" {
            // C4 localized by maintainer knockout audition; source selection remains separately gated.
            perform_pocket_experiment(
                &song,
                &w,
                PerformanceOptions::default(),
                PocketOptions {
                    support_top_voice: true,
                    ..PocketOptions::NONE
                },
            )
        } else {
            perform_pocketed(&song, &w, PerformanceOptions::default())
        };
        let mut arms = vec![
            ("r14".to_string(), r14),
            ("r16".to_string(), r16),
            ("r17".to_string(), r17),
        ];
        for bits in 0..16 {
            let factors = PocketOptions {
                lattice_positions: bits & 1 != 0,
                legato_connectives: bits & 2 != 0,
                mono_voice: bits & 4 != 0,
                support_top_voice: bits & 8 != 0,
                stable_precursors: false,
            };
            arms.push((
                format!("f{bits:04b}"),
                perform_pocket_experiment(&song, &w, PerformanceOptions::default(), factors),
            ));
        }
        let stable = PocketOptions {
            stable_precursors: true,
            ..PocketOptions::default()
        };
        arms.push((
            "stable_precursors".into(),
            perform_pocket_experiment(&song, &w, PerformanceOptions::default(), stable),
        ));
        for (arm, c) in &arms {
            if arm != "r14" {
                std::fs::write(
                    dir.join(format!("{name}.r14-{arm}.perturbation.txt")),
                    perturbation(&arms[0].1, c),
                )?;
            }
            if arm != "r16" {
                std::fs::write(
                    dir.join(format!("{name}.r16-{arm}.perturbation.txt")),
                    perturbation(&arms[1].1, c),
                )?;
            }
            let prefix = format!("{name}_{arm}");
            let id = IdentityDiagnostics::measure_score(&c.score, &c.perf.contexts, &w);
            let temporal = TemporalPitchDiagnostics::measure(&c.perf, &c.score);
            let actions = witness::audit(&c.perf, &c.score);
            writeln!(
                summary,
                "{prefix} score={:016x} notes={} flips={} stale={} temporal_false={} actions={}/{} mono_voice={} retained_unresolved={}",
                c.score.fingerprint(),
                c.score.notes.len(),
                id.flips().count(),
                c.score.stale_hearings().len(),
                temporal.false_function_claims,
                actions.rows.iter().filter(|a| a.witnessed).count(),
                actions.rows.len(),
                c.score.mono_voice,
                c.score.expression_decisions.iter().filter(|d| d.strategy == ExpressionStrategy::RetainedObligation).count()
            )
            .unwrap();
            for (suffix, data) in [
                (
                    "pocket",
                    PocketDiagnostics::measure(&c.perf, &c.score, &w).report(),
                ),
                (
                    "phrase",
                    PhraseSurfaceDiagnostics::measure(&c.song.plan, &c.perf, &c.score, &w).report(),
                ),
                (
                    "voicing",
                    VoicingSurfaceDiagnostics::measure(&c.perf, &c.score, &w).report(),
                ),
                (
                    "notes",
                    c.score
                        .notes
                        .iter()
                        .map(|n| format!("{n:?}\n"))
                        .collect::<String>(),
                ),
                (
                    "drums",
                    c.score.drums.iter().map(|d| format!("{d:?}\n")).collect(),
                ),
                ("phrase_plans", format!("{:#?}", c.score.phrase_plans)),
                (
                    "continuity_links",
                    format!("{:#?}", c.score.voice_continuity),
                ),
                (
                    "expression_decisions",
                    format!("{:#?}", c.score.expression_decisions),
                ),
                ("occupancy", format!("{:#?}", c.score.occupancy)),
                (
                    "voicing_decisions",
                    c.score
                        .support_voicing_decisions
                        .iter()
                        .map(|d| d.report())
                        .collect(),
                ),
                (
                    "expression",
                    ExpressionDiagnostics::measure(&c.perf, &c.score, &w).report(w.tempo_bpm),
                ),
                ("identity", id.report(&c.score.notes, &c.perf.contexts)),
                (
                    "mass",
                    MassDiagnostics::measure(&c.perf, &c.score, &w).report(),
                ),
                (
                    "tension",
                    TensionDiagnostics::measure(&c.score, &c.perf.contexts, &w).report(&c.score),
                ),
                (
                    "conformance",
                    SongMapConformance::check(&song, &c.perf, &c.score).report(),
                ),
                (
                    "hearings",
                    format!(
                        "{:?}\nstale={:?}\n",
                        c.score.hearings,
                        c.score.stale_hearings()
                    ),
                ),
                ("actions", format!("{:#?}", actions.rows)),
            ] {
                std::fs::write(dir.join(format!("{prefix}.{suffix}.txt")), data)?;
            }
            if (args.iter().any(|s| s == "--render")
                && !arm.starts_with('f')
                && arm != "stable_precursors")
                || (args.iter().any(|s| s == "--render-factorial")
                    && (arm.starts_with('f') || arm == "stable_precursors"))
            {
                let masks = [
                    ("full", StemMask::full()),
                    ("lead", StemMask::solo("lead")),
                    ("bass", StemMask::solo("bass")),
                    ("keys", StemMask::solo("keys")),
                    ("pad", StemMask::solo("pad")),
                    (
                        "lead_bass",
                        StemMask {
                            lead: true,
                            bass: true,
                            ..StemMask::silent()
                        },
                    ),
                    (
                        "support",
                        StemMask {
                            pad: true,
                            keys: true,
                            bass: true,
                            ..StemMask::silent()
                        },
                    ),
                    (
                        "rhythm",
                        StemMask {
                            keys: true,
                            bass: true,
                            drums: true,
                            ..StemMask::silent()
                        },
                    ),
                ];
                for (stem, mask) in masks {
                    if (arm.starts_with('f') || arm == "stable_precursors") && stem != "full" {
                        continue;
                    }
                    renders.push_str(&render(dir, &format!("{prefix}.{stem}"), c, &w, mask)?);
                }
            }
        }
    }
    std::fs::write(dir.join("summary.txt"), &summary)?;
    if !renders.is_empty() {
        std::fs::write(dir.join("render.txt"), &renders)?;
    }
    print!("{summary}{renders}");
    Ok(())
}
