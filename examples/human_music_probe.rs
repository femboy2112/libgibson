//! HumanMusic analysis probe for the beef-up round (promoted from the throwaway `gap_dump`).
//!
//! Dumps HumanMusic's composed `Score` — pitched notes + drum hits, WITH velocity (which the
//! shipped `human_music_lab --dump-notes` omits) AND the semantic provenance the composer already
//! stamps on every event — to a unified TSV, so the Python rig in `scripts/humanmusic_beefup/`
//! measures the engine with the same instrument as the reference corpus.
//!
//! Row schema (tab-separated, one header line):
//!   source voice kind start_beat dur_beats pitch velocity family phrase anchor motif_id motif_xform drole func
//! where
//!   kind    ∈ {note, drum};  pitch is MIDI (drums: -1);  velocity is HumanMusic's raw f32
//!   voice   = semantic role (pad/keys/bass/lead) for notes, drum lane for drums
//!   family  = section-family label (A / A' / B / climax …);  phrase = FormGraph phrase index
//!   anchor  = coherence anchor realized (motif / groove / riff …)
//!   motif_id / motif_xform = stable motif identity + transformation-chain label (melodic only)
//!   drole   = phrase discourse role (establish / culminate / answer …);  func = pitch function
//! Empty string = the composer left that field unset (NOT fabricated).
//!
//! NOTE on timing: this dumps the COMPOSED score, whose positions are METRIC (beat) coordinates.
//! Performed micro-timing (the `FeelTransport` seam) is applied downstream of compose and is
//! `Identity` today, so `start_beat` is both the metric and the performed coordinate for now; a
//! performed-position probe will be needed once coordinated feel lands (Wave 4).
//!
//!   cargo run --example human_music_probe -- --out=/tmp/hm --beats=96 --seed=2112

use std::fmt::Write as _;

use gibson::audio::human_music::score::{DrumVoice, Note, Provenance, Score};
use gibson::audio::human_music::semantic::{
    deflected_lift_trace, demo_trace, false_climax, SemanticTrace,
};
use gibson::audio::human_music::{compose, MusicWorld, WorldId};

/// A named story-trace constructor (beats -> trace). Aliased to keep clippy's type-complexity happy.
type Story = (&'static str, fn(f64) -> SemanticTrace);

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

fn os(o: Option<&'static str>) -> &'static str {
    o.unwrap_or("")
}
fn ou32(o: Option<u32>) -> String {
    o.map(|v| v.to_string()).unwrap_or_default()
}
fn ou8(o: Option<u8>) -> String {
    o.map(|v| v.to_string()).unwrap_or_default()
}

/// The 8 provenance columns shared by notes and drums.
fn prov_cols(p: &Provenance) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}",
        os(p.family),
        ou32(p.phrase),
        os(p.anchor),
        ou8(p.motif_id),
        os(p.motif_xform),
        os(p.role),
    )
}

fn note_row(source: &str, n: &Note) -> String {
    let func = n.function.map(|f| f.label()).unwrap_or("");
    format!(
        "{source}\t{}\tnote\t{:.4}\t{:.4}\t{}\t{:.4}\t{}\t{}\n",
        n.role.label(),
        n.start_beat,
        n.dur_beats,
        n.pitch,
        n.velocity,
        prov_cols(&n.prov),
        func,
    )
}

fn dump(source: &str, score: &Score) -> String {
    let mut s = String::new();
    for n in &score.notes {
        s.push_str(&note_row(source, n));
    }
    for d in &score.drums {
        // drums carry no pitch/motif/function; fill those columns empty, keep provenance.
        let _ = writeln!(
            s,
            "{source}\t{}\tdrum\t{:.4}\t0.0000\t-1\t{:.4}\t{}\t",
            drum_label(d.voice),
            d.start_beat,
            d.velocity,
            prov_cols(&d.prov),
        );
    }
    s
}

fn main() -> std::io::Result<()> {
    let seed: u64 = arg("--seed=").and_then(|s| s.parse().ok()).unwrap_or(2112);
    let beats: f64 = arg("--beats=").and_then(|s| s.parse().ok()).unwrap_or(96.0);
    let out_dir = std::path::PathBuf::from(arg("--out=").unwrap_or_else(|| "hm_probe_out".into()));
    std::fs::create_dir_all(&out_dir)?;

    let worlds = [
        ("vapor95", WorldId::Vapor95),
        ("black_ice", WorldId::BlackIce),
        ("swiss_signal", WorldId::SwissSignal),
    ];
    let stories: [Story; 3] = [
        ("demo", demo_trace),
        ("false_climax", false_climax),
        ("deflected", deflected_lift_trace),
    ];

    let header = "source\tvoice\tkind\tstart_beat\tdur_beats\tpitch\tvelocity\t\
                  family\tphrase\tanchor\tmotif_id\tmotif_xform\tdrole\tfunc\n";
    let mut combined = String::from(header);

    for (wname, wid) in worlds {
        let world = MusicWorld::from_id(wid);
        for (sname, make) in stories {
            let trace = make(beats);
            let score = compose(&trace, &world, seed);
            let source = format!("hm:{wname}:{sname}");
            let body = dump(&source, &score);
            std::fs::write(
                out_dir.join(format!("{wname}.{sname}.notes.tsv")),
                format!("{header}{body}"),
            )?;
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
