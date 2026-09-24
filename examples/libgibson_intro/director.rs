//! A seekable cue sheet. No timer lives in Node or in the painter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Harness,
    Membrane,
    City,
    Facades,
    Couriers,
    Ascent,
    Planet,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cue {
    pub act: Act,
    pub start: f32,
    pub end: f32,
    pub name: &'static str,
}
pub const CUES: [Cue; 7] = [
    Cue {
        act: Act::Harness,
        start: 0.0,
        end: 20.0,
        name: "harness",
    },
    Cue {
        act: Act::Membrane,
        start: 20.0,
        end: 28.0,
        name: "membrane",
    },
    Cue {
        act: Act::City,
        start: 28.0,
        end: 36.0,
        name: "city",
    },
    Cue {
        act: Act::Facades,
        start: 36.0,
        end: 44.0,
        name: "facades",
    },
    Cue {
        act: Act::Couriers,
        start: 44.0,
        end: 53.0,
        name: "couriers",
    },
    Cue {
        act: Act::Ascent,
        start: 53.0,
        end: 60.0,
        name: "ascent",
    },
    Cue {
        act: Act::Planet,
        start: 60.0,
        end: 72.0,
        name: "planet",
    },
];
#[derive(Clone, Debug, PartialEq)]
pub struct Director {
    pub seconds: f32,
    pub paused: bool,
    /// Explicit presentation time, never wall-clock or paint-time state.
    pub hints_until: f32,
}
impl Default for Director {
    fn default() -> Self {
        Self {
            seconds: 0.0,
            paused: false,
            hints_until: 4.0,
        }
    }
}
impl Director {
    pub fn hints_visible(&self) -> bool {
        self.paused || self.seconds < self.hints_until || self.seconds >= 71.0
    }
    pub fn interacted(&mut self) {
        self.hints_until = self.seconds + 3.0;
    }
    pub fn cue(&self) -> Cue {
        CUES.iter()
            .copied()
            .find(|c| self.seconds < c.end)
            .unwrap_or(CUES[6])
    }
    pub fn seek(&mut self, seconds: f32) {
        if seconds.is_finite() {
            self.seconds = seconds.clamp(0.0, 72.0)
        }
    }
    pub fn advance(&mut self, dt: f32) {
        if !self.paused && dt.is_finite() && dt > 0.0 {
            self.seek(self.seconds + dt)
        }
    }
    pub fn stage(&mut self, name: &str) -> bool {
        if let Some(c) = CUES.iter().find(|c| c.name == name) {
            self.seconds = c.start;
            true
        } else {
            false
        }
    }
    pub fn skip(&mut self, forward: bool) {
        let index = CUES
            .iter()
            .position(|c| c.act == self.cue().act)
            .unwrap_or(0);
        self.seconds = CUES[if forward {
            (index + 1).min(6)
        } else {
            index.saturating_sub(1)
        }]
        .start;
        self.interacted();
    }
}
