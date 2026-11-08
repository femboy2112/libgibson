//! Held-key inference for press-only terminals (v0.7 execution graph, node D).
//!
//! A terminal keyboard reports key **presses**. While a key stays down, the terminal's
//! **autorepeat** re-sends the same press after an initial delay (commonly 200–660 ms) at a
//! steady rate (commonly 25–40 Hz). Outside optional keyboard protocols it reports **no
//! release**, and [`KeyEvent`](crate::input::KeyEvent) carries no press / repeat / release kind.
//! A program that acts *while* a key is held — continuous movement, a held trigger — therefore
//! has to infer the hold. [`HoldTracker`](crate::hold::HoldTracker) makes that inference explicit,
//! deterministic and labelled as a model:
//!
//! * **Simulation time only.** Every call takes a tic (`u64`) supplied by the caller's
//!   fixed-step simulation (for example [`FixedTimestep`](crate::clock::FixedTimestep)). The
//!   tracker never reads a clock, so one `(tic, key)` trace yields the same holds at any
//!   presentation cadence and on any machine.
//! * **An observation starts or extends a hold.** The first observation of a key starts a hold
//!   that ends `initial_grace` tics later, long enough to bridge the terminal's autorepeat delay.
//!   Each later observation while the key is held is treated as autorepeat and re-arms the end
//!   `repeat_timeout` tics after it.
//! * **The end is inferred, unless releases are reported (below).** A hold ends when its
//!   deadline passes ([`HoldEnd::Timeout`](crate::hold::HoldEnd::Timeout)) or when the caller
//!   ends holds for a declared reason ([`HoldEnd::Released`](crate::hold::HoldEnd::Released) —
//!   focus lost, input disconnected, or an explicit release). No
//!   synthetic key-release [`Event`] is ever produced: an ended hold is returned as model output
//!   ([`EndedHold`](crate::hold::EndedHold)) that says how it ended.
//!
//! **The rule, exactly.** Let `L` be a key's latest observation. Its timeout `T` is
//! `initial_grace` if `L` started the hold, and `repeat_timeout` otherwise. Unless a release
//! intervenes, the key is held at every tic `t` with `L <= t < L + T` and is no longer held at
//! tic `L + T`. Anything the hold drives therefore stops exactly `T` tics after the last
//! observation. A physical release cannot be detected sooner; that lag is the price of the
//! missing release event, and it is stated here rather than hidden. An observation that arrives
//! exactly at `L + T` starts a new hold.
//!
//! **Reported releases (#275).** A terminal that speaks the kitty keyboard protocol can report
//! releases (`TerminalSession::set_key_release_reporting`). Once the tracker is told so
//! ([`set_releases_reported`](crate::hold::HoldTracker::set_releases_reported), or an
//! [`Event::KeyReleaseReporting`](crate::input::Event::KeyReleaseReporting) through
//! [`observe_event`](crate::hold::HoldTracker::observe_event)), nothing is inferred for a key
//! observed from then on: its hold has no deadline and ends when its release is observed
//! ([`observe_release`](crate::hold::HoldTracker::observe_release), ending as
//! [`ReleaseCause::KeyReleased`](crate::hold::ReleaseCause::KeyReleased)) or for a declared
//! reason. So a tap lasts as long as the key was down, and keys held together stay held
//! together. A release observed at the tic its hold started ends it one tic later: every press
//! is held for at least one tic. A hold that started before releases were reported keeps its
//! deadline until the key is observed again; if reporting stops, every hold falls back to the
//! inferred rule, as if pressed at that tic.
//!
//! The tracker is pure: no I/O, no clock, no global state. Memory is one small record per key
//! currently held; ended holds are returned to the caller, not retained.

use crate::input::{Event, FocusChange, KeyEvent, KeyReleaseReporting};
use std::fmt;
use std::time::Duration;

/// Timeouts of the hold model, in simulation tics. Both are at least 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoldConfig {
    initial_grace: u64,
    repeat_timeout: u64,
}

impl HoldConfig {
    /// A hold lasts `initial_grace` tics after its first observation and `repeat_timeout` tics
    /// after each later one. Each value is clamped to at least 1 tic (0 would mean "never held").
    pub const fn new(initial_grace: u64, repeat_timeout: u64) -> Self {
        const fn at_least_one(n: u64) -> u64 {
            if n == 0 {
                1
            } else {
                n
            }
        }
        Self {
            initial_grace: at_least_one(initial_grace),
            repeat_timeout: at_least_one(repeat_timeout),
        }
    }

    /// Converts wall-clock timeouts into tics of length `tic`, rounding **up** so a hold never
    /// ends earlier than the requested duration. `tic` is clamped to at least 1 ns.
    pub fn from_durations(
        tic: Duration,
        initial_grace: Duration,
        repeat_timeout: Duration,
    ) -> Self {
        let tic_ns = tic.as_nanos().max(1);
        let tics = |d: Duration| -> u64 {
            let n = d.as_nanos().div_ceil(tic_ns);
            u64::try_from(n).unwrap_or(u64::MAX)
        };
        Self::new(tics(initial_grace), tics(repeat_timeout))
    }

    /// Tics a hold lasts after its first observation.
    pub fn initial_grace(&self) -> u64 {
        self.initial_grace
    }

    /// Tics a hold lasts after each later (autorepeat) observation.
    pub fn repeat_timeout(&self) -> u64 {
        self.repeat_timeout
    }
}

/// Why the caller ended holds before their deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReleaseCause {
    /// The terminal reported a focus loss: keys pressed elsewhere cannot be observed.
    FocusLost,
    /// The input source failed or closed: no further observation is possible.
    Disconnected,
    /// The application ended the hold for its own reason (a pause, a mode change).
    Explicit,
    /// The terminal reported the key going up (key-release reporting, #275).
    KeyReleased,
}

/// How a hold ended. Only [`ReleaseCause::KeyReleased`] is an observed key release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HoldEnd {
    /// No observation arrived before the deadline.
    Timeout,
    /// The caller ended the hold for the stated reason.
    Released(ReleaseCause),
}

/// One active hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hold {
    /// Tic of the observation that started the hold.
    pub started: u64,
    /// Tic of the latest observation.
    pub last_seen: u64,
    /// Observations folded into this hold (saturating).
    pub observations: u32,
    /// First tic at which the key is no longer held, absent a further observation. `u64::MAX`
    /// while releases are reported: the hold ends when its release is observed.
    pub ends_at: u64,
}

/// A hold that has ended, returned as model output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndedHold<K> {
    /// The caller's key identity.
    pub key: K,
    /// The hold as it stood when it ended.
    pub hold: Hold,
    /// First tic at which the key was no longer held. For a timeout this is
    /// `hold.ends_at`, independent of when the caller advanced the tracker.
    pub ended_at: u64,
    /// How it ended.
    pub end: HoldEnd,
}

/// A call supplied a tic earlier than one already processed. The tracker is unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HoldError {
    /// `got` is earlier than the tracker's current tic `now`.
    TicRegressed { now: u64, got: u64 },
}

impl fmt::Display for HoldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TicRegressed { now, got } => {
                write!(f, "tic {got} is earlier than the current tic {now}")
            }
        }
    }
}

impl std::error::Error for HoldError {}

/// Infers held keys from press / autorepeat observations on a simulation timebase.
///
/// `K` is the caller's key identity (for example [`KeyCode`](crate::input::KeyCode), or an
/// action enum after the caller's own key mapping). Keys are independent: any number can be
/// held at once. Iteration order is the order in which holds started.
///
/// ```
/// use gibson::hold::{HoldConfig, HoldEnd, HoldTracker};
///
/// // 35 Hz tics: ~0.51 s to bridge the autorepeat delay, ~0.09 s between repeats.
/// let mut holds = HoldTracker::new(HoldConfig::new(18, 3));
/// holds.observe(0, 'w').unwrap(); // press
/// assert!(holds.is_held('w'));
/// holds.observe(16, 'w').unwrap(); // first autorepeat, before the grace ran out
/// holds.observe(17, 'w').unwrap(); // autorepeat
/// holds.advance(19).unwrap();
/// assert!(holds.is_held('w')); // 17 + 3 = 20 is the first tic without the key
/// let ended = holds.advance(20).unwrap();
/// assert_eq!(ended[0].ended_at, 20);
/// assert_eq!(ended[0].end, HoldEnd::Timeout);
/// assert!(!holds.is_held('w'));
/// ```
#[derive(Debug, Clone)]
pub struct HoldTracker<K> {
    config: HoldConfig,
    now: u64,
    /// Invariant: every stored hold has `ends_at > now` — `advance` removes the rest and an
    /// observation always sets `ends_at` past its own tic.
    holds: Vec<(K, Hold)>,
    /// Whether the terminal reports releases: holds observed now have no deadline.
    releases_reported: bool,
    /// Keys whose release was observed at the tic their hold started: each ends at its
    /// `ends_at` (one tic later) as [`ReleaseCause::KeyReleased`], not as a timeout.
    released: Vec<K>,
}

impl<K: Copy + Eq> HoldTracker<K> {
    /// An empty tracker at tic 0.
    pub fn new(config: HoldConfig) -> Self {
        Self {
            config,
            now: 0,
            holds: Vec::new(),
            releases_reported: false,
            released: Vec::new(),
        }
    }

    /// Whether holds observed now end at their reported release instead of a deadline.
    pub fn releases_reported(&self) -> bool {
        self.releases_reported
    }

    /// Declares whether the terminal reports key releases (see the module documentation).
    /// Turning it on changes holds as they are observed from now on. Turning it off gives every
    /// current hold the inferred deadline it would have if pressed at the current tic, so none
    /// waits for a release that will not come.
    pub fn set_releases_reported(&mut self, on: bool) {
        if self.releases_reported && !on {
            let ends_at = self.now.saturating_add(self.config.initial_grace);
            for (key, hold) in &mut self.holds {
                if !self.released.contains(key) {
                    // `ends_at > now` (grace >= 1) except at the last representable tic.
                    hold.ends_at = hold.ends_at.min(ends_at);
                }
            }
        }
        self.releases_reported = on;
    }

    /// The model's timeouts.
    pub fn config(&self) -> HoldConfig {
        self.config
    }

    /// The latest tic the tracker has processed.
    pub fn now(&self) -> u64 {
        self.now
    }

    fn check(&self, tic: u64) -> Result<(), HoldError> {
        if tic < self.now {
            Err(HoldError::TicRegressed {
                now: self.now,
                got: tic,
            })
        } else {
            Ok(())
        }
    }

    /// Moves the tracker to `tic` and returns every hold whose deadline is at or before it,
    /// ordered by `ended_at` and then by start order — the same records whether the caller
    /// advances every tic or jumps many tics at once.
    pub fn advance(&mut self, tic: u64) -> Result<Vec<EndedHold<K>>, HoldError> {
        self.check(tic)?;
        self.now = tic;
        let mut ended = Vec::new();
        let released = &mut self.released;
        self.holds.retain(|&(key, hold)| {
            if hold.ends_at <= tic {
                let end = match released.iter().position(|k| *k == key) {
                    Some(i) => {
                        released.swap_remove(i);
                        HoldEnd::Released(ReleaseCause::KeyReleased)
                    }
                    None => HoldEnd::Timeout,
                };
                ended.push(EndedHold {
                    key,
                    hold,
                    ended_at: hold.ends_at,
                    end,
                });
                false
            } else {
                true
            }
        });
        // `retain` visits in start order; a stable sort by end tic keeps that order on ties.
        ended.sort_by_key(|e| e.ended_at);
        Ok(ended)
    }

    /// Records an observation (a press or an autorepeat) of `key` at `tic`. First advances to
    /// `tic`, returning the holds that ended on the way; then starts a hold for `key`, or extends
    /// its current one.
    ///
    /// At `u64::MAX`, the last representable tic, a hold's interval `[MAX, MAX)` is empty: the
    /// observation is reported at once as a hold that ended at `MAX`, and nothing stays held.
    pub fn observe(&mut self, tic: u64, key: K) -> Result<Vec<EndedHold<K>>, HoldError> {
        let mut ended = self.advance(tic)?;
        let config = self.config;
        let (grace, repeat) = if self.releases_reported {
            (u64::MAX, u64::MAX)
        } else {
            (config.initial_grace, config.repeat_timeout)
        };
        // Observed again after its release at this same tic: the key is down again.
        if let Some(i) = self.released.iter().position(|k| *k == key) {
            self.released.swap_remove(i);
        }
        let index = self.holds.iter().position(|(k, _)| *k == key);
        let hold = match index {
            Some(i) => {
                let mut hold = self.holds[i].1;
                hold.last_seen = tic;
                hold.observations = hold.observations.saturating_add(1);
                hold.ends_at = tic.saturating_add(repeat);
                hold
            }
            None => Hold {
                started: tic,
                last_seen: tic,
                observations: 1,
                ends_at: tic.saturating_add(grace),
            },
        };
        if hold.ends_at > tic {
            match index {
                Some(i) => self.holds[i].1 = hold,
                None => self.holds.push((key, hold)),
            }
        } else {
            // Saturated at the end of time: keep the invariant `ends_at > now` by never storing it.
            if let Some(i) = index {
                self.holds.remove(i);
            }
            ended.push(EndedHold {
                key,
                hold,
                ended_at: tic,
                end: HoldEnd::Timeout,
            });
        }
        Ok(ended)
    }

    /// Records that the terminal reported `key` going up at `tic` (#275). First advances to
    /// `tic`, returning the holds that ended on the way. Then ends `key`'s hold with
    /// [`ReleaseCause::KeyReleased`]: at `tic`, or at the following tic if the hold started at
    /// `tic`, so every press is held for at least one tic. A release of a key that is not held
    /// changes nothing.
    pub fn observe_release(&mut self, tic: u64, key: K) -> Result<Vec<EndedHold<K>>, HoldError> {
        let mut ended = self.advance(tic)?;
        let Some(i) = self.holds.iter().position(|(k, _)| *k == key) else {
            return Ok(ended);
        };
        if self.holds[i].1.started < tic {
            ended.extend(self.release(key, ReleaseCause::KeyReleased));
        } else if tic < u64::MAX {
            self.holds[i].1.ends_at = tic + 1;
            if !self.released.contains(&key) {
                self.released.push(key);
            }
        } else {
            // At the end of time there is no next tic to hold it for.
            ended.extend(self.release(key, ReleaseCause::KeyReleased));
        }
        Ok(ended)
    }

    /// Ends every current hold at the current tic for `cause`, in start order.
    pub fn release_all(&mut self, cause: ReleaseCause) -> Vec<EndedHold<K>> {
        self.released.clear();
        let now = self.now;
        self.holds
            .drain(..)
            .map(|(key, hold)| EndedHold {
                key,
                hold,
                ended_at: now,
                end: HoldEnd::Released(cause),
            })
            .collect()
    }

    /// Ends `key`'s hold at the current tic for `cause`, if it is held.
    pub fn release(&mut self, key: K, cause: ReleaseCause) -> Option<EndedHold<K>> {
        if let Some(i) = self.released.iter().position(|k| *k == key) {
            self.released.swap_remove(i);
        }
        let index = self.holds.iter().position(|(k, _)| *k == key)?;
        let (key, hold) = self.holds.remove(index);
        Some(EndedHold {
            key,
            hold,
            ended_at: self.now,
            end: HoldEnd::Released(cause),
        })
    }

    /// Routes one input event at `tic`: a key event mapped by `key_of` is an observation; a key
    /// release mapped by `key_of` is a reported release ([`observe_release`](Self::observe_release));
    /// the answer to a key-release reporting request sets
    /// [`set_releases_reported`](Self::set_releases_reported); a focus loss ends every hold
    /// ([`ReleaseCause::FocusLost`]); every other event (focus gain, resize, paste, mouse, tick,
    /// or a key `key_of` ignores) only advances time, so geometry changes never touch holds.
    pub fn observe_event(
        &mut self,
        tic: u64,
        event: &Event,
        key_of: impl FnOnce(&KeyEvent) -> Option<K>,
    ) -> Result<Vec<EndedHold<K>>, HoldError> {
        match event {
            Event::Key(k) => match key_of(k) {
                Some(key) => self.observe(tic, key),
                None => self.advance(tic),
            },
            Event::KeyRelease(k) => match key_of(k) {
                Some(key) => self.observe_release(tic, key),
                None => self.advance(tic),
            },
            Event::KeyReleaseReporting(r) => {
                let ended = self.advance(tic)?;
                self.set_releases_reported(*r == KeyReleaseReporting::Active);
                Ok(ended)
            }
            Event::Focus(FocusChange::Lost) => {
                let mut ended = self.advance(tic)?;
                ended.extend(self.release_all(ReleaseCause::FocusLost));
                Ok(ended)
            }
            _ => self.advance(tic),
        }
    }

    /// Whether `key` is held at the current tic.
    pub fn is_held(&self, key: K) -> bool {
        self.hold(key).is_some()
    }

    /// `key`'s current hold, if it is held at the current tic.
    pub fn hold(&self, key: K) -> Option<Hold> {
        self.holds.iter().find(|(k, _)| *k == key).map(|&(_, h)| h)
    }

    /// The keys held at the current tic, in start order.
    pub fn held(&self) -> impl Iterator<Item = (K, Hold)> + '_ {
        self.holds.iter().copied()
    }
}
