#![cfg(test)]
//! **Round IX song probes** — the claim under test: *the song is one object upstream of every
//! performance*. Same trace, same seed, same grammar (the flagship: [`deflected_lift_trace`]`(120.0)`,
//! forced DeflectedLift, seed 2112); what varies is only the room ([`MusicWorld`]) and the idiom
//! ([`MusicalLanguage`]).
//!
//! The round started from three witnesses, each asserting a defect AS IT STOOD on the R8b tip
//! (0b4483d): the composition plan was identical across worlds, yet the room picked the theme and
//! the chart, and the idiom rewrote the chart's rhythm through a field no song coordinate owned.
//! Each is replaced by the law it witnessed when that law lands (the git history keeps the flip).

use super::backbone::{ChartCell, ChartRoot, HarmonicGesture};
use super::contract::CompositionGrammar;
use super::functor::{perform, Composition};
use super::language::{HarmonicRhythm, MusicalLanguage};
use super::performance::PerformanceOptions;
use super::score::Role;
use super::semantic::deflected_lift_trace;
use super::song::{HarmonicMap, SongMap, SongMapConformance, ThemeSite, CHART_BARS_PER_CHORD};
use super::theory::Mode;
use super::world::MusicWorld;

const SEED: u64 = 2112;

/// The four acceptance performances of ONE song: three rooms speaking the flagship idiom, and the
/// Aeolian room speaking the plain one.
fn acceptance(song: &SongMap) -> [(&'static str, Composition); 4] {
    let fusion = PerformanceOptions::default();
    let simple = PerformanceOptions {
        language: MusicalLanguage::simple(),
        ..fusion
    };
    [
        (
            "BLACK_ICE/fusion",
            perform(song, &MusicWorld::black_ice(), fusion),
        ),
        (
            "VAPOR95/fusion",
            perform(song, &MusicWorld::vapor95(), fusion),
        ),
        (
            "SWISS_SIGNAL/fusion",
            perform(song, &MusicWorld::swiss_signal(), fusion),
        ),
        (
            "BLACK_ICE/simple",
            perform(song, &MusicWorld::black_ice(), simple),
        ),
    ]
}

fn flagship_song() -> SongMap {
    SongMap::build(
        &deflected_lift_trace(120.0),
        SEED,
        Some(CompositionGrammar::DeflectedLift),
    )
}

/// **Law 1 (was witness 1): the theme is the song's.** Every performance states the SongMap's own
/// bank, and at every identity site (the thesis coming home, the hook) states exactly what the song
/// states there — and the lead is heard stating it. On 0b4483d the Aeolian room stated germ id 1
/// `[0,3,6,4,3,1]` while the Ionian rooms stated id 0 `[0,4,3,5,2]`.
#[test]
fn the_theme_is_the_songs() {
    let song = flagship_song();
    let identity: Vec<&ThemeSite> = song
        .thematic
        .sites
        .iter()
        .filter(|s| s.is_identity())
        .collect();
    assert!(
        identity.len() >= 3,
        "the flagship states its thesis and hook"
    );
    for (label, c) in acceptance(&song) {
        assert_eq!(
            c.perf.bank, song.thematic.bank,
            "{label}: the bank is the song's"
        );
        for site in &identity {
            let st = c
                .perf
                .statements
                .iter()
                .find(|st| st.phrase == site.phrase)
                .unwrap_or_else(|| panic!("{label}: phrase {} states nothing", site.phrase));
            assert_eq!(
                st.motif, site.motif,
                "{label}: phrase {} restated",
                site.phrase
            );
            let heard = c
                .score
                .notes
                .iter()
                .filter(|n| n.role == Role::Lead && n.prov.material == Some(st.material))
                .count();
            assert!(heard > 0, "{label}: phrase {} is silent", site.phrase);
        }
        eprintln!(
            "{label}: germ {:?} at {} identity sites",
            c.perf.bank.identity.degrees,
            identity.len()
        );
    }
}

/// **Law 2 (was witness 2): the chart is the song's.** Every room sounds the SAME relational
/// journey: at every slot's downbeat the chart's anchor, and the pointer closing every Lift — each
/// root read in the region in force there. On 0b4483d the Aeolian room searched its own cell and
/// opened on degree 2 (bIII) where the Ionian rooms opened on degree 3 (IV).
#[test]
fn the_chart_is_the_songs() {
    let song = flagship_song();
    let marks = song.landmarks();
    let kinds: std::collections::BTreeSet<&str> = marks.iter().map(|m| m.1).collect();
    assert_eq!(kinds.len(), 5, "every landmark kind is charted: {kinds:?}");
    for (label, c) in acceptance(&song) {
        let mut wrong = Vec::new();
        for &(beat, what, root) in &marks {
            let heard = c
                .score
                .chords
                .iter()
                .find(|sp| {
                    sp.start_beat <= beat + 1e-6 && beat < sp.start_beat + sp.dur_beats as f64
                })
                .map(|sp| sp.chord.root_pc);
            let region = c.perf.region_at(beat);
            if heard != Some(root.root_pc(&region)) {
                wrong.push((beat, what, root, heard));
            }
        }
        eprintln!("{label}: {} landmarks, wrong {:?}", marks.len(), wrong);
        assert!(wrong.is_empty(), "{label}: the room played another journey");
    }
}

/// Chord changes inside the backbone.
fn changes(c: &Composition) -> usize {
    c.score.chords.len()
}

/// **Law 3 (was witness 3): the chart's rhythm is the song's.** The canonical harmonic rhythm is a
/// SongMap coordinate; an idiom applies a declared, typed transform to it (Simple `AsCharted`,
/// Fusion `Diminished`) and both keep every landmark. On 0b4483d the change points came from
/// `MusicalLanguage::harmonic_rhythm_bars` (2 / 1), a number no song coordinate owned.
#[test]
fn the_chart_rhythm_is_the_songs() {
    let song = flagship_song();
    let hm = song.harmonic.expect("DeflectedLift has a chart");
    assert_eq!(hm.bars_per_chord, CHART_BARS_PER_CHORD);
    let world = MusicWorld::black_ice();
    let fusion = PerformanceOptions::default();
    let simple = PerformanceOptions {
        language: MusicalLanguage::simple(),
        ..fusion
    };
    assert_eq!(simple.language.harmonic_rhythm, HarmonicRhythm::AsCharted);
    assert_eq!(fusion.language.harmonic_rhythm, HarmonicRhythm::Diminished);
    let (s, f) = (
        perform(&song, &world, simple),
        perform(&song, &world, fusion),
    );
    for c in [&s, &f] {
        let r = SongMapConformance::check(&song, &c.perf, &c.score);
        assert!(r.passes(), "{}", r.report());
    }
    // The song, not the idiom, owns the rhythm: re-chart it at four bars per chord and the plain
    // idiom follows the song — fewer changes, the same landmarks, still conformant.
    let mut slower = song.clone();
    slower.harmonic = Some(HarmonicMap {
        bars_per_chord: 4,
        ..hm
    });
    let s4 = perform(&slower, &world, simple);
    eprintln!(
        "chord changes: simple {} / fusion {} / simple on the 4-bar chart {}",
        changes(&s),
        changes(&f),
        changes(&s4)
    );
    assert!(changes(&s4) < changes(&s) && changes(&s) < changes(&f));
    let r4 = SongMapConformance::check(&slower, &s4.perf, &s4.score);
    assert!(r4.passes(), "{}", r4.report());
    // ...and the 2-bar performance is NOT a performance of the 4-bar song: its extra changes are
    // off the declared grid (the rhythm check is not vacuous).
    let cross = SongMapConformance::check(&slower, &s.perf, &s.score);
    assert!(
        !cross.illegal_harmonic_transforms.is_empty(),
        "{}",
        cross.report()
    );
}

/// **The acceptance contract.** ONE SongMap, four performances — three rooms speaking the flagship
/// idiom and the Aeolian room speaking the plain one: the song's fingerprint is identical (and
/// rebuilding the song from the same inputs reproduces it), every performance conforms, and the
/// performance plans and the scores all differ. Whether they SOUND like one song is the listen's.
#[test]
fn one_song_four_performances() {
    let song = flagship_song();
    let fp = song.fingerprint();
    assert_eq!(
        flagship_song().fingerprint(),
        fp,
        "the song is deterministic"
    );
    let hm = song.harmonic.expect("DeflectedLift has a chart");
    eprintln!(
        "SongMap {fp:#018x}  ThematicMap {:#018x}  HarmonicMap {:#018x}",
        song.thematic.fingerprint(),
        hm.fingerprint()
    );
    let perfs = acceptance(&song);
    let mut perf_fps = Vec::new();
    let mut score_fps = Vec::new();
    for (label, c) in &perfs {
        assert_eq!(
            c.song.fingerprint(),
            fp,
            "{label}: the song travelled intact"
        );
        let r = SongMapConformance::check(&song, &c.perf, &c.score);
        eprintln!(
            "{label}: perf {:#018x} score {:#018x} | {} {:?} home {}{:?} | {} chords, {} statements, {} actions, {} notes",
            c.perf.fingerprint(),
            c.score.fingerprint(),
            c.perf.language.id.label(),
            c.perf.language.harmonic_rhythm,
            c.perf.region.tonic_pc,
            c.perf.region.mode,
            c.score.chords.len(),
            c.perf.statements.len(),
            c.perf.actions.actions.len(),
            c.score.notes.len()
        );
        eprintln!("  {}", r.report());
        assert!(r.passes(), "{label}: {}", r.report());
        assert!(r.sites_checked >= 3 && r.landmarks_checked >= 10 && r.changes_checked >= 10);
        perf_fps.push(c.perf.fingerprint());
        score_fps.push(c.score.fingerprint());
    }
    let distinct = |v: &[u64]| v.iter().collect::<std::collections::BTreeSet<_>>().len();
    assert_eq!(distinct(&perf_fps), 4, "four different performances");
    assert_eq!(distinct(&score_fps), 4, "four different scores");
}

/// **Negative controls: a real song coordinate moves the map; the check sees it.** Mutating the
/// thesis contour, a harmonic arrival or a landmark changes the song's fingerprint, and a
/// performance of the ORIGINAL song fails conformance against the mutated one with the exact
/// reason. (That no room or idiom moves it is structural: `SongMap::build` takes neither.)
#[test]
fn real_mutations_move_the_song() {
    let song = flagship_song();
    let fp = song.fingerprint();
    let hm = song.harmonic.expect("DeflectedLift has a chart");
    let c = perform(&song, &MusicWorld::vapor95(), PerformanceOptions::default());

    // The thesis contour.
    let mut thesis = song.clone();
    thesis.thematic.bank.identity.degrees[1] += 1;
    assert_ne!(thesis.fingerprint(), fp);
    assert_ne!(thesis.thematic.fingerprint(), song.thematic.fingerprint());
    assert_eq!(thesis.harmonic, song.harmonic, "the chart did not move");
    let r = SongMapConformance::check(&thesis, &c.perf, &c.score);
    assert!(r.bank_mismatch && !r.passes(), "{}", r.report());

    // An identity site restated otherwise.
    let mut site = song.clone();
    let ix = site
        .thematic
        .sites
        .iter()
        .position(|s| s.is_identity())
        .expect("an identity site");
    site.thematic.sites[ix].motif = site.thematic.sites[ix].motif.transpose(1);
    assert_ne!(site.fingerprint(), fp);
    let r = SongMapConformance::check(&site, &c.perf, &c.score);
    assert!(
        r.missing_theme_sites
            .iter()
            .any(|m| m.why == "stated otherwise"),
        "{}",
        r.report()
    );

    // The harmonic arrival: the deflection lands on another degree.
    let mut arrival = song.clone();
    arrival.harmonic = Some(HarmonicMap {
        cell: ChartCell {
            deflect: ChartRoot::Degree(2),
            ..hm.cell
        },
        ..hm
    });
    assert_ne!(hm.cell.deflect, ChartRoot::Degree(2), "a real change");
    assert_ne!(arrival.fingerprint(), fp);
    let r = SongMapConformance::check(&arrival, &c.perf, &c.score);
    assert!(
        r.wrong_harmonic_landmarks
            .iter()
            .any(|m| m.landmark == "deflect"),
        "{}",
        r.report()
    );

    // A landmark: one Deflect slot charted as an Open.
    let mut landmark = song.clone();
    let tl = landmark.plan.backbone.as_mut().expect("a backbone");
    let k = tl
        .slots
        .iter()
        .position(|s| s.gesture == HarmonicGesture::Deflect)
        .expect("a Deflect slot");
    tl.slots[k].gesture = HarmonicGesture::Open;
    assert_ne!(landmark.fingerprint(), fp);
    let r = SongMapConformance::check(&landmark, &c.perf, &c.score);
    assert!(!r.wrong_harmonic_landmarks.is_empty(), "{}", r.report());

    // A planned key excursion in the song's causal timeline (the performance lifts its Modulate
    // actions from it): song data, so it moves the fingerprint.
    let mut excursion = song.clone();
    excursion.timeline.transitions[1]
        .applied
        .push(super::intent::IntentMorphism::Modulate);
    assert_ne!(excursion.fingerprint(), fp);

    // The R8b defect, re-enacted: the Aeolian room searching its OWN mode for the cell (on
    // 0b4483d it found degree journey [1,5,2,0], opening on bIII). Performed, and checked against
    // the song: caught at the Open landmarks and as chords outside the chart's vocabulary.
    let mut searched = song.clone();
    searched.harmonic = Some(HarmonicMap {
        cell: ChartCell::chart(Mode::Aeolian, song.seed),
        ..hm
    });
    let old = perform(
        &searched,
        &MusicWorld::black_ice(),
        PerformanceOptions::default(),
    );
    // (This is also the configuration the Resolve-arrival constraint was built for — the song's
    // answer over the Aeolian room's own C add9 — and it arrives: every action witnessed, nothing
    // snapped.)
    let w = super::witness::audit(&old.perf, &old.score);
    assert_eq!(w.witnessed(), w.total(), "{}", w.report());
    assert_eq!(old.score.melody_repairs, 0);
    let r = SongMapConformance::check(&song, &old.perf, &old.score);
    eprintln!("{}", r.report());
    assert!(
        r.wrong_harmonic_landmarks
            .iter()
            .all(|m| m.landmark == "open")
            && !r.wrong_harmonic_landmarks.is_empty(),
        "{}",
        r.report()
    );
    assert!(
        r.illegal_harmonic_transforms
            .iter()
            .any(|m| m.why.starts_with("root outside")),
        "{}",
        r.report()
    );
}

/// **The law beyond the flagship.** Every stock story (and a trace whose danger impact asks for a
/// modulation), forced onto DeflectedLift, at three lengths, performed four ways: every
/// performance states the song's theme, sounds its chart's landmarks, changes chords only as the
/// chart and its idiom's declared transform license (or a recorded action explains), and keeps its
/// form. Before the landmarks were protected from harmonic actions and the checker knew an
/// action's split remainder, 32 of a 288-performance release sweep of this shape passed.
///
/// Song obligations are the exception, and an inherited one: the R8b tip left a settled
/// obligation unwitnessed in 208 of 240 performances of the five stock stories, and Round IX in
/// exactly as many. Pinned exactly here (a known defect's anchor, not a pass): fewer is progress
/// to re-pin, more fails.
#[test]
fn the_law_holds_across_songs() {
    use super::semantic::{
        calm_loop, demo_trace, false_climax, rise_unresolved, Density, Elevation, Emphasis,
        EventKind, SemanticEvent, SemanticState, SemanticTrace, Tone,
    };
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
    let (mut checked, mut unwitnessed) = (0, 0);
    for (name, story) in stories {
        for beats in [80.0, 120.0, 160.0] {
            let song = SongMap::build(&story(beats), SEED, Some(CompositionGrammar::DeflectedLift));
            for (label, c) in acceptance(&song) {
                let r = SongMapConformance::check(&song, &c.perf, &c.score);
                let identity = r.song == r.performed
                    && !r.bank_mismatch
                    && r.missing_theme_sites.is_empty()
                    && r.wrong_harmonic_landmarks.is_empty()
                    && r.illegal_harmonic_transforms.is_empty()
                    && r.form_mismatch.is_empty();
                assert!(identity, "{name} {beats} {label}: {}", r.report());
                checked += 1;
                unwitnessed += usize::from(r.unwitnessed_song_obligations > 0);
            }
        }
    }
    eprintln!("{checked} performances conform on theme, chart, rhythm and form; {unwitnessed} leave a song obligation unwitnessed (inherited)");
    assert_eq!(checked, 72);
    assert_eq!(unwitnessed, UNWITNESSED_OBLIGATION_PERFORMANCES);
}

/// Performances in [`the_law_holds_across_songs`] that leave a settled song obligation with no
/// witnessing action — the inherited R7b binding gap (see its doc), measured on 301278e+.
pub(super) const UNWITNESSED_OBLIGATION_PERFORMANCES: usize = 60;
