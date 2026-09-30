//! Round XIV: a gesture is what the speakers emit, not what the analysis calls it.
//!
//! [`PitchFunction::SlidePath`] lets an intermediate pitch inherit its justification from the
//! route it sits on. That is honest only when the route is physically ONE gesture: one attack,
//! then the pitch travels. The synth has no such thing. `synth.rs` renders every [`Note`] with
//! a fresh `SynthVoice::trigger`: a new fixed frequency, the amplitude and filter envelopes gated
//! on, the filter reset. Nothing in `Note`, `Patch` or the synth glides, ties or bends. So
//! C–C#–D–D# as four Notes is four attacked notes, however fast, and every one of them owes its
//! own function.
//!
//! This module keeps that distinction in three words:
//!
//! - [`Gesture::Slide`]: one attack, every later pitch of the route glided into. It is only an
//!   abstract witness until a renderer can play it.
//! - [`Gesture::ChromaticApproach`]: two attacked pitches a semitone apart, the first leaning
//!   into the second.
//! - [`Gesture::DiscreteLine`]: any other route of attacked notes.
//!
//! [`GestureDiagnostics`] audits a score. It lists every note the Round XII observer supports as
//! a slide, or that declares one, and the attacked route it sits on. A slide claim on an attacked
//! note is a fake slide when the claim stands on the slide alone.
use super::context::HarmonicContext;
use super::performance::PerformancePlan;
use super::score::{Note, PitchFunction, Score};
use super::temporal::TemporalPitchDiagnostics;
use super::theory::{note_name, pitch_class, Midi};
use std::fmt::Write;

/// How a pitch state is entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// A fresh attack: new voice, envelopes gated on.
    Attacked,
    /// Reached by a continuous pitch glide from the previous state, with no new attack.
    Glided,
}

/// How the synth enters a [`Note`]: always a fresh attack (see the module docs).
pub fn entry_of(_: &Note) -> Entry {
    Entry::Attacked
}

/// One pitch state of a route.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathEvent {
    pub pitch: Midi,
    pub start_beat: f64,
    pub dur_beats: f64,
    pub entry: Entry,
}

impl PathEvent {
    /// The physical event a [`Note`] is.
    pub fn of(n: &Note) -> PathEvent {
        PathEvent {
            pitch: n.pitch,
            start_beat: n.start_beat,
            dur_beats: f64::from(n.dur_beats),
            entry: entry_of(n),
        }
    }
}

/// The physical gesture a route of pitch states is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    /// One attack; every later pitch glided into.
    Slide,
    /// Two attacked pitches a semitone apart.
    ChromaticApproach,
    /// Any other route of attacked pitches: each owes its own function.
    DiscreteLine,
}

impl Gesture {
    pub fn label(self) -> &'static str {
        match self {
            Gesture::Slide => "slide",
            Gesture::ChromaticApproach => "chromatic approach",
            Gesture::DiscreteLine => "discrete line",
        }
    }
}

/// Classify a monotone stepwise route: every step 1 or 2 semitones, one direction. `None` for
/// anything else (a leap, a repeat, a turn, fewer than two states).
pub fn classify(route: &[PathEvent]) -> Option<Gesture> {
    let first = route.get(1)?.pitch - route[0].pitch;
    let steps_ok = route.windows(2).all(|w| {
        let d = w[1].pitch - w[0].pitch;
        (1..=2).contains(&d.abs()) && d.signum() == first.signum()
    });
    if !steps_ok {
        return None;
    }
    Some(if route[1..].iter().all(|e| e.entry == Entry::Glided) {
        Gesture::Slide
    } else if route.len() == 2 && first.abs() == 1 {
        Gesture::ChromaticApproach
    } else {
        Gesture::DiscreteLine
    })
}

/// One note the analysis calls a slide (declared, or supported by the Round XII observer), on
/// the attacked route the observer followed.
#[derive(Debug, Clone, PartialEq)]
pub struct GestureRow {
    pub note: usize,
    /// The route from `note` along the observer's next-note links to its chord-tone landing.
    pub route: Vec<usize>,
    pub declared_slide: bool,
    /// The observer lists `SlidePath` among the note's reconstructed functions.
    pub slide_supported: bool,
    /// The note's justification stands on the slide alone (declared `SlidePath`, or `SlidePath`
    /// is its only reconstructed function).
    pub slide_dependent: bool,
    /// Every event of the route is a fresh attack.
    pub attacks: usize,
    pub gesture: Option<Gesture>,
    /// A slide claim the physical route cannot honour.
    pub fake: bool,
}

/// The gesture audit of one score: counts, then one row per slide claim.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GestureDiagnostics {
    /// Notes whose declared function is `SlidePath`.
    pub declared_slides: usize,
    /// Notes the Round XII observer supports as `SlidePath` (usually alongside other functions).
    pub slide_supported: usize,
    /// Notes whose justification stands on the slide alone.
    pub slide_dependent: usize,
    /// Slide claims on routes the synth plays as attacked notes, standing on the slide alone.
    pub fake_slides: usize,
    /// Physical slides (always 0: no renderer glides).
    pub physical_slides: usize,
    /// Routes of slide-supported notes that are one semitone approach.
    pub chromatic_approaches: usize,
    /// Routes of slide-supported notes with more than one attacked step, or a whole step.
    pub discrete_lines: usize,
    /// Of those, routes with two or more attacked chromatic intermediates (a staircase that could
    /// have hidden behind a slide).
    pub chromatic_staircases: usize,
    pub rows: Vec<GestureRow>,
}

impl GestureDiagnostics {
    /// Audit `score` (the Round XII observer supplies the reconstructed functions and links).
    pub fn measure(perf: &PerformancePlan, score: &Score) -> GestureDiagnostics {
        let observer = TemporalPitchDiagnostics::measure(perf, score);
        let mut out = GestureDiagnostics::default();
        let ctx_tone = |i: usize| {
            let n = &score.notes[i];
            perf.context_at(n.start_beat)
                .is_some_and(|c: &HarmonicContext| c.chord.contains_pc(pitch_class(n.pitch)))
        };
        for row in &observer.rows {
            let i = row.note_index;
            let n = &score.notes[i];
            let declared_slide = n.function == Some(PitchFunction::SlidePath);
            let slide_supported = row.supported.contains(&PitchFunction::SlidePath);
            if !declared_slide && !slide_supported {
                continue;
            }
            // The observer's route: next-note links, steps of 1–2 semitones in one direction,
            // until a chord tone of its own harmony (the slide rule in `temporal.rs`).
            let mut route = vec![i];
            let mut at = row;
            let mut dir = 0;
            while let Some(j) = at.next_note {
                let b = &score.notes[j];
                let d = b.pitch - score.notes[*route.last().unwrap_or(&i)].pitch;
                if !(1..=2).contains(&d.abs())
                    || (dir != 0 && d.signum() != dir)
                    || b.start_beat > n.start_beat + 2.0
                {
                    break;
                }
                dir = d.signum();
                route.push(j);
                if ctx_tone(j) {
                    break;
                }
                match observer.rows.iter().find(|r| r.note_index == j) {
                    Some(r) => at = r,
                    None => break,
                }
            }
            let events: Vec<PathEvent> = route
                .iter()
                .map(|&k| PathEvent::of(&score.notes[k]))
                .collect();
            let attacks = events.iter().filter(|e| e.entry == Entry::Attacked).count();
            let gesture = classify(&events);
            let slide_dependent = declared_slide
                || (slide_supported
                    && row.supported.iter().all(|&f| f == PitchFunction::SlidePath));
            let fake = slide_dependent && gesture != Some(Gesture::Slide);
            out.declared_slides += declared_slide as usize;
            out.slide_supported += slide_supported as usize;
            out.slide_dependent += slide_dependent as usize;
            out.fake_slides += fake as usize;
            match gesture {
                Some(Gesture::Slide) => out.physical_slides += 1,
                Some(Gesture::ChromaticApproach) => out.chromatic_approaches += 1,
                Some(Gesture::DiscreteLine) => {
                    out.discrete_lines += 1;
                    let chromatic_inner = events
                        .windows(2)
                        .filter(|w| (w[1].pitch - w[0].pitch).abs() == 1)
                        .count();
                    if chromatic_inner >= 3 {
                        out.chromatic_staircases += 1;
                    }
                }
                None => {}
            }
            out.rows.push(GestureRow {
                note: i,
                route,
                declared_slide,
                slide_supported,
                slide_dependent,
                attacks,
                gesture,
                fake,
            });
        }
        out
    }

    /// Counts, then every row: route pitches, onsets, inter-onset intervals, lengths, attacks,
    /// declared functions, the observer's support, and the physical gesture.
    pub fn report(&self, score: &Score) -> String {
        let spb = 60.0 / f64::from(score.tempo_bpm.max(1.0));
        let mut out = String::new();
        let _ = writeln!(
            out,
            "gesture: declared slides {} | slide-supported {} | slide-dependent {} | FAKE SLIDES {} | physical slides {} | chromatic approaches {} | discrete lines {} (chromatic staircases {})",
            self.declared_slides,
            self.slide_supported,
            self.slide_dependent,
            self.fake_slides,
            self.physical_slides,
            self.chromatic_approaches,
            self.discrete_lines,
            self.chromatic_staircases
        );
        for r in &self.rows {
            let n = &score.notes[r.note];
            let pitches: Vec<String> = r
                .route
                .iter()
                .map(|&k| note_name(score.notes[k].pitch))
                .collect();
            let onsets: Vec<String> = r
                .route
                .iter()
                .map(|&k| format!("{:.3}", score.notes[k].start_beat))
                .collect();
            let iois: Vec<String> = r
                .route
                .windows(2)
                .map(|w| {
                    let d = score.notes[w[1]].start_beat - score.notes[w[0]].start_beat;
                    format!("{d:.3}b/{:.3}s", d * spb)
                })
                .collect();
            let durs: Vec<String> = r
                .route
                .iter()
                .map(|&k| {
                    let d = f64::from(score.notes[k].dur_beats);
                    format!("{d:.3}b/{:.3}s", d * spb)
                })
                .collect();
            let declared: Vec<String> = r
                .route
                .iter()
                .map(|&k| {
                    score.notes[k]
                        .function
                        .map_or("none", |f| f.label())
                        .to_string()
                })
                .collect();
            let _ = writeln!(
                out,
                "{:<4} {:>7.3} b {:>7.3} s  route {}  onsets [{}]  IOI [{}]  dur [{}]  attacks {}/{}  declared [{}]  slide {}{}{}  physical {}{}",
                n.role.label(),
                n.start_beat,
                n.start_beat * spb,
                pitches.join("-"),
                onsets.join(" "),
                iois.join(" "),
                durs.join(" "),
                r.attacks,
                r.route.len(),
                declared.join(" "),
                if r.declared_slide { "declared " } else { "" },
                if r.slide_supported { "supported" } else { "" },
                if r.slide_dependent { " DEPENDENT" } else { " (redundant)" },
                r.gesture.map_or("not a stepwise route", |g| g.label()),
                if r.fake { "  FAKE SLIDE" } else { "" }
            );
        }
        out
    }
}
