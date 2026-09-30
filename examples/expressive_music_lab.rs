//! Round XV: frozen R14 vs source expression, full mixes, isolated lines, and exact receipts.
use gibson::audio::{
    human_music::{
        composer::Composer,
        expression::ExpressionDiagnostics,
        functor::{perform_coherent, perform_expressive, Composition},
        identity::IdentityDiagnostics,
        performance::PerformanceOptions,
        score::{Note, Role},
        semantic::deflected_lift_trace,
        song::{SongMap, SongMapConformance},
        synth::StemMask,
        temporal::TemporalPitchDiagnostics,
        theory::note_name,
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
    x: &Composition,
    w: &MusicWorld,
    mask: StemMask,
) -> std::io::Result<String> {
    let sr = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::new(&x.score, w, sr);
    synth.set_stem_mask(mask);
    let frames = synth.total_samples();
    let audio = OfflineRenderer::new(sr, 256).render(&mut synth, frames);
    assert!(!audio.had_nonfinite);
    let path = dir.join(format!("{name}.wav"));
    write_wav_i16(&path, &audio.audio, sr)?;
    Ok(format!(
        "{} frames={frames} peak={} rms={}",
        path.display(),
        audio.peak,
        audio.rms
    ))
}

fn signature(n: &Note) -> String {
    format!("{n:?}")
}

/// Preserve role sequence identity; omissions use the decision ledger, and all retained events
/// are matched by provenance/pitch in order. This additionally captures dependent keys/unisons.
fn perturbation(a: &Composition, b: &Composition) -> String {
    let mut out=String::from("Independent role-sequence diff; source omissions identified explicitly. Times in beats AND seconds.\n");
    let spb = 60.0 / f64::from(a.score.tempo_bpm);
    for role in [Role::Lead, Role::Bass, Role::Keys, Role::Pad] {
        let mut old: Vec<_> = a.score.role_notes(role).collect();
        let mut new: Vec<_> = b.score.role_notes(role).collect();
        old.sort_by(|a, b| {
            a.start_beat
                .total_cmp(&b.start_beat)
                .then(a.pitch.cmp(&b.pitch))
        });
        new.sort_by(|a, b| {
            a.start_beat
                .total_cmp(&b.start_beat)
                .then(a.pitch.cmp(&b.pitch))
        });
        let same = |n: &Note, m: &Note| {
            n.prov == m.prov
                && (n.pitch == m.pitch && n.start_beat == m.start_beat
                    || n.pitch == m.pitch
                        && (n.prov.role_note == "unison"
                            || b.score.expression_decisions.iter().any(|d| {
                                d.before.note.role == role
                                    && d.before.note.start_beat == n.start_beat
                                    && d.after
                                        .is_some_and(|after| after.start_beat == m.start_beat)
                            })))
        };
        let mut dp = vec![vec![0usize; new.len() + 1]; old.len() + 1];
        for i in (0..old.len()).rev() {
            for j in (0..new.len()).rev() {
                dp[i][j] = if same(old[i], new[j]) {
                    1 + dp[i + 1][j + 1]
                } else {
                    dp[i + 1][j].max(dp[i][j + 1])
                };
            }
        }
        let (mut i, mut j, mut omitted, mut added) = (0, 0, 0, 0);
        let mut pairs = Vec::new();
        let mut unmatched = String::new();
        while i < old.len() || j < new.len() {
            if i < old.len() && j < new.len() && same(old[i], new[j]) {
                pairs.push((old[i], new[j]));
                i += 1;
                j += 1;
            } else if j == new.len() || i < old.len() && dp[i + 1][j] >= dp[i][j + 1] {
                let _ = writeln!(unmatched, "omitted {:?}", old[i]);
                omitted += 1;
                i += 1;
            } else {
                let _ = writeln!(unmatched, "added {:?}", new[j]);
                added += 1;
                j += 1;
            }
        }
        let (mut retimed, mut duration, mut velocity, mut pitch) = (0, 0, 0, 0);
        let mut lines = String::new();
        for (n, m) in pairs {
            if signature(n) == signature(m) {
                continue;
            }
            retimed += usize::from(n.start_beat != m.start_beat);
            duration += usize::from(n.dur_beats != m.dur_beats);
            velocity += usize::from(n.velocity != m.velocity);
            pitch += usize::from(n.pitch != m.pitch);
            let strategy = b
                .score
                .expression_decisions
                .iter()
                .find(|d| d.before.note.role == role && d.before.note.start_beat == n.start_beat)
                .map_or("inherited final lead/unison".into(), |d| {
                    format!("{:?}", d.strategy)
                });
            let _=writeln!(lines,"{} {} -> {} {:?} onset {:.9}b/{:.9}s -> {:.9}b/{:.9}s duration {:.9}b/{:.9}s -> {:.9}b/{:.9}s velocity {:.9} -> {:.9} strategy={strategy}",role.label(),note_name(n.pitch),note_name(m.pitch),n.function,n.start_beat,n.start_beat*spb,m.start_beat,m.start_beat*spb,n.dur_beats,f64::from(n.dur_beats)*spb,m.dur_beats,f64::from(m.dur_beats)*spb,n.velocity,m.velocity);
        }
        let _=writeln!(out,"{}: events {} -> {}; retimed={retimed} duration={duration} velocity={velocity} pitch_substitute={pitch} omitted={omitted} added={added} glide=0",role.label(),old.len(),new.len());
        out.push_str(&lines);
        out.push_str(&unmatched);
    }
    let _ = writeln!(
        out,
        "drums_identical={} sfx_identical={}",
        format!("{:?}", a.score.drums) == format!("{:?}", b.score.drums),
        format!("{:?}", a.score.sfx) == format!("{:?}", b.score.sfx)
    );
    out
}

fn main() -> std::io::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let dir = Path::new(
        args.iter()
            .find_map(|s| s.strip_prefix("--out="))
            .unwrap_or("target/humanmusic-r15/final"),
    );
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
    let mut renders = String::new();
    for (name, w) in [
        ("black_ice", MusicWorld::black_ice()),
        ("swiss", MusicWorld::swiss_signal()),
    ] {
        let a = perform_coherent(&song, &w, PerformanceOptions::default());
        let b = perform_expressive(&song, &w, PerformanceOptions::default());
        std::fs::write(
            dir.join(format!("{name}.perturbation.txt")),
            perturbation(&a, &b),
        )?;
        let mut decisions = String::new();
        for d in &b.score.expression_decisions {
            let _ = writeln!(
                decisions,
                "{:?}: {}\nbefore: {}after: {:?}\n",
                d.strategy,
                d.reason,
                ExpressionDiagnostics {
                    rows: vec![d.before.clone()]
                }
                .report(w.tempo_bpm),
                d.after
            );
            if let Some(r) = &d.after_observation {
                decisions.push_str(
                    &ExpressionDiagnostics {
                        rows: vec![r.clone()],
                    }
                    .report(w.tempo_bpm),
                );
            }
        }
        std::fs::write(dir.join(format!("{name}.decisions.txt")), decisions)?;
        for (arm, x) in [("r14", &a), ("expressive", &b)] {
            let id =
                IdentityDiagnostics::measure(&x.score.notes, &x.perf.contexts, &w, w.tempo_bpm);
            let audit = ExpressionDiagnostics::measure(&x.perf, &x.score, &w);
            let temporal = TemporalPitchDiagnostics::measure(&x.perf, &x.score);
            let witnesses = witness::audit(&x.perf, &x.score);
            let _=writeln!(summary,"{name} {arm} tempo={} score={:016x} perf={:016x} notes={} decisions={} flips={} flip_seconds={} stale_hearings={} temporal_false_claims={} receipts={}/{}",w.tempo_bpm,x.score.fingerprint(),x.perf.fingerprint(),x.score.notes.len(),x.score.expression_decisions.len(),id.flips().count(),id.flipped_secs(),x.score.stale_hearings().len(),temporal.false_function_claims,witnesses.rows.iter().filter(|r| r.witnessed).count(),witnesses.rows.len());
            std::fs::write(
                dir.join(format!("{name}_{arm}.expression.txt")),
                audit.report(w.tempo_bpm),
            )?;
            std::fs::write(
                dir.join(format!("{name}_{arm}.identity.txt")),
                id.report(&x.score.notes, &x.perf.contexts),
            )?;
            std::fs::write(
                dir.join(format!("{name}_{arm}.conformance.txt")),
                SongMapConformance::check(&song, &x.perf, &x.score).report(),
            )?;
            std::fs::write(
                dir.join(format!("{name}_{arm}.hearings.txt")),
                format!(
                    "{:?}\nstale={:?}\n",
                    x.score.hearings,
                    x.score.stale_hearings()
                ),
            )?;
            std::fs::write(
                dir.join(format!("{name}_{arm}.receipts.txt")),
                format!("{:#?}\n", witnesses.rows),
            )?;
            std::fs::write(
                dir.join(format!("{name}_{arm}.notes.txt")),
                x.score
                    .notes
                    .iter()
                    .map(|n| format!("{n:?}\n"))
                    .collect::<String>(),
            )?;
            if args.iter().any(|s| s == "--render") {
                let _ = writeln!(
                    renders,
                    "{}",
                    render(dir, &format!("{name}_{arm}"), x, &w, StemMask::full())?
                );
                if name == "black_ice" {
                    for role in ["lead", "bass"] {
                        let _ = writeln!(
                            renders,
                            "{}",
                            render(
                                dir,
                                &format!("{name}_{arm}.{role}"),
                                x,
                                &w,
                                StemMask::solo(role)
                            )?
                        );
                    }
                }
            }
        }
    }
    print!("{summary}{renders}");
    std::fs::write(dir.join("summary.txt"), summary)?;
    std::fs::write(dir.join("render.txt"), renders)?;
    Ok(())
}
