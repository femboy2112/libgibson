//! C137 "fusion" REJECTION-gate instrument. Renders `MusicalArgument::fusion` through the
//! production path (`compile` then `perform_argument`, BAND) and measures the nine calibrated
//! gates R1..R9 against the resulting `Score`.
//!
//! EAR = ORACLE. These gates can only REJECT. A pass means "not auto-disqualified" -- NEVER
//! "sounds good". Calibration proof that they cannot certify: the deliberately-wrong negative
//! control `band_scrambled` passes all nine. Floors are fractions of our own hand-authored
//! teacher (`rick_probe` / `band_story_probe`); no third-party material is read or encoded.
//!
//! The definitions below are a line-for-line port of the calibration `py/gates.py`
//! (per-role / per-bar / per-16-beat-window, so tempo and song length cancel).
//!
//! Run: `cargo run --release --example fusion_gates -- [--seed=2112] [--seeds=2112,7,99]
//!      [--tempo=108] [--beats=128] [--table-only] [--dump-tsv=DIR]
//!      or: --tsv=FILE   (cross-check a calibration-format dump; no render)`
//! Exit code is non-zero only when the render pipeline errors; a REJECT is a measurement, not a crash.

use gibson::audio::human_music::{
    argument::MusicalArgument,
    composer::Composer,
    contract::CompositionGrammar,
    functor::perform_argument,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    score::{Role, Score},
    semantic::deflected_lift_trace,
    synth::StemMask,
    theory::Mode,
    HumanMusicSynth, MusicWorld, SongMap,
};
use gibson::audio::{wav::write_wav_i16, OfflineRenderer, SampleRate};
use std::collections::{BTreeSet, HashMap, HashSet};

const BPB: f64 = 4.0;

/// The calibration dumps wrote f32 fields as shortest-decimal text that Python re-read as f64.
/// Mirror that so the arithmetic matches the calibrated gates bit-for-bit.
fn widen(x: f32) -> f64 {
    format!("{x}").parse().unwrap_or(f64::from(x))
}

/// Python `round(x, nd)` for reporting and the rounded comparisons the gates make.
fn round_to(x: f64, nd: i32) -> f64 {
    let m = 10f64.powi(nd);
    (x * m).round() / m
}

/// Length of the union of `[a, b)` intervals.
fn union_len(iv: &[(f64, f64)]) -> f64 {
    let mut v = iv.to_vec();
    v.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let mut tot = 0.0;
    let mut cur: Option<(f64, f64)> = None;
    for (a, b) in v {
        match cur {
            None => cur = Some((a, b)),
            Some((ca, cb)) if a <= cb => cur = Some((ca, cb.max(b))),
            Some((ca, cb)) => {
                tot += cb - ca;
                cur = Some((a, b));
            }
        }
    }
    if let Some((ca, cb)) = cur {
        tot += cb - ca;
    }
    tot
}

/// Python `difflib.SequenceMatcher(None, a, b).ratio()` (no junk; autojunk only bites at n>=200,
/// far beyond a 16-beat lead window).
fn seq_ratio(a: &[i32], b: &[i32]) -> f64 {
    fn longest(
        a: &[i32],
        b: &[i32],
        alo: usize,
        ahi: usize,
        blo: usize,
        bhi: usize,
    ) -> (usize, usize, usize) {
        let (mut bi, mut bj, mut bk) = (alo, blo, 0usize);
        let mut prev = vec![0usize; b.len() + 1];
        for (i, ai) in a.iter().enumerate().take(ahi).skip(alo) {
            let mut cur = vec![0usize; b.len() + 1];
            for j in blo..bhi {
                if *ai == b[j] {
                    let k = prev[j] + 1;
                    cur[j + 1] = k;
                    if k > bk {
                        bi = i + 1 - k;
                        bj = j + 1 - k;
                        bk = k;
                    }
                }
            }
            prev = cur;
        }
        (bi, bj, bk)
    }
    let mut matched = 0usize;
    let mut stack = vec![(0usize, a.len(), 0usize, b.len())];
    while let Some((alo, ahi, blo, bhi)) = stack.pop() {
        if alo >= ahi || blo >= bhi {
            continue;
        }
        let (i, j, k) = longest(a, b, alo, ahi, blo, bhi);
        if k == 0 {
            continue;
        }
        matched += k;
        stack.push((alo, i, blo, j));
        stack.push((i + k, ahi, j + k, bhi));
    }
    let t = a.len() + b.len();
    if t == 0 {
        1.0
    } else {
        2.0 * matched as f64 / t as f64
    }
}

struct Gate {
    name: &'static str,
    measured: String,
    floor: &'static str,
    pass: bool,
}

/// Role-tagged pitched note, widened exactly as the calibration dumps were (f32 text -> f64).
struct PNote {
    role: Role,
    t: f64,
    d: f64,
    p: i32,
    v: f64,
}

/// Everything the gates read: pitched notes, chord symbols, total beats. Built from a live `Score`
/// or (cross-check mode) from a calibration-format TSV dump.
struct Input {
    notes: Vec<PNote>,
    chords: Vec<(f64, f64, String)>,
    /// Per-span chord tones (pitch classes) — the live-render path only; empty on the TSV path.
    /// Carries what R-ROOT needs: is a strong-beat lead note a chord tone of the chord under it.
    chord_pcs: Vec<(f64, f64, Vec<i32>)>,
    total: f64,
}

impl Input {
    fn from_score(score: &Score) -> Input {
        Input {
            notes: score
                .notes
                .iter()
                .map(|n| PNote {
                    role: n.role,
                    t: n.start_beat,
                    d: widen(n.dur_beats),
                    p: n.pitch,
                    v: widen(n.velocity),
                })
                .collect(),
            chords: score
                .chords
                .iter()
                .map(|c| (c.start_beat, widen(c.dur_beats), c.chord.label()))
                .collect(),
            chord_pcs: score
                .chords
                .iter()
                .map(|c| (c.start_beat, widen(c.dur_beats), c.chord.pitch_classes()))
                .collect(),
            total: score.total_beats,
        }
    }

    /// Parse a calibration-format TSV (`T`/`N`/`C` lines). Chord symbol = root pc + quality text,
    /// which is all the gates need (distinctness only).
    fn from_tsv(text: &str) -> Result<Input, Box<dyn std::error::Error>> {
        let mut inp = Input {
            notes: vec![],
            chords: vec![],
            chord_pcs: vec![],
            total: 0.0,
        };
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            match f.first().copied() {
                Some("T") => inp.total = f[3].parse()?,
                Some("N") => {
                    let role = match f[1] {
                        "lead" => Role::Lead,
                        "bass" => Role::Bass,
                        "keys" => Role::Keys,
                        "pad" => Role::Pad,
                        other => return Err(format!("unknown role {other}").into()),
                    };
                    inp.notes.push(PNote {
                        role,
                        t: f[2].parse()?,
                        d: f[3].parse()?,
                        p: f[4].parse()?,
                        v: f[5].parse()?,
                    });
                }
                Some("C") => {
                    inp.chords
                        .push((f[1].parse()?, f[2].parse()?, format!("{}{}", f[3], f[4])))
                }
                _ => {}
            }
        }
        Ok(inp)
    }
}

struct LeadNote {
    t: f64,
    d: f64,
    p: i32,
    v: f64,
}

/// Evaluate R1..R9 on a rendered Score. Returns the gates in the order R1,R2,R3,R4,R5,R6a,R6b,R6c,
/// R7,R8,R9 (eleven lines, nine gates: R6 is the AND of its three parts).
fn evaluate(inp: &Input) -> Vec<Gate> {
    let total = inp.total;
    let mut lead: Vec<LeadNote> = inp
        .notes
        .iter()
        .filter(|n| n.role == Role::Lead)
        .map(|n| LeadNote {
            t: n.t,
            d: n.d,
            p: n.p,
            v: n.v,
        })
        .collect();
    lead.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap());
    // Drop the terminal tail (final 4 beats), as the calibration does.
    let body: Vec<&LeadNote> = lead.iter().filter(|n| n.t < total - 4.0).collect();
    let mut g: Vec<Gate> = Vec::new();
    if body.is_empty() {
        for name in ["R1", "R2", "R3", "R4", "R5", "R6", "R7", "R8", "R9"] {
            g.push(Gate {
                name,
                measured: "no lead notes before the terminal tail".into(),
                floor: "-",
                pass: false,
            });
        }
        return g;
    }
    let first = body[0].t;
    let last_end = body.iter().map(|n| n.t + n.d).fold(f64::MIN, f64::max);
    let span = last_end - first;
    let iv: Vec<(f64, f64)> = body.iter().map(|n| (n.t, n.t + n.d)).collect();

    // R1 lead onsets/bar over span
    let r1 = round_to(body.len() as f64 / (span / BPB), 2);
    g.push(Gate {
        name: "R1 lead onsets/bar over span",
        measured: format!("{r1}"),
        floor: ">=3.0",
        pass: r1 >= 3.0,
    });
    // R2 sounding coverage
    let r2 = round_to(union_len(&iv) / span, 3);
    g.push(Gate {
        name: "R2 lead sounding coverage of span",
        measured: format!("{r2}"),
        floor: ">=0.50",
        pass: r2 >= 0.50,
    });
    // R3 dead air (gaps > 4 beats inside span)
    let mut s = iv.clone();
    s.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let mut gaps: Vec<f64> = Vec::new();
    let mut cur = s[0].1;
    for &(a, b) in &s[1..] {
        if a - cur > 4.0 {
            gaps.push(round_to(a - cur, 1));
        }
        cur = cur.max(b);
    }
    let r3 = gaps.len() <= 1 && gaps.iter().all(|x| *x <= 16.5);
    g.push(Gate {
        name: "R3 lead silences >4 beats in span",
        measured: format!("{} gap(s) {:?}", gaps.len(), gaps),
        floor: "count<=1 and each<=16.5",
        pass: r3,
    });
    // R4 16-beat-window occupancy
    let w0 = (first / 16.0).floor() * 16.0;
    let mut wins: Vec<f64> = Vec::new();
    let mut w = w0;
    while w + 16.0 <= last_end + 1e-6 || w < first + span - 8.0 {
        if w + 16.0 > total {
            break;
        }
        let lw: Vec<(f64, f64)> = iv
            .iter()
            .copied()
            .filter(|(a, b)| *b > w && *a < w + 16.0)
            .collect();
        let cov = if lw.is_empty() {
            0.0
        } else {
            let clipped: Vec<(f64, f64)> = lw
                .iter()
                .map(|(a, b)| (a.max(w), b.min(w + 16.0)))
                .collect();
            union_len(&clipped) / 16.0
        };
        wins.push(round_to(cov, 2));
        w += 16.0;
        if w >= last_end {
            break;
        }
    }
    let empty = wins.iter().filter(|c| **c == 0.0).count();
    let r4 = empty <= 1 && wins.iter().filter(|c| **c > 0.0).all(|c| *c >= 0.55);
    g.push(Gate {
        name: "R4 16-beat window lead coverage",
        measured: format!("windows {wins:?} (empty {empty})"),
        floor: "every non-empty >=0.55, <=1 empty",
        pass: r4,
    });
    // R5 distinct lead-line identities (greedy clustering at ratio 0.75)
    let mut reps: Vec<Vec<i32>> = Vec::new();
    let mut wb = w0;
    while wb < total.floor() {
        let lw: Vec<&&LeadNote> = body
            .iter()
            .filter(|n| n.t >= wb && n.t < wb + 16.0)
            .collect();
        if lw.len() >= 5 {
            let ln: Vec<i32> = lw.windows(2).map(|p| p[1].p - p[0].p).collect();
            if !reps.iter().any(|r| seq_ratio(&ln, r) >= 0.75) {
                reps.push(ln);
            }
        }
        wb += 16.0;
    }
    g.push(Gate {
        name: "R5 distinct lead-line identities",
        measured: format!("{}", reps.len()),
        floor: ">=3",
        pass: reps.len() >= 3,
    });

    // R6 harmony: chord symbol = root pitch class + quality
    let ch = &inp.chords;
    let syms: BTreeSet<&str> = ch.iter().map(|c| c.2.as_str()).collect();
    let mut runs: Vec<(f64, f64, String)> = Vec::new();
    let mut cur_s: Option<String> = None;
    let (mut cur_len, mut cur_start) = (0.0, 0.0);
    for (t, d, sy) in ch {
        if cur_s.as_deref() == Some(sy.as_str()) {
            cur_len += d;
        } else {
            if let Some(cs) = cur_s.take() {
                runs.push((cur_start, cur_len, cs));
            }
            cur_s = Some(sy.clone());
            cur_len = *d;
            cur_start = *t;
        }
    }
    if let Some(cs) = cur_s.take() {
        runs.push((cur_start, cur_len, cs));
    }
    let maxrun = runs
        .iter()
        .filter(|r| r.0 >= 16.0 && r.0 + r.1 <= total - 16.0 + 1e-6)
        .map(|r| r.1)
        .fold(0.0, f64::max);
    let mut wd: Vec<usize> = Vec::new();
    let mut w6 = 16.0;
    while w6 < total.floor() - 16.0 {
        let n: HashSet<&str> = ch
            .iter()
            .filter(|c| c.0 >= w6 && c.0 < w6 + 16.0)
            .map(|c| c.2.as_str())
            .collect();
        wd.push(n.len());
        w6 += 16.0;
    }
    let moving = wd.iter().filter(|x| **x >= 2).count();
    let r6a = syms.len() >= 5;
    let r6b = maxrun <= 16.0;
    let r6c = moving as f64 >= 0.75 * wd.len() as f64;
    g.push(Gate {
        name: "R6a distinct chord symbols",
        measured: format!("{} {:?}", syms.len(), syms),
        floor: ">=5",
        pass: r6a,
    });
    g.push(Gate {
        name: "R6b longest interior identical-chord run",
        measured: format!("{maxrun} beats"),
        floor: "<=16",
        pass: r6b,
    });
    g.push(Gate {
        name: "R6c interior windows with >=2 chords",
        measured: format!("{moving}/{} per-window {wd:?}", wd.len()),
        floor: ">=75% of windows",
        pass: r6c,
    });

    // R7 pitched attack instants per bar (union of all pitched roles)
    let key = |t: f64| (t * 1000.0).round() as i64;
    let mut inst: HashMap<i64, HashSet<i64>> = HashMap::new();
    let mut all: HashSet<i64> = HashSet::new();
    for n in &inp.notes {
        inst.entry((n.t / 16.0).floor() as i64)
            .or_default()
            .insert(key(n.t));
        all.insert(key(n.t));
    }
    let nw = (total / 16.0).floor() as i64;
    let per: Vec<f64> = (0..nw)
        .map(|w| inst.get(&w).map_or(0, HashSet::len) as f64 / 4.0)
        .collect();
    let mean_all = all.len() as f64 / (total / 4.0);
    let min_int = if nw > 2 {
        per[1..per.len() - 1]
            .iter()
            .copied()
            .fold(f64::MAX, f64::min)
    } else {
        f64::NAN
    };
    g.push(Gate {
        name: "R7 pitched attack instants/bar",
        measured: format!(
            "mean {} ; min interior window {min_int}",
            round_to(mean_all, 2)
        ),
        floor: "mean>=7.5 and min interior>=5.0",
        pass: mean_all >= 7.5 && min_int >= 5.0,
    });
    // R8 bass distinct pitches per 16-beat window
    let bp: Vec<usize> = (0..nw)
        .map(|w| {
            inp.notes
                .iter()
                .filter(|n| {
                    n.role == Role::Bass && n.t >= w as f64 * 16.0 && n.t < w as f64 * 16.0 + 16.0
                })
                .map(|n| n.p)
                .collect::<HashSet<_>>()
                .len()
        })
        .collect();
    let after: &[usize] = if bp.is_empty() { &[] } else { &bp[1..] };
    let ok8 = after.iter().filter(|x| **x >= 6).count();
    g.push(Gate {
        name: "R8 bass distinct pitches/16 beats",
        measured: format!("{ok8}/{} windows>=6 {bp:?}", after.len()),
        floor: ">=6 in >=90% of windows after first",
        pass: ok8 as f64 >= 0.9 * after.len() as f64 - 1e-9,
    });
    // R9 lead window mean-velocity range
    let lm: Vec<f64> = (0..nw)
        .filter_map(|w| {
            let v: Vec<f64> = body
                .iter()
                .filter(|n| n.t >= w as f64 * 16.0 && n.t < w as f64 * 16.0 + 16.0)
                .map(|n| n.v)
                .collect();
            (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
        })
        .collect();
    let range = if lm.is_empty() {
        0.0
    } else {
        lm.iter().copied().fold(f64::MIN, f64::max) - lm.iter().copied().fold(f64::MAX, f64::min)
    };
    g.push(Gate {
        name: "R9 lead window mean-vel range",
        measured: format!("{}", round_to(range, 3)),
        floor: ">=0.15",
        pass: range >= 0.15,
    });

    // --- Diagnostic backstops (Citadel-Rick teacher calibration). NOT folded into the nine: they
    // prove the lead is COMPOSED, not improvised. A random walk in scale fails R-ROOT (mean ~0.50);
    // a single-grid loop fails R-GRID. These are the real anti-Goodhart guards — motif-count alone
    // is gameable, so it is deliberately not a gate here. ---
    if !inp.chord_pcs.is_empty() {
        // R-ROOT: pooled strong-beat (beats 1 and 3) lead chord-tone fraction against the chord
        // playing under that beat. Floor max(0.65, 0.50 + 1.16/sqrt(n)), n >= 30 strong beats.
        let active_pcs = |t: f64| -> Option<&Vec<i32>> {
            inp.chord_pcs
                .iter()
                .find(|(s, d, _)| t >= *s - 1e-6 && t < *s + *d - 1e-6)
                .or_else(|| inp.chord_pcs.last())
                .map(|(_, _, pcs)| pcs)
        };
        let mut strong = 0usize;
        let mut rooted = 0usize;
        for n in &body {
            let b = n.t.rem_euclid(4.0);
            let on_strong = b < 0.25 || (b - 2.0).abs() < 0.25 || b > 3.75;
            if !on_strong {
                continue;
            }
            strong += 1;
            if let Some(pcs) = active_pcs(n.t) {
                if pcs.contains(&n.p.rem_euclid(12)) {
                    rooted += 1;
                }
            }
        }
        let frac = if strong == 0 {
            0.0
        } else {
            rooted as f64 / strong as f64
        };
        let floor = 0.65f64.max(0.50 + 1.16 / (strong.max(1) as f64).sqrt());
        g.push(Gate {
            name: "R-ROOT strong-beat chord-tone fraction",
            measured: format!("{} ({rooted}/{strong})", round_to(frac, 3)),
            floor: "pooled >= max(0.65, 0.50+1.16/sqrt(n)), n>=30",
            pass: strong >= 30 && frac >= floor - 1e-9,
        });

        // R-GRID: distinct lead onset grids (per-bar onset sets on the 16th grid). A single-grid
        // loop collapses to 1; the teacher runs 5. Floor >= 3 distinct across the song.
        use std::collections::BTreeSet;
        let mut grids: HashSet<Vec<i64>> = HashSet::new();
        let mut per_bar: HashMap<i64, BTreeSet<i64>> = HashMap::new();
        for n in &body {
            let bar = (n.t / 4.0).floor() as i64;
            let off = ((n.t.rem_euclid(4.0)) * 4.0).round() as i64;
            per_bar.entry(bar).or_default().insert(off);
        }
        for set in per_bar.values() {
            grids.insert(set.iter().copied().collect());
        }
        g.push(Gate {
            name: "R-GRID distinct lead onset grids",
            measured: format!("{}", grids.len()),
            floor: ">=3 across the song",
            pass: grids.len() >= 3,
        });
    }

    // R-DYN: the BACKING (every non-lead pitched voice) per-16-beat-window mean-velocity range —
    // does the whole BAND breathe, or play one flat wall at a single dynamic? This is the band's
    // analog of R9 (which measures only the lead). A flat backing — every window the same velocity
    // — collapses to ~0.0 and fails; the teacher's band and the section-energy arc span a real
    // range. The anti-Goodhart guard for "the band is all going full blast at the same energy":
    // it rewards dynamic RANGE, never note count, so it cannot be gamed by piling on events.
    let bm: Vec<f64> = (0..nw)
        .filter_map(|w| {
            let v: Vec<f64> = inp
                .notes
                .iter()
                .filter(|n| {
                    n.role != Role::Lead && n.t >= w as f64 * 16.0 && n.t < w as f64 * 16.0 + 16.0
                })
                .map(|n| n.v)
                .collect();
            (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
        })
        .collect();
    let brange = if bm.is_empty() {
        0.0
    } else {
        bm.iter().copied().fold(f64::MIN, f64::max) - bm.iter().copied().fold(f64::MAX, f64::min)
    };
    g.push(Gate {
        name: "R-DYN backing window mean-vel range",
        measured: format!(
            "{} {:?}",
            round_to(brange, 3),
            bm.iter().map(|x| round_to(*x, 3)).collect::<Vec<_>>()
        ),
        floor: ">=0.08 (the band breathes, not a flat wall)",
        pass: brange >= 0.08,
    });
    g
}

/// Fold the eleven report lines into the nine gate verdicts R1..R9 (R6 = a AND b AND c).
fn nine(g: &[Gate]) -> [bool; 9] {
    if g.len() == 9 {
        // empty-lead short circuit: every gate already rejected
        return [false; 9];
    }
    [
        g[0].pass,
        g[1].pass,
        g[2].pass,
        g[3].pass,
        g[4].pass,
        g[5].pass && g[6].pass && g[7].pass,
        g[8].pass,
        g[9].pass,
        g[10].pass,
    ]
}

/// Calibration-format TSV (T/N/D/C lines) so the Python `gates.py` can cross-check this port.
fn dump_tsv(score: &Score) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "T\t{}\t{}\t{}",
        score.tempo_bpm, score.beats_per_bar, score.total_beats
    );
    for n in &score.notes {
        let _ = writeln!(
            s,
            "N\t{}\t{}\t{}\t{}\t{}\t{}\t{:?}\t{:?}",
            n.role.label(),
            n.start_beat,
            n.dur_beats,
            n.pitch,
            n.velocity,
            n.prov.section.label(),
            n.prov.phrase,
            n.function
        );
    }
    for d in &score.drums {
        let _ = writeln!(
            s,
            "D\t{:?}\t{}\t{}\t{}",
            d.voice,
            d.start_beat,
            d.velocity,
            d.prov.section.label()
        );
    }
    for c in &score.chords {
        let _ = writeln!(
            s,
            "C\t{}\t{}\t{}\t{:?}\t{:?}\t{}",
            c.start_beat, c.dur_beats, c.chord.root_pc, c.chord.quality, c.function, c.degree
        );
    }
    s
}

struct Run {
    seed: u64,
    fingerprint: u64,
    lead_pitches: Vec<i32>,
    lead_shape: Vec<i32>,
    lead_count: usize,
    verdict: [bool; 9],
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |p: &str| {
        args.iter()
            .find_map(|a| a.strip_prefix(p).map(str::to_string))
    };
    // THE CAR (edit A, harness only — NOT gated): the teacher drives at 138 BPM; ours idled at
    // 108. Pure time-scale on the default; --tempo= still overrides.
    let tempo: f32 = arg("--tempo=")
        .and_then(|s| s.parse().ok())
        .unwrap_or(138.0);
    let beats: f64 = arg("--beats=")
        .and_then(|s| s.parse().ok())
        .unwrap_or(128.0);
    let table_only = args.iter().any(|a| a == "--table-only");
    let seeds: Vec<u64> = match arg("--seeds=") {
        Some(list) => list.split(',').map(str::parse).collect::<Result<_, _>>()?,
        None => vec![arg("--seed=").and_then(|s| s.parse().ok()).unwrap_or(2112)],
    };

    // Cross-check mode: evaluate a calibration-format TSV dump directly (no render).
    if let Some(path) = arg("--tsv=") {
        let gates = evaluate(&Input::from_tsv(&std::fs::read_to_string(&path)?)?);
        println!("{path}");
        for gt in &gates {
            println!(
                "  {:<42} measured: {}  | floor: {}  => {}",
                gt.name,
                gt.measured,
                gt.floor,
                if gt.pass { "PASS" } else { "REJECT" }
            );
        }
        println!("  PASSED {}/9", nine(&gates).iter().filter(|b| **b).count());
        return Ok(());
    }
    // Base song exactly as examples/humanmusic_argument.rs builds it.
    let base = SongMap::compose(
        &deflected_lift_trace(beats),
        2112,
        Some(CompositionGrammar::DeflectedLift),
        Composer::MeaningDirected,
    );
    let mut world = MusicWorld::black_ice();
    world.tempo_bpm = tempo;
    world.tonic_pc = 9;
    world.mode = Mode::Aeolian;
    world.use_sevenths = true;
    world.allow_modal_mixture = true;
    let nphrases = base.plan.form.phrases.len();
    println!("fusion_gates: black_ice, A Aeolian, {tempo} BPM, {beats} beats, {nphrases} phrases");
    println!("REJECTION gates only: PASS = not auto-disqualified, NEVER 'sounds good'. The ear is the oracle.");

    let mut runs: Vec<Run> = Vec::new();
    for &seed in &seeds {
        let argument = MusicalArgument::fusion(seed, &base)?;
        let compiled = argument.compile(&base)?;
        let c = match perform_argument(
            &compiled,
            &world,
            PerformanceOptions::default(),
            PerformanceProfile::BAND,
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("perform_argument ERROR (seed {seed}): {e:?}");
                std::process::exit(2);
            }
        };
        let gates = evaluate(&Input::from_score(&c.score));
        let verdict = nine(&gates);
        if !table_only {
            println!(
                "\n== seed {seed}  (Score fingerprint {:016x}) ==",
                c.score.fingerprint()
            );
            for gt in &gates {
                println!(
                    "  {:<42} measured: {}  | floor: {}  => {}",
                    gt.name,
                    gt.measured,
                    gt.floor,
                    if gt.pass { "PASS" } else { "REJECT" }
                );
            }
            let k = verdict.iter().filter(|b| **b).count();
            println!("  PASSED {k}/9");
        }
        let lead_pitches: Vec<i32> = c
            .score
            .notes
            .iter()
            .filter(|n| n.role == Role::Lead)
            .map(|n| n.pitch)
            .collect();
        if let Some(dir) = arg("--dump-tsv=") {
            let path = format!("{dir}/fusion_{seed}.tsv");
            std::fs::write(&path, dump_tsv(&c.score))?;
            println!("dumped calibration-format TSV: {path}");
        }
        if let Some(dir) = arg("--wav=") {
            std::fs::create_dir_all(&dir)?;
            let rate = SampleRate::new(48_000).ok_or("zero sample rate")?;
            let mut synth = HumanMusicSynth::new(&c.score, &world, rate);
            synth.set_stem_mask(StemMask::full());
            let frames = synth.total_samples();
            let audio = OfflineRenderer::new(rate, 256)
                .render(&mut synth, frames)
                .audio;
            let path = format!("{dir}/fusion_{seed}_mix.wav");
            write_wav_i16(&path, &audio, rate)?;
            println!(
                "wrote {path} (peak {:.3}, rms {:.3}, {} frames)",
                audio.peak(),
                audio.rms(),
                audio.frames()
            );
        }
        runs.push(Run {
            seed,
            fingerprint: c.score.fingerprint(),
            lead_shape: lead_pitches.windows(2).map(|p| p[1] - p[0]).collect(),
            lead_count: lead_pitches.len(),
            lead_pitches,
            verdict,
        });
    }

    println!("\n== seed table ==");
    println!(
        "{:<10} {:<18} {:>5} R1 R2 R3 R4 R5 R6 R7 R8 R9  PASSED",
        "seed", "fingerprint", "lead"
    );
    for r in &runs {
        let cells: String = r
            .verdict
            .iter()
            .map(|b| if *b { "ok  " } else { "REJ " })
            .collect();
        println!(
            "{:<10} {:016x}   {:>5} {}  {}/9",
            r.seed,
            r.fingerprint,
            r.lead_count,
            cells,
            r.verdict.iter().filter(|b| **b).count()
        );
    }
    if runs.len() > 1 {
        let fps: HashSet<u64> = runs.iter().map(|r| r.fingerprint).collect();
        let leads: HashSet<&Vec<i32>> = runs.iter().map(|r| &r.lead_pitches).collect();
        let shapes: HashSet<&Vec<i32>> = runs.iter().map(|r| &r.lead_shape).collect();
        println!(
            "seed variation: {} distinct Score fingerprints, {} distinct lead pitch sequences, {} distinct lead interval shapes (transposition-invariant) across {} seeds",
            fps.len(),
            leads.len(),
            shapes.len(),
            runs.len()
        );
    }
    Ok(())
}
