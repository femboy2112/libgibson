//! Round XIIIb: sounding tension — the matched A/B against Round XIII, its receipts and renders.
//!
//! B is Round XIII `perform_mass` (the arm the maintainer listened to), pinned to its Round XIII
//! receipt fingerprints; C is Round XIIIb `perform_tension`. For the StablePropulsion acceptance
//! song and the MeaningDirected/DeflectedLift control, in SWISS_SIGNAL and BLACK_ICE, the lab
//! checks that C keeps the song, the plan, the drums, the SFX, the chart and the Round XII claims
//! of B, and loses none of B's interaction witnesses; that C leaves no unjustified clash; and that an independent diff
//! of the two scores counts exactly the gate's own ledger. It writes the sounding-tension report of
//! A (Round XII), B and C, the edit ledger, and before/after receipts at every `--around-seconds`.
//!
//! `--render` writes `{swiss,black_ice}_{mass,tension}.wav` (full mix); `--pitched` the same with
//! drums and SFX muted (`*.pitched.wav`, the roughness grader's input); `--dump-notes` every note
//! and the chart as TSV (the independent Python grader's input). StablePropulsion only.
//!
//! Run:
//!   cargo run --release --example sounding_tension_lab -- --around-seconds=16 --around-seconds=44
//!   cargo run --release --example sounding_tension_lab -- --render --out=target/humanmusic-r13b
use gibson::audio::human_music::{
    composer::Composer,
    contract::CompositionGrammar,
    functor::{perform_mass, perform_temporal, perform_tension, Composition},
    mass::patch,
    mass::MassDiagnostics,
    performance::PerformanceOptions,
    score::{Note, Score},
    semantic::deflected_lift_trace,
    song::{SongMap, SongMapConformance},
    sonority::audible_end,
    synth::StemMask,
    tension::{heard_windows, TensionAction, TensionDiagnostics},
    theory::note_name,
    witness, MusicWorld,
};
use std::fmt::Write;
use std::path::Path;

/// Round XIII final-receipt B-arm score fingerprints (docs/fixtures/humanmusic-r13/final/*_mass.mass.txt).
const R13_MASS: [(&str, &str, u64); 4] = [
    ("stable", "swiss", 0xcc5a3c30a90fcd5f),
    ("stable", "black_ice", 0x25bd8016f915e19a),
    ("deflected", "swiss", 0xf5370c7d2f1c7bd4),
    ("deflected", "black_ice", 0xe5b54d4af3e5218b),
];

/// A note's identity for the diff: everything but what the gate may change.
fn same_voice(a: &Note, b: &Note) -> bool {
    a.role == b.role && a.prov.role_note == b.prov.role_note && a.prov.phrase == b.prov.phrase
}

fn exact(a: &Note, b: &Note) -> bool {
    same_voice(a, b)
        && a.pitch == b.pitch
        && a.start_beat == b.start_beat
        && a.dur_beats == b.dur_beats
}

/// The independent diff of B → C: (pitch-changed, duration-changed, re-timed, omitted, added).
fn diff(b: &Score, c: &Score) -> (usize, usize, usize, usize, usize) {
    let mut free: Vec<bool> = vec![true; c.notes.len()];
    let mut gone = Vec::new();
    for n in &b.notes {
        match (0..c.notes.len()).find(|&j| free[j] && exact(n, &c.notes[j])) {
            Some(j) => free[j] = false,
            None => gone.push(*n),
        }
    }
    let (mut pitch, mut dur, mut retimed, mut omitted) = (0, 0, 0, 0);
    for n in gone {
        let end = n.start_beat + f64::from(n.dur_beats);
        let pair = (0..c.notes.len()).find(|&j| {
            let m = &c.notes[j];
            free[j]
                && same_voice(&n, m)
                && ((m.start_beat == n.start_beat
                    && (m.pitch == n.pitch) != (m.dur_beats == n.dur_beats))
                    || (m.pitch == n.pitch && m.start_beat > n.start_beat && m.start_beat < end))
        });
        match pair {
            Some(j) => {
                free[j] = false;
                let m = &c.notes[j];
                if m.start_beat != n.start_beat {
                    retimed += 1;
                } else if m.pitch != n.pitch {
                    pitch += 1;
                } else {
                    dur += 1;
                }
            }
            None => omitted += 1,
        }
    }
    let added = free.iter().filter(|f| **f).count();
    (pitch, dur, retimed, omitted, added)
}

/// Every pitched note sounding at `secs` in `s`, by role, as "role: pitches".
fn sounding(s: &Score, world: &MusicWorld, secs: f64) -> String {
    let spb = 60.0 / f64::from(s.tempo_bpm);
    let win = heard_windows(&s.notes, world, s.tempo_bpm);
    let at = secs / spb;
    let roles: Vec<String> = ["pad", "keys", "bass", "lead"]
        .iter()
        .map(|role| {
            let mut p: Vec<i32> = (0..s.notes.len())
                .filter(|&i| s.notes[i].role.label() == *role && win[i].0 <= at && win[i].1 > at)
                .map(|i| s.notes[i].pitch)
                .collect();
            p.sort_unstable();
            let names: Vec<String> = p.iter().map(|&q| note_name(q)).collect();
            let names = if names.is_empty() {
                "-".into()
            } else {
                names.join(" ")
            };
            format!("{role}: {names}")
        })
        .collect();
    roles.join("  ")
}

/// Every pitched note as TSV (beat, seconds, role, role note, MIDI, name, written beats,
/// velocity, audible seconds), then the chart — the input of the independent Python graders.
fn dump(s: &Score, world: &MusicWorld) -> (String, String) {
    let spb = 60.0 / f64::from(s.tempo_bpm);
    let mut t =
        String::from("beat\tsecs\trole\trole_note\tmidi\tname\tdur_beats\tvel\taudible_secs\n");
    for n in &s.notes {
        let end = audible_end(
            n.start_beat,
            f64::from(n.dur_beats),
            patch(world, n.role),
            s.tempo_bpm,
        );
        let _ = writeln!(
            t,
            "{:.4}\t{:.4}\t{}\t{}\t{}\t{}\t{:.4}\t{:.3}\t{:.4}",
            n.start_beat,
            n.start_beat * spb,
            n.role.label(),
            n.prov.role_note,
            n.pitch,
            note_name(n.pitch),
            n.dur_beats,
            n.velocity,
            (end - n.start_beat) * spb
        );
    }
    let mut c = String::from("beat\tdur\tlabel\tpcs\n");
    for x in &s.chords {
        let _ = writeln!(
            c,
            "{:.3}\t{:.3}\t{}\t{:?}",
            x.start_beat,
            x.dur_beats,
            x.chord.label(),
            x.chord.pitch_classes()
        );
    }
    (t, c)
}

fn render(
    dir: &Path,
    stem: &str,
    score: &Score,
    world: &MusicWorld,
    mask: StemMask,
) -> std::io::Result<String> {
    use gibson::audio::{
        human_music::HumanMusicSynth, render::OfflineRenderer, wav::write_wav_i16, SampleRate,
    };
    let sr = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::new(score, world, sr);
    synth.set_stem_mask(mask);
    let frames = synth.total_samples();
    let audio = OfflineRenderer::new(sr, 256).render(&mut synth, frames);
    assert!(!audio.had_nonfinite);
    let path = dir.join(format!("{stem}.wav"));
    write_wav_i16(&path, &audio.audio, sr)?;
    Ok(format!(
        "wav={} frames={frames} peak={} rms={}",
        path.display(),
        audio.peak,
        audio.rms
    ))
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| {
        args.iter()
            .find_map(|a| a.strip_prefix(k).map(str::to_owned))
    };
    let out = arg("--out=").unwrap_or_else(|| "target/humanmusic-r13b".into());
    let radius: f64 = arg("--radius=").map_or(1.5, |r| r.parse().expect("--radius=seconds"));
    let around: Vec<f64> = args
        .iter()
        .filter_map(|a| a.strip_prefix("--around-seconds="))
        .map(|s| s.parse().expect("--around-seconds=seconds"))
        .collect();
    let do_render = args.iter().any(|a| a == "--render");
    let pitched = args.iter().any(|a| a == "--pitched");
    let dump_notes = args.iter().any(|a| a == "--dump-notes");
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
            let a = perform_temporal(&song, &world, opts);
            let b = perform_mass(&song, &world, opts);
            let c = perform_tension(&song, &world, opts);
            let pinned = R13_MASS
                .iter()
                .find(|f| f.0 == regime && f.1 == name)
                .unwrap()
                .2;
            assert_eq!(b.score.fingerprint(), pinned, "not the Round XIII B arm");
            assert_eq!(b.perf.fingerprint(), c.perf.fingerprint(), "plan moved");
            for (what, x, y) in [
                (
                    "drums",
                    format!("{:?}", b.score.drums),
                    format!("{:?}", c.score.drums),
                ),
                (
                    "sfx",
                    format!("{:?}", b.score.sfx),
                    format!("{:?}", c.score.sfx),
                ),
                (
                    "chart",
                    format!("{:?}", b.score.chords),
                    format!("{:?}", c.score.chords),
                ),
            ] {
                assert_eq!(x, y, "{what} moved");
            }
            let law = SongMapConformance::check(&song, &c.perf, &c.score);
            assert!(law.passes(), "{}", law.report());
            let base = MassDiagnostics::measure(&c.perf, &c.score, &world).base;
            let claims = base.false_function_claims
                + base.false_suspensions
                + base.broken_anticipations
                + base.bad_arrivals
                + base.unresolved_tendencies;
            assert_eq!(claims, 0, "{}", base.report());
            let (wb, wc) = (
                witness::audit(&b.perf, &b.score),
                witness::audit(&c.perf, &c.score),
            );
            assert_eq!(wb.total(), wc.total());
            let mut gained = 0;
            for (x, y) in wb.rows.iter().zip(&wc.rows) {
                assert!(
                    !x.witnessed || y.witnessed,
                    "receipt lost: {:?}\n{}",
                    x.action,
                    wc.report()
                );
                gained += usize::from(y.witnessed && !x.witnessed);
            }
            let tension =
                |x: &Composition| TensionDiagnostics::measure(&x.score, &x.perf.contexts, &world);
            let (ta, tb, tc) = (tension(&a), tension(&b), tension(&c));
            assert_eq!(tc.unjustified, 0, "{}", tc.report(&c.score));
            let header = |arm: &str, x: &Composition| {
                format!(
                    "regime={regime} world={name} seed=2112 beats=120 tempo={} arm={arm}\nsong={:#018x} performance={:#018x} score={:#018x}\n",
                    x.score.tempo_bpm,
                    song.fingerprint(),
                    x.perf.fingerprint(),
                    x.score.fingerprint()
                )
            };
            for (arm, x, t) in [("r12", &a, &ta), ("mass", &b, &tb), ("tension", &c, &tc)] {
                std::fs::write(
                    dir.join(format!("{name}_{regime}_{arm}.tension.txt")),
                    format!("{}{}", header(arm, x), t.report(&x.score)),
                )?;
            }
            // The gate's ledger, and an independent diff that must count the same edits.
            let edits = &c.score.tension_edits;
            let count =
                |f: fn(&TensionAction) -> bool| edits.iter().filter(|e| f(&e.action)).count();
            let (pitch, dur, retimed, omitted, added) = diff(&b.score, &c.score);
            let ledger_counts = (
                count(|a| matches!(a, TensionAction::Displaced(_) | TensionAction::Moved(_))),
                count(|a| matches!(a, TensionAction::Shortened(_))),
                count(|a| matches!(a, TensionAction::Ornament { .. })),
                count(|a| matches!(a, TensionAction::OmittedCovered | TensionAction::Omitted)),
                0,
            );
            assert_eq!(
                (pitch, dur, retimed, omitted, added),
                ledger_counts,
                "ledger vs diff"
            );
            let spb = 60.0 / f64::from(c.score.tempo_bpm);
            let mut tsv = String::from(
                "beat\tseconds\trole\trole_note\tnote\tbeats\taction\ttension\tpartner\tinterval\texposure\tclass\n",
            );
            for e in edits {
                let n = &e.note;
                let _ = writeln!(
                    tsv,
                    "{:.4}\t{:.3}\t{}\t{}\t{}\t{:.3}\t{:?}\t{} {} @{:.3}\t{} {} @{:.3}\t{}\t{:.3}\t{:?}",
                    n.start_beat,
                    n.start_beat * spb,
                    n.role.label(),
                    n.prov.role_note,
                    note_name(n.pitch),
                    n.dur_beats,
                    e.action,
                    e.tension.role.label(),
                    note_name(e.tension.pitch),
                    e.tension.start_beat,
                    e.partner.role.label(),
                    note_name(e.partner.pitch),
                    e.partner.start_beat,
                    (e.tension.pitch - e.partner.pitch).abs(),
                    e.exposure,
                    e.class
                );
            }
            let total = b.score.notes.len();
            let changed = pitch + dur + retimed + omitted + added;
            let perturbation = format!(
                "pitched_notes={total} pitch_changed={pitch} duration_changed={dur} retimed={retimed} omitted={omitted} added={added} changed_events={changed} affected={:.3}%",
                100.0 * changed as f64 / total as f64
            );
            std::fs::write(
                dir.join(format!("{name}_{regime}.tension-changes.tsv")),
                format!("# {perturbation}\n{tsv}"),
            )?;
            let summary = |t: &TensionDiagnostics| {
                format!(
                    "clashes={} transient={} foreshadowing={} suspension={} anticipation={} unjustified={}",
                    t.clashes.len(),
                    t.transient,
                    t.foreshadowing,
                    t.suspension,
                    t.anticipation,
                    t.unjustified
                )
            };
            println!(
                "{name}/{regime} r12:     {}\n{name}/{regime} mass:    {}\n{name}/{regime} tension: {}\n{name}/{regime} {perturbation}\n{name}/{regime} witnesses {}/{} (stamped {}; lost 0, gained {gained}), R12 claims 0, {}",
                summary(&ta),
                summary(&tb),
                summary(&tc),
                wc.witnessed(),
                wc.total(),
                wc.witnessed_stamped(),
                law.report().lines().next().unwrap_or("")
            );
            if regime != "stable" {
                continue;
            }
            for &s in &around {
                let mut text = format!(
                    "{}{}before (mass) -> after (tension), within {radius} s of {s} s\n",
                    header("mass", &b),
                    header("tension", &c)
                );
                for probe in [s - 1.0, s - 0.5, s, s + 0.5, s + 1.0] {
                    let _ = writeln!(
                        text,
                        "sounding at {probe:.2} s\n  before: {}\n  after:  {}",
                        sounding(&b.score, &world, probe),
                        sounding(&c.score, &world, probe)
                    );
                }
                let _ = writeln!(
                    text,
                    "clashes before (beat, seconds, responsible note, partner, exposure, verdict):\n{}clashes after:\n{}edits:",
                    tb.around_seconds(&b.score, s, radius),
                    tc.around_seconds(&c.score, s, radius)
                );
                for e in edits {
                    let t0 = e.note.start_beat * spb;
                    if t0 <= s + radius && t0 + f64::from(e.note.dur_beats) * spb >= s - radius {
                        let _ = writeln!(
                            text,
                            "  beat {:.3} ({t0:.3} s) {} {} {} {:.2} beats -> {:?} (clash {} vs {}, exposure {:.3} {:?})",
                            e.note.start_beat,
                            e.note.role.label(),
                            e.note.prov.role_note,
                            note_name(e.note.pitch),
                            e.note.dur_beats,
                            e.action,
                            note_name(e.tension.pitch),
                            note_name(e.partner.pitch),
                            e.exposure,
                            e.class
                        );
                    }
                }
                std::fs::write(dir.join(format!("{name}_stable.around-{s}s.txt")), &text)?;
            }
            for (arm, x) in [("mass", &b), ("tension", &c)] {
                if dump_notes {
                    let (notes, chart) = dump(&x.score, &world);
                    std::fs::write(dir.join(format!("{name}_{arm}.notes.tsv")), notes)?;
                    std::fs::write(dir.join(format!("{name}_{arm}.chords.tsv")), chart)?;
                }
                if do_render {
                    let full = render(
                        dir,
                        &format!("{name}_{arm}"),
                        &x.score,
                        &world,
                        StemMask::full(),
                    )?;
                    println!("{full}");
                }
                if pitched {
                    let p = render(
                        dir,
                        &format!("{name}_{arm}.pitched"),
                        &x.score,
                        &world,
                        StemMask::pitched(),
                    )?;
                    println!("{p}");
                }
            }
        }
    }
    Ok(())
}
