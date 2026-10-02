//! Round XVII diagnostic-only single-note knockouts of the frozen Round XVI SWISS arm.
//! These counterfactual clones are listening probes, never a production score repair.
use gibson::audio::{
    buffer::StereoBlock,
    human_music::{
        composer::Composer,
        expression::patch,
        functor::perform_phrased,
        performance::PerformanceOptions,
        score::{Role, Score},
        semantic::deflected_lift_trace,
        song::SongMap,
        sonority::audible_end,
        theory::note_name,
        HumanMusicSynth, MusicWorld,
    },
    render::{AudioSource, RenderCtx},
    wav::write_wav_i16,
    SampleRate, SampleTime,
};
use std::{fmt::Write, path::Path};

/// Advance every voice and the complete shared bus from sample zero. Retain only the window,
/// with exactly the normal renderer's 256-frame boundaries, avoiding full-song allocations.
fn render_window(score: &Score, world: &MusicWorld, start: f64, end: f64) -> StereoBlock {
    let sr = SampleRate::STUDIO;
    let samples_per_beat = sr.as_f64() * 60.0 / f64::from(score.tempo_bpm);
    let first = (start * samples_per_beat).round() as u64;
    let last = (end * samples_per_beat).round() as u64;
    let mut synth = HumanMusicSynth::new(score, world, sr);
    let mut out = StereoBlock::new(0);
    let mut block = StereoBlock::new(256);
    let mut pos = 0_u64;
    while pos < last {
        let len = (last - pos).min(256) as usize;
        if block.frames() != len {
            block = StereoBlock::new(len);
        } else {
            block.clear();
        }
        synth.render(
            &mut block,
            &RenderCtx {
                sr,
                start: SampleTime(pos),
            },
        );
        assert!(!block.has_nonfinite());
        if pos + len as u64 > first {
            let offset = first.saturating_sub(pos) as usize;
            out.left.extend_from_slice(&block.left[offset..]);
            out.right.extend_from_slice(&block.right[offset..]);
        }
        pos += len as u64;
    }
    assert_eq!(out.frames() as u64, last - first);
    out
}

fn main() -> std::io::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let dir = Path::new(
        args.iter()
            .find_map(|a| a.strip_prefix("--out="))
            .unwrap_or("target/humanmusic-r17/knockouts"),
    );
    std::fs::create_dir_all(dir)?;
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let world = MusicWorld::swiss_signal();
    let composition = perform_phrased(&song, &world, PerformanceOptions::default());
    let score = &composition.score;
    assert_eq!(score.fingerprint(), 0x0381_7af0_45ff_24d6);
    let mut manifest = format!(
        "Verified frozen source: perform_phrased SWISS score={:016x} song={:016x}\n\
         StablePropulsion seed=2112 default options; sample_rate={} block_frames=256\n\
         Windows [site-2,site+6), continuous rendering from score origin; no tail reset.\n\
         Each knockout sets exactly one source note velocity to zero, retaining its scheduling,\n\
         envelope, voice allocation and oscillator state. Nonlinear shared bus response may change.\n\
         Selection: Pad/Keys/Lead onset < window end and model audible_end > window start.\n\
         Envelope audible_end excludes bus reverb: candidates audible only via older reverb are not enumerated.\n\
         Source note indices are zero-based in the frozen score; note IDs are diagnostic, not new material IDs.\n\
         Boundary: listening probes mutate disposable score clones only; no production repair, no listening verdict.\n\n",
        score.fingerprint(),
        song.fingerprint(),
        SampleRate::STUDIO.get(),
    );
    for site in [28, 36, 44, 60] {
        let start = f64::from(site - 2);
        let end = f64::from(site + 6);
        let baseline = render_window(score, &world, start, end);
        let name = format!("swiss_r16_b{site:03}_baseline.wav");
        write_wav_i16(dir.join(&name), &baseline, SampleRate::STUDIO)?;
        writeln!(
            manifest,
            "{name} beats=[{start},{end}) frames={} peak={:.9} rms={:.9}",
            baseline.frames(),
            baseline.peak(),
            baseline.rms()
        )
        .unwrap();
        for (id, n) in score.notes.iter().enumerate() {
            let end_beat = audible_end(
                n.start_beat,
                f64::from(n.dur_beats),
                patch(&world, n.role),
                score.tempo_bpm,
            );
            if !matches!(n.role, Role::Pad | Role::Keys | Role::Lead)
                || n.start_beat >= end
                || end_beat <= start
            {
                continue;
            }
            let mut knockout = score.clone();
            knockout.notes[id].velocity = 0.0;
            let audio = render_window(&knockout, &world, start, end);
            let delta_rms = (audio
                .left
                .iter()
                .chain(&audio.right)
                .zip(baseline.left.iter().chain(&baseline.right))
                .map(|(a, b)| f64::from(a - b).powi(2))
                .sum::<f64>()
                / (audio.frames() * 2) as f64)
                .sqrt();
            let name = format!(
                "swiss_r16_b{site:03}_mute_{}_{id:04}_m{}.wav",
                n.role.label(),
                n.pitch
            );
            write_wav_i16(dir.join(&name), &audio, SampleRate::STUDIO)?;
            writeln!(manifest, "{name} note_id={id} role={} pitch={} name={} onset={:.6} gate_beats={:.6} audible_end={end_beat:.6} incoming={} sounding_at_site={} peak={:.9} rms={:.9} delta_rms={delta_rms:.9}\n  provenance={:?}", n.role.label(), n.pitch, note_name(n.pitch), n.start_beat, n.dur_beats, n.start_beat < start, n.start_beat <= f64::from(site) && end_beat > f64::from(site), audio.peak(), audio.rms(), n.prov).unwrap();
        }
        eprintln!("finished b{site:03}");
        manifest.push('\n');
    }
    std::fs::write(dir.join("manifest.txt"), manifest)?;
    Ok(())
}
