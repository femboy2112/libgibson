//! The same finite job graph is realized as a harness, then as a city.
//! Demo-local simulation: no agents, tools, or network requests actually run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Agent {
    pub name: &'static str,
    pub role: &'static str,
    pub task: &'static str,
    pub result: &'static str,
    pub start_ms: u32,
    pub finish_ms: u32,
    pub depends_on: Option<usize>,
}
pub const AGENTS: [Agent; 4] = [
    Agent {
        name: "ARCHITECT",
        role: "plan / coordinate",
        task: "Decompose a transit disruption planner",
        result: "4 contracts / acceptance plan sealed",
        start_ms: 0,
        finish_ms: 3800,
        depends_on: None,
    },
    Agent {
        name: "SCOUT",
        role: "read / map",
        task: "Map the offline route graph + constraints",
        result: "128 stops / 384 links / 6 invariants",
        start_ms: 3800,
        finish_ms: 9400,
        depends_on: Some(0),
    },
    Agent {
        name: "BUILDER",
        role: "implement / refine",
        task: "Build an accessible reroute solver",
        result: "3 candidates / deterministic tie break",
        start_ms: 9400,
        finish_ms: 15500,
        depends_on: Some(1),
    },
    Agent {
        name: "VERIFY",
        role: "challenge / validate",
        task: "Probe disconnected and hostile fixtures",
        result: "24 fixtures / 24 pass / replay exact",
        start_ms: 15500,
        finish_ms: 19500,
        depends_on: Some(2),
    },
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobStatus {
    Queued,
    Running,
    Complete,
}
impl Agent {
    pub fn progress(self, seconds: f32) -> f32 {
        ((seconds * 1000.0 - self.start_ms as f32) / (self.finish_ms - self.start_ms) as f32)
            .clamp(0.0, 1.0)
    }
    pub fn status(self, seconds: f32) -> JobStatus {
        if seconds * 1000.0 >= self.finish_ms as f32 {
            JobStatus::Complete
        } else if seconds * 1000.0 >= self.start_ms as f32 {
            JobStatus::Running
        } else {
            JobStatus::Queued
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MessageRoute {
    pub from: usize,
    pub to: usize,
    pub payload: &'static str,
    pub depart: f32,
    pub arrive: f32,
}
pub const MESSAGES: [MessageRoute; 4] = [
    MessageRoute {
        from: 0,
        to: 1,
        payload: "ACCEPTANCE CONTRACT",
        depart: 44.0,
        arrive: 46.2,
    },
    MessageRoute {
        from: 1,
        to: 2,
        payload: "ROUTE GRAPH / 384 LINKS",
        depart: 46.0,
        arrive: 48.3,
    },
    MessageRoute {
        from: 2,
        to: 3,
        payload: "CANDIDATE / REPLAY SEED",
        depart: 48.1,
        arrive: 50.6,
    },
    MessageRoute {
        from: 3,
        to: 0,
        payload: "VERIFIED / 24 OF 24",
        depart: 50.4,
        arrive: 53.0,
    },
];
