//! OPERATION THE HOMOSAPIANS — the REHEARSAL COMPOSER: the band WRITES the song.
//!
//! Every prior Homosapians cut authored a finished backing and made the players perform it. The
//! owner's next ask: construct the song *through musicians collaborating*. A seed brings a germ; the
//! guitarist discovers a two-bar riff; the bassist hears THAT riff and finds something that grooves
//! with it; the drummer hears both; the guitarist hears the whole band and REVISES. The song is the
//! output of the negotiation, not a page the players read.
//!
//! This module is the first executable vertical slice of that process. It is PURE SYMBOLIC logic —
//! a *pre-source* composer that negotiates a [`CompiledArgument`], which the existing witnessed
//! kernel then renders with ZERO kernel edits. The two causal clocks are kept distinct:
//!   * performance time `t` — immutable once frozen (the existing Score/kernel laws own it);
//!   * rehearsal revision time `k` — the guitarist may revise an earlier bar in draft `k+1`, while
//!     the audition take of draft `k` stays a frozen historical trial ([`TrialTake`]). That is not
//!     retrocausality: a revision makes a NEW source, it never edits a committed performance.
//!
//! What is PROVEN here (symbolically, no ear needed): the seed picks the material (not fixed
//! per-flavor constants); the bassist's choice is a deterministic function of the riff's rhythm, so
//! a different riff provably yields a different bass; the drummer's choice is a function of the bass;
//! the guitarist revises AFTER hearing the band; rejected proposals do not contaminate the draft;
//! and the frozen Score carries LITERALLY ZERO Lead notes (the fake 0.06-velocity carrier is gone) —
//! the keys COMP the chords (the AM-radio chordal bed, not a single-note line trying to lead) over a
//! Keys carrier that holds only a soft chord-tone fill, lawful because the carrier witness now excludes
//! the kernel's comp notes. What is NOT proven here: that it is catchy. The owner's ear owns that.
//!
//! Opt-in; no C137 source is touched; nothing is promoted.

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
    rng::Rng,
    semantic::{EventKind, SemanticEvent, SemanticState, SemanticTrace, Tone},
    theme_family::{pocket_offset, AEOLIAN},
    theory::{PitchBasis, Quality},
    SongMap,
};

// ======================================================================================
// Seed whitening + in-world vocabulary
// ======================================================================================

/// SplitMix-style whitener (copied from the crate's private `theme_family::mix`, which is not
/// exported). A raw seed like `7` draws weakly out of the bare xorshift state, so every rehearsal
/// RNG is seeded through this mix with a per-role `tag` and per-candidate `salt`.
fn mix(seed: u64, salt: u64, tag: u64) -> u64 {
    (seed ^ tag)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(salt.wrapping_mul(0xD1B5_4A32_D192_ED03))
}

const TAG_RIFF: u64 = 0x5249_4646; // "RIFF"
const TAG_PICK: u64 = 0x5049_434b; // "PICK"

/// Map an Aeolian scale-degree index (may be negative or > 6) to its semitone offset above the song
/// tonic A, wrapping octaves. Building riffs out of degree indices keeps every pitch in the A-Aeolian
/// pool BY CONSTRUCTION (so the ≥95%-in-scale referent law is satisfied without a rejection loop).
fn aeolian_pitch(degree_index: i32) -> i32 {
    let n = AEOLIAN.len() as i32; // 7
    let oct = degree_index.div_euclid(n);
    let idx = degree_index.rem_euclid(n) as usize;
    AEOLIAN[idx] + 12 * oct
}

// In-world chords, region-relative (semitones above the A tonic, Quality) — the smooth-pop vocabulary
// the teacher licenses (same set the authored flavors draw from). Two-chord "camps" the riff can land in.
const I_MIN9: (i32, Quality) = (0, Quality::Min9); // Am9  (tonic)
const III_MAJ9: (i32, Quality) = (3, Quality::Maj9); // Cmaj9 (relative major, ♭III)
const IV_MIN9: (i32, Quality) = (5, Quality::Min9); // Dm9  (iv)
const V_MIN7: (i32, Quality) = (7, Quality::Min7); // Em7  (modal minor v)
const V_DOM7: (i32, Quality) = (7, Quality::Dom7); // E7   (the dominant, the one cadence colour)
const VI_MAJ9: (i32, Quality) = (8, Quality::Maj9); // Fmaj9 (♭VI)
const VII_DOM9: (i32, Quality) = (10, Quality::Dom9); // G9   (♭VII)

/// A two-chord harmonic world read OUT of the riff (the guitarist interpreting what's there): a VERSE
/// pole-pair — the tonic home plus wherever the riff keeps pointing, a slow repetitive vamp — and a
/// CHORUS that REINTERPRETS it (the semantic possibility of where the loop wants to go: brightened or
/// tonicised, with the one dominant turnaround the vamp was implying). Both derived from the riff's own
/// emphasis, never a default — so the harmony means something about THIS riff, not a hash.
#[derive(Debug, Clone, PartialEq)]
pub struct HarmonyCamp {
    pub verse: [(i32, Quality); 2],
    pub chorus: [(i32, Quality); 2],
    pub label: String,
}

/// The diatonic second chord keyed by a scale degree's pitch class (semitones above the A tonic): the
/// verse loop is "the tonic home plus the chord that best carries the riff's most-dwelt-on note".
fn second_chord_for_pc(pc: i32) -> ((i32, Quality), &'static str) {
    match pc.rem_euclid(12) {
        5 => (IV_MIN9, "iv"),     // D (the 4th) implies iv — Dm9
        8 => (VI_MAJ9, "bVI"),    // F (the b6) implies bVI — Fmaj9
        10 => (VII_DOM9, "bVII"), // G (the b7) implies bVII — G9
        7 => (V_MIN7, "v"),       // E (the 5th) leans to the modal minor v — Em7
        // A riff that mostly dwells on tonic-chord tones (C = b3, B = 9) does NOT imply leaving i for
        // the CLICHED relative major bIII — it leans to the warm bVI instead. bIII is reserved for the
        // CHORUS arrival (the accepted lift), so it can never be the verse loop the owner flagged tired.
        _ => (VI_MAJ9, "bVI"),
    }
}

/// INTERPRET the riff into its harmonic ground. Weight each pitch class by how long the riff dwells on
/// it; the tonic A is the fixed home pole, and the heaviest NON-tonic degree chooses the loop's second
/// chord — so the verse loop is READ OUT of the riff, not picked by a hash (which is what made every
/// seed land on the same clichéd relative-major camp). The CHORUS reinterprets that loop toward where
/// it wants to go: a dark/subdominant pole (iv or v) brightens the tonic to its relative major ♭III; an
/// already-bright pole (♭VI/♭VII/♭III) is tonicised by the dominant instead — either way the chorus
/// keeps the verse's tonal content and adds the one dominant arrival the vamp was implying.
pub fn interpret_harmony(events: &[MaterialEvent]) -> HarmonyCamp {
    let mut weight = [0.0_f64; 12];
    for e in events {
        if let Some(s) = e.step {
            weight[s.rem_euclid(12) as usize] += e.dur.max(0.05);
        }
    }
    let mut best_pc = 7_i32;
    let mut best_w = -1.0_f64;
    for (pc, &w) in weight.iter().enumerate() {
        if pc != 0 && w > best_w {
            best_w = w;
            best_pc = pc as i32;
        }
    }
    let (x, xlabel) = second_chord_for_pc(best_pc);
    let verse = [I_MIN9, x];
    let (chorus, strategy) = if x == IV_MIN9 || x == V_MIN7 {
        ([III_MAJ9, x], "lift the tonic to bIII")
    } else {
        ([x, V_DOM7], "tonicise the pole on V7")
    };
    HarmonyCamp {
        verse,
        chorus,
        label: format!("i-{xlabel} loop; chorus: {strategy}"),
    }
}

// ======================================================================================
// Stage B — the guitarist proposes two-bar riffs
// ======================================================================================

/// The guitarist's two-bar idea (8 beats): a monophonic, in-world melodic figure with intentional
/// rests and a recognisable shape, plus the harmonic world it implies and an identity fingerprint.
#[derive(Debug, Clone)]
pub struct RiffCandidate {
    pub source_id: u64,
    pub events: Vec<MaterialEvent>,
    pub camp: HarmonyCamp,
    pub label: String,
}

/// Five two-bar rhythm cells (onsets in beats on the quarter-beat lattice, within [0,8)). Each has a
/// distinct FEEL — an on-beat statement, a syncopated funk line, a sparse call-and-space, a driving
/// eighth line, and an anticipating pickup line. Rests are simply absent onsets. A cool riff can be a
/// few notes and a well-placed silence, so these are deliberately NOT maximised for note count.
const RHYTHMS: [&[f64]; 5] = [
    &[0.0, 1.0, 2.0, 3.5, 4.0, 6.0],                // statement
    &[0.0, 0.75, 1.5, 3.5, 4.0, 4.75, 6.5],         // syncopated funk
    &[0.0, 1.5, 4.0, 5.5],                          // sparse call-and-space
    &[0.0, 0.5, 1.0, 2.0, 2.5, 4.0, 4.5, 5.0, 6.0], // driving eighths
    &[0.0, 2.0, 3.75, 4.0, 6.0, 7.75],              // anticipating pickups
];

const RHYTHM_LABELS: [&str; 5] = [
    "on-beat statement",
    "syncopated funk",
    "sparse call-and-space",
    "driving eighths",
    "anticipating pickups",
];

/// Launch scale-degree indices (into the octave-wrapped Aeolian), the strong places a riff starts
/// from: E (idx 4), A-up (idx 7), G (idx 6), D (idx 3).
const LAUNCHES: [i32; 4] = [4, 7, 6, 3];

/// Generate `n` distinct two-bar riff candidates deterministically from `seed`. Each candidate draws
/// its own whitened RNG (`mix(seed, k, TAG_RIFF)`), picks a rhythm cell, a launch degree and a contour
/// bias, then walks the Aeolian pool along that contour — landing its final note on the tonic-or-fifth
/// so the figure sounds resolved-but-open. Candidates that collide (same rhythm index AND same pitch
/// set) with one already kept are skipped, so the set is genuinely varied, not one contour re-painted.
pub fn generate_riffs(seed: u64, n: usize) -> Vec<RiffCandidate> {
    let mut out: Vec<RiffCandidate> = Vec::new();
    let mut k: u64 = 0;
    let mut guard = 0;
    while out.len() < n && guard < (n as u64 * 16 + 32) {
        guard += 1;
        let mut rng = Rng::new(mix(seed, k, TAG_RIFF));
        k += 1;

        let r_idx = rng.below(RHYTHMS.len());
        let onsets = RHYTHMS[r_idx];
        let launch = *rng.pick(&LAUNCHES).unwrap();
        let contour = rng.below(4); // 0 rising, 1 falling, 2 arch, 3 wave

        let mut events = Vec::with_capacity(onsets.len());
        let mut deg = launch;
        let count = onsets.len();
        for (i, &onset) in onsets.iter().enumerate() {
            let frac = i as f32 / (count.max(2) - 1) as f32;
            // Contour drives the next scale-step; the magnitude (1 or 2 degrees) is seeded.
            let span = if rng.chance(0.35) { 2 } else { 1 };
            let dir = match contour {
                0 => 1,  // rising
                1 => -1, // falling
                2 => {
                    if frac < 0.5 {
                        1
                    } else {
                        -1
                    }
                } // arch
                _ => {
                    if i % 2 == 0 {
                        1
                    } else {
                        -1
                    }
                } // wave
            };
            if i > 0 {
                deg += dir * span;
            }
            // Keep the line in a singable register (A3..~C5 => semitones 0..16 above tonic A).
            deg = deg.clamp(-1, 10);
            let mut step = aeolian_pitch(deg);
            // Land the final note home: tonic (0 or 12) or fifth (7), whichever is nearer the contour end.
            if i + 1 == count {
                let home = if step >= 9 {
                    12
                } else if step <= 3 {
                    0
                } else {
                    7
                };
                step = home;
            }
            let next = onsets.get(i + 1).copied().unwrap_or(8.0);
            let gap = (next - onset).max(0.25);
            let dur = (gap * 0.9).min(1.2);
            let accent = 0.52 + 0.10 * rng.next_f32(); // 0.52..0.62, a human breath, never flat
            events.push(MaterialEvent {
                onset,
                dur,
                accent,
                step: Some(step),
            });
        }

        let camp = interpret_harmony(&events);
        let source_id = fingerprint(&events);

        // Distinctness: skip if some kept candidate has the same rhythm index AND identical pitch set.
        let pitches: Vec<i32> = events.iter().filter_map(|e| e.step).collect();
        let dup = out.iter().any(|c| {
            let cp: Vec<i32> = c.events.iter().filter_map(|e| e.step).collect();
            c.events.len() == events.len() && cp == pitches
        });
        if dup {
            continue;
        }

        let label = format!(
            "{} | launch {} | {} | {}",
            RHYTHM_LABELS[r_idx],
            launch,
            ["rising", "falling", "arch", "wave"][contour],
            camp.label
        );
        out.push(RiffCandidate {
            source_id,
            events,
            camp,
            label,
        });
    }
    out
}

/// A content fingerprint of a monophonic line: folds the quantised (onset, step, dur) of each event.
/// Deterministic and order-sensitive; used for riff identity and draft-revision immutability.
pub fn fingerprint(events: &[MaterialEvent]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV-ish offset
    for e in events {
        let o = (e.onset * 16.0).round() as i64 as u64;
        let d = (e.dur * 16.0).round() as i64 as u64;
        let s = e.step.unwrap_or(-99) as i64 as u64;
        for v in [o, d, s] {
            h ^= v.wrapping_add(0x9E37_79B9_7F4A_7C15);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// The band's TASTE: how syncopated a line FEELS — the life that makes a riff groove instead of chug.
/// Measured on the 16th-note lattice over the 2-bar cell (8 beats = 32 slots), two contributions:
///  - Longuet-Higgins/Lee displacement: a note on a weak slot followed by SILENCE over a stronger beat
///    "steals" that beat's emphasis (the classic off-beat pop); its strength is the stolen beat's
///    metrical weight minus the note's. Driving eighths fill every strong beat, so they steal nothing
///    and score near zero — exactly the on-the-grid "chug" the ear rejects.
///  - Anticipation: a note one 16th BEFORE a strong beat leans into it (a push), credited even when the
///    beat is also played, because the early attack is what the ear hears as drive.
///
/// Higher = more syncopated; a pure on-beat statement scores 0. This is the value the band selects on.
pub fn syncopation_score(onsets: &[f64]) -> f64 {
    // Metrical weight of a within-bar 16th slot: downbeat 4, beat 3 -> 3, beats 2 & 4 -> 2,
    // eighth "ands" -> 1, sixteenths -> 0.
    fn weight(slot: i64) -> f64 {
        match slot.rem_euclid(16) {
            0 => 4.0,
            8 => 3.0,
            4 | 12 => 2.0,
            2 | 6 | 10 | 14 => 1.0,
            _ => 0.0,
        }
    }
    let slots: Vec<i64> = onsets
        .iter()
        .filter(|&&o| (0.0..8.0).contains(&o))
        .map(|&o| (o * 4.0).round() as i64)
        .collect();
    let mut score = 0.0;
    for (i, &s) in slots.iter().enumerate() {
        let s_next = slots.get(i + 1).copied().unwrap_or(32);
        // Strongest EMPTY beat between this note and the next steals emphasis (L-H-L displacement).
        let mut max_rest = 0.0_f64;
        for r in (s + 1)..s_next {
            max_rest = max_rest.max(weight(r));
        }
        let w = weight(s);
        if max_rest > w {
            score += max_rest - w;
        }
        // Anticipation: this note is the 16th right before a strong beat (slot+1 lands on a beat).
        if (s + 1).rem_euclid(4) == 0 {
            let nb = weight(s + 1);
            if nb >= 2.0 {
                score += nb - 1.0; // a push into the downbeat (+3), beat 3 (+2), or beats 2/4 (+1)
            }
        }
    }
    score
}

// ======================================================================================
// Stage C — the bassist responds to THAT riff
// ======================================================================================

/// The bassist's response: four per-bar cells (the kernel plays `cells[bar % len]`), plus the plain
/// reasons each choice was made. Every cell opens on `(0.0, Root)` (the foundation law). Crucially the
/// cells are a DETERMINISTIC FUNCTION of the riff's rhythm — not an RNG draw — so a different riff
/// provably yields a different bass, and that difference is traceable to a named riff feature.
#[derive(Debug, Clone, PartialEq)]
pub struct BassResponse {
    pub cells: Vec<Vec<(f64, BassDegree, f32, f32)>>,
    pub reasons: Vec<String>,
}

/// How busy the riff is in a given bar (onsets whose beat falls in `[bar*4, bar*4+4)`).
fn bar_density(events: &[MaterialEvent], bar: usize) -> usize {
    let lo = bar as f64 * 4.0;
    let hi = lo + 4.0;
    events
        .iter()
        .filter(|e| e.onset >= lo && e.onset < hi)
        .count()
}

/// The largest silent window (in beats) inside a bar — the riff's negative space the bass can fill.
fn bar_biggest_rest(events: &[MaterialEvent], bar: usize) -> f64 {
    let lo = bar as f64 * 4.0;
    let hi = lo + 4.0;
    let mut ons: Vec<f64> = events
        .iter()
        .map(|e| e.onset)
        .filter(|o| *o >= lo && *o < hi)
        .collect();
    ons.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut prev = lo;
    let mut best = 0.0_f64;
    for o in &ons {
        best = best.max(o - prev);
        prev = *o;
    }
    best = best.max(hi - prev);
    best
}

/// Does the riff push the very end (an onset in the last half-beat of either bar)? That is a pickup
/// the bassist can answer by anticipating the turnaround.
fn riff_has_pickup(events: &[MaterialEvent]) -> bool {
    events
        .iter()
        .any(|e| (3.5..4.0).contains(&e.onset) || (7.5..8.0).contains(&e.onset))
}

/// Build the bassist's response to a riff. `driven` raises the energy (the chorus pass): an extra
/// attack and hotter velocities, the same shapes louder — the arrival comes from energy, not a
/// faster harmonic rate. The turnaround cell drops to the dark ♭7 (preserved from the accepted work).
pub fn bass_for_riff(riff: &RiffCandidate, driven: bool) -> BassResponse {
    let v = if driven { 0.10 } else { 0.0 }; // velocity lift when driven
    let d0 = bar_density(&riff.events, 0);
    let d1 = bar_density(&riff.events, 1);
    let busy = d0 + d1 >= 7; // the riff is dense -> the bass stays out of the way
    let rest0 = bar_biggest_rest(&riff.events, 0);
    let rest1 = bar_biggest_rest(&riff.events, 1);
    let pickup = riff_has_pickup(&riff.events);

    let mut cells: Vec<Vec<(f64, BassDegree, f32, f32)>> = Vec::with_capacity(4);
    let mut reasons: Vec<String> = Vec::new();

    // Bar 0 — statement. If the riff is busy here, the bass SIMPLIFIES to a root pedal with one fifth.
    if busy || d0 >= 5 {
        cells.push(vec![
            (0.0, BassDegree::Root, 0.6, 0.80 + v),
            (2.0, BassDegree::Fifth, 0.5, 0.56 + v),
        ]);
        reasons.push(format!(
            "bar0: riff busy (density {d0}) -> bass simplifies to a root pedal + one fifth, staying out of the way"
        ));
    } else {
        cells.push(vec![
            (0.0, BassDegree::Root, 0.6, 0.80 + v),
            (0.75, BassDegree::Octave, 0.22, 0.46 + v),
            (1.5, BassDegree::Fifth, 0.5, 0.56 + v),
            (2.5, BassDegree::Root, 0.5, 0.60 + v),
        ]);
        reasons.push(format!(
            "bar0: riff open (density {d0}) -> bass bounces root/octave/fifth to carry the pocket"
        ));
    }

    // Bar 1 — answer. If the riff left a big rest here, the bass WALKS into that window.
    if rest1 >= 1.5 {
        cells.push(vec![
            (0.0, BassDegree::Root, 0.45, 0.76 + v),
            (1.0, BassDegree::Fifth, 0.4, 0.50 + v),
            (2.0, BassDegree::Octave, 0.4, 0.54 + v),
            (2.75, BassDegree::Fifth, 0.5, 0.52 + v),
        ]);
        reasons.push(format!(
            "bar1: riff leaves a {rest1:.2}-beat rest -> bass fills it with a walking answer"
        ));
    } else {
        cells.push(vec![
            (0.0, BassDegree::Root, 0.5, 0.76 + v),
            (2.0, BassDegree::Fifth, 0.5, 0.56 + v),
        ]);
        reasons.push(format!(
            "bar1: riff fills the bar (rest {rest1:.2}) -> bass lays back, root + fifth only"
        ));
    }

    // Bar 2 — drive (re-reads bar 0's shape, a touch busier). Mirrors bar 0 density.
    if busy {
        cells.push(vec![
            (0.0, BassDegree::Root, 0.5, 0.82 + v),
            (2.0, BassDegree::Fifth, 0.5, 0.58 + v),
        ]);
        reasons.push("bar2: still busy above -> bass holds the simplified drive".to_string());
    } else {
        cells.push(vec![
            (0.0, BassDegree::Root, 0.5, 0.82 + v),
            (0.75, BassDegree::Octave, 0.22, 0.48 + v),
            (1.5, BassDegree::Fifth, 0.4, 0.62 + v),
            (2.75, BassDegree::Fifth, 0.5, 0.54 + v),
        ]);
        reasons.push(format!(
            "bar2: room above (rest0 {rest0:.2}) -> bass drives with an octave bounce"
        ));
    }

    // Bar 3 — turnaround. If the riff pushed a pickup, the bass ANTICIPATES with an and-of-4 push;
    // otherwise it settles. The ♭7 descent colours the cadence either way.
    if pickup {
        cells.push(vec![
            (0.0, BassDegree::Root, 0.7, 0.76 + v),
            (1.0, BassDegree::Seventh, 0.4, 0.52 + v), // dark ♭7 descent at the turn
            (2.5, BassDegree::Root, 0.4, 0.56 + v),
            (3.5, BassDegree::Octave, 0.45, 0.66 + v), // the anticipation the riff's pickup invited
        ]);
        reasons.push(
            "bar3: riff ends on a pickup -> bass anticipates the turn with an and-of-4 push + ♭7"
                .to_string(),
        );
    } else {
        cells.push(vec![
            (0.0, BassDegree::Root, 0.7, 0.76 + v),
            (1.0, BassDegree::Seventh, 0.4, 0.52 + v), // dark ♭7 descent at the turn
            (2.5, BassDegree::Root, 0.4, 0.56 + v),
        ]);
        reasons.push(
            "bar3: riff settles -> bass turns around on the ♭7, no pushed anticipation".to_string(),
        );
    }

    BassResponse { cells, reasons }
}

// ======================================================================================
// Stage D — the drummer responds to the bass (and the riff)
// ======================================================================================

/// The drummer's discretionary decisions, per bar, over a 4-bar phrase. The backbone (a steady kick
/// spine + 2-and-4 backbone) is always kept; what the drummer CHOOSES — where to reinforce the bass,
/// where to leave a gap, where to drop a fill — is a function of the committed bass. The kernel's
/// `pop_funk_kit` renders the actual kit from the frozen bass; this record is the drummer's intent and
/// drives the back-edge (a busy bass+fill collision is what the guitarist later hears and answers).
#[derive(Debug, Clone, PartialEq)]
pub struct DrumResponse {
    /// Per bar: does the kick reinforce the bass's off-beat push?
    pub reinforce_push: Vec<bool>,
    /// Per bar: does the drummer leave a gap (stay out) instead of answering?
    pub leave_gap: Vec<bool>,
    /// Per bar: does a fill land here?
    pub fill: Vec<bool>,
    pub reasons: Vec<String>,
}

/// The number of attacks in a bass cell.
fn cell_attacks(cell: &[(f64, BassDegree, f32, f32)]) -> usize {
    cell.len()
}

/// Does a bass cell push the and-of-4 (an attack at offset ≥ 3.5)?
fn cell_pushes(cell: &[(f64, BassDegree, f32, f32)]) -> bool {
    cell.iter().any(|(off, _, _, _)| *off >= 3.5)
}

/// Build the drummer's response from the committed bass. Changing the bass's rhythm (its pushes, its
/// busiest bar) changes the drummer's discretionary choices; the spine never moves.
pub fn drums_for_bass(bass: &BassResponse) -> DrumResponse {
    let n = bass.cells.len();
    let mut reinforce_push = vec![false; n];
    let mut leave_gap = vec![false; n];
    let mut fill = vec![false; n];
    let mut reasons = Vec::new();

    // The busiest bar is where a fill is most tempting — but a good drummer fills the TURN, not the
    // busiest bar if that bar is already crowded. So: fill the last bar if it pushes; else the busiest.
    let busiest = (0..n)
        .max_by_key(|&b| cell_attacks(&bass.cells[b]))
        .unwrap_or(0);

    for b in 0..n {
        let pushes = cell_pushes(&bass.cells[b]);
        let attacks = cell_attacks(&bass.cells[b]);
        if pushes {
            // The bass pushed the and-of-4: reinforce it (lock the kick to the push).
            reinforce_push[b] = true;
            reasons.push(format!(
                "bar{b}: bass pushes the and-of-4 -> kick reinforces the push"
            ));
        } else if attacks <= 2 {
            // The bass laid back and left space: the drummer stays out of the way too.
            leave_gap[b] = true;
            reasons.push(format!(
                "bar{b}: bass lays back ({attacks} attacks) -> drummer leaves a gap"
            ));
        } else {
            reasons.push(format!(
                "bar{b}: bass walks ({attacks} attacks) -> drummer keeps the plain pocket"
            ));
        }
    }

    // One fill: the turnaround bar if it pushes, else the busiest bar.
    let fill_bar = if n > 0 && cell_pushes(&bass.cells[n - 1]) {
        n - 1
    } else {
        busiest
    };
    if n > 0 {
        fill[fill_bar] = true;
        reasons.push(format!(
            "fill lands in bar{fill_bar} (the turn if pushed, else the busiest)"
        ));
    }

    DrumResponse {
        reinforce_push,
        leave_gap,
        fill,
        reasons,
    }
}

// ======================================================================================
// Stage E — the guitarist listens AGAIN and revises (the breakthrough back-edge)
// ======================================================================================

/// What kind of revision the guitarist proposed after hearing the whole band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevKind {
    /// Thin out a riff bar that now collides with a drum fill (make room).
    Simplify,
    /// Add a pickup at the end that leads into the next section (a transition the bass's turn invited).
    Transition,
    /// A flashy dense run — tested and REJECTED because it breaks the riff's identity / the pocket.
    Flourish,
    /// CREATE syncopation: push a square interior on-beat note a sixteenth early so it anticipates the
    /// beat — a deliberate interpretation that manufactures groove, not merely picks it.
    Syncopate,
}

/// A logged revision decision: what was proposed, whether the band accepted it, and the plain reason.
#[derive(Debug, Clone)]
pub struct GuitarRevision {
    pub kind: RevKind,
    pub accepted: bool,
    pub reason: String,
}

/// An immutable audition take: a frozen historical trial of draft revision `k`. Revising to `k+1`
/// never edits this — it is the record that performance time stayed causal while rehearsal time moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrialTake {
    pub revision: u32,
    pub fingerprint: u64,
}

// ======================================================================================
// The rehearsal draft (the two clocks) + the full A→F jam
// ======================================================================================

/// The band's song draft at one rehearsal revision `k`. Cloning + a pure `apply` is the whole of the
/// two-clock discipline: a proposal maps draft `k` → a NEW draft `k+1`; a rejected proposal returns
/// the draft unchanged (no contamination); every prior state is snapshotted in `takes` (append-only).
#[derive(Debug, Clone)]
pub struct RehearsalDraft {
    pub revision: u32,
    pub seed: u64,
    pub candidates: Vec<RiffCandidate>,
    pub selected: usize,
    pub bass_verse: BassResponse,
    pub bass_chorus: BassResponse,
    pub drums: DrumResponse,
    pub log: Vec<GuitarRevision>,
    pub takes: Vec<TrialTake>,
    /// The band's CONTEMPLATION — the step-by-step reasoning that shapes the clay toward a song. Each
    /// line is a move or a "step back and look" judgement ("it grooves but it just circles — it needs
    /// somewhere to go"). This is the underlying VISION made legible: the choices serve the goal of
    /// becoming a song, rather than being thrown at the wall to see what vibes.
    pub shaping_log: Vec<String>,
}

impl RehearsalDraft {
    /// The current selected riff.
    pub fn riff(&self) -> &RiffCandidate {
        &self.candidates[self.selected]
    }

    /// Snapshot the current riff as an immutable audition take (does not mutate the draft).
    pub fn audition(&self) -> TrialTake {
        TrialTake {
            revision: self.revision,
            fingerprint: fingerprint(&self.riff().events),
        }
    }
}

/// Run the full rehearsal from a seed and return the negotiated draft. Deterministic.
///
/// The VISION is the song itself as a goal: the seed is a block of clay, each musician makes an
/// INTERPRETIVE move that shapes it, and between moves the band steps back and asks "does this feel
/// like a song yet — where does it need to go?" (the `shaping_log`). The moves: clay → the guitarist
/// reads a riff and the harmonic ground it implies → the bass grooves → the drums compensate →
/// keys/pad fill → CONTEMPLATE ("it circles; it needs somewhere to go") → the guitarist answers the
/// band and that exchange becomes the chorus the loop was heading toward → CONTEMPLATE again → the
/// back-edge revision → done. Every move interprets what is there rather than throwing material at it.
pub fn rehearse(seed: u64) -> RehearsalDraft {
    rehearse_opts(seed, true)
}

/// As [`rehearse`], with `create_syncopation` gating the guitarist's CREATE-syncopation move (Stage E′).
/// It exists only as an A/B control (the brief's negative-control discipline): a render can turn the
/// create-move OFF to hear what it contributes. `rehearse` leaves it ON; every gate uses `rehearse`.
pub fn rehearse_opts(seed: u64, create_syncopation: bool) -> RehearsalDraft {
    let candidates = generate_riffs(seed, 5);
    // The band's pick is TASTE, not a blind draw: SYNCOPATION IS KEY — the life of the groove. Each
    // candidate is weighted by how syncopated it feels, so a straight on-the-grid "chug" (driving
    // eighths / on-beat statement, ~0 score) is all but passed over for an off-beat idea, while the
    // seed still chooses WHICH syncopated riff (so different seeds still write different songs).
    let mut pick_rng = Rng::new(mix(seed, 0, TAG_PICK));
    let onsets_of = |c: &RiffCandidate| -> Vec<f64> { c.events.iter().map(|e| e.onset).collect() };
    let weights: Vec<f64> = candidates
        .iter()
        .map(|c| syncopation_score(&onsets_of(c)))
        .collect();
    let total: f64 = weights.iter().sum();
    let selected = if total <= 0.0 {
        // Degenerate pool (no syncopation to reach for anywhere): fall back to a reproducible draw.
        pick_rng.below(candidates.len())
    } else {
        let mut target = pick_rng.next_f32() as f64 * total;
        let mut idx = candidates.len() - 1;
        for (i, &w) in weights.iter().enumerate() {
            if target < w {
                idx = i;
                break;
            }
            target -= w;
        }
        idx
    };
    let camp = candidates[selected].camp.clone();

    let bass_verse = bass_for_riff(&candidates[selected], false);
    let bass_chorus = bass_for_riff(&candidates[selected], true);
    let drums = drums_for_bass(&bass_verse);

    // The band's contemplation — the clay shaped toward a song, move by move (the underlying vision).
    let mut shaping_log = Vec::new();
    shaping_log.push(format!(
        "CLAY: seed {seed} offers {} riff ideas; the band reaches for the SYNCOPATION (its life) and takes #{selected} — {} (sync {:.0})",
        candidates.len(),
        candidates[selected].label,
        weights[selected]
    ));
    shaping_log.push(format!(
        "GUITARIST reads the riff and hears its harmonic ground: {} (read OUT of the riff, not defaulted)",
        camp.label
    ));
    if let Some(r) = bass_verse.reasons.first() {
        shaping_log.push(format!("BASS interprets the riff -> {r}"));
    }
    shaping_log.push(format!(
        "DRUMS compensate -> {}",
        drums.reasons.last().cloned().unwrap_or_default()
    ));
    shaping_log
        .push("KEYS/PAD fill: the keys comp the loop as chords, the pad beds under it".to_string());
    shaping_log.push(
        "CONTEMPLATE: it's a loop that grooves — but it just circles. A song needs somewhere to GO."
            .to_string(),
    );
    shaping_log.push(
        "GUITARIST answers the band: the loop's 2nd chord wants to move -> the chorus REINTERPRETS it (the semantic possibility of where the vamp was heading)".to_string(),
    );
    shaping_log.push(
        "BASS responds: walk the turnaround into the chorus (the transition the exchange implies)"
            .to_string(),
    );

    let mut draft = RehearsalDraft {
        revision: 0,
        seed,
        candidates,
        selected,
        bass_verse,
        bass_chorus,
        drums,
        log: Vec::new(),
        takes: Vec::new(),
        shaping_log,
    };
    // Record the opening audition (take 0) BEFORE any revision — it must survive untouched.
    let take0 = draft.audition();
    draft.takes.push(take0);

    // Stage E — the guitarist listens to the band and proposes revisions.
    let proposals = guitar_proposals(&draft);
    for (kind, revised, reason) in proposals {
        match (kind, revised) {
            (RevKind::Flourish, _) => {
                // A flashy dense run — REJECTED: it breaks the riff identity / the pocket. The
                // rejected material NEVER enters the draft (no contamination); only the decision logs.
                draft.log.push(GuitarRevision {
                    kind,
                    accepted: false,
                    reason,
                });
            }
            (k, Some(new_events)) if revision_is_lawful(&new_events) => {
                // Accept: advance rehearsal time. The prior take stays frozen in `takes`.
                accept_revision(&mut draft, k, new_events, reason);
            }
            (k, _) => {
                // Proposed but unlawful (would break a referent invariant) -> rejected, no change.
                draft.log.push(GuitarRevision {
                    kind: k,
                    accepted: false,
                    reason: format!("{reason} (rejected: would break a referent invariant)"),
                });
            }
        }
    }

    // Stage E′ — the guitarist CREATES syncopation, not only picks it (the owner's next move): a
    // deliberate interpretation that pushes a square interior beat off the grid so the riff leans into
    // the beat instead of marching on it. Computed from the LIVE riff, so it builds on any Simplify /
    // Transition already accepted (it never overwrites them). The move preserves both bar downbeats
    // (the form) and the final resolution note; it only fires when it genuinely raises the groove.
    if create_syncopation {
        if let Some((new_events, reason)) = syncopate_proposal(&draft.riff().events) {
            if revision_is_lawful(&new_events) {
                draft.shaping_log.push(format!(
                    "CONTEMPLATE: the riff still marches on the grid -> the guitarist PUSHES a beat off it ({reason})"
                ));
                accept_revision(&mut draft, RevKind::Syncopate, new_events, reason);
            }
        }
    }

    // CONTEMPLATE (the band steps back and looks at the whole): record what the revision pass did and
    // whether the piece now reads as a song — a loop that grooves, a contrast it lifts into, and the
    // transition between them.
    let accepted = draft.log.iter().filter(|r| r.accepted).count();
    if let Some(rev) = draft
        .log
        .iter()
        .find(|r| r.accepted && r.kind != RevKind::Syncopate)
    {
        draft.shaping_log.push(format!(
            "CONTEMPLATE: hearing the whole band back, the guitarist REVISES — {}",
            rev.reason
        ));
    } else if accepted == 0 {
        draft.shaping_log.push(
            "CONTEMPLATE: hearing the whole band back, the guitarist leaves the riff as it stands (it already serves the song)".to_string(),
        );
    }
    draft.shaping_log.push(format!(
        "DONE: a loop that grooves + a contrast it lifts into + the transition between = a song shape ({accepted} revision(s) accepted, the flashy flourish rejected)"
    ));
    draft
}

/// The guitarist's proposals after hearing the band. Returns `(kind, revised_events, reason)`; a
/// `None` revised means "diagnostic only". The proposals are a function of the current band state:
/// a Simplify where a drum fill collides with a dense riff bar; a Transition when the bass anticipated
/// the turn; and a Flourish that is always offered and always rejected (the band protects the organism).
pub fn guitar_proposals(
    draft: &RehearsalDraft,
) -> Vec<(RevKind, Option<Vec<MaterialEvent>>, String)> {
    let riff = draft.riff();
    let mut out = Vec::new();

    // SIMPLIFY — the DENSE case: hearing the band (the drummer dropped a fill), a busy riff (≥7 notes)
    // is now too crowded, so the guitarist thins the weakest (softest) non-head note to let the band
    // breathe. The thinned note is chosen in the FILL's own bar when there is one there, so the cut
    // makes room for the exact fill the guitarist heard; otherwise the softest note anywhere. (The
    // research note: "the original guitar riff is now too dense and would sound better with one omitted".)
    if riff.events.len() >= 7 {
        if let Some(fill_bar) = draft.drums.fill.iter().position(|f| *f) {
            let local = (fill_bar % 2) as f64 * 4.0;
            let (lo, hi) = (local, local + 4.0);
            let victim = riff
                .events
                .iter()
                .enumerate()
                .filter(|(i, e)| *i > 0 && e.onset >= lo && e.onset < hi)
                .min_by(|a, b| a.1.accent.partial_cmp(&b.1.accent).unwrap())
                .or_else(|| {
                    riff.events
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i > 0)
                        .min_by(|a, b| a.1.accent.partial_cmp(&b.1.accent).unwrap())
                });
            if let Some((vi, _)) = victim {
                let mut ev = riff.events.clone();
                ev.remove(vi);
                out.push((
                    RevKind::Simplify,
                    Some(ev),
                    format!(
                        "heard the drummer drop a fill in bar{fill_bar}; the riff was too dense -> thin the weakest note to make room"
                    ),
                ));
            }
        }
    }

    // TRANSITION — the SPARSE case: a riff that doesn't already lead into the next section, hearing the
    // bassist turn around on the ♭7, gets a pickup on the last sixteenth that leads the ear into the
    // chorus (a step under the tonic, G = ♭7, pulling back to the top).
    if riff.events.len() < 7 && !riff_has_pickup(&riff.events) {
        let mut ev = riff.events.clone();
        let pickup_onset = 7.75;
        if !ev.iter().any(|e| (e.onset - pickup_onset).abs() < 1e-6) {
            ev.push(MaterialEvent {
                onset: pickup_onset,
                dur: 0.2,
                accent: 0.5,
                step: Some(10),
            });
            ev.sort_by(|a, b| a.onset.partial_cmp(&b.onset).unwrap());
            out.push((
                RevKind::Transition,
                Some(ev),
                "heard the bassist turn around on the ♭7 -> add a pickup that leads into the next section"
                    .to_string(),
            ));
        }
    }

    // FLOURISH: a flashy sixteenth run jammed onto the downbeat. Always proposed, always rejected —
    // it would double the note count and bury the riff's identity. The rejected events are discarded.
    out.push((
        RevKind::Flourish,
        None,
        "a flashy sixteenth run over the whole first bar -> rejected: it buries the riff's identity and the pocket"
            .to_string(),
    ));

    out
}

/// Apply an accepted revision: snapshot the current take (append-only), advance rehearsal time, swap in
/// the new riff events, and re-negotiate the bass/drums to it (the band adjusts to the change). The
/// prior take stays frozen in `takes` — performance time stays causal while rehearsal time moves.
fn accept_revision(
    draft: &mut RehearsalDraft,
    kind: RevKind,
    new_events: Vec<MaterialEvent>,
    reason: String,
) {
    let prior = draft.audition();
    draft.takes.push(prior);
    draft.revision += 1;
    let sel = draft.selected;
    draft.candidates[sel].events = new_events;
    draft.candidates[sel].source_id = fingerprint(&draft.candidates[sel].events);
    draft.bass_verse = bass_for_riff(&draft.candidates[sel], false);
    draft.bass_chorus = bass_for_riff(&draft.candidates[sel], true);
    draft.drums = drums_for_bass(&draft.bass_verse);
    draft.log.push(GuitarRevision {
        kind,
        accepted: true,
        reason,
    });
}

/// CREATE syncopation (the guitarist's generative move, not a pick): push ONE square interior on-beat
/// note a sixteenth EARLY so it anticipates (leans into) its beat, raising the riff's syncopation. The
/// move keeps both bar downbeats (beats 0 and 4 — the form's anchors) and the final resolution note
/// untouched, stays a lawful monophonic referent, and only commits a push that genuinely increases the
/// syncopation score. Returns the revised events + a plain reason, or `None` if the riff has no square
/// interior beat worth pushing (e.g. it is already off the grid). Rhythm only — pitches/durations and
/// so the harmonic camp are unchanged; the band re-negotiates its groove to the pushed riff.
pub fn syncopate_proposal(events: &[MaterialEvent]) -> Option<(Vec<MaterialEvent>, String)> {
    let onsets: Vec<f64> = events.iter().map(|e| e.onset).collect();
    let before = syncopation_score(&onsets);
    let last = events.len().saturating_sub(1);
    let mut best: Option<(usize, f64)> = None;
    let mut best_gain = 0.0_f64;
    for i in 1..last {
        let o = events[i].onset;
        // A SQUARE interior beat: on an integer beat, but NOT a bar downbeat (keep bars 1 & 2 anchored).
        if o.fract().abs() > 1e-9 || o.rem_euclid(4.0).abs() < 1e-9 {
            continue;
        }
        let pushed = o - 0.25;
        // Stay strictly after the previous onset (lawful monophony) and non-negative.
        if pushed <= events[i - 1].onset + 1e-9 || pushed < 0.0 {
            continue;
        }
        let mut trial = onsets.clone();
        trial[i] = pushed;
        let gain = syncopation_score(&trial) - before;
        if gain > best_gain + 1e-12 {
            best_gain = gain;
            best = Some((i, pushed));
        }
    }
    let (i, pushed) = best?;
    let beat = events[i].onset;
    let mut ev = events.to_vec();
    ev[i].onset = pushed;
    // Lengthen the note to its old landing so the line stays legato across the push (no new gap).
    ev[i].dur = (ev[i].dur + 0.25).min(1.2);
    Some((
        ev,
        format!(
            "the riff leans square on beat {beat:.0} -> push it a sixteenth early so it anticipates the beat (create the groove, don't just pick it)"
        ),
    ))
}

/// A revised riff must still be a lawful referent (3..=64 events, strictly increasing onsets, in-range
/// steps, finite positive durations). The band refuses a revision that would break the source.
pub fn revision_is_lawful(events: &[MaterialEvent]) -> bool {
    if events.len() < 3 || events.len() > 64 {
        return false;
    }
    let mut prev = -1.0_f64;
    for e in events {
        if !(e.onset >= 0.0 && e.onset.is_finite()) || e.onset <= prev {
            return false;
        }
        if !(e.dur > 0.0 && e.dur.is_finite()) {
            return false;
        }
        match e.step {
            Some(s) if (-48..=48).contains(&s) => {}
            _ => return false,
        }
        prev = e.onset;
    }
    true
}

// ======================================================================================
// Stage G — FREEZE the negotiated draft into a kernel-ready CompiledArgument
// ======================================================================================

/// Freeze the negotiated draft into the existing witnessed pipeline: TWO 16-beat (4-bar) Keys-carried
/// `Establish` episodes (the verse camp, then the brighter conjugate), 8 bars total. The keys COMP the
/// chords on every bar (the AM-radio chordal bed); the required Keys carrier holds only a soft
/// chord-tone fill in the comp's open bar, so the keys read as CHORDS, never a single-note lead. The
/// Score is LITERALLY Lead-empty (the fake 0.06 carrier is gone) — lawful because the carrier witness
/// excludes the kernel's "comp" notes. The negotiated RIFF drives the harmonic camp (the route) and
/// the bassist's groove; the funk kit is the drummer's pocket; the pad beds under it all.
pub fn freeze(draft: &RehearsalDraft) -> CompiledArgument {
    let riff = draft.riff();
    let camp = riff.camp.clone();

    let trace = SemanticTrace {
        total_beats: 32.0,
        events: vec![
            SemanticEvent {
                at_beat: 0.0,
                state: SemanticState::toned(Tone::Accent),
                kind: EventKind::ActChanged,
            },
            SemanticEvent {
                at_beat: 16.0,
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
    let mut song = SongMap::build(&trace, draft.seed, Some(CompositionGrammar::HookArc));
    let mut contract = song.plan.contract.clone();
    contract.phrase_bars = 1;
    song.plan = gibson::audio::human_music::plan::CompositionPlan::build_with_contract_for_beats(
        &song.timeline,
        32.0,
        contract,
    );
    // Keys CARRY (the riff), the pad holds the chords, the bass is the foundation, the drummer keeps
    // the pocket. Lead is left Support here but compile SILENCES it for the Keys-carried phrases, so
    // the Score is literally Lead-empty (argument.rs: a non-Lead carrier sets that phrase's lead=Silent).
    for p in song.plan.arrangement.phrases.iter_mut() {
        p.lead = ArrangementRole::Support;
        p.keys = ArrangementRole::Support;
        p.bass = ArrangementRole::Foundation;
        p.drums = ArrangementRole::Support;
        p.pad = ArrangementRole::Support;
    }

    // Episode carriers: the keys COMP the chords (comp_keys below), so the keys read as CHORDS, not a
    // single-note line trying to lead (the owner's verdict on the first cut). The required Keys carrier
    // is therefore a gentle in-harmony fill in the ONE bar the comp always leaves open (local bar 1,
    // beats 4–8) — chord tones of that bar's chord, part of the chordal texture, never a topline. The
    // negotiated RIFF is not sounded as a melody here; it shaped the harmonic camp (the route) and the
    // bassist's groove. The lead "key" comes later. Episode 0's open bar sits on the verse's 2nd chord,
    // episode 1's on the chorus's 2nd chord, so the fill is always consonant with the bar it lands in.
    let verse = MusicalReferent {
        id: ReferentId(0),
        pitch_basis: PitchBasis::Semitones,
        events: keys_carrier(camp.verse[1]),
    };
    let chorus = MusicalReferent {
        id: ReferentId(1),
        pitch_basis: PitchBasis::Semitones,
        events: keys_carrier(camp.chorus[1]),
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
        referents: vec![verse, chorus],
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
        .expect("rehearsal-frozen backing compiles");

    // Route: the negotiated camp. Episode 0 = verse vamp (period-2, a true repeat), episode 1 = the
    // brighter chorus conjugate with the lone E7 turnaround on the last bar (the one dominant moment).
    // loop_bars = route.len() keeps pop_route_law vacuous (documented loophole; a real section witness
    // is the deferred G6 work). The WITHIN-section period-2 repetition is genuine, not vacuous.
    let route = vec![
        camp.verse[0],
        camp.verse[1],
        camp.verse[0],
        camp.verse[1],
        camp.chorus[0],
        camp.chorus[1],
        camp.chorus[0],
        V_DOM7, // the cadence turnaround (E7), the only dominant in the route
    ];
    compiled.route = route;
    compiled.domain = SourceDomain::SmoothPop(PopSourceGrammar {
        loop_bars: 8, // == route.len(): documented loophole, G6 (pop_section_law) deferred
    });

    // Bass: the negotiated verse response over episode 0, the driven chorus response over episode 1 —
    // and the emergent TRANSITION the band found: the LAST verse bar walks up (R-3-5-8 + a push) into
    // the chorus downbeat, so the A loop is heard LEADING somewhere instead of merely repeating.
    let mut bass_cells: Vec<Vec<(f64, BassDegree, f32, f32)>> = Vec::with_capacity(8);
    bass_cells.extend(draft.bass_verse.cells.iter().cloned());
    if let Some(last_verse_bar) = bass_cells.len().checked_sub(1) {
        bass_cells[last_verse_bar] = transition_walk_cell();
    }
    bass_cells.extend(draft.bass_chorus.cells.iter().cloned());
    compiled.groove = Some(GrooveLaw { bass_cells });

    // KEYS = CHORDS: the engine comp voices rootless chord stabs on every episode (comp_keys +
    // comp_all_sections), so the keys hold the harmonic carrier throughout instead of a single-note
    // line — the AM-radio chordal bed the owner accepted at R2. Literal zero Lead is preserved: the
    // comp notes are kernel-stamped "comp" and the carrier-content witness now excludes them
    // (argument_witness::in_window), so a Keys-carried step may carry real polyphony without the fake
    // Lead scaffold. The funk kit reads the frozen bass; the pad bed runs under every episode.
    compiled.pop_funk_kit = true;
    compiled.comp_keys = true;
    compiled.comp_all_sections = true;
    compiled.pad_all_sections = true;
    // Make the band's HOOK audible: the keys comp stabs its chords on the RIFF's own rhythm (the
    // guitarist's figure heard as chordal hits, not sung) instead of the fixed comp pattern. The riff's
    // onsets drive the comp across each 16-beat episode (stated twice); the bass still owns the root.
    compiled.comp_onsets = Some(riff.events.iter().map(|e| e.onset).collect());
    compiled
}

/// The emergent A→B transition: the last verse bar walks up the chord (root → 3rd → 5th → octave) and
/// pushes the "and of 4", so the loop is heard LEADING into the chorus rather than just repeating.
/// Degrees resolve against the bar's own chord; the climb builds momentum into the section change.
fn transition_walk_cell() -> Vec<(f64, BassDegree, f32, f32)> {
    vec![
        (0.0, BassDegree::Root, 0.5, 0.78),
        (1.0, BassDegree::Third, 0.4, 0.60),
        (2.0, BassDegree::Fifth, 0.4, 0.64),
        (3.0, BassDegree::Octave, 0.4, 0.68),
        (3.5, BassDegree::Fifth, 0.4, 0.72), // the push into the chorus downbeat
    ]
}

/// The required Keys carrier as a gentle chord-tone fill in the comp's open bar (local bar 1, beats
/// 4–8). It arpeggiates the sounding chord's tones low-to-high at a soft velocity, so it reads as part
/// of the chordal texture rather than a melodic lead — and, being the only "argument"-provenance Keys
/// content, it is what the carrier witness checks (the comp chords are excluded). Guarantees a lawful
/// referent: four strictly-increasing onsets, in-range chord-tone steps, finite positive durations.
fn keys_carrier(chord: (i32, Quality)) -> Vec<MaterialEvent> {
    let (root, quality) = chord;
    // Up to four chord tones (root, 3rd, 5th, 7th) as semitones above the A tonic, voiced in a soft
    // mid-register arpeggio across the comp's open bar.
    let tones: Vec<i32> = quality
        .intervals()
        .iter()
        .take(4)
        .map(|&iv| (root + iv).clamp(-48, 48))
        .collect();
    let onsets = [4.5_f64, 5.5, 6.5, 7.5];
    tones
        .iter()
        .enumerate()
        .map(|(i, &step)| {
            let onset = onsets[i];
            MaterialEvent {
                onset: onset + pocket_offset(onset),
                dur: 0.8,
                accent: 0.38,
                step: Some(step),
            }
        })
        .collect()
}

/// A short honest manifest line describing a rehearsed+frozen song (for the example header).
#[allow(dead_code)]
pub fn manifest(draft: &RehearsalDraft) -> String {
    let riff = draft.riff();
    format!(
        "seed {} | riff: {} | revisions: {} (accepted {}) | camp: {}",
        draft.seed,
        riff.label,
        draft.log.len(),
        draft.log.iter().filter(|r| r.accepted).count(),
        riff.camp.label,
    )
}
