//! Bounded keyed presentation continuity and an optional `Context`-owned app loop.

use super::compile::{collect_metadata, compile, BuildCx, Compiled, ElementState};
use super::element::{Element, Key};
use super::interaction::{EventOutcome, InteractionMap, UiError};
use super::motion::{MotionPlan, MotionPreference, MotionRole};
use super::skin::{skins, ResolvedSkin, Skin, UiEnvironment};
use crate::focus::{FocusId, FocusRing};
use crate::glyph::detect_glyph_mode_from_env;
use crate::input::{Event, KeyCode, KeyModifiers};
use crate::{Context, RenderMode};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::time::{Duration, Instant};

/// The reconciliation vocabulary. An exit is reported but removed immediately;
/// this first implementation intentionally retains no departed tree or ghost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Enter,
    Exit,
    Stable,
    Change,
    Focus,
    Blur,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyChange {
    pub key: Key,
    pub kind: ChangeKind,
}

struct ActiveMotion {
    plan: MotionPlan,
    started: Duration,
}

/// Owns focus and finite presentation effects, never the application's model.
///
/// A frame replaces the interaction snapshot; dispatch returned actions into the
/// model and build another frame before processing the next edit. Persistent
/// maps contain only live keys, effects hold at most one finite plan per key,
/// and capture storage is bounded by the live modal count. Path keys preserve
/// positions only: give changing lists explicit keys to preserve identity.
pub struct UiRuntime<A> {
    skin: Skin,
    resolved: Option<(UiEnvironment, ResolvedSkin)>,
    previous: BTreeMap<Key, ElementState>,
    interactions: InteractionMap<A>,
    focus_ring: FocusRing,
    focus_keys: Vec<Key>,
    captures: Vec<Key>,
    focus_memory: BTreeMap<Option<Key>, Key>,
    presented_focus: Option<Key>,
    animations: BTreeMap<Key, ActiveMotion>,
    pending: BTreeMap<Key, MotionRole>,
    changes: Vec<KeyChange>,
    last_time: Duration,
}

impl<A: Clone> UiRuntime<A> {
    pub fn new(skin: Skin) -> Self {
        Self {
            skin,
            resolved: None,
            previous: BTreeMap::new(),
            interactions: InteractionMap::default(),
            focus_ring: FocusRing::default(),
            focus_keys: Vec::new(),
            captures: Vec::new(),
            focus_memory: BTreeMap::new(),
            presented_focus: None,
            animations: BTreeMap::new(),
            pending: BTreeMap::new(),
            changes: Vec::new(),
            last_time: Duration::ZERO,
        }
    }

    pub fn set_skin(&mut self, skin: Skin) {
        self.skin = skin;
        self.resolved = None;
        // Do not carry the previous design grammar's effects into the new skin.
        self.animations.clear();
        self.pending.clear();
    }

    pub fn skin(&self) -> &Skin {
        &self.skin
    }

    pub fn focus(&self) -> Option<&Key> {
        self.focus_ring
            .current()
            .and_then(|id| self.focus_keys.get(id.0 as usize))
    }

    /// Focuses an enabled element within the currently active modal scope.
    pub fn set_focus(&mut self, key: &Key) -> bool {
        self.focus_keys
            .iter()
            .position(|candidate| candidate == key)
            .is_some_and(|index| self.focus_ring.set(FocusId(index as u64)))
    }

    pub fn changes(&self) -> &[KeyChange] {
        &self.changes
    }

    pub fn active_animation_count(&self) -> usize {
        self.animations.len()
    }

    pub fn retained_key_count(&self) -> usize {
        self.previous.len()
    }

    pub fn modal_depth(&self) -> usize {
        self.captures.len()
    }

    pub fn interactions(&self) -> &InteractionMap<A> {
        &self.interactions
    }

    fn resolve(&mut self, environment: UiEnvironment) -> ResolvedSkin {
        if let Some((cached_environment, skin)) = self.resolved {
            if cached_environment == environment {
                return skin;
            }
        }
        let skin = self.skin.resolve(&environment);
        self.resolved = Some((environment, skin));
        skin
    }

    /// A custom component/view's environment before the next reconciliation.
    /// The resulting ordinary `BuildCx` can also be used with free `compile`.
    pub fn build_cx(&mut self, environment: UiEnvironment, time: Duration) -> BuildCx {
        let time = time.max(self.last_time);
        BuildCx {
            skin: self.resolve(environment),
            environment,
            focused: self.focus().cloned(),
            motions: self
                .animations
                .iter()
                .filter_map(|(key, active)| {
                    let effects = active.plan.effects(time.saturating_sub(active.started));
                    (!effects.is_empty()).then(|| (key.clone(), effects))
                })
                .collect(),
            time,
        }
    }

    /// Reconciles a semantic tree and lowers it to an inspectable ordinary node.
    ///
    /// Time is supplied by the caller. Repeated samples are deterministic;
    /// backward samples clamp to the previous time. Construct a fresh runtime
    /// for an independent replay. Invalid trees leave runtime state untouched.
    pub fn frame(
        &mut self,
        element: &Element<A>,
        environment: UiEnvironment,
        time: Duration,
    ) -> Result<Compiled<A>, UiError> {
        // Validate before changing identity, capture state, or effects. This is
        // a cheap metadata pass; expensive surfaces are lowered only once.
        let metadata = collect_metadata(element)?;
        let time = time.max(self.last_time);
        let skin = self.resolve(environment);
        self.changes.clear();
        self.animations.retain(|key, active| {
            // Preference/capability changes apply to already-running plans too.
            // Preserve their original clock so changing policy cannot extend life.
            active.plan = skin.motion(active.plan.role);
            metadata.keys.contains_key(key)
                && time.saturating_sub(active.started) < active.plan.duration
                && environment.motion != MotionPreference::None
        });
        let mut starts = BTreeMap::new();
        for (key, state) in &metadata.keys {
            let kind = match self.previous.get(key) {
                None => ChangeKind::Enter,
                Some(old) if same_state(old, state) => ChangeKind::Stable,
                Some(_) => ChangeKind::Change,
            };
            self.changes.push(KeyChange {
                key: key.clone(),
                kind,
            });
            if matches!(key, Key::Named(_)) || state.motion.is_some() || state.modal {
                let role = match kind {
                    ChangeKind::Enter if state.modal => Some(MotionRole::ModalEnter),
                    ChangeKind::Enter => Some(state.motion.unwrap_or(MotionRole::Enter)),
                    ChangeKind::Change => Some(state.motion.unwrap_or(MotionRole::Change)),
                    _ => None,
                };
                if let Some(role) = role {
                    starts.insert(key.clone(), role);
                }
            }
        }
        for key in self.previous.keys() {
            if !metadata.keys.contains_key(key) {
                self.changes.push(KeyChange {
                    key: key.clone(),
                    kind: ChangeKind::Exit,
                });
            }
        }
        self.reconcile_focus(&metadata.interactions);
        let focused = self.focus().cloned();
        if self.presented_focus != focused {
            if let Some(key) = &self.presented_focus {
                self.changes.push(KeyChange {
                    key: key.clone(),
                    kind: ChangeKind::Blur,
                });
                if metadata.keys.contains_key(key) {
                    starts.insert(key.clone(), MotionRole::Blur);
                }
            }
            if let Some(key) = &focused {
                self.changes.push(KeyChange {
                    key: key.clone(),
                    kind: ChangeKind::Focus,
                });
                starts.insert(key.clone(), MotionRole::Focus);
            }
        }
        starts.append(&mut self.pending);
        for (key, role) in starts {
            if metadata.keys.contains_key(&key) {
                let plan = skin.motion(role);
                // A zero-duration replacement also supersedes an older plan.
                self.animations.remove(&key);
                if !plan.duration.is_zero() {
                    self.animations.insert(
                        key,
                        ActiveMotion {
                            plan,
                            started: time,
                        },
                    );
                }
            }
        }
        self.interactions = metadata.interactions;
        self.previous = metadata.keys;
        self.presented_focus = focused;
        self.last_time = time;
        let cx = self.build_cx(environment, time);
        compile(element, &cx)
    }

    fn reconcile_focus(&mut self, map: &InteractionMap<A>) {
        let old_focus = self.focus().cloned();
        if let Some(key) = &old_focus {
            self.focus_memory
                .insert(self.captures.last().cloned(), key.clone());
        }

        // Modal layering follows rendered traversal, including sibling overlays.
        // Remember one focus key per *live* scope so opening a sibling above a
        // modal, removing an underlying modal, or reordering overlays does not
        // discard the covered scope's focus. No departed scope is retained.
        let path: Vec<Key> = map.modals.iter().map(|scope| scope.key.clone()).collect();
        let live_scopes: BTreeSet<&Key> = path.iter().collect();
        let mut eligible: BTreeMap<Option<Key>, Vec<Key>> = BTreeMap::new();
        let mut owner: BTreeMap<&Key, &Option<Key>> = BTreeMap::new();
        for entry in map.entries.iter().filter(|entry| !entry.disabled) {
            eligible
                .entry(entry.modal.clone())
                .or_default()
                .push(entry.key.clone());
            owner.insert(&entry.key, &entry.modal);
        }
        self.focus_memory.retain(|scope, key| {
            scope
                .as_ref()
                .is_none_or(|modal| live_scopes.contains(modal))
                && owner.get(key).is_some_and(|current| *current == scope)
        });
        for (scope, keys) in &eligible {
            self.focus_memory
                .entry(scope.clone())
                .or_insert_with(|| keys[0].clone());
        }
        let active = path.last().cloned();
        let focus_keys = eligible.remove(&active).unwrap_or_default();
        let desired = old_focus
            .filter(|key| focus_keys.contains(key))
            .or_else(|| self.focus_memory.get(&active).cloned());
        if focus_keys != self.focus_keys {
            self.focus_keys = focus_keys;
            self.focus_ring = FocusRing::new((0..self.focus_keys.len()).map(|i| FocusId(i as u64)));
        }
        if let Some(key) = desired {
            self.set_focus(&key);
        }
        if let Some(key) = self.focus().cloned() {
            self.focus_memory.insert(active, key);
        }
        self.captures = path;
    }

    /// Routes focus traversal, activation, app-owned input editing and dismissal.
    /// A modal consumes all keyboard/paste events, including unbound keys.
    pub fn handle_event(&mut self, event: &Event) -> EventOutcome<A> {
        let mut outcome = EventOutcome::default();
        let modal_active = self.interactions.active_modal().is_some();
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Tab | KeyCode::BackTab => {
                    if key.code == KeyCode::BackTab || key.modifiers.contains(KeyModifiers::SHIFT) {
                        self.focus_ring.focus_prev();
                    } else {
                        self.focus_ring.focus_next();
                    }
                    outcome.consumed = !self.focus_keys.is_empty() || modal_active;
                    return outcome;
                }
                KeyCode::Esc if modal_active => {
                    if let Some(action) = self
                        .interactions
                        .active_modal()
                        .and_then(|scope| scope.on_dismiss.clone())
                    {
                        outcome.actions.push(action);
                    }
                    outcome.consumed = true;
                    return outcome;
                }
                _ => {}
            }
        }
        if let Some(entry) = self.focus().and_then(|key| self.interactions.get(key)) {
            if let Event::Key(key) = event {
                let activate = key.code == KeyCode::Enter
                    || (key.code == KeyCode::Char(' ')
                        && key.modifiers.is_empty()
                        && entry.input.is_none());
                if activate {
                    if let Some(action) = &entry.on_press {
                        outcome.actions.push(action.clone());
                        outcome.consumed = true;
                        self.pending.insert(entry.key.clone(), MotionRole::Activate);
                        return outcome;
                    }
                }
            }
            if let (Some(state), Some(on_edit)) = (&entry.input, &entry.on_edit) {
                let mut edited = state.clone();
                if edited.handle_event(event) {
                    outcome.actions.push(on_edit(edited));
                    outcome.consumed = true;
                    return outcome;
                }
                if is_edit_event(event) {
                    // An arrow at the edge, or backspace in an empty buffer,
                    // still belongs to the editor, even when no state changes.
                    outcome.consumed = true;
                    return outcome;
                }
            }
            if let Some(on_event) = &entry.on_event {
                outcome.actions.push(on_event(event.clone()));
                outcome.consumed = true;
                return outcome;
            }
        }
        outcome.consumed = modal_active && matches!(event, Event::Key(_) | Event::Paste(_));
        outcome
    }
}

fn is_edit_event(event: &Event) -> bool {
    match event {
        Event::Paste(_) => true,
        Event::Key(key) => match key.code {
            KeyCode::Char(c) => {
                !key.modifiers.contains(KeyModifiers::CONTROL) || matches!(c, 'a' | 'e' | 'u' | 'k')
            }
            KeyCode::Backspace
            | KeyCode::Delete
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Home
            | KeyCode::End => true,
            _ => false,
        },
        _ => false,
    }
}

fn same_state(left: &ElementState, right: &ElementState) -> bool {
    left.revision == right.revision
        && left.selected == right.selected
        && left.disabled == right.disabled
        && left.modal == right.modal
        && left.motion == right.motion
}

/// Inputs reaching the application update function.
#[derive(Debug, Clone)]
pub enum AppEvent<A> {
    Action(A),
    /// An event not consumed by a focused control or modal.
    Input(Event),
    Tick(Duration),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Continue,
    Quit,
}

/// Optional small application loop using the ordinary terminal-owning `Context`.
///
/// Use `UiRuntime` directly when the application already has a loop. This helper
/// sends Ctrl-C to the application's `Input` handler and then quits, even under
/// modal capture. It renders one frame on redirected/noninteractive output.
pub struct App {
    mode: RenderMode,
    skin: Skin,
    motion: MotionPreference,
    fps: u32,
}

impl App {
    pub fn inline() -> Self {
        Self {
            mode: RenderMode::Inline,
            skin: skins::BLACK_ICE,
            motion: MotionPreference::Full,
            fps: 30,
        }
    }

    pub fn fullscreen() -> Self {
        Self {
            mode: RenderMode::Fullscreen,
            ..Self::inline()
        }
    }

    pub fn skin(mut self, skin: Skin) -> Self {
        self.skin = skin;
        self
    }

    pub fn motion(mut self, preference: MotionPreference) -> Self {
        self.motion = preference;
        self
    }

    pub fn fps(mut self, fps: u32) -> Self {
        self.fps = fps.clamp(1, 120);
        self
    }

    pub fn run<M, A: Clone>(
        self,
        model: M,
        mut update: impl FnMut(&mut M, AppEvent<A>) -> Control,
        view: impl FnMut(&M, &BuildCx) -> Element<A>,
    ) -> io::Result<M> {
        let mut context = Context::new(self.mode)?;
        let result = self.run_with_context(
            &mut context,
            model,
            |model, event, _context| Ok(update(model, event)),
            view,
        );
        let restored = context.restore();
        match result {
            Ok(model) => restored.map(|()| model),
            Err(error) => Err(error),
        }
    }

    /// Uses a caller-owned `Context`, preserving scrollback and custom rendering
    /// access in update. It neither acquires another session nor restores the
    /// caller's context on return. The caller remains responsible for its scope.
    pub fn run_with_context<M, A: Clone>(
        self,
        context: &mut Context,
        mut model: M,
        mut update: impl FnMut(&mut M, AppEvent<A>, &mut Context) -> io::Result<Control>,
        mut view: impl FnMut(&M, &BuildCx) -> Element<A>,
    ) -> io::Result<M> {
        context.set_max_fps(self.fps);
        let interval = Duration::from_secs_f64(1.0 / self.fps as f64);
        let start = Instant::now();
        let mut runtime = UiRuntime::new(self.skin);
        let glyph_mode = detect_glyph_mode_from_env(None)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        // Headless sessions deliberately emulate TTY rendering, but have no
        // real input reader. Do not confuse renderer capability with ownership
        // of an interactive terminal.
        let interactive = context.session.is_tty && !context.session.is_headless();
        loop {
            let now = start.elapsed();
            let (width, terminal_height) = context.session.terminal_size();
            let height = match context.renderer.mode {
                RenderMode::Inline => terminal_height.saturating_sub(1).max(1),
                RenderMode::Fullscreen => terminal_height,
            };
            let environment = UiEnvironment {
                width,
                height,
                color_depth: context.capabilities().color_depth,
                glyph_mode,
                motion: if interactive {
                    self.motion
                } else {
                    MotionPreference::None
                },
            };
            let cx = runtime.build_cx(environment, now);
            let element = view(&model, &cx);
            let frame = runtime
                .frame(&element, environment, now)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
            context.set_root(frame.node);
            if !interactive {
                context.render_now()?;
                return Ok(model);
            }
            context.render_if_due()?;
            if let Some(event) = context.poll_event(interval)? {
                if matches!(&event, Event::Key(key) if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    update(&mut model, AppEvent::Input(event), context)?;
                    return Ok(model);
                }
                let outcome = runtime.handle_event(&event);
                for action in outcome.actions {
                    if update(&mut model, AppEvent::Action(action), context)? == Control::Quit {
                        return Ok(model);
                    }
                }
                if !outcome.consumed
                    && update(&mut model, AppEvent::Input(event), context)? == Control::Quit
                {
                    return Ok(model);
                }
            }
            if update(&mut model, AppEvent::Tick(start.elapsed()), context)? == Control::Quit {
                return Ok(model);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyEvent, TextInputState};
    use crate::ui::{button, column, modal, status, text, text_input};

    fn modal_stack(order: &[&str]) -> Element<()> {
        let mut tree = column()
            .child(button("base first").key("base.1").on_press(()))
            .child(button("base second").key("base.2").on_press(()));
        for name in order {
            tree = tree.overlay(
                modal(*name)
                    .key(*name)
                    .child(button("first").key(format!("{name}.1")).on_press(()))
                    .child(button("second").key(format!("{name}.2")).on_press(())),
            );
        }
        tree
    }

    fn present_modals(runtime: &mut UiRuntime<()>, order: &[&str]) {
        runtime
            .frame(
                &modal_stack(order),
                UiEnvironment::default(),
                Duration::ZERO,
            )
            .unwrap();
    }

    #[test]
    fn sibling_modal_restores_covered_modals_selected_focus() {
        let mut runtime = UiRuntime::new(skins::BLACK_ICE);
        present_modals(&mut runtime, &[]);
        assert!(runtime.set_focus(&Key::from("base.2")));
        present_modals(&mut runtime, &["A"]);
        assert!(runtime.set_focus(&Key::from("A.2")));
        present_modals(&mut runtime, &["A", "B"]);
        assert_eq!(runtime.focus(), Some(&Key::from("B.1")));
        assert_eq!(runtime.modal_depth(), 2);
        present_modals(&mut runtime, &["A"]);
        assert_eq!(runtime.focus(), Some(&Key::from("A.2")));
        present_modals(&mut runtime, &[]);
        assert_eq!(runtime.focus(), Some(&Key::from("base.2")));
    }

    #[test]
    fn modal_reordering_and_underlying_removal_preserve_live_focus() {
        let mut runtime = UiRuntime::new(skins::VAPOR95);
        present_modals(&mut runtime, &[]);
        assert!(runtime.set_focus(&Key::from("base.2")));
        present_modals(&mut runtime, &["A"]);
        assert!(runtime.set_focus(&Key::from("A.2")));
        present_modals(&mut runtime, &["A", "B"]);
        assert!(runtime.set_focus(&Key::from("B.2")));
        present_modals(&mut runtime, &["B", "A"]);
        assert_eq!(runtime.focus(), Some(&Key::from("A.2")));
        present_modals(&mut runtime, &["A", "B"]);
        assert_eq!(runtime.focus(), Some(&Key::from("B.2")));
        present_modals(&mut runtime, &["B"]);
        assert_eq!(runtime.focus(), Some(&Key::from("B.2")));
        assert_eq!(runtime.modal_depth(), 1);
        // Removed scopes are forgotten instead of accumulating history.
        assert!(!runtime.focus_memory.contains_key(&Some(Key::from("A"))));
        present_modals(&mut runtime, &[]);
        assert_eq!(runtime.focus(), Some(&Key::from("base.2")));
        assert_eq!(runtime.focus_memory.len(), 1);
    }

    #[test]
    fn changing_policy_restricts_already_running_motion() {
        let mut runtime = UiRuntime::<()>::new(skins::BLACK_ICE);
        let tree = status("READY").key("state");
        let mut environment = UiEnvironment::default();
        runtime.frame(&tree, environment, Duration::ZERO).unwrap();
        assert!(runtime.active_animation_count() > 0);

        // Full motion is still active, but the reduced-motion deadline has
        // already passed. Changing policy must not keep the Full effect alive.
        environment.motion = MotionPreference::Reduced;
        runtime
            .frame(&tree, environment, Duration::from_millis(100))
            .unwrap();
        assert_eq!(runtime.active_animation_count(), 0);

        let changed = status("UPDATED").key("state").revision(1);
        runtime
            .frame(&changed, environment, Duration::from_millis(100))
            .unwrap();
        assert!(runtime.active_animation_count() > 0);
        environment.motion = MotionPreference::None;
        runtime
            .frame(&changed, environment, Duration::from_millis(100))
            .unwrap();
        assert_eq!(runtime.active_animation_count(), 0);
    }

    #[test]
    fn editor_boundary_keys_do_not_leak_into_background_shortcuts() {
        let mut runtime = UiRuntime::new(skins::SWISS_SIGNAL);
        let tree = text_input(&TextInputState::new())
            .key("input")
            .on_edit(|state| state.text);
        runtime
            .frame(&tree, UiEnvironment::default(), Duration::ZERO)
            .unwrap();
        for code in [KeyCode::Backspace, KeyCode::Delete, KeyCode::Left] {
            let outcome =
                runtime.handle_event(&Event::Key(KeyEvent::new(code, KeyModifiers::empty())));
            assert!(outcome.consumed);
            assert!(outcome.actions.is_empty());
        }
        let outcome = runtime.handle_event(&Event::Key(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::empty(),
        )));
        assert!(!outcome.consumed);
    }

    #[test]
    fn borrowed_headless_app_renders_settled_once_and_context_remains_usable() {
        let mut context = Context::headless(RenderMode::Inline, 40, 8);
        let model = App::inline()
            .skin(skins::VAPOR95)
            .run_with_context(
                &mut context,
                7,
                |_, _: AppEvent<()>, _| panic!("headless app should not poll input"),
                |_, cx| {
                    assert_eq!(cx.environment.height, 7);
                    text("READY").key("ready")
                },
            )
            .unwrap();
        assert_eq!(model, 7);
        assert_eq!(context.stats().frames, 1);
        assert!(context.take_output().contains("READY"));
        context.commit_text("still owned by caller").unwrap();
        assert!(context.stats().commit_bytes > 0);
    }
}
