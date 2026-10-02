//! Round XII: independent audit of the unchanged Round XI specimen.
use gibson::audio::human_music::{
    composer::Composer,
    contract::CompositionGrammar,
    diagnostics::RealizationDiagnostics,
    functor::{perform, perform_temporal},
    performance::PerformanceOptions,
    phenomenal::PhenomenalTrajectory,
    score::Score,
    semantic::deflected_lift_trace,
    song::{SongMap, SongMapConformance},
    temporal::TemporalPitchDiagnostics,
    theory::note_name,
    MusicWorld,
};
use std::{fmt::Write, path::Path};

fn notes(score: &Score) -> String {
    let mut out =
        String::from("index\tbeat\tduration\trole\tpitch\tname\tfunction\tmaterial\trole_note\n");
    for (i, n) in score.notes.iter().enumerate() {
        let _ = writeln!(
            out,
            "{i}\t{:.17}\t{:.9}\t{}\t{}\t{}\t{:?}\t{:?}\t{}",
            n.start_beat,
            n.dur_beats,
            n.role.label(),
            n.pitch,
            note_name(n.pitch),
            n.function,
            n.prov.material,
            n.prov.role_note
        );
    }
    out
}
fn main() -> std::io::Result<()> {
    let out = std::env::args()
        .find_map(|a| a.strip_prefix("--out=").map(str::to_owned))
        .unwrap_or_else(|| "target/humanmusic-r12/baseline".into());
    let compare = std::env::args().any(|a| a == "--compare");
    let render = std::env::args().any(|a| a == "--render");
    let dir = Path::new(&out);
    std::fs::create_dir_all(dir)?;
    let trace = deflected_lift_trace(120.0);
    let opts = PerformanceOptions::default();
    for (regime, composer, grammar) in [
        ("stable", Composer::StablePropulsion, None),
        (
            "deflected",
            Composer::MeaningDirected,
            Some(CompositionGrammar::DeflectedLift),
        ),
    ] {
        let song = SongMap::compose(&trace, 2112, grammar, composer);
        for (name, world) in [
            ("swiss", MusicWorld::swiss_signal()),
            ("black_ice", MusicWorld::black_ice()),
        ] {
            let c = perform(&song, &world, opts);
            std::fs::write(
                dir.join(format!("{name}_{regime}.performance.txt")),
                format!("{:?}", c.perf),
            )?;
            std::fs::write(dir.join(format!("{regime}.song.txt")), format!("{song:?}"))?;
            if compare {
                let b = perform_temporal(&song, &world, opts);
                assert_eq!(c.perf.fingerprint(), b.perf.fingerprint());
                let audit_b = TemporalPitchDiagnostics::measure(&b.perf, &b.score);
                let law_b = SongMapConformance::check(&song, &b.perf, &b.score);
                assert!(law_b.passes(), "{}", law_b.report());
                assert_eq!(audit_b.false_function_claims, 0, "{}", audit_b.report());
                assert_eq!(
                    audit_b.false_suspensions
                        + audit_b.broken_anticipations
                        + audit_b.bad_arrivals
                        + audit_b.unresolved_tendencies,
                    0,
                    "{}",
                    audit_b.report()
                );
                let audit_a = TemporalPitchDiagnostics::measure(&c.perf, &c.score);
                assert!(
                    audit_b.orphan_structural_extensions < audit_a.orphan_structural_extensions
                );
                assert!(
                    audit_b.path_breaks_at_harmony_changes
                        <= audit_a.path_breaks_at_harmony_changes
                );
                let stem = format!("{name}_{regime}_temporal");
                std::fs::write(
                    dir.join(format!("{stem}.score.txt")),
                    format!("{:?}", b.score),
                )?;
                let old_b = RealizationDiagnostics::measure(&song.plan, &b.score);
                std::fs::write(dir.join(format!("{stem}.notes.tsv")), notes(&b.score))?;
                std::fs::write(dir.join(format!("{stem}.audit.txt")),format!("song={:#018x} performance={:#018x} score={:#018x}\nold_unjustified={:?}\n{}\n{}",song.fingerprint(),b.perf.fingerprint(),b.score.fingerprint(),old_b.unjustified_by_role,law_b.report(),audit_b.report()))?;
                std::fs::write(
                    dir.join(format!("{name}_{regime}.changes.tsv")),
                    changes(&c.score, &b.score, &c.perf),
                )?;
                let good = audit_b
                    .rows
                    .iter()
                    .filter(|r| {
                        !r.suspect
                            && b.score.notes[r.note_index].function.is_some_and(|f| {
                                f != gibson::audio::human_music::score::PitchFunction::ChordTone
                            })
                    })
                    .map(|r| r.detail.as_str())
                    .collect::<String>();
                std::fs::write(dir.join(format!("{name}_{regime}.good-paths.txt")), good)?;
                println!("temporal {name}/{regime}: {}", law_b.report());
                if render && regime == "stable" {
                    render_score(dir, &format!("{name}_r11"), &c.score, &world)?;
                    render_score(dir, &format!("{name}_temporal"), &b.score, &world)?;
                }
            }
            let old = RealizationDiagnostics::measure(&song.plan, &c.score);
            let audit = TemporalPitchDiagnostics::measure(&c.perf, &c.score);
            let law = SongMapConformance::check(&song, &c.perf, &c.score);
            assert!(law.passes(), "{}", law.report());
            let header = format!("regime={regime} world={name} seed=2112 beats=120 options={opts:?}\nsong={:#018x} performance={:#018x} score={:#018x}\nold_unjustified={:?}\n{}\n",song.fingerprint(),c.perf.fingerprint(),c.score.fingerprint(),old.unjustified_by_role,law.report());
            println!("{header}{}", audit.report());
            let stem = format!("{name}_{regime}_r11");
            std::fs::write(dir.join(format!("{stem}.notes.tsv")), notes(&c.score))?;
            std::fs::write(
                dir.join(format!("{stem}.audit.txt")),
                format!("{header}{}", audit.report()),
            )?;
            std::fs::write(
                dir.join(format!("{stem}.score.txt")),
                format!("{:?}", c.score),
            )?;
            std::fs::write(
                dir.join(format!("{regime}.trajectory.txt")),
                PhenomenalTrajectory::observe(&song).report(),
            )?;
        }
    }
    Ok(())
}

fn changes(
    a: &Score,
    b: &Score,
    perf: &gibson::audio::human_music::performance::PerformancePlan,
) -> String {
    let mut out=String::from("index\tbeat\trole\told_note\tnew_note\told_function\tnew_function\tcurrent_chord\tnext_chord\treason\n");
    let before = TemporalPitchDiagnostics::measure(perf, a);
    let after = TemporalPitchDiagnostics::measure(perf, b);
    let mut pitches = 0;
    let mut durations = 0;
    let mut onsets = 0;
    let mut labels = 0;
    for (i, (x, y)) in a.notes.iter().zip(&b.notes).enumerate() {
        durations += usize::from(x.dur_beats != y.dur_beats);
        onsets += usize::from(x.start_beat != y.start_beat || x.role != y.role);
        labels += usize::from(x.function != y.function);
        if x.pitch == y.pitch {
            continue;
        }
        pitches += 1;
        let ci = perf.contexts.iter().position(|c| {
            x.start_beat >= c.start_beat && x.start_beat < c.start_beat + f64::from(c.dur_beats)
        });
        let chord = ci
            .map(|j| perf.contexts[j].chord.label())
            .unwrap_or_default();
        let next = ci
            .and_then(|j| perf.contexts.get(j + 1))
            .map(|c| c.chord.label())
            .unwrap_or_default();
        let why = match x.role {
            gibson::audio::human_music::score::Role::Lead => "bounded target/connector path search",
            gibson::audio::human_music::score::Role::Bass => {
                "actual approach destination register or shared unison"
            }
            gibson::audio::human_music::score::Role::Keys => {
                "response path, written landing, boundary carry or lead-dependent voicing"
            }
            _ => "lead-dependent existing voicing",
        };
        let reason = format!(
            "{why}; before: {}; after: {:?} {:?} supported {:?}",
            before.rows[i].reasons.join("; "),
            after.rows[i].status,
            after.rows[i].extension,
            after.rows[i].supported
        );
        let _ = writeln!(
            out,
            "{i}\t{:.4}\t{}\t{}\t{}\t{:?}\t{:?}\t{chord}\t{next}\t{reason}",
            x.start_beat,
            x.role.label(),
            note_name(x.pitch),
            note_name(y.pitch),
            x.function,
            y.function
        );
    }
    let summary=format!("total_pitched_notes={} changed_pitches={pitches} changed_durations={durations} changed_onsets_or_roles={onsets} changed_labels={labels} removed_notes={} added_notes={} edit_fraction={:.6}",a.notes.len(),a.notes.len().saturating_sub(b.notes.len()),b.notes.len().saturating_sub(a.notes.len()),pitches as f64/a.notes.len().max(1) as f64);
    assert_eq!(
        a.notes.len(),
        b.notes.len(),
        "pitch trial changed note count"
    );
    assert_eq!(durations + onsets, 0, "pitch trial changed event geometry");
    assert_eq!(format!("{:?}", a.drums), format!("{:?}", b.drums));
    assert_eq!(format!("{:?}", a.sfx), format!("{:?}", b.sfx));
    assert_eq!(format!("{:?}", a.chords), format!("{:?}", b.chords));
    for (x, y) in a.notes.iter().zip(&b.notes) {
        assert_eq!(x.velocity, y.velocity);
        assert_eq!(format!("{:?}", x.prov), format!("{:?}", y.prov));
    }
    println!("{summary}");
    format!("# {summary}\n{out}")
}
fn render_score(dir: &Path, stem: &str, score: &Score, world: &MusicWorld) -> std::io::Result<()> {
    use gibson::audio::{
        human_music::HumanMusicSynth, render::OfflineRenderer, wav::write_wav_i16, SampleRate,
    };
    let sr = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::new(score, world, sr);
    let frames = synth.total_samples();
    let audio = OfflineRenderer::new(sr, 256).render(&mut synth, frames);
    assert!(!audio.had_nonfinite);
    let path = dir.join(format!("{stem}.wav"));
    write_wav_i16(&path, &audio.audio, sr)?;
    println!(
        "wav={} frames={frames} peak={} rms={}",
        path.display(),
        audio.peak,
        audio.rms
    );
    Ok(())
}
