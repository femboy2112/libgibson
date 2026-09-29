//! **The composer** (Round X) — how a song's CONTENT is chosen: the thesis, its consequent, where
//! each is stated, and the chart. Round IX's composer (`StructuralR9`) derives the theme and the
//! chart from the seed alone; it is kept, unchanged, as the control. The `MeaningDirected`
//! composer writes toward the listener plan its story asks for ([`MeaningPlan::target`]) under an
//! explicit [`CompositionalPrior`] — a small, bounded, inspectable search, never a score.
//!
//! **What changes and what does not.** Both composers share the SAME form (contract, phrases,
//! roles, the lead's seats, the backbone slots — [`super::plan::CompositionPlan`]); only the theme
//! and the chart differ. So an A/B between them isolates the song: same trace, seed, grammar and
//! band, a different page.
//!
//! **How a candidate is chosen** — filters in order, each with its survivors counted, no weighted
//! total: (1) the hard constraints (a lawful theme / a lawful DeflectedLift journey); (2) the law —
//! μ of the song with this candidate must agree with F ([`super::meaning::Commutation`]); (3) the
//! prior's soft bands; (4) the prior's preference (theme: chart fit); (5) the seed, among equals.
//! A filter that would empty the field is skipped and reported, never forced.

use super::backbone::{lead_sheet, ChartCell, ChartRoot};
use super::context::{common_tones, PullEvidence};
use super::discourse::DiscourseRole;
use super::harmony::ChordSpan;
use super::meaning::{
    landing, reach, Close, Commutation, Family, Lane, Level, MeaningKind, MeaningPlan, Owner,
};
use super::motif::{Handoff, Motif, MotifBank, ThematicTrajectory};
use super::rng::Rng;
use super::song::{HarmonicMap, SongMap, ThematicMap, ThemeSite};
use super::theory::{Quality, Scale};

/// Which composer chose a song's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Composer {
    /// Round IX: the germ and the chart from the seed alone. The control.
    StructuralR9,
    /// Round X: content selected against the story's [`MeaningPlan`] under a
    /// [`CompositionalPrior`]. Experimental; NOT the default before the human listen accepts it.
    MeaningDirected,
}

/// **What kinds of SONG a composer and its listeners find syntactically familiar** — separate
/// from [`super::language::MusicalLanguage`], which says how a BAND performs a song. One prior is
/// implemented: [`CompositionalPrior::HOOKY_FUSION`] — hook first, a compact singable thesis in the
/// octave above home with one salient reach, repetition before transformation, moderate
/// displacement, a legible chart whose only surprise is the designated miss, and home more
/// familiar than the chord that misses it.
///
/// Hard constraints make a candidate lawful; soft preferences only rank lawful candidates, and a
/// candidate outside them stays valid (the diagnostics say why it is unusual).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompositionalPrior {
    pub name: &'static str,
    // --- theme, HARD ---
    /// Beats per thesis statement (so it fits the shortest lead phrase with a breath).
    pub theme_beats: f32,
    /// Fewest notes (every culminating / returning statement must be at least the thesis long).
    pub min_notes: usize,
    /// Widest span, in scale steps.
    pub max_span: i32,
    /// The landing note's least length, in beats.
    pub min_landing_beats: f32,
    // --- theme, SOFT ---
    /// Least fraction of moves by step or repeat.
    pub min_stepwise: f32,
    /// Most leaps of a fourth or wider (one salient reach, not six).
    pub max_large_leaps: usize,
    /// The highest scale degree the thesis may climb to, counted from home: a tune that sits in
    /// the octave above its tonic, not a fourth above it (the band sets the octave; the song sets
    /// where in it the tune lives).
    pub max_top: i32,
    /// The rhythmic displacement band aimed at (stable pulse + patterned push, not maximal).
    pub displacement: Level,
    /// Chart-fit resolution: fits within one bucket are equals (the seed decides among them).
    pub fit_bucket: f32,
    // --- chart, SOFT ---
    /// Most satellites that merely hold their anchor (a pedal).
    pub max_pedals: usize,
    /// Home is heard at least as long as the chord that deflects from it — the expected arrival
    /// stays the more familiar harmony, or missing it means nothing.
    pub home_over_miss: bool,
    /// Home is reached from the Open by a fourth/fifth (plagal or authentic) move.
    pub strong_reset: bool,
}

impl CompositionalPrior {
    /// The one prior this round implements.
    pub const HOOKY_FUSION: CompositionalPrior = CompositionalPrior {
        name: "hooky_fusion",
        theme_beats: 6.0,
        min_notes: 5,
        max_span: 8,
        min_landing_beats: 1.5,
        min_stepwise: 0.5,
        max_large_leaps: 1,
        max_top: 7,
        displacement: Level::Mid,
        fit_bucket: 0.1,
        max_pedals: 1,
        home_over_miss: true,
        strong_reset: true,
    };
}

// -------------------------------------------------------------------------------------------------
// Theme

/// One point of the thesis grammar: `[pickup] cell cell' reach recovery landing`, six beats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeParams {
    /// A pickup a step below the launch.
    pub pickup: bool,
    /// The recurring two-note cell as long-short (dotted) instead of even eighths.
    pub long_short: bool,
    /// The launch degree (a tonic-triad degree: 0, 2 or 4).
    pub launch: i32,
    /// The cell's restatement: in place (0) or sequenced a step up (1).
    pub sequence: i32,
    /// The reach above the cell's top, in scale steps (2: none salient, 3: a fourth, 5: a sixth).
    pub reach: i32,
}

/// The thesis grammar's full space.
pub fn theme_grammar() -> Vec<ThemeParams> {
    let mut v = Vec::new();
    for pickup in [true, false] {
        for long_short in [false, true] {
            for launch in [0, 2, 4] {
                for sequence in [0, 1] {
                    for reach in [2, 3, 5] {
                        v.push(ThemeParams {
                            pickup,
                            long_short,
                            launch,
                            sequence,
                            reach,
                        });
                    }
                }
            }
        }
    }
    v
}

/// The degree nearest `from` (excluding `from`) whose class (mod 7) is in `classes`, preferring
/// below on a tie.
fn nearest_of(from: i32, classes: &[i32]) -> i32 {
    (1..14)
        .flat_map(|d| [from - d, from + d])
        .find(|&x| classes.contains(&x.rem_euclid(7)))
        .unwrap_or(from)
}

/// How a line built from the grammar ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ending {
    /// The thesis: reach, step back, land on the third or fifth — stable, not final.
    Antecedent,
    /// The consequent: rise only a step, step back, and settle ([`Close::Home`]: the tonic) or
    /// hang ([`Close::Open`]: the fifth or the second).
    Consequent(Close),
}

fn build_line(p: &ThemeParams, ending: Ending, beats: f32) -> Motif {
    let cell: [f32; 2] = if p.long_short {
        [0.75, 0.25]
    } else {
        [0.5, 0.5]
    };
    let a = p.launch;
    let (mut d, mut r) = (Vec::new(), Vec::new());
    if p.pickup {
        d.push(a - 1);
        r.push(0.5);
    }
    let top = a + 1 + p.sequence;
    d.extend([a, a + 1, a + p.sequence, top]);
    r.extend([cell[0], cell[1], cell[0], cell[1]]);
    let x = match ending {
        Ending::Antecedent => top + p.reach,
        Ending::Consequent(_) => top + 1,
    };
    d.extend([x, x - 1]);
    r.extend([1.0, 0.5]);
    let land = match ending {
        Ending::Antecedent => nearest_of(x - 1, &[2, 4]),
        Ending::Consequent(Close::Home) => nearest_of(x - 1, &[0]),
        // Hanging: the fifth or the second (a half cadence), whichever is nearer.
        Ending::Consequent(Close::Open) => nearest_of(x - 1, &[4, 1]),
    };
    d.push(land);
    r.push(beats - r.iter().sum::<f32>());
    Motif {
        id: 0,
        degrees: d,
        rhythm: r,
    }
}

/// A thesis's listener-relevant shape — an inspectable vector, not a verdict. Which way each field
/// should go is the prior's business (bands, not maxima).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeProfile {
    pub notes: usize,
    pub beats: f32,
    /// Widest span, in scale steps.
    pub span: i32,
    /// The highest degree, counted from home.
    pub top: i32,
    /// Fraction of moves by step or repeat.
    pub stepwise: f32,
    /// Leaps of a fourth or wider.
    pub large_leaps: usize,
    /// Every such leap is followed by a step back against it.
    pub recovered: bool,
    /// Inter-onset patterns (duration pairs) heard more than once — the rhythmic cell recurring.
    pub recurrence: usize,
    /// Rhythmic displacement from a downbeat start: sixteenth-offset onsets plus long notes struck
    /// off the beat (0 low, 1–2 mid, 3+ high).
    pub displacement: Level,
    /// The widest leap's level ([`reach`]).
    pub reach: Level,
    /// Where it comes to rest ([`landing`]).
    pub landing: Close,
    /// The landing note's scale degree class (0 tonic, 2 third, 4 fifth…).
    pub landing_class: i32,
    pub landing_beats: f32,
    /// The longest run of one repeated degree.
    pub longest_repeat: usize,
    /// Every duration a positive multiple of a sixteenth.
    pub well_formed: bool,
}

impl ThemeProfile {
    /// The profile of `m`.
    pub fn of(m: &Motif) -> ThemeProfile {
        let moves: Vec<i32> = m.degrees.windows(2).map(|w| w[1] - w[0]).collect();
        let large: Vec<usize> = (0..moves.len()).filter(|&i| moves[i].abs() >= 3).collect();
        let recovered = large.iter().all(|&i| {
            moves
                .get(i + 1)
                .is_some_and(|&n| n.abs() <= 2 && n.signum() == -moves[i].signum())
        });
        let pairs: Vec<(u32, u32)> = m
            .rhythm
            .windows(2)
            .map(|w| ((w[0] * 4.0).round() as u32, (w[1] * 4.0).round() as u32))
            .collect();
        let recurrence = pairs
            .iter()
            .enumerate()
            .filter(|(i, p)| pairs[..*i].contains(p))
            .count();
        let mut t = 0.0f32;
        let mut displaced = 0;
        for &r in &m.rhythm {
            let frac = t - t.floor();
            // A sixteenth-offset onset, or a long note struck off the beat.
            let sixteenth = (frac - 0.25).abs() < 1e-3 || (frac - 0.75).abs() < 1e-3;
            let long_off_beat = (frac - 0.5).abs() < 1e-3 && r >= 1.0 - 1e-3;
            if sixteenth || long_off_beat {
                displaced += 1;
            }
            t += r;
        }
        let mut longest_repeat = 1;
        let mut run = 1;
        for w in m.degrees.windows(2) {
            run = if w[0] == w[1] { run + 1 } else { 1 };
            longest_repeat = longest_repeat.max(run);
        }
        let (lo, hi) = (
            m.degrees.iter().copied().min().unwrap_or(0),
            m.degrees.iter().copied().max().unwrap_or(0),
        );
        ThemeProfile {
            notes: m.len(),
            beats: m.total_beats(),
            span: hi - lo,
            top: hi,
            stepwise: if moves.is_empty() {
                1.0
            } else {
                moves.iter().filter(|x| x.abs() <= 1).count() as f32 / moves.len() as f32
            },
            large_leaps: large.len(),
            recovered,
            recurrence,
            displacement: match displaced {
                0 => Level::Low,
                1 | 2 => Level::Mid,
                _ => Level::High,
            },
            reach: reach(m),
            landing: landing(m),
            landing_class: m.degrees.last().map_or(0, |d| d.rem_euclid(7)),
            landing_beats: m.rhythm.last().copied().unwrap_or(0.0),
            longest_repeat,
            well_formed: m.rhythm.iter().all(|&r| {
                let q = r * 4.0;
                r >= 0.25 - 1e-6 && (q - q.round()).abs() < 1e-4
            }),
        }
    }

    /// The prior's HARD constraints: why a line is not a lawful thesis, if it is not. Lawful means
    /// playable as a statement — it fits the shortest lead phrase with a breath, is at least the
    /// thesis long, stays in a singable span, ends on a note long enough to end on, and its
    /// rhythm is well formed. Nothing about taste.
    pub fn unlawful(&self, prior: &CompositionalPrior) -> Option<&'static str> {
        if (self.beats - prior.theme_beats).abs() > 1e-4 {
            Some("wrong length")
        } else if !self.well_formed {
            Some("malformed rhythm")
        } else if self.notes < prior.min_notes {
            Some("too few notes")
        } else if self.span > prior.max_span {
            Some("too wide")
        } else if self.landing_beats < prior.min_landing_beats {
            Some("no landing")
        } else {
            None
        }
    }

    /// The prior's SOFT bands this line sits outside (empty: a typical line for the prior). A line
    /// outside them is still lawful; this is WHY it is unusual, not a verdict on it.
    pub fn unusual(&self, prior: &CompositionalPrior) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.stepwise < prior.min_stepwise {
            v.push("mostly leaps");
        }
        if self.large_leaps > prior.max_large_leaps {
            v.push("more than one salient leap");
        }
        if self.top > prior.max_top {
            v.push("climbs above the octave over home");
        }
        if !self.recovered {
            v.push("a leap left unrecovered");
        }
        if self.recurrence == 0 {
            v.push("no recurring rhythmic cell");
        }
        if self.displacement != prior.displacement {
            v.push("displacement outside the band");
        }
        if ![0, 2, 4].contains(&self.landing_class) {
            v.push("lands off the tonic triad");
        }
        if self.longest_repeat >= 3 {
            v.push("one degree struck three times running");
        }
        v
    }
}

/// A thesis candidate: its grammar point, the thesis and its consequent, its profile, how well
/// its structural notes sit on the chart where it will be stated, and the filter that dropped it.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeCandidate {
    pub params: ThemeParams,
    pub thesis: Motif,
    pub answer: Motif,
    pub profile: ThemeProfile,
    /// Fraction of structural notes that are chord tones of the charted chord under them, over
    /// every statement of the thesis and its consequent (mode-safe: a dominant's leading tone
    /// never counts, since a minor room raises it).
    pub fit: f32,
    /// Theme-lane divergences from the MeaningPlan when this thesis and consequent are scheduled
    /// into the song (the law, run on the candidate — not assumed from its construction).
    pub divergences: usize,
    /// `None` if it survived every filter it met; else the first filter that removed it.
    pub dropped_by: Option<&'static str>,
}

/// Structural notes of a statement starting at `start` (the line engine's own rule: first, last,
/// the contour peak, bar-strong onsets, long notes), as `(beat, degree)`.
fn structural(m: &Motif, start: f64) -> Vec<(f64, i32)> {
    let n = m.len();
    let peak = (0..n)
        .max_by(|&a, &b| m.degrees[a].cmp(&m.degrees[b]).then(b.cmp(&a)))
        .unwrap_or(0);
    let mut t = 0.0f64;
    let mut out = Vec::new();
    for i in 0..n {
        let abs = start + t;
        let b = abs.rem_euclid(super::form::BEATS_PER_BAR);
        let bar_strong = (b - b.round()).abs() < 1e-6 && (b.round() as i64).rem_euclid(2) == 0;
        if i == 0 || i + 1 == n || i == peak || bar_strong || m.rhythm[i] >= 1.5 {
            out.push((abs, m.degrees[i]));
        }
        t += m.rhythm[i] as f64;
    }
    out
}

/// Whether scale degree `d` (reference frame) is a chord tone of `span`'s chord, mode-safely.
fn sits(d: i32, span: &ChordSpan, frame: &Scale) -> bool {
    let pc = frame.degree_pitch(d, 4).rem_euclid(12);
    let c = span.chord;
    let leading = matches!(c.quality, Quality::Dom7) && pc == (c.root_pc + 4).rem_euclid(12);
    c.pitch_classes().contains(&pc) && !leading
}

/// The statement onsets a phrase gives a line of `len` beats (the performance's rule: from the
/// phrase start, then the next two-bar boundary after each statement, while it fits).
fn statement_starts(start: f64, end: f64, len: f64) -> Vec<f64> {
    let two_bar = 2.0 * super::form::BEATS_PER_BAR;
    let mut v = Vec::new();
    let mut at = start;
    while at + len <= end + 1e-6 && v.len() < 32 {
        v.push(at);
        let after = at + len;
        let boundary = (after / two_bar).ceil() * two_bar;
        at = if boundary > after + 1e-6 {
            boundary
        } else {
            after
        };
    }
    v
}

// -------------------------------------------------------------------------------------------------
// Chart

/// A chart candidate: the cell, its observed harmony against the plan, and its prior profile.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartCandidate {
    pub cell: ChartCell,
    /// Harmony-lane divergences from the MeaningPlan the composer could have avoided.
    pub divergences: usize,
    /// Satellites that merely hold their anchor.
    pub pedals: usize,
    /// Home is heard at least as long as the deflecting chord (on the lead sheet).
    pub home_over_miss: bool,
    /// The move into home at every Reset is a fourth/fifth.
    pub strong_reset: bool,
    pub dropped_by: Option<&'static str>,
}

fn deg(d: i32) -> ChartRoot {
    ChartRoot::Degree(d)
}

/// The chart space: lift {I, ii, IV} × deflect {vi, iii, IV, bVI} × open {IV, ii, vi, iii} (not
/// the deflect) × the Reset's neighbour {IV, vi, held}; the pointer is always V7 on the fifth
/// degree and home is I (the DeflectedLift journey — the pointer carries the leading tone, the
/// Reset IS home). The lift's partner and the Deflect / Open neighbours follow by rule.
pub fn chart_space() -> Vec<ChartCell> {
    let frame = Scale::new(0, super::song::REFERENCE_FRAME);
    let chord = |r: ChartRoot| r.chord(&frame);
    let home = frame.tonic_pc;
    let familiar =
        |a: ChartRoot, b: ChartRoot| Family::of(&chord(a), &chord(b), &frame).is_familiar();
    let mut v = Vec::new();
    let deflects = [
        deg(5),
        deg(2),
        deg(3),
        ChartRoot::Chromatic {
            semitones: 8,
            quality: Quality::Maj,
        },
    ];
    for lift in [0, 1, 3] {
        let lift_alt = [1, 3, 5].into_iter().find(|&d| d != lift).unwrap_or(1);
        for deflect in deflects {
            for open in [3, 1, 5, 2] {
                let open = deg(open);
                if chord(open).root_pc == chord(deflect).root_pc {
                    continue;
                }
                // The deflect's neighbour: a diatonic chord with no pull home, a familiar step from
                // the miss and a familiar step into the Open; the smoothest into the Open wins.
                let sat0 = [1, 2, 3, 5]
                    .into_iter()
                    .map(deg)
                    .filter(|&d| {
                        let c = chord(d);
                        c.root_pc != chord(deflect).root_pc
                            && c.root_pc != chord(open).root_pc
                            && PullEvidence::of(&c, home).strength() < 0.35
                            && familiar(deflect, d)
                            && familiar(d, open)
                    })
                    .max_by_key(|&d| {
                        (
                            common_tones(&chord(d), &chord(open)),
                            common_tones(&chord(deflect), &chord(d)),
                        )
                    })
                    .unwrap_or(deflect);
                // The Open's neighbour: the plagal IV when the Open is not IV itself.
                let sat1 = if open != deg(3) && familiar(open, deg(3)) {
                    deg(3)
                } else {
                    open
                };
                for sat2 in [deg(3), deg(5), deg(0)] {
                    v.push(ChartCell {
                        lift: deg(lift),
                        lift_alt: deg(lift_alt),
                        pointer: deg(4),
                        expected: deg(0),
                        deflect,
                        open,
                        reset: deg(0),
                        satellites: [sat0, sat1, sat2],
                    });
                }
            }
        }
    }
    v
}

// -------------------------------------------------------------------------------------------------
// Selection

/// Filters applied in order: the survivors after each, by name — the audit trail of a choice.
pub type Stages = Vec<(&'static str, usize)>;

/// Apply one named filter to the live candidates; a filter that would leave none is skipped (and
/// recorded as such), never forced.
fn stage<T>(
    live: &mut Vec<usize>,
    all: &mut [T],
    name: &'static str,
    keep: impl Fn(&T) -> bool,
    drop: impl Fn(&mut T, &'static str),
    stages: &mut Stages,
) {
    let kept: Vec<usize> = live.iter().copied().filter(|&i| keep(&all[i])).collect();
    if kept.is_empty() {
        stages.push((name, usize::MAX));
        return;
    }
    for &i in live.iter() {
        if !kept.contains(&i) {
            drop(&mut all[i], name);
        }
    }
    *live = kept;
    stages.push((name, live.len()));
}

/// Everything the meaning-directed composer considered and why it chose what it chose.
#[derive(Debug, Clone, PartialEq)]
pub struct CompositionReport {
    pub prior: CompositionalPrior,
    pub target: MeaningPlan,
    pub charts: Vec<ChartCandidate>,
    pub chart_stages: Stages,
    pub chart: Option<usize>,
    pub themes: Vec<ThemeCandidate>,
    pub theme_stages: Stages,
    pub theme: usize,
}

/// Compose the meaning-directed content for `song`'s form and story: its chart (DeflectedLift
/// only), its thesis and consequent, and every site's statement. Returns the song with that
/// content (and its MeaningPlan) and the report of the choice. `song` supplies the form — the
/// Round IX plan, identical under either composer — and is otherwise replaced.
pub fn compose_meaning(
    mut song: SongMap,
    prior: &CompositionalPrior,
) -> (SongMap, CompositionReport) {
    let target = MeaningPlan::target(&song.trace, &song.plan);
    let seed = song.seed;

    // --- the chart: the law first, then the prior, then the seed ---
    let (mut charts, mut chart_stages, mut chart) = (Vec::new(), Vec::new(), None);
    if let Some(hm) = song.harmonic {
        let frame = Scale::new(0, song.frame);
        charts = chart_space()
            .into_iter()
            .map(|cell| {
                let mut trial = song.clone();
                trial.harmonic = Some(HarmonicMap { cell, ..hm });
                let c = Commutation::against(target.clone(), &trial);
                let divergences = c
                    .divergences
                    .iter()
                    .filter(|d| d.lane == Lane::Harmony && d.owner == Owner::Composer)
                    .count();
                let sheet = lead_sheet(
                    trial
                        .plan
                        .backbone
                        .as_ref()
                        .expect("a chart implies a backbone"),
                    trial.harmonic.as_ref().unwrap(),
                    song.frame,
                );
                let end = song.plan.form.total_beats;
                let mut heard = [0.0f64; 12];
                for s in sheet.iter().filter(|s| s.start_beat < end - 1e-9) {
                    let len = (s.dur_beats as f64).min(end - s.start_beat);
                    heard[s.chord.root_pc.rem_euclid(12) as usize] += len;
                }
                let home = heard[frame.tonic_pc as usize];
                let resets: Vec<_> = c
                    .observed
                    .events
                    .iter()
                    .filter(|w| w.event.kind == MeaningKind::Reset)
                    .collect();
                ChartCandidate {
                    cell,
                    divergences,
                    pedals: [
                        (cell.satellites[0], cell.deflect),
                        (cell.satellites[1], cell.open),
                        (cell.satellites[2], cell.reset),
                    ]
                    .iter()
                    .filter(|(s, a)| s == a)
                    .count(),
                    home_over_miss: heard[cell.deflect.root_pc(&frame) as usize] <= home + 1e-9,
                    strong_reset: !resets.is_empty()
                        && resets.iter().all(|w| {
                            matches!(
                                w.witness,
                                super::meaning::Witness::Move {
                                    family: Family::Fifth,
                                    ..
                                }
                            )
                        }),
                    dropped_by: None,
                }
            })
            .collect();
        let mut live: Vec<usize> = (0..charts.len()).collect();
        chart_stages.push(("journeys (lawful by construction)", live.len()));
        let mark = |c: &mut ChartCandidate, why| c.dropped_by = Some(why);
        stage(
            &mut live,
            &mut charts,
            "means the plan (μ = F)",
            |c| c.divergences == 0,
            mark,
            &mut chart_stages,
        );
        stage(
            &mut live,
            &mut charts,
            "few pedals",
            |c| c.pedals <= prior.max_pedals,
            mark,
            &mut chart_stages,
        );
        if prior.home_over_miss {
            stage(
                &mut live,
                &mut charts,
                "home over the miss",
                |c| c.home_over_miss,
                mark,
                &mut chart_stages,
            );
        }
        if prior.strong_reset {
            stage(
                &mut live,
                &mut charts,
                "strong reset",
                |c| c.strong_reset,
                mark,
                &mut chart_stages,
            );
        }
        let pick = live[Rng::new(seed ^ 0xC4A7_0510).below(live.len())];
        for &i in &live {
            if i != pick {
                charts[i].dropped_by = Some("seed");
            }
        }
        chart_stages.push(("seed", 1));
        chart = Some(pick);
        song.harmonic = Some(HarmonicMap {
            cell: charts[pick].cell,
            ..hm
        });
    }

    // --- the theme: lawful, the plan's reach and settling, the prior's bands, fit, the seed ---
    let frame = Scale::new(0, song.frame);
    let sheet: Vec<ChordSpan> = match (&song.plan.backbone, &song.harmonic) {
        (Some(bb), Some(hm)) => lead_sheet(bb, hm, song.frame),
        _ => Vec::new(),
    };
    let theme_events: Vec<(u32, MeaningKind)> = target
        .events
        .iter()
        .filter(|e| e.lane == Lane::Theme && !matches!(e.kind, MeaningKind::Thesis(_)))
        .map(|e| (e.at, e.kind))
        .collect();
    let phrase = |ix: u32| {
        song.plan
            .form
            .phrases
            .iter()
            .find(|p| p.ix == ix)
            .map(|p| (p.start_beat(), p.end_beat()))
            .unwrap_or((0.0, 0.0))
    };
    let fit_of = |thesis: &Motif, answer: &Motif| -> f32 {
        let (mut hit, mut all) = (0usize, 0usize);
        for &(at, kind) in &theme_events {
            let line = match kind {
                MeaningKind::Answer(_) => answer,
                MeaningKind::Develop => continue,
                _ => thesis,
            };
            let (s, e) = phrase(at);
            for start in statement_starts(s, e, line.total_beats() as f64) {
                for (beat, d) in structural(line, start) {
                    if let Some(span) = sheet.iter().find(|sp| {
                        sp.start_beat <= beat + 1e-6 && beat < sp.start_beat + sp.dur_beats as f64
                    }) {
                        all += 1;
                        hit += usize::from(sits(d, span, &frame));
                    }
                }
            }
        }
        if all == 0 {
            1.0
        } else {
            hit as f32 / all as f32
        }
    };
    let mut themes: Vec<ThemeCandidate> = theme_grammar()
        .into_iter()
        .map(|params| {
            let thesis = build_line(&params, Ending::Antecedent, prior.theme_beats);
            let answer = build_line(
                &params,
                Ending::Consequent(target.resolution),
                prior.theme_beats,
            );
            let mut trial = song.clone();
            trial.thematic = schedule(&song, &target, &thesis, &answer);
            let divergences = Commutation::against(target.clone(), &trial)
                .divergences
                .iter()
                .filter(|d| d.lane == Lane::Theme && d.owner == Owner::Composer)
                .count();
            ThemeCandidate {
                params,
                profile: ThemeProfile::of(&thesis),
                fit: fit_of(&thesis, &answer),
                thesis,
                answer,
                divergences,
                dropped_by: None,
            }
        })
        .collect();
    let mut theme_stages = Vec::new();
    let mut live: Vec<usize> = (0..themes.len()).collect();
    theme_stages.push(("grammar", live.len()));
    let mark = |c: &mut ThemeCandidate, why| c.dropped_by = Some(why);
    stage(
        &mut live,
        &mut themes,
        "lawful",
        |c| {
            c.profile.unlawful(prior).is_none()
                && ThemeProfile::of(&c.answer).unlawful(prior).is_none()
        },
        mark,
        &mut theme_stages,
    );
    // The law: scheduled into the song, the thesis reaches as far as the story's arc, is taught
    // before it is developed, and its consequent settles as the story does (μ = F, theme lane).
    stage(
        &mut live,
        &mut themes,
        "means the plan (μ = F)",
        |c| c.divergences == 0,
        mark,
        &mut theme_stages,
    );
    // Typical: the thesis inside every soft band; its consequent too, except where it lands —
    // the story decides that (a hanging answer lands off the tonic triad on purpose).
    stage(
        &mut live,
        &mut themes,
        "typical for the prior",
        |c| {
            c.profile.unusual(prior).is_empty()
                && ThemeProfile::of(&c.answer)
                    .unusual(prior)
                    .iter()
                    .all(|&why| why == "lands off the tonic triad")
        },
        mark,
        &mut theme_stages,
    );
    let bucket = |f: f32| ((f + 1e-4) / prior.fit_bucket).floor() as i32;
    let best = live
        .iter()
        .map(|&i| bucket(themes[i].fit))
        .max()
        .unwrap_or(0);
    stage(
        &mut live,
        &mut themes,
        "sits on the chart",
        |c| bucket(c.fit) == best,
        mark,
        &mut theme_stages,
    );
    let pick = live[Rng::new(seed ^ 0x7E3E_50A6).below(live.len())];
    for &i in &live {
        if i != pick {
            themes[i].dropped_by = Some("seed");
        }
    }
    theme_stages.push(("seed", 1));

    let (thesis, answer) = (themes[pick].thesis.clone(), themes[pick].answer.clone());
    song.thematic = schedule(&song, &target, &thesis, &answer);
    song.meaning = Some(target.clone());
    let report = CompositionReport {
        prior: *prior,
        target,
        charts,
        chart_stages,
        chart,
        themes,
        theme_stages,
        theme: pick,
    };
    (song, report)
}

/// The theme sites the plan asks for: the thesis as written wherever the listener is taught,
/// reminded, paid off or recognized; its consequent at every answer; a development (Round IX's
/// trajectory, from the thesis) only where the listener has learned it.
fn schedule(song: &SongMap, target: &MeaningPlan, thesis: &Motif, answer: &Motif) -> ThematicMap {
    use MeaningKind as K;
    let bank = MotifBank {
        identity: thesis.clone(),
        hook: thesis.clone(),
        rhythmic_cell: thesis.fragment(3).scale_rhythm(0.5),
        bass_cell: thesis.fragment(2).transpose(-7),
        countermotif: Some(thesis.invert()),
    };
    let mut traj = ThematicTrajectory::new(&bank);
    let mut sites = Vec::new();
    for t in song.plan.targets() {
        let Some(e) = target.events.iter().find(|e| {
            e.lane == Lane::Theme && e.at == t.phrase.ix && !matches!(e.kind, K::Thesis(_))
        }) else {
            continue;
        };
        let (motif, handoff) = match e.kind {
            K::Payoff => (thesis.clone(), Handoff::Hook),
            K::Answer(_) => (answer.clone(), Handoff::Consequent),
            K::Develop => traj.next_for(t.goal.role),
            _ => {
                traj.next_for(DiscourseRole::Restate);
                (thesis.clone(), Handoff::Restatement)
            }
        };
        sites.push(ThemeSite {
            phrase: t.phrase.ix,
            role: t.goal.role,
            motif,
            handoff,
        });
    }
    ThematicMap { bank, sites }
}
