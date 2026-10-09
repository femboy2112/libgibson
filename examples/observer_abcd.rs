//! **Frozen A/B/C/D auditory-observer experiment** (observer MVP, §10E/§11).
//!
//! One fixed Score / world / seed / sample-rate / block. From the single rendered take A we
//! derive three *controlled* perturbations and ask the observer ([`AuditoryTrace`]) how each
//! moves — per witness, never as one quality number. The machine numbers are **diagnostics
//! only**; the maintainer's ear decides whether the observer is tracking anything that matters.
//!
//! The four signals (all built in the mono domain the observer analyzes, then written as
//! stereo WAVs with L=R=mono, so *what you hear is exactly what was observed*):
//!
//! * **A** — the original HumanMusic render.
//! * **B** — **remove** the declared high-frequency transient/stochastic component. We *declare*
//!   that component operationally as the output of an RBJ high-pass at `--hf=` Hz; `lf = A − hf`
//!   is its exact complement, and `B = lf`. (This is a declared decomposition, not a claim about
//!   HumanMusic's internal stems — the observer never reaches into synthesis.)
//! * **C** — **replace** that HF band with *stationary* noise of matched band energy: deterministic
//!   white noise, high-passed to the same band, scaled to `energy(hf)`. `C = lf + noise_hp`. Spectrum
//!   and energy ≈ A, but the HF transient structure is gone (noise has no attacks).
//! * **D** — **smear transient timing** while preserving spectrum and energy *exactly*: an allpass
//!   cascade over the whole signal. Allpass filters have unit magnitude response, so by Parseval the
//!   magnitude spectrum and total energy are preserved; only phase/group-delay changes, dispersing
//!   transients in time. `D = allpass(A)`. This is the pure-timing control.
//!
//! The hypothesis the quartet tests: **B** moves everything (spectral removal); **C** holds
//! excitation+physical but breaks the HF *temporal* witnesses (stationary substitution); **D** holds
//! excitation+physical *exactly* but moves *onset* alone (pure timing). If the observer shows that
//! pattern it separates three distinct failure modes; if it cannot, this harness says so.
//!
//!   cargo run --release --example observer_abcd -- --out=target/humanmusic-beefup/observer_abcd
//!
//! Not shipped in the library; an analysis/listening instrument on the beef-up branch.

use std::fmt::Write as _;
use std::path::PathBuf;

use gibson::audio::human_music::semantic::demo_trace;
use gibson::audio::human_music::{render, MusicWorld, WorldId};
use gibson::audio::perception::{ablate, AuditoryTrace, ErbBank, ListeningContext, TraceDistance};
use gibson::audio::wav::write_wav_f32;
use gibson::audio::{SampleRate, StereoBlock};

fn arg(prefix: &str) -> Option<String> {
    std::env::args()
        .find(|a| a.starts_with(prefix))
        .map(|a| a[prefix.len()..].to_string())
}

// ---------- tiny deterministic DSP (example-local; the observer stays RNG/DSP-neutral) ----------

/// One RBJ high-pass biquad (Direct Form I), `fc` Hz at quality `q`, sample rate `sr`.
struct HighPass {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}
impl HighPass {
    fn new(fc: f64, q: f64, sr: f64) -> Self {
        let w0 = 2.0 * std::f64::consts::PI * fc / sr;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        let a0 = 1.0 + alpha;
        HighPass {
            b0: ((1.0 + cos) / 2.0 / a0) as f32,
            b1: (-(1.0 + cos) / a0) as f32,
            b2: ((1.0 + cos) / 2.0 / a0) as f32,
            a1: (-2.0 * cos / a0) as f32,
            a2: ((1.0 - alpha) / a0) as f32,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }
    fn run(&mut self, x: &[f32]) -> Vec<f32> {
        x.iter()
            .map(|&x0| {
                let y = self.b0 * x0 + self.b1 * self.x1 + self.b2 * self.x2
                    - self.a1 * self.y1
                    - self.a2 * self.y2;
                self.x2 = self.x1;
                self.x1 = x0;
                self.y2 = self.y1;
                self.y1 = y;
                y
            })
            .collect()
    }
}

/// First-order allpass `H(z) = (c + z^-1)/(1 + c z^-1)` — unit magnitude, frequency-dependent
/// group delay. A cascade disperses transients while preserving the magnitude spectrum exactly.
fn allpass_cascade(x: &[f32], c: f32, sections: usize) -> Vec<f32> {
    let mut sig = x.to_vec();
    for _ in 0..sections {
        let (mut x1, mut y1) = (0.0f32, 0.0f32);
        for s in sig.iter_mut() {
            let x0 = *s;
            let y0 = c * x0 + x1 - c * y1;
            x1 = x0;
            y1 = y0;
            *s = y0;
        }
    }
    sig
}

/// Deterministic white noise in [-1,1], xorshift64 — reproducible, example-local.
fn white_noise(n: usize, seed: u64) -> Vec<f32> {
    let mut s = seed | 1;
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ((s >> 11) as f32 / (1u64 << 53) as f32) * 2.0 - 1.0
        })
        .collect()
}

fn energy(x: &[f32]) -> f64 {
    x.iter().map(|&v| (v as f64) * (v as f64)).sum()
}

/// Non-cryptographic FNV-1a content fingerprint of the mono samples (provenance / repro check).
fn fingerprint(x: &[f32]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &v in x {
        for b in v.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

fn mono_to_stereo(m: &[f32]) -> StereoBlock {
    StereoBlock {
        left: m.to_vec(),
        right: m.to_vec(),
    }
}

fn fmt_dist(d: &TraceDistance) -> String {
    let mut s = String::new();
    for (name, v) in d.witnesses() {
        let _ = write!(s, "{name}={v:.4} ");
    }
    let (w, m) = d.max_witness();
    let _ = write!(s, " [max: {w}={m:.4}]");
    s
}

fn main() -> std::io::Result<()> {
    // ---- frozen experiment parameters ----
    let seed: u64 = arg("--seed=").and_then(|s| s.parse().ok()).unwrap_or(2112);
    let beats: f64 = arg("--beats=").and_then(|s| s.parse().ok()).unwrap_or(32.0);
    let hf_cut: f64 = arg("--hf=").and_then(|s| s.parse().ok()).unwrap_or(4000.0);
    let out = PathBuf::from(
        arg("--out=").unwrap_or_else(|| "target/humanmusic-beefup/observer_abcd".into()),
    );
    std::fs::create_dir_all(&out)?;
    let sr = SampleRate::STUDIO;
    let block = 1024usize;
    let (frame, hop) = (1024usize, 512usize);
    let world = MusicWorld::from_id(WorldId::BlackIce);

    // ---- A: the original render ----
    let trace = demo_trace(beats);
    let (score, res) = render(&trace, &world, seed, sr, block);
    let n = res.audio.frames();
    // mono downmix (the domain the observer analyzes)
    let mono_a: Vec<f32> = (0..n)
        .map(|i| (res.audio.left[i] + res.audio.right[i]) * 0.5)
        .collect();

    // ---- declared HF component and its complement ----
    let hf = HighPass::new(hf_cut, std::f64::consts::FRAC_1_SQRT_2, sr.get() as f64).run(&mono_a);
    let lf: Vec<f32> = (0..n).map(|i| mono_a[i] - hf[i]).collect();

    // ---- B: HF removed ----
    let mono_b = lf.clone();

    // ---- C: HF replaced by energy-matched stationary noise ----
    let noise = white_noise(n, seed ^ 0x5151_5151_5151_5151);
    let noise_hp =
        HighPass::new(hf_cut, std::f64::consts::FRAC_1_SQRT_2, sr.get() as f64).run(&noise);
    let e_hf = energy(&hf);
    let e_nh = energy(&noise_hp).max(1e-12);
    let scale = (e_hf / e_nh).sqrt() as f32;
    let mono_c: Vec<f32> = (0..n).map(|i| lf[i] + noise_hp[i] * scale).collect();

    // ---- D: allpass transient smear (magnitude spectrum + energy preserved) ----
    let mono_d = allpass_cascade(&mono_a, 0.72, 24);

    // ---- observe all four ----
    let ctx = ListeningContext::relative(sr.get());
    let bank = ErbBank::default_for(sr.get());
    let takes: [(&str, &str, &[f32]); 4] = [
        ("A", "original HumanMusic render", &mono_a),
        ("B", "HF transient/stochastic component removed", &mono_b),
        (
            "C",
            "HF replaced by energy-matched stationary noise",
            &mono_c,
        ),
        (
            "D",
            "allpass transient-timing smear (spectrum+energy preserved)",
            &mono_d,
        ),
    ];
    let traces: Vec<AuditoryTrace> = takes
        .iter()
        .map(|(_, _, m)| AuditoryTrace::observe(&ctx, &bank, m, frame, hop))
        .collect();

    // ---- ablation: what does removing the declared HF component do (A vs A−hf == A vs B)? ----
    let abl = ablate(&ctx, &bank, &mono_a, &hf, frame, hop);

    // ---- write WAVs + per-take receipts, and a combined summary ----
    let mut summary = String::new();
    let _ = writeln!(
        summary,
        "observer A/B/C/D — world=BLACK_ICE seed={seed} beats={beats} hf_cut={hf_cut}Hz sr={} block={block} frame={frame} hop={hop}",
        sr.get()
    );
    let _ = writeln!(
        summary,
        "score: {} notes, {} drums, {:.1} beats @ {:.0} bpm; render: {} frames, peak={:.4} rms={:.4}\n",
        score.notes.len(), score.drums.len(), score.total_beats, score.tempo_bpm,
        n, res.peak, res.rms
    );
    for (i, (tag, desc, m)) in takes.iter().enumerate() {
        let wav = out.join(format!("{tag}.wav"));
        write_wav_f32(&wav, &mono_to_stereo(m), sr)?;
        let t = &traces[i];
        let fp = fingerprint(m);
        let mut rec = String::new();
        let _ = writeln!(rec, "take {tag}: {desc}");
        let _ = writeln!(
            rec,
            "world=BLACK_ICE seed={seed} beats={beats} sr={} frames={n}",
            sr.get()
        );
        let _ = writeln!(rec, "wav={} fingerprint(fnv1a)={fp:016x}", wav.display());
        let _ = writeln!(
            rec,
            "physical: rms={:.5} peak={:.5} abs_db_spl={:?} (Relative ctx → None expected)",
            t.physical.rms, t.physical.peak, t.physical.abs_db_spl
        );
        let _ = writeln!(rec, "n_bands={} n_frames={}", t.n_bands, t.n_frames);
        if i > 0 {
            let d = traces[0].distance(t);
            let _ = writeln!(rec, "distance from A: {}", fmt_dist(&d));
            let _ = writeln!(summary, "  d(A,{tag}): {}", fmt_dist(&d));
        } else {
            let _ = writeln!(summary, "  {tag}: {desc}");
        }
        std::fs::write(out.join(format!("{tag}.receipt.txt")), &rec)?;
    }
    let _ = writeln!(
        summary,
        "\nablation (declared HF component removed from A):\n  physical_energy_share = {:.4}\n  observer_distance     = {}",
        abl.physical_energy_share,
        fmt_dist(&abl.observer_distance)
    );
    let _ = writeln!(
        summary,
        "\nREADING GUIDE (hypothesis — the ear is the judge):\n  \
         d(A,B): excitation+onset+physical all move (a band was removed).\n  \
         d(A,C): excitation+physical small, but envelope/tfs/onset move (HF went stationary).\n  \
         d(A,D): excitation+physical ~0 (allpass preserves spectrum+energy), onset moves (pure timing).\n  \
         If C and D are indistinguishable, or D's onset is ~0, the observer does NOT yet separate\n  \
         these modes — report that, do not force it."
    );
    print!("{summary}");
    std::fs::write(out.join("SUMMARY.txt"), &summary)?;
    eprintln!("wrote {}", out.join("SUMMARY.txt").display());
    Ok(())
}
