//! Public-API host recipe. Headless integration evidence, not a visual/physical gate.
use gibson::raster::{Rgb, RgbRaster, RgbaRaster};
use gibson::stage::*;
use gibson::ui::experience::{
    handle_key_outcome, Action, Blades, Content, Destination, Experience, FrameDemand, Grammar,
    Item, KeyOutcome, PresentationReceipt, PresentationState, Standard,
};
use gibson::ui::prelude::*;
use gibson::{
    Context, Event, KeyCode, KeyEvent, KeyModifiers, Node, RenderMode, Surface, TextInputState,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Inner {
    Open(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Host {
    Inner(Inner),
    Outside,
    Edit(String),
}
fn key(s: &str) -> Key {
    Key::named(s)
}
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn event(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::empty()))
}
fn model(order: &[&str]) -> Experience<Inner> {
    Experience::new("Synthetic").destination(Destination::new(
        "items",
        "Items",
        Content::Collection(
            order
                .iter()
                .map(|id| {
                    Item::new(*id, *id).action(Action::new(
                        format!("open-{id}"),
                        "Open",
                        Inner::Open((*id).into()),
                    ))
                })
                .collect(),
        ),
    ))
}
fn grammars() -> Vec<Box<dyn Grammar<Inner>>> {
    vec![Box::new(Standard), Box::new(Blades::new())]
}
struct Produced {
    frame: StageFrame<Host>,
    receipt: PresentationReceipt,
    demand: FrameDemand,
}
struct Fixture {
    grammar: Box<dyn Grammar<Inner>>,
    experience: Experience<Inner>,
    state: PresentationState,
    ui: UiRuntime<Host>,
    driver: PresentationDriver,
    ctx: Context,
    size: (u16, u16),
    x: u16,
    reorder: bool,
    phase: u16,
    reading: String,
    editor: TextInputState,
    info: FrameDemand,
    motion: MotionPreference,
    image: Arc<RgbaRaster>,
    builds: usize,
    paints: Arc<AtomicUsize>,
}
impl Fixture {
    fn new(grammar: Box<dyn Grammar<Inner>>, motion: MotionPreference) -> Self {
        let experience = model(&["a", "b"]);
        let state = PresentationState::new(&experience);
        let mut image = RgbaRaster::new(4, 4);
        image.pixels_mut().fill([240, 0, 0, 255]);
        image.set(0, 0, [255, 255, 255, 255]);
        image.set(1, 1, [200, 100, 0, 128]);
        image.set(2, 0, [200, 100, 0, 0]);
        let mut ctx = Context::headless(RenderMode::Fullscreen, 80, 24);
        ctx.scheduler.set_max_fps(u32::MAX);
        Self {
            grammar,
            state,
            experience,
            ui: UiRuntime::new(skins::SWISS_SIGNAL),
            driver: PresentationDriver::new(PresentationOptions {
                transition: ms(200),
                cadence: ms(25),
            })
            .unwrap(),
            ctx,
            size: (80, 24),
            x: 0,
            reorder: false,
            phase: 0,
            reading: "Synthetic reading".into(),
            editor: TextInputState::with_text("draft"),
            info: FrameDemand::After(ms(100)),
            motion,
            image: Arc::new(image),
            builds: 0,
            paints: Arc::new(AtomicUsize::new(0)),
        }
    }
    fn substrate(&self, x: u16, y: u16) -> Rgb {
        if (x / 2 + y / 2 + self.phase) % 2 == 0 {
            (10, 30, 50)
        } else {
            (90, 110, 130)
        }
    }
    fn resolve(&mut self, now: Duration) -> Produced {
        self.builds += 1;
        // UI and grammar share this time. Strict reduced host normalization keeps
        // their own animations off; Stage policy controls only graphical placement.
        let grammar_env = UiEnvironment {
            width: self.size.0,
            height: self.size.1 - 8,
            motion: MotionPreference::None,
            ..Default::default()
        };
        let shown = self
            .grammar
            .present(&self.experience, &self.state, &grammar_env, now);
        let receipt = shown.receipt;
        let demand = shown.demand; // Actual producer output, never replaced by a constant.
        let mounted = map_actions(shown.element, Host::Inner);
        let before = spacer().width(self.x).grow(0.); // Fixed width: #413.
        let art = column().key("art").width(8).height(4).padding(0).gap(0);
        let sibling = spacer().key("sibling").grow(1.);
        let children = if self.reorder {
            vec![art, before, sibling]
        } else {
            vec![before, art, sibling]
        };
        let paints = Arc::clone(&self.paints);
        let tree = column()
            .key("host")
            .width(self.size.0)
            .height(self.size.1)
            .padding(0)
            .gap(0)
            .child(row().height(4).padding(0).gap(0).children(children))
            .child(
                column()
                    .key("info")
                    .height(1)
                    .padding(0)
                    .gap(0)
                    .child(label(self.reading.clone()).key("reading")),
            )
            .child(
                text_input(&self.editor)
                    .key("editor")
                    .on_edit(|s| Host::Edit(s.text)),
            )
            .child(button("Outside").key("outside").on_press(Host::Outside))
            .child(
                raw(Node::canvas(move |r| {
                    paints.fetch_add(1, Ordering::SeqCst);
                    Surface::new(r.width, r.height)
                })
                .height(1.))
                .key("probe"),
            )
            .child(
                column()
                    .key("experience")
                    .height(grammar_env.height)
                    .padding(0)
                    .gap(0)
                    .child(mounted),
            );
        let compiled = self
            .ui
            .frame(
                &tree,
                UiEnvironment {
                    width: self.size.0,
                    height: self.size.1,
                    motion: MotionPreference::None,
                    ..Default::default()
                },
                now,
            )
            .unwrap();
        let mut rgb = RgbRaster::new(self.size.0, 2 * self.size.1);
        for y in 0..rgb.height() {
            for x in 0..rgb.width() {
                rgb.set(x.into(), y.into(), self.substrate(x, y));
            }
        }
        let plane = RasterPlane {
            anchor: RasterAnchor::Key(key("art")),
            source: Arc::clone(&self.image),
            offset_px: (0, 0),
            z: 0,
        };
        let mut demands = vec![SourceDemand {
            key: key("experience"),
            demand,
        }];
        demands.push(SourceDemand {
            key: key("info"),
            demand: self.info,
        });
        let frame = self
            .driver
            .resolve_frame(
                now,
                FrameParts {
                    compiled,
                    viewport: self.size,
                    underlay: &rgb,
                    planes: &[plane],
                    focus: self.ui.focus(),
                    demands: &demands,
                    omissions: &receipt.omitted,
                    motion: self.motion,
                },
                RasterLimits {
                    planes: 4,
                    nodes: 4096,
                    pixels: 100_000,
                },
            )
            .unwrap();
        Produced {
            frame,
            receipt,
            demand,
        }
    }
    fn submit(&mut self, p: &mut Produced) {
        let mut node = p.frame.to_node();
        gibson::compute_layout(&mut node, self.size.0, self.size.1).unwrap();
        let mut cells = Surface::new(self.size.0, self.size.1);
        let paint = gibson::paint(&node, &mut cells);
        assert_eq!(
            cells, p.frame.snapshot.cells,
            "ordinary Context-node replay cells"
        );
        assert_eq!(paint.cursor_position, p.frame.snapshot.cursor_position);
        self.ctx.set_root(p.frame.to_node());
        let report = self.ctx.render_now().unwrap();
        assert_eq!(self.ctx.frame_size(), Some(self.size));
        assert_eq!(report.cursor_position, p.frame.snapshot.cursor_position);
        p.frame.snapshot.frame_report = Some(self.ctx.last_frame_report());
    }
    // The existing scoped #411 recipe. Enter/pointer/Tab are UI-first. The host
    // alone knows pane ownership; Stage never receives the event.
    fn route(&mut self, p: &Produced, input: &Event) -> EventOutcome<Host> {
        let capture = p.frame.compiled.interactions.active_modal().is_some()
            || self
                .ui
                .focus()
                .and_then(|k| p.frame.compiled.interactions.get(k))
                .is_some_and(|i| i.input.is_some() || i.on_event.is_some());
        let owns = !capture
            && self
                .ui
                .focus()
                .is_none_or(|k| p.receipt.actions.contains(k));
        let selection = owns
            && matches!(input, Event::Key(k) if matches!(k.code,
            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End | KeyCode::Char('h'|'j'|'k'|'l')));
        if selection {
            let out = self.grammar_route(input);
            if out.consumed {
                return out;
            }
            return self.ui.handle_event(input);
        }
        let out = self.ui.handle_event(input);
        if out.consumed || !owns {
            out
        } else {
            self.grammar_route(input)
        }
    }
    fn grammar_route(&mut self, input: &Event) -> EventOutcome<Host> {
        let Event::Key(k) = input else {
            return EventOutcome::default();
        };
        match handle_key_outcome(self.grammar.as_ref(), &self.experience, &mut self.state, k) {
            KeyOutcome::Action(a) => EventOutcome {
                actions: vec![Host::Inner(a)],
                consumed: true,
            },
            KeyOutcome::Consumed => EventOutcome {
                actions: vec![],
                consumed: true,
            },
            KeyOutcome::Unhandled => EventOutcome::default(),
        }
    }
    fn idle(&mut self, now: Duration) {
        assert!(
            self.ctx.scheduler.frame_due(),
            "idle test establishes Context permission before work"
        );
        let builds = self.builds;
        let paints = self.paints.load(Ordering::SeqCst);
        if self.driver.next_deadline().is_some_and(|at| at <= now) {
            let mut p = self.resolve(now);
            self.submit(&mut p);
        }
        assert!(
            self.ctx.scheduler.frame_due(),
            "idle oracle is not hidden by FPS throttling"
        );
        assert_eq!(
            self.builds, builds,
            "settled tick must not construct producers/resolve Stage"
        );
        assert_eq!(
            self.paints.load(Ordering::SeqCst),
            paints,
            "actual callback generation must remain zero"
        );
        assert!(!self.ctx.render_if_due().unwrap());
        assert!(self.ctx.take_rendered_bytes().is_empty());
    }
}
fn typed_activation(f: &mut Fixture, p: &Produced, id: &str) {
    let k = key(&format!("open-{id}"));
    assert!(p.receipt.actions.contains(&k));
    assert!(p
        .frame
        .snapshot
        .targets
        .iter()
        .any(|t| t.key == k && !t.rect.is_empty()));
    assert_eq!(
        p.frame.compiled.interactions.get(&k).unwrap().on_press,
        Some(Host::Inner(Inner::Open(id.into())))
    );
    assert!(f.ui.set_focus(&k));
    let out = f.route(p, &event(KeyCode::Enter));
    assert!(out.consumed);
    assert_eq!(
        out.actions,
        [Host::Inner(Inner::Open(id.into()))],
        "one typed activation exactly once"
    );
}
fn pixels(f: &Fixture, p: &Produced, x: u16) {
    let snap = &p.frame.snapshot;
    assert_eq!(
        snap.planes[0].presented.x,
        i32::from(x),
        "sprite follows its semantic art key"
    );
    assert_eq!(
        snap.pixels.get(x.into(), 0),
        Some((255, 255, 255)),
        "actual asymmetric marker"
    );
    assert_eq!(
        snap.pixels.get(i32::from(x + 2), 0),
        Some(f.substrate(x + 2, 0)),
        "alpha0 preserves current RGB"
    );
    let bg = f.substrate(x + 1, 1);
    let blend = |s: u8, d: u8| ((u32::from(s) * 128 + u32::from(d) * 127 + 127) / 255) as u8;
    assert_eq!(
        snap.pixels.get(i32::from(x + 1), 1),
        Some((blend(200, bg.0), blend(100, bg.1), blend(0, bg.2)))
    );
    assert_eq!(
        snap.pixels.get(i32::from(x + 3), 2),
        Some((240, 0, 0)),
        "alpha255 replaces RGB"
    );
}

#[test]
fn all_three_seams_preserve_actions_pixels_deadlines_and_settled_work() {
    for grammar in grammars() {
        let mut f = Fixture::new(grammar, MotionPreference::Full);
        let mut first = f.resolve(ms(0));
        assert_eq!(first.demand, FrameDemand::OnChange);
        assert_eq!(
            first.frame.snapshot.next_deadline,
            Some(ms(100)),
            "complete producer roster arms informational deadline"
        );
        pixels(&f, &first, 0);
        typed_activation(&mut f, &first, "a");
        f.submit(&mut first);
        assert!(!f.ctx.take_rendered_bytes().is_empty());
        let nav = if f.grammar.name() == "BLADES" {
            KeyCode::Right
        } else {
            KeyCode::Down
        };
        assert!(f.route(&first, &event(nav)).consumed);
        assert_eq!(f.state.selected_key(&f.experience), Some(key("b")));
        let p90 = f.resolve(ms(90));
        assert_eq!(
            p90.frame.snapshot.next_deadline,
            Some(ms(100)),
            "90ms navigation cannot renew source100"
        );
        typed_activation(&mut f, &p90, "b");
        f.info = FrameDemand::OnChange;
        let p100 = f.resolve(ms(100));
        assert_eq!(p100.frame.snapshot.due_sources, [key("info")]);
        assert_eq!(p100.frame.snapshot.next_deadline, None);
        assert!(f.ui.set_focus(&key("editor")));
        let edit = f.route(&p100, &event(KeyCode::Char('x')));
        assert!(edit.consumed && edit.actions.iter().any(|a| matches!(a, Host::Edit(_))));
        for action in &edit.actions {
            if let Host::Edit(text) = action {
                f.editor = TextInputState::with_text(text);
            }
        }
        assert_eq!(f.state.selected_key(&f.experience), Some(key("b")));
        assert!(f.ui.set_focus(&key("outside")));
        assert_eq!(
            f.route(&p100, &event(KeyCode::Enter)).actions,
            [Host::Outside]
        );

        f.x = 12;
        let p200 = f.resolve(ms(200));
        assert_eq!(p200.frame.snapshot.planes[0].layout_rect.x, 12);
        assert_eq!(
            p200.frame.snapshot.keys[&key("sibling")]
                .layout_rect
                .unwrap()
                .x,
            20,
            "semantic room is immediate"
        );
        pixels(&f, &p200, 0);
        let p300 = f.resolve(ms(300));
        pixels(&f, &p300, 6);
        f.x = 2;
        f.phase = 1;
        let reversed = f.resolve(ms(300));
        assert_eq!(
            reversed.frame.snapshot.planes[0].presented,
            p300.frame.snapshot.planes[0].presented
        );
        pixels(&f, &reversed, 6);
        let mut settled = f.resolve(ms(500));
        pixels(&f, &settled, 2);
        assert_eq!(settled.frame.snapshot.next_deadline, None);
        assert!(f.ui.set_focus(&key("editor")));
        settled = f.resolve(ms(500));
        assert_eq!(settled.frame.snapshot.focus, Some(key("editor")));
        assert!(settled.frame.snapshot.cursor_position.is_some());
        assert_eq!(
            settled
                .frame
                .compiled
                .interactions
                .get(&key("editor"))
                .unwrap()
                .input
                .as_ref()
                .unwrap()
                .text,
            f.editor.text
        );
        assert!(f.editor.text.contains('x'));
        f.submit(&mut settled);
        assert!(settled.frame.snapshot.frame_report.is_some());
        f.ctx.take_rendered_bytes();
        let builds = f.builds;
        let paints = f.paints.load(Ordering::SeqCst);
        assert!(builds > 0 && paints > 0);
        for now in [600, 1000, 5000] {
            f.idle(ms(now));
            assert_eq!(
                f.builds, builds,
                "settled ticks must not construct producers or resolve Stage"
            );
            assert_eq!(
                f.paints.load(Ordering::SeqCst),
                paints,
                "zero diff is not zero generation"
            );
        }
        f.reading = "Changed synthetic reading".into();
        let mut changed = f.resolve(ms(5000));
        f.submit(&mut changed);
        assert!(f.builds > builds && f.paints.load(Ordering::SeqCst) > paints);
        assert!(
            !f.ctx.take_rendered_bytes().is_empty(),
            "explicit change emits bytes"
        );

        f.size = (100, 32);
        f.ctx.resize(f.size.0, f.size.1).unwrap();
        f.reorder = true;
        f.experience = model(&["b", "a"]);
        let resized = f.resolve(ms(5100));
        assert_eq!(f.state.selected_key(&f.experience), Some(key("b")));
        assert_eq!(resized.frame.snapshot.planes[0].layout_rect.x, 0);
        typed_activation(&mut f, &resized, "b");
        let mut end = f.resolve(ms(5300));
        pixels(&f, &end, 0);
        f.submit(&mut end);
        assert_eq!(end.frame.snapshot.next_deadline, None);
    }
}

#[test]
fn strict_reduced_and_none_keep_literal_timers_and_declared_narrow_omissions() {
    for motion in [MotionPreference::Reduced, MotionPreference::None] {
        for grammar in grammars() {
            let mut f = Fixture::new(grammar, motion);
            f.resolve(ms(0));
            f.x = 12;
            let snap = f.resolve(ms(90));
            pixels(&f, &snap, 12);
            assert_eq!(
                snap.frame.snapshot.next_deadline,
                Some(ms(100)),
                "only informational timer survives strict reduced"
            );
            assert_eq!(f.ui.active_animation_count(), 0);
            f.info = FrameDemand::OnChange;
            let consumed = f.resolve(ms(100));
            assert_eq!(consumed.frame.snapshot.due_sources, [key("info")]);
            assert_eq!(consumed.frame.snapshot.next_deadline, None);
            f.size = (40, 15);
            f.ctx.resize(40, 15).unwrap();
            let mut narrow = f.resolve(ms(110));
            assert_eq!(narrow.frame.snapshot.next_deadline, None);
            assert_eq!(narrow.frame.snapshot.omissions, narrow.receipt.omitted);
            assert!(narrow.frame.snapshot.keys.contains_key(&key("editor")));
            assert!(narrow
                .frame
                .snapshot
                .targets
                .iter()
                .any(|t| t.key == key("outside") && !t.rect.is_empty()));
            for omitted in &narrow.receipt.omitted {
                assert!(narrow.frame.compiled.interactions.get(omitted).is_none());
            }
            if f.grammar.name() == "BLADES" {
                assert!(narrow.receipt.omitted.contains(&key("open-a")));
            }
            f.submit(&mut narrow);
        }
    }
}
