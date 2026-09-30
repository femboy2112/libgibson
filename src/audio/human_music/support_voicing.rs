//! Round XVI's bounded source-level pad voicing choice.
//!
//! The chart, note count, pitch-class multiset and upper-layer obligation stay fixed. Only
//! octave placement is searched, before the pad is returned to the score. An actual high
//! seventh excursion asks for this search; identity alone neither asks for nor certifies it.
//! Every kept alternative retains the old identity result, improves adjacent ordered motion,
//! and introduces no new sustained band cluster. Brief new contacts with the pad's own release
//! tails remain explicit tradeoffs on the receipt. This is a listening hypothesis.

use super::comp::realize_pad_on;
use super::expression::{observe, ConnectiveViability, ExpressionEvent};
use super::identity::{keeps_identity, IdentityDiagnostics};
use super::performance::PerformancePlan;
use super::score::{Note, PitchFunction, Role, Score};
use super::sonority::{audible_end, Clash};
use super::theory::{pitch_class, Midi};
use super::voicing::{RolePath, VoiceRange};
use super::voicing_diagnostics::{IntervalContact, VoicingSurfaceDiagnostics, VoicingSurfaceRow};
use super::world::MusicWorld;
use std::fmt::Write;

/// A new actual contact between different pad attacks across a harmony boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct VoicingTailContact {
    pub from_pitch: Midi,
    pub to_pitch: Midi,
    pub from_onset: f64,
    pub to_onset: f64,
    pub start_beat: f64,
    pub end_beat: f64,
    pub seconds: f64,
}

/// A new root/seventh contact admitted only because the already-performed bass reaches the
/// chart root by semitone and the unchanged Round XV physical ruler accepts that performance.
/// This records a listening hypothesis; it does not relabel or change either bass note.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvingBassContact {
    pub contact: IntervalContact,
    pub source_function: Option<PitchFunction>,
    pub source_duration_beats: f32,
    pub source_velocity: f32,
    pub target_pitch: Midi,
    pub target_beat: f64,
    pub target_function: Option<PitchFunction>,
    pub target_velocity: f32,
    pub target_latency_seconds: f64,
    pub source_audible_seconds: f64,
    pub physical_verdict: ConnectiveViability,
    /// Whether the original uniform contact guard rejected this actual contact.
    pub legacy_guard_rejected: bool,
}

/// One selected source voicing and its decomposed before/after evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct VoicingPathDecision {
    pub context: usize,
    pub start_beat: f64,
    pub before: Vec<Midi>,
    pub after: Vec<Midi>,
    pub before_surface: VoicingSurfaceRow,
    pub after_surface: VoicingSurfaceRow,
    pub added_tail_contacts: Vec<VoicingTailContact>,
    pub resolving_bass_contacts: Vec<ResolvingBassContact>,
    /// All finite same-PC octave placements that met the structural spacing constraints.
    pub candidates_considered: usize,
    pub reason: &'static str,
}

impl VoicingPathDecision {
    /// Explain the changed path and finite release-tail tradeoff without a quality scalar.
    pub fn report(&self) -> String {
        let mut out = format!(
            "context={} beat={:.3} {:?}->{:?} candidates={} reason={}\n  incoming {:?}->{:?}; outgoing {:?}->{:?}; top {:?}/{:?}->{:?}/{:?}; root {:?}->{:?}; guides {:?}->{:?}; span {}->{}; top_life_s {:.6}->{:.6}; top_exposure_proxy {:.6}->{:.6}\n",
            self.context, self.start_beat, self.before, self.after, self.candidates_considered, self.reason,
            self.before_surface.incoming_motion, self.after_surface.incoming_motion,
            self.before_surface.outgoing_motion, self.after_surface.outgoing_motion,
            self.before_surface.top_incoming_semitones, self.before_surface.top_outgoing_semitones,
            self.after_surface.top_incoming_semitones, self.after_surface.top_outgoing_semitones,
            self.before_surface.root_pitches, self.after_surface.root_pitches,
            self.before_surface.guide_pitches, self.after_surface.guide_pitches,
            self.before_surface.span, self.after_surface.span,
            self.before_surface.top_audible_seconds, self.after_surface.top_audible_seconds,
            self.before_surface.top_weighted_exposure, self.after_surface.top_weighted_exposure,
        );
        for c in &self.added_tail_contacts {
            let _ = writeln!(
                out,
                "  NEW pad tail contact {}@{:.3}->{}@{:.3} overlap={:.6}..{:.6} beats ({:.6}s)",
                c.from_pitch,
                c.from_onset,
                c.to_pitch,
                c.to_onset,
                c.start_beat,
                c.end_beat,
                c.seconds
            );
        }
        for c in &self.resolving_bass_contacts {
            let _ = writeln!(out, "  NEW resolving bass contact pad={}@{:.3} bass={}@{:.3} function={:?} dur_beats={} velocity={} target={}@{:.3} target_function={:?} target_velocity={} latency_s={:.6} audible_s={:.6} overlap_s={:.6} frozen_R15={:?} legacy_guard_rejected={} provisional_listening_hypothesis=true",c.contact.pad_pitch,c.contact.pad_onset_beat,c.contact.other_pitch,c.contact.other_onset_beat,c.source_function,c.source_duration_beats,c.source_velocity,c.target_pitch,c.target_beat,c.target_function,c.target_velocity,c.target_latency_seconds,c.source_audible_seconds,c.contact.overlap_seconds,c.physical_verdict,c.legacy_guard_rejected);
        }
        out
    }
}

fn measure(
    perf: &PerformancePlan,
    world: &MusicWorld,
    band: &[Note],
    pad: &[Note],
    continuity: Option<&[super::voice::VoiceContinuation]>,
) -> (VoicingSurfaceDiagnostics, IdentityDiagnostics) {
    // Source-owned candidate instrument, never a mutation of the finished composition.
    let mut trial = Score::new(world.tempo_bpm, 4.0, perf.total_beats);
    if let Some(links) = continuity {
        trial.mono_voice = true;
        trial.voice_continuity = links.to_vec();
    }
    trial.notes.extend_from_slice(band);
    trial.notes.extend_from_slice(pad);
    (
        VoicingSurfaceDiagnostics::measure(perf, &trial, world),
        IdentityDiagnostics::measure_score(&trial, &perf.contexts, world),
    )
}

fn tail_contacts(pad: &[Note], world: &MusicWorld) -> Vec<VoicingTailContact> {
    let spb = 60.0 / f64::from(world.tempo_bpm.max(1.0));
    let end = |n: &Note| {
        audible_end(
            n.start_beat,
            f64::from(n.dur_beats),
            &world.pad,
            world.tempo_bpm,
        )
    };
    let mut out = Vec::new();
    for p in pad {
        for q in pad {
            if q.start_beat <= p.start_beat + 1e-6 || Clash::of(p.pitch, q.pitch).is_none() {
                continue;
            }
            let finish = end(p).min(end(q));
            if finish > q.start_beat + 1e-6 {
                out.push(VoicingTailContact {
                    from_pitch: p.pitch,
                    to_pitch: q.pitch,
                    from_onset: p.start_beat,
                    to_onset: q.start_beat,
                    start_beat: q.start_beat,
                    end_beat: finish,
                    seconds: (finish - q.start_beat) * spb,
                });
            }
        }
    }
    out
}

/// All same-PC octave placements in the existing Round XIV pad span. The original index
/// supplies voice ownership for octave-edit counts; candidates themselves are ordered anew.
fn alternatives(before: &[Midi]) -> Vec<(usize, i32, Vec<Midi>)> {
    let low = VoiceRange::pad(0.0).low;
    let high = VoiceRange::pad_upper(0.0).high;
    let upper = VoiceRange::pad_upper(0.0).min_top.unwrap_or(high);
    let needs_upper = before.iter().any(|&p| p >= upper);
    let mut paths = vec![(0, 0, Vec::new())];
    for &p in before {
        let mut next = Vec::new();
        for q in (low..=high).filter(|&q| pitch_class(q) == pitch_class(p)) {
            for (edits, distance, rest) in &paths {
                if rest.contains(&q) || rest.iter().any(|&r| Clash::of(q, r).is_some()) {
                    continue;
                }
                let mut v = rest.clone();
                v.push(q);
                next.push((edits + usize::from(q != p), distance + (q - p).abs(), v));
            }
        }
        paths = next;
    }
    paths.retain(|(_, _, v)| !needs_upper || v.iter().any(|&p| p >= upper));
    for (_, _, v) in &mut paths {
        v.sort_unstable();
    }
    paths.sort();
    paths.dedup_by(|a, b| a.2 == b.2);
    paths
}

fn adjacent_motion(r: &VoicingSurfaceRow) -> i32 {
    r.incoming_motion
        .iter()
        .chain(&r.outgoing_motion)
        .map(|d| d.abs())
        .sum()
}

/// Inspect the actual next bass attack, never a convenient later pitch or a new label.
fn resolving_bass_contact(
    perf: &PerformancePlan,
    world: &MusicWorld,
    band: &[Note],
    pad: &[Note],
    contact: &IntervalContact,
) -> Option<ResolvingBassContact> {
    if contact.other_role != Role::Bass || contact.other_is_sfx {
        return None;
    }
    let mut bass: Vec<_> = band.iter().filter(|n| n.role == Role::Bass).collect();
    bass.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    let i = bass.iter().position(|n| {
        n.pitch == contact.other_pitch && (n.start_beat - contact.other_onset_beat).abs() < 1e-6
    })?;
    let source = bass[i];
    let target = *bass.get(i + 1)?;
    if target.start_beat <= source.start_beat + 1e-6
        || bass
            .get(i + 2)
            .is_some_and(|n| (n.start_beat - target.start_beat).abs() < 1e-6)
        || (source.pitch - target.pitch).abs() != 1
        || target.velocity < source.velocity
    {
        return None;
    }
    let ctx = perf.context_at(target.start_beat)?;
    if pitch_class(target.pitch) != ctx.chord.root_pc.rem_euclid(12)
        || pitch_class(contact.pad_pitch) != ctx.chord.root_pc.rem_euclid(12)
    {
        return None;
    }
    let evidence = observe(
        perf,
        world,
        &ExpressionEvent {
            note: *source,
            structural: true,
        },
        i.checked_sub(1).map(|j| bass[j]),
        Some(target),
        pad,
    );
    if !evidence.pitch_valid || evidence.verdict != ConnectiveViability::AsWritten {
        return None;
    }
    Some(ResolvingBassContact {
        contact: contact.clone(),
        source_function: source.function,
        source_duration_beats: source.dur_beats,
        source_velocity: source.velocity,
        target_pitch: target.pitch,
        target_beat: target.start_beat,
        target_function: target.function,
        target_velocity: target.velocity,
        target_latency_seconds: evidence.target_latency_secs,
        source_audible_seconds: evidence.audible_duration_secs,
        physical_verdict: evidence.verdict,
        legacy_guard_rejected: contact.overlap_seconds > 0.3,
    })
}

/// Compare all contacts including release tails. Only explicit resolving-bass evidence can
/// exempt a new contact from the original aggregate/held-contact guard; the frozen physical
/// ruler supplies its exposure limit without changing any threshold here.
fn check_band_contacts(
    perf: &PerformancePlan,
    world: &MusicWorld,
    band: &[Note],
    pad: &[Note],
    before: &VoicingSurfaceDiagnostics,
    after: &VoicingSurfaceDiagnostics,
) -> Option<Vec<ResolvingBassContact>> {
    let contacts = |r: &VoicingSurfaceRow| {
        r.contacts
            .iter()
            .filter(|c| c.other_role != Role::Pad && matches!(c.semitones, 1 | 13))
            .map(|c| c.overlap_seconds)
            .sum::<f64>()
    };
    let mut permissions = Vec::new();
    for r in &after.rows {
        let old = before.rows.iter().find(|p| p.context == r.context);
        let mut permitted_seconds = 0.0;
        for c in r
            .contacts
            .iter()
            .filter(|c| c.other_role != Role::Pad && matches!(c.semitones, 1 | 13))
        {
            let existed = old.is_some_and(|p| {
                p.contacts.iter().any(|o| {
                    o.pad_pitch == c.pad_pitch
                        && o.other_pitch == c.other_pitch
                        && o.other_role == c.other_role
                        && o.other_is_sfx == c.other_is_sfx
                        && o.pad_onset_beat == c.pad_onset_beat
                        && o.other_onset_beat == c.other_onset_beat
                        && o.overlap_seconds + 1e-9 >= c.overlap_seconds
                })
            });
            if existed {
                continue;
            }
            if let Some(evidence) = resolving_bass_contact(perf, world, band, pad, c) {
                permitted_seconds += c.overlap_seconds;
                permissions.push(evidence);
            } else if c.overlap_seconds > 0.3 {
                return None;
            }
        }
        if contacts(r) - permitted_seconds > old.map_or(0.0, contacts) + 1e-9 {
            return None;
        }
    }
    Some(permissions)
}

/// Select source voicings only. The caller emits the returned path through `realize_pad_on`.
pub(super) fn select(
    perf: &PerformancePlan,
    world: &MusicWorld,
    band: &[Note],
    path: RolePath,
    initial_pad: &[Note],
) -> (RolePath, Vec<VoicingPathDecision>) {
    select_impl(perf, world, band, path, initial_pad, None)
}

pub(super) fn select_with_continuity(
    perf: &PerformancePlan,
    world: &MusicWorld,
    band: &[Note],
    path: RolePath,
    initial_pad: &[Note],
    links: &[super::voice::VoiceContinuation],
) -> (RolePath, Vec<VoicingPathDecision>) {
    select_impl(perf, world, band, path, initial_pad, Some(links))
}

fn select_impl(
    perf: &PerformancePlan,
    world: &MusicWorld,
    band: &[Note],
    mut path: RolePath,
    initial_pad: &[Note],
    continuity: Option<&[super::voice::VoiceContinuation]>,
) -> (RolePath, Vec<VoicingPathDecision>) {
    let (mut surfaces, mut identity) = measure(perf, world, band, initial_pad, continuity);
    let mut tails = tail_contacts(initial_pad, world);
    let requests: Vec<_> = surfaces
        .rows
        .iter()
        .filter(|r| r.high_seventh_excursion)
        .map(|r| r.context)
        .collect();
    let mut decisions = Vec::new();
    for ci in requests {
        let Ok(t) = path.context_ix.binary_search(&ci) else {
            continue;
        };
        let Some(before_surface) = surfaces.rows.iter().find(|r| r.context == ci).cloned() else {
            continue;
        };
        if !before_surface.high_seventh_excursion {
            continue;
        }
        let before = path.path.voicings[t].voices.clone();
        let mut candidates = alternatives(&before);
        let considered = candidates.len();
        // Structural octave change first, then smaller motion through the adjacent path.
        candidates.sort_by_key(|(edits, distance, v)| {
            let movement = super::theory::voice_motion(&before_surface.previous_pad, v)
                + super::theory::voice_motion(v, &before_surface.next_pad);
            (*edits, *distance, movement, v.clone())
        });
        for (_, _, after) in candidates {
            if after == before {
                continue;
            }
            let candidate_motion =
                super::theory::voice_motion(&before_surface.previous_pad, &after)
                    + super::theory::voice_motion(&after, &before_surface.next_pad);
            if candidate_motion >= adjacent_motion(&before_surface) {
                continue;
            }
            let mut trial = path.clone();
            trial.path.voicings[t].voices = after.clone();
            let pad = realize_pad_on(perf, world, &trial, None);
            let (next_surfaces, next_identity) = measure(perf, world, band, &pad, continuity);
            let Some(after_surface) = next_surfaces.rows.iter().find(|r| r.context == ci).cloned()
            else {
                continue;
            };
            if after_surface.high_seventh_excursion
                || next_surfaces.rows.iter().any(|r| {
                    r.high_seventh_excursion
                        && !surfaces
                            .rows
                            .iter()
                            .any(|old| old.context == r.context && old.high_seventh_excursion)
                })
                || after_surface.incoming_motion.is_empty()
                || after_surface.outgoing_motion.is_empty()
                || adjacent_motion(&after_surface) >= adjacent_motion(&before_surface)
                || !keeps_identity(&identity, &next_identity, 0.0, perf.total_beats + 64.0)
            {
                continue;
            }
            let Some(resolving_bass_contacts) =
                check_band_contacts(perf, world, band, &pad, &surfaces, &next_surfaces)
            else {
                continue;
            };
            let next_tails = tail_contacts(&pad, world);
            let added_tail_contacts: Vec<_> = next_tails
                .iter()
                .filter(|c| !tails.contains(c))
                .cloned()
                .collect();
            // Conservative acoustic boundary, independent of beat rate: the alternative may
            // trade a sustained high excursion for a brief release contact, not another hold.
            if added_tail_contacts.iter().any(|c| c.seconds > 0.3) {
                continue;
            }
            decisions.push(VoicingPathDecision { context:ci,start_beat:perf.contexts[ci].start_beat,before,after,before_surface,after_surface,added_tail_contacts,resolving_bass_contacts,candidates_considered:considered,reason:"same-PC octave placement removes a sustained top-seventh excursion and reduces adjacent voice motion; identity retained; new resolving-bass contacts separately justified by frozen physical ruler" });
            path = trial;
            surfaces = next_surfaces;
            identity = next_identity;
            tails = next_tails;
            break;
        }
    }
    (path, decisions)
}

#[cfg(test)]
mod pocket_support_probe {
    use super::*;
    use crate::audio::human_music::{
        composer::Composer, functor::perform_phrased, performance::PerformanceOptions,
        semantic::deflected_lift_trace, song::SongMap,
    };

    /// Diagnostic source trials only. This neither selects nor emits a production repair.
    #[test]
    #[ignore = "bounded Round XVII support enumeration writes explicit diagnostic receipts"]
    fn pocket_low_root_octave_only_no_go() {
        let world = MusicWorld::swiss_signal();
        let song = SongMap::compose(
            &deflected_lift_trace(120.0),
            2112,
            None,
            Composer::StablePropulsion,
        );
        let c = perform_phrased(&song, &world, PerformanceOptions::default());
        let band: Vec<_> = c
            .score
            .notes
            .iter()
            .filter(|n| n.role != Role::Pad)
            .copied()
            .collect();
        let original_pad: Vec<_> = c.score.role_notes(Role::Pad).copied().collect();
        let (surfaces, identity) = measure(&c.perf, &world, &band, &original_pad, None);
        let original_tails = tail_contacts(&original_pad, &world);
        // Recover the frozen source path and prove its source emission matches the control.
        let mut source = super::super::voicing::pad_path(&c.perf, world.voicing_spread);
        for r in &surfaces.rows {
            let t = source.context_ix.binary_search(&r.context).unwrap();
            source.path.voicings[t].voices = r.pad.clone();
        }
        let signature = |notes: &[Note]| {
            let mut rows: Vec<_> = notes
                .iter()
                .map(|n| {
                    (
                        n.start_beat.to_bits(),
                        n.pitch,
                        n.dur_beats.to_bits(),
                        n.velocity.to_bits(),
                    )
                })
                .collect();
            rows.sort_unstable();
            rows
        };
        assert_eq!(
            signature(&realize_pad_on(&c.perf, &world, &source, None)),
            signature(&original_pad)
        );
        let mut table = String::from("beat\tcandidate\tedits\toctave_distance\tremoves_C4\tno_new_high_seventh\tidentity\tband_contacts\ttails\tmotion\told_motion\tstrict_motion_improvement\ttop\tresult\n");
        let mut detail = String::from("Source-owned trial paths, not finished-score repairs. Same implementation provenance as production guards; no independent listening evidence.\n");
        let mut summary = String::from(
            "beat all no_C4 no_high_seventh identity band_contacts tails full_motion_gate\n",
        );
        for (site, beat) in [28.0, 36.0, 44.0].into_iter().enumerate() {
            let old = surfaces.rows.iter().find(|r| r.beat == beat).unwrap();
            let t = source.context_ix.binary_search(&old.context).unwrap();
            let mut counts = [0usize; 7];
            for (edits, distance, after) in alternatives(&old.pad) {
                counts[0] += 1;
                if after.contains(&60) {
                    writeln!(table,"{beat}\t{after:?}\t{edits}\t{distance}\tfalse\tNA\tNA\tNA\tNA\tNA\t{}\tNA\t{}\tretains_human_implicated_C4",adjacent_motion(old),after.last().unwrap()).unwrap();
                    continue;
                }
                counts[1] += 1;
                let mut trial = source.clone();
                trial.path.voicings[t].voices = after.clone();
                let pad = realize_pad_on(&c.perf, &world, &trial, None);
                let (next, next_identity) = measure(&c.perf, &world, &band, &pad, None);
                let row = next.rows.iter().find(|r| r.context == old.context).unwrap();
                let no_high = !next.rows.iter().any(|r| {
                    r.high_seventh_excursion
                        && !surfaces
                            .rows
                            .iter()
                            .any(|p| p.context == r.context && p.high_seventh_excursion)
                });
                let ident =
                    keeps_identity(&identity, &next_identity, 0.0, c.perf.total_beats + 64.0);
                let contacts = check_band_contacts(&c.perf, &world, &band, &pad, &surfaces, &next);
                let tails = tail_contacts(&pad, &world)
                    .iter()
                    .all(|x| original_tails.contains(x) || x.seconds <= 0.3);
                let motion = adjacent_motion(row);
                let improved = motion < adjacent_motion(old);
                let band_ok = contacts.is_some();
                let flags = [no_high, ident, band_ok, tails, improved];
                let mut prior = true;
                for (i, flag) in flags.into_iter().enumerate() {
                    prior &= flag;
                    counts[i + 2] += usize::from(prior);
                }
                let result = if !no_high {
                    "high_seventh_excursion"
                } else if !ident {
                    "identity"
                } else if !band_ok {
                    "unchanged_band_contact_guard"
                } else if !tails {
                    "new_tail_over_300ms"
                } else if !improved {
                    "unchanged_motion_guard"
                } else {
                    "admitted"
                };
                writeln!(table,"{beat}\t{after:?}\t{edits}\t{distance}\ttrue\t{no_high}\t{ident}\t{band_ok}\t{tails}\t{motion}\t{}\t{improved}\t{}\t{result}",adjacent_motion(old),after.last().unwrap()).unwrap();
                writeln!(detail,"beat={beat} candidate={after:?} result={result} root={:?} incoming={:?} outgoing={:?} resolving_permissions={:?}\n  semitone_contacts={:?}",row.root_pitches,row.incoming_motion,row.outgoing_motion,contacts.as_ref().map(Vec::len),row.contacts.iter().filter(|x|x.other_role!=Role::Pad&&matches!(x.semitones,1|13)).collect::<Vec<_>>()).unwrap();
            }
            let admitted_band = if site == 0 { 8 } else { 0 };
            assert_eq!(counts, [58, 32, 24, 24, admitted_band, admitted_band, 0]);
            writeln!(
                summary,
                "{beat} {} {} {} {} {} {} {}",
                counts[0], counts[1], counts[2], counts[3], counts[4], counts[5], counts[6]
            )
            .unwrap();
        }
        summary.push_str("Verified finite NO-GO: zero common C4-removing octave-only alternatives under unchanged source guards. No solver treatment implemented. Boundary: not a theorem about all voicings, instruments, omissions, or human acceptance.\n");
        summary.push_str("Human-observed: b028 mute_G5 retains complaint; mute_C4 removes it. Conjectured: low-root/register involvement. Not established: root/B2 collision versus timbre/masking interaction.\n");
        let out = std::env::var_os("HUMANMUSIC_SUPPORT_RECEIPT_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "target/humanmusic-r17/support-no-go".into());
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("candidates.tsv"), table).unwrap();
        std::fs::write(out.join("contacts.txt"), detail).unwrap();
        std::fs::write(out.join("summary.txt"), &summary).unwrap();
        println!("{summary}");
    }
}
