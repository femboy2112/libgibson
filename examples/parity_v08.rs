//! LibGibson v0.8 parity slice -- Rust consumer.
//!
//! Interprets `bindings/parity/v08/script.txt` with the gibson crate's NATIVE Rust API
//! (`Context::headless`, `Node`, `InjectedQueue` + `Context::poll_from`, `Context::frame_cell`,
//! `Context::rendered_bytes`) -- no `gibson::ffi` call -- and writes the canonical transcript to
//! stdout. Format and record schema: `docs/V08_PARITY_SLICE.md`.
//!
//! ```text
//! cargo +1.98.1 run --release --example parity_v08 -- bindings/parity/v08/script.txt [REPS]
//! ```
//!
//! Where the Rust API has no equivalent of a C ABI operation (NULL pointers, raw enum integers,
//! invalid UTF-8, the inline glyph cap, the kind-less legacy key path), the record says
//! `"st":"UNSUPPORTED"` with a `why`; where Rust rejects an input natively it reports the Rust
//! outcome (`RUST_NONE` / `RUST_ERR`) and where it accepts what the C ABI rejects, `RUST_ACCEPTED`.
//! Those differences are data, not failures.
//!
//! A delivered `Resize` resizes the headless context inside `poll_from`, the same implementation
//! the C ABI's `gibson_poll_event` uses (#293), so this consumer takes no extra step.
//! `PARITY_V08_RUST_RESIZE=unapplied` (a negative control in `scripts/dev/parity_v08.sh`, never on
//! by default) puts the pre-resize geometry back after the poll, reproducing the behaviour before
//! #293, so the comparator must catch r02/r03/r04.
//!
//! Negative controls (never on by default): `PARITY_V08_PERTURB=cell | events` (see parity_v08.c).

use gibson::cell::{Color, Style};
use gibson::context::{Context, FrameCell, RenderMode};
use gibson::event_source::InjectedQueue;
use gibson::input::{Event, KeyCode, KeyEvent, KeyModifiers};
use gibson::node::{Node, WrapMode};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;
/// Mirrors GIBSON_EVENT_QUEUE_CAPACITY; in Rust the caller chooses the capacity.
const QUEUE_CAPACITY: usize = 1024;

fn fnv(mut h: u64, data: &[u8]) -> u64 {
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

/// One transcript record; `BTreeMap` keeps the keys sorted bytewise.
#[derive(Default)]
struct Rec(BTreeMap<&'static str, String>);

impl Rec {
    fn i(mut self, k: &'static str, v: i64) -> Self {
        self.0.insert(k, v.to_string());
        self
    }
    fn s(mut self, k: &'static str, v: &str) -> Self {
        let safe = v
            .bytes()
            .all(|c| (0x20..=0x7e).contains(&c) && c != b'"' && c != b'\\');
        let v = if safe {
            v.to_string()
        } else {
            format!("hex:{}", hex(v.as_bytes()))
        };
        self.0.insert(k, format!("\"{v}\""));
        self
    }
    fn a4(mut self, k: &'static str, a: [i64; 4]) -> Self {
        self.0
            .insert(k, format!("[{},{},{},{}]", a[0], a[1], a[2], a[3]));
        self
    }
    fn emit(self, out: &mut String) {
        out.push('{');
        for (n, (k, v)) in self.0.iter().enumerate() {
            if n > 0 {
                out.push(',');
            }
            out.push_str(&format!("\"{k}\":{v}"));
        }
        out.push_str("}\n");
    }
}

const NAMED: [Color; 17] = [
    Color::Reset,
    Color::Black,
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::White,
    Color::BrightBlack,
    Color::BrightRed,
    Color::BrightGreen,
    Color::BrightYellow,
    Color::BrightBlue,
    Color::BrightMagenta,
    Color::BrightCyan,
    Color::BrightWhite,
];

fn parse_color(spec: &str) -> Option<Color> {
    if spec == "reset" {
        return None;
    }
    if let Some(rgb) = spec.strip_prefix("rgb:") {
        let p: Vec<u8> = rgb.split(',').map(|x| x.parse().unwrap()).collect();
        return Some(Color::Rgb(p[0], p[1], p[2]));
    }
    if let Some(n) = spec.strip_prefix("ansi:") {
        return Some(Color::Ansi256(n.parse().unwrap()));
    }
    spec.strip_prefix("named:")
        .map(|n| NAMED[n.parse::<usize>().unwrap()])
}

/// The canonical `[color_type, r, g, b]` (the C ABI's numbering; `None` and `Reset` are 0).
fn c4(c: Option<Color>) -> [i64; 4] {
    match c {
        None | Some(Color::Reset) => [0, 0, 0, 0],
        Some(Color::Ansi256(n)) => [17, n as i64, 0, 0],
        Some(Color::Rgb(r, g, b)) => [18, r as i64, g as i64, b as i64],
        Some(named) => [
            NAMED.iter().position(|&x| x == named).unwrap() as i64,
            0,
            0,
            0,
        ],
    }
}

fn attrs(s: &Style) -> String {
    [s.bold, s.dim, s.italic, s.underline, s.reverse]
        .iter()
        .map(|&b| if b { '1' } else { '0' })
        .collect()
}

fn cell_line(c: &FrameCell<'_>) -> String {
    let (f, b) = (c4(c.style.fg), c4(c.style.bg));
    format!(
        "{};{};{};{};{},{},{},{};{},{},{},{};{}\n",
        hex(c.glyph.as_bytes()),
        c.width,
        c.is_continuation as u8,
        c.transparent as u8,
        f[0],
        f[1],
        f[2],
        f[3],
        b[0],
        b[1],
        b[2],
        b[3],
        attrs(&c.style)
    )
}

const SPECIAL: [(&str, KeyCode, u32); 13] = [
    ("ENTER", KeyCode::Enter, 0x1001),
    ("LEFT", KeyCode::Left, 0x1002),
    ("RIGHT", KeyCode::Right, 0x1003),
    ("UP", KeyCode::Up, 0x1004),
    ("DOWN", KeyCode::Down, 0x1005),
    ("HOME", KeyCode::Home, 0x1006),
    ("END", KeyCode::End, 0x1007),
    ("BACKSPACE", KeyCode::Backspace, 0x1008),
    ("DELETE", KeyCode::Delete, 0x1009),
    ("ESC", KeyCode::Esc, 0x100A),
    ("TAB", KeyCode::Tab, 0x100B),
    ("PAGE_UP", KeyCode::PageUp, 0x100C),
    ("PAGE_DOWN", KeyCode::PageDown, 0x100D),
];

/// A key's canonical (key_code, kind): a typed char is its scalar value; a special key is the
/// GIBSON_KEY_* value from include/gibson.h (BackTab folds to TAB exactly as the C ABI does).
fn key_code(code: KeyCode) -> (i64, &'static str) {
    if let KeyCode::Char(c) = code {
        return (c as i64, "CHAR");
    }
    if code == KeyCode::BackTab {
        return (0x100B, "SPECIAL");
    }
    match SPECIAL.iter().find(|(_, k, _)| *k == code) {
        Some((_, _, v)) => (*v as i64, "SPECIAL"),
        None => (0, "UNMAPPED"),
    }
}

fn unsupported(id: &str, k: &'static str, why: &str) -> Rec {
    Rec::default()
        .s("id", id)
        .s("k", k)
        .s("st", "UNSUPPORTED")
        .s("why", why)
}

struct Interp<'o> {
    out: &'o mut String,
    ctx: Option<Context>,
    root: Option<Node>,
    styles: BTreeMap<String, Style>,
    queue: InjectedQueue,
    unapply_resize: bool,
}

impl Interp<'_> {
    fn ctx(&mut self) -> &mut Context {
        self.ctx.as_mut().expect("no context")
    }

    fn emit_event(&mut self, id: &str, polled: std::io::Result<Option<Event>>) {
        let st = if polled.is_ok() { "OK" } else { "RUST_ERR" };
        let ev = polled.ok().flatten();
        let (ty, code, kind, mods, paste, cols, rows) = match &ev {
            Some(Event::Key(k)) => {
                let (code, kind) = key_code(k.code);
                (
                    "KEY",
                    code,
                    kind,
                    k.modifiers.bits() as i64,
                    String::new(),
                    0,
                    0,
                )
            }
            Some(Event::Paste(s)) => ("PASTE", 0, "NONE", 0, hex(s.as_bytes()), 0, 0),
            Some(Event::Resize(c, r)) => {
                ("RESIZE", 0, "NONE", 0, String::new(), *c as i64, *r as i64)
            }
            _ => ("NONE", 0, "NONE", 0, String::new(), 0, 0),
        };
        Rec::default()
            .i("cols", cols)
            .s("id", id)
            .s("k", "event")
            .i("key_code", code)
            .s("kind", kind)
            .i("mods", mods)
            .s("paste", &paste)
            .i("rows", rows)
            .s("st", st)
            .s("type", ty)
            .emit(self.out);
    }

    fn probe(&mut self, id: &str, name: &str) {
        let base = Rec::default().s("id", id).s("k", "probe").s("name", name);
        let (cols, rows) = self.ctx().frame_size().unwrap_or((0, 0));
        let unsup = |why: &str| base_clone(id, name).s("st", "UNSUPPORTED").s("why", why);
        let rec = match name {
            "create_bad_mode" => unsup("RenderMode is a closed enum"),
            "create_zero_cols" => {
                let prev = std::panic::take_hook();
                std::panic::set_hook(Box::new(|_| {}));
                let r = std::panic::catch_unwind(|| {
                    let mut c = Context::headless(RenderMode::Fullscreen, 0, 12);
                    c.set_root(Node::col().child(Node::text("x", Style::default())));
                    let ok = c.render().is_ok();
                    (ok, c.frame_size())
                });
                std::panic::set_hook(prev);
                let (render, size) = match r {
                    Ok((true, size)) => ("OK", size),
                    Ok((false, size)) => ("RUST_ERR", size),
                    Err(_) => ("PANIC", None),
                };
                let (c, r) = size.unwrap_or((0, 0));
                base.i("cols", c as i64)
                    .s("render", render)
                    .i("rows", r as i64)
                    .s("st", "RUST_ACCEPTED")
            }
            "create_null_out" => unsup("Context::headless returns the context; no out-pointer"),
            "render_null_ctx" | "cell_null_out" | "push_null_event" | "paste_null"
            | "poll_null_out" | "poll_kind_null" => unsup("references cannot be null"),
            "cell_oob_x" => {
                let st = if self.ctx().frame_cell(cols, 0).is_none() {
                    "RUST_NONE"
                } else {
                    "OK"
                };
                base.s("st", st)
            }
            "cell_oob_y" => {
                let st = if self.ctx().frame_cell(0, rows).is_none() {
                    "RUST_NONE"
                } else {
                    "OK"
                };
                base.s("st", st)
            }
            "cells_oob" => unsup("no bulk frame readback in the Rust API"),
            "text_bad_utf8" => unsup("str cannot hold invalid UTF-8"),
            "push_surrogate" => {
                let st = if char::from_u32(0xD800).is_none() {
                    "RUST_NONE"
                } else {
                    "OK"
                };
                base.s("st", st)
            }
            "push_special_not_key" => unsup("KeyCode is a closed enum"),
            "push_bad_kind" => unsup("no key-kind integer; the kind is the KeyCode variant"),
            "push_bad_mods" => {
                let st = if KeyModifiers::from_bits(0x80).is_none() {
                    "RUST_NONE"
                } else {
                    "OK"
                };
                base.s("st", st)
            }
            "push_resize_zero" => {
                // A scratch queue, so an accepted event cannot leak into the script's queue.
                let mut q = InjectedQueue::with_capacity(QUEUE_CAPACITY);
                let st = match q.push(Event::Resize(0, 12)) {
                    Ok(()) => "RUST_ACCEPTED",
                    Err(_) => "RUST_ERR",
                };
                base.s("st", st)
            }
            "paste_bad_utf8" => {
                let st = match String::from_utf8(vec![0xc3, 0x28]) {
                    Ok(_) => "OK",
                    Err(_) => "RUST_ERR",
                };
                base.s("st", st)
            }
            "color_depth_bad" => unsup("ColorDepth is a closed enum"),
            "queue_full" => {
                let mut accepted = 0i64;
                let q = Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty()));
                for _ in 0..QUEUE_CAPACITY {
                    if self.queue.push(q.clone()).is_ok() {
                        accepted += 1;
                    }
                }
                let (st, err) = match self.queue.push(q) {
                    Ok(()) => ("OK", String::new()),
                    Err(full) => ("RUST_ERR", full.to_string()),
                };
                let mut drained = 0i64;
                let ctx = self.ctx.as_mut().expect("no context");
                for _ in 0..2 * QUEUE_CAPACITY {
                    match ctx.poll_from(&mut self.queue, Duration::ZERO) {
                        Ok(Some(_)) => drained += 1,
                        _ => break,
                    }
                }
                base.i("accepted", accepted)
                    .i("drained", drained)
                    .s("err", &err)
                    .s("st", st)
            }
            "frame_before_render" => {
                let c2 = Context::headless(RenderMode::Fullscreen, 8, 2);
                let size = c2.frame_size();
                let (c, r) = size.unwrap_or((0, 0));
                let st = if c2.frame_cell(0, 0).is_none() {
                    "RUST_NONE"
                } else {
                    "OK"
                };
                base.i("cols", c as i64)
                    .i("rows", r as i64)
                    .s("size_st", if size.is_none() { "RUST_NONE" } else { "OK" })
                    .s("st", st)
            }
            _ => base.s("st", "UNKNOWN_PROBE"),
        };
        rec.emit(self.out);
    }

    fn step(&mut self, s: &[String]) {
        let (id, op, a) = (s[0].as_str(), s[2].as_str(), &s[3..]);
        let num = |i: usize| a[i].parse::<u16>().unwrap();
        match op {
            "create" => {
                let mode = if a[0] == "INLINE" {
                    RenderMode::Inline
                } else {
                    RenderMode::Fullscreen
                };
                self.ctx = Some(Context::headless(mode, num(1), num(2)));
                Rec::default()
                    .i("cols", num(1) as i64)
                    .s("id", id)
                    .s("k", "create")
                    .s("mode", &a[0])
                    .i("rows", num(2) as i64)
                    .s("st", "OK")
                    .emit(self.out);
            }
            "style" => {
                let f = a[3].as_bytes();
                let st = Style {
                    fg: parse_color(&a[1]),
                    bg: parse_color(&a[2]),
                    bold: f[0] == b'1',
                    dim: f[1] == b'1',
                    italic: f[2] == b'1',
                    underline: f[3] == b'1',
                    reverse: f[4] == b'1',
                };
                self.styles.insert(a[0].clone(), st);
            }
            "col" => {
                self.root = Some(Node::col());
                Rec::default()
                    .s("id", id)
                    .s("k", "node")
                    .s("st", "OK")
                    .emit(self.out);
            }
            "text" => {
                let wrap = match a[1].as_str() {
                    "CHAR" => WrapMode::CharWrap,
                    "WORD" => WrapMode::WordWrap,
                    _ => WrapMode::NoWrap,
                };
                let text = String::from_utf8(unhex(&a[2])).expect("script text is UTF-8");
                let style = self.styles.get(&a[0]).copied().unwrap_or_default();
                let node = Node::text_wrapped(text, style, wrap);
                self.root.as_mut().expect("no col").add_child(node);
                Rec::default()
                    .s("id", id)
                    .s("k", "node")
                    .s("st", "OK")
                    .emit(self.out);
            }
            "mount" => {
                let root = self.root.take().expect("no col");
                self.ctx().set_root(root);
                Rec::default()
                    .s("id", id)
                    .s("k", "mount")
                    .s("st", "OK")
                    .emit(self.out);
            }
            "render" => {
                let st = if self.ctx().render().is_ok() {
                    "OK"
                } else {
                    "RUST_ERR"
                };
                Rec::default()
                    .s("id", id)
                    .s("k", "render")
                    .s("st", st)
                    .emit(self.out);
            }
            "geom" => {
                let size = self.ctx().frame_size();
                let (c, r) = size.unwrap_or((0, 0));
                Rec::default()
                    .i("cols", c as i64)
                    .s("id", id)
                    .s("k", "geom")
                    .i("rows", r as i64)
                    .s("st", if size.is_some() { "OK" } else { "RUST_NONE" })
                    .emit(self.out);
            }
            "frame" => {
                let ctx = self.ctx.as_ref().expect("no context");
                let size = ctx.frame_size();
                let (c, r) = size.unwrap_or((0, 0));
                let mut h = FNV_OFFSET;
                let mut n = 0i64;
                for y in 0..r {
                    for x in 0..c {
                        let cell = ctx.frame_cell(x, y).expect("in-bounds cell");
                        h = fnv(h, cell_line(&cell).as_bytes());
                        n += 1;
                    }
                }
                let wire = ctx.rendered_bytes();
                Rec::default()
                    .i("cells", n)
                    .s("digest", &format!("{h:016x}"))
                    .s("id", id)
                    .s("k", "frame")
                    .s("st", if size.is_some() { "OK" } else { "RUST_NONE" })
                    .s("wire_digest", &format!("{:016x}", fnv(FNV_OFFSET, wire)))
                    .i("wire_len", wire.len() as i64)
                    .emit(self.out);
            }
            "cell" => {
                let (x, y) = (num(0), num(1));
                let ctx = self.ctx.as_ref().expect("no context");
                let rec = match ctx.frame_cell(x, y) {
                    Some(c) => Rec::default()
                        .s("attrs", &attrs(&c.style))
                        .a4("bg", c4(c.style.bg))
                        .i("cont", c.is_continuation as i64)
                        .a4("fg", c4(c.style.fg))
                        .s("glyph", &hex(c.glyph.as_bytes()))
                        .s("st", "OK")
                        .i("transp", c.transparent as i64)
                        .i("w", c.width as i64),
                    None => Rec::default().s("st", "RUST_NONE"),
                };
                rec.s("id", id)
                    .s("k", "cell")
                    .i("x", x as i64)
                    .i("y", y as i64)
                    .emit(self.out);
            }
            "glyph" => {
                let (x, y) = (num(0), num(1));
                let ctx = self.ctx.as_ref().expect("no context");
                let g = ctx
                    .frame_cell(x, y)
                    .map(|c| c.glyph.as_bytes().to_vec())
                    .unwrap_or_default();
                Rec::default()
                    .s("full_hex", &hex(&g))
                    .i("full_len", g.len() as i64)
                    .s("id", id)
                    .s("k", "glyph")
                    .s("st", "OK")
                    .i("x", x as i64)
                    .i("y", y as i64)
                    .emit(self.out);
            }
            "glyphcap" => unsupported(
                id,
                "glyphcap",
                "FrameCell.glyph is the full str; the 16-byte inline cap is a C ABI artifact",
            )
            .emit(self.out),
            "push_char" | "push_special" | "push_paste" | "push_resize" => {
                let ev = match op {
                    "push_char" => {
                        let cp = u32::from_str_radix(&a[0][2..], 16).unwrap();
                        let mods = KeyModifiers::from_bits(a[1].parse().unwrap()).unwrap();
                        Event::Key(KeyEvent::new(
                            KeyCode::Char(char::from_u32(cp).unwrap()),
                            mods,
                        ))
                    }
                    "push_special" => {
                        let code = SPECIAL.iter().find(|(n, _, _)| *n == a[0]).unwrap().1;
                        let mods = KeyModifiers::from_bits(a[1].parse().unwrap()).unwrap();
                        Event::Key(KeyEvent::new(code, mods))
                    }
                    "push_paste" => Event::Paste(String::from_utf8(unhex(&a[0])).unwrap()),
                    _ => Event::Resize(num(0), num(1)),
                };
                let st = if self.queue.push(ev).is_ok() {
                    "OK"
                } else {
                    "RUST_ERR"
                };
                Rec::default()
                    .s("id", id)
                    .s("k", "push")
                    .s("st", st)
                    .emit(self.out);
            }
            "push_legacy" => unsupported(
                id,
                "push",
                "no kind-less key push: Event::Key carries a typed KeyCode",
            )
            .emit(self.out),
            "poll" => {
                for k in 1..=a[0].parse::<usize>().unwrap() {
                    let ctx = self.ctx.as_mut().expect("no context");
                    let before = ctx.session.terminal_size();
                    let polled = ctx.poll_from(&mut self.queue, Duration::ZERO);
                    if let Ok(Some(Event::Resize(..))) = &polled {
                        if self.unapply_resize {
                            // Negative control only: undo what poll_from applied (pre-#293).
                            ctx.session.set_terminal_size(before.0, before.1);
                        }
                    }
                    self.emit_event(&format!("{id}.{k}"), polled);
                }
            }
            "poll_legacy" => unsupported(
                id,
                "event",
                "no kind-less poll: a polled KeyCode always says char vs special",
            )
            .emit(self.out),
            "probe" => self.probe(id, &a[0]),
            "lifecycle" => {
                let rec = match a[0].as_str() {
                    "node_free_unattached" => {
                        drop(Node::text("unattached", Style::default()));
                        Rec::default().s("st", "OK")
                    }
                    "destroy_null" => Rec::default()
                        .s("st", "UNSUPPORTED")
                        .s("why", "there is no null Context to drop"),
                    _ => Rec::default().s("st", "UNKNOWN_LIFECYCLE"),
                };
                rec.s("id", id)
                    .s("k", "lifecycle")
                    .s("name", &a[0])
                    .emit(self.out);
            }
            "destroy" => {
                drop(self.ctx.take());
                Rec::default()
                    .s("id", id)
                    .s("k", "destroy")
                    .s("st", "OK")
                    .emit(self.out);
            }
            _ => unsupported(id, "unknown_op", "unknown op").emit(self.out),
        }
    }
}

fn base_clone(id: &str, name: &str) -> Rec {
    Rec::default().s("id", id).s("k", "probe").s("name", name)
}

fn run_script(steps: &[Vec<String>], unapply_resize: bool) -> String {
    let mut out = String::new();
    let mut it = Interp {
        out: &mut out,
        ctx: None,
        root: None,
        styles: BTreeMap::new(),
        queue: InjectedQueue::with_capacity(QUEUE_CAPACITY),
        unapply_resize,
    };
    for s in steps {
        it.step(s);
    }
    drop(it);
    out
}

fn maxrss_kb() -> i64 {
    // SAFETY: getrusage writes into a zeroed, correctly sized rusage it is handed.
    unsafe {
        let mut ru: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut ru);
        ru.ru_maxrss
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: parity_v08 SCRIPT [REPS]");
        std::process::exit(2);
    }
    let reps = args
        .get(2)
        .and_then(|r| r.parse::<usize>().ok())
        .unwrap_or(1)
        .max(1);
    let text = std::fs::read_to_string(&args[1]).expect("read script");
    let mut steps: Vec<Vec<String>> = text
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.split(' ').map(str::to_string).collect())
        .collect();
    match std::env::var("PARITY_V08_PERTURB").as_deref() {
        Ok("cell") => {
            for s in steps.iter_mut().filter(|s| s[0] == "s06") {
                let h = s.last_mut().unwrap();
                h.replace_range(h.len() - 2.., "3f");
            }
        }
        Ok("events") => {
            let a = steps.iter().position(|s| s[0] == "e01");
            let b = steps.iter().position(|s| s[0] == "e02");
            if let (Some(a), Some(b)) = (a, b) {
                steps.swap(a, b);
            }
        }
        _ => {}
    }
    let unapply_resize = std::env::var("PARITY_V08_RUST_RESIZE").as_deref() == Ok("unapplied");

    let t0 = Instant::now();
    let first = run_script(&steps, unapply_resize);
    let rss1 = maxrss_kb();
    let mut rss_mid = rss1;
    let mut identical = 1;
    for rep in 2..=reps {
        if run_script(&steps, unapply_resize) != first {
            identical = 0;
        }
        if rep == reps.div_ceil(2) {
            rss_mid = maxrss_kb();
        }
    }
    let wall = t0.elapsed().as_micros() as i64;
    print!("{first}");
    if reps > 1 {
        let mut line = String::new();
        Rec::default()
            .i("iter_identical", identical)
            .s("k", "resource")
            .s("lang", "rust")
            .i("maxrss_kb_1", rss1)
            .i("maxrss_kb_mid", rss_mid)
            .i("maxrss_kb_n", maxrss_kb())
            .i("per_rep_us", wall / reps as i64)
            .i("reps", reps as i64)
            .i("wall_ms", wall / 1000)
            .emit(&mut line);
        eprint!("{line}");
    }
}
