//! OPERATION THE HOMOSAPIANS — the controlled pocket laboratory (R1 metrology).
//!
//! Read-only research instrument. It does NOT change any default, any library ABI, or the C137
//! render path: it reads the SAME frozen rehearsal Score as `--form=pop-rehearsal`, renders it
//! under the SAME warm `pop_world` palette, and the only thing it varies is ONE isolated factor
//! at a time so the ear and the waveform can answer a single question:
//!
//!   is the riff-comp's KEYS_ATTACK_COMP (a 0.03-BEAT early pull, kernel.rs:906/1652) a real
//!   physical-latency correction, or a musical choice wearing a lab coat?
//!
//! TWO FACTORS, held orthogonal (everything else — pitches, riff, chords, bass, drums, voicing,
//! tempo, world — is byte-identical across arms):
//!
//! TIMING — `baseline` is the Score as the kernel emits it (comp pulled 0.03 beat early);
//! `no-advance` is the SAME Score with every `comp` note shifted +0.03 beat, so the comp rides the
//! shared pocket exactly like the required Keys carrier does. A Score-level shift, NOT a const edit,
//! so one build serves all arms.
//!
//! CHORUS — the warm memory-bus chorus ON (ship) vs OFF.
//!
//! It emits, per seed: a loudness-matched A/B (current vs no-advance, full warm band, chorus on)
//! for the ear; dry / chorus Keys stems and a dry kit stem for onset metrology; and a per-comp-note
//! timing receipt (`receipt.tsv`) carrying the P1/P2/P3 witnesses and the measured acoustic onsets.
//! PCM sha256 is taken by the runner (`sha256sum`), not here, so no new dependency is pulled.
//!
//!   cargo +1.99.0 run --release --example homosapians_pocket_lab -- \
//!       --out=/tmp/pocket-lab --seeds=7,42,2112,9,13 --diag-seed=2112
//!
//! NOTE: no source metric in here certifies audio quality. Only Leah's ear promotes an arm.

#[path = "c137_song/rehearsal.rs"]
#[allow(dead_code)]
mod rehearsal;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use gibson::audio::human_music::{
    kernel,
    score::{DrumVoice, Role, Score},
    synth::{ProductionControl, StemMask},
    theory::Mode,
    HumanMusicSynth, MusicWorld,
};
use gibson::audio::{buffer::StereoBlock, wav::write_wav_i16, OfflineRenderer, SampleRate};

/// Mirrors kernel.rs:906 `KEYS_ATTACK_COMP` (private). The amount the riff-comp is pulled early.
const KEYS_ATTACK_COMP: f64 = 0.03;

/// The pop feel renders at 104 BPM ("render --tempo=104 for the pop feel", c137_song.rs).
const POCKET_TEMPO: f32 = 104.0;

/// The composition world the rehearsal Score is performed under (mirrors the audit's `world()` and
/// the `--form=pop-rehearsal` default: A Aeolian, sevenths + modal mixture, 104 BPM).
fn base_world() -> MusicWorld {
    let mut w = MusicWorld::black_ice();
    w.tonic_pc = 9; // A
    w.mode = Mode::Aeolian;
    w.use_sevenths = true;
    w.allow_modal_mixture = true;
    w.tempo_bpm = POCKET_TEMPO;
    w
}

/// The warm render world — a FAITHFUL mirror of `c137_song::pop_world` @ this HEAD (VAPOR95 retuned
/// to key/mode/tempo, tempo echo dropped, room pulled to 0.20). Composition is unchanged; this is a
/// render-world only. Held IDENTICAL across every arm, so the timing A/B isolates timing alone.
fn warm_render_world(base: &MusicWorld) -> MusicWorld {
    let mut w = MusicWorld::vapor95();
    w.tonic_pc = base.tonic_pc;
    w.mode = base.mode;
    w.tempo_bpm = base.tempo_bpm;
    w.production.echo = None;
    w.production.space.mix = 0.20;
    w
}

/// Perform the frozen rehearsal Score for a seed (the SAME path `--form=pop-rehearsal` ships).
fn baseline_score(seed: u64, base: &MusicWorld) -> Score {
    let draft = rehearsal::rehearse(seed);
    let source = rehearsal::freeze(&draft);
    kernel::perform_song(&source, base)
        .expect("opt-in rehearsal Score is lawful")
        .score()
        .clone()
}

/// The `no-advance` timing arm: the SAME Score with every riff-comp note shifted +0.03 beat, so the
/// comp sits on the shared pocket instead of 17 ms ahead of the kit. Nothing else moves — not the
/// carrier, not the bass, not the kit, not a pitch. This is the single-factor timing treatment.
fn no_advance(score: &Score) -> Score {
    let mut s = score.clone();
    for n in s.notes.iter_mut() {
        if n.prov.role_note == "comp" {
            n.start_beat += KEYS_ATTACK_COMP;
        }
    }
    s
}

fn mask(pad: bool, keys: bool, bass: bool, lead: bool, drums: bool, sfx: bool) -> StemMask {
    StemMask {
        pad,
        keys,
        bass,
        lead,
        drums,
        sfx,
    }
}

/// Render a Score under the warm world with a production spec and a stem mask.
fn render(
    score: &Score,
    world: &MusicWorld,
    production: ProductionControl,
    m: StemMask,
) -> StereoBlock {
    let rate = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::with_production(score, world, rate, production);
    synth.set_stem_mask(m);
    let frames = synth.total_samples();
    let out = OfflineRenderer::new(rate, 256).render(&mut synth, frames);
    assert!(!out.had_nonfinite, "nonfinite render");
    out.audio
}

fn prod(spec: &str) -> ProductionControl {
    ProductionControl::parse(spec).expect("valid production spec")
}

fn mono(b: &StereoBlock) -> Vec<f32> {
    (0..b.frames())
        .map(|i| 0.5 * (b.left[i] + b.right[i]))
        .collect()
}

fn rms(b: &StereoBlock) -> f64 {
    let n = b.frames().max(1) as f64;
    let s: f64 = (0..b.frames())
        .map(|i| f64::from(b.left[i]).powi(2) + f64::from(b.right[i]).powi(2))
        .sum();
    (s / (2.0 * n)).sqrt()
}

fn peak(b: &StereoBlock) -> f64 {
    (0..b.frames())
        .map(|i| f64::from(b.left[i]).abs().max(f64::from(b.right[i]).abs()))
        .fold(0.0, f64::max)
}

fn scaled(b: &StereoBlock, g: f32) -> StereoBlock {
    let mut c = b.clone();
    c.scale(g);
    c
}

/// Match arm `b` to the loudness of reference arm `a` (RMS), then pull BOTH down equally if the
/// matched `b` would clip, so the A/B differs only in timing/chorus, never in level.
fn loudness_match(a: &StereoBlock, b: &StereoBlock) -> (StereoBlock, StereoBlock) {
    let (ra, rb) = (rms(a), rms(b));
    let g = if rb > 1e-12 { (ra / rb) as f32 } else { 1.0 };
    let b_m = scaled(b, g);
    let pk = peak(a).max(peak(&b_m));
    if pk > 0.99 {
        let d = (0.99 / pk) as f32;
        (scaled(a, d), scaled(&b_m, d))
    } else {
        (a.clone(), b_m)
    }
}

/// Rectified, ~1 ms-smoothed envelope: a sliding-window mean of |x| over `w` samples. Each step also
/// drops the sample `w` back, so we index `m` by hand while writing `env` through its iterator.
fn envelope(m: &[f32], w: usize) -> Vec<f32> {
    let w = w.max(1);
    let mut env = vec![0.0f32; m.len()];
    let mut acc = 0.0f32;
    for (i, out) in env.iter_mut().enumerate() {
        acc += m[i].abs();
        if i >= w {
            acc -= m[i - w].abs();
        }
        let span = (i + 1).min(w) as f32;
        *out = acc / span;
    }
    env
}

fn beat_to_sample(beat: f64, bpm: f32, sr: f64) -> usize {
    (beat * (60.0 / f64::from(bpm)) * sr).round().max(0.0) as usize
}

/// First sample in [at-2ms, at+30ms] where the envelope crosses `thresh`·(window peak); offset ms.
fn onset_threshold_ms(env: &[f32], at: usize, sr: f64, thresh: f32) -> Option<f64> {
    let lo = at.saturating_sub((0.002 * sr) as usize);
    let hi = (at + (0.030 * sr) as usize).min(env.len());
    if lo >= hi {
        return None;
    }
    let pk = env[lo..hi].iter().copied().fold(0.0f32, f32::max);
    if pk <= 1e-9 {
        return None;
    }
    let target = thresh * pk;
    (lo..hi)
        .find(|&i| env[i] >= target)
        .map(|i| (i as f64 - at as f64) / sr * 1000.0)
}

/// Energy-centroid onset offset (ms) over the same window — robust to the soft chorus tail.
fn onset_centroid_ms(env: &[f32], at: usize, sr: f64) -> Option<f64> {
    let lo = at.saturating_sub((0.002 * sr) as usize);
    let hi = (at + (0.030 * sr) as usize).min(env.len());
    if lo >= hi {
        return None;
    }
    let (mut sw, mut siw) = (0.0f64, 0.0f64);
    for (k, &sample) in env[lo..hi].iter().enumerate() {
        let i = lo + k;
        let e = f64::from(sample);
        sw += e;
        siw += (i as f64 - at as f64) * e;
    }
    if sw <= 1e-12 {
        None
    } else {
        Some(siw / sw / sr * 1000.0)
    }
}

struct Args {
    out: PathBuf,
    seeds: Vec<u64>,
    diag_seed: u64,
}

fn parse_args() -> Args {
    let mut out = PathBuf::from("/tmp/homosapians-pocket-lab");
    let mut seeds = vec![7u64, 42, 2112, 9, 13];
    let mut diag_seed = 2112u64;
    for a in std::env::args().skip(1) {
        if let Some(v) = a.strip_prefix("--out=") {
            out = PathBuf::from(v);
        } else if let Some(v) = a.strip_prefix("--seeds=") {
            seeds = v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        } else if let Some(v) = a.strip_prefix("--diag-seed=") {
            diag_seed = v.parse().unwrap_or(2112);
        }
    }
    Args {
        out,
        seeds,
        diag_seed,
    }
}

/// The three keys productions for the 2x2 and the metrology stems.
fn wav(path: &Path, b: &StereoBlock) -> std::io::Result<()> {
    write_wav_i16(path, b, SampleRate::STUDIO)
}

fn comp_notes(score: &Score) -> Vec<f64> {
    score
        .role_notes(Role::Keys)
        .filter(|n| n.prov.role_note == "comp")
        .map(|n| n.start_beat)
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args();
    std::fs::create_dir_all(&args.out)?;
    let sr = SampleRate::STUDIO.as_f64();

    let mut receipt = String::new();
    receipt.push_str(
        "seed\tcomp_beat_baseline\tcomp_beat_noadv\tnearest_kick_beat\tadvance_beats\tadvance_ms\t\
         chord_at_performed\tchord_at_grid\tp2_crossing\tdry_onset_ms_t25\tchorus_onset_ms_t25\t\
         dry_centroid_ms\tchorus_centroid_ms\n",
    );
    let mut summary = String::new();

    for &seed in &args.seeds {
        let base = base_world();
        let score = baseline_score(seed, &base);
        let score_na = no_advance(&score);
        let rw = warm_render_world(&base);
        let bpm = score.tempo_bpm;
        let seed_dir = args.out.join(format!("seed-{seed}"));
        std::fs::create_dir_all(&seed_dir)?;

        // ---- Listening A/B: current (timing ON) vs no-advance (timing OFF), full warm band, chorus
        //      on, loudness-matched. This is the money comparison for the ear (G10).
        let a_full = render(&score, &rw, ProductionControl::NORMAL, StemMask::full());
        let b_full = render(&score_na, &rw, ProductionControl::NORMAL, StemMask::full());
        let (a_m, b_m) = loudness_match(&a_full, &b_full);
        wav(&seed_dir.join("A_current.wav"), &a_m)?;
        wav(&seed_dir.join("B_noadvance.wav"), &b_m)?;

        // ---- 2x2 chorus diagnostic on the designated seed only (timing x chorus).
        if seed == args.diag_seed {
            let on = ProductionControl::NORMAL;
            let off = prod("nochorus");
            let g = |s: &Score, p: ProductionControl| render(s, &rw, p, StemMask::full());
            // Loudness-match the whole quad to A's RMS so the ear hears timing/chorus, not level.
            let quad = [
                ("2x2_timingON_chorusON", g(&score, on)),
                ("2x2_timingOFF_chorusON", g(&score_na, on)),
                ("2x2_timingON_chorusOFF", g(&score, off)),
                ("2x2_timingOFF_chorusOFF", g(&score_na, off)),
            ];
            let ref_rms = rms(&quad[0].1);
            let pk = quad
                .iter()
                .map(|(_, b)| {
                    let gn = if rms(b) > 1e-12 {
                        (ref_rms / rms(b)) as f32
                    } else {
                        1.0
                    };
                    peak(&scaled(b, gn))
                })
                .fold(0.0, f64::max);
            let headroom = if pk > 0.99 { 0.99 / pk } else { 1.0 } as f32;
            for (name, b) in &quad {
                let gn = if rms(b) > 1e-12 {
                    (ref_rms / rms(b)) as f32
                } else {
                    1.0
                };
                wav(
                    &seed_dir.join(format!("{name}.wav")),
                    &scaled(b, gn * headroom),
                )?;
            }
        }

        // ---- Metrology stems (baseline Score): dry keys, chorus keys, dry kit.
        let keys_only = mask(false, true, false, false, false, false);
        let drums_only = mask(false, false, false, false, true, false);
        let dry_keys = render(
            &score,
            &rw,
            prod("nochorus,noecho,dry,nosat,nocomp"),
            keys_only,
        );
        let chorus_keys = render(&score, &rw, prod("noecho,dry,nosat,nocomp"), keys_only);
        let dry_kit = render(
            &score,
            &rw,
            prod("nochorus,noecho,dry,nosat,nocomp"),
            drums_only,
        );
        wav(&seed_dir.join("stem_dry_keys.wav"), &dry_keys)?;
        wav(&seed_dir.join("stem_chorus_keys.wav"), &chorus_keys)?;
        wav(&seed_dir.join("stem_dry_kit.wav"), &dry_kit)?;

        let env_dry = envelope(&mono(&dry_keys), (0.001 * sr) as usize);
        let env_chorus = envelope(&mono(&chorus_keys), (0.001 * sr) as usize);
        let env_kit = envelope(&mono(&dry_kit), (0.001 * sr) as usize);

        // Kick onsets (dry kit), for the Keys-vs-kit phase.
        let kicks: Vec<f64> = score
            .drums
            .iter()
            .filter(|d| d.voice == DrumVoice::Kick)
            .map(|d| d.start_beat)
            .collect();
        let nearest_kick = |beat: f64| {
            kicks
                .iter()
                .copied()
                .min_by(|a, b| (a - beat).abs().partial_cmp(&(b - beat).abs()).unwrap())
                .unwrap_or(f64::NAN)
        };

        // Per comp-note receipt rows. The baseline comp beat is grid+pocket-0.03; the no-advance
        // beat is +0.03. The nearest kick is the kit's hard anchor at that slot.
        let comps = comp_notes(&score);
        let mut early_vs_kick_ms = Vec::new();
        for &cb in &comps {
            let kb = nearest_kick(cb);
            let adv = kb - cb; // positive => comp is earlier than the kick
            let adv_ms = adv * 60_000.0 / f64::from(bpm);
            // P2: chord sounding at the comp's performed time vs the chord at the grid bar it read.
            let grid = cb + KEYS_ATTACK_COMP; // undo the pull to recover the grid onset
            let bar_start = (grid / 4.0).floor() * 4.0;
            let chord_at = |t: f64| {
                score
                    .chords
                    .iter()
                    .find(|c| c.start_beat <= t && t < c.start_beat + f64::from(c.dur_beats))
                    .map(|c| c.chord)
            };
            let perf_chord = chord_at(cb);
            let grid_chord = chord_at(bar_start);
            let crossing = perf_chord.is_some() && grid_chord.is_some() && perf_chord != grid_chord;

            let at = beat_to_sample(cb, bpm, sr);
            let dry_t25 = onset_threshold_ms(&env_dry, at, sr, 0.25);
            let ch_t25 = onset_threshold_ms(&env_chorus, at, sr, 0.25);
            let dry_c = onset_centroid_ms(&env_dry, at, sr);
            let ch_c = onset_centroid_ms(&env_chorus, at, sr);

            if kb.is_finite() && adv.abs() < 0.2 {
                early_vs_kick_ms.push(adv_ms);
            }
            let f = |o: Option<f64>| o.map(|v| format!("{v:.2}")).unwrap_or_else(|| "NA".into());
            writeln!(
                receipt,
                "{seed}\t{cb:.3}\t{:.3}\t{kb:.3}\t{adv:.3}\t{adv_ms:.2}\t{:?}\t{:?}\t{crossing}\t{}\t{}\t{}\t{}",
                cb + KEYS_ATTACK_COMP,
                perf_chord,
                grid_chord,
                f(dry_t25),
                f(ch_t25),
                f(dry_c),
                f(ch_c),
            )?;
        }

        // Kick acoustic onset offset (dry), averaged over the first handful — the kit's reference.
        let kick_onsets: Vec<f64> = kicks
            .iter()
            .take(8)
            .filter_map(|&kb| onset_threshold_ms(&env_kit, beat_to_sample(kb, bpm, sr), sr, 0.25))
            .collect();
        let mean = |v: &[f64]| {
            if v.is_empty() {
                f64::NAN
            } else {
                v.iter().sum::<f64>() / v.len() as f64
            }
        };

        let crossings = comps
            .iter()
            .filter(|&&cb| {
                let grid = cb + KEYS_ATTACK_COMP;
                let bar = (grid / 4.0).floor() * 4.0;
                let cat = |t: f64| {
                    score
                        .chords
                        .iter()
                        .find(|c| c.start_beat <= t && t < c.start_beat + f64::from(c.dur_beats))
                        .map(|c| c.chord)
                };
                cat(cb).is_some() && cat(bar).is_some() && cat(cb) != cat(bar)
            })
            .count();

        writeln!(
            summary,
            "seed {seed}: {} comp notes | mean comp-earlier-than-kick = {:.2} ms (sched) | \
             dry-keys onset mean = {:.2} ms, kit onset mean = {:.2} ms | P2 bar-crossings = {}",
            comps.len(),
            mean(&early_vs_kick_ms),
            mean(
                &comps
                    .iter()
                    .filter_map(|&cb| onset_threshold_ms(
                        &env_dry,
                        beat_to_sample(cb, bpm, sr),
                        sr,
                        0.25
                    ))
                    .collect::<Vec<_>>()
            ),
            mean(&kick_onsets),
            crossings,
        )?;
    }

    std::fs::write(args.out.join("receipt.tsv"), &receipt)?;
    std::fs::write(args.out.join("summary.txt"), &summary)?;
    print!("{summary}");
    println!(
        "\nwrote {} and per-seed WAVs under {}",
        args.out.join("receipt.tsv").display(),
        args.out.display()
    );
    Ok(())
}
