// Shared by the `field_cinema` example (`--live --interactive`) and by the Gate C/D tests via
// `#[path]`. Includers must also declare `mod cinema; mod live; mod live_link;` plus the reused
// wire_instrument modules `capture`, `trace`, `transport`, `channel`, `input`.
#![allow(dead_code)]
//! The interactive live instrument-film (RXIV Gate C) and its causal record/replay (Gate D4).
//!
//! RXIII's `--live` was a scripted six-beat demonstration: the probe was chosen by the program, the
//! target was fixed, and the "film" was printed text. [`Film`] is the interactive version: a REAL
//! source field arrives, the viewer navigates a probe, chooses a target, issues the command through
//! the certified [`LiveSession`](crate::live_link::LiveSession) channel, sees the producer's
//! correlated verdict, inspects the next authoritative field, and the composition responds to the
//! VERIFIED difference — the arc's typed effect contract (Gate A), never the ACK.
//!
//! **Separation.** `Film` is presentation-separated: it consumes session events (`on_field`,
//! `on_submitted`, `on_verdict`, `on_failure`) and keys (`on_key` → [`FilmAction`]) and renders a
//! `Node` for any terminal size ([`Film::view`]). The I/O loop lives in `main.rs`; the headless
//! oracle tests drive this same type against the real producer, so the shots and surfaces the
//! oracle judges are the ones the viewer sees.
//!
//! **Cinematic structure.** ESTABLISH → DISCOVER → INTERVENE → AWAIT → TRANSFORM → RESOLVE is
//! carried by the existing `gibson::scene` machinery: every panel is a [`SceneEntity`] placed by
//! effects; a beat change slides the narration in, an evidenced TRANSFORM jolts the field, and the
//! camera eases between the shots `cinema::compose` derives from what was observed (data-driven
//! framing, not a timeline). **Reduced motion** collapses every one of those to an instantaneous
//! cut — same information, no movement.
//!
//! **Record / replay.** Every causal event the film acts on is recorded — admitted frames (capture)
//! and observations / probes / commands-with-arguments / verdicts / outcome (trace v2) — and
//! [`replay_film`] re-derives the arc's beat path, outcome, final shot and final field surface from
//! that record alone, with no producer. Equality of live and replayed beat paths is exact semantic
//! replay; it is a reproduction of DECISIONS from recorded observations, not a re-execution of the
//! producer, and the checksums involved are deterministic receipts, not authentication.

use std::time::Duration;

use crate::capture::{content_digest, CaptureLimits, CapturedFrame};
use crate::cinema::{
    compose, observe, view_focused_on, Emphasis, FieldObservation, Shot, SourceId,
};
use crate::live::{Arc, Beat, CycleOutcome, Evidence, ObsSummary, Verdict};
use crate::live_link::{
    arc_summary, verdict_of, LiveError, LiveField, ANCHOR, KIND_SET_AMPLITUDE, LIVE_SOURCE,
};
use crate::trace::{
    Admission, CausalTrace, OutcomeKind, TraceEvent, TraceLimits, Verdict as TraceVerdict,
};
use gibson::cell::{Cell, Glyph, Line, Span};
use gibson::field_view::{Colormap, FieldView, RangePolicy, Sample};
use gibson::input::KeyCode;
use gibson::scene::{Easing, Effect, Presentation, Scene, SceneEntity, SceneTarget};
use gibson::surface::Surface;
use gibson::wire::AckStatus;
use gibson::{Node, Point, Rect, Style};

/// Target adjustment step and ceiling. The ceiling deliberately exceeds the reference producer's
/// accepted range (`[0, 10]`), so a refusal is reachable from the keyboard.
pub const TARGET_STEP: f64 = 0.5;
pub const TARGET_MAX: f64 = 12.0;
/// Motion timings (all collapse to zero under reduced motion).
pub const CAMERA_TWEEN: Duration = Duration::from_millis(520);
pub const CAPTION_SLIDE: Duration = Duration::from_millis(420);
pub const TRANSFORM_JOLT: Duration = Duration::from_millis(320);
pub const VERDICT_FLASH: Duration = Duration::from_millis(900);
/// Auto-advance cadence (one lockstep frame per period, never more than one in flight).
pub const AUTO_PERIOD: Duration = Duration::from_millis(160);

/// Presentation options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilmConfig {
    pub reduced_motion: bool,
    pub auto_advance: bool,
}

/// What the I/O loop must do in response to a key.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilmAction {
    Nothing,
    Quit,
    /// Issue a set-amplitude command for this target through the session.
    Submit(f64),
    /// Advance the lockstep producer one frame.
    Step,
}

/// The causal record: admitted frames + the semantic trace.
#[derive(Debug, Clone, Default)]
pub struct FilmRecord {
    pub frames: Vec<CapturedFrame>,
    pub trace: CausalTrace,
    /// Frames stopped being retained at the capture budget (the trace's own `truncated` marker
    /// covers events).
    pub frames_truncated: bool,
    frame_bytes: usize,
}

impl FilmRecord {
    fn push_frame(&mut self, f: CapturedFrame) {
        let lim = CaptureLimits::default();
        let bytes = f.data.len() * 8 + f.mask.as_ref().map_or(0, |m| m.len());
        if self.frames_truncated
            || self.frames.len() >= lim.max_frames
            || self.frame_bytes + bytes > lim.max_total_bytes
        {
            self.frames_truncated = true;
            return;
        }
        self.frame_bytes += bytes;
        self.frames.push(f);
    }
}

/// The interactive film.
pub struct Film {
    cfg: FilmConfig,
    arc: Arc,
    field: Option<LiveField>,
    obs: Option<FieldObservation>,
    shot: Option<Shot>,
    shot_from: Option<Shot>,
    shot_since: Duration,
    probe: Option<(usize, usize)>,
    target: f64,
    pending: Option<u64>,
    last_command: Option<(u64, f64, Admission)>,
    last_verdict: Option<(u64, AckStatus, Duration)>,
    last_evidence: Option<Evidence>,
    beat_since: Duration,
    jolt_at: Option<Duration>,
    failure: Option<String>,
    resolved: bool,
    note: String,
    beats: Vec<Beat>,
    record: FilmRecord,
    producer_pid: Option<u32>,
}

impl Film {
    pub fn new(cfg: FilmConfig) -> Self {
        Self {
            cfg,
            arc: Arc::new(),
            field: None,
            obs: None,
            shot: None,
            shot_from: None,
            shot_since: Duration::ZERO,
            probe: None,
            target: 2.0,
            pending: None,
            last_command: None,
            last_verdict: None,
            last_evidence: None,
            beat_since: Duration::ZERO,
            jolt_at: None,
            failure: None,
            resolved: false,
            note: String::new(),
            beats: Vec::new(),
            record: FilmRecord::default(),
            producer_pid: None,
        }
    }

    /// Show the producer's OS pid in the title (provenance for the viewer and for process tests).
    pub fn set_producer_pid(&mut self, pid: Option<u32>) {
        self.producer_pid = pid;
    }

    // ---- read-only state ----------------------------------------------------------------------

    pub fn beat(&self) -> Beat {
        self.arc.beat()
    }
    pub fn arc(&self) -> &Arc {
        &self.arc
    }
    pub fn auto(&self) -> bool {
        self.cfg.auto_advance && !self.resolved && self.failure.is_none()
    }
    pub fn pending(&self) -> Option<u64> {
        self.pending
    }
    pub fn resolved(&self) -> bool {
        self.resolved
    }
    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }
    pub fn target(&self) -> f64 {
        self.target
    }
    pub fn probe(&self) -> Option<(usize, usize)> {
        self.probe
    }
    pub fn shot(&self) -> Option<Shot> {
        self.shot
    }
    /// The beat after every recorded causal event — the sequence [`replay_film`] must reproduce.
    pub fn beats(&self) -> &[Beat] {
        &self.beats
    }
    pub fn record(&self) -> &FilmRecord {
        &self.record
    }

    /// The settled field surface at the scripted [`ANCHOR`] in the shot's own colormap — exactly
    /// what [`replay_film`] re-renders, and what the causal oracle compares across histories.
    pub fn field_surface(&self) -> Option<Surface> {
        let f = self.field.as_ref()?;
        let shot = self.shot?;
        render_field(f, &shot, ANCHOR, shot.colormap, self.probe_world())
    }

    fn probe_world(&self) -> Option<Point> {
        let f = self.field.as_ref()?;
        let (c, r) = self.probe?;
        f.field.extent.cell_center(c, r)
    }

    // ---- recording ----------------------------------------------------------------------------

    /// Record one causal event and the beat it left the arc in (replay pushes at the same points).
    fn log(&mut self, ev: TraceEvent) {
        self.record.trace.record(ev, &TraceLimits::default());
        self.beats.push(self.arc.beat());
    }

    fn mark_beat(&mut self, before: Beat, now: Duration) {
        let b = self.arc.beat();
        if b != before {
            self.beat_since = now;
            if b == Beat::Transform {
                self.jolt_at = Some(now);
            }
        }
    }

    // ---- session events -----------------------------------------------------------------------

    /// An admitted authoritative frame arrived.
    pub fn on_field(&mut self, f: LiveField, now: Duration) {
        if self.resolved {
            return;
        }
        let ext = f.field.extent;
        let Some(view) = FieldView::new(ext, &f.field.data, f.field.mask.as_deref()) else {
            self.note = "an admitted frame was not presentable".into();
            return;
        };
        let source = SourceId(u32::try_from(f.source).unwrap_or(u32::MAX));
        let obs = observe(&view, source, f.cursor.epoch, f.field.model_tick);
        let p = self.probe.get_or_insert((ext.cols() / 2, ext.rows() / 2));
        p.0 = p.0.min(ext.cols().saturating_sub(1));
        p.1 = p.1.min(ext.rows().saturating_sub(1));

        let frame = CapturedFrame::from_field(f.source, f.cursor, &f.field);
        let field_checksum = content_digest(std::slice::from_ref(&frame));
        self.record.push_frame(frame);

        let before = self.arc.beat();
        if let Some(ev) = self.arc.observe(arc_summary(&obs, f.cursor)) {
            self.last_evidence = Some(ev);
        }
        self.log(TraceEvent::Observation {
            source: f.source,
            epoch: f.cursor.epoch,
            sequence: f.cursor.sequence,
            model_tick: f.field.model_tick,
            field_checksum,
        });
        self.mark_beat(before, now);
        self.field = Some(f);
        self.obs = Some(obs);
        self.recompose(now);
    }

    /// The command for the current target was handed to the session. `admission` is
    /// `Accepted` (delivered) or `TransportUncertain` (whole frame written, flush failed — a genuine
    /// ACK may still reconcile it).
    pub fn on_submitted(&mut self, command_id: u64, admission: Admission, now: Duration) {
        let before = self.arc.beat();
        let armed = self.arc.intervene(self.target);
        self.last_evidence = Some(armed);
        self.log(TraceEvent::Command {
            command_id,
            dest_source: LIVE_SOURCE,
            kind: KIND_SET_AMPLITUDE,
            arg: self.target.to_le_bytes().to_vec(),
            admission,
        });
        self.mark_beat(before, now);
        self.pending = Some(command_id);
        self.last_command = Some((command_id, self.target, admission));
        self.note.clear();
    }

    /// The session refused to issue (or failed to deliver) the command. Recorded with its true
    /// admission; the arc is untouched. A terminal session error also fails the film.
    pub fn on_submit_failed(&mut self, err: &LiveError, now: Duration) {
        let (command_id, admission) = match err {
            LiveError::Local(_) => (0, Admission::Rejected),
            LiveError::Transport {
                command_id,
                uncertain: false,
                ..
            } => (command_id.unwrap_or(0), Admission::TransportFailed),
            _ => (0, Admission::Rejected),
        };
        self.log(TraceEvent::Command {
            command_id,
            dest_source: LIVE_SOURCE,
            kind: KIND_SET_AMPLITUDE,
            arg: self.target.to_le_bytes().to_vec(),
            admission,
        });
        self.note = format!("not sent: {err}");
        if !matches!(err, LiveError::Local(_)) {
            self.on_failure(err, now);
        }
    }

    /// The producer's verdict, already correlated to `command_id` by the session's ledger.
    pub fn on_verdict(&mut self, command_id: u64, status: AckStatus, now: Duration) {
        if self.pending != Some(command_id) {
            return;
        }
        let before = self.arc.beat();
        self.arc.verdict(verdict_of(status));
        self.log(TraceEvent::Verdict {
            command_id,
            verdict: match status {
                AckStatus::Acknowledged => TraceVerdict::Acknowledged,
                AckStatus::Refused => TraceVerdict::Refused,
                _ => TraceVerdict::Failed,
            },
        });
        self.mark_beat(before, now);
        self.pending = None;
        self.last_verdict = Some((command_id, status, now));
    }

    /// The session failed (typed). The film resolves, records the outcome, and shows the reason
    /// until the viewer quits.
    pub fn on_failure(&mut self, err: &LiveError, now: Duration) {
        if self.failure.is_some() {
            return;
        }
        self.failure = Some(err.to_string());
        if self.resolved {
            return;
        }
        let before = self.arc.beat();
        self.arc.resolve();
        let kind = match err {
            LiveError::Disconnected { .. } | LiveError::Retired { .. } => OutcomeKind::Disconnected,
            _ => OutcomeKind::FatalTransport,
        };
        self.log(TraceEvent::Outcome {
            kind,
            pending_commands: u32::from(self.pending.is_some()),
        });
        self.mark_beat(before, now);
        self.resolved = true;
    }

    /// Close the film on quit (if not already resolved by `r` or a failure).
    pub fn finish(&mut self, now: Duration) {
        if !self.resolved {
            self.resolve(now);
        }
    }

    fn resolve(&mut self, now: Duration) {
        let before = self.arc.beat();
        self.arc.resolve();
        self.log(TraceEvent::Outcome {
            kind: OutcomeKind::Quit,
            pending_commands: u32::from(self.pending.is_some()),
        });
        self.mark_beat(before, now);
        self.resolved = true;
    }

    // ---- input --------------------------------------------------------------------------------

    /// Reduce a key to film state and an I/O action. Precedence: quit > a latched failure (nothing
    /// else) > navigation/intervention.
    pub fn on_key(&mut self, code: KeyCode, now: Duration) -> FilmAction {
        if matches!(code, KeyCode::Char('q') | KeyCode::Esc) {
            return FilmAction::Quit;
        }
        if self.failure.is_some() {
            return FilmAction::Nothing;
        }
        match code {
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
                self.move_probe(code, now);
                FilmAction::Nothing
            }
            KeyCode::Char('[') => {
                self.target = ((self.target - TARGET_STEP).max(0.0) * 2.0).round() / 2.0;
                FilmAction::Nothing
            }
            KeyCode::Char(']') => {
                self.target = ((self.target + TARGET_STEP).min(TARGET_MAX) * 2.0).round() / 2.0;
                FilmAction::Nothing
            }
            KeyCode::Char('a') => {
                self.cfg.auto_advance = !self.cfg.auto_advance;
                FilmAction::Nothing
            }
            KeyCode::Char('r') if !self.resolved => {
                self.resolve(now);
                FilmAction::Nothing
            }
            KeyCode::Enter => {
                if self.resolved || self.field.is_none() {
                    FilmAction::Nothing
                } else if self.pending.is_some() {
                    self.note = "a command is already in flight".into();
                    FilmAction::Nothing
                } else if matches!(self.arc.beat(), Beat::Await) {
                    self.note = "still awaiting evidence — step to observe".into();
                    FilmAction::Nothing
                } else {
                    FilmAction::Submit(self.target)
                }
            }
            KeyCode::Char(' ') if !self.resolved && self.pending.is_none() => FilmAction::Step,
            _ => FilmAction::Nothing,
        }
    }

    fn move_probe(&mut self, code: KeyCode, now: Duration) {
        let Some(f) = &self.field else { return };
        let (cols, rows) = (f.field.extent.cols(), f.field.extent.rows());
        let Some((c, r)) = self.probe else { return };
        let (nc, nr) = match code {
            KeyCode::Left => (c.saturating_sub(1), r),
            KeyCode::Right => ((c + 1).min(cols.saturating_sub(1)), r),
            KeyCode::Up => (c, r.saturating_sub(1)),
            KeyCode::Down => (c, (r + 1).min(rows.saturating_sub(1))),
            _ => (c, r),
        };
        if (nc, nr) == (c, r) {
            return;
        }
        self.probe = Some((nc, nr));
        if !self.resolved {
            // Post-resolve inspection is NOT part of the causal record.
            let before = self.arc.beat();
            self.arc.discover();
            if let Some(w) = self.probe_world() {
                self.log(TraceEvent::Probe {
                    world_x: w.x,
                    world_y: w.y,
                });
            }
            self.mark_beat(before, now);
        }
        self.recompose(now);
    }

    /// Re-derive the shot from the current observation and probe; start a camera move if it
    /// changed.
    fn recompose(&mut self, now: Duration) {
        let (Some(f), Some(obs)) = (&self.field, &self.obs) else {
            return;
        };
        let Some(view) = FieldView::new(f.field.extent, &f.field.data, f.field.mask.as_deref())
        else {
            return;
        };
        let world = self
            .probe_world()
            .unwrap_or_else(|| crate::cinema::field_center(f.field.extent));
        let shot = compose(obs, &view.probe(world), f.field.extent);
        match self.shot {
            Some(old) if same_framing(&old, &shot) => self.shot = Some(shot),
            Some(_) => {
                self.shot_from = self.camera(now);
                self.shot_since = now;
                self.shot = Some(shot);
            }
            None => {
                self.shot = Some(shot);
                self.shot_from = None;
                self.shot_since = now;
            }
        }
    }

    /// The camera at `now`: an eased move from the previous shot to the current one (an instant
    /// cut under reduced motion).
    fn camera(&self, now: Duration) -> Option<Shot> {
        let to = self.shot?;
        let Some(from) = self.shot_from else {
            return Some(to);
        };
        if self.cfg.reduced_motion {
            return Some(to);
        }
        let k = (now.saturating_sub(self.shot_since).as_secs_f64() / CAMERA_TWEEN.as_secs_f64())
            .clamp(0.0, 1.0);
        let e = ease_in_out(k);
        Some(Shot {
            focus: Point::new(
                from.focus.x + (to.focus.x - from.focus.x) * e,
                from.focus.y + (to.focus.y - from.focus.y) * e,
            ),
            zoom: from.zoom + (to.zoom - from.zoom) * e,
            ..to
        })
    }

    // ---- view ---------------------------------------------------------------------------------

    /// Compose the frame for a `w × h` terminal. `mono` selects the colour-free density ramp for
    /// the field (a monochrome terminal cannot show the heat ramp's background colours).
    pub fn view(&self, now: Duration, w: u16, h: u16, mono: bool) -> Node {
        let compact = w < 64 || h < 20;
        let mut scene = Scene::new();
        let mut fx: Vec<Effect> = Vec::new();
        let place = |fx: &mut Vec<Effect>, id, x: f32, y: f32| {
            fx.push(Effect::translate(
                SceneTarget::Id(id),
                (x, y),
                (x, y),
                Duration::ZERO,
            ));
        };
        let motion = !self.cfg.reduced_motion;

        let (field_w, field_h, panel_x) = if compact {
            (w.max(8), h.saturating_sub(6).max(3), 0)
        } else {
            let fw = w.saturating_sub(36).clamp(24, 72);
            let fh = h.saturating_sub(7).clamp(8, 28);
            (fw, fh, fw + 2)
        };
        let anchor = Rect::new(0, 0, field_w, field_h);

        // Title / header.
        let header = if compact {
            format!(
                "{} · t{} · {}",
                beat_name(self.arc.beat()),
                self.obs.map_or(0, |o| o.tick),
                self.verdict_brief()
            )
        } else {
            format!(
                "LIVE FIELD CINEMA · source {} · epoch {} · tick {} · pid {} · motion {} · {}",
                LIVE_SOURCE,
                self.field.as_ref().map_or(0, |f| f.cursor.epoch),
                self.obs.map_or(0, |o| o.tick),
                self.producer_pid.unwrap_or(0),
                if motion { "full" } else { "reduced" },
                if self.auto() { "auto" } else { "step" },
            )
        };
        let title = scene.add(SceneEntity::new(
            "title",
            Node::text(clip(&header, w as usize), Style::new().bold()),
        ));
        place(&mut fx, title, 0.0, 0.0);

        let mut y = 1.0;
        if !compact {
            let strip = scene.add(SceneEntity::new("beats", Node::line(self.beat_strip(now))));
            place(&mut fx, strip, 0.0, 1.0);
            y = 3.0;
        }

        // The field.
        let colormap_override = if mono { Some(Colormap::Density) } else { None };
        if let (Some(f), Some(cam)) = (&self.field, self.camera(now)) {
            let cm = colormap_override.unwrap_or(cam.colormap);
            if let Some(surface) = render_field(f, &cam, anchor, cm, self.probe_world()) {
                let fid = scene.add(SceneEntity::new("field", Node::raster(surface)));
                place(&mut fx, fid, 0.0, y);
                if motion {
                    if let Some(j) = self.jolt_at {
                        if now.saturating_sub(j) < TRANSFORM_JOLT {
                            fx.push(Effect::jitter(
                                SceneTarget::Id(fid),
                                1.0,
                                Duration::from_millis(60),
                                TRANSFORM_JOLT,
                            ));
                        }
                    }
                }
            }
        } else {
            let wait = scene.add(SceneEntity::new(
                "field",
                Node::text(
                    "awaiting the first authoritative field…",
                    Style::new().dim(),
                ),
            ));
            place(&mut fx, wait, 0.0, y);
        }

        // The readout panel.
        let lines = if compact {
            self.compact_lines(w as usize)
        } else {
            self.panel_lines(now)
        };
        let (px, py) = if compact {
            (0.0, y + field_h as f32)
        } else {
            (panel_x as f32, y)
        };
        let mut panel = Node::col();
        for (text, style) in lines {
            let max = if compact {
                w as usize
            } else {
                w.saturating_sub(panel_x) as usize
            };
            panel = panel.child(Node::text(clip(&text, max), style));
        }
        let pid = scene.add(SceneEntity::new("panel", panel));
        place(&mut fx, pid, px, py);

        // Narration (wide layout): slides in on every beat change.
        if !compact {
            let (caption, style) = self.caption(now);
            let cy = (h.saturating_sub(3)) as f32;
            let cid = scene.add(SceneEntity::new(
                "caption",
                Node::text(clip(&caption, w as usize), style),
            ));
            place(&mut fx, cid, 0.0, cy);
            if motion && now.saturating_sub(self.beat_since) < CAPTION_SLIDE {
                fx.push(Effect::Delay(
                    self.beat_since,
                    Box::new(
                        Effect::translate(
                            SceneTarget::Id(cid),
                            (-24.0, cy),
                            (0.0, cy),
                            CAPTION_SLIDE,
                        )
                        .eased(Easing::EaseOut),
                    ),
                ));
            }
            let help = scene.add(SceneEntity::new(
                "help",
                Node::text(
                    clip(
                        "arrows probe · [ ] target · enter intervene · space step · a auto · r resolve · q quit",
                        w as usize,
                    ),
                    Style::new().dim(),
                ),
            ));
            place(&mut fx, help, 0.0, (h.saturating_sub(1)) as f32);
        }

        let mut p = Presentation::new();
        Effect::parallel(fx).eval(now, &scene, &mut p);
        scene.to_node(&p, w as f32, h as f32)
    }

    fn beat_strip(&self, now: Duration) -> Line {
        let order = [
            Beat::Establish,
            Beat::Discover,
            Beat::Intervene,
            Beat::Await,
            Beat::Transform,
            Beat::Resolve,
        ];
        let cur = order
            .iter()
            .position(|b| *b == self.arc.beat())
            .unwrap_or(0);
        let pulse = !self.cfg.reduced_motion && now.saturating_sub(self.beat_since) < CAPTION_SLIDE;
        let mut spans = Vec::new();
        for (i, b) in order.iter().enumerate() {
            let style = if i == cur {
                let s = Style::new().bold().reverse();
                if pulse {
                    s.underline()
                } else {
                    s
                }
            } else if i < cur {
                Style::new().dim()
            } else {
                Style::new()
            };
            spans.push(Span::styled(format!(" {} ", beat_name(*b)), style));
            if i + 1 < order.len() {
                spans.push(Span::styled("▸", Style::new().dim()));
            }
        }
        Line::from(spans)
    }

    fn probe_text(&self) -> String {
        let (Some(f), Some((c, r))) = (&self.field, self.probe) else {
            return "probe   —".into();
        };
        let Some(view) = FieldView::new(f.field.extent, &f.field.data, f.field.mask.as_deref())
        else {
            return "probe   —".into();
        };
        let reading = match view.sample(c, r) {
            Some(Sample::Value(v)) => format!("{v:.3}"),
            Some(Sample::Masked) => "masked".into(),
            Some(Sample::NonFinite) => "non-finite".into(),
            None => "off-field".into(),
        };
        format!("probe   ({c},{r}) = {reading}")
    }

    fn verdict_brief(&self) -> String {
        match (&self.last_command, &self.last_verdict, self.pending) {
            (_, _, Some(id)) => format!("cmd #{id} in flight"),
            (_, Some((id, st, _)), None) => format!("cmd #{id} {st:?}"),
            _ => "no command".into(),
        }
    }

    fn evidence_text(&self) -> (String, String) {
        let Some(ev) = self.last_evidence else {
            return ("evidence —".into(), String::new());
        };
        let detail = match self.arc.effect() {
            Some(e) => format!(
                "  base {:.3} → want {:.3} ±{:.3}",
                e.baseline_value, e.target, e.tolerance
            ),
            None => match self.arc.last_outcome() {
                Some(CycleOutcome::Transformed { observed }) => {
                    format!("  observed {observed:.3} (consistent)")
                }
                Some(CycleOutcome::Unevidenced { last }) => format!("  closed: {}", last.label()),
                Some(CycleOutcome::Refused) => "  refused: nothing armed".into(),
                Some(CycleOutcome::Void { reason }) => format!("  void: {}", reason.label()),
                None => String::new(),
            },
        };
        (format!("evidence {}", ev.label()), detail)
    }

    fn panel_lines(&self, now: Duration) -> Vec<(String, Style)> {
        let mut v = Vec::new();
        v.push((self.probe_text(), Style::new()));
        v.push((
            format!("target  amplitude {:.1}   [ ] ±{TARGET_STEP}", self.target),
            Style::new(),
        ));
        let cmd = match self.last_command {
            Some((id, t, adm)) => format!("command #{id} set-amp {t:.1} ({adm:?})"),
            None => "command —".into(),
        };
        v.push((cmd, Style::new()));
        let flash = matches!(self.last_verdict, Some((_, _, at)) if now.saturating_sub(at) < VERDICT_FLASH)
            && !self.cfg.reduced_motion;
        let verdict = match (self.pending, &self.last_verdict) {
            (Some(id), _) => format!("verdict awaiting #{id}…"),
            (None, Some((id, st, _))) => format!("verdict {st:?} (#{id})"),
            _ => "verdict —".into(),
        };
        v.push((
            verdict,
            if flash {
                Style::new().bold()
            } else {
                Style::new()
            },
        ));
        let (e1, e2) = self.evidence_text();
        v.push((e1, Style::new().bold()));
        if !e2.is_empty() {
            v.push((e2, Style::new()));
        }
        let (n, drift) = self.arc.calibration();
        v.push((
            format!("calib   {n} obs · drift {drift:.4}"),
            Style::new().dim(),
        ));
        if let Some(s) = self.shot {
            v.push((
                format!(
                    "shot    zoom {:.2} {} {}",
                    s.zoom,
                    match s.emphasis {
                        Emphasis::PunchIn => "punch-in",
                        Emphasis::Wide => "wide",
                    },
                    match s.colormap {
                        Colormap::Heat => "heat",
                        Colormap::Density => "density",
                        _ => "other",
                    }
                ),
                Style::new().dim(),
            ));
        }
        v.push((
            format!("transforms {}", self.arc.transforms()),
            Style::new().dim(),
        ));
        if !self.note.is_empty() {
            v.push((self.note.clone(), Style::new().italic()));
        }
        v
    }

    fn compact_lines(&self, w: usize) -> Vec<(String, Style)> {
        let (e1, _) = self.evidence_text();
        let mut v = vec![
            (self.probe_text(), Style::new()),
            (
                format!("target {:.1} · {}", self.target, self.verdict_brief()),
                Style::new(),
            ),
            (e1, Style::new().bold()),
        ];
        let last = match &self.failure {
            Some(f) => (format!("FAILED: {f} — q"), Style::new().bold().reverse()),
            None if !self.note.is_empty() => (self.note.clone(), Style::new().italic()),
            None => (
                if w >= 40 {
                    "arrows [ ] enter spc a r q".into()
                } else {
                    "keys: arrows [ ] ⏎ ␣ r q".into()
                },
                Style::new().dim(),
            ),
        };
        v.push(last);
        v
    }

    fn caption(&self, now: Duration) -> (String, Style) {
        if let Some(f) = &self.failure {
            return (
                format!("PRODUCER FAILURE — {f} — q to leave"),
                Style::new().bold().reverse(),
            );
        }
        let dots = if self.cfg.reduced_motion {
            "…".to_string()
        } else {
            ".".repeat(1 + (now.as_millis() / 300 % 3) as usize)
        };
        let peak = self.obs.and_then(|o| o.peak_value).unwrap_or(f64::NAN);
        let text = match self.arc.beat() {
            Beat::Establish => format!(
                "The source speaks: an authoritative field, peak {peak:.3}. Probe it with the arrows."
            ),
            Beat::Discover => match self.arc.last_outcome() {
                Some(CycleOutcome::Refused) => {
                    "Refused by the producer — the field is untouched. Choose another target.".into()
                }
                Some(CycleOutcome::Unevidenced { last }) => format!(
                    "Acknowledged, but {}: no consistent change, so no transformation.",
                    last.label()
                ),
                Some(CycleOutcome::Void { reason }) => {
                    format!("The baseline was voided ({}).", reason.label())
                }
                _ => {
                    let (n, d) = self.arc.calibration();
                    format!(
                        "Discovering: natural variation {d:.4} over {n} observations. Set a target, then Enter."
                    )
                }
            },
            Beat::Intervene => format!(
                "Request: amplitude → {:.1}. Command sent; awaiting the producer's verdict{dots}",
                self.target
            ),
            Beat::Await => format!("Acknowledged. Awaiting authoritative evidence{dots} (space steps)"),
            Beat::Transform => match self.arc.last_outcome() {
                Some(CycleOutcome::Transformed { observed }) => format!(
                    "Evidence: the field now peaks at {observed:.3}, consistent with the request. (Not a proof of mechanism.)"
                ),
                _ => "Transformed.".into(),
            },
            Beat::Resolve => format!(
                "Resolved — {} evidenced transformation(s). Inspect freely; q to leave.",
                self.arc.transforms()
            ),
        };
        (text, Style::new())
    }
}

fn same_framing(a: &Shot, b: &Shot) -> bool {
    a.focus == b.focus && a.zoom == b.zoom && a.colormap == b.colormap
}

fn ease_in_out(k: f64) -> f64 {
    if k < 0.5 {
        2.0 * k * k
    } else {
        1.0 - (-2.0 * k + 2.0).powi(2) / 2.0
    }
}

/// Short beat label.
pub fn beat_name(b: Beat) -> &'static str {
    match b {
        Beat::Establish => "ESTABLISH",
        Beat::Discover => "DISCOVER",
        Beat::Intervene => "INTERVENE",
        Beat::Await => "AWAIT",
        Beat::Transform => "TRANSFORM",
        Beat::Resolve => "RESOLVE",
    }
}

/// Truncate to at most `max` display columns on grapheme boundaries (never splitting a cluster).
fn clip(s: &str, max: usize) -> String {
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;
    let mut out = String::new();
    let mut used = 0usize;
    for g in s.graphemes(true) {
        let w = g.width();
        if used + w > max {
            break;
        }
        used += w;
        out.push_str(g);
    }
    out
}

/// Render the field through a shot's framing into `anchor`, marking the probe cell.
fn render_field(
    f: &LiveField,
    shot: &Shot,
    anchor: Rect,
    colormap: Colormap,
    probe: Option<Point>,
) -> Option<Surface> {
    let view = FieldView::new(f.field.extent, &f.field.data, f.field.mask.as_deref())?;
    let wv = view_focused_on(shot.focus, shot.zoom, anchor);
    let mut s = view
        .render(&wv, anchor, colormap, RangePolicy::Auto)
        .surface;
    if let Some(p) = probe {
        if let Some(&(cx, cy)) = wv.world_to_cell(p.x, p.y).value() {
            if cx < s.width && cy < s.height {
                s.set_cell(
                    cx,
                    cy,
                    // 'X' is outside the density ramp (` .:-=+*#%@`), so the marker is never
                    // mistaken for a sample in a monochrome frame.
                    Cell::new(Glyph::from_char('X'), Style::new().bold().reverse()),
                );
            }
        }
    }
    Some(s)
}

// ---------------------------------------------------------------------------------------------
// Replay
// ---------------------------------------------------------------------------------------------

/// What [`replay_film`] re-derived from a record alone.
#[derive(Debug, Clone)]
pub struct FilmReplay {
    pub beats: Vec<Beat>,
    pub transforms: u32,
    pub last_outcome: Option<CycleOutcome>,
    pub final_shot: Option<Shot>,
    pub final_surface: Option<Surface>,
}

/// Why a record could not be replayed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    /// An Observation event has no corresponding captured frame.
    FrameMissing { event: usize },
    /// The captured frame's identity disagrees with the Observation event.
    FrameMismatch { event: usize },
    /// The captured frame's content checksum disagrees with the Observation event.
    ChecksumMismatch { event: usize },
    /// A captured frame's geometry is not presentable.
    Unpresentable { event: usize },
    /// An admitted command's argument is not an 8-byte f64.
    BadArgument { event: usize },
    /// Frames remain after the last Observation (the record is inconsistent).
    UnusedFrames { count: usize },
}

/// Re-derive the film's decisions from its record, with no producer: every Observation is matched
/// to its captured frame by identity AND content checksum, then observed exactly as live; probes,
/// admitted commands (with their recorded argument), verdicts and the outcome drive the same arc.
pub fn replay_film(
    frames: &[CapturedFrame],
    trace: &CausalTrace,
) -> Result<FilmReplay, ReplayError> {
    let mut arc = Arc::new();
    let mut beats = Vec::with_capacity(trace.len());
    let mut next = 0usize;
    let mut last_frame: Option<usize> = None;
    let mut probe: Option<Point> = None;
    for (i, ev) in trace.events().iter().enumerate() {
        match ev {
            TraceEvent::Observation {
                source,
                epoch,
                sequence,
                model_tick,
                field_checksum,
            } => {
                let fr = frames
                    .get(next)
                    .ok_or(ReplayError::FrameMissing { event: i })?;
                if (fr.source, fr.epoch, fr.sequence, fr.model_tick)
                    != (*source, *epoch, *sequence, *model_tick)
                {
                    return Err(ReplayError::FrameMismatch { event: i });
                }
                if content_digest(std::slice::from_ref(fr)) != *field_checksum {
                    return Err(ReplayError::ChecksumMismatch { event: i });
                }
                let ext = fr.extent().ok_or(ReplayError::Unpresentable { event: i })?;
                let view = FieldView::new(ext, &fr.data, fr.mask.as_deref())
                    .ok_or(ReplayError::Unpresentable { event: i })?;
                let o = observe(
                    &view,
                    SourceId(u32::try_from(fr.source).unwrap_or(u32::MAX)),
                    fr.epoch,
                    fr.model_tick,
                );
                arc.observe(ObsSummary::new(
                    fr.epoch,
                    fr.sequence,
                    fr.model_tick,
                    o.peak_value,
                ));
                last_frame = Some(next);
                next += 1;
            }
            TraceEvent::Probe { world_x, world_y } => {
                arc.discover();
                probe = Some(Point::new(*world_x, *world_y));
            }
            TraceEvent::Focus { .. } => {}
            TraceEvent::Command { arg, admission, .. } => {
                if matches!(
                    admission,
                    Admission::Accepted | Admission::TransportUncertain
                ) {
                    let bytes: [u8; 8] = arg
                        .as_slice()
                        .try_into()
                        .map_err(|_| ReplayError::BadArgument { event: i })?;
                    arc.intervene(f64::from_le_bytes(bytes));
                }
            }
            TraceEvent::Verdict { verdict, .. } => arc.verdict(match verdict {
                TraceVerdict::Acknowledged => Verdict::Acknowledged,
                TraceVerdict::Refused | TraceVerdict::Failed => Verdict::Refused,
            }),
            TraceEvent::Outcome { .. } => arc.resolve(),
        }
        beats.push(arc.beat());
    }
    if next < frames.len() {
        return Err(ReplayError::UnusedFrames {
            count: frames.len() - next,
        });
    }

    // The final shot and settled surface, from the last frame and the last recorded probe (the
    // live film's default probe is the centre cell of the first frame).
    let (mut final_shot, mut final_surface) = (None, None);
    if let Some(fi) = last_frame {
        let fr = &frames[fi];
        if let Some(ext) = fr.extent() {
            if let Some(view) = FieldView::new(ext, &fr.data, fr.mask.as_deref()) {
                let first = frames[0].extent().unwrap_or(ext);
                let world = probe.or_else(|| first.cell_center(first.cols() / 2, first.rows() / 2));
                let o = observe(
                    &view,
                    SourceId(u32::try_from(fr.source).unwrap_or(u32::MAX)),
                    fr.epoch,
                    fr.model_tick,
                );
                let at = world.unwrap_or_else(|| crate::cinema::field_center(ext));
                let shot = compose(&o, &view.probe(at), ext);
                let lf = LiveField {
                    source: fr.source,
                    cursor: gibson::wire::SourceCursor::new(fr.epoch, fr.sequence),
                    field: gibson::wire::WireField {
                        extent: ext,
                        model_tick: fr.model_tick,
                        data: fr.data.clone(),
                        mask: fr.mask.clone(),
                    },
                };
                final_surface = render_field(&lf, &shot, ANCHOR, shot.colormap, world);
                final_shot = Some(shot);
            }
        }
    }
    Ok(FilmReplay {
        beats,
        transforms: arc.transforms(),
        last_outcome: arc.last_outcome(),
        final_shot,
        final_surface,
    })
}

/// The beat path with consecutive repeats collapsed (`ESTABLISH>DISCOVER>…`).
pub fn beat_path(beats: &[Beat]) -> String {
    let mut out: Vec<&str> = Vec::new();
    for b in beats {
        let n = beat_name(*b);
        if out.last() != Some(&n) {
            out.push(n);
        }
    }
    out.join(">")
}

/// FNV-1a over the FULL per-event beat sequence (a deterministic receipt for exact comparison).
pub fn beats_digest(beats: &[Beat]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in beats {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}
