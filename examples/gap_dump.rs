//! SCRATCH instrumentation for the "HumanMusic vs GOOD_MIDI" note-quality gap analysis.
//!
//! Dumps HumanMusic's NOTE output (pitched notes + drum hits, WITH velocity — the one thing
//! the shipped `human_music_lab --dump-notes` omits) to a unified TSV schema that matches the
//! MIDI-side extractor, so both corpora are measured by the same instrument.
//!
//! Unified row schema (tab-separated, one header line):
//!   source  voice  kind  start_beat  dur_beats  pitch  velocity
//! where kind ∈ {note, drum}; pitch is MIDI (drums: -1); velocity is HumanMusic's raw f32.
//!
//!   cargo run --example gap_dump -- --out=/tmp/hm --beats=96 --seed=2112
//!
//! This file is throwaway analysis tooling; remove it (and its Cargo.toml entry) when done.

use std::fmt::Write as _;

use gibson::audio::human_music::score::{DrumVoice, Score};
use gibson::audio::human_music::semantic::{deflected_lift_trace, demo_trace, false_climax};
use gibson::audio::human_music::{compose, MusicWorld, WorldId};

fn arg(prefix: &str) -> Option<String> {
    std::env::args()
        .find(|a| a.starts_with(prefix))
        .map(|a| a[prefix.len()..].to_string())
}

fn drum_label(v: DrumVoice) -> &'static str {
    match v {
        DrumVoice::Kick => "kick",
        DrumVoice::Snare => "snare",
        DrumVoice::ClosedHat => "chat",
        DrumVoice::OpenHat => "ohat",
        DrumVoice::Clap => "clap",
    }
}

fn dump(source: &str, score: &Score) -> String {
    let mut s = String::new();
    for n in &score.notes {
        let _ = writeln!(
            s,
            "{source}\t{}\tnote\t{:.4}\t{:.4}\t{}\t{:.4}",
            n.role.label(),
            n.start_beat,
            n.dur_beats,
            n.pitch,
            n.velocity,
        );
    }
    for d in &score.drums {
        let _ = writeln!(
            s,
            "{source}\t{}\tdrum\t{:.4}\t0.0000\t-1\t{:.4}",
            drum_label(d.voice),
            d.start_beat,
            d.velocity,
        );
    }
    s
}

fn main() -> std::io::Result<()> {
    let seed: u64 = arg("--seed=").and_then(|s| s.parse().ok()).unwrap_or(2112);
    let beats: f64 = arg("--beats=").and_then(|s| s.parse().ok()).unwrap_or(96.0);
    let out_dir = std::path::PathBuf::from(arg("--out=").unwrap_or_else(|| "gap_out".into()));
    std::fs::create_dir_all(&out_dir)?;

    let worlds = [
        ("vapor95", WorldId::Vapor95),
        ("black_ice", WorldId::BlackIce),
        ("swiss_signal", WorldId::SwissSignal),
    ];
    let stories: [(&str, fn(f64) -> _); 3] = [
        ("demo", demo_trace),
        ("false_climax", false_climax),
        ("deflected", deflected_lift_trace),
    ];

    let header = "source\tvoice\tkind\tstart_beat\tdur_beats\tpitch\tvelocity\n";
    let mut combined = String::from(header);

    for (wname, wid) in worlds {
        let world = MusicWorld::from_id(wid);
        for (sname, make) in stories {
            let trace = make(beats);
            let score = compose(&trace, &world, seed);
            let source = format!("hm:{wname}:{sname}");
            let body = dump(&source, &score);
            let per = format!("{header}{body}");
            std::fs::write(out_dir.join(format!("{wname}.{sname}.notes.tsv")), &per)?;
            combined.push_str(&body);
            eprintln!(
                "{source}: {} notes, {} drums, {:.1} beats @ {:.0} bpm",
                score.notes.len(),
                score.drums.len(),
                score.total_beats,
                score.tempo_bpm,
            );
        }
    }
    std::fs::write(out_dir.join("ALL.notes.tsv"), &combined)?;
    eprintln!("wrote {}", out_dir.join("ALL.notes.tsv").display());
    Ok(())
}
