//! Round XIV: what musical object did the listener actually hear?
//!
//! The heard-identity audit ([`identity`]), the gesture audit ([`gesture`]) and the hearing
//! ledger ([`Score::stale_hearings`]) over four arms of one song: Round XII `perform_temporal`
//! (the accepted listening baseline, A), Round XIII `perform_mass`, Round XIIIb
//! `perform_tension` (the rejected intervention), and Round XIV `perform_coherent` (B).
//! StablePropulsion, seed 2112, `deflected_lift_trace(120)`, default options, SWISS_SIGNAL and
//! BLACK_ICE.
//!
//! Writes, per world:
//! - `{world}_{arm}.identity.txt`: every rival run, every flipped slice spelled out;
//! - `{world}_{arm}.gesture.txt`: every note the analysis calls a slide, on its physical route;
//! - `{world}.at-seconds.txt`: every heard pitched event and the identity verdict at the
//!   maintainer's timestamps (15.0–17.0 s and 43.0–45.0 s), for R12, R13b and R14;
//! - `{world}.chart-spans.txt`: every chart span whose pad R14 changed, with both voicings and
//!   the identity each arm gives it; for SWISS, every Cmaj7 span;
//! - `{world}.pad-voicing-edits.tsv`: R14's ledger;
//! - `{world}.perturbation.txt`: an independent A → B diff, checked against the ledger;
//! - `{world}.lines.txt`: every chromatic line gesture in B (declared function, notes, onsets,
//!   durations, attacks, physical gesture, the observer's per-note support);
//! - `{world}.hearings.txt`: the stale hearings of every arm;
//! - `summary.txt`.
//!
//! `--render` writes `{world}_r12.wav` and `{world}_r14.wav` (full mix) and
//! `{world}_{r12,r14}.pad.wav` (the pad alone, the only stem that differs).
//!
//! Run:
//!   cargo run --release --example heard_object_lab -- --render --out=target/humanmusic-r14/final
use gibson::audio::human_music::{
    comp::PadVoicingReason,
    composer::Composer,
    functor::{perform_coherent, perform_mass, perform_temporal, perform_tension, Composition},
    gesture::{classify, GestureDiagnostics, PathEvent},
    identity::IdentityDiagnostics,
    performance::PerformanceOptions,
    score::{Note, PitchFunction, Role, Score},
    semantic::deflected_lift_trace,
    song::SongMap,
    synth::StemMask,
    temporal::TemporalPitchDiagnostics,
    theory::{note_name, Quality},
    MusicWorld,
};
use std::fmt::Write;
use std::path::Path;

fn identity(x: &Composition, world: &MusicWorld) -> IdentityDiagnostics {
    IdentityDiagnostics::measure(&x.score.notes, &x.perf.contexts, world, x.score.tempo_bpm)
}

fn names(v: &[i32]) -> String {
    v.iter()
        .map(|&p| note_name(p))
        .collect::<Vec<_>>()
        .join(" ")
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

/// A note's exact identity for the diff.
type Key = (&'static str, u64, u32, i32);

fn key(n: &Note) -> Key {
    (
        n.role.label(),
        n.start_beat.to_bits(),
        n.dur_beats.to_bits(),
        n.pitch,
    )
}

/// The independent A → B diff: (pitch-changed, duration-changed, omitted, added, lines).
/// A pitch change is a note gone and a note come at the same role, onset and length; a
/// duration change the same role, onset and pitch.
fn diff(a: &Score, b: &Score) -> (usize, usize, usize, usize, Vec<String>) {
    let mut gone: Vec<Key> = a.notes.iter().map(key).collect();
    let mut came: Vec<Key> = b.notes.iter().map(key).collect();
    gone.sort();
    came.sort();
    let (mut g2, mut c2) = (Vec::new(), Vec::new());
    let (mut i, mut j) = (0, 0);
    while i < gone.len() || j < came.len() {
        match (gone.get(i), came.get(j)) {
            (Some(x), Some(y)) if x == y => {
                i += 1;
                j += 1;
            }
            (Some(x), Some(y)) if x < y => {
                g2.push(*x);
                i += 1;
            }
            (Some(_), Some(y)) => {
                c2.push(*y);
                j += 1;
            }
            (Some(x), None) => {
                g2.push(*x);
                i += 1;
            }
            (None, Some(y)) => {
                c2.push(*y);
                j += 1;
            }
            (None, None) => break,
        }
    }
    let mut lines = Vec::new();
    let (mut pitch, mut dur) = (0, 0);
    let take = |g2: &mut Vec<Key>, c2: &mut Vec<Key>, same: fn(&Key, &Key) -> bool| {
        let mut k = 0;
        let mut out = Vec::new();
        while k < g2.len() {
            if let Some(m) = c2.iter().position(|y| same(&g2[k], y)) {
                out.push((g2.remove(k), c2.remove(m)));
            } else {
                k += 1;
            }
        }
        out
    };
    for (x, y) in take(&mut g2, &mut c2, |x, y| {
        x.0 == y.0 && x.1 == y.1 && x.2 == y.2
    }) {
        pitch += 1;
        lines.push(format!(
            "pitch    {:<4} beat {:>7.3}  {} -> {}",
            x.0,
            f64::from_bits(x.1),
            note_name(x.3),
            note_name(y.3)
        ));
    }
    for (x, y) in take(&mut g2, &mut c2, |x, y| {
        x.0 == y.0 && x.1 == y.1 && x.3 == y.3
    }) {
        dur += 1;
        lines.push(format!(
            "duration {:<4} beat {:>7.3}  {} {} -> {}",
            x.0,
            f64::from_bits(x.1),
            note_name(x.3),
            f32::from_bits(x.2),
            f32::from_bits(y.2)
        ));
    }
    for x in &g2 {
        lines.push(format!(
            "omitted  {:<4} beat {:>7.3}  {}",
            x.0,
            f64::from_bits(x.1),
            note_name(x.3)
        ));
    }
    for y in &c2 {
        lines.push(format!(
            "added    {:<4} beat {:>7.3}  {}",
            y.0,
            f64::from_bits(y.1),
            note_name(y.3)
        ));
    }
    (pitch, dur, g2.len(), c2.len(), lines)
}

/// Every chromatic line gesture of `x`: each note declared a connective or tendency function in
/// the bass or the lead, on the route to its next note, with the observer's per-note support.
fn lines(x: &Composition) -> String {
    let s = &x.score;
    let spb = 60.0 / f64::from(s.tempo_bpm);
    let audit = TemporalPitchDiagnostics::measure(&x.perf, s);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "every bass/lead note declared ChromaticApproach, ChromaticPassing, Neighbor or Enclosure, with its next note in the same role\n\
         attacks: every Note is a fresh SynthVoice::trigger (new frequency, envelopes gated on, filter reset); nothing glides, ties or bends\n"
    );
    for role in [Role::Bass, Role::Lead] {
        let ix: Vec<usize> = (0..s.notes.len())
            .filter(|&i| s.notes[i].role == role)
            .collect();
        for (k, &i) in ix.iter().enumerate() {
            let n = &s.notes[i];
            let Some(f) = n.function.filter(|f| {
                matches!(
                    f,
                    PitchFunction::ChromaticApproach
                        | PitchFunction::ChromaticPassing
                        | PitchFunction::Neighbor
                        | PitchFunction::Enclosure
                )
            }) else {
                continue;
            };
            let Some(&j) = ix.get(k + 1) else {
                continue;
            };
            let t = &s.notes[j];
            let route = [PathEvent::of(n), PathEvent::of(t)];
            let sup = |q: usize| {
                audit
                    .rows
                    .iter()
                    .find(|r| r.note_index == q)
                    .map_or("-".to_string(), |r| {
                        r.supported
                            .iter()
                            .map(|f| f.label())
                            .collect::<Vec<_>>()
                            .join("+")
                    })
            };
            let chart = |b: f64| x.perf.context_at(b).map_or("-".into(), |c| c.chord.label());
            let _ = writeln!(
                out,
                "{:<4} declared {:<18} {} -> {}  onsets {:.3} / {:.3} b ({:.3} / {:.3} s)  IOI {:.3} b = {:.3} s  dur {:.3} / {:.3} b ({:.3} / {:.3} s)  attacks 2/2  physical {}  chart {} -> {}  support [{}] -> [{}]",
                role.label(),
                f.label(),
                note_name(n.pitch),
                note_name(t.pitch),
                n.start_beat,
                t.start_beat,
                n.start_beat * spb,
                t.start_beat * spb,
                t.start_beat - n.start_beat,
                (t.start_beat - n.start_beat) * spb,
                n.dur_beats,
                t.dur_beats,
                f64::from(n.dur_beats) * spb,
                f64::from(t.dur_beats) * spb,
                classify(&route).map_or("leap/turn (not a stepwise route)", |g| g.label()),
                chart(n.start_beat),
                chart(t.start_beat),
                sup(i),
                sup(j)
            );
        }
    }
    out
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .iter()
        .find_map(|a| a.strip_prefix("--out="))
        .unwrap_or("target/humanmusic-r14")
        .to_string();
    let do_render = args.iter().any(|a| a == "--render");
    let dir = Path::new(&out);
    std::fs::create_dir_all(dir)?;
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let opts = PerformanceOptions::default();
    let mut summary = String::new();
    let mut renders = String::new();
    for (name, world) in [
        ("swiss", MusicWorld::swiss_signal()),
        ("black_ice", MusicWorld::black_ice()),
    ] {
        let arms = [
            ("r12", perform_temporal(&song, &world, opts)),
            ("r13", perform_mass(&song, &world, opts)),
            ("r13b", perform_tension(&song, &world, opts)),
            ("r14", perform_coherent(&song, &world, opts)),
        ];
        let ids: Vec<IdentityDiagnostics> = arms.iter().map(|(_, x)| identity(x, &world)).collect();
        let mut hearings = String::new();
        for ((arm, x), id) in arms.iter().zip(&ids) {
            let g = GestureDiagnostics::measure(&x.perf, &x.score);
            let flips: Vec<String> = id
                .flips()
                .map(|r| {
                    format!(
                        "{:.2}-{:.2} s {} heard as {} ({:.3} s)",
                        id.secs(r.start_beat),
                        id.secs(r.end_beat),
                        r.chart.label(),
                        r.rival.label(),
                        r.secs
                    )
                })
                .collect();
            let stale = x.score.stale_hearings();
            let line = format!(
                "{name:<9} {arm:<4} notes {} | identity flips {} ({:.3} s): [{}]\n{name:<9} {arm:<4} {}\n{name:<9} {arm:<4} stale hearings {}: [{}]\n",
                x.score.notes.len(),
                flips.len(),
                id.flipped_secs(),
                flips.join("; "),
                g.report(&x.score).lines().next().unwrap_or(""),
                stale.len(),
                stale
                    .iter()
                    .map(|h| format!(
                        "{} heard {} notes of the {} not in the final score",
                        h.listener,
                        h.missing.len(),
                        h.source.label()
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
            print!("{line}");
            summary.push_str(&line);
            std::fs::write(
                dir.join(format!("{name}_{arm}.identity.txt")),
                id.report(&x.score.notes, &x.perf.contexts),
            )?;
            std::fs::write(
                dir.join(format!("{name}_{arm}.gesture.txt")),
                g.report(&x.score),
            )?;
            let _ = writeln!(
                hearings,
                "{arm}: {} hearings recorded",
                x.score.hearings.len()
            );
            for h in &x.score.hearings {
                let _ = writeln!(
                    hearings,
                    "    {} <- {} ({} notes)",
                    h.listener,
                    h.source.label(),
                    h.notes.len()
                );
            }
            for h in &stale {
                let _ = writeln!(
                    hearings,
                    "  STALE {} <- {}: {} heard notes not in the final score, {} final notes never heard",
                    h.listener,
                    h.source.label(),
                    h.missing.len(),
                    h.unheard.len()
                );
                for m in &h.missing {
                    let _ = writeln!(
                        hearings,
                        "      heard {} at {:.3} for {:.3} ({:?})",
                        note_name(m.2),
                        m.0,
                        m.1,
                        m.3
                    );
                }
            }
        }
        std::fs::write(dir.join(format!("{name}.hearings.txt")), hearings)?;
        let (a, b) = (&arms[0].1, &arms[3].1);
        let (ia, ib) = (&ids[0], &ids[3]);

        // The maintainer's timestamps.
        let mut at = String::new();
        for secs in [15.0, 15.5, 16.0, 16.5, 17.0, 43.0, 43.5, 44.0, 44.5, 45.0] {
            let _ = writeln!(at, "t = {secs:.1} s");
            for (k, arm) in [(0, "r12 "), (2, "r13b"), (3, "r14 ")] {
                let x = &arms[k].1;
                let line = ids[k]
                    .at_beat(secs * f64::from(x.score.tempo_bpm) / 60.0)
                    .map_or("silence".to_string(), |s| {
                        ids[k].line(&x.score.notes, &x.perf.contexts, s)
                    });
                let _ = writeln!(at, "  {arm} {line}");
            }
        }
        std::fs::write(dir.join(format!("{name}.at-seconds.txt")), at)?;

        // Chart spans: every span whose pad R14 changed; for SWISS every Cmaj7 span too.
        let pad_at = |x: &Composition, beat: f64| -> Vec<i32> {
            let mut v: Vec<i32> = x
                .score
                .role_notes(Role::Pad)
                .filter(|n| {
                    (n.start_beat - beat).abs() < 1e-6
                        || (n.start_beat > beat && n.start_beat < beat + 1.0 + 1e-6)
                })
                .map(|n| n.pitch)
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        let mut spans = String::new();
        for (ci, c) in b.perf.contexts.iter().enumerate() {
            let edited = b.score.pad_voicing_edits.iter().any(|e| e.context == ci);
            let cmaj7 = name == "swiss" && c.chord.root_pc == 0 && c.chord.quality == Quality::Maj7;
            if !edited && !cmaj7 {
                continue;
            }
            let worst = |id: &IdentityDiagnostics| {
                id.slices
                    .iter()
                    .filter(|s| s.context == ci)
                    .map(|s| s.status)
                    .min()
                    .map_or("silent".to_string(), |s| s.label().to_string())
            };
            let _ = writeln!(
                spans,
                "beat {:>6.2} ({:>6.3} s) {:<6} pad r12 [{}] r14 [{}]  worst heard identity: r12 {} | r13b {} | r14 {}{}",
                c.start_beat,
                ia.secs(c.start_beat),
                c.chord.label(),
                names(&pad_at(a, c.start_beat)),
                names(&pad_at(b, c.start_beat)),
                worst(ia),
                worst(&ids[2]),
                worst(ib),
                if edited { "  (edited)" } else { "" }
            );
        }
        std::fs::write(dir.join(format!("{name}.chart-spans.txt")), spans)?;

        // The ledger.
        let mut tsv = String::from("context\tbeat\tseconds\tchart\tbefore\tafter\treason\n");
        for e in &b.score.pad_voicing_edits {
            let reason = match &e.reason {
                PadVoicingReason::Spacing { from, to } => {
                    format!("spacing {} -> {}", note_name(*from), note_name(*to))
                }
                PadVoicingReason::Rooting {
                    rival,
                    replaced,
                    root,
                } => format!(
                    "rooting: heard as {}; root {} in place of {}",
                    rival.label(),
                    note_name(*root),
                    replaced.map_or("nothing (added)".to_string(), note_name)
                ),
                PadVoicingReason::Release { rival } => format!(
                    "release: heard as {}; the pad's incoming tails let go",
                    rival.label()
                ),
            };
            let _ = writeln!(
                tsv,
                "{}\t{:.3}\t{:.3}\t{}\t{}\t{}\t{}",
                e.context,
                e.start_beat,
                ib.secs(e.start_beat),
                b.perf.contexts[e.context].chord.label(),
                names(&e.before),
                names(&e.after),
                reason
            );
        }
        std::fs::write(dir.join(format!("{name}.pad-voicing-edits.tsv")), tsv)?;

        // The independent perturbation receipt, checked against the ledger.
        let (pitch, dur, omitted, added, dl) = diff(&a.score, &b.score);
        let pitched = a.score.notes.len();
        let voicings: std::collections::BTreeSet<u64> = dl
            .iter()
            .filter(|l| l.starts_with("pitch"))
            .filter_map(|l| {
                l.split_whitespace()
                    .nth(3)
                    .and_then(|b| b.parse::<f64>().ok())
                    .map(f64::to_bits)
            })
            .collect();
        let edited_voicings: std::collections::BTreeSet<u64> = b
            .score
            .pad_voicing_edits
            .iter()
            .map(|e| e.start_beat.to_bits())
            .collect();
        assert_eq!(dur + omitted + added, 0, "{name}: R14 changes pitches only");
        assert_eq!(
            pitch,
            b.score.pad_voicing_edits.len(),
            "{name}: diff != ledger"
        );
        assert_eq!(voicings, edited_voicings, "{name}: diff voicings != ledger");
        assert!(
            dl.iter().all(|l| l.contains(" pad ")),
            "{name}: only the pad moves"
        );
        let mut p = format!(
            "Round XII (A) -> Round XIV (B), {name}, independent diff of the two scores\n\
             pitched events (A): {pitched}\n\
             notes pitch-changed: {pitch}\n\
             notes duration-changed: {dur}\n\
             notes omitted: {omitted}\n\
             notes added: {added}\n\
             note count: {} -> {}\n\
             total changed events: {}\n\
             percentage of pitched events affected: {:.3}%\n\
             pad voicings changed: {} (ledger: {} edits over {} voicings)\n\
             lead, keys, bass: identical; drums identical: {}; sfx identical: {}\n",
            a.score.notes.len(),
            b.score.notes.len(),
            pitch + dur + omitted + added,
            100.0 * (pitch + dur + omitted + added) as f64 / pitched as f64,
            voicings.len(),
            b.score.pad_voicing_edits.len(),
            edited_voicings.len(),
            format!("{:?}", a.score.drums) == format!("{:?}", b.score.drums),
            format!("{:?}", a.score.sfx) == format!("{:?}", b.score.sfx),
        );
        for l in &dl {
            let _ = writeln!(p, "{l}");
        }
        std::fs::write(dir.join(format!("{name}.perturbation.txt")), p)?;
        std::fs::write(dir.join(format!("{name}.lines.txt")), lines(b))?;

        if do_render {
            for (arm, x) in [("r12", a), ("r14", b)] {
                let full = render(
                    dir,
                    &format!("{name}_{arm}"),
                    &x.score,
                    &world,
                    StemMask::full(),
                )?;
                let pad = render(
                    dir,
                    &format!("{name}_{arm}.pad"),
                    &x.score,
                    &world,
                    StemMask {
                        pad: true,
                        ..StemMask::silent()
                    },
                )?;
                let _ = writeln!(
                    renders,
                    "{name} {arm} score=0x{:016x} {full}",
                    x.score.fingerprint()
                );
                let _ = writeln!(renders, "{name} {arm} pad-stem {pad}");
            }
        }
    }
    std::fs::write(dir.join("summary.txt"), summary)?;
    if do_render {
        print!("{renders}");
        std::fs::write(dir.join("render.txt"), renders)?;
    }
    Ok(())
}
