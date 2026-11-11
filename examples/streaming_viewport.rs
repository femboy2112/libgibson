//! Application-owned bounded line-cache experiment, using public LibGibson APIs only.
//! Anchor contract: stable block and visual row at one width; resize preserves block,
//! then clamps the row. This does not claim source-byte anchoring across reflow.
use gibson::{
    cell::Style,
    context::{Context, RenderMode},
    node::Node,
    wrap_text, WrapMode,
};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone)]
struct Block {
    id: u64,
    source: String,
}
struct Flow {
    blocks: VecDeque<Block>,
    lines: BTreeMap<u64, Vec<String>>,
    width: u16,
    retain: usize,
    anchor: Option<(u64, usize)>,
    mutation: bool,
}
impl Flow {
    fn new(width: u16, retain: usize, mutation: bool) -> Self {
        assert!(width > 0 && retain > 0);
        Self {
            blocks: VecDeque::new(),
            lines: BTreeMap::new(),
            width,
            retain,
            anchor: None,
            mutation,
        }
    }
    fn source_is_safe(source: &str) -> bool {
        source
            .chars()
            .all(|c| !c.is_control() || c == '\n' || c == '\t')
    }
    fn append(&mut self, id: u64, source: String) -> Result<Vec<u64>, &'static str> {
        if !Self::source_is_safe(&source) {
            return Err("terminal controls rejected");
        }
        if self.blocks.iter().any(|b| b.id == id) {
            return Err("duplicate block id");
        }
        let mut evictions = Vec::new();
        self.blocks.push_back(Block { id, source });
        while self.blocks.len() > self.retain {
            let old = self.blocks.pop_front().unwrap();
            self.lines.remove(&old.id);
            evictions.push(old.id);
            if self.anchor.map(|a| a.0) == Some(old.id) {
                self.anchor = self.blocks.front().map(|b| (b.id, 0));
            }
        }
        Ok(evictions)
    }
    fn replace(&mut self, id: u64, source: String) -> Result<(), &'static str> {
        if !Self::source_is_safe(&source) {
            return Err("terminal controls rejected");
        }
        let block = self
            .blocks
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or("unknown block")?;
        block.source = source;
        if !self.mutation {
            self.lines.remove(&id);
        }
        Ok(())
    }
    fn resize(&mut self, width: u16) {
        assert!(width > 0);
        if self.width != width {
            self.width = width;
            self.lines.clear();
        }
    }
    fn all_rows(&mut self) -> Vec<(u64, usize, String)> {
        let mut result = Vec::new();
        for block in &self.blocks {
            let rows = self
                .lines
                .entry(block.id)
                .or_insert_with(|| wrap_text(&block.source, WrapMode::WordWrap, self.width));
            result.extend(
                rows.iter()
                    .enumerate()
                    .map(|(row, text)| (block.id, row, text.clone())),
            );
        }
        assert!(self.lines.len() <= self.retain, "cache exceeds retention");
        result
    }
    fn reference_rows(&self) -> Vec<(u64, usize, String)> {
        self.blocks
            .iter()
            .flat_map(|b| {
                wrap_text(&b.source, WrapMode::WordWrap, self.width)
                    .into_iter()
                    .enumerate()
                    .map(move |(r, t)| (b.id, r, t))
            })
            .collect()
    }
    fn window(&mut self, height: usize) -> Vec<String> {
        let rows = self.all_rows();
        let start = match self.anchor {
            None => rows.len().saturating_sub(height),
            Some((id, row)) => rows
                .iter()
                .position(|r| r.0 == id && r.1 >= row)
                .or_else(|| rows.iter().rposition(|r| r.0 == id))
                .unwrap_or(0),
        };
        rows.into_iter()
            .skip(start)
            .take(height)
            .map(|r| r.2)
            .collect()
    }
}
fn check_frame(flow: &mut Flow, width: u16, height: u16) -> std::io::Result<()> {
    flow.resize(width);
    let body = flow.window(usize::from(height - 2));
    let mut nodes = vec![Node::text("HOST STATUS: review pending", Style::default())];
    for n in 0..height - 2 {
        nodes.push(Node::text(
            body.get(usize::from(n)).cloned().unwrap_or_default(),
            Style::default(),
        ));
    }
    nodes.push(Node::text("INPUT >", Style::default()));
    let mut root = Node::col();
    for node in nodes {
        root = root.child(node);
    }
    let mut context = Context::headless(RenderMode::Fullscreen, width, height);
    context.set_root(root);
    context.render()?;
    for (row, expected) in [(0, "HOST STATUS: review pending"), (height - 1, "INPUT >")] {
        for (col, c) in expected.chars().enumerate() {
            let actual = context
                .frame_cell(col as u16, row)
                .unwrap()
                .glyph
                .to_owned();
            assert_eq!(actual, c.to_string(), "protected cell ({col},{row})");
        }
    }
    Ok(())
}
fn main() -> std::io::Result<()> {
    let mutation = std::env::args().any(|a| a == "--mutate-stale-cache");
    let mut flow = Flow::new(38, 32, mutation);
    for id in 1..=24 {
        flow.append(
            id,
            format!("TURN {id:04}: synthetic paragraph e\u{301} 東京 👩‍🔬.\nlet value = {id};\n")
                .repeat(3),
        )
        .unwrap();
    }
    assert_eq!(flow.all_rows(), flow.reference_rows());
    flow.anchor = Some((12, 0));
    let held = flow.window(10);
    flow.append(25, "LIVE: first token".to_owned()).unwrap();
    for n in 1..=20 {
        flow.replace(
            25,
            format!("LIVE: chunk {n:03}\n{}", "new synthetic line\n".repeat(n)),
        )
        .unwrap();
        assert_eq!(
            flow.all_rows(),
            flow.reference_rows(),
            "cached layout diverged after replace"
        );
        assert_eq!(held, flow.window(10), "new stream moved held history");
    }
    for (width, height) in [(40, 15), (80, 24), (120, 40)] {
        check_frame(&mut flow, width, height)?;
        assert_eq!(
            flow.all_rows(),
            flow.reference_rows(),
            "resize left stale cache"
        );
        assert!(
            flow.window(10)[0].starts_with("TURN 0012"),
            "resize lost anchored block"
        );
    }
    let old_count = flow.blocks.len();
    assert!(flow.append(100, "hostile\u{1b}[2J".to_owned()).is_err());
    assert_eq!(flow.blocks.len(), old_count);
    let mut evictions = Vec::new();
    for id in 26..=70 {
        evictions.extend(
            flow.append(id, format!("TURN {id:04}: retained synthetic source"))
                .unwrap(),
        );
    }
    assert_eq!(flow.blocks.len(), 32);
    assert_eq!(evictions, (1..=38).collect::<Vec<_>>());
    assert_eq!(flow.anchor, Some((39, 0)));
    assert_eq!(flow.all_rows(), flow.reference_rows());
    println!("PASS: cached/reference rows, stream-held history, three headless sizes, protected cells, control rejection, bounded retention, eviction receipt");
    println!("LIMIT: cell/headless evidence only; resize preserves block identity, not exact source-byte position; no focus or real-terminal acceptance");
    Ok(())
}
