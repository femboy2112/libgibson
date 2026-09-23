//! A tiny, fictional graph contest. No operating-system or network interaction.
//! Integer influence and a fixed 50ms reducer quantum make every input replayable.
use std::collections::VecDeque;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeId {
    Modem,
    Route,
    Auth,
    Shell,
    Files,
    Display,
    Decoy,
}
impl NodeId {
    pub const ALL: [Self; 7] = [
        Self::Modem,
        Self::Route,
        Self::Auth,
        Self::Shell,
        Self::Files,
        Self::Display,
        Self::Decoy,
    ];
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        [
            "modem", "route", "auth", "shell", "files", "display", "decoy",
        ][self.index()]
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Crash,
    Contested,
    Acid,
}
impl Control {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Crash => "Crash",
            Self::Contested => "Contested",
            Self::Acid => "Acid",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeState {
    pub id: NodeId,
    /// -1000 is Acid; +1000 is Crash. This is independent of structural integrity.
    pub influence: i16,
    pub integrity: u16,
    pub isolated: bool,
    pub visible: bool,
    pub activity: u16,
    pub decoy: bool,
}
impl NodeState {
    pub fn owner(&self) -> Control {
        if self.influence > 350 {
            Control::Crash
        } else if self.influence < -350 {
            Control::Acid
        } else {
            Control::Contested
        }
    }
    pub fn acid_fraction(&self) -> f32 {
        (1000 - self.influence) as f32 / 2000.0
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdgeState {
    pub from: NodeId,
    pub to: NodeId,
    pub connected: bool,
    pub pressure: i16,
    pub cost: u16,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BattleGraph {
    pub nodes: [NodeState; 7],
    pub edges: Vec<EdgeState>,
}
impl Default for BattleGraph {
    fn default() -> Self {
        Self::new()
    }
}
impl BattleGraph {
    pub fn new() -> Self {
        use NodeId::*;
        Self {
            nodes: NodeId::ALL.map(|id| NodeState {
                id,
                influence: 1000,
                integrity: 1000,
                isolated: id == Decoy,
                visible: id != Decoy,
                activity: 0,
                decoy: id == Decoy,
            }),
            edges: [
                (Modem, Route, 1),
                (Route, Auth, 1),
                (Route, Shell, 1),
                (Auth, Files, 1),
                (Shell, Files, 1),
                (Files, Display, 1),
                (Modem, Shell, 3),
                (Shell, Decoy, 2),
                (Files, Decoy, 1),
            ]
            .into_iter()
            .map(|(from, to, cost)| EdgeState {
                from,
                to,
                connected: to != Decoy,
                pressure: 0,
                cost,
            })
            .collect(),
        }
    }
    pub fn node(&self, id: NodeId) -> &NodeState {
        &self.nodes[id.index()]
    }
    pub fn connected(&self, a: NodeId, b: NodeId) -> bool {
        self.edges
            .iter()
            .any(|e| e.connected && ((e.from == a && e.to == b) || (e.from == b && e.to == a)))
    }
    /// Stable shortest-hop route, respecting both endpoint isolation and severed edges.
    pub fn route(&self, from: NodeId, to: NodeId) -> Option<Vec<NodeId>> {
        if self.node(from).isolated || self.node(to).isolated {
            return None;
        }
        let mut queue = VecDeque::from([vec![from]]);
        let mut seen = [false; 7];
        seen[from.index()] = true;
        while let Some(path) = queue.pop_front() {
            let current = *path.last()?;
            if current == to {
                return Some(path);
            }
            for next in NodeId::ALL {
                if !seen[next.index()] && !self.node(next).isolated && self.connected(current, next)
                {
                    seen[next.index()] = true;
                    let mut extended = path.clone();
                    extended.push(next);
                    queue.push_back(extended);
                }
            }
        }
        None
    }
    fn isolate(&mut self, id: NodeId) {
        let node = &mut self.nodes[id.index()];
        node.isolated = true;
        node.visible = false;
        for edge in &mut self.edges {
            if edge.from == id || edge.to == id {
                edge.connected = false;
                edge.pressure = 0;
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    Explore,
    GainFoothold,
    ReachDisplay,
    EvadeTrace,
    PunishIsolation,
    TestDecoy,
    ShowOff,
    Escape,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tactic {
    ProbeEdge,
    PressureNode,
    Pivot,
    SplitRoute,
    Feint,
    Occupy,
    ScrambleDisplay,
    Retreat,
    AttackDecoy,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Personality {
    pub aggression: i32,
    pub curiosity: i32,
    pub pride: i32,
    pub evasiveness: i32,
    pub playfulness: i32,
}
impl Default for Personality {
    fn default() -> Self {
        Self {
            aggression: 65,
            curiosity: 80,
            pride: 75,
            evasiveness: 70,
            playfulness: 90,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AcidMemory {
    pub route_isolations: u16,
    pub auth_isolations: u16,
    pub decoys_seen: u16,
    pub decoy_deployments: u16,
    pub feints_completed: u16,
    pub trace_attempts: u16,
    pub passive_ticks: u32,
    pub visits: [u16; 7],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub tactic: Tactic,
    pub target: NodeId,
    pub path: Vec<NodeId>,
    pub score: i32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcidPlanner {
    pub personality: Personality,
    pub memory: AcidMemory,
    pub goal: Goal,
    pub tactic: Tactic,
    pub target: NodeId,
    pub location: NodeId,
    pub path: Vec<NodeId>,
    pub progress: u16,
    pub candidates: Vec<Candidate>,
    pub seed: u64,
}
impl AcidPlanner {
    fn new(seed: u64) -> Self {
        Self {
            personality: Personality::default(),
            memory: AcidMemory::default(),
            goal: Goal::Explore,
            tactic: Tactic::ProbeEdge,
            target: NodeId::Modem,
            location: NodeId::Modem,
            path: vec![NodeId::Modem],
            progress: 0,
            candidates: vec![],
            seed,
        }
    }
    /// Candidate legality is a graph property; utility can never override a missing route.
    pub fn choose(
        &mut self,
        graph: &BattleGraph,
        trace: u16,
        awareness: u16,
        elapsed: u64,
    ) -> Option<Candidate> {
        use NodeId::*;
        self.goal = if trace >= 650 || awareness >= 180 {
            Goal::EvadeTrace
        } else if graph.node(Decoy).visible
            && self.memory.decoy_deployments > self.memory.decoys_seen
        {
            Goal::TestDecoy
        } else if self.memory.route_isolations > 0 {
            Goal::PunishIsolation
        } else if graph.node(Display).influence < -350 {
            Goal::ShowOff
        } else if elapsed < 8000 {
            Goal::Explore
        } else if graph
            .nodes
            .iter()
            .filter(|n| n.owner() == Control::Acid)
            .count()
            < 2
        {
            Goal::GainFoothold
        } else {
            Goal::ReachDisplay
        };
        let origin = if graph.route(Modem, self.location).is_some() {
            self.location
        } else {
            Modem
        };
        self.candidates.clear();
        for target in NodeId::ALL.into_iter().filter(|id| *id != Modem) {
            // Both the active lease and the target must still connect to the entry.
            if graph.route(Modem, target).is_none() {
                continue;
            }
            let Some(path) = graph.route(origin, target) else {
                continue;
            };
            let node = graph.node(target);
            let repeat_mirror = target == Decoy
                && self.memory.decoys_seen > 0
                && self.memory.decoy_deployments > self.memory.decoys_seen;
            let tactic = if repeat_mirror {
                Tactic::Feint
            } else if target == Decoy {
                Tactic::AttackDecoy
            } else if self.goal == Goal::EvadeTrace {
                Tactic::SplitRoute
            } else if target == Display {
                Tactic::ScrambleDisplay
            } else if target != self.target && self.memory.route_isolations > 0 {
                Tactic::Pivot
            } else if node.influence < -350 {
                Tactic::Occupy
            } else if elapsed < 8000 {
                Tactic::ProbeEdge
            } else {
                Tactic::PressureNode
            };
            let route_cost: i32 = path
                .windows(2)
                .map(|pair| {
                    graph
                        .edges
                        .iter()
                        .find(|e| {
                            (e.from == pair[0] && e.to == pair[1])
                                || (e.from == pair[1] && e.to == pair[0])
                        })
                        .map_or(0, |e| i32::from(e.cost) * 18)
                })
                .sum();
            let novelty = 70 / (1 + i32::from(self.memory.visits[target.index()]));
            let vulnerable = (1000 - node.integrity) as i32 / 12;
            let foothold = if node.influence < -350 {
                -220
            } else {
                (1000 - i32::from(node.influence)) / 18
            };
            let objective = match target {
                Route => {
                    if elapsed < 11000 {
                        160
                    } else {
                        10
                    }
                }
                Auth => 115,
                Shell => 100,
                Files => 160,
                Display => {
                    if elapsed > 15000 {
                        320
                    } else {
                        -260
                    }
                }
                Decoy => {
                    if self.memory.decoys_seen == 0 || repeat_mirror {
                        750
                    } else {
                        -420
                    }
                }
                Modem => 0,
            };
            let bias = if target == Display {
                self.personality.pride + self.personality.playfulness
            } else if target == Decoy {
                self.personality.curiosity
            } else {
                self.personality.aggression
            };
            let evasion = if self.goal == Goal::EvadeTrace {
                if target != self.target {
                    self.personality.evasiveness + 100
                } else {
                    -150
                }
            } else {
                0
            };
            // Seed is a stable tie breaker, never hidden wall-clock randomness.
            let tie =
                ((self.seed.rotate_left(target.index() as u32) ^ target.index() as u64) % 7) as i32;
            let boldness = if target == Display {
                (self.memory.passive_ticks / 20).min(90) as i32
            } else {
                0
            };
            let repeated_route = if path.contains(&Route) {
                i32::from(self.memory.route_isolations) * 70
            } else {
                0
            };
            let exposed_hops = path
                .iter()
                .filter(|id| graph.node(**id).owner() == Control::Crash)
                .count() as i32;
            let trace_risk = i32::from(trace) * exposed_hops / 35;
            let score =
                objective + novelty + vulnerable + foothold + bias + evasion + tie + boldness
                    - route_cost
                    - repeated_route
                    - trace_risk;
            self.candidates.push(Candidate {
                tactic,
                target,
                path,
                score,
            });
        }
        self.candidates
            .sort_by(|a, b| b.score.cmp(&a.score).then(a.target.cmp(&b.target)));
        self.candidates.first().cloned()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Crash,
    Acid,
    Stalemate,
}
impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Crash => "CRASH CONTAINS",
            Self::Acid => "ACID WINS THE ROUND",
            Self::Stalemate => "STALEMATE / MUTUAL RESPECT",
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BattleQuality {
    pub first_response_ms: Option<u64>,
    pub systems_lost: u16,
    pub systems_recovered: u16,
    pub collateral_isolation: u16,
    pub decoy_success: u16,
    pub adaptations: u16,
    pub altered_files: u16,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionStatus {
    pub available: bool,
    pub cooldown_ms: u64,
    pub cost: &'static str,
    pub label: &'static str,
    pub reason: &'static str,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Milestone {
    RouteProbe,
    IdentityResolved,
    Foothold,
    DisplayIntrusion,
    DecoyTriggered,
    TraceAttempt,
    Adapted,
    TakeoverReady,
    Resolved,
}
impl Milestone {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RouteProbe => "RouteProbe",
            Self::IdentityResolved => "IdentityResolved",
            Self::Foothold => "Foothold",
            Self::DisplayIntrusion => "DisplayIntrusion",
            Self::DecoyTriggered => "DecoyTriggered",
            Self::TraceAttempt => "TraceAttempt",
            Self::Adapted => "Adapted",
            Self::TakeoverReady => "TakeoverReady",
            Self::Resolved => "Resolved",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncounterModel {
    pub graph: BattleGraph,
    pub planner: AcidPlanner,
    pub trace_confidence: u16,
    pub awareness: u16,
    pub resources: u16,
    pub elapsed_ms: u64,
    pub outcome: Option<Outcome>,
    pub outcome_quality: String,
    pub quality: BattleQuality,
    pub receipts: Vec<String>,
    pub last_action: String,
    pub remote_line: String,
    pub remote_line_at_ms: u64,
    pub cooldowns: [u64; 5],
    pub takeover: u16,
    pub remote_active: bool,
    resolution_ms: u64,
    remainder_ns: u128,
    planning_ms: u64,
    decoy_dwell_ms: u64,
    milestones: u16,
    lost: [bool; 7],
}
impl EncounterModel {
    pub fn new(stage: &str, seed: u64) -> Self {
        use NodeId::*;
        let elapsed_ms = match stage {
            "quiet" => 0,
            "knock" => 3500,
            "signature" => 6500,
            "route-contested" | "contest" => 10000,
            "first-breach" | "intrusion" => 16000,
            "adapt" | "counterplay" | "sidepath" | "decoy" | "trace" | "pressure" => 22000,
            "display-intrusion" | "ghost" | "escalation" => 32000,
            "trap" => 40000,
            "climax" => 49000,
            "takeover" => 56000,
            _ => 65000,
        };
        let mut this = Self {
            graph: BattleGraph::new(),
            planner: AcidPlanner::new(seed),
            trace_confidence: 0,
            awareness: 0,
            resources: 1000,
            elapsed_ms,
            outcome: None,
            outcome_quality: String::new(),
            quality: BattleQuality::default(),
            receipts: vec![],
            last_action: "LOCAL CONTROLS READY".into(),
            remote_line: String::new(),
            remote_line_at_ms: 0,
            cooldowns: [0; 5],
            takeover: 0,
            remote_active: true,
            resolution_ms: 0,
            remainder_ns: 0,
            planning_ms: 0,
            decoy_dwell_ms: 0,
            milestones: 0,
            lost: [false; 7],
        };
        if elapsed_ms >= 3500 {
            this.graph.nodes[Modem.index()].influence = -600;
        }
        if elapsed_ms >= 6500 {
            this.say("hello crash.");
        }
        if elapsed_ms >= 10000 {
            this.graph.nodes[Route.index()].influence = 0;
            this.planner.location = Route;
        }
        if elapsed_ms >= 16000 {
            this.graph.nodes[Route.index()].influence = -600;
            this.graph.nodes[Auth.index()].influence = -450;
            this.graph.nodes[Auth.index()].integrity = 870;
            this.planner.location = Auth;
        }
        if elapsed_ms >= 32000 {
            this.graph.nodes[Files.index()].influence = -500;
            this.graph.nodes[Display.index()].influence = -200;
            this.planner.location = Display;
            this.quality.altered_files = 3;
        }
        if elapsed_ms >= 49000 {
            this.graph.nodes[Display.index()].influence = -700;
            this.takeover = 600;
        }
        if stage == "sidepath" {
            this.command("isolate");
        }
        if stage == "decoy" {
            this.command("decoy");
        }
        if stage == "trace" {
            this.trace_confidence = 700;
            this.awareness = 360;
            this.planner.memory.trace_attempts = 2;
        }
        if stage == "crash-win" {
            this.resolve(Outcome::Crash);
        }
        if stage == "acid-win" {
            this.graph.nodes[Display.index()].influence = -1000;
            this.takeover = 1000;
            this.resolve(Outcome::Acid);
        }
        if stage == "stalemate" {
            this.resolve(Outcome::Stalemate);
        }
        this.replan();
        this
    }
    fn say(&mut self, line: &str) {
        self.remote_line = line.into();
        self.remote_line_at_ms = self.elapsed_ms;
    }
    fn receipt(&mut self, line: String) {
        self.last_action = line.clone();
        self.receipts.push(line);
        if self.receipts.len() > 24 {
            self.receipts.remove(0);
        }
    }
    pub fn action_status(&self, command: &str) -> ActionStatus {
        let (slot, price, cost, label) = match command {
            "trace" | "trace token" => (Some(0), 100, "bandwidth / awareness", "TRACE"),
            "isolate" | "isolate route" | "isolate auth" => {
                (Some(1), 0, "local visibility", "ISOLATE")
            }
            "decoy" | "bait" => (Some(2), 250, "250 resources", "DECOY"),
            "kill" | "kill session" => (Some(3), 100, "100 resources / one lease", "KILL SESSION"),
            "hard isolate" => (Some(4), 0, "telemetry / four systems", "HARD ISOLATE"),
            "turn trace" => (None, 0, "requires trace 80%", "TURN TRACE"),
            "spring decoy" => (None, 0, "requires occupied mirror", "SPRING DECOY"),
            "cut link" => (None, 0, "disconnects local graph", "CUT LINK"),
            "let her in" => (None, 0, "sacrifices DISPLAY", "LET HER IN"),
            _ => (None, 0, "observation", "WATCH"),
        };
        let cooldown_ms = slot.map_or(0, |i| self.cooldowns[i]);
        let telemetry = match command {
            "trace" | "trace token" => {
                self.graph.node(NodeId::Route).visible || self.graph.node(NodeId::Shell).visible
            }
            "decoy" | "bait" => self.graph.node(NodeId::Files).visible,
            _ => true,
        };
        let final_ready = self.elapsed_ms >= 45000 || self.takeover >= 600;
        let requirement = match command {
            "cut link" | "let her in" => final_ready,
            "turn trace" => final_ready && self.trace_confidence >= 800,
            "spring decoy" => {
                final_ready
                    && self.graph.node(NodeId::Decoy).visible
                    && self.graph.node(NodeId::Decoy).influence < 0
            }
            _ => true,
        };
        ActionStatus {
            available: self.outcome.is_none()
                && cooldown_ms == 0
                && self.resources >= price
                && telemetry
                && requirement,
            cooldown_ms,
            cost,
            label,
            reason: if self.outcome.is_some() {
                "RESOLVED"
            } else if cooldown_ms > 0 {
                "COOLDOWN"
            } else if self.resources < price {
                "LOW RESERVE"
            } else if !telemetry {
                "NO TELEMETRY"
            } else if !requirement {
                match command {
                    "turn trace" if final_ready => "NEED TRACE80",
                    "spring decoy" if final_ready => "NEED OCCUPIED MIRROR",
                    _ => "FINAL ACT",
                }
            } else {
                "READY"
            },
        }
    }
    pub fn command(&mut self, command: &str) {
        use NodeId::*;
        let command = command.trim().to_ascii_lowercase();
        if matches!(
            command.as_str(),
            "watch" | "facts" | "scene" | "acid" | "crash" | "damage" | "replay" | "exit"
        ) {
            return;
        }
        if !matches!(
            command.as_str(),
            "trace"
                | "trace token"
                | "isolate"
                | "isolate route"
                | "isolate auth"
                | "hard isolate"
                | "decoy"
                | "bait"
                | "kill"
                | "kill session"
                | "cut link"
                | "turn trace"
                | "spring decoy"
                | "let her in"
        ) {
            return;
        }
        let status = self.action_status(&command);
        if !status.available {
            self.receipt(format!(
                "{} UNAVAILABLE / {}",
                status.label,
                if status.cooldown_ms > 0 {
                    "cooldown"
                } else {
                    status.cost
                }
            ));
            return;
        }
        self.quality
            .first_response_ms
            .get_or_insert(self.elapsed_ms);
        self.planner.memory.passive_ticks = 0;
        match command.as_str() {
            "trace" | "trace token" => {
                self.trace_confidence = (self.trace_confidence + 280).min(1000);
                self.awareness = (self.awareness + 180).min(1000);
                self.resources -= 100;
                self.cooldowns[0] = 1200;
                self.planner.memory.trace_attempts =
                    self.planner.memory.trace_attempts.saturating_add(1);
                self.say("following me costs you room.");
                self.receipt("TRACE / return pulse / awareness rises".into());
            }
            "isolate" | "isolate route" | "isolate auth" => {
                let id = if command == "isolate auth" {
                    Auth
                } else {
                    Route
                };
                let newly_isolated = !self.graph.node(id).isolated;
                self.graph.isolate(id);
                self.graph.nodes[id.index()].influence =
                    (self.graph.node(id).influence + 400).min(1000);
                self.cooldowns[1] = 800;
                self.quality.collateral_isolation = self
                    .quality
                    .collateral_isolation
                    .saturating_add(u16::from(newly_isolated));
                if id == Route {
                    self.planner.memory.route_isolations =
                        self.planner.memory.route_isolations.saturating_add(1);
                } else {
                    self.planner.memory.auth_isolations =
                        self.planner.memory.auth_isolations.saturating_add(1);
                }
                self.say("you closed a door. left a window.");
                self.receipt(format!(
                    "ISOLATE {} / telemetry lost",
                    id.name().to_uppercase()
                ));
            }
            "hard isolate" => {
                let newly_isolated = [Route, Auth, Files, Display]
                    .into_iter()
                    .filter(|id| !self.graph.node(*id).isolated)
                    .count() as u16;
                for id in [Route, Auth, Files, Display] {
                    self.graph.isolate(id);
                }
                self.cooldowns[4] = 5000;
                self.quality.collateral_isolation = self
                    .quality
                    .collateral_isolation
                    .saturating_add(newly_isolated);
                self.planner.memory.route_isolations =
                    self.planner.memory.route_isolations.saturating_add(1);
                self.say("dark in there now, isn't it?");
                self.receipt("HARD ISOLATE / TELEMETRY BLIND / islands severed".into());
            }
            "decoy" | "bait" => {
                self.planner.memory.decoy_deployments =
                    self.planner.memory.decoy_deployments.saturating_add(1);
                let node = &mut self.graph.nodes[Decoy.index()];
                node.visible = true;
                node.isolated = false;
                node.influence = 600;
                for edge in &mut self.graph.edges {
                    if edge.to == Decoy && !self.graph.nodes[edge.from.index()].isolated {
                        edge.connected = true;
                    }
                }
                self.resources -= 250;
                self.cooldowns[2] = 2500;
                self.decoy_dwell_ms = 0;
                self.receipt("DECOY / mirror attached to FILES".into());
            }
            "kill" | "kill session" => {
                if let Some(id) = NodeId::ALL
                    .into_iter()
                    .filter(|id| *id != Modem && self.graph.node(*id).visible)
                    .min_by_key(|id| self.graph.node(*id).influence)
                {
                    self.graph.nodes[id.index()].influence =
                        (self.graph.node(id).influence + 850).min(1000);
                    self.receipt(format!("KILL SESSION / {} lease collapsed", id.name()));
                }
                self.resources -= 100;
                self.cooldowns[3] = 1800;
            }
            "cut link" => {
                self.resolve(Outcome::Crash);
            }
            "turn trace" => {
                self.resolve(Outcome::Crash);
            }
            "spring decoy" => {
                self.quality.decoy_success = self.quality.decoy_success.saturating_add(1);
                self.trace_confidence = (self.trace_confidence + 400).min(1000);
                self.resolve(Outcome::Crash);
            }
            "let her in" => {
                // This is an explicit invitation, not a bypass of topology:
                // Crash opens a visible corridor and sacrifices its control.
                let invitation = [Modem, Shell, Files, Display];
                for id in invitation {
                    let node = &mut self.graph.nodes[id.index()];
                    node.isolated = false;
                    node.visible = true;
                    node.influence = -600;
                    node.activity = 1000;
                }
                for pair in invitation.windows(2) {
                    for edge in &mut self.graph.edges {
                        if (edge.from == pair[0] && edge.to == pair[1])
                            || (edge.from == pair[1] && edge.to == pair[0])
                        {
                            edge.connected = true;
                            edge.pressure = -1000;
                        }
                    }
                }
                self.planner.path = invitation.to_vec();
                self.planner.location = Display;
                self.planner.target = Display;
                self.planner.progress = 1000;
                self.planner.tactic = Tactic::ScrambleDisplay;
                self.receipt("INVITE / MODEM > SHELL > FILES > DISPLAY opened".into());
                self.graph.nodes[Display.index()].influence = -1000;
                self.takeover = 1000;
                self.trace_confidence = (self.trace_confidence + 200).min(1000);
                self.resolve(Outcome::Acid);
            }
            _ => return,
        }
        self.replan();
    }
    fn replan(&mut self) {
        if self.outcome.is_some() {
            return;
        }
        let previous = self.planner.target;
        if let Some(choice) = self.planner.choose(
            &self.graph,
            self.trace_confidence,
            self.awareness,
            self.elapsed_ms,
        ) {
            if choice.target != previous {
                self.quality.adaptations = self.quality.adaptations.saturating_add(1);
            }
            if choice.path != self.planner.path {
                self.planner.progress = 0;
            }
            self.planner.target = choice.target;
            self.planner.tactic = choice.tactic;
            self.planner.path = choice.path;
            self.planner.location = self.planner.path[0];
        } else {
            self.planner.goal = Goal::Escape;
            self.planner.tactic = Tactic::Retreat;
            self.planner.path.clear();
            self.planner.progress = 0;
        }
    }
    fn resolve(&mut self, outcome: Outcome) {
        self.outcome = Some(outcome);
        self.resolution_ms = self.elapsed_ms;
        self.remote_active = outcome == Outcome::Acid;
        self.outcome_quality = match outcome {
            Outcome::Crash => {
                if self.quality.collateral_isolation >= 3 {
                    "costly contain"
                } else if self.trace_confidence >= 800 {
                    "traced contain"
                } else {
                    "clean contain"
                }
            }
            Outcome::Acid => {
                if self.quality.systems_lost >= 4 {
                    "decisive possession"
                } else {
                    "display prank"
                }
            }
            Outcome::Stalemate => {
                if self.quality.collateral_isolation > 0 {
                    "mutual disconnect"
                } else {
                    "mutual route hold"
                }
            }
        }
        .into();
        if outcome != Outcome::Acid {
            for node in &mut self.graph.nodes {
                node.activity = 0;
            }
            for edge in &mut self.graph.edges {
                if outcome == Outcome::Crash || self.quality.collateral_isolation > 0 {
                    edge.connected = false;
                }
                edge.pressure = 0;
            }
        }
        if outcome == Outcome::Stalemate {
            // Neither side claims the previously invaded territory. A route
            // hold preserves its topology; prior isolation yields disconnect.
            for node in &mut self.graph.nodes {
                if node.influence < 350 {
                    node.influence = 0;
                }
            }
            self.takeover = 0;
        }
        if outcome == Outcome::Crash {
            for node in &mut self.graph.nodes {
                node.influence = 700;
            }
            self.takeover = 0;
        }
        self.say(match outcome {
            Outcome::Crash => "you kept the machine. keep the receipt.",
            Outcome::Acid => "borrowed your view. returning it shortly.",
            Outcome::Stalemate => "same time, different route.",
        });
        self.receipt(format!("RESOLVED / {}", self.outcome_quality));
    }
    /// Commands execute once at the update boundary. Time advances in exact 50ms
    /// quanta with remainder retained. The encounter has a finite 90s horizon,
    /// so even Duration::MAX requires at most 1800 reducer iterations.
    pub fn update(&mut self, dt: Duration, commands: &[String]) -> Vec<Milestone> {
        for command in commands {
            self.command(command);
        }
        let total = self.remainder_ns.saturating_add(dt.as_nanos());
        let steps = (total / 50_000_000).min(1800) as u64;
        self.remainder_ns = total % 50_000_000;
        for _ in 0..steps {
            if self.elapsed_ms >= 90000 {
                break;
            }
            self.step();
        }
        let mut events = vec![];
        let conditions = [
            (Milestone::RouteProbe, self.elapsed_ms >= 3500),
            (Milestone::IdentityResolved, self.elapsed_ms >= 6500),
            (
                Milestone::Foothold,
                self.graph
                    .nodes
                    .iter()
                    .filter(|n| !n.decoy && n.id != NodeId::Modem)
                    .any(|n| n.owner() == Control::Acid),
            ),
            (
                Milestone::DisplayIntrusion,
                self.graph.node(NodeId::Display).influence < 350,
            ),
            (
                Milestone::DecoyTriggered,
                self.graph.node(NodeId::Decoy).influence < 0,
            ),
            (
                Milestone::TraceAttempt,
                self.planner.memory.trace_attempts > 0,
            ),
            (Milestone::Adapted, self.quality.adaptations >= 2),
            (Milestone::TakeoverReady, self.takeover >= 600),
            (Milestone::Resolved, self.outcome.is_some()),
        ];
        for (event, condition) in conditions {
            let bit = 1 << event as u16;
            if condition && self.milestones & bit == 0 {
                self.milestones |= bit;
                events.push(event);
            }
        }
        events
    }
    fn step(&mut self) {
        use NodeId::*;
        self.elapsed_ms += 50;
        for cooldown in &mut self.cooldowns {
            *cooldown = cooldown.saturating_sub(50);
        }
        if self.outcome.is_some() {
            if self.outcome == Some(Outcome::Acid)
                && self.elapsed_ms.saturating_sub(self.resolution_ms) >= 3500
            {
                self.remote_active = false;
                for edge in &mut self.graph.edges {
                    edge.pressure = 0;
                }
                for node in &mut self.graph.nodes {
                    node.activity = 0;
                    node.influence = (node.influence + 30).min(700);
                }
                self.takeover = self.takeover.saturating_sub(20);
            }
            return;
        }
        self.resources = (self.resources + 1).min(1000);
        for node in &mut self.graph.nodes {
            node.activity = node.activity.saturating_sub(35);
            if node.isolated {
                node.influence = (node.influence + 12).min(1000);
            }
        }
        if self.elapsed_ms < 3500 {
            return;
        }
        self.graph.nodes[Modem.index()].influence = -650;
        self.planner.memory.passive_ticks = self.planner.memory.passive_ticks.saturating_add(1);
        if self.elapsed_ms >= 6500 && self.remote_line.is_empty() {
            self.say("hello crash.");
        }
        self.planning_ms += 50;
        let occupied = self.graph.node(self.planner.target).influence < -700;
        let reconsider = self.planning_ms >= 1200
            && ((occupied
                && (self.planner.target != Decoy || self.planner.memory.decoys_seen > 0))
                || self.planner.goal == Goal::EvadeTrace);
        if reconsider
            || !self
                .planner
                .path
                .windows(2)
                .all(|p| self.graph.connected(p[0], p[1]))
        {
            self.planning_ms = 0;
            self.replan();
        }
        for edge in &mut self.graph.edges {
            edge.pressure = edge.pressure.saturating_add(20).min(0);
        }
        if let Some(&target) = self.planner.path.last() {
            if self.graph.route(Modem, target).is_some() {
                let len = self.planner.path.len();
                if len > 1 {
                    // A fortified intermediate node must yield before the next edge
                    // is usable. Influence travels from the current location.
                    if self.graph.node(self.planner.location).influence <= 350 {
                        self.planner.progress =
                            (self.planner.progress + (70 / (len as u16 - 1)).max(1)).min(1000);
                    }
                    let scaled = usize::from(self.planner.progress) * (len - 1);
                    let segment = (scaled / 1000).min(len - 1);
                    self.planner.location = self.planner.path[segment];
                    for pair in self.planner.path.windows(2).take(segment.saturating_add(1)) {
                        for edge in &mut self.graph.edges {
                            if edge.connected
                                && ((edge.from == pair[0] && edge.to == pair[1])
                                    || (edge.from == pair[1] && edge.to == pair[0]))
                            {
                                edge.pressure = -700;
                            }
                        }
                    }
                } else {
                    self.planner.progress = 1000;
                }
                // Recognized mirrors provoke a visible approach toward their
                // final edge, then a retreat before the mirror receives control.
                // The next plan starts at the last actual node, never the lure.
                if self.planner.tactic == Tactic::Feint
                    && (len <= 1
                        || usize::from(self.planner.progress) * (len - 1) >= (len - 2) * 1000 + 650)
                {
                    if len > 1 {
                        self.planner.location = self.planner.path[len - 2];
                    }
                    self.planner.memory.decoys_seen = self.planner.memory.decoy_deployments;
                    self.planner.memory.feints_completed =
                        self.planner.memory.feints_completed.saturating_add(1);
                    self.say("same mirror. different reflection.");
                    self.receipt("FEINT / mirror recognized / route withdrawn".into());
                    self.replan();
                    return;
                }
                let current = self.planner.location;
                let rate = if self.elapsed_ms < 8000 {
                    5
                } else if self.planner.goal == Goal::EvadeTrace {
                    14
                } else {
                    28
                };
                let node = &mut self.graph.nodes[current.index()];
                node.influence = (node.influence - rate).max(-1000);
                node.activity = 1000;
                if node.influence < 0 && self.elapsed_ms.is_multiple_of(500) {
                    node.integrity = node.integrity.saturating_sub(2);
                }
                if self.planner.progress == 1000 {
                    self.planner.memory.visits[target.index()] =
                        self.planner.memory.visits[target.index()].saturating_add(1);
                }
                if current == Decoy && self.graph.node(Decoy).influence < 0 {
                    self.decoy_dwell_ms += 50;
                    if self.decoy_dwell_ms >= 1800 {
                        self.planner.memory.decoys_seen =
                            self.planner.memory.decoys_seen.saturating_add(1);
                        self.quality.decoy_success = self.quality.decoy_success.saturating_add(1);
                        self.trace_confidence = (self.trace_confidence + 180).min(1000);
                        self.say("a mirror. cute once.");
                        self.decoy_dwell_ms = 0;
                        self.replan();
                    }
                }
            }
        }
        for id in NodeId::ALL
            .into_iter()
            .filter(|id| *id != Decoy && *id != Modem)
        {
            let acid = self.graph.node(id).owner() == Control::Acid;
            if acid && !self.lost[id.index()] {
                self.quality.systems_lost = self.quality.systems_lost.saturating_add(1);
                self.lost[id.index()] = true;
            }
            if self.lost[id.index()] && self.graph.node(id).owner() == Control::Crash {
                self.quality.systems_recovered = self.quality.systems_recovered.saturating_add(1);
                self.lost[id.index()] = false;
            }
        }
        if self.graph.node(Files).influence < 0 && self.elapsed_ms.is_multiple_of(2000) {
            self.quality.altered_files = self.quality.altered_files.saturating_add(1);
        }
        if self.graph.node(Display).influence < -350 && self.graph.route(Modem, Display).is_some() {
            self.takeover = (self.takeover + 3).min(1000);
        } else {
            self.takeover = self.takeover.saturating_sub(9);
        }
        if self.elapsed_ms >= 80000 {
            self.resolve(if self.graph.route(Modem, Display).is_none() {
                Outcome::Crash
            } else if self.takeover >= 600 {
                Outcome::Acid
            } else {
                Outcome::Stalemate
            });
        }
    }
    pub fn defender_command(&self) -> Option<String> {
        if self.outcome.is_some() || self.elapsed_ms < 11000 {
            return None;
        }
        let command = if self.elapsed_ms >= 58000 {
            if self.trace_confidence >= 800 {
                "turn trace"
            } else {
                "cut link"
            }
        } else if self.planner.target == NodeId::Decoy
            && self.graph.node(NodeId::Decoy).influence < 0
            && self.elapsed_ms > 40000
        {
            "spring decoy"
        } else if self.graph.node(NodeId::Auth).influence < 350
            && !self.graph.node(NodeId::Auth).isolated
        {
            "isolate auth"
        } else if self.planner.memory.trace_attempts >= 2
            && !self.graph.node(NodeId::Route).isolated
            && self.graph.node(NodeId::Route).influence < -350
        {
            "isolate"
        } else if (self.planner.goal == Goal::ReachDisplay || self.planner.goal == Goal::EvadeTrace)
            && !self.graph.node(NodeId::Decoy).visible
        {
            "decoy"
        } else if self.graph.node(NodeId::Display).influence < -500 {
            "kill"
        } else if self.trace_confidence < 840 {
            "trace"
        } else {
            return None;
        };
        self.action_status(command)
            .available
            .then(|| command.into())
    }
}
