//! Demo-local cinema: semantic events choose shots, recorded update boundaries
//! choose transitions, and painting only evaluates the resulting camera rig.
//! Every shot observes the same machine; it never changes encounter state.
use super::battle::{EncounterModel, Goal, NodeId, Outcome, Tactic};
use super::cyber::{position, VisualHistory};
use gibson::{raster3d::Camera, Vec3};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotKind {
    Establishing,
    Arrival,
    RouteContest,
    NodeCloseup,
    Trace,
    Isolation,
    Decoy,
    Evasion,
    DisplayAssault,
    FinalDuel,
    CrashWin,
    AcidWin,
    Stalemate,
    Aftermath,
}
impl ShotKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Establishing => "ESTABLISHING",
            Self::Arrival => "ARRIVAL",
            Self::RouteContest => "ROUTE CONTEST",
            Self::NodeCloseup => "VAULT APPROACH",
            Self::Trace => "TRACE CHASE",
            Self::Isolation => "ISOLATION",
            Self::Decoy => "MIRROR",
            Self::Evasion => "FALSE PATHS",
            Self::DisplayAssault => "DISPLAY ASSAULT",
            Self::FinalDuel => "FINAL DUEL",
            Self::CrashWin => "CONTAINMENT",
            Self::AcidWin => "POSSESSION",
            Self::Stalemate => "STANDING WAVE",
            Self::Aftermath => "AFTERMATH",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPose {
    pub position: Vec3,
    pub target: Vec3,
    pub fov: f32,
}
impl CameraPose {
    fn blend(self, other: Self, amount: f32) -> Self {
        Self {
            position: lerp(self.position, other.position, amount),
            target: lerp(self.target, other.target, amount),
            fov: self.fov + (other.fov - self.fov) * amount,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelPolicy {
    All,
    FocalAndRoute,
    Minimal,
    Scars,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotPlan {
    pub kind: ShotKind,
    pub camera: Camera,
    pub focal: Option<NodeId>,
    pub field_strength: f32,
    pub light_strength: f32,
    pub labels: LabelPolicy,
    /// Seconds since entry, including the recorded aftermath clock.
    pub phase: f32,
    pub transition: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Selection {
    kind: ShotKind,
    focal: Option<NodeId>,
}
impl Selection {
    fn new(kind: ShotKind, focal: Option<NodeId>) -> Self {
        Self { kind, focal }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct ShotHistory {
    selection: Selection,
    from: CameraPose,
    from_field: f32,
    from_light: f32,
    last_pose: CameraPose,
    entered_at: Duration,
    last_action: String,
    cooldowns: [u64; 5],
    action: Option<(Selection, Duration)>,
}
impl ShotHistory {
    pub fn new(world: &EncounterModel) -> Self {
        let action = if world.outcome.is_none() {
            action_shot(world)
                .filter(|_| world.cooldowns.iter().any(|value| *value > 0))
                .or_else(|| {
                    (world.trace_confidence >= 650 && world.planner.goal == Goal::EvadeTrace)
                        .then(|| Selection::new(ShotKind::Trace, Some(world.planner.location)))
                })
        } else {
            None
        };
        let selection = action.unwrap_or_else(|| select(world, Duration::ZERO));
        Self {
            selection,
            from: rig(world, selection, 0.0),
            from_field: lighting(selection.kind).0,
            from_light: lighting(selection.kind).1,
            last_pose: rig(world, selection, world.visual_time().as_secs_f32().min(2.0)),
            // Inspection hooks start on a settled composition. Ordinary quiet
            // starts on the same pose, so this does not skip its opening.
            entered_at: world.visual_time().saturating_sub(Duration::from_secs(2)),
            last_action: world.last_action.clone(),
            cooldowns: world.cooldowns,
            action: action.map(|shot| (shot, world.visual_time())),
        }
    }
    pub fn update(&mut self, world: &EncounterModel, aftermath: Duration) {
        let now = world.visual_time().saturating_add(aftermath);
        let new_action = self.last_action != world.last_action
            || world
                .cooldowns
                .iter()
                .zip(self.cooldowns)
                .any(|(a, b)| *a > b);
        if new_action {
            if let Some(shot) = action_shot(world) {
                self.action = Some((shot, now));
            }
            self.last_action.clone_from(&world.last_action);
        }
        self.cooldowns = world.cooldowns;
        let mut next = select(world, aftermath);
        let action = self
            .action
            .filter(|(_, at)| now.saturating_sub(*at) < Duration::from_millis(2400));
        if world.outcome.is_none() {
            if let Some((shot, _)) = action {
                next = shot;
            }
        }
        // A new tactical decision should not create a camera cut every reducer
        // tick. New defenses and resolution are allowed to interrupt a hold.
        let urgent = world.outcome.is_some() || (new_action && action.is_some());
        let defense = new_action && action_shot(world).is_some();
        if (next != self.selection || defense)
            && (urgent || now.saturating_sub(self.entered_at) >= Duration::from_millis(1800))
        {
            // The just-mutated world may already have moved an isolated node.
            // Preserve the previously presented rig rather than re-evaluating
            // its old focal point against the new topology at zero elapsed time.
            self.from = self.last_pose;
            let blend = smooth(now.saturating_sub(self.entered_at).as_secs_f32() / 1.25);
            let (field, light, _) = lighting(self.selection.kind);
            self.from_field += (field - self.from_field) * blend;
            self.from_light += (light - self.from_light) * blend;
            self.selection = next;
            self.entered_at = now;
        }
        self.last_pose = self.pose(world, now);
    }
    fn pose(&self, world: &EncounterModel, now: Duration) -> CameraPose {
        let elapsed = now.saturating_sub(self.entered_at).as_secs_f32();
        self.from
            .blend(rig(world, self.selection, elapsed), smooth(elapsed / 1.25))
    }
    pub fn kind(&self) -> ShotKind {
        self.selection.kind
    }
}

fn action_shot(world: &EncounterModel) -> Option<Selection> {
    use ShotKind::*;
    let action = &world.last_action;
    if action.contains("UNAVAILABLE") {
        return None;
    }
    if action.starts_with("TRACE /") {
        Some(Selection::new(Trace, Some(world.planner.location)))
    } else if action.starts_with("ISOLATE") || action.starts_with("HARD ISOLATE") {
        let target = if action.contains("AUTH") {
            NodeId::Auth
        } else {
            NodeId::Route
        };
        Some(Selection::new(Isolation, Some(target)))
    } else if action.starts_with("DECOY /") {
        Some(Selection::new(Decoy, Some(NodeId::Decoy)))
    } else if action.starts_with("KILL SESSION /") {
        Some(Selection::new(NodeCloseup, Some(world.planner.location)))
    } else {
        None
    }
}
fn select(world: &EncounterModel, aftermath: Duration) -> Selection {
    use ShotKind::*;
    if let Some(outcome) = world.outcome {
        return Selection::new(
            if aftermath >= Duration::from_millis(4500) {
                Aftermath
            } else {
                match outcome {
                    Outcome::Crash => CrashWin,
                    Outcome::Acid => AcidWin,
                    Outcome::Stalemate => Stalemate,
                }
            },
            Some(NodeId::Display),
        );
    }
    if world.takeover >= 500 {
        return Selection::new(FinalDuel, Some(NodeId::Display));
    }
    if world.graph.node(NodeId::Display).acid_fraction() > 0.45
        && !world.graph.node(NodeId::Display).isolated
    {
        return Selection::new(DisplayAssault, Some(NodeId::Display));
    }
    if world.graph.node(NodeId::Decoy).visible
        && (world.planner.target == NodeId::Decoy || world.planner.tactic == Tactic::AttackDecoy)
    {
        return Selection::new(Decoy, Some(NodeId::Decoy));
    }
    if world.planner.tactic == Tactic::SplitRoute || world.planner.tactic == Tactic::Feint {
        return Selection::new(Evasion, Some(world.planner.location));
    }
    // Stored trace confidence is a strategic state, not an endlessly repeated
    // command. Only an accepted TRACE action launches the chase camera.
    if world.planner.goal == Goal::EvadeTrace && world.trace_confidence >= 650 {
        return Selection::new(Evasion, Some(world.planner.location));
    }
    if world.graph.node(NodeId::Route).isolated || world.graph.node(NodeId::Auth).isolated {
        let id = if world.graph.node(NodeId::Auth).isolated {
            NodeId::Auth
        } else {
            NodeId::Route
        };
        // Once she has a valid bypass, follow her instead of staring at a cut.
        if world.planner.path.len() < 2 {
            return Selection::new(Isolation, Some(id));
        }
    }
    if [NodeId::Auth, NodeId::Files, NodeId::Shell]
        .into_iter()
        .any(|id| world.graph.node(id).acid_fraction() > 0.6)
    {
        let focal =
            if [NodeId::Auth, NodeId::Files, NodeId::Shell].contains(&world.planner.location) {
                world.planner.location
            } else {
                NodeId::Auth
            };
        return Selection::new(NodeCloseup, Some(focal));
    }
    if world.graph.node(NodeId::Route).acid_fraction() > 0.15 {
        return Selection::new(RouteContest, Some(NodeId::Route));
    }
    if world.graph.node(NodeId::Modem).acid_fraction() > 0.1 {
        return Selection::new(Arrival, Some(NodeId::Modem));
    }
    Selection::new(Establishing, None)
}
fn lerp(a: Vec3, b: Vec3, amount: f32) -> Vec3 {
    a.plus(b.minus(a).scale(amount))
}
fn smooth(value: f32) -> f32 {
    let v = value.clamp(0.0, 1.0);
    v * v * (3.0 - 2.0 * v)
}
fn rig(world: &EncounterModel, shot: Selection, phase: f32) -> CameraPose {
    use ShotKind::*;
    let focal = position(world, shot.focal.unwrap_or(NodeId::Route));
    let core = Vec3::new(0.0, 0.8, 1.0);
    let pose = |position, target, fov| CameraPose {
        position,
        target,
        fov,
    };
    match shot.kind {
        Establishing => pose(Vec3::new(10.0, 11.0, -18.0), core, 0.66),
        Arrival => pose(
            Vec3::new(4.0, 5.5, -12.0),
            focal.plus(Vec3::new(0.0, 0.6, 0.0)),
            0.73,
        ),
        RouteContest => pose(
            focal.plus(Vec3::new(5.2, 4.0, -8.2)),
            focal.plus(Vec3::new(0.0, 0.9, 0.0)),
            0.78,
        ),
        NodeCloseup => pose(
            focal.plus(Vec3::new(-4.2, 3.5, -6.8)),
            focal.plus(Vec3::new(0.0, 1.0, 0.0)),
            0.77,
        ),
        Trace => {
            let start = world
                .planner
                .path
                .last()
                .copied()
                .unwrap_or(world.planner.location);
            let end = world.planner.path.first().copied().unwrap_or(NodeId::Modem);
            let a = position(world, start);
            let b = position(world, end);
            let travel = smooth((phase / 3.0).min(0.75));
            let target = lerp(a, b, travel).plus(Vec3::new(0.0, 0.6, 0.0));
            pose(
                target.plus(Vec3::new(1.8, 2.4, 6.2)),
                lerp(target, b, 0.38),
                0.82,
            )
        }
        Isolation => pose(focal.plus(Vec3::new(7.0, 4.5, -5.5)), focal, 0.87),
        Decoy => {
            let files = position(world, NodeId::Files);
            pose(
                Vec3::new(-8.5, 5.0, -3.5),
                lerp(files, focal, 0.5).plus(Vec3::new(0.0, 0.7, 0.0)),
                0.8,
            )
        }
        Evasion => pose(Vec3::new(0.5, 13.0, -2.5), focal, 0.87),
        DisplayAssault => pose(
            focal.plus(Vec3::new(-1.2, 3.0, -7.2)),
            focal.plus(Vec3::new(0.0, 1.8, 0.0)),
            0.89,
        ),
        FinalDuel => pose(Vec3::new(7.0, 7.0, -8.0), Vec3::new(-0.3, 1.0, 1.7), 0.75),
        CrashWin => {
            let retreat = smooth(phase / 4.0);
            pose(
                lerp(
                    Vec3::new(-5.0, 5.0, -10.0),
                    Vec3::new(11.0, 13.0, -19.0),
                    retreat,
                ),
                core,
                0.7,
            )
        }
        AcidWin => pose(Vec3::new(0.0, 5.0, -15.5), Vec3::new(0.0, 1.3, 1.6), 0.76),
        Stalemate => pose(Vec3::new(0.0, 13.0, -13.0), core, 0.76),
        Aftermath => pose(Vec3::new(10.0, 12.0, -19.0), core, 0.69),
    }
}

/// Pure composition decision. The viewport changes framing, never shot history
/// or simulation. Narrow terminals favor one focal silhouette over an overview.
pub fn plan(world: &EncounterModel, history: &VisualHistory, width: u16, height: u16) -> ShotPlan {
    let shots = &history.shots;
    let now = world.visual_time().saturating_add(history.aftermath);
    let phase = now.saturating_sub(shots.entered_at).as_secs_f32();
    let mut pose = shots.pose(world, now);
    let narrow = width < 76;
    if narrow {
        if let Some(id) = shots.selection.focal {
            let focus = position(world, id).plus(Vec3::new(0.0, 0.8, 0.0));
            pose.target = lerp(pose.target, focus, 0.45);
            pose.position = lerp(pose.position, pose.target, 0.1);
        }
        pose.fov *= 1.15;
    }
    // More vertical space exposes architecture instead of making tiny objects.
    if height >= 36 && width >= 120 {
        pose.fov *= 0.94;
    }
    let (field, light, labels) = lighting(shots.selection.kind);
    let blend = smooth(phase / 1.25);
    let field_strength = shots.from_field + (field - shots.from_field) * blend;
    let light_strength = shots.from_light + (light - shots.from_light) * blend;
    ShotPlan {
        kind: shots.selection.kind,
        camera: Camera {
            position: pose.position,
            target: pose.target,
            fov_y: pose.fov,
            near: 0.15,
            far: 80.0,
            ..Camera::default()
        },
        focal: shots.selection.focal,
        field_strength,
        light_strength,
        labels,
        phase,
        transition: smooth(phase / 1.25),
    }
}

fn lighting(kind: ShotKind) -> (f32, f32, LabelPolicy) {
    use ShotKind::*;
    match kind {
        Establishing => (0.17, 0.52, LabelPolicy::All),
        Arrival => (0.28, 0.72, LabelPolicy::FocalAndRoute),
        RouteContest => (0.6, 1.0, LabelPolicy::FocalAndRoute),
        NodeCloseup => (0.72, 1.0, LabelPolicy::FocalAndRoute),
        Trace | Evasion => (0.46, 1.25, LabelPolicy::FocalAndRoute),
        Isolation => (0.38, 1.1, LabelPolicy::FocalAndRoute),
        Decoy => (0.65, 1.1, LabelPolicy::FocalAndRoute),
        DisplayAssault => (0.9, 1.15, LabelPolicy::Minimal),
        FinalDuel => (1.0, 1.25, LabelPolicy::Minimal),
        CrashWin => (0.5, 0.95, LabelPolicy::Scars),
        AcidWin => (1.0, 1.15, LabelPolicy::Minimal),
        Stalemate => (0.62, 0.82, LabelPolicy::Minimal),
        Aftermath => (0.2, 0.65, LabelPolicy::Scars),
    }
}
