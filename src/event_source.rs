//! Injectable, traceable input sources (issue #131).
//!
//! The core loop normally reads events from the real controlling terminal via
//! [`crate::input::poll_event`]. That is impossible to drive deterministically in a
//! test and impossible to replay. This module introduces one small seam — the
//! [`EventSource`] trait — between *where events come from* and the consumer that
//! polls them, plus the two things that seam unlocks:
//!
//! * **Injection.** [`ScriptedSource`] yields a programmed sequence of events with
//!   explicit virtual timing, so a test (or a replay) drives the exact same input a
//!   real terminal would, with no terminal at all.
//! * **Capture / replay.** [`Recorder`] wraps *any* source — including the real
//!   terminal — and records every delivered event into an [`EventTrace`]. A trace
//!   round-trips through [`EventTrace::replay`] back into a [`ScriptedSource`] that
//!   reproduces the exact event sequence.
//!
//! ## The four lifecycle stages
//!
//! The roadmap asks that a trace *distinguish* an event that was **received**,
//! **normalized**, **queued** and **delivered**. The honest seam this slice owns is
//! the first and the last, made observable and conserved:
//!
//! * **received** — a raw datum arrived ([`EventTrace::received`]). Injection models
//!   this with [`RawEvent`]: a [`RawEvent::Typed`] datum carries an event the UI can
//!   dispatch; a [`RawEvent::Unrecognized`] datum is an escape sequence we do not
//!   model.
//! * **normalized** — a received datum that resolved to a typed [`Event`]. An
//!   `Unrecognized` datum is received but normalizes to nothing and is **dropped**
//!   ([`EventTrace::dropped`]), never delivered — this is exactly the
//!   "parser receipt is not UI dispatch" distinction.
//! * **delivered** — handed to the consumer ([`EventTrace::delivered`]).
//!
//! The conservation law `received == delivered + dropped` holds on every trace
//! ([`EventTrace::conserves`]); a test that watches a dropped `Unrecognized` datum
//! never reach the consumer is the falsifier. A finer **queued**-depth receipt
//! (backpressure / coalescing between normalize and deliver) is deliberately left to
//! the input-pipeline work in issue #126; this module does not fake one.
//!
//! Every type here is pure data with no clock and no global state: a `ScriptedSource`
//! advances a *virtual* clock from the timeouts its consumer passes, so replays are
//! bit-for-bit reproducible regardless of wall-clock speed.

use crate::input::Event;
use std::collections::VecDeque;
use std::io;
use std::time::Duration;

/// A pollable source of input [`Event`]s.
///
/// The real terminal ([`TerminalSource`]), a scripted test ([`ScriptedSource`]) and a
/// recording wrapper ([`Recorder`]) are all sources; a consumer polls one without
/// caring which. The signature mirrors [`crate::input::poll_event`]: poll for up to
/// `timeout`, returning `Ok(Some(event))` if one arrived, `Ok(None)` on timeout, or
/// the underlying I/O error. The trait is object-safe, so `Box<dyn EventSource>` works
/// for erasing the concrete source behind a boundary.
pub trait EventSource {
    /// Polls for the next event, waiting at most `timeout`.
    fn poll_event(&mut self, timeout: Duration) -> io::Result<Option<Event>>;
}

/// The real controlling terminal: delegates straight to [`crate::input::poll_event`]
/// (hangup-aware on a Unix tty — see that function). This is the default source a
/// production loop uses.
#[derive(Debug, Default, Clone, Copy)]
pub struct TerminalSource;

impl EventSource for TerminalSource {
    fn poll_event(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        crate::input::poll_event(timeout)
    }
}

/// A raw datum offered to a source *before* normalization, modelling the seam between
/// "bytes a parser received" and "a typed [`Event`] the UI can dispatch".
///
/// `#[non_exhaustive]`: future raw kinds (a distinct mouse-wire record, a focus
/// report) are expected, so a downstream `match` must carry a `_` arm and adding a
/// variant stays a non-breaking change.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RawEvent {
    /// A datum that normalizes to this typed event and is delivered.
    Typed(Event),
    /// An escape sequence we received but do not model: received, then dropped at
    /// normalization and never delivered. The payload is the opaque sequence, kept
    /// only so a trace can show *what* was dropped.
    Unrecognized(String),
}

impl From<Event> for RawEvent {
    fn from(event: Event) -> Self {
        RawEvent::Typed(event)
    }
}

/// One scripted datum plus when it becomes available, as a virtual offset from the
/// start of the session. Availability lets a script (or a replay) reproduce timing:
/// a datum is not delivered until a poll's virtual deadline reaches its
/// `available_after`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptedInput {
    /// Virtual time from session start at which this datum becomes available.
    pub available_after: Duration,
    /// The raw datum (typed, or an unrecognized sequence that will be dropped).
    pub raw: RawEvent,
}

impl ScriptedInput {
    /// A typed event available immediately (offset zero).
    pub fn now(event: Event) -> Self {
        Self {
            available_after: Duration::ZERO,
            raw: RawEvent::Typed(event),
        }
    }

    /// A typed event available at `at`.
    pub fn at(at: Duration, event: Event) -> Self {
        Self {
            available_after: at,
            raw: RawEvent::Typed(event),
        }
    }
}

/// A deterministic injected [`EventSource`]: it yields scripted raw data as a virtual
/// clock — advanced by the timeouts its consumer passes — reaches each datum's
/// availability. Unrecognized data is received but dropped at normalization, never
/// delivered.
///
/// Construct from already-typed events with [`ScriptedSource::from_events`] (each
/// available immediately, one delivered per poll) or from explicitly-timed raw data
/// with [`ScriptedSource::from_inputs`]. Call [`ScriptedSource::recording`] to capture
/// an [`EventTrace`] of the lifecycle as it plays.
#[derive(Debug, Clone, Default)]
pub struct ScriptedSource {
    /// Pending data in availability order (ascending `available_after`).
    pending: VecDeque<ScriptedInput>,
    /// Virtual time already consumed by prior polls.
    elapsed: Duration,
    /// Optional lifecycle capture.
    trace: Option<EventTrace>,
}

impl ScriptedSource {
    /// Injects already-typed events, each available immediately. Consecutive polls
    /// deliver them one at a time, in order.
    pub fn from_events<I>(events: I) -> Self
    where
        I: IntoIterator<Item = Event>,
    {
        Self::from_inputs(events.into_iter().map(ScriptedInput::now))
    }

    /// Injects raw data with explicit availability. Inputs are sorted by
    /// `available_after` (stably, so equal offsets keep their given order), which is
    /// also exactly the shape [`EventTrace::replay`] produces.
    pub fn from_inputs<I>(inputs: I) -> Self
    where
        I: IntoIterator<Item = ScriptedInput>,
    {
        let mut pending: Vec<ScriptedInput> = inputs.into_iter().collect();
        pending.sort_by_key(|i| i.available_after);
        Self {
            pending: pending.into(),
            elapsed: Duration::ZERO,
            trace: None,
        }
    }

    /// Enables lifecycle tracing: the returned source records received / delivered /
    /// dropped counts and the ordered delivered events into an [`EventTrace`]
    /// retrievable with [`ScriptedSource::trace`] / [`ScriptedSource::into_trace`].
    pub fn recording(mut self) -> Self {
        self.trace = Some(EventTrace::default());
        self
    }

    /// The lifecycle trace, if recording was enabled.
    pub fn trace(&self) -> Option<&EventTrace> {
        self.trace.as_ref()
    }

    /// Consumes the source, returning the lifecycle trace if recording was enabled.
    pub fn into_trace(self) -> Option<EventTrace> {
        self.trace
    }

    /// `true` once every scripted datum has been consumed.
    pub fn is_exhausted(&self) -> bool {
        self.pending.is_empty()
    }
}

impl EventSource for ScriptedSource {
    fn poll_event(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        let deadline = self.elapsed.saturating_add(timeout);
        loop {
            match self.pending.front() {
                // Nothing left, or the next datum is not yet due within this poll's
                // deadline: consume the whole timeout and report a quiet poll.
                None => {
                    self.elapsed = deadline;
                    return Ok(None);
                }
                Some(front) if front.available_after > deadline => {
                    self.elapsed = deadline;
                    return Ok(None);
                }
                Some(_) => {}
            }
            // The front datum is due. Advance virtual time to its availability
            // (never backwards), then normalize it.
            let input = self.pending.pop_front().expect("front checked above");
            self.elapsed = input.available_after.max(self.elapsed);
            if let Some(trace) = self.trace.as_mut() {
                trace.received += 1;
            }
            match input.raw {
                RawEvent::Typed(event) => {
                    if let Some(trace) = self.trace.as_mut() {
                        trace.entries.push(TraceEntry {
                            offset: self.elapsed,
                            event: event.clone(),
                        });
                    }
                    return Ok(Some(event));
                }
                RawEvent::Unrecognized(_) => {
                    // Received but normalizes to nothing: drop it and keep scanning
                    // for a deliverable datum still within this poll's deadline.
                    if let Some(trace) = self.trace.as_mut() {
                        trace.dropped += 1;
                    }
                    continue;
                }
            }
        }
    }
}

/// An [`EventSource`] wrapper that records every delivered event into an
/// [`EventTrace`] as it polls through to an inner source.
///
/// Unlike [`ScriptedSource`]'s trace — which sees the raw lifecycle including drops —
/// a recorder observes only the *delivered* stream of whatever it wraps, so it is the
/// way to capture a **real** [`TerminalSource`] session for later replay. Its trace
/// reports `dropped == 0` and `received == delivered`, because an inner source has
/// already dropped anything it did not deliver.
///
/// Recorded offsets are the accumulated poll timeout budget before each delivery — a
/// best-effort pacing hint, not a wall-clock timestamp. Event **order** is exact, and
/// so is a replay of it.
#[derive(Debug, Clone, Default)]
pub struct Recorder<S> {
    inner: S,
    elapsed: Duration,
    trace: EventTrace,
}

impl<S: EventSource> Recorder<S> {
    /// Wraps `inner`, recording from a zeroed virtual clock.
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            elapsed: Duration::ZERO,
            trace: EventTrace::default(),
        }
    }

    /// The trace recorded so far.
    pub fn trace(&self) -> &EventTrace {
        &self.trace
    }

    /// Consumes the recorder, returning the recorded trace.
    pub fn into_trace(self) -> EventTrace {
        self.trace
    }

    /// Consumes the recorder, returning the wrapped source.
    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl<S: EventSource> EventSource for Recorder<S> {
    fn poll_event(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        let offset = self.elapsed;
        self.elapsed = self.elapsed.saturating_add(timeout);
        let polled = self.inner.poll_event(timeout)?;
        if let Some(event) = &polled {
            self.trace.received += 1;
            self.trace.entries.push(TraceEntry {
                offset,
                event: event.clone(),
            });
        }
        Ok(polled)
    }
}

/// One delivered event in an [`EventTrace`], with the virtual offset at which it was
/// delivered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEntry {
    /// Virtual time from session start at which this event was delivered.
    pub offset: Duration,
    /// The delivered event.
    pub event: Event,
}

/// A recorded capture of an input session: the ordered delivered events plus the
/// received / dropped tallies that make the lifecycle observable.
///
/// A trace replays into a [`ScriptedSource`] ([`EventTrace::replay`]) that reproduces
/// the exact delivered sequence, and it always satisfies the conservation law
/// `received == delivered + dropped` ([`EventTrace::conserves`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventTrace {
    /// Ordered delivered events with their virtual offsets.
    entries: Vec<TraceEntry>,
    /// Count of raw data received (delivered plus dropped).
    received: u64,
    /// Count of received data dropped at normalization (never delivered).
    dropped: u64,
}

impl EventTrace {
    /// The ordered delivered entries (event + offset).
    pub fn entries(&self) -> &[TraceEntry] {
        &self.entries
    }

    /// The delivered events in order, cloned out without their offsets.
    pub fn delivered_events(&self) -> Vec<Event> {
        self.entries.iter().map(|e| e.event.clone()).collect()
    }

    /// How many raw data were received (delivered plus dropped).
    pub fn received(&self) -> u64 {
        self.received
    }

    /// How many events were delivered.
    pub fn delivered(&self) -> u64 {
        self.entries.len() as u64
    }

    /// How many received data were dropped at normalization.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// The conservation law: every received datum was either delivered or dropped,
    /// with nothing fabricated downstream and nothing lost without a receipt.
    pub fn conserves(&self) -> bool {
        self.received == self.delivered() + self.dropped
    }

    /// Builds a [`ScriptedSource`] that reproduces this trace's delivered events at
    /// their recorded offsets. Polling the replay yields the exact same event
    /// sequence that was recorded.
    pub fn replay(&self) -> ScriptedSource {
        ScriptedSource::from_inputs(self.entries.iter().map(|e| ScriptedInput {
            available_after: e.offset,
            raw: RawEvent::Typed(e.event.clone()),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Event, KeyCode, KeyEvent, KeyModifiers};

    fn key(c: char) -> Event {
        Event::Key(KeyEvent::char(c))
    }

    /// Drives a source to exhaustion-or-quiet with a fixed poll slice, returning the
    /// delivered events in order. Bounded so a bug cannot hang the test.
    fn drain(src: &mut impl EventSource, slice: Duration, max_polls: usize) -> Vec<Event> {
        let mut out = Vec::new();
        let mut quiet = 0;
        for _ in 0..max_polls {
            match src.poll_event(slice).expect("scripted poll never errors") {
                Some(ev) => {
                    out.push(ev);
                    quiet = 0;
                }
                None => {
                    quiet += 1;
                    // Two consecutive quiet polls past any pending work: done.
                    if quiet >= 2 {
                        break;
                    }
                }
            }
        }
        out
    }

    #[test]
    fn scripted_source_yields_injected_events_in_order() {
        let mut src = ScriptedSource::from_events([key('a'), key('b'), key('c')]);
        assert_eq!(
            src.poll_event(Duration::from_millis(10)).unwrap(),
            Some(key('a'))
        );
        assert_eq!(
            src.poll_event(Duration::from_millis(10)).unwrap(),
            Some(key('b'))
        );
        assert_eq!(
            src.poll_event(Duration::from_millis(10)).unwrap(),
            Some(key('c'))
        );
        // Exhausted: a further poll is quiet, not a panic and not a repeat.
        assert_eq!(src.poll_event(Duration::from_millis(10)).unwrap(), None);
        assert!(src.is_exhausted());
    }

    #[test]
    fn empty_source_polls_none_and_consumes_the_timeout() {
        let mut src = ScriptedSource::from_events([]);
        assert_eq!(src.poll_event(Duration::from_millis(5)).unwrap(), None);
        assert!(src.is_exhausted());
    }

    #[test]
    fn scripted_timing_gates_delivery_until_the_virtual_deadline() {
        // One event available at 50ms. Polling in 10ms slices must stay quiet until
        // the accumulated virtual clock first reaches 50ms.
        let mut src =
            ScriptedSource::from_inputs([ScriptedInput::at(Duration::from_millis(50), key('x'))]);
        let slice = Duration::from_millis(10);
        // Polls 1..=4 cover virtual [0,40): not yet due.
        for _ in 0..4 {
            assert_eq!(src.poll_event(slice).unwrap(), None);
        }
        // Poll 5 reaches a deadline of 50ms >= availability: delivered.
        assert_eq!(src.poll_event(slice).unwrap(), Some(key('x')));
    }

    #[test]
    fn record_then_replay_reproduces_the_exact_event_sequence() {
        let original = vec![key('h'), key('i'), Event::Resize(80, 24), key('!')];
        let mut src = ScriptedSource::from_events(original.clone()).recording();
        let delivered = drain(&mut src, Duration::from_millis(1), 64);
        assert_eq!(delivered, original);

        let trace = src.into_trace().expect("recording was enabled");
        // The trace captured every delivered event, nothing dropped.
        assert_eq!(trace.delivered_events(), original);
        assert_eq!(trace.received(), original.len() as u64);
        assert_eq!(trace.dropped(), 0);
        assert!(trace.conserves());

        // Replay is exact AND deterministic across repeats.
        let mut r1 = trace.replay();
        let mut r2 = trace.replay();
        let replayed1 = drain(&mut r1, Duration::from_millis(1), 64);
        let replayed2 = drain(&mut r2, Duration::from_millis(1), 64);
        assert_eq!(replayed1, original);
        assert_eq!(replayed1, replayed2);
    }

    #[test]
    fn replay_depends_on_trace_content_not_a_canned_sequence() {
        // MUTATION CONTROL: a trace of a DIFFERENT sequence must replay differently,
        // proving replay reads the trace rather than echoing a fixed canned list.
        let a = {
            let mut s = ScriptedSource::from_events([key('a'), key('b')]).recording();
            drain(&mut s, Duration::from_millis(1), 16);
            s.into_trace().unwrap()
        };
        let b = {
            let mut s = ScriptedSource::from_events([key('z')]).recording();
            drain(&mut s, Duration::from_millis(1), 16);
            s.into_trace().unwrap()
        };
        assert_ne!(a, b);
        let mut ra = a.replay();
        let mut rb = b.replay();
        assert_ne!(
            drain(&mut ra, Duration::from_millis(1), 16),
            drain(&mut rb, Duration::from_millis(1), 16)
        );
    }

    #[test]
    fn unrecognized_data_is_received_but_dropped_never_delivered() {
        // received = 3, delivered = [a, b], dropped = 1 (the unrecognized escape).
        let mut src = ScriptedSource::from_inputs([
            ScriptedInput::at(Duration::from_millis(0), key('a')),
            ScriptedInput {
                available_after: Duration::from_millis(1),
                raw: RawEvent::Unrecognized("\x1b[>0;0c".into()),
            },
            ScriptedInput::at(Duration::from_millis(2), key('b')),
        ])
        .recording();
        let delivered = drain(&mut src, Duration::from_millis(1), 64);
        assert_eq!(delivered, vec![key('a'), key('b')]);

        let trace = src.into_trace().unwrap();
        assert_eq!(trace.received(), 3);
        assert_eq!(trace.delivered(), 2);
        assert_eq!(trace.dropped(), 1);
        assert!(trace.conserves(), "received must equal delivered + dropped");
    }

    #[test]
    fn conservation_without_drops_has_zero_dropped() {
        // MUTATION CONTROL for the drop path: identical but with the unrecognized
        // datum removed, dropped must be 0 and received must equal delivered.
        let mut src = ScriptedSource::from_events([key('a'), key('b')]).recording();
        drain(&mut src, Duration::from_millis(1), 64);
        let trace = src.into_trace().unwrap();
        assert_eq!(trace.received(), 2);
        assert_eq!(trace.dropped(), 0);
        assert_eq!(trace.delivered(), 2);
        assert!(trace.conserves());
    }

    #[test]
    fn recorder_captures_the_delivered_stream_of_any_source() {
        // A Recorder wrapping a ScriptedSource records exactly what is delivered and
        // replays it. This is the path that also works over a real TerminalSource.
        let events = vec![key('q'), Event::Resize(100, 40), key('w')];
        let inner = ScriptedSource::from_events(events.clone());
        let mut rec = Recorder::new(inner);
        let delivered = drain(&mut rec, Duration::from_millis(1), 64);
        assert_eq!(delivered, events);

        let trace = rec.into_trace();
        assert_eq!(trace.delivered_events(), events);
        assert_eq!(trace.received(), events.len() as u64);
        assert_eq!(trace.dropped(), 0);
        assert!(trace.conserves());

        let mut replay = trace.replay();
        assert_eq!(drain(&mut replay, Duration::from_millis(1), 64), events);
    }

    #[test]
    fn terminal_source_is_the_default_and_object_safe() {
        // Compile-time proof that the trait is object-safe and TerminalSource builds
        // (we do not poll it: there is no terminal under the test harness).
        let _boxed: Box<dyn EventSource> = Box::<TerminalSource>::default();
        let _raw: RawEvent = key('k').into();
        assert_eq!(_raw, RawEvent::Typed(key('k')));
    }

    #[test]
    fn modifiers_survive_the_round_trip() {
        // A non-trivial event (ctrl+c) records and replays intact.
        let ctrl_c = Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        let mut src = ScriptedSource::from_events([ctrl_c.clone()]).recording();
        assert_eq!(
            src.poll_event(Duration::from_millis(1)).unwrap(),
            Some(ctrl_c.clone())
        );
        let trace = src.into_trace().unwrap();
        let mut replay = trace.replay();
        assert_eq!(
            replay.poll_event(Duration::from_millis(1)).unwrap(),
            Some(ctrl_c)
        );
    }
}
