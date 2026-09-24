//! Bounded diagnostic trace-record type shared by every Observatory mode.
//!
//! Yeah, this is the tool eating its own dog food: issue #10 was "a Vec grows
//! forever and nobody notices." So this log — the one THIS example uses to
//! remember what it just drew — is itself capped, and it tells on itself the
//! second it starts throwing records away. No panel here gets to quietly
//! pretend it's holding the whole run. If we didn't measure it, it's `?`.

use std::collections::VecDeque;

/// Where a diagnostic record comes from. Not every mode feeds every category —
/// Million-Tick, for instance, has nothing to say about RENDER/OUTPUT/INPUT/
/// SESSION, and that's fine: an empty category renders as `?`, never a guess.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Resource,
    Render,
    Output,
    Input,
    Session,
}

impl Category {
    pub const ALL: [Category; 5] = [
        Category::Resource,
        Category::Render,
        Category::Output,
        Category::Input,
        Category::Session,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Resource => "RESOURCE",
            Category::Render => "RENDER",
            Category::Output => "OUTPUT",
            Category::Input => "INPUT",
            Category::Session => "SESSION",
        }
    }
}

/// One observed fact. `source` names exactly where it came from (a real API
/// call, a real file), because "we measured this" is only a claim worth
/// trusting if you can say how.
#[derive(Clone, Debug, PartialEq)]
pub struct DiagRecord {
    pub seq: u64,
    pub monotonic_us: u64,
    pub category: Category,
    pub kind: String,
    pub source: String,
    pub value: String,
    /// Parsed numeric form, when `value` is a plain measurement, for the
    /// sparklines. `None` for the honestly non-numeric ones (e.g. yes/no).
    pub numeric: Option<f64>,
}

/// A ring buffer with a memory: once it starts evicting, it counts every
/// eviction forever, and that count is part of the public API, not a debug
/// aside. A caller who reads `retained()` without `dropped()` is reading a
/// lie by omission, so we make `dropped()` sit right next to it.
pub struct DiagLog {
    cap: usize,
    next_seq: u64,
    dropped: usize,
    records: VecDeque<DiagRecord>,
}

impl DiagLog {
    pub fn new(cap: usize) -> Self {
        Self {
            cap: cap.max(1),
            next_seq: 0,
            dropped: 0,
            records: VecDeque::with_capacity(cap.max(1)),
        }
    }

    pub fn push(
        &mut self,
        monotonic_us: u64,
        category: Category,
        kind: impl Into<String>,
        source: impl Into<String>,
        value: impl std::fmt::Display,
        numeric: Option<f64>,
    ) {
        let value = value.to_string();
        let record = DiagRecord {
            seq: self.next_seq,
            monotonic_us,
            category,
            kind: kind.into(),
            source: source.into(),
            value,
            numeric,
        };
        self.next_seq += 1;
        if self.records.len() >= self.cap {
            self.records.pop_front();
            self.dropped += 1;
        }
        self.records.push_back(record);
    }

    pub fn cap(&self) -> usize {
        self.cap
    }

    pub fn retained(&self) -> usize {
        self.records.len()
    }

    /// Diagnostic-log evictions. This is the Observatory's OWN drop count,
    /// distinct from a driven StoryDirector's `trace().dropped_steps()` — two
    /// different bounded buffers, two honest counters, never conflated.
    pub fn dropped(&self) -> usize {
        self.dropped
    }

    /// Most recent records first, oldest last, at most `n`.
    pub fn recent(&self, n: usize) -> impl Iterator<Item = &DiagRecord> {
        self.records.iter().rev().take(n)
    }

    /// Last record observed anywhere in a category, for the subsystem-status
    /// summary. `None` means "never observed" (or aged out) — render `?`.
    pub fn last_of_category(&self, category: Category) -> Option<&DiagRecord> {
        self.records.iter().rev().find(|r| r.category == category)
    }

    pub fn count_of_category(&self, category: Category) -> usize {
        self.records
            .iter()
            .filter(|r| r.category == category)
            .count()
    }

    /// Numeric series for a kind, oldest to newest, in retained order only —
    /// exactly what's still in the window, no reconstruction of what aged out.
    pub fn series_of_kind(&self, kind: &str) -> Vec<f64> {
        self.records
            .iter()
            .filter(|r| r.kind == kind)
            .filter_map(|r| r.numeric)
            .collect()
    }
}
