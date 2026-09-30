//! Explicit continuity between two source-owned events, shared by direct-voice observation and PCM.
//! Role is an instrument family, never sufficient evidence that two notes share a string.
use super::{
    expression::patch,
    ids::{ActionStamp, MaterialId},
    instrument::Patch,
    score::{Note, Role},
    sonority::{audible_end_at, AUDIBLE_FLOOR_DB},
    world::MusicWorld,
};

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

/// An explicit source-owned same-string continuation. No implicit role-wide choke is licensed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceContinuation {
    pub from: VoiceEventId,
    pub to: VoiceEventId,
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
