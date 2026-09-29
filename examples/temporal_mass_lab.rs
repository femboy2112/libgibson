//! Round XIII: temporal-mass audit, human-timestamp lookup, and the matched A/B.
//!
//! `--around-seconds=S` (repeatable) prints every note sounding within `--radius=R` seconds
//! (default 1.5) of `S` in the SWISS and BLACK_ICE StablePropulsion scores: harmony, written and
//! audible seconds, mass evidence, Round XII path and the mass verdict. Text only.
//!
//! `--compare` performs the same song and plan twice — A: Round XII `perform_temporal`, B: Round
//! XIII `perform_mass` — checks that only pad/keys voicing notes moved, and writes the
//! perturbation ledger and before/after receipts at every `--around-seconds`. `--render` writes
//! the four StablePropulsion WAVs (`{swiss,black_ice}_{r12,mass}.wav`).
use gibson::audio::human_music::{
    comp::{self, SupportMassAction},
    composer::Composer,
    contract::CompositionGrammar,
    functor::{perform_mass, perform_temporal},
    mass::{MassDiagnostics, MassVerdict},
    melody,
    performance::PerformanceOptions,
    phenomenal::PhenomenalTrajectory,
    score::{Note, Role, Score},
    semantic::deflected_lift_trace,
    song::{SongMap, SongMapConformance},
    theory::note_name,
    MusicWorld,
};
use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;

/// Round XII final-receipt score fingerprints (docs/fixtures/humanmusic-r12/final/*_temporal.audit.txt).
const FROZEN: [(&str, &str, u64); 2] = [
    ("stable", "swiss", 0x8ab3cd9fa10d751c),
    ("stable", "black_ice", 0x06446ebcbbd14038),
];

/// How a B note realizes an A note.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Fate {
    Same(usize),
    Changed(usize),
    Omitted,
}

/// Match A's notes to B's, one onset bundle (role, onset, role note) at a time: shared pitches
/// first, then the remaining pitches by nearest distance (a substitution); leftovers were omitted
/// (A) or added (B).
fn align(a: &Score, b: &Score) -> (Vec<Fate>, Vec<usize>) {
    type Key = (&'static str, i64, &'static str);
    let key = |n: &Note| -> Key {
        (
            n.role.label(),
            (n.start_beat * 1e6).round() as i64,
            n.prov.role_note,
        )
    };
    let mut bundles: BTreeMap<Key, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
    for (i, n) in a.notes.iter().enumerate() {
        bundles.entry(key(n)).or_default().0.push(i);
    }
    for (j, n) in b.notes.iter().enumerate() {
        bundles.entry(key(n)).or_default().1.push(j);
    }
    let mut fate = vec![Fate::Omitted; a.notes.len()];
    let mut added = Vec::new();
    for (_, (xs, ys)) in bundles {
        let mut free: Vec<usize> = ys.clone();
        let mut left = Vec::new();
        for &i in &xs {
            if let Some(k) = free
                .iter()
                .position(|&j| b.notes[j].pitch == a.notes[i].pitch)
            {
                let j = free.remove(k);
                let same = a.notes[i].dur_beats == b.notes[j].dur_beats;
                fate[i] = if same {
                    Fate::Same(j)
                } else {
                    Fate::Changed(j)
                };
            } else {
                left.push(i);
            }
        }
        for i in left {
            if let Some(k) =
                (0..free.len()).min_by_key(|&k| (b.notes[free[k]].pitch - a.notes[i].pitch).abs())
            {
                fate[i] = Fate::Changed(free.remove(k));
            }
        }
        added.extend(free);
    }
    (fate, added)
}

/// The note of `role` struck at `beat` on `pitch`.
fn find(s: &Score, role: Role, beat: f64, pitch: i32) -> Option<usize> {
    s.notes
        .iter()
        .position(|n| n.role == role && n.start_beat == beat && n.pitch == pitch)
}

fn event(s: &Score, i: usize, d: &MassDiagnostics) -> String {
    let n = &s.notes[i];
    let r = &d.rows[i];
    format!(
        "{} {:.2} beats, audible {:.3} s, mass {:.3} {:?}{}, ownership {:?}, verdict {:?}",
        note_name(n.pitch),
        n.dur_beats,
        r.mass.audible_secs,
        r.mass.mass,
        r.mass.class,
        if r.mass.fused { " (fused)" } else { "" },
        r.ownership,
        r.verdict
    )
}

fn render(dir: &Path, stem: &str, score: &Score, world: &MusicWorld) -> std::io::Result<String> {
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
    let out = arg("--out=").unwrap_or_else(|| "target/humanmusic-r13/baseline".into());
    let radius: f64 = arg("--radius=").map_or(1.5, |r| r.parse().expect("--radius=seconds"));
    let around: Vec<f64> = args
        .iter()
        .filter_map(|a| a.strip_prefix("--around-seconds="))
        .map(|s| s.parse().expect("--around-seconds=seconds"))
        .collect();
    let compare = args.iter().any(|a| a == "--compare");
    let do_render = args.iter().any(|a| a == "--render");
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
        std::fs::write(
            dir.join(format!("{regime}.trajectory.txt")),
            PhenomenalTrajectory::observe(&song).report(),
        )?;
        for (name, world) in [
            ("swiss", MusicWorld::swiss_signal()),
            ("black_ice", MusicWorld::black_ice()),
        ] {
            let a = perform_temporal(&song, &world, opts);
            if let Some(f) = FROZEN.iter().find(|f| f.0 == regime && f.1 == name) {
                assert_eq!(a.score.fingerprint(), f.2, "not the frozen Round XII score");
            }
            let da = MassDiagnostics::measure(&a.perf, &a.score, &world);
            let header = |arm: &str, s: &Score, p: u64| {
                format!(
                    "regime={regime} world={name} seed=2112 beats=120 tempo={} arm={arm}\nsong={:#018x} performance={p:#018x} score={:#018x}\n",
                    s.tempo_bpm,
                    song.fingerprint(),
                    s.fingerprint()
                )
            };
            let ha = header("r12-temporal", &a.score, a.perf.fingerprint());
            std::fs::write(
                dir.join(format!("{name}_{regime}_r12.mass.txt")),
                format!("{ha}{}", da.report()),
            )?;
            let summary = |d: &MassDiagnostics| {
                format!(
                    "unowned_asserted_colors={} accidental_reharmonizations={} overdrawn_gestures={} soft_orphans={} asserted_events={}",
                    d.unowned_asserted_colors,
                    d.accidental_reharmonizations,
                    d.overdrawn_gestures,
                    d.soft_orphans,
                    d.asserted_events
                )
            };
            println!("{name}/{regime} r12: {}", summary(&da));
            if regime == "stable" && !compare {
                for &s in &around {
                    let text = da.around_seconds(&a.score, s, radius);
                    std::fs::write(
                        dir.join(format!("{name}_stable_r12.around-{s}s.txt")),
                        format!("{ha}{text}"),
                    )?;
                    println!("--- {name} around {s} s\n{text}");
                }
            }
            if !compare {
                continue;
            }
            let b = perform_mass(&song, &world, opts);
            assert_eq!(a.perf.fingerprint(), b.perf.fingerprint(), "plan moved");
            assert_eq!(
                format!("{:?}", a.score.drums),
                format!("{:?}", b.score.drums)
            );
            assert_eq!(format!("{:?}", a.score.sfx), format!("{:?}", b.score.sfx));
            assert_eq!(
                format!("{:?}", a.score.chords),
                format!("{:?}", b.score.chords)
            );
            let law = SongMapConformance::check(&song, &b.perf, &b.score);
            assert!(law.passes(), "{}", law.report());
            let db = MassDiagnostics::measure(&b.perf, &b.score, &world);
            let base = &db.base;
            assert_eq!(
                base.false_function_claims
                    + base.false_suspensions
                    + base.broken_anticipations
                    + base.bad_arrivals
                    + base.unresolved_tendencies,
                0,
                "{}",
                base.report()
            );
            // Route 1: the gate's own decisions, replayed through the same support realization.
            let lead = melody::realize_lead_temporal(&b.perf, &song.plan);
            let mut keys =
                comp::realize_keys_temporal(&b.perf, &song.plan, &world, &lead.notes, song.seed);
            let mut pad = comp::realize_pad(&b.perf, &song.plan, &world);
            let edits = comp::gate_support_mass(&b.perf, &world, &mut pad, &mut keys);
            // Route 2: an independent diff of the two finished scores.
            let (fate, added) = align(&a.score, &b.score);
            let (mut pitch, mut dur, mut omitted) = (0, 0, 0);
            for (i, f) in fate.iter().enumerate() {
                let n = &a.score.notes[i];
                match *f {
                    Fate::Same(_) => continue,
                    Fate::Changed(j) => {
                        pitch += usize::from(b.score.notes[j].pitch != n.pitch);
                        dur += usize::from(b.score.notes[j].dur_beats != n.dur_beats);
                    }
                    Fate::Omitted => omitted += 1,
                }
                assert!(
                    matches!(n.role, Role::Pad | Role::Keys)
                        && matches!(n.prov.role_note, "pad" | "comp" | "hold"),
                    "only support voicings may move: {n:?}"
                );
            }
            assert!(added.is_empty(), "the gate never adds a note: {added:?}");
            let count =
                |f: fn(&SupportMassAction) -> bool| edits.iter().filter(|e| f(&e.action)).count();
            assert_eq!(omitted, count(|a| *a == SupportMassAction::Omitted));
            assert_eq!(pitch, count(|a| matches!(a, SupportMassAction::Moved(_))));
            assert_eq!(dur, count(|a| matches!(a, SupportMassAction::Shortened(_))));
            let spb = 60.0 / f64::from(a.score.tempo_bpm);
            let mut ledger = String::from(
                "beat\tseconds\trole\trole_note\tchord\told\taction\tnew\told_mass\told_class\told_verdict\tnew_mass\tnew_verdict\n",
            );
            for e in &edits {
                let n = &e.note;
                let i = find(&a.score, n.role, n.start_beat, n.pitch).expect("edited note is in A");
                let now = match e.action {
                    SupportMassAction::Omitted => None,
                    SupportMassAction::Moved(q) => find(&b.score, n.role, n.start_beat, q),
                    SupportMassAction::Shortened(_) => {
                        find(&b.score, n.role, n.start_beat, n.pitch)
                    }
                };
                let chord = a
                    .perf
                    .context_at(n.start_beat)
                    .map(|c| c.chord.label())
                    .unwrap_or_default();
                let _ = writeln!(
                    ledger,
                    "{:.4}\t{:.3}\t{}\t{}\t{chord}\t{} {:.2}\t{:?}\t{}\t{:.3}\t{:?}\t{:?}\t{}\t{}",
                    n.start_beat,
                    n.start_beat * spb,
                    n.role.label(),
                    n.prov.role_note,
                    note_name(n.pitch),
                    n.dur_beats,
                    e.action,
                    now.map_or("-".into(), |j| format!(
                        "{} {:.2}",
                        note_name(b.score.notes[j].pitch),
                        b.score.notes[j].dur_beats
                    )),
                    da.rows[i].mass.mass,
                    da.rows[i].mass.class,
                    da.rows[i].verdict,
                    now.map_or("-".into(), |j| format!("{:.3}", db.rows[j].mass.mass)),
                    now.map_or("-".into(), |j| format!("{:?}", db.rows[j].verdict)),
                );
            }
            let total = a.score.notes.len();
            let events = fate.iter().filter(|f| !matches!(f, Fate::Same(_))).count();
            let perturbation = format!(
                "pitched_notes={total} pitch_changed={pitch} duration_changed={dur} omitted={omitted} added={} changed_events={events} affected={:.3}%",
                added.len(),
                100.0 * events as f64 / total as f64
            );
            let hb = header("r13-mass", &b.score, b.perf.fingerprint());
            std::fs::write(
                dir.join(format!("{name}_{regime}_mass.mass.txt")),
                format!("{hb}{}", db.report()),
            )?;
            std::fs::write(
                dir.join(format!("{name}_{regime}.changes.tsv")),
                format!("# {perturbation}\n{ledger}"),
            )?;
            println!(
                "{name}/{regime} mass: {}\n{name}/{regime} {perturbation}\n{}",
                summary(&db),
                law.report()
            );
            if regime == "stable" {
                for &s in &around {
                    let mut text = format!(
                        "{ha}{hb}before -> after, every A note sounding within {radius} s of {s} s\n"
                    );
                    let mut idx: Vec<usize> = (0..a.score.notes.len())
                        .filter(|&i| {
                            let t = a.score.notes[i].start_beat * spb;
                            let r = &da.rows[i].mass;
                            t <= s + radius && t + r.audible_secs.max(r.written_secs) >= s - radius
                        })
                        .collect();
                    idx.sort_by(|&x, &y| {
                        a.score.notes[x]
                            .start_beat
                            .total_cmp(&a.score.notes[y].start_beat)
                    });
                    for i in idx {
                        let n = &a.score.notes[i];
                        let edit = edits.iter().find(|e| {
                            e.note.role == n.role
                                && e.note.start_beat == n.start_beat
                                && e.note.pitch == n.pitch
                        });
                        let after = match edit.map(|e| e.action) {
                            None => find(&b.score, n.role, n.start_beat, n.pitch)
                                .map_or("unchanged".into(), |j| {
                                    format!("unchanged: {}", event(&b.score, j, &db))
                                }),
                            Some(SupportMassAction::Omitted) => "OMITTED".into(),
                            Some(SupportMassAction::Moved(q)) => format!(
                                "MOVED to {}",
                                find(&b.score, n.role, n.start_beat, q)
                                    .map_or("?".into(), |j| event(&b.score, j, &db))
                            ),
                            Some(SupportMassAction::Shortened(d)) => format!(
                                "SHORTENED to {d:.2} beats: {}",
                                find(&b.score, n.role, n.start_beat, n.pitch)
                                    .map_or("?".into(), |j| event(&b.score, j, &db))
                            ),
                        };
                        let flag = if da.rows[i].verdict == MassVerdict::Carried {
                            " "
                        } else {
                            "*"
                        };
                        let _ = writeln!(
                            text,
                            "{flag} beat {:.3} ({:.3} s) {} {}\n    before: {}\n    after:  {after}",
                            n.start_beat,
                            n.start_beat * spb,
                            n.role.label(),
                            n.prov.role_note,
                            event(&a.score, i, &da)
                        );
                    }
                    std::fs::write(dir.join(format!("{name}_stable.around-{s}s.txt")), &text)?;
                }
                if do_render {
                    println!("{}", render(dir, &format!("{name}_r12"), &a.score, &world)?);
                    println!(
                        "{}",
                        render(dir, &format!("{name}_mass"), &b.score, &world)?
                    );
                }
            }
        }
    }
    Ok(())
}
