//! Explicit continuity between two source-owned events, shared by direct-voice observation and PCM.
//! Role is an instrument family, never sufficient evidence that two notes share a string.
use super::{
    ids::{ActionStamp, MaterialId},
    instrument::Patch,
    score::{Note, Role, Score},
    world::MusicWorld,
};

/// The default observation floor in dB below a direct voice's peak.
pub const AUDIBLE_FLOOR_DB: f64 = 30.0;
/// The historical masking floor used for coupled support release.
pub const MASKING_FLOOR_DB: f64 = 20.0;

/// The patch for a pitched role. Roles select instruments, never logical voice identity.
pub fn patch(world: &MusicWorld, role: Role) -> &Patch {
    match role {
        Role::Pad => &world.pad,
        Role::Keys => &world.keys,
        Role::Bass => &world.bass,
        Role::Lead => &world.lead,
    }
}

/// Envelope approximation of a direct audible endpoint, in beats. This is not a sample
/// simulation and excludes downstream shared reverb. Historical arithmetic is preserved.
pub fn audible_end_at(start: f64, dur: f64, patch: &Patch, tempo_bpm: f32, floor_db: f64) -> f64 {
    let spb = 60.0 / tempo_bpm.max(1.0) as f64;
    let (a, d, s, r) = patch.adsr;
    let (a, d, s, r) = (a as f64, d as f64, s as f64, r as f64);
    let nominal = dur * spb;
    let frac = floor_db / 40.0;
    let secs = if s <= 0.02 {
        (a + frac * d).min(nominal + frac * r)
    } else {
        nominal + release_tail_secs(patch, floor_db)
    };
    start + secs / spb
}

/// [`audible_end_at`] at the default observation floor.
pub fn audible_end(start: f64, dur: f64, patch: &Patch, tempo_bpm: f32) -> f64 {
    audible_end_at(start, dur, patch, tempo_bpm, AUDIBLE_FLOOR_DB)
}

/// Seconds from release at sustain level until the declared relative floor.
pub fn release_tail_secs(patch: &Patch, floor_db: f64) -> f64 {
    let (_, _, s, r) = patch.adsr;
    let (s, r) = (s as f64, r as f64);
    if s <= 0.02 {
        return 0.0;
    }
    r * ((floor_db + 20.0 * s.log10()) / 40.0).max(0.0)
}

/// Observation policy, distinguished from the source planner and actual render edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservedLifetimePolicy {
    /// Frozen mass/tension heuristic: the next attack of a role masks a released tail.
    LegacyRoleMasking,
    /// Only explicit source-owned continuation edges can shorten an envelope lifetime.
    ExplicitContinuity,
}

impl super::fingerprint::CanonicalFingerprint for ObservedLifetimePolicy {
    fn encode(&self, writer: &mut super::fingerprint::FingerprintWriter) {
        writer.tag("ObservedLifetimePolicy/v2");
        writer.tag(match self {
            Self::LegacyRoleMasking => "legacy-role-masking",
            Self::ExplicitContinuity => "explicit-continuity",
        });
    }
}

/// One immutable acoustic reconstruction shared by observers of the same notes and tempo.
/// Borrowing the notes prevents in-place edits from making a live reconstruction stale.
#[derive(Debug)]
pub struct HeardWindows<'a> {
    notes: &'a [Note],
    tempo_bpm: f32,
    policy: ObservedLifetimePolicy,
    windows: Vec<(f64, f64)>,
}

impl<'a> HeardWindows<'a> {
    /// Historical role masking, using one sorted onset index rather than a scan per note.
    pub fn historical(notes: &'a [Note], world: &MusicWorld, tempo_bpm: f32) -> Self {
        const EPS: f64 = 1e-6;
        let role_index = |role| match role {
            Role::Pad => 0,
            Role::Keys => 1,
            Role::Bass => 2,
            Role::Lead => 3,
        };
        let mut onsets: [Vec<f64>; 4] = Default::default();
        for note in notes {
            onsets[role_index(note.role)].push(note.start_beat);
        }
        for onsets in &mut onsets {
            onsets.sort_by(f64::total_cmp);
        }
        let windows = notes
            .iter()
            .map(|note| {
                let dur = f64::from(note.dur_beats);
                let written_end = note.start_beat + dur;
                let heard = audible_end(note.start_beat, dur, patch(world, note.role), tempo_bpm);
                let onsets = &onsets[role_index(note.role)];
                let k = onsets.partition_point(|&x| x < written_end - EPS);
                let mask = onsets[k..]
                    .iter()
                    .copied()
                    .find(|&x| x > note.start_beat + EPS);
                let end = mask.map_or(heard, |m| heard.min(m));
                (note.start_beat, end.max(note.start_beat))
            })
            .collect();
        Self {
            notes,
            tempo_bpm,
            policy: ObservedLifetimePolicy::LegacyRoleMasking,
            windows,
        }
    }

    pub fn explicit(
        notes: &'a [Note],
        world: &MusicWorld,
        tempo_bpm: f32,
        links: &[VoiceContinuation],
    ) -> Self {
        let windows = notes
            .iter()
            .map(|note| {
                (
                    note.start_beat,
                    effective_audible_end_at(
                        note,
                        patch(world, note.role),
                        tempo_bpm,
                        AUDIBLE_FLOOR_DB,
                        links,
                    ),
                )
            })
            .collect();
        Self {
            notes,
            tempo_bpm,
            policy: ObservedLifetimePolicy::ExplicitContinuity,
            windows,
        }
    }

    /// Use the declared observation policy, including modern scores with no continuation edges.
    /// Archived scores retain their explicit compatibility interpretation in Score.
    pub fn of_score(score: &'a Score, world: &MusicWorld) -> Self {
        if score.observed_lifetime_policy() == ObservedLifetimePolicy::LegacyRoleMasking {
            Self::historical(&score.notes, world, score.tempo_bpm)
        } else {
            Self::explicit(
                &score.notes,
                world,
                score.tempo_bpm,
                &score.voice_continuity,
            )
        }
    }

    pub fn notes(&self) -> &'a [Note] {
        self.notes
    }
    pub fn tempo_bpm(&self) -> f32 {
        self.tempo_bpm
    }
    pub fn policy(&self) -> ObservedLifetimePolicy {
        self.policy
    }
    pub fn windows(&self) -> &[(f64, f64)] {
        &self.windows
    }
    pub fn into_windows(self) -> Vec<(f64, f64)> {
        self.windows
    }
}

/// Direct-voice choke duration. Shared downstream reverb may continue after this bound.
pub const MONO_CHOKE_SECS: f64 = 0.015;

/// Source event identity stable under final gate clipping and arrangement stamps. Equal
/// duplicate identities remain ambiguous and cannot be assigned continuity by a source author.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceEventId {
    onset: u64,
    pitch: i32,
    role: Role,
    material: Option<MaterialId>,
    actions: ActionStamp,
    role_note: &'static str,
    motif: Option<u8>,
}
impl VoiceEventId {
    pub fn of(note: &Note) -> Self {
        Self {
            onset: note.start_beat.to_bits(),
            pitch: note.pitch,
            role: note.role,
            material: note.prov.material,
            actions: note.prov.actions,
            role_note: note.prov.role_note,
            motif: note.prov.motif_id,
        }
    }
    pub fn onset(self) -> f64 {
        f64::from_bits(self.onset)
    }
}

impl super::fingerprint::CanonicalFingerprint for VoiceEventId {
    fn encode(&self, writer: &mut super::fingerprint::FingerprintWriter) {
        let Self {
            onset,
            pitch,
            role,
            material,
            actions,
            role_note,
            motif,
        } = self;
        writer.tag("VoiceEventId/v2");
        writer.field("onset", &f64::from_bits(*onset));
        writer.field("pitch", pitch);
        writer.field("role", role);
        writer.field("material", material);
        writer.field("actions", actions);
        writer.field("role_note", role_note);
        writer.field("motif", motif);
    }
}

/// An explicit source-owned same-string continuation. No implicit role-wide choke is licensed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceContinuation {
    pub from: VoiceEventId,
    pub to: VoiceEventId,
}

impl super::fingerprint::CanonicalFingerprint for VoiceContinuation {
    fn encode(&self, writer: &mut super::fingerprint::FingerprintWriter) {
        let Self { from, to } = self;
        writer.tag("VoiceContinuation/v2");
        writer.field("from", from);
        writer.field("to", to);
    }
}
impl VoiceContinuation {
    /// The source planner must additionally establish that these events belong to one gesture.
    pub fn new(from: &Note, to: &Note) -> Option<Self> {
        (matches!(from.role, Role::Lead | Role::Bass)
            && from.role == to.role
            && from.start_beat.is_finite()
            && to.start_beat.is_finite()
            && to.start_beat > from.start_beat)
            .then(|| Self {
                from: VoiceEventId::of(from),
                to: VoiceEventId::of(to),
            })
    }
}

/// Direct audible endpoint under explicit continuity, in beats. Empty links reproduce the
/// historical release-tail ruler. Shared reverb is not included by either definition.
pub fn effective_audible_end(note: &Note, world: &MusicWorld, links: &[VoiceContinuation]) -> f64 {
    effective_audible_end_at(
        note,
        patch(world, note.role),
        world.tempo_bpm,
        AUDIBLE_FLOOR_DB,
        links,
    )
}

/// Floor- and tempo-aware form of the same law, for every scored acoustic consumer.
pub fn effective_audible_end_at(
    note: &Note,
    patch: &Patch,
    tempo_bpm: f32,
    floor_db: f64,
    links: &[VoiceContinuation],
) -> f64 {
    let ordinary = audible_end_at(
        note.start_beat,
        f64::from(note.dur_beats),
        patch,
        tempo_bpm,
        floor_db,
    );
    let id = VoiceEventId::of(note);
    let spb = 60.0 / f64::from(tempo_bpm.max(1.0));
    links
        .iter()
        .filter(|link| link.from == id)
        .map(|link| link.to.onset() + MONO_CHOKE_SECS / spb)
        .fold(ordinary, f64::min)
}

/// Refuse missing/ambiguous identities and competing successors instead of inventing a lane.
pub fn continuity_violations(notes: &[Note], links: &[VoiceContinuation]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, link) in links.iter().enumerate() {
        for (name, id) in [("from", link.from), ("to", link.to)] {
            let count = notes.iter().filter(|n| VoiceEventId::of(n) == id).count();
            if count != 1 {
                out.push(format!("continuity {i} {name}: {count} matching events"));
            }
        }
        if links[..i].iter().any(|old| old.from == link.from) {
            out.push(format!("continuity {i}: duplicate predecessor"));
        }
        if link.from.role != link.to.role
            || !matches!(link.from.role, Role::Lead | Role::Bass)
            || !link.from.onset().is_finite()
            || !link.to.onset().is_finite()
            || link.to.onset() <= link.from.onset()
        {
            out.push(format!("continuity {i}: invalid direction or role"));
        }
    }
    out
}
