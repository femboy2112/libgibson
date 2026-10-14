//! Full-song-form source: the zoom-out from the four-section vertical slice to a real pop song
//! skeleton — INTRO/VERSE → CHORUS → VERSE → CHORUS → BRIDGE → CHORUS → CHORUS → CHORUS — laid as one
//! continuous 128-beat argument across EIGHT 16-beat episodes.
//!
//! The crux: named sections are NOT a bar-range template bolted on top. The band never reads a section
//! name — it reads `ArgumentRelation`, and the kernel gates every behavior off that. So a chorus here
//! is nothing but the engine's existing recurrence vocabulary fired at the hook referent:
//!   * a VERSE is an `Establish` of the verse referent (sparse: the kernel gates keys-comp and the pad
//!     to `Consequent | Return`, so a verse is naturally held back — a semantic dynamic, not a bar
//!     number);
//!   * a pre-bridge CHORUS is a `Consequent` of the preceding verse (the hook ANSWERS the verse's open
//!     question — the `AlteredConsequent` kinship law — and the comp/pad gate makes it arrive full);
//!   * the BRIDGE is a `Depart` from the hook (the `is_departure` law — recycles a cell, lifts, ends
//!     open, wanting the return);
//!   * a post-bridge CHORUS is a `Return` to the hook, and because its `depends_on` names the bridge,
//!     the kernel's earned-return fires — the keys QUOTE the bridge's call home, so the homecoming
//!     carries a consequence of the journey. A different lawful bridge comes home different.
//!
//! The referents are the SAME developed verse/hook/bridge material the 16-bar arc uses
//! (`source_bridge.rs`), so NO per-bar lead density or coherence is lost in the zoom-out — the hook
//! RECURS verbatim at every chorus pillar (that is what makes it a refrain) and the verse recurs. The
//! 16-bar slice and the 8-bar control stay byte-identical; this is an opt-in arm (`--form=song`),
//! gated behind the maintainer's ear before it could ever become a default.
use gibson::audio::human_music::{
    action::Agent,
    argument::{
        ArcSpan, ArgumentEnding, ArgumentFamily, ArgumentRelation, ArgumentStep, ArgumentStepId,
        ArgumentTransform, CompiledArgument, MusicalArgument, MusicalReferent, ReferentId,
    },
    contract::CompositionGrammar,
    material::MaterialEvent,
    plan::ArrangementRole,
    semantic::{EventKind, SemanticEvent, SemanticState, SemanticTrace, Tone},
    theme_family::{generate_developed_bridge_phrased, generate_developed_pair_phrased, Phrasing},
    theory::{PitchBasis, Quality},
    SongMap,
};

/// The region-relative per-bar charts, mirroring `source_bridge.rs` (the C137 region identity — the
/// verse hangs open on its V7, the hook lands home, the bridge pivots away and ends open on the home
/// dominant). Re-declared here so the song-form module is self-contained; the values are the stable
/// C137 charts and the generated referents are identical to the 16-bar arc's by construction.
const VERSE_PROG: [(i32, Quality); 4] = [
    (0, Quality::Min),
    (5, Quality::Min),
    (7, Quality::Dom7),
    (0, Quality::Min),
];
const HOOK_PROG: [(i32, Quality); 4] = [
    (8, Quality::Maj),
    (10, Quality::Maj),
    (5, Quality::Min),
    (0, Quality::Min),
];
const BRIDGE_PROG: [(i32, Quality); 4] = [
    (0, Quality::Dom7),
    (5, Quality::Min),
    (8, Quality::Maj),
    (7, Quality::Dom7),
];

/// The eight-section song argument carried by the lead. Verse/hook/bridge are the SAME developed
/// referents the 16-bar arc generates; only the ARC (eight typed sections) and the ROUTE (32 bars)
/// differ. Factored out so the red tests can assert the recurrence and earned-return laws on the
/// pre-compiled argument.
pub fn argument_song(seed: u64) -> (MusicalArgument, SongMap) {
    argument_song_with_bridge_seed_phrased(seed, seed, Phrasing::Plain, false)
}

/// [`argument_song`] with the bridge motif drawn from `bridge_seed` and an explicit lead [`Phrasing`].
/// Two different `bridge_seed`s give two LAWFUL bridges over the same chart and the same returning
/// hook — the earned-return counterfactual, now pulled across the full song's post-bridge choruses.
pub fn argument_song_with_bridge_seed_phrased(
    seed: u64,
    bridge_seed: u64,
    phrasing: Phrasing,
    keys_bridge: bool,
) -> (MusicalArgument, SongMap) {
    let trace = SemanticTrace {
        total_beats: 128.0,
        events: vec![
            SemanticEvent {
                at_beat: 0.0,
                state: SemanticState::toned(Tone::Accent),
                kind: EventKind::ActChanged,
            },
            SemanticEvent {
                at_beat: 124.0,
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
        128.0,
        contract,
    );
    for p in &mut song.plan.arrangement.phrases {
        p.lead = ArrangementRole::Support;
        p.keys = ArrangementRole::Support;
        p.bass = ArrangementRole::Foundation;
        p.drums = ArrangementRole::Support;
        // The RUG: the sustained chordal bed under the band (seated on stage for the full song).
        p.pad = ArrangementRole::Support;
    }
    // The SAME developed verse/hook pair and departing bridge the 16-bar arc uses — so the full song
    // is the accepted C137 material arranged into a longer form, not new material. The hook recurs
    // verbatim at every chorus; the verse recurs at every verse.
    let pair = generate_developed_pair_phrased(seed, 16.0, 4.0, &VERSE_PROG, &HOOK_PROG, phrasing);
    let bridge_events = generate_developed_bridge_phrased(
        bridge_seed,
        &pair.hook,
        16.0,
        4.0,
        &BRIDGE_PROG,
        phrasing,
    );
    let verse = MusicalReferent {
        id: ReferentId(0),
        pitch_basis: PitchBasis::Semitones,
        events: pair.verse,
    };
    let hook = MusicalReferent {
        id: ReferentId(1),
        pitch_basis: PitchBasis::Semitones,
        events: pair.hook,
    };
    let bridge = MusicalReferent {
        id: ReferentId(2),
        pitch_basis: PitchBasis::Semitones,
        events: bridge_events,
    };
    // Each episode is four one-bar phrases; thirty-two phrases land on beats 0,4,…,124. The eight
    // sections begin on beats 0,16,…,112 — found by position so the arc binds to real phrase indices.
    let at = |beat: f64| -> u32 {
        song.plan
            .form
            .phrases
            .iter()
            .position(|p| p.start_beat() == beat)
            .expect("thirty-two one-bar phrases put episode boundaries on the sixteen-beat grid")
            as u32
    };
    let eps = [
        at(0.0),
        at(16.0),
        at(32.0),
        at(48.0),
        at(64.0),
        at(80.0),
        at(96.0),
        at(112.0),
    ];
    let total_phrases = song.plan.form.phrases.len() as u32;
    let span = |i: usize| -> ArcSpan {
        let next = if i + 1 < eps.len() {
            eps[i + 1]
        } else {
            total_phrases
        };
        ArcSpan {
            step: ArgumentStepId(i as u8),
            start_phrase: eps[i],
            phrase_count: next - eps[i],
            start_beat: i as f64 * 16.0,
            span_beats: 16.0,
        }
    };
    let step = |id: u8, referent: u8, relation: ArgumentRelation, deps: &[u8]| ArgumentStep {
        id: ArgumentStepId(id),
        referent: ReferentId(referent),
        transform: ArgumentTransform::default(),
        relation,
        depends_on: deps.iter().copied().map(ArgumentStepId).collect(),
        carriers: vec![Agent::Lead],
    };
    // THE HANDOFF: in the narrative arm the BRIDGE is carried by the KEYS, not the lead. At the
    // departure the lead withholds (the stage takes it off automatically for a non-lead carrier) and
    // the keys own the bridge's whole thematic obligation — the ear-proven lead→keys custody, now
    // authored at the SOURCE so the modern kernel realizes it lawfully. Off (the shipped arm) this is
    // `[Lead]`, byte-identical to before.
    let bridge_carriers = if keys_bridge {
        vec![Agent::Keys]
    } else {
        vec![Agent::Lead]
    };
    let argument = MusicalArgument {
        family: ArgumentFamily::CallAndEarnedAnswer,
        referents: vec![verse, hook, bridge],
        ending: ArgumentEnding::Resolved,
        steps: vec![
            // ep0  VERSE 1  — establish the open question.
            step(0, 0, ArgumentRelation::Establish, &[]),
            // ep1  CHORUS 1 — the hook ANSWERS the verse (altered consequent); arrives full.
            step(
                1,
                1,
                ArgumentRelation::Consequent {
                    antecedent: ArgumentStepId(0),
                },
                &[0],
            ),
            // ep2  VERSE 2  — restate the verse (sparse again; contrast restored).
            step(2, 0, ArgumentRelation::Establish, &[]),
            // ep3  CHORUS 2 — the hook answers verse 2 (the refrain, pre-bridge).
            step(
                3,
                1,
                ArgumentRelation::Consequent {
                    antecedent: ArgumentStepId(2),
                },
                &[2],
            ),
            // ep4  BRIDGE   — depart from the hook (recycles a cell, lifts, ends open). In the
            //                narrative arm the KEYS carry this departure (the lead withholds).
            ArgumentStep {
                id: ArgumentStepId(4),
                referent: ReferentId(2),
                transform: ArgumentTransform::default(),
                relation: ArgumentRelation::Depart {
                    source: ArgumentStepId(1),
                },
                depends_on: vec![ArgumentStepId(1)],
                carriers: bridge_carriers.clone(),
            },
            // ep5  CHORUS 3 — RETURN to the hook, earned: depends on the bridge, so the keys quote the
            //                 journey home. A different lawful bridge comes home different.
            step(
                5,
                1,
                ArgumentRelation::Return {
                    source: ArgumentStepId(1),
                },
                &[1, 4],
            ),
            // ep6  CHORUS 4 — earned homecoming, held.
            step(
                6,
                1,
                ArgumentRelation::Return {
                    source: ArgumentStepId(1),
                },
                &[1, 4],
            ),
            // ep7  CHORUS 5 — earned, final; lands the song home on the tonic (Resolved ending).
            step(
                7,
                1,
                ArgumentRelation::Return {
                    source: ArgumentStepId(1),
                },
                &[1, 4],
            ),
        ],
        arc: Some((0..8).map(span).collect()),
    };
    (argument, song)
}

/// The shipped full-song source (bridge seed == song seed), with the plain-delegating convenience.
pub fn source_song(seed: u64) -> CompiledArgument {
    source_song_phrased(seed, Phrasing::Plain)
}

/// [`source_song`] with an explicit lead [`Phrasing`] (shipped arc: bridge seed == song seed).
pub fn source_song_phrased(seed: u64, phrasing: Phrasing) -> CompiledArgument {
    source_song_with_bridge_seed_phrased(seed, seed, phrasing)
}

/// [`source_song`] with the bridge motif drawn from `bridge_seed` — the compiled counterfactual arm.
/// The route (verse ++ hook ++ verse ++ hook ++ bridge ++ hook ++ hook ++ hook) is identical for every
/// `bridge_seed`; only the bridge's realized motif (and what the band lawfully derives from it at the
/// post-bridge choruses) changes between arms.
pub fn source_song_with_bridge_seed_phrased(
    seed: u64,
    bridge_seed: u64,
    phrasing: Phrasing,
) -> CompiledArgument {
    compile_song(seed, bridge_seed, phrasing, false)
}

/// The NARRATIVE arm (`--form=song-narrative`): identical to [`source_song`] EXCEPT the bridge is
/// carried by the KEYS. At the departure the lead WITHHOLDS and the keys own the bridge's whole
/// thematic obligation — the ear-proven lead→keys custody (accepted 2026-10-09 on the hand-authored
/// `rick_probe`), recovered inside the modern source-first kernel. The theme then survives a
/// lead-mute. Opt-in; the shipped `source_song` is byte-identical to before.
pub fn source_song_narrative(seed: u64) -> CompiledArgument {
    source_song_narrative_phrased(seed, Phrasing::Plain)
}

/// [`source_song_narrative`] with an explicit lead [`Phrasing`] (bridge seed == song seed).
pub fn source_song_narrative_phrased(seed: u64, phrasing: Phrasing) -> CompiledArgument {
    compile_song(seed, seed, phrasing, true)
}

fn compile_song(
    seed: u64,
    bridge_seed: u64,
    phrasing: Phrasing,
    keys_bridge: bool,
) -> CompiledArgument {
    let (argument, song) =
        argument_song_with_bridge_seed_phrased(seed, bridge_seed, phrasing, keys_bridge);
    let mut compiled = argument
        .compile(&song)
        .expect("seeded eight-section full-song source compiles");
    // The 32-bar route: VERSE ++ HOOK ++ VERSE ++ HOOK ++ BRIDGE ++ HOOK ++ HOOK ++ HOOK. The verse
    // travels to its open V7 (route[2]=(7,Dom7), route[3] home); the first hook lands home at bar 7
    // (route[7] home); the bridge pivots and ends open on the home dominant; and the SONG lands home
    // on the tonic at its final bar (route.last() home) — the Resolved ending's earned home-coming.
    // Declared here so the long-form domain and witness read the exact traveling harmony.
    let mut route = Vec::with_capacity(32);
    route.extend_from_slice(&VERSE_PROG);
    route.extend_from_slice(&HOOK_PROG);
    route.extend_from_slice(&VERSE_PROG);
    route.extend_from_slice(&HOOK_PROG);
    route.extend_from_slice(&BRIDGE_PROG);
    route.extend_from_slice(&HOOK_PROG);
    route.extend_from_slice(&HOOK_PROG);
    route.extend_from_slice(&HOOK_PROG);
    compiled.route = route;
    // The capability booleans track the lead phrasing exactly as the 16-bar arc sets them (the
    // example's `Resolved::apply_caps` then overrides them per the resolved preset).
    compiled.answer_breaths = matches!(phrasing, Phrasing::Ensemble);
    compiled.comp_keys = phrasing.lead_is_expressive();
    compiled.anticipate_bass = phrasing.lead_is_expressive();
    compiled.translate_lead_rhythm = phrasing.lead_is_expressive();
    compiled
}

// --- The front-intro arm --------------------------------------------------------------------------
// A real pop song opens; the eight-section arc above does not — it starts cold on the verse. An INTRO
// cannot be faked without lying: a cold-open on the hook inverts the call/answer argument (the hook is
// the verse's ANSWER), and relabelling a quieter verse "Intro" is exactly the bar-range template this
// module refuses. The honest intro the machinery DOES admit is "establish the pocket, earn the
// entrance": bass + drums lay the shared pocket alone, a thin lead enters LATE, and the first verse
// then states the head in full. The lead cannot be fully silenced (every long-form placement is
// hard-required to carry the lead), so the intro's lead is a sparse LATE pickup rather than absent.

/// The intro's thin, late-entering lead line: a sparse rising pickup drawn from the verse's OWN head,
/// re-timed to the back of the sixteen-beat episode so bass + drums establish the shared pocket alone
/// before the lead enters. Because the pitches are the verse's first steps (re-anchored to this
/// referent), the intro FORESHADOWS the head the verse then states in full, and because they come from
/// developed, feasible material the cell passes `validate_material` by construction — a genuine
/// fragment of the song, never new exogenous material.
fn intro_pickup(verse: &[MaterialEvent]) -> Vec<MaterialEvent> {
    let steps: Vec<i32> = verse.iter().filter_map(|e| e.step).take(4).collect();
    let base = steps.first().copied().unwrap_or(0);
    // A long tonic settling, then a three-note rise that lifts into the verse downbeat at beat 16. The
    // first lead onset is beat 9, so beats 0..9 of the song are pure pocket (bass + drums, keys Silent).
    let onsets = [9.0_f64, 12.0, 14.0, 15.0];
    let durs = [2.6_f64, 1.6, 0.9, 0.9];
    let accents = [0.38_f32, 0.50, 0.62, 0.74];
    steps
        .iter()
        .zip(onsets)
        .zip(durs)
        .zip(accents)
        .map(|(((s, onset), dur), accent)| MaterialEvent {
            onset,
            dur,
            accent,
            step: Some(s - base),
        })
        .collect()
}

/// The eight-section song argument WITH a front intro (bridge seed == song seed, plain lead).
pub fn argument_song_intro(seed: u64) -> (MusicalArgument, SongMap) {
    argument_song_intro_with_bridge_seed_phrased(seed, seed, Phrasing::Plain)
}

/// [`argument_song_intro`] with the bridge motif drawn from `bridge_seed` and an explicit [`Phrasing`].
/// Same developed verse/hook/bridge referents as [`argument_song`]; the arc gains a leading `Establish`
/// of a sparse intro referent and drops one trailing chorus to keep the 128-beat budget:
/// INTRO → VERSE → CHORUS → VERSE → CHORUS → BRIDGE → CHORUS → CHORUS. The verse `depends_on` the intro
/// (a provenance edge the witness enforces for ordering), and the intro's keys are seated Silent so the
/// intro's keys call goes UNANSWERED — the verse's keys then discharge it by quoting the intro's cell,
/// a real intro→verse causal edge rather than a positional label.
pub fn argument_song_intro_with_bridge_seed_phrased(
    seed: u64,
    bridge_seed: u64,
    phrasing: Phrasing,
) -> (MusicalArgument, SongMap) {
    let trace = SemanticTrace {
        total_beats: 128.0,
        events: vec![
            SemanticEvent {
                at_beat: 0.0,
                state: SemanticState::toned(Tone::Accent),
                kind: EventKind::ActChanged,
            },
            SemanticEvent {
                at_beat: 124.0,
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
        128.0,
        contract,
    );
    for p in &mut song.plan.arrangement.phrases {
        p.lead = ArrangementRole::Support;
        p.keys = ArrangementRole::Support;
        p.bass = ArrangementRole::Foundation;
        p.drums = ArrangementRole::Support;
        p.pad = ArrangementRole::Support;
    }
    let pair = generate_developed_pair_phrased(seed, 16.0, 4.0, &VERSE_PROG, &HOOK_PROG, phrasing);
    let bridge_events = generate_developed_bridge_phrased(
        bridge_seed,
        &pair.hook,
        16.0,
        4.0,
        &BRIDGE_PROG,
        phrasing,
    );
    let intro_events = intro_pickup(&pair.verse);
    let verse = MusicalReferent {
        id: ReferentId(0),
        pitch_basis: PitchBasis::Semitones,
        events: pair.verse,
    };
    let hook = MusicalReferent {
        id: ReferentId(1),
        pitch_basis: PitchBasis::Semitones,
        events: pair.hook,
    };
    let bridge = MusicalReferent {
        id: ReferentId(2),
        pitch_basis: PitchBasis::Semitones,
        events: bridge_events,
    };
    let intro = MusicalReferent {
        id: ReferentId(3),
        pitch_basis: PitchBasis::Semitones,
        events: intro_events,
    };
    let at = |beat: f64| -> u32 {
        song.plan
            .form
            .phrases
            .iter()
            .position(|p| p.start_beat() == beat)
            .expect("thirty-two one-bar phrases put episode boundaries on the sixteen-beat grid")
            as u32
    };
    let eps = [
        at(0.0),
        at(16.0),
        at(32.0),
        at(48.0),
        at(64.0),
        at(80.0),
        at(96.0),
        at(112.0),
    ];
    // Seat the intro's keys Silent (the four phrases of episode 0). The intro's keys call then goes
    // unanswered (produced == 0), so the verse's keys discharge it by quoting the intro's cell — the
    // honest intro→verse causal edge. Bass and drums stay on, so the intro is pocket + thin late lead.
    for p in &mut song.plan.arrangement.phrases[eps[0] as usize..eps[1] as usize] {
        p.keys = ArrangementRole::Silent;
    }
    let total_phrases = song.plan.form.phrases.len() as u32;
    let span = |i: usize| -> ArcSpan {
        let next = if i + 1 < eps.len() {
            eps[i + 1]
        } else {
            total_phrases
        };
        ArcSpan {
            step: ArgumentStepId(i as u8),
            start_phrase: eps[i],
            phrase_count: next - eps[i],
            start_beat: i as f64 * 16.0,
            span_beats: 16.0,
        }
    };
    let step = |id: u8, referent: u8, relation: ArgumentRelation, deps: &[u8]| ArgumentStep {
        id: ArgumentStepId(id),
        referent: ReferentId(referent),
        transform: ArgumentTransform::default(),
        relation,
        depends_on: deps.iter().copied().map(ArgumentStepId).collect(),
        carriers: vec![Agent::Lead],
    };
    let argument = MusicalArgument {
        family: ArgumentFamily::CallAndEarnedAnswer,
        referents: vec![verse, hook, bridge, intro],
        ending: ArgumentEnding::Resolved,
        steps: vec![
            // ep0  INTRO    — establish the pocket; the thin lead enters late (foreshadows the verse).
            step(0, 3, ArgumentRelation::Establish, &[]),
            // ep1  VERSE 1  — state the open question in full; depends on the intro it answers.
            step(1, 0, ArgumentRelation::Establish, &[0]),
            // ep2  CHORUS 1 — the hook ANSWERS the verse (altered consequent); arrives full.
            step(
                2,
                1,
                ArgumentRelation::Consequent {
                    antecedent: ArgumentStepId(1),
                },
                &[1],
            ),
            // ep3  VERSE 2  — restate the verse (sparse again; contrast restored).
            step(3, 0, ArgumentRelation::Establish, &[]),
            // ep4  CHORUS 2 — the hook answers verse 2 (the refrain, pre-bridge).
            step(
                4,
                1,
                ArgumentRelation::Consequent {
                    antecedent: ArgumentStepId(3),
                },
                &[3],
            ),
            // ep5  BRIDGE   — depart from the hook (recycles a cell, lifts, ends open).
            step(
                5,
                2,
                ArgumentRelation::Depart {
                    source: ArgumentStepId(2),
                },
                &[2],
            ),
            // ep6  CHORUS 3 — RETURN to the hook, earned: depends on the bridge, so the keys quote the
            //                 journey home. A different lawful bridge comes home different.
            step(
                6,
                1,
                ArgumentRelation::Return {
                    source: ArgumentStepId(2),
                },
                &[2, 5],
            ),
            // ep7  CHORUS 4 — earned, final; lands the song home on the tonic (Resolved ending).
            step(
                7,
                1,
                ArgumentRelation::Return {
                    source: ArgumentStepId(2),
                },
                &[2, 5],
            ),
        ],
        arc: Some((0..8).map(span).collect()),
    };
    (argument, song)
}

/// The shipped full-song-with-intro source (bridge seed == song seed), plain-delegating convenience.
pub fn source_song_intro(seed: u64) -> CompiledArgument {
    source_song_intro_phrased(seed, Phrasing::Plain)
}

/// [`source_song_intro`] with an explicit lead [`Phrasing`] (bridge seed == song seed).
pub fn source_song_intro_phrased(seed: u64, phrasing: Phrasing) -> CompiledArgument {
    source_song_intro_with_bridge_seed_phrased(seed, seed, phrasing)
}

/// [`source_song_intro`] with the bridge motif drawn from `bridge_seed` — the intro arm's earned-return
/// counterfactual. Route: VERSE ++ VERSE ++ HOOK ++ VERSE ++ HOOK ++ BRIDGE ++ HOOK ++ HOOK. The intro
/// carries the verse chart so the absolute route pins still hold (bar 2 = the open V7, bars 3/7 home,
/// the final bar home).
pub fn source_song_intro_with_bridge_seed_phrased(
    seed: u64,
    bridge_seed: u64,
    phrasing: Phrasing,
) -> CompiledArgument {
    let (argument, song) =
        argument_song_intro_with_bridge_seed_phrased(seed, bridge_seed, phrasing);
    let mut compiled = argument
        .compile(&song)
        .expect("seeded eight-section intro full-song source compiles");
    let mut route = Vec::with_capacity(32);
    route.extend_from_slice(&VERSE_PROG); // ep0 INTRO (verse chart: pins the open V7 at bar 2)
    route.extend_from_slice(&VERSE_PROG); // ep1 VERSE 1
    route.extend_from_slice(&HOOK_PROG); // ep2 CHORUS 1
    route.extend_from_slice(&VERSE_PROG); // ep3 VERSE 2
    route.extend_from_slice(&HOOK_PROG); // ep4 CHORUS 2
    route.extend_from_slice(&BRIDGE_PROG); // ep5 BRIDGE
    route.extend_from_slice(&HOOK_PROG); // ep6 CHORUS 3 (earned return)
    route.extend_from_slice(&HOOK_PROG); // ep7 CHORUS 4 (earned return, final)
    compiled.route = route;
    compiled.answer_breaths = matches!(phrasing, Phrasing::Ensemble);
    compiled.comp_keys = phrasing.lead_is_expressive();
    compiled.anticipate_bass = phrasing.lead_is_expressive();
    compiled.translate_lead_rhythm = phrasing.lead_is_expressive();
    // The intro BUILDS (rhythm section enters reduced and swells into the first verse) — set here only,
    // so the intro arm is the one place it fires; every other source leaves it default-false (full
    // density). Gated with the sibling capabilities so a plain-lead intro is the full-density control.
    compiled.intro_build = phrasing.lead_is_expressive();
    compiled
}
