//! Opt-in SMOOTH-POP groove source (R2): a lead-free, Keys-carried band pocket at a pop harmonic
//! loop, carrying a syncopated bass identity. No foreground lead — the probe is whether kick / snare
//! / bass / keys alone make a recognizable, intentional pocket (the plan's R2 "can the band groove
//! without a virtuoso lead"). Both episodes are Keys-carried `Establish`, so the lead lane is
//! genuinely EMPTY (not a bus-muted ghost); the bass plays the authored per-bar [`GrooveLaw`] cells;
//! the harmony is a repeating [`SourceDomain::SmoothPop`] loop that closes as a groove-cycle, never on
//! a literal tonic. Everything here is additive and opt-in; no C137 source is touched.
//!
//! The first cut used a single stamped bass cell over the Axis-of-Awesome loop (i–VI–III–VII); the
//! ear's verdict was "offensively cliché" harmony and a "robotic" groove. This cut answers BOTH:
//! - Two fresh, less-worn progressions offered as an ear-gated **A/B** ([`PopFlavor`]) — a
//!   circle-of-fourths turnaround and a leading-tone "lift" — never the four-chord loop.
//! - The groove breathes: FOUR varied per-bar bass cells (a settled statement, a walking answer, a
//!   driving bar, a turnaround that pushes back to the top) with ghost notes and a dynamic arc,
//!   instead of one identical bar stamped eight times.
//! - The keys are re-voiced to the **guide tones** (3rd/7th + a 9th colour) of EACH flavor's chords,
//!   because the carried keys comp does not follow the route — it must be authored to match it or it
//!   clashes, and no gate but the ear catches a clash.
//! - Keys and bass LOCK and LIFT: both are laid into the shared pocket (so the melodic voice lays
//!   back WITH the rhythm section instead of floating ahead of it, dead on the grid) and PUSH the
//!   "and of 4" together every bar — the band coalescing into one collective gesture while each voice
//!   keeps its own expressive line.
//! - The keys MOVE THROUGH the groove: not detached stabs dotting the pocket's points, but a flowing,
//!   legato guide-tone LINE that arcs and steps across the progression (rising then resolving), so the
//!   melodic voice traverses the geometry the pocket creates rather than punctuating it.
use gibson::audio::human_music::{
    action::Agent,
    argument::{
        ArcSpan, ArgumentEnding, ArgumentFamily, ArgumentRelation, ArgumentStep, ArgumentStepId,
        ArgumentTransform, BassDegree, CompiledArgument, GrooveLaw, MusicalArgument,
        MusicalReferent, PopSourceGrammar, ReferentId, SourceDomain,
    },
    contract::CompositionGrammar,
    material::MaterialEvent,
    plan::ArrangementRole,
    semantic::{EventKind, SemanticEvent, SemanticState, SemanticTrace, Tone},
    theme_family::pocket_offset,
    theory::{PitchBasis, Quality},
    SongMap,
};

/// The ear-gated A/B of smooth-pop harmonic flavors. Both are lawful A-Aeolian (black_ice world,
/// `color_depth=2`, modal mixture admitted) and both are deliberately NOT the i–VI–III–VII cliché;
/// the maintainer's ear picks the harmonic world (nothing is promoted). Each flavor pairs a 4-bar
/// progression with a keys comp that voices THAT progression's guide tones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopFlavor {
    /// `Am9 – Dm9 – G9 – Cmaj9` — a circle-of-fourths turnaround (i9–iv9–♭VII9–♭IIImaj9; a vi–ii–V–I
    /// in the relative major). Ascending-fourth root motion is the smoothest motion in tonal music;
    /// warm, forward, jazzy-smooth. Closes by cycling back to Am9 (groove-cycle, not a tonic arrival).
    Circle,
    /// `Am9 – Fmaj9 – Dm9 – E7` — a leading-tone "lift" (i9–VImaj9–iv9–V7). The iv→V with a real
    /// dominant (E7 borrows G♯, the leading tone, via modal mixture) is the authentic minor-key pull;
    /// cinematic, emotional-smooth. The dynamic arc climbs to the dominant and resolves back to the top.
    Lift,
}

impl PopFlavor {
    /// The region-relative per-bar pop loop for this flavor (one 4-bar cycle). Offsets are semitones
    /// above the region tonic (A): C=+3, D=+5, E=+7, F=+8, G=+10. Every quality is admitted by the
    /// black_ice A-Aeolian vocabulary (sevenths + colour + mixture), and the sounded chord is exactly
    /// this (the route is not conformed on the perform path), so what is authored is what is heard.
    fn loop4(self) -> [(i32, Quality); 4] {
        match self {
            // i9 – iv9 – ♭VII9 – ♭IIImaj9 (circle of fourths: A→D→G→C).
            PopFlavor::Circle => [
                (0, Quality::Min9),
                (5, Quality::Min9),
                (10, Quality::Dom9),
                (3, Quality::Maj9),
            ],
            // i9 – VImaj9 – iv9 – V7 (the lift: A→F→D→E, resolving E7→Am).
            PopFlavor::Lift => [
                (0, Quality::Min9),
                (8, Quality::Maj9),
                (5, Quality::Min9),
                (7, Quality::Dom7),
            ],
        }
    }

    /// The pop SONG's fresher per-bar progression (one 4-bar cycle) — OFF the circle-of-fourths cliché
    /// the groove used. Fully diatonic to A-Aeolian, so the Aeolian topline never clashes; the fresh
    /// character is in the modal MINOR v (Em7 — pop almost always forces a major V) and the ♭VI, not in
    /// stacked colour. `(semitones_above_A, Quality)`.
    fn song_loop4(self) -> [(i32, Quality); 4] {
        match self {
            // i9 – v7 – ♭VImaj9 – V7: the fresh modal MINOR v (Em7) early, then a real DOMINANT (E7) at
            // the cadence — the "minor-v becomes major-V" soul turnaround. The V7 gives the hook a
            // functional home AND an in-context seat for the lead's leading-tone colour (G# = E7's 3rd).
            PopFlavor::Circle => [
                (0, Quality::Min9),
                (7, Quality::Min7),
                (8, Quality::Maj9),
                (7, Quality::Dom7),
            ],
            // i9 – ♭VImaj9 – iv9 – V7: the bright ♭VI lift, resolved by the dominant (E7) cadence —
            // anthemic, and the same in-context home for the leading-tone colour.
            PopFlavor::Lift => [
                (0, Quality::Min9),
                (8, Quality::Maj9),
                (5, Quality::Min9),
                (7, Quality::Dom7),
            ],
        }
    }

    /// A flowing, connected guide-tone LINE over one 16-beat episode. The keys no longer DOT the
    /// pocket's anchor points with detached stabs — they MOVE THROUGH its geometry: a melodic line
    /// that steps and arcs across the progression (rising through the first half of the loop,
    /// resolving down through the second), LEGATO (each note all but reaches the next, so the line
    /// connects instead of punctuating), with a breathing dynamic arc. Every onset is still laid into
    /// the shared [`pocket_offset`] so the line lays back WITH the band, and the "and of 4" of each
    /// bar still pushes with the bass (the collective hit). Tonic-relative semitones, voiced to THIS
    /// flavor's chords. `(onset, semitone_above_A, dur, velocity)`.
    fn keys_comp(self) -> Vec<MaterialEvent> {
        // Onsets a beat apart = a connected quarter-ish line; durations ~0.9 all but touch the next
        // note (legato), the push (beat-4 "and") and the final resolution shaped shorter; velocities
        // breathe an arch, cresting into the third bar and easing into the resolution.
        let rows: &[(f64, i32, f64, f32)] = match self {
            PopFlavor::Circle => &[
                // Am9 — rise E G A B
                (0.5, 7, 0.9, 0.48),
                (1.5, 10, 0.9, 0.52),
                (2.5, 12, 0.9, 0.56),
                (3.5, 14, 0.7, 0.60),
                // Dm9 — fall A G F D
                (4.5, 12, 0.9, 0.56),
                (5.5, 10, 0.9, 0.52),
                (6.5, 8, 0.9, 0.50),
                (7.5, 5, 0.7, 0.56),
                // G9 — rise F G A B
                (8.5, 8, 0.9, 0.54),
                (9.5, 10, 0.9, 0.58),
                (10.5, 12, 0.9, 0.60),
                (11.5, 14, 0.7, 0.64),
                // Cmaj9 — resolve down B G E C
                (12.5, 14, 0.9, 0.58),
                (13.5, 10, 0.9, 0.54),
                (14.5, 7, 0.9, 0.52),
                (15.5, 3, 0.4, 0.50),
            ],
            PopFlavor::Lift => &[
                // Am9 — rise E G A B
                (0.5, 7, 0.9, 0.48),
                (1.5, 10, 0.9, 0.52),
                (2.5, 12, 0.9, 0.56),
                (3.5, 14, 0.7, 0.60),
                // Fmaj9 — fall A G F C
                (4.5, 12, 0.9, 0.56),
                (5.5, 10, 0.9, 0.52),
                (6.5, 8, 0.9, 0.50),
                (7.5, 3, 0.7, 0.56),
                // Dm9 — rise F G A C
                (8.5, 8, 0.9, 0.54),
                (9.5, 10, 0.9, 0.58),
                (10.5, 12, 0.9, 0.60),
                (11.5, 15, 0.7, 0.64),
                // E7 — resolve B G# D E (G# the leading tone pulling home)
                (12.5, 14, 0.9, 0.58),
                (13.5, 11, 0.9, 0.54),
                (14.5, 5, 0.9, 0.52),
                (15.5, 7, 0.4, 0.52),
            ],
        };
        rows.iter()
            .map(|&(onset, step, dur, accent)| MaterialEvent {
                onset: onset + pocket_offset(onset), // lock into the shared pocket the band orbits
                dur,
                accent,
                step: Some(step),
            })
            .collect()
    }

    /// A short honest label of the progression for the manifest header. (Used by the example binary;
    /// the test crate includes this module via `#[path]` but does not call it.)
    #[allow(dead_code)]
    pub fn harmony_label(self) -> &'static str {
        match self {
            PopFlavor::Circle => {
                "Am9 - Dm9 - G9 - Cmaj9 (i9-iv9-bVII9-bIIImaj9, circle-of-4ths turnaround, A-Aeolian)"
            }
            PopFlavor::Lift => {
                "Am9 - Fmaj9 - Dm9 - E7 (i9-VImaj9-iv9-V7, leading-tone pull, A-Aeolian + mixture)"
            }
        }
    }

    /// The pop SONG's progression label (the fresher [`song_loop4`], not the groove's [`loop4`]).
    #[allow(dead_code)]
    pub fn song_harmony_label(self) -> &'static str {
        match self {
            PopFlavor::Circle => {
                "Am9 - Em7 - Fmaj9 - E7 (i9-v7-bVImaj9-V7, minor-v becomes major-V soul cadence, A-Aeolian)"
            }
            PopFlavor::Lift => {
                "Am9 - Fmaj9 - Dm9 - E7 (i9-bVImaj9-iv9-V7, bright bVI lift + dominant cadence, A-Aeolian)"
            }
        }
    }

    /// The `--form=` token that selects this flavor. (Used by the example binary; the test crate
    /// includes this module via `#[path]` but does not call it.)
    #[allow(dead_code)]
    pub fn form_token(self) -> &'static str {
        match self {
            PopFlavor::Circle => "pop-groove",
            PopFlavor::Lift => "pop-groove-lift",
        }
    }
}

/// The breathing per-bar bass cells (one 4-bar cycle), SHARED across flavors because the degrees
/// resolve per bar against whatever chord sits under them. A smooth bassist does not stamp one bar
/// eight times — these four bars are a settled statement, a walking answer, a driving bar, and a
/// turnaround that pushes back to the top, each with ghost notes (soft velocities) and a dynamic
/// contour. Every cell opens on `(0.0, Root, ..)` — the downbeat anchor the foundation law reads.
/// `(offset_in_bar_beats, degree, dur_beats, velocity)`.
fn bass_cells() -> Vec<Vec<(f64, BassDegree, f32, f32)>> {
    vec![
        // Bar 0 — land & bounce: the root anchors, a soft octave bounce, a laid-back fifth, an
        // off-beat root, then the collective PUSH on the "and" of 4. Five attacks (the signature).
        vec![
            (0.0, BassDegree::Root, 0.6, 0.80),
            (0.75, BassDegree::Octave, 0.22, 0.46),
            (1.5, BassDegree::Fifth, 0.5, 0.56),
            (2.5, BassDegree::Root, 0.5, 0.60),
            (3.5, BassDegree::Octave, 0.4, 0.62),
        ],
        // Bar 1 — walking answer: stepping fifth/octave/fifth with a ghost root, same "and of 4" push.
        vec![
            (0.0, BassDegree::Root, 0.45, 0.76),
            (0.5, BassDegree::Fifth, 0.3, 0.44),
            (1.0, BassDegree::Octave, 0.4, 0.50),
            (2.0, BassDegree::Fifth, 0.5, 0.58),
            (2.75, BassDegree::Root, 0.5, 0.48),
            (3.5, BassDegree::Octave, 0.4, 0.60),
        ],
        // Bar 2 — driving: the bounciest bar (and the flavor's loud chord lands here), two octave
        // bounces around a ghost root and fifth, pushing hardest into the turn.
        vec![
            (0.0, BassDegree::Root, 0.5, 0.82),
            (0.75, BassDegree::Octave, 0.22, 0.48),
            (1.5, BassDegree::Fifth, 0.4, 0.62),
            (2.0, BassDegree::Octave, 0.3, 0.50),
            (2.75, BassDegree::Fifth, 0.5, 0.54),
            (3.5, BassDegree::Octave, 0.4, 0.64),
        ],
        // Bar 3 — turnaround: opens with root + fifth, a mid-bar octave bounce, then the STRONGEST
        // push on the "and" of 4 (the loudest in the loop) that pulls the ear back to the top.
        vec![
            (0.0, BassDegree::Root, 0.7, 0.76),
            (1.0, BassDegree::Fifth, 0.5, 0.56),
            (1.75, BassDegree::Octave, 0.4, 0.52),
            (2.5, BassDegree::Root, 0.4, 0.56),
            (3.0, BassDegree::Fifth, 0.3, 0.58),
            (3.5, BassDegree::Octave, 0.45, 0.66),
        ],
    ]
}

/// Build the lead-free smooth-pop groove source for one seed and one [`PopFlavor`]. Two 16-beat
/// Keys-carried `Establish` episodes (8 bars = two loop cycles), the flavor's pop harmonic loop, its
/// matching guide-tone keys comp, and the shared breathing bass cells.
pub fn source_pop_groove(seed: u64, flavor: PopFlavor) -> CompiledArgument {
    let trace = SemanticTrace {
        total_beats: 32.0,
        events: vec![
            SemanticEvent {
                at_beat: 0.0,
                state: SemanticState::toned(Tone::Accent),
                kind: EventKind::ActChanged,
            },
            SemanticEvent {
                at_beat: 28.0,
                state: SemanticState::toned(Tone::Success),
                kind: EventKind::SectionResolved,
            },
        ],
    };
    let mut song = SongMap::build(&trace, seed, Some(CompositionGrammar::HookArc));
    let mut contract = song.plan.contract.clone();
    contract.phrase_bars = 1;
    song.plan = gibson::audio::human_music::plan::CompositionPlan::build_with_contract_for_beats(
        &song.timeline,
        32.0,
        contract,
    );
    // Lead-free staging: the lead is off-stage, the keys carry, the bass is the foundation, the
    // drummer keeps the pocket, the pad rests. The absence of any Lead carrier (below) is what makes
    // this genuinely lead-free; the staging just reflects it.
    for p in &mut song.plan.arrangement.phrases {
        p.lead = ArrangementRole::Support;
        p.keys = ArrangementRole::Support;
        p.bass = ArrangementRole::Foundation;
        p.drums = ArrangementRole::Support;
        p.pad = ArrangementRole::Silent;
    }
    let riff0 = MusicalReferent {
        id: ReferentId(0),
        pitch_basis: PitchBasis::Semitones,
        events: flavor.keys_comp(),
    };
    let riff1 = MusicalReferent {
        id: ReferentId(1),
        pitch_basis: PitchBasis::Semitones,
        events: flavor.keys_comp(),
    };
    let boundary =
        song.plan
            .form
            .phrases
            .iter()
            .position(|p| p.start_beat() == 16.0)
            .expect("eight one-bar phrases put an episode boundary at beat 16") as u32;
    let total_phrases = song.plan.form.phrases.len() as u32;
    let argument = MusicalArgument {
        family: ArgumentFamily::CallAndEarnedAnswer,
        referents: vec![riff0, riff1],
        ending: ArgumentEnding::Resolved,
        steps: vec![
            ArgumentStep {
                id: ArgumentStepId(0),
                referent: ReferentId(0),
                transform: ArgumentTransform::default(),
                relation: ArgumentRelation::Establish,
                depends_on: vec![],
                carriers: vec![Agent::Keys],
            },
            ArgumentStep {
                id: ArgumentStepId(1),
                referent: ReferentId(1),
                transform: ArgumentTransform::default(),
                relation: ArgumentRelation::Establish,
                depends_on: vec![ArgumentStepId(0)],
                carriers: vec![Agent::Keys],
            },
        ],
        arc: Some(vec![
            ArcSpan {
                step: ArgumentStepId(0),
                start_phrase: 0,
                phrase_count: boundary,
                start_beat: 0.0,
                span_beats: 16.0,
            },
            ArcSpan {
                step: ArgumentStepId(1),
                start_phrase: boundary,
                phrase_count: total_phrases - boundary,
                start_beat: 16.0,
                span_beats: 16.0,
            },
        ]),
    };
    let mut compiled = argument
        .compile(&song)
        .expect("smooth-pop groove source compiles");
    // The eight-bar route is the four-bar pop loop twice. Declared after compile, like the C137
    // builders set `route` — so the long-form domain reads it under the smooth-pop closure law.
    let loop4 = flavor.loop4();
    let mut route = loop4.to_vec();
    route.extend_from_slice(&loop4);
    compiled.route = route;
    // Opt this ONE source into the smooth-pop closure law (groove-cycle, loop of 4 bars) and the
    // breathing per-bar bass cells. No C137 source is affected; both are set after compile, like `route`.
    compiled.domain = SourceDomain::SmoothPop(PopSourceGrammar { loop_bars: 4 });
    compiled.groove = Some(GrooveLaw {
        bass_cells: bass_cells(),
    });
    compiled
}

// ======================================================================================
// OPERATION JOHNNY FIVE — the pop SONG: a source-authored Lead topline (verse → hook).
// ======================================================================================
// The groove above proved the band can hold a pocket with no foreground. This is the next
// probe: give the song an actual voice. A hand-authored vocal-proxy LEAD states a four-bar
// VERSE that hangs OPEN, then a four-bar HOOK that ANSWERS it — a lawful `altered_consequent`
// of the verse (shared germ head + rhythm skeleton + contour, genuinely re-voiced and lifted
// 2–6 semitones, landing HOME on the tonic where the verse hung open). Because the lead now
// CARRIES both episodes, the keyboard is freed from the melodic obligation: with `comp_keys`
// the keys comp short guide-tone stabs in the holes under the hook, the rug (pad) enters on
// the Consequent, and the drummer answers into the lead's breaths. The hook ARRIVES because
// the voice lands home and the band fills in around it — not because anything merely got
// louder. Still opt-in; still the breathing pop bass; no C137 source is touched.

/// The four-bar VERSE topline (one 16-beat episode), tonic-relative semitones, Aeolian. The germ is
/// no longer a stepwise climb — it's a SIGNATURE LEAP: E kicks up a fourth to A, then steps down to G,
/// landing off the beat (a funk pickup, not a scale). Syncopated 16th phrasing with real space, a low
/// reach in the second bar, and a settle that ends OPEN on the 9th (B), still wanting the hook.
/// `(onset, semitone_above_A, dur_beats, velocity)`.
pub fn pop_song_verse() -> Vec<MaterialEvent> {
    rows_to_lead_events(&[
        // Am9 — the germ LEAP: E, up a 4th to A, step down to G (syncopated), then an off-beat answer.
        (0.0, 7, 0.45, 0.58),
        (0.5, 12, 0.20, 0.64),
        (0.75, 10, 0.5, 0.56),
        (2.5, 7, 0.45, 0.54),
        (3.0, 8, 0.5, 0.56),
        // Em7 — the fall is a GESTURE, not a scale walk: a leap DOWN a 5th to the low D (the funk drop),
        // a step back up, settle again on the low D with space (the breath). Same low reach, no G-F-E run.
        (4.0, 10, 0.6, 0.56),
        (4.75, 5, 0.6, 0.54),
        (5.5, 7, 0.5, 0.54),
        (7.0, 5, 0.6, 0.52),
        // Fmaj9 — reach the octave A by a LEAP, not a run: a quick dip to E, then up a 5th to the octave A
        // (the reached high note), then a syncopated G. The connecting note is a chord-tone skip, not a scale.
        (8.0, 8, 0.45, 0.56),
        (8.5, 7, 0.20, 0.58),
        (8.75, 12, 0.5, 0.62),
        (10.5, 10, 0.6, 0.56),
        // E7 — leap to the open B (the dominant's 5th), then a LOWER neighbour A (down, not the cliché
        // upper-neighbour C that reached UP — the "goes up when it should go down" the ear flagged at the
        // cadence), and hang OPEN on B. The neighbour now falls and recovers. Still wanting the hook.
        (12.0, 7, 0.45, 0.54),
        (12.75, 2, 0.6, 0.52),
        (13.5, 0, 0.5, 0.52),
        (14.5, 2, 1.0, 0.50),
    ])
}

/// The four-bar HOOK topline (one 16-beat episode) — a lawful `altered_consequent` of
/// [`pop_song_verse`]: it OPENS with the verse's exact germ LEAP (E–A–G, so the ear knows the song is
/// the same song), shares the verse's syncopated rhythm skeleton, then re-voices it a register higher
/// (mean lifted ~3 semitones) WITHOUT transposing it, peaks a step above the verse's peak, and leaps
/// back HOME to the tonic A at the end (Closed) where the verse hung open. `(onset, step, dur, vel)`.
pub fn pop_song_hook() -> Vec<MaterialEvent> {
    rows_to_lead_events(&[
        // Am9 — the germ leap (E–A–G), then up to A. The chorus's chromatic flare now lives at the
        // CADENCE, over the E7 dominant where the leading tone belongs — NOT here over the tonic (where
        // a G# fought the chord's own G natural, the "out of place" note the ear flagged).
        (0.0, 7, 0.45, 0.62),
        (0.5, 12, 0.20, 0.68),
        (0.75, 10, 0.5, 0.64),
        (2.5, 10, 0.45, 0.60),
        (3.0, 12, 0.5, 0.64),
        // Em7 — the answer up a register, riding A and the octave (where the verse fell to D).
        (4.0, 12, 0.6, 0.62),
        (4.75, 10, 0.6, 0.60),
        (5.5, 12, 0.5, 0.64),
        (7.0, 10, 0.6, 0.60),
        // Fmaj9 — reach the hook's peak B (a step above the verse's A) by a LEAP, not a stepwise climb:
        // a quick dip to E, then up a 5th to the peak B (the song's high point), then settle to A. The
        // connector is a chord-tone skip mirroring the verse's bar-2 leap, not a G-A-B scale run.
        (8.0, 10, 0.45, 0.60),
        (8.5, 7, 0.20, 0.64),
        (8.75, 14, 0.5, 0.70),
        (10.5, 12, 0.6, 0.64),
        // E7 — the homecoming, DE-BLUED: no leading-tone scoop (G#) and no blue ♭3 (G♮) rub — those were
        // the "blues type lead tones" the ear flagged as cliché. Instead a clean 9→8 resolution: the 9th
        // (B) leans HOME to the tonic A, dips to a diatonic lower neighbour G (the ♭7, no longer a blue
        // note now the G# is gone), and settles HOME on A (Closed). Fully diatonic; flare from line, not blues.
        (12.0, 14, 0.45, 0.60),
        (12.75, 12, 0.6, 0.56),
        (13.5, 10, 0.5, 0.56),
        (14.5, 12, 1.0, 0.60),
    ])
}

/// Lead topline events from `(onset, step, dur, velocity)` rows. Unlike the keys comp, the
/// lead onsets are NOT laid into the shared pocket — the voice floats clean over a laid-back
/// rhythm section (a classic pop tension), and clean onsets sit exactly on the 16th lattice.
fn rows_to_lead_events(rows: &[(f64, i32, f64, f32)]) -> Vec<MaterialEvent> {
    rows.iter()
        .map(|&(onset, step, dur, accent)| MaterialEvent {
            onset,
            dur,
            accent,
            step: Some(step),
        })
        .collect()
}

/// The song's FUNK bass: four syncopated per-bar cells of octave-and-ghost disco-funk — the bassline
/// as a rhythmic HOOK (Espresso / Feels / Off the Wall), not a smooth root-walk. Every cell still
/// opens on `(0.0, Root)` (the foundation law the kernel checks), stays monophonic and leaves space;
/// the bounce is in the octave pops, the off-beat placement and the ghosts. `(offset, degree, dur, vel)`.
fn song_funk_bass_cells() -> Vec<Vec<(f64, BassDegree, f32, f32)>> {
    // Root and Octave share a pitch when the chord root sits high (the kernel folds the octave back
    // onto the root), so they are kept ≥ 1 beat apart within a bar; the Fifth (never folds onto the
    // root) carries the off-beat bounce. A root–fifth–octave boogie with space — the funk idiom.
    vec![
        // Bar 0 — statement: root, off-beat fifth, ONE octave bounce on beat 2, space, then the bar-end
        // push RESOLVES DOWN onto the low root (not a leap up to the octave — the "goes up when it wants
        // down" the ear flagged). The mid-bar octave keeps the boogie bounce; the resolution settles low.
        vec![
            (0.0, BassDegree::Root, 0.24, 0.86),
            (0.5, BassDegree::Fifth, 0.20, 0.50),
            (1.0, BassDegree::Octave, 0.22, 0.56),
            (2.0, BassDegree::Root, 0.22, 0.58),
            (2.5, BassDegree::Third, 0.20, 0.52), // the chord 3rd — colour, outlining the quality
            (3.5, BassDegree::Root, 0.26, 0.62),
        ],
        // Bar 1 — walking funk: root, fifth, a syncopated octave, fifth, a late root, the fifth push.
        vec![
            (0.0, BassDegree::Root, 0.24, 0.84),
            (0.5, BassDegree::Fifth, 0.20, 0.50),
            (1.5, BassDegree::Octave, 0.22, 0.54),
            (2.0, BassDegree::Fifth, 0.22, 0.58),
            (3.0, BassDegree::Root, 0.22, 0.54),
            (3.5, BassDegree::Fifth, 0.26, 0.60),
        ],
        // Bar 2 — driving: the busiest, a full eighth-note root/octave–fifth boogie into the turn.
        vec![
            (0.0, BassDegree::Root, 0.22, 0.88),
            (0.5, BassDegree::Fifth, 0.20, 0.50),
            (1.0, BassDegree::Octave, 0.20, 0.54),
            (1.5, BassDegree::Fifth, 0.20, 0.50),
            (2.0, BassDegree::Root, 0.20, 0.58),
            (2.5, BassDegree::Third, 0.20, 0.52), // the chord 3rd — colour
            (3.0, BassDegree::Octave, 0.20, 0.54),
            (3.5, BassDegree::Fifth, 0.26, 0.62),
        ],
        // Bar 3 — turnaround (the cadence bar, ~7-9s): root, then the line FALLS — fifth DOWN to the third
        // (no octave leap up, the "bass should be going down not up" the ear flagged here), and the
        // STRONGEST push RESOLVES DOWN onto the low root. The whole turnaround descends into the loop.
        vec![
            (0.0, BassDegree::Root, 0.24, 0.82),
            (1.0, BassDegree::Fifth, 0.20, 0.52),
            (1.5, BassDegree::Third, 0.22, 0.55),
            (2.0, BassDegree::Fifth, 0.22, 0.56),
            (2.5, BassDegree::Root, 0.20, 0.54),
            (3.5, BassDegree::Root, 0.28, 0.68),
        ],
    ]
}

/// Build the smooth-pop SONG source for one seed and [`PopFlavor`]: a Lead-carried verse → hook
/// over the flavor's pop loop (twice), with the comping keyboard, the pad rug, the breathing
/// bass and the lead-answering drummer all switched on. The hook is a lawful `altered_consequent`
/// of the verse, so the engine's own kinship law certifies the song relationship at compile time.
pub fn source_pop_song(seed: u64, flavor: PopFlavor) -> CompiledArgument {
    let trace = SemanticTrace {
        total_beats: 32.0,
        events: vec![
            SemanticEvent {
                at_beat: 0.0,
                state: SemanticState::toned(Tone::Accent),
                kind: EventKind::ActChanged,
            },
            SemanticEvent {
                at_beat: 28.0,
                state: SemanticState::toned(Tone::Success),
                kind: EventKind::SectionResolved,
            },
        ],
    };
    let mut song = SongMap::build(&trace, seed, Some(CompositionGrammar::HookArc));
    let mut contract = song.plan.contract.clone();
    contract.phrase_bars = 1;
    song.plan = gibson::audio::human_music::plan::CompositionPlan::build_with_contract_for_beats(
        &song.timeline,
        32.0,
        contract,
    );
    // The lead now CARRIES (compile promotes the seat where a Lead step lands); the keys are
    // freed to comp, the bass is the foundation, the drummer keeps the pocket, and the pad RUG
    // is seated on stage so it can enter on the hook (the Consequent) as the earned arrival.
    for p in &mut song.plan.arrangement.phrases {
        p.lead = ArrangementRole::Support;
        p.keys = ArrangementRole::Support;
        p.bass = ArrangementRole::Foundation;
        p.drums = ArrangementRole::Support;
        p.pad = ArrangementRole::Support;
    }
    let verse = MusicalReferent {
        id: ReferentId(0),
        pitch_basis: PitchBasis::Semitones,
        events: pop_song_verse(),
    };
    let hook = MusicalReferent {
        id: ReferentId(1),
        pitch_basis: PitchBasis::Semitones,
        events: pop_song_hook(),
    };
    let boundary =
        song.plan
            .form
            .phrases
            .iter()
            .position(|p| p.start_beat() == 16.0)
            .expect("eight one-bar phrases put an episode boundary at beat 16") as u32;
    let total_phrases = song.plan.form.phrases.len() as u32;
    let argument = MusicalArgument {
        family: ArgumentFamily::CallAndEarnedAnswer,
        referents: vec![verse, hook],
        ending: ArgumentEnding::Resolved,
        steps: vec![
            // VERSE — establish the open question (the lead carries it).
            ArgumentStep {
                id: ArgumentStepId(0),
                referent: ReferentId(0),
                transform: ArgumentTransform::default(),
                relation: ArgumentRelation::Establish,
                depends_on: vec![],
                carriers: vec![Agent::Lead],
            },
            // HOOK — the earned answer: a lawful altered-consequent of the verse that lands home.
            ArgumentStep {
                id: ArgumentStepId(1),
                referent: ReferentId(1),
                transform: ArgumentTransform::default(),
                relation: ArgumentRelation::Consequent {
                    antecedent: ArgumentStepId(0),
                },
                depends_on: vec![ArgumentStepId(0)],
                carriers: vec![Agent::Lead],
            },
        ],
        arc: Some(vec![
            ArcSpan {
                step: ArgumentStepId(0),
                start_phrase: 0,
                phrase_count: boundary,
                start_beat: 0.0,
                span_beats: 16.0,
            },
            ArcSpan {
                step: ArgumentStepId(1),
                start_phrase: boundary,
                phrase_count: total_phrases - boundary,
                start_beat: 16.0,
                span_beats: 16.0,
            },
        ]),
    };
    let mut compiled = argument
        .compile(&song)
        .expect("smooth-pop song source compiles");
    let loop4 = flavor.song_loop4();
    let mut route = loop4.to_vec();
    route.extend_from_slice(&loop4);
    compiled.route = route;
    compiled.domain = SourceDomain::SmoothPop(PopSourceGrammar { loop_bars: 4 });
    compiled.groove = Some(GrooveLaw {
        bass_cells: song_funk_bass_cells(),
    });
    // Free the band AND make it groove: the keyboard comps guide-tone stabs under the hook, the drummer
    // answers the lead's breaths and translates its phrase-ending rhythm, the bass leans its approach
    // toward the harmonic destination, and the kit turns to a FUNK/DISCO pocket (four-on-the-floor +
    // sixteenth hats) under the syncopated octave bass — the Espresso/Feels/Off-the-Wall rhythm section.
    // Opt-in levers: the expressive ones are proven on the C137 song; the funk kit is new here. Each is
    // ablated individually in the experiments (the lead-free groove control leaves them all off).
    compiled.comp_keys = true;
    compiled.answer_breaths = true;
    compiled.anticipate_bass = true;
    compiled.translate_lead_rhythm = true;
    compiled.pop_funk_kit = true;
    compiled
}
