#![cfg(test)]
//! **Round X meaning probes** — the claim under test: *the song has identity but no listener
//! model*. Round IX made one song upstream of every performance; the listen said the band finally
//! plays one song, and that the song is written in a language the listener does not understand.
//!
//! The round starts from three witnesses, each asserting a defect AS IT STANDS on the Round IX tip
//! (d99ebff), on the Round IX composer (`SongMap::build`, kept unchanged as the control):
//! the composer's content is blind to what the story means, the listener hears the thesis
//! developed before it is ever stated, and the harmony raises its first expectation before home
//! has been heard.

use super::backbone::{ChartCell, ChartRoot};
use super::composer::{compose_meaning, Composer, CompositionalPrior, ThemeProfile};
use super::contract::CompositionGrammar;
use super::diagnostics::RealizationDiagnostics;
use super::discourse::DiscourseRole;
use super::functor::perform;
use super::language::MusicalLanguage;
use super::meaning::{
    Close, Commutation, Lane, Level, MeaningKind as K, MeaningPlan, Owner, Witness,
};
use super::motif::{Handoff, Motif, MotifBank};
use super::performance::PerformanceOptions;
use super::semantic::{
    calm_loop, deflected_lift_trace, demo_trace, false_climax, rise_unresolved, EventKind,
    SemanticTrace,
};
use super::song::{SongMap, SongMapConformance};
use super::theory::Quality;
use super::world::MusicWorld;

const SEED: u64 = 2112;

fn r9_song(trace: &SemanticTrace) -> SongMap {
    SongMap::build(trace, SEED, Some(CompositionGrammar::DeflectedLift))
}

/// The story's peak semantic pressure and whether a release (a confirmation or a resolved
/// section) follows that peak — the two things the stock traces most plainly disagree on.
fn peak_and_release(trace: &SemanticTrace) -> (f32, bool) {
    let (i, peak) = trace
        .events
        .iter()
        .enumerate()
        .map(|(i, e)| (i, e.state.pressure()))
        .fold((0, 0.0f32), |a, b| if b.1 > a.1 { b } else { a });
    let released = trace.events[i + 1..]
        .iter()
        .any(|e| matches!(e.kind, EventKind::Confirmation | EventKind::SectionResolved));
    (peak, released)
}

/// WITNESS 1 — the composer is meaning-blind. A calm loop (no salient event, pressure never above
/// Info) and a rise that ends on an unreleased danger impact are materially different stories; at
/// the same seed, grammar and frame the Round IX composer gives them the same germ and the same
/// chart. (The goal is not that every trace must differ in notes — it is that nothing the story
/// means reaches the content at all.)
#[test]
fn witness_the_composer_is_meaning_blind() {
    let (calm, rise) = (calm_loop(120.0), rise_unresolved(120.0));
    let (calm_peak, calm_released) = peak_and_release(&calm);
    let (rise_peak, rise_released) = peak_and_release(&rise);
    assert!(calm_peak < 0.5, "calm peaks at {calm_peak}");
    assert!(rise_peak > 0.85, "rise peaks at {rise_peak}");
    assert!(!rise_released, "rise must never release its peak");
    let _ = calm_released;

    let (a, b) = (r9_song(&calm), r9_song(&rise));
    assert_eq!(
        a.thematic.bank, b.thematic.bank,
        "the germ ignores the story"
    );
    assert_eq!(
        a.harmonic.map(|h| h.cell),
        b.harmonic.map(|h| h.cell),
        "the chart ignores the story"
    );
}

/// WITNESS 2 — development before exposition. The flagship's first lead statement is a
/// transposition of a thesis nobody has heard; the thesis itself is first stated at the last lead
/// site (phrase 7 of 9), and two stock stories never state it at all. The culminating "hook" is a
/// third object: neither the thesis nor the bank's own hook.
#[test]
fn witness_development_before_exposition() {
    let s = r9_song(&deflected_lift_trace(120.0));
    let thesis = &s.thematic.bank.identity;
    let first = &s.thematic.sites[0];
    assert_eq!(first.role, DiscourseRole::Depart);
    assert_eq!(first.motif.degrees, vec![2, 6, 5, 7, 4]);
    assert_eq!(
        first.motif,
        thesis.transpose(2),
        "a development of the unheard thesis"
    );
    let literal: Vec<u32> = s
        .thematic
        .sites
        .iter()
        .filter(|x| &x.motif == thesis)
        .map(|x| x.phrase)
        .collect();
    assert_eq!(literal, vec![7], "the thesis is stated once, last");
    assert_eq!(s.thematic.sites.len(), 6);

    for t in [calm_loop(120.0), rise_unresolved(120.0)] {
        let s = r9_song(&t);
        assert!(
            s.thematic
                .sites
                .iter()
                .all(|x| x.motif != s.thematic.bank.identity),
            "this story never states its thesis"
        );
    }

    let hook = s
        .thematic
        .sites
        .iter()
        .find(|x| x.role == DiscourseRole::Culminate)
        .unwrap();
    assert_eq!(hook.motif.degrees, vec![0, 4, 4, 6, 3]);
    assert_ne!(&hook.motif, thesis);
    assert_ne!(
        hook.motif, s.thematic.bank.hook,
        "the bank's hook is never stated"
    );
}

/// WITNESS 3 — expectation before home. The flagship's chart opens on its lift, so the pointer and
/// the first deflection (beat 4) sound before the tonic ever has (beat 12); the deflect and open
/// satellites collapse onto their anchors (16-beat pedals); and home is the LEAST-heard of the
/// three anchors the song sits on — the ear is asked to feel a miss of a home it was never given.
#[test]
fn witness_expectation_before_home() {
    let s = r9_song(&deflected_lift_trace(120.0));
    let cell = s.harmonic.unwrap().cell;
    assert_ne!(cell.lift, cell.reset, "the song opens away from home");
    assert_eq!(cell.satellites[0], cell.deflect, "deflect pedal");
    assert_eq!(cell.satellites[1], cell.open, "open pedal");

    // As charted, in C (SWISS_SIGNAL, the plain idiom — no rhythm transform, no colour).
    let simple = PerformanceOptions {
        language: MusicalLanguage::simple(),
        ..PerformanceOptions::default()
    };
    let c = perform(&s, &MusicWorld::swiss_signal(), simple);
    let first = |pc: i32| {
        c.score
            .chords
            .iter()
            .find(|sp| sp.chord.root_pc == pc)
            .map(|sp| sp.start_beat)
            .unwrap()
    };
    let (tonic, deflect) = (first(0), first(9));
    assert!(
        deflect < tonic,
        "the first miss (beat {deflect}) precedes home (beat {tonic})"
    );
    let heard = |pc: i32| -> f64 {
        c.score
            .chords
            .iter()
            .filter(|sp| sp.chord.root_pc == pc)
            .map(|sp| sp.dur_beats as f64)
            .sum()
    };
    let (i, vi, iv) = (heard(0), heard(9), heard(5));
    assert!(
        i < vi && i < iv,
        "home is the least-heard anchor: I {i} vi {vi} IV {iv} beats"
    );
}

// ---------------------------------------------------------------------------------------------
// The instrument (F, μ, the law), calibrated BEFORE any composer exists: the Round IX song must
// fail it for the reasons the witnesses name, a hand-written song must be able to pass it, and
// every scrambled version of that song must fail it for its own exact reason.

/// The divergences as `(lane, at, wanted, heard)`, in order.
fn divergent(c: &Commutation) -> Vec<(Lane, u32, Option<K>, Option<K>)> {
    c.divergences
        .iter()
        .map(|d| (d.lane, d.at, d.wanted, d.heard))
        .collect()
}

/// A thesis written by hand for the calibration: a pickup, a two-note cell stated and sequenced a
/// step up, one reach of a fourth, a step back, a landing on the third. Six beats.
fn hand_thesis() -> Motif {
    Motif {
        id: 0,
        degrees: vec![-1, 0, 1, 1, 2, 5, 4, 2],
        rhythm: vec![0.5, 0.5, 0.5, 0.5, 0.5, 1.0, 0.5, 2.0],
    }
}

/// Its consequent: the same head and rhythm, stepping down to rest on the tonic.
fn hand_answer() -> Motif {
    Motif {
        degrees: vec![-1, 0, 1, 1, 2, 3, 2, 0],
        ..hand_thesis()
    }
}

/// A chart written by hand: home first (I), the pointer (V7), the textbook miss (vi) moving to
/// ii, the open window (IV), home (I) with its plagal neighbour.
fn hand_chart() -> ChartCell {
    ChartCell {
        lift: ChartRoot::Degree(0),
        lift_alt: ChartRoot::Degree(1),
        pointer: ChartRoot::Degree(4),
        expected: ChartRoot::Degree(0),
        deflect: ChartRoot::Degree(5),
        open: ChartRoot::Degree(3),
        reset: ChartRoot::Degree(0),
        satellites: [
            ChartRoot::Degree(1),
            ChartRoot::Degree(3),
            ChartRoot::Degree(3),
        ],
    }
}

/// The flagship's form with the hand-written theme and chart: the thesis taught first, paid off at
/// every culmination, answered by its consequent, recognized on return.
fn hand_song() -> SongMap {
    let mut s = r9_song(&deflected_lift_trace(120.0));
    let (thesis, answer) = (hand_thesis(), hand_answer());
    s.thematic.bank = MotifBank {
        identity: thesis.clone(),
        hook: thesis.clone(),
        rhythmic_cell: thesis.fragment(3).scale_rhythm(0.5),
        bass_cell: thesis.fragment(2).transpose(-7),
        countermotif: Some(thesis.invert()),
    };
    let mut first = true;
    for site in &mut s.thematic.sites {
        let (motif, handoff) = match site.role {
            _ if first => (thesis.clone(), Handoff::Restatement),
            DiscourseRole::Culminate => (thesis.clone(), Handoff::Hook),
            DiscourseRole::Answer => (answer.clone(), Handoff::Response),
            _ => (thesis.clone(), Handoff::Restatement),
        };
        first = false;
        site.motif = motif;
        site.handoff = handoff;
    }
    s.harmonic.as_mut().unwrap().cell = hand_chart();
    s
}

/// The four acceptance performances of `song` all conform to it (π holds: a valid, playable song).
fn plays(song: &SongMap) -> bool {
    let fusion = PerformanceOptions::default();
    let simple = PerformanceOptions {
        language: MusicalLanguage::simple(),
        ..fusion
    };
    [
        (MusicWorld::black_ice(), fusion),
        (MusicWorld::vapor95(), fusion),
        (MusicWorld::swiss_signal(), fusion),
        (MusicWorld::black_ice(), simple),
    ]
    .iter()
    .all(|(w, o)| {
        let c = perform(song, w, *o);
        let r = SongMapConformance::check(song, &c.perf, &c.score);
        assert!(r.passes(), "{}", r.report());
        (c.score.total_beats - song.plan.form.total_beats).abs() < 1e-9
    })
}

/// The Round IX flagship does not mean its story, and for exactly the reasons the witnesses name:
/// every theme site hears material before it was taught (the thesis is first LEARNED at the
/// return that was meant to be recognized), and the first pointer sounds before home. The miss
/// itself is right — V7 to vi, the textbook deceptive arrival, a mid surprise for a mid arc.
#[test]
fn the_r9_song_does_not_mean_its_story() {
    let s = r9_song(&deflected_lift_trace(120.0));
    let c = Commutation::check(&s);
    assert_eq!(c.target.arc, Level::Mid);
    assert_eq!(c.target.resolution, Close::Home);
    assert!(c.checked >= 20, "non-vacuous: {} events", c.checked);
    use Lane::*;
    assert_eq!(
        divergent(&c),
        vec![
            (Theme, 1, Some(K::Learn), Some(K::Premature)),
            (Theme, 2, Some(K::Payoff), Some(K::Premature)),
            (Theme, 3, Some(K::Answer(Close::Home)), Some(K::Premature)),
            (Theme, 5, Some(K::Payoff), Some(K::Premature)),
            (Theme, 6, Some(K::Answer(Close::Home)), Some(K::Premature)),
            (Theme, 7, Some(K::Recognize), Some(K::Learn)),
            (Harmony, 0, Some(K::Establish), Some(K::Unestablished)),
        ],
        "{}",
        c.report()
    );
    assert!(c.divergences.iter().all(|d| d.owner == Owner::Composer));
    assert_eq!(c.observed.count(K::Miss(Level::Mid)), 3);
}

/// Calibration: the instrument CAN pass. A hand-written theme and chart on the same form mean
/// exactly what the story asks — and the song is a valid SongMap every band plays (π holds).
#[test]
fn a_hand_written_song_can_mean_its_story() {
    let s = hand_song();
    let c = Commutation::check(&s);
    assert!(c.commutes(), "{}", c.report());
    assert_eq!(c.observed.count(K::Payoff), 2);
    assert_eq!(c.observed.count(K::Recognize), 1);
    assert_eq!(c.observed.count(K::Miss(Level::Mid)), 3);
    assert!(plays(&s));
}

/// Scrambled expectation structure is caught, each for its own reason — and "surprise
/// everywhere" is a VALID song (every band plays it, π holds) that the listener model refuses.
#[test]
fn scrambled_meaning_breaks_the_law() {
    use Lane::*;
    let thesis = hand_thesis();

    // Premature development: the first thing heard is the thesis inverted and moved.
    let mut s = hand_song();
    s.thematic.sites[0].motif = thesis.invert().transpose(3);
    let c = Commutation::check(&s);
    assert_eq!(
        divergent(&c)[..2],
        [
            (Theme, 1, Some(K::Learn), Some(K::Premature)),
            (Theme, 2, Some(K::Payoff), Some(K::Learn)),
        ]
    );

    // A payoff with no setup: the first statement is the consequent, so the culmination is
    // where the thesis is first heard.
    let mut s = hand_song();
    s.thematic.sites[0].motif = hand_answer();
    let c = Commutation::check(&s);
    assert!(divergent(&c).contains(&(Theme, 2, Some(K::Payoff), Some(K::Learn))));

    // The return of something never learned: every statement before the restatement developed.
    let mut s = hand_song();
    let last = s.thematic.sites.len() - 1;
    for site in &mut s.thematic.sites[..last] {
        site.motif = site.motif.transpose(2);
    }
    let c = Commutation::check(&s);
    assert!(divergent(&c).contains(&(Theme, 7, Some(K::Recognize), Some(K::Learn))));

    // No miss at all: the deflection lands the expected arrival.
    let mut s = hand_song();
    s.harmonic.as_mut().unwrap().cell.deflect = ChartRoot::Degree(0);
    let c = Commutation::check(&s);
    assert_eq!(c.observed.count(K::Arrive), 3);
    assert!(!c.commutes());

    // Surprise everywhere: every slot remote. Lawful, playable — and meaningless to the model.
    let mut s = hand_song();
    let chrom = |semitones, quality| ChartRoot::Chromatic { semitones, quality };
    s.harmonic.as_mut().unwrap().cell = ChartCell {
        lift: chrom(6, Quality::Maj),
        lift_alt: chrom(1, Quality::Maj),
        deflect: chrom(1, Quality::Maj),
        open: chrom(11, Quality::Maj),
        satellites: [
            chrom(6, Quality::Min),
            chrom(10, Quality::Maj),
            chrom(3, Quality::Maj),
        ],
        ..hand_chart()
    };
    let c = Commutation::check(&s);
    assert_eq!(c.observed.count(K::Unestablished), 1);
    assert_eq!(c.observed.count(K::Unrelated), 3);
    assert!(c.observed.count(K::Stray) >= 3, "{}", c.report());
    assert!(c.observed.count(K::NoRelief) + c.observed.count(K::NoHome) >= 3);
    assert!(plays(&s), "a valid song every band can play");

    // A song written for one arc does not mean another: the same content under a calm story.
    let mut s = hand_song();
    s.trace = calm_loop(120.0);
    let c = Commutation::check(&s);
    assert!(divergent(&c).contains(&(
        Theme,
        1,
        Some(K::Thesis(Level::Low)),
        Some(K::Thesis(Level::Mid))
    )));
    assert_eq!(
        c.divergences
            .iter()
            .filter(|d| d.wanted == Some(K::Miss(Level::Low)))
            .count(),
        3
    );
}

/// The meaning-blind composer, in the instrument's terms: F separates the calm story from the
/// unresolved rise (arc and resolution), and the Round IX songs for them observe the SAME thesis
/// reach and the SAME miss — so they cannot both mean their stories.
#[test]
fn the_r9_composer_cannot_mean_two_stories() {
    let (calm, rise) = (calm_loop(120.0), rise_unresolved(120.0));
    let (a, b) = (r9_song(&calm), r9_song(&rise));
    let (fa, fb) = (
        MeaningPlan::target(&a.trace, &a.plan),
        MeaningPlan::target(&b.trace, &b.plan),
    );
    assert_eq!((fa.arc, fa.resolution), (Level::Low, Close::Home));
    assert_eq!((fb.arc, fb.resolution), (Level::High, Close::Open));
    let content = |s: &SongMap| {
        let o = MeaningPlan::observe(s);
        o.events
            .iter()
            .map(|w| w.event.kind)
            .filter(|k| matches!(k, K::Thesis(_) | K::Miss(_)))
            .map(|k| format!("{k:?}"))
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(content(&a), content(&b), "one content for two stories");
    let (ca, cb) = (Commutation::check(&a), Commutation::check(&b));
    let content_miss = |c: &Commutation| {
        c.divergences
            .iter()
            .filter(|d| matches!(d.wanted, Some(K::Thesis(_)) | Some(K::Miss(_))))
            .count()
    };
    assert!(content_miss(&ca) > 0 && content_miss(&cb) > 0);
}

/// Every stock story: F is non-vacuous, the Round IX composer never commutes, and where the Round
/// IX FORM itself cannot carry the plan (false_climax: two deflections placed with no Lift before
/// them) the divergence is the form's, not the composer's. Exact counts: the control is frozen.
#[test]
fn the_instrument_reads_every_story() {
    let expect = [
        ("bounce", deflected_lift_trace(120.0), 7, 0),
        ("demo", demo_trace(120.0), 11, 0),
        ("calm", calm_loop(120.0), 15, 0),
        ("rise", rise_unresolved(120.0), 10, 0),
        ("false_climax", false_climax(120.0), 12, 2),
    ];
    for (name, t, divergent, form_owned) in expect {
        let c = Commutation::check(&r9_song(&t));
        assert!(c.checked >= 10, "{name}: {} events", c.checked);
        let form = c
            .divergences
            .iter()
            .filter(|d| d.owner == Owner::Form)
            .count();
        assert_eq!(
            (c.divergences.len(), form),
            (divergent, form_owned),
            "{name}\n{}",
            c.report()
        );
    }
}

// ---------------------------------------------------------------------------------------------
// The meaning-directed composer (Round X): the same form as the control, a different page.

fn r10_song(trace: &SemanticTrace) -> SongMap {
    SongMap::compose(
        trace,
        SEED,
        Some(CompositionGrammar::DeflectedLift),
        Composer::MeaningDirected,
    )
}

/// The Round IX composer is the control and stays byte-for-byte what it was; the meaning-directed
/// song shares its form exactly (trace, timeline, contract, phrases, roles, seats, slots), so an
/// A/B between them changes only the song's content.
#[test]
fn the_control_is_untouched_and_shares_the_form() {
    let t = deflected_lift_trace(120.0);
    let a = r9_song(&t);
    assert_eq!(
        a.fingerprint(),
        0xf4d4_3f1c_1da2_ac91,
        "the Round IX flagship"
    );
    let a2 = SongMap::compose(
        &t,
        SEED,
        Some(CompositionGrammar::DeflectedLift),
        Composer::StructuralR9,
    );
    assert_eq!(a2.fingerprint(), a.fingerprint());
    assert_eq!(
        (a.composer(), a.meaning.is_some()),
        (Composer::StructuralR9, false)
    );

    let b = r10_song(&t);
    assert_eq!(b.composer(), Composer::MeaningDirected);
    assert_eq!(format!("{:?}", a.plan), format!("{:?}", b.plan), "one form");
    assert_eq!(format!("{:?}", a.timeline), format!("{:?}", b.timeline));
    assert_eq!(
        a.harmonic.map(|h| h.bars_per_chord),
        b.harmonic.map(|h| h.bars_per_chord)
    );
    assert_ne!(a.thematic, b.thematic, "a different theme");
    assert_ne!(a.harmonic, b.harmonic, "a different chart");
    assert_ne!(a.fingerprint(), b.fingerprint());
}

/// The acceptance song: the flagship story composed toward its MeaningPlan. It means what the story
/// asks on all 20 modelled events; every band plays it (π: one song, four distinct performances and
/// scores, all conforming) with no melody repaired or re-judged, no unjustified note, exact length.
#[test]
fn the_meaning_directed_flagship() {
    let r9 = r9_song(&deflected_lift_trace(120.0));
    let (song, report) = compose_meaning(r9, &CompositionalPrior::HOOKY_FUSION);
    assert_eq!(song.fingerprint(), 0x2c46_6177_1531_4b76);

    // The chart: home first; V7; the textbook miss (vi) moving to ii; the open window (IV);
    // home with its plagal neighbour. Chosen from 117 lawful journeys: 6 mean the plan.
    let cell = song.harmonic.unwrap().cell;
    let d = ChartRoot::Degree;
    assert_eq!(
        (cell.lift, cell.pointer, cell.deflect, cell.open, cell.reset),
        (d(0), d(4), d(5), d(3), d(0))
    );
    assert_eq!(cell.satellites, [d(1), d(3), d(3)]);
    let survivors: Vec<usize> = report.chart_stages.iter().map(|s| s.1).collect();
    assert_eq!(
        survivors,
        vec![117, 6, 5, 5, 2, 1],
        "{:?}",
        report.chart_stages
    );

    // The theme: a pickup, a two-note cell stated twice in place, one reach of a fourth, a step
    // back, a landing on the third; its consequent settles on the tonic. 72 lawful lines; 24 reach
    // as far as the arc; 12 are typical for the prior; 4 sit best on the chart; the seed picks.
    let chosen = &report.themes[report.theme];
    assert_eq!(chosen.thesis.degrees, vec![3, 4, 5, 4, 5, 8, 7, 9]);
    assert_eq!(
        chosen.thesis.rhythm,
        vec![0.5, 0.5, 0.5, 0.5, 0.5, 1.0, 0.5, 2.0]
    );
    assert_eq!(chosen.answer.degrees, vec![3, 4, 5, 4, 5, 6, 5, 7]);
    let survivors: Vec<usize> = report.theme_stages.iter().map(|s| s.1).collect();
    assert_eq!(
        survivors,
        vec![72, 72, 24, 24, 12, 4, 1],
        "{:?}",
        report.theme_stages
    );
    assert!(chosen.profile.unusual(&report.prior).is_empty());

    // The listener plan, met: learned first, paid off, answered, recognized; home before the
    // first pointer; three prepared, related, mid misses — each heard with HIGH certainty.
    let c = Commutation::check(&song);
    assert!(c.commutes(), "{}", c.report());
    assert_eq!(c.checked, 20);
    let certain = c
        .observed
        .events
        .iter()
        .filter(|w| w.event.kind == K::Miss(Level::Mid))
        .all(|w| {
            matches!(
                w.witness,
                Witness::Move {
                    certainty: Level::High,
                    ..
                }
            )
        });
    assert!(certain, "every miss follows home and a strong pointer");
    assert_eq!(c.observed.count(K::Stray), 0);

    // π: one song, four performances.
    let fusion = PerformanceOptions::default();
    let simple = PerformanceOptions {
        language: MusicalLanguage::simple(),
        ..fusion
    };
    let mut perf_prints = Vec::new();
    let mut score_prints = Vec::new();
    for (w, o) in [
        (MusicWorld::black_ice(), fusion),
        (MusicWorld::vapor95(), fusion),
        (MusicWorld::swiss_signal(), fusion),
        (MusicWorld::black_ice(), simple),
    ] {
        let p = perform(&song, &w, o);
        let r = SongMapConformance::check(&song, &p.perf, &p.score);
        assert!(r.passes(), "{}", r.report());
        assert_eq!(r.song, song.fingerprint());
        assert_eq!((p.score.melody_repairs, p.score.melody_rejudged), (0, 0));
        let rd = RealizationDiagnostics::measure(&song.plan, &p.score);
        assert!(rd.unjustified_by_role.iter().all(|x| x.1 == 0));
        assert!((p.score.total_beats - 120.0).abs() < 1e-9);
        perf_prints.push(p.perf.fingerprint());
        score_prints.push(p.score.fingerprint());
    }
    perf_prints.sort();
    perf_prints.dedup();
    score_prints.sort();
    score_prints.dedup();
    assert_eq!((perf_prints.len(), score_prints.len()), (4, 4));
}

/// WITNESS 1, flipped: the composer now hears the story. Calm (a low arc, settled) gets a thesis
/// with no salient reach and a soft miss (V7→iii, which keeps two tones of home AND prolongs the
/// pointer); the unresolved rise gets a wide reach, a hard miss (V7→IV) and a consequent left
/// hanging. Each means its own story; neither means the other's.
#[test]
fn the_composer_now_hears_the_story() {
    let (calm, rise) = (
        r10_song(&calm_loop(120.0)),
        r10_song(&rise_unresolved(120.0)),
    );
    assert_ne!(calm.thematic.bank, rise.thematic.bank);
    let (cc, rc) = (calm.harmonic.unwrap().cell, rise.harmonic.unwrap().cell);
    assert_eq!(
        (cc.deflect, rc.deflect),
        (ChartRoot::Degree(2), ChartRoot::Degree(3))
    );
    for s in [&calm, &rise] {
        assert_eq!(Commutation::check(s).composer_divergences(), 0);
    }
    let o = |s: &SongMap, k: K| MeaningPlan::observe(s).count(k);
    assert_eq!(o(&calm, K::Thesis(Level::Low)), 1);
    assert_eq!(o(&rise, K::Thesis(Level::High)), 1);
    assert!(o(&calm, K::Miss(Level::Low)) > 0 && o(&calm, K::Miss(Level::High)) == 0);
    assert!(o(&rise, K::Miss(Level::High)) > 0 && o(&rise, K::Miss(Level::Low)) == 0);
    assert!(o(&rise, K::Answer(Close::Open)) > 0 && o(&rise, K::Answer(Close::Home)) == 0);

    // Swap the stories: the calm page under the rise's story (and back) no longer commutes.
    let mut swapped = calm.clone();
    swapped.trace = rise_unresolved(120.0);
    assert!(Commutation::check(&swapped).composer_divergences() > 0);
    let mut swapped = rise.clone();
    swapped.trace = calm_loop(120.0);
    assert!(Commutation::check(&swapped).composer_divergences() > 0);
}

/// The alien melody: lawful (it fits, it is long enough, it is playable by every band — π holds),
/// yet the prior names exactly why it is unusual, where the composer's thesis is typical. The
/// machine says "unusual / low-prior", never "bad".
#[test]
fn the_prior_says_why_a_melody_is_unusual() {
    let prior = CompositionalPrior::HOOKY_FUSION;
    let alien = Motif {
        id: 0,
        degrees: vec![0, 5, -1, 4, -2, 6, 1],
        rhythm: vec![0.75, 0.25, 1.25, 0.5, 1.0, 0.75, 1.5],
    };
    let p = ThemeProfile::of(&alien);
    assert_eq!(p.unlawful(&prior), None, "a lawful motif");
    assert_eq!(
        p.unusual(&prior),
        vec![
            "mostly leaps",
            "more than one salient leap",
            "a leap left unrecovered",
            "no recurring rhythmic cell",
            "displacement outside the band",
            "lands off the tonic triad",
        ]
    );
    let (_, report) = compose_meaning(r9_song(&deflected_lift_trace(120.0)), &prior);
    assert!(report.themes[report.theme]
        .profile
        .unusual(&prior)
        .is_empty());

    // A song built on it is valid and every band plays it; the listener plan notices the reach.
    let mut s = hand_song();
    s.thematic.bank.identity = alien.clone();
    for site in &mut s.thematic.sites {
        if site.motif == hand_thesis() {
            site.motif = alien.clone();
        }
    }
    assert!(plays(&s));
    let c = Commutation::check(&s);
    assert!(divergent(&c).contains(&(
        Lane::Theme,
        1,
        Some(K::Thesis(Level::Mid)),
        Some(K::Thesis(Level::High))
    )));
}

/// Groove extremes: the chosen thesis's degrees under a low, the chosen (medium) and an extreme
/// displacement are all lawful — the prior aims at the middle band; it does not outlaw the rest
/// (the grammar's dotted cells stay in the candidate set, dropped only as atypical).
#[test]
fn groove_extremes_are_lawful_and_the_prior_aims_between() {
    let prior = CompositionalPrior::HOOKY_FUSION;
    let (_, report) = compose_meaning(r9_song(&deflected_lift_trace(120.0)), &prior);
    let thesis = report.themes[report.theme].thesis.clone();
    assert_eq!(thesis.len(), 8);
    let with = |rhythm: Vec<f32>| Motif {
        rhythm,
        ..thesis.clone()
    };
    let low = with(vec![0.5, 0.5, 0.5, 0.5, 1.0, 0.5, 0.5, 2.0]);
    let extreme = with(vec![0.75, 0.25, 0.75, 0.25, 0.75, 1.25, 0.5, 1.5]);
    let levels: Vec<Level> = [&low, &thesis, &extreme]
        .iter()
        .map(|m| {
            let p = ThemeProfile::of(m);
            assert_eq!(p.unlawful(&prior), None);
            p.displacement
        })
        .collect();
    assert_eq!(levels, vec![Level::Low, Level::Mid, Level::High]);
    assert_eq!(prior.displacement, Level::Mid);
    assert!(report.themes.iter().any(|c| c.params.long_short
        && c.profile.displacement == Level::High
        && c.dropped_by == Some("typical for the prior")));
}

/// Across songs: every stock story (and the modulating impact trace) at three lengths, composed
/// toward its plan — every song means it wherever its form lets it (no composer-owned divergence;
/// the form-owned ones are the Round IX form's, identical to the control's), every performance plays the song (π identity),
/// nothing repaired, re-judged or unjustified. The inherited obligation-witness gap is unchanged:
/// the same 60 of 72 performances as the Round IX control (a performance-side defect, PARKED).
#[test]
fn the_meaning_composer_across_songs() {
    use super::semantic::{Density, Elevation, Emphasis, SemanticEvent, SemanticState, Tone};
    let e = |at: f64, tone: Tone, kind: EventKind| SemanticEvent {
        at_beat: at,
        state: SemanticState {
            tone,
            emphasis: Emphasis::Normal,
            density: Density::Normal,
            elevation: Elevation::Raised,
        },
        kind,
    };
    let impact = |beats: f64| {
        SemanticTrace::new(
            vec![
                e(16.0, Tone::Info, EventKind::FocusAcquired),
                e(32.0, Tone::Danger, EventKind::Impact),
                e(48.0, Tone::Success, EventKind::Confirmation),
                e(64.0, Tone::Neutral, EventKind::SectionResolved),
            ],
            beats,
        )
    };
    let stories: [(&str, &dyn Fn(f64) -> SemanticTrace); 6] = [
        ("bounce", &deflected_lift_trace),
        ("demo", &demo_trace),
        ("rise", &rise_unresolved),
        ("false_climax", &false_climax),
        ("calm", &calm_loop),
        ("impact", &impact),
    ];
    let fusion = PerformanceOptions::default();
    let simple = PerformanceOptions {
        language: MusicalLanguage::simple(),
        ..fusion
    };
    let (mut performances, mut unwitnessed, mut form_owned) = (0, 0, 0);
    for (name, story) in stories {
        for beats in [80.0, 120.0, 160.0] {
            let song = r10_song(&story(beats));
            let c = Commutation::check(&song);
            assert_eq!(
                c.composer_divergences(),
                0,
                "{name} {beats}\n{}",
                c.report()
            );
            // What remains is the form's, and exactly the control's (the form is shared).
            let form = |c: &Commutation| {
                c.divergences
                    .iter()
                    .filter(|d| d.owner == Owner::Form)
                    .map(|d| (d.at, d.wanted, d.heard))
                    .collect::<Vec<_>>()
            };
            assert_eq!(form(&c), form(&Commutation::check(&r9_song(&story(beats)))));
            form_owned += c.divergences.len();
            for (w, o) in [
                (MusicWorld::black_ice(), fusion),
                (MusicWorld::vapor95(), fusion),
                (MusicWorld::swiss_signal(), fusion),
                (MusicWorld::black_ice(), simple),
            ] {
                let p = perform(&song, &w, o);
                let r = SongMapConformance::check(&song, &p.perf, &p.score);
                let identity = r.song == r.performed
                    && !r.bank_mismatch
                    && r.missing_theme_sites.is_empty()
                    && r.wrong_harmonic_landmarks.is_empty()
                    && r.illegal_harmonic_transforms.is_empty()
                    && r.form_mismatch.is_empty();
                assert!(identity, "{name} {beats} {}: {}", w.name, r.report());
                assert_eq!((p.score.melody_repairs, p.score.melody_rejudged), (0, 0));
                let rd = RealizationDiagnostics::measure(&song.plan, &p.score);
                assert!(
                    rd.unjustified_by_role.iter().all(|x| x.1 == 0),
                    "{name} {beats}"
                );
                performances += 1;
                unwitnessed += usize::from(r.unwitnessed_song_obligations > 0);
            }
        }
    }
    assert_eq!(performances, 72);
    assert_eq!(
        unwitnessed, 60,
        "the inherited gap, exactly as the control has it"
    );
    assert_eq!(
        form_owned, 7,
        "false_climax's two unliftable deflects at every length; one in the 80-beat bounce"
    );
}

/// The Round IX checker, recalibrated (an inherited instrument gap Round X's content exposed): a
/// modulation may return home through the span's own last chord when it is diatonic to both
/// regions, and the planner starts home ON it. That chord is the chart's, as the region it leaves
/// transposed it, and is read there. The negative control: any other root at that boundary is
/// still outside the chart.
#[test]
fn a_return_pivot_is_read_in_the_region_it_leaves() {
    let song = r10_song(&demo_trace(120.0));
    let simple = PerformanceOptions {
        language: MusicalLanguage::simple(),
        ..PerformanceOptions::default()
    };
    let mut p = perform(&song, &MusicWorld::black_ice(), simple);
    let at = 80.0;
    let (home, left) = (p.perf.region_at(at), p.perf.region_at(at - 1e-3));
    assert_ne!(home, left, "a region boundary");
    let k = p
        .score
        .chords
        .iter()
        .position(|c| (c.start_beat - at).abs() < 1e-9)
        .unwrap();
    let cell = song.harmonic.unwrap().cell;
    let heard = p.score.chords[k].chord.root_pc;
    assert!(
        [cell.deflect, cell.satellites[0]]
            .iter()
            .any(|r| r.root_pc(&left) == heard),
        "the chart's chord in the region it leaves"
    );
    let r = SongMapConformance::check(&song, &p.perf, &p.score);
    assert!(r.illegal_harmonic_transforms.is_empty(), "{}", r.report());

    let lawful: Vec<i32> = [cell.deflect, cell.satellites[0]]
        .iter()
        .flat_map(|r| [r.root_pc(&home), r.root_pc(&left)])
        .collect();
    let stray = (0..12).find(|pc| !lawful.contains(pc)).unwrap();
    p.score.chords[k].chord.root_pc = stray;
    let r = SongMapConformance::check(&song, &p.perf, &p.score);
    assert_eq!(
        r.illegal_harmonic_transforms
            .iter()
            .map(|t| t.beat)
            .collect::<Vec<_>>(),
        vec![at]
    );
}
