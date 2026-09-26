//! High-level counterpart to polished_agent, retaining its core interaction
//! story: inspect -> permission -> patch -> test failure -> recovery -> prompt.
//! This is a simulated agent: it never edits files or executes tool commands.
//! Inline transcript uses Context; UI owns keyed focus and modal restoration.
//! Domain state, scripted work, viewport position and toast expiry remain here.
//!
//! --stage=plan|permission|failure|prompt|rejected|cancelled starts a coherent state.
//! --auto runs a deterministic approve-once demonstration. --dump captures a
//! settled state headlessly. --skin=, --width=, --height=, --mono, --ansi16,
//! --reduced-motion, --no-motion, --fullscreen match the galleries.

#[path = "ui_support/options.rs"]
mod options;

use gibson::ui::prelude::*;
use gibson::{Context, Event, KeyCode, KeyModifiers, TextInputState};
use options::{present, Options};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const TOOLS: [(&str, &str); 9] = [
    ("Research", "read renderer pipeline"),
    ("Inspect", "find insertion call sites"),
    ("Reproduce", "confirm clipping regression"),
    ("Patch", "simulate geometry change"),
    ("Test", "FAIL: wide-glyph clip leak"),
    ("Diagnose", "clip must win over glyph"),
    ("Patch", "simulate surface correction"),
    ("Retest", "simulated checks pass"),
    ("Review", "inspect recovered invariant"),
];
const CODE: &str = "01  fn clip_cell(surface, x, y) {\n02      // Preserve wide grapheme pairs.\n03 -    surface.put(x, y, glyph);\n04 +    if clip.contains(x, y) {\n05 +        surface.put(x, y, glyph);\n06 +    }\n07  }\n08\n09  #[test]\n10  fn wide_glyph_respects_clip() {\n11      let pair = make_wide_pair();\n12      let visible = clip(pair);\n13      assert_pair_is_complete(visible);\n14  }\n15\n16  // Synthetic review buffer.\n17  // No source file is edited by this demo.\n18  // Home/End/PageUp/PageDown scroll.";
const STREAM: &str = "I inspected the live-region boundary and reproduced the clipping defect. The proposed correction keeps complete grapheme pairs inside the visible clip. A failed first check is followed by diagnosis, correction, and a successful simulated rerun.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Inspect,
    Permission,
    Patch,
    Prompt,
    Rejected,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Approval {
    Once,
    Session,
}
#[derive(Clone)]
enum Action {
    Approve(Approval),
    Reject,
    Cancel,
    Edit(TextInputState),
    Submit,
    Scroll(Event),
    Quit,
}

struct Model {
    stage: Stage,
    cursor: usize,
    progress: f32,
    approved: Option<Approval>,
    simulated_changes: usize,
    input: TextInputState,
    code_y: i32,
    events: VecDeque<String>,
    pending_history: VecDeque<String>,
    toast: Option<(String, Tone, Duration)>,
    now: Duration,
    frame_bytes: VecDeque<f32>,
    frames: u64,
    total_bytes: u64,
    followups: usize,
    background_sent: bool,
    quit: bool,
}

impl Model {
    fn new(stage: Option<&str>) -> Self {
        let mut model = Self {
            stage: Stage::Inspect,
            cursor: 0,
            progress: 0.,
            approved: None,
            simulated_changes: 0,
            input: TextInputState::new(),
            code_y: 0,
            events: VecDeque::new(),
            pending_history: VecDeque::new(),
            toast: None,
            now: Duration::ZERO,
            frame_bytes: VecDeque::new(),
            frames: 0,
            total_bytes: 0,
            followups: 0,
            background_sent: false,
            quit: false,
        };
        match stage {
            Some("permission") => {
                model.cursor = 3;
                model.stage = Stage::Permission;
            }
            Some("failure") => {
                model.cursor = 5;
                model.stage = Stage::Patch;
                model.approved = Some(Approval::Once);
                model.simulated_changes = 1;
                model.notify("First check failed: wide-glyph clipping", Tone::Danger);
            }
            Some("prompt") => {
                model.cursor = TOOLS.len();
                model.stage = Stage::Prompt;
                model.approved = Some(Approval::Once);
                model.simulated_changes = 2;
            }
            Some("rejected") => {
                model.cursor = 3;
                model.stage = Stage::Rejected;
            }
            Some("cancelled") => model.stage = Stage::Cancelled,
            _ => {}
        }
        model
    }

    fn notify(&mut self, message: impl Into<String>, tone: Tone) {
        let message = message.into();
        if self.events.len() == 24 {
            self.events.pop_front();
        }
        self.events.push_back(message.clone());
        self.pending_history.push_back(message.clone());
        self.toast = Some((message, tone, self.now + Duration::from_secs(3)));
    }

    fn action(&mut self, action: Action) {
        match action {
            Action::Approve(scope) if self.stage == Stage::Permission => {
                self.approved = Some(scope);
                self.stage = Stage::Patch;
                self.notify(
                    match scope {
                        Approval::Once => "Approved once: simulate proposed change",
                        Approval::Session => "Approved for session: simulate this change class",
                    },
                    Tone::Success,
                );
            }
            Action::Reject if self.stage == Stage::Permission => {
                self.stage = Stage::Rejected;
                self.notify(
                    "Rejected before mutation: no simulated changes",
                    Tone::Warning,
                );
            }
            Action::Cancel => {
                self.stage = Stage::Cancelled;
                self.notify(
                    format!(
                        "Cancelled after {} simulated changes; no real files written",
                        self.simulated_changes
                    ),
                    Tone::Warning,
                );
            }
            Action::Edit(input) => self.input = input,
            Action::Submit if self.stage == Stage::Prompt && !self.input.text.is_empty() => {
                let input = std::mem::take(&mut self.input);
                self.followups += 1;
                self.notify(format!("> {}", input.text), Tone::Info);
            }
            Action::Scroll(Event::Key(key)) => {
                self.code_y = match key.code {
                    KeyCode::Up => self.code_y - 1,
                    KeyCode::Down => self.code_y + 1,
                    KeyCode::PageUp => self.code_y - 5,
                    KeyCode::PageDown => self.code_y + 5,
                    KeyCode::Home => 0,
                    KeyCode::End => 13,
                    _ => self.code_y,
                }
                .clamp(0, 13);
            }
            Action::Quit => self.quit = true,
            _ => {}
        }
    }

    fn advance(&mut self, now: Duration, delta: Duration) {
        self.now = now;
        if self
            .toast
            .as_ref()
            .is_some_and(|(_, _, expires)| *expires <= now)
        {
            self.toast = None;
        }
        let end = match self.stage {
            Stage::Inspect => 3,
            Stage::Patch if self.approved.is_some() => TOOLS.len(),
            _ => return,
        };
        // A mutation runner cannot advance without an explicit approval.
        self.progress += delta.as_secs_f32() / 0.7;
        if self.progress < 1.0 || self.cursor >= end {
            return;
        }
        let done = self.cursor;
        if matches!(done, 3 | 6) {
            self.simulated_changes += 1;
        }
        self.notify(
            format!("{}: {}", TOOLS[done].0, TOOLS[done].1),
            if done == 4 {
                Tone::Danger
            } else {
                Tone::Success
            },
        );
        self.cursor += 1;
        self.progress = 0.;
        if self.cursor == end {
            self.stage = if end == 3 {
                Stage::Permission
            } else {
                Stage::Prompt
            };
        }
    }

    fn sample(&mut self, context: &mut Context) {
        let stats = context.stats();
        if stats.frames > self.frames {
            if self.frame_bytes.len() == 64 {
                self.frame_bytes.pop_front();
            }
            self.frame_bytes
                .push_back(stats.frame_bytes.saturating_sub(self.total_bytes) as f32);
        }
        self.frames = stats.frames;
        self.total_bytes = stats.frame_bytes;
    }

    fn flush_history(&mut self, context: &mut Context) -> std::io::Result<()> {
        while let Some(line) = self.pending_history.pop_front() {
            context.insert_text_before_live(&line)?;
        }
        Ok(())
    }

    fn summary(&self) -> String {
        let result = match self.stage {
            Stage::Rejected => "rejected",
            Stage::Cancelled => "cancelled",
            Stage::Prompt => "complete",
            _ => "stopped before completion",
        };
        format!(
            "[demo] session {result}; {} simulated changes, {} follow-ups; no real files written",
            self.simulated_changes, self.followups
        )
    }
}

fn view(model: &Model, cx: &BuildCx) -> Element<Action> {
    let narrow = cx.environment.width < 60;
    let count = if narrow {
        3
    } else if cx.environment.height < 30 {
        5
    } else {
        TOOLS.len()
    };
    let start = if count < TOOLS.len() {
        model.cursor.saturating_sub(1).min(TOOLS.len() - count)
    } else {
        0
    };
    let plan = panel("TASK PLAN").grow(1.0).children(
        TOOLS
            .iter()
            .enumerate()
            .skip(start)
            .take(count)
            .map(|(i, (verb, _))| {
                let (state, tone) = if i == 4 && i < model.cursor {
                    ("FAILED", Tone::Danger)
                } else if i < model.cursor {
                    ("DONE", Tone::Success)
                } else if matches!(model.stage, Stage::Rejected | Stage::Cancelled) {
                    ("SKIPPED", Tone::Neutral)
                } else if i == model.cursor && model.stage != Stage::Permission {
                    ("RUNNING", Tone::Accent)
                } else {
                    ("QUEUED", Tone::Neutral)
                };
                status(format!("{:02} {verb:<9} {state}", i + 1))
                    .tone(tone)
                    .key(format!("task.{i}"))
                    .revision(u64::from(i < model.cursor))
            }),
    );
    let plan = if narrow {
        plan
    } else {
        plan.child(progress(
            "session",
            (model.cursor as f32 + model.progress) / TOOLS.len() as f32,
        ))
    };
    let code_view = panel("CODE REVIEW / TAB THEN ARROWS")
        .grow(1.0)
        .child(
            viewport(0, model.code_y)
                .height(if narrow { 3 } else { 5 })
                .key("code")
                .on_event(Action::Scroll)
                .child(code(CODE)),
        )
        .child(label("Synthetic patch • no disk writes").emphasis(Emphasis::Muted));
    let shown =
        ((model.cursor as f32 + model.progress) / 5. * STREAM.chars().count() as f32) as usize;
    let mut tree = screen()
        .gap(0)
        .density(Density::Compact)
        .child(heading("GIBSON / AGENT CONSOLE"))
        .child(
            row()
                .gap(1)
                .child(badge(if narrow { "DEMO" } else { "SIMULATED SESSION" }).tone(Tone::Info))
                .child(status(format!("{:?}", model.stage)).tone(
                    if model.stage == Stage::Rejected {
                        Tone::Warning
                    } else {
                        Tone::Success
                    },
                )),
        )
        .child(row().responsive(60).gap(0).child(plan).child(code_view));
    // In a constrained viewport, prioritize actionable controls and review content.
    if cx.environment.height >= 30 {
        tree =
            tree.child(panel("STREAM").child(text(STREAM.chars().take(shown).collect::<String>())));
    }
    if !narrow && cx.environment.height >= 30 {
        let bytes: Vec<_> = model.frame_bytes.iter().copied().collect();
        tree = tree.child(
            row()
                .gap(1)
                .child(
                    panel("RENDERER / ACTUAL BYTES")
                        .grow(1.0)
                        .child(sparkline(&bytes))
                        .child(label(format!(
                            "{} frames / {} B",
                            model.frames, model.total_bytes
                        ))),
                )
                .child(
                    panel("EVENTS").grow(1.0).children(
                        model
                            .events
                            .iter()
                            .rev()
                            .take(2)
                            .rev()
                            .map(|event| text(event.clone()).emphasis(Emphasis::Muted)),
                    ),
                ),
        );
    }
    tree = tree
        .child(
            panel("PROMPT").child(
                row()
                    .gap(1)
                    .child(
                        text_input(&model.input)
                            .key("prompt")
                            .grow(1.0)
                            .placeholder(if model.stage == Stage::Prompt {
                                "Type a follow-up…"
                            } else {
                                "Waiting for session…"
                            })
                            .disabled(model.stage != Stage::Prompt)
                            .on_edit(Action::Edit)
                            .on_press(Action::Submit),
                    )
                    .child(
                        button(
                            if matches!(
                                model.stage,
                                Stage::Rejected | Stage::Cancelled | Stage::Prompt
                            ) {
                                "Finish"
                            } else {
                                "Cancel"
                            },
                        )
                        .key("finish")
                        .on_press(
                            if matches!(
                                model.stage,
                                Stage::Rejected | Stage::Cancelled | Stage::Prompt
                            ) {
                                Action::Quit
                            } else {
                                Action::Cancel
                            },
                        ),
                    ),
            ),
        )
        .child(
            label("Tab focus • Enter activate • Ctrl-C cancel / exit").emphasis(Emphasis::Muted),
        );
    if model.stage == Stage::Permission {
        tree = tree.overlay(
            modal("PERMISSION / BEFORE MUTATION")
                .key("permission")
                .height(12)
                .density(Density::Compact)
                .on_dismiss(Action::Reject)
                .child(text(
                    "Apply the simulated correction? No real files are edited.",
                ))
                .child(
                    button("Approve once")
                        .key("approve.once")
                        .on_press(Action::Approve(Approval::Once)),
                )
                .child(
                    button("Approve for session")
                        .key("approve.session")
                        .on_press(Action::Approve(Approval::Session)),
                )
                .child(
                    button("Reject")
                        .key("reject")
                        .tone(Tone::Danger)
                        .on_press(Action::Reject),
                )
                .child(
                    button("Cancel session")
                        .key("cancel")
                        .on_press(Action::Cancel),
                ),
        );
    } else if let Some((message, tone, _)) = &model.toast {
        tree = tree.overlay(toast(message.clone()).key("toast").tone(*tone));
    }
    tree
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = Options::read();
    let mut context = options.context()?;
    let mut runtime = UiRuntime::new(options.skin);
    let mut model = Model::new(options.value("--stage="));
    let auto = options.has("--auto");
    let started = Instant::now();
    let mut previous = Duration::ZERO;
    let mut frames = 0;
    if !options.dump {
        context.commit_text(
            "$ gibson-agent-ui\n[demo] Audit clipping; permission precedes simulated changes.",
        )?;
    }
    loop {
        let now = if auto {
            Duration::from_millis(frames * 50)
        } else if options.dump {
            Duration::from_secs(1)
        } else {
            started.elapsed()
        };
        if !options.dump {
            model.advance(now, now.saturating_sub(previous));
        }
        previous = now;
        if auto {
            match model.stage {
                Stage::Permission => model.action(Action::Approve(Approval::Once)),
                Stage::Prompt => {
                    model.action(Action::Edit(TextInputState::with_text(
                        "review 🦀 你好 e\u{301} 👩\u{200d}💻",
                    )));
                    model.action(Action::Submit);
                    model.quit = true;
                }
                Stage::Rejected | Stage::Cancelled => model.quit = true,
                _ => {}
            }
        }
        // This independent event demonstrates history insertion while draft text
        // remains owned by the application, even while the prompt has focus.
        if model.stage == Stage::Prompt && !model.background_sent {
            model.background_sent = true;
            model.notify(
                "Background index complete; current draft preserved",
                Tone::Info,
            );
        }
        if !options.dump {
            model.flush_history(&mut context)?;
            model.sample(&mut context);
        }
        let environment = options.environment(&context);
        let tree = view(&model, &BuildCx::new(options.skin, environment));
        if options.dump {
            present(
                &mut runtime,
                &tree,
                &mut context,
                environment,
                Duration::ZERO,
            )?;
        }
        present(&mut runtime, &tree, &mut context, environment, now)?;
        if options.dump {
            options.print_capture(&context);
            break;
        }
        if model.quit {
            break;
        }
        if !auto {
            if let Some(event) = context.run_once(Duration::from_millis(33))? {
                if matches!(&event, Event::Key(k) if k.code == KeyCode::Char('c')
                    && k.modifiers.contains(KeyModifiers::CONTROL))
                {
                    if matches!(
                        model.stage,
                        Stage::Cancelled | Stage::Rejected | Stage::Prompt
                    ) {
                        model.quit = true;
                    } else {
                        model.action(Action::Cancel);
                    }
                } else {
                    for action in runtime.handle_event(&event).actions {
                        model.action(action);
                    }
                }
            }
        }
        frames += 1;
    }
    if !options.dump {
        model.flush_history(&mut context)?;
        context.commit_text(&model.summary())?;
    }
    context.restore()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_buttons_route_through_the_real_typed_runtime() {
        for (key, expected_stage, approval) in [
            ("approve.once", Stage::Patch, Some(Approval::Once)),
            ("approve.session", Stage::Patch, Some(Approval::Session)),
            ("reject", Stage::Rejected, None),
            ("cancel", Stage::Cancelled, None),
        ] {
            let mut model = Model::new(Some("permission"));
            let environment = UiEnvironment {
                motion: MotionPreference::None,
                ..UiEnvironment::default()
            };
            let mut runtime = UiRuntime::new(skins::BLACK_ICE);
            runtime
                .frame(
                    &view(&model, &BuildCx::new(skins::BLACK_ICE, environment)),
                    environment,
                    Duration::ZERO,
                )
                .unwrap();
            assert!(runtime.set_focus(&Key::named(key)));
            let pressed = Event::Key(gibson::KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
            for action in runtime.handle_event(&pressed).actions {
                model.action(action);
            }
            assert_eq!(model.stage, expected_stage);
            assert_eq!(model.approved, approval);
            assert_eq!(model.simulated_changes, 0);
        }
    }

    #[test]
    fn permission_precedes_every_simulated_mutation() {
        let mut model = Model::new(None);
        for second in 1..20 {
            model.advance(Duration::from_secs(second), Duration::from_secs(1));
        }
        assert_eq!(model.stage, Stage::Permission);
        assert_eq!(model.simulated_changes, 0);
        model.action(Action::Reject);
        for second in 20..30 {
            model.advance(Duration::from_secs(second), Duration::from_secs(1));
        }
        assert_eq!(model.simulated_changes, 0);
        assert_eq!(model.stage, Stage::Rejected);
    }

    #[test]
    fn both_approval_scopes_run_failure_and_recovery() {
        for scope in [Approval::Once, Approval::Session] {
            let mut model = Model::new(Some("permission"));
            model.action(Action::Approve(scope));
            for second in 1..8 {
                model.advance(Duration::from_secs(second), Duration::from_secs(1));
            }
            assert_eq!(model.approved, Some(scope));
            assert_eq!(model.stage, Stage::Prompt);
            assert_eq!(model.simulated_changes, 2);
            assert!(model.events.iter().any(|s| s.contains("FAIL")));
            assert!(model
                .events
                .iter()
                .any(|s| s.contains("simulated checks pass")));
        }
    }

    #[test]
    fn cancellation_stops_runner_and_retains_truthful_partial_result() {
        let mut model = Model::new(Some("failure"));
        model.action(Action::Cancel);
        model.advance(Duration::from_secs(99), Duration::from_secs(99));
        assert_eq!(model.cursor, 5);
        assert_eq!(model.simulated_changes, 1);
        assert!(model.summary().contains("1 simulated changes"));
    }

    #[test]
    fn background_notification_preserves_unicode_draft() {
        let mut model = Model::new(Some("prompt"));
        model.action(Action::Edit(TextInputState::with_text("🦀 e\u{301} 你好")));
        model.notify("background event", Tone::Info);
        assert_eq!(model.input.text, "🦀 e\u{301} 你好");
        model.action(Action::Submit);
        assert!(model.input.text.is_empty());
        assert_eq!(model.followups, 1);
    }
}
