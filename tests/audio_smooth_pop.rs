//! R1/R2 smooth-pop: the opt-in pop domain and the lead-free, bass-led groove, plus the
//! counterfactuals that prove each seam is LOAD-BEARING rather than a relabel. The maintainer's EAR
//! gates whether the groove is *good*; these tests gate the structural *claims* — lead-free, bass
//! anchors every bar, the syncopated cell drives the foundation, and the pop domain is required.
#[path = "../examples/c137_song/source_pop.rs"]
mod source_pop;

use gibson::audio::human_music::{
    argument::SourceDomain,
    kernel,
    score::{DrumVoice, Role, Score},
    theme_family::Kinship,
    theory::Mode,
    MusicWorld,
};

/// The A-Aeolian world the pop groove is lawful in (tempo is irrelevant to the symbolic Score — the
/// example renders the pop feel at `--tempo=104`, but the notes are beat-placed and tempo-free).
fn world() -> MusicWorld {
    let mut world = MusicWorld::black_ice();
    world.tonic_pc = 9;
    world.mode = Mode::Aeolian;
    world.use_sevenths = true;
    world.allow_modal_mixture = true;
    world
}

fn render_pop(seed: u64) -> Score {
    kernel::perform_song(
        &source_pop::source_pop_groove(seed, source_pop::PopFlavor::Circle),
        &world(),
    )
    .expect("the smooth-pop groove is admitted and commits")
    .score()
    .clone()
}

#[test]
fn the_pop_groove_is_lead_free_and_the_bass_anchors_every_bar() {
    let score = render_pop(2112);
    // Genuinely lead-free: no foreground lead notes AT ALL — the band grooves without a virtuoso
    // lead, and this is not a bus-muted ghost (there is no lead material in the Score to mute).
    assert_eq!(
        score.role_notes(Role::Lead).count(),
        0,
        "the pop groove carries no foreground lead"
    );
    // The band is genuinely present.
    assert!(
        score.role_notes(Role::Keys).count() > 0,
        "the keys carry the comp"
    );
    assert!(!score.drums.is_empty(), "the drummer keeps the pocket");
    // The bass attacks the root on EVERY bar downbeat — the foundation law holds on the pop path.
    let bars = (score.total_beats / 4.0).round() as i64;
    for bar in 0..bars {
        let at = bar as f64 * 4.0;
        assert!(
            score
                .role_notes(Role::Bass)
                .any(|n| (n.start_beat - at).abs() < 1e-6),
            "the bass anchors the downbeat of bar {bar}"
        );
    }
}

#[test]
fn the_bass_plays_the_syncopated_cell_not_a_c137_figure() {
    let score = render_pop(2112);
    // Bar 0 is the cell cycle's "statement" bar: FIVE attacks (downbeat root, a soft octave pop on
    // the "a" of 1, a laid-back fifth, an off-beat root, a colour third into the turn). A C137 bass
    // bar is at most four (the walk) or two (the home anchor), and never attacks inside beat 1 — so an
    // off-beat attack in (0.5, 1.0) plus five attacks in the bar is the pop cell's unmistakable
    // signature. (Later bars vary — a walking answer, a driving bar, a turnaround — but bar 0 pins the
    // statement so the signature stays checkable.)
    let bar0: Vec<f64> = score
        .role_notes(Role::Bass)
        .map(|n| n.start_beat)
        .filter(|b| *b < 4.0)
        .collect();
    assert!(
        bar0.len() >= 5,
        "the syncopated pop bass cell lays five attacks in bar 0, got {}",
        bar0.len()
    );
    assert!(
        bar0.iter().any(|b| *b > 0.5 && *b < 1.0),
        "the cell pops an off-beat inside beat 1 (no C137 bass figure attacks there)"
    );
}

#[test]
fn the_lift_flavor_also_grooves_lead_free_with_a_borrowed_dominant() {
    // The second ear-gated A/B flavor (Am9 - Fmaj9 - Dm9 - E7): a leading-tone pull whose E7 borrows
    // G# via modal mixture and whose 9ths are colour. This proves that richer progression is ADMITTED
    // (sounds as authored, not refused or retracted) and still grooves lead-free over the shared cells.
    let score = kernel::perform_song(
        &source_pop::source_pop_groove(2112, source_pop::PopFlavor::Lift),
        &world(),
    )
    .expect("the lift-flavor smooth-pop groove is admitted and commits")
    .score()
    .clone();
    assert_eq!(
        score.role_notes(Role::Lead).count(),
        0,
        "the lift flavor is lead-free too"
    );
    assert!(
        score.role_notes(Role::Bass).count() > 0,
        "the lift flavor keeps a bass foundation"
    );
    assert!(
        score.role_notes(Role::Keys).count() > 0,
        "the lift flavor's keys carry the guide-tone comp"
    );
    // The borrowed E7 is sounded as authored: a G# (pitch class 8) lives in the chart, which a plain
    // A-Aeolian-only vocabulary would never admit — the mixture-bearing world is load-bearing here.
    assert!(
        score
            .chords
            .iter()
            .any(|c| c.chord.pitch_classes().contains(&8)),
        "the E7 dominant's leading tone (G#) is present — modal mixture is doing real work"
    );
}

#[test]
fn the_pop_groove_needs_the_pop_domain() {
    let src = source_pop::source_pop_groove(2112, source_pop::PopFlavor::Circle);
    assert!(
        matches!(src.domain, SourceDomain::SmoothPop(_)),
        "the pop groove declares the SmoothPop domain"
    );
    // Under the pop domain it performs — the groove-cycle closure law admits the loop.
    assert!(
        kernel::perform_song(&src, &world()).is_ok(),
        "the pop loop is admitted by the smooth-pop closure law"
    );
    // Flip the SAME source to the C137 domain and it is REFUSED: the pop route never lands the C137
    // tonic pins, so the domain selector is genuinely load-bearing, not a cosmetic relabel.
    let mut c137 = source_pop::source_pop_groove(2112, source_pop::PopFlavor::Circle);
    c137.domain = SourceDomain::C137;
    assert!(
        kernel::perform_song(&c137, &world()).is_err(),
        "the pop loop fails the C137 literal-tonic law — the domain seam is load-bearing"
    );
}

#[test]
fn the_pop_foundation_law_refuses_a_rootless_downbeat() {
    let mut src = source_pop::source_pop_groove(2112, source_pop::PopFlavor::Circle);
    // Strip the downbeat-root anchor from EVERY per-bar cell, keeping only the off-beats: a syncopated
    // bass that abandons the downbeat root is not a lawful foundation, and the guard must refuse it —
    // the foundation law bites on the pop path exactly as it does on C137.
    if let Some(g) = &mut src.groove {
        for cell in &mut g.bass_cells {
            cell.retain(|(offset, _, _, _)| *offset != 0.0);
        }
    }
    assert!(
        kernel::perform_song(&src, &world()).is_err(),
        "a rootless downbeat is refused by the harmonic-foundation law"
    );
}

// --- OPERATION JOHNNY FIVE: the pop SONG (Lead-carried verse → hook) --------------------------

fn render_pop_song(seed: u64) -> Score {
    kernel::perform_song(
        &source_pop::source_pop_song(seed, source_pop::PopFlavor::Circle),
        &world(),
    )
    .expect("the smooth-pop song is admitted and commits")
    .score()
    .clone()
}

#[test]
fn the_pop_hook_is_a_lawful_altered_consequent_of_the_verse() {
    // The song claim stands or falls here: the hook must be a GENUINE altered-consequent of the
    // verse — sharing the germ head, rhythm skeleton and contour, yet genuinely re-voiced, lifted
    // and landing home where the verse hung open. This is the engine's own kinship law, not a
    // relabel; if it ever fails, the verse→hook relationship is a lie and the source must change.
    let verse = source_pop::pop_song_verse();
    let hook = source_pop::pop_song_hook();
    let kin = Kinship::measure(&verse, &hook, 4.0);
    println!("pop hook kinship vs verse: {kin:#?}"); // --nocapture to diagnose a failing clause
    assert!(
        kin.altered_consequent(),
        "the hook must be a lawful altered-consequent of the verse: {kin:#?}"
    );
}

#[test]
fn a_scrambled_hook_is_not_a_lawful_consequent() {
    // P1 negative control: a hook with the SAME note budget but a scrambled source identity
    // (pitches reversed) must FAIL the kinship law. The relationship lives in the content, not the
    // count — if a scramble passes, "the hook answers the verse" is a vacuous claim.
    let verse = source_pop::pop_song_verse();
    let mut scrambled = source_pop::pop_song_hook();
    let reversed: Vec<_> = scrambled.iter().rev().map(|e| e.step).collect();
    for (e, s) in scrambled.iter_mut().zip(reversed) {
        e.step = s;
    }
    let kin = Kinship::measure(&verse, &scrambled, 4.0);
    assert!(
        !kin.altered_consequent(),
        "a scrambled hook (same count, reversed pitches) must not satisfy the law: {kin:#?}"
    );
}

#[test]
fn the_pop_song_sounds_a_lead_a_comping_keyboard_and_a_pad() {
    let score = render_pop_song(2112);
    // The source-authored vocal-proxy LEAD is present (the groove arm carries zero; THIS is the song).
    assert!(
        score.role_notes(Role::Lead).count() > 0,
        "the pop song carries a source-authored lead topline"
    );
    // The keyboard COMPS, freed from the melodic obligation — a harmonic voice, not the carrier.
    assert!(
        score.role_notes(Role::Keys).count() > 0,
        "the freed keyboard comps under the song"
    );
    // The pad RUG enters (seated on stage + the hook is a Consequent): an earned harmonic bed, not
    // permanent wallpaper. The lead-free groove arm has a silent pad; the song earns one.
    assert!(
        score.role_notes(Role::Pad).count() > 0,
        "the pad rug enters on the hook"
    );
    // The funk pop bass is still the foundation, anchoring every bar downbeat.
    let bars = (score.total_beats / 4.0).round() as i64;
    for bar in 0..bars {
        let at = bar as f64 * 4.0;
        assert!(
            score
                .role_notes(Role::Bass)
                .any(|n| (n.start_beat - at).abs() < 1e-6),
            "the bass anchors the downbeat of bar {bar}"
        );
    }
}

#[test]
fn the_funk_kit_is_load_bearing_four_on_the_floor() {
    // The funk/disco kit must be a real pattern change, not a relabel: four-on-the-floor puts a kick on
    // EVERY beat — including beats 2 and 4, where the archived kick-on-1-and-3 kit never places one.
    // Flip `pop_funk_kit` off on the same song and those kicks must disappear (and the kit stays gated).
    let funk = render_pop_song(2112); // source_pop_song sets pop_funk_kit = true
    let mut plain_src = source_pop::source_pop_song(2112, source_pop::PopFlavor::Circle);
    plain_src.pop_funk_kit = false;
    let plain = kernel::perform_song(&plain_src, &world())
        .expect("the song with the archived kit still commits")
        .score()
        .clone();
    let kicks_on = |s: &Score, beat: f64| {
        s.drums
            .iter()
            .filter(|d| matches!(d.voice, DrumVoice::Kick) && (d.start_beat - beat).abs() < 1e-6)
            .count()
    };
    assert!(
        kicks_on(&funk, 1.0) > 0 && kicks_on(&funk, 3.0) > 0,
        "the funk kit lands a kick on beats 2 and 4 (four-on-the-floor)"
    );
    assert_eq!(
        kicks_on(&plain, 1.0),
        0,
        "the archived kit never kicks on beat 2 — so the funk flag is genuinely load-bearing"
    );
    assert!(
        funk.drums.len() != plain.drums.len(),
        "the funk kit commits a different drum pattern than the archived kit"
    );
}
