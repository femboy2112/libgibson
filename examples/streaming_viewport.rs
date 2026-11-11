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
    byte_limit: usize,
    row_limit: usize,
    anchor: Option<(u64, usize)>,
    mutation: bool,
}
impl Flow {
    fn new(width: u16, retain: usize, mutation: bool) -> Self {
        Self::with_budgets(width, retain, 32 * 1024, 4096, mutation)
    }
    fn with_budgets(
        width: u16,
        retain: usize,
        byte_limit: usize,
        row_limit: usize,
        mutation: bool,
    ) -> Self {
        assert!(width > 0 && retain > 0 && byte_limit > 0 && row_limit > 0);
        Self {
            blocks: VecDeque::new(),
            lines: BTreeMap::new(),
            width,
            retain,
            byte_limit,
            row_limit,
            anchor: None,
            mutation,
        }
    }
    fn source_is_safe(source: &str) -> bool {
        source
            .chars()
            .all(|c| !c.is_control() || c == '\n' || c == '\t')
    }
    // Source and cached-row budgets constrain retained values. They are not a
    // process-RSS cap: the caller owns the input String and selection clones rows.
    fn prepare(&self, source: &str) -> Result<Vec<String>, &'static str> {
        // Reject oversized input before scanning or wrapping it. The caller owns
        // the supplied String and its allocation is outside this budget.
        if source.len() > self.byte_limit || source.len() > 8192 {
            return Err("source byte budget exceeded");
        }
        if !Self::source_is_safe(source) {
            return Err("terminal controls rejected");
        }
        let rows = wrap_text(source, WrapMode::WordWrap, self.width);
        if rows.len() > self.row_limit {
            return Err("rendered row budget exceeded");
        }
        Ok(rows)
    }
    fn retained_bytes(&self) -> usize {
        self.blocks.iter().map(|b| b.source.len()).sum()
    }
    fn cached_rows(&self) -> usize {
        self.lines.values().map(Vec::len).sum()
    }
    fn append(&mut self, id: u64, source: String) -> Result<Vec<u64>, &'static str> {
        if self.blocks.iter().any(|b| b.id == id) {
            return Err("duplicate block id");
        }
        let rows = self.prepare(&source)?;
        let mut evictions = Vec::new();
        let mut evicted_anchor = false;
        while self.blocks.len() >= self.retain
            || self.retained_bytes() > self.byte_limit - source.len()
            || self.cached_rows() > self.row_limit - rows.len()
        {
            let old = self
                .blocks
                .pop_front()
                .expect("prepared block fits by itself");
            self.lines.remove(&old.id);
            evictions.push(old.id);
            if self.anchor.map(|a| a.0) == Some(old.id) {
                evicted_anchor = true;
                self.anchor = self.blocks.front().map(|b| (b.id, 0));
            }
        }
        self.blocks.push_back(Block { id, source });
        if evicted_anchor && self.anchor.is_none() {
            self.anchor = Some((id, 0));
        }
        self.lines.insert(id, rows);
        Ok(evictions)
    }
    fn replace(&mut self, id: u64, source: String) -> Result<(), &'static str> {
        let index = self
            .blocks
            .iter()
            .position(|b| b.id == id)
            .ok_or("unknown block")?;
        let rows = self.prepare(&source)?;
        let remaining_bytes = self.retained_bytes() - self.blocks[index].source.len();
        let remaining_rows = self.cached_rows() - self.lines[&id].len();
        if remaining_bytes > self.byte_limit - source.len() {
            return Err("source byte budget exceeded");
        }
        if remaining_rows > self.row_limit - rows.len() {
            return Err("rendered row budget exceeded");
        }
        self.blocks[index].source = source;
        if !self.mutation {
            self.lines.insert(id, rows);
        }
        Ok(())
    }
    // Reject a resize that cannot fit without evicting content. On error the
    // prior width, source, cache, and anchor are unchanged.
    fn resize(&mut self, width: u16) -> Result<(), &'static str> {
        if width == 0 {
            return Err("zero width");
        }
        if self.width != width {
            let mut next = BTreeMap::new();
            let mut count = 0usize;
            for block in &self.blocks {
                let rows = wrap_text(&block.source, WrapMode::WordWrap, width);
                if rows.len() > self.row_limit - count {
                    return Err("rendered row budget exceeded on resize");
                }
                count += rows.len();
                next.insert(block.id, rows);
            }
            self.width = width;
            self.lines = next;
        }
        Ok(())
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
    flow.resize(width).map_err(std::io::Error::other)?;
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
// A synthetic local timing probe, not a benchmark of a terminal or downstream app.
fn run_workloads() {
    use std::time::{Duration, Instant};
    for width in [38, 118] {
        for turns in [1_u64, 10, 100, 1000] {
            let mut flow = Flow::with_budgets(width, turns as usize + 1, 1_000_000, 200_000, false);
            for id in 1..=turns {
                let source = format!(
                    "TURN {id:04}: code e\u{301} 東京 👩‍🔬; {}\n",
                    "let value = 12345; ".repeat(4)
                );
                flow.append(id, source).unwrap();
            }
            flow.append(turns + 1, "LIVE".into()).unwrap();
            flow.anchor = Some((turns.saturating_sub(100).max(1), 0));
            let held = flow.window(1);
            let mut incremental = Duration::ZERO;
            let mut rebuilt = Duration::ZERO;
            for chunk in 1..=20 {
                let source = format!("LIVE {chunk:03} {}", "token ".repeat(chunk));
                let begin = Instant::now();
                flow.replace(turns + 1, source).unwrap();
                let actual = flow.all_rows();
                incremental += begin.elapsed();
                let begin = Instant::now();
                let reference = flow.reference_rows();
                rebuilt += begin.elapsed();
                assert_eq!(actual, reference, "cached and rebuilt workloads diverged");
                assert_eq!(flow.window(1), held, "held history moved");
            }
            println!("width={width} turns={turns} chunks=20 source_bytes={} cached_rows={} incremental_us={} rebuilt_us={}",
                flow.retained_bytes(), flow.cached_rows(), incremental.as_micros(), rebuilt.as_micros());
        }
    }
}
fn main() -> std::io::Result<()> {
    if std::env::args().any(|a| a == "--workload") {
        run_workloads();
        return Ok(());
    }
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

#[cfg(test)]
mod budget_tests {
    use super::Flow;

    #[test]
    fn single_growing_block_cannot_exceed_source_budget() {
        let mut flow = Flow::with_budgets(4, 4, 10, 8, false);
        flow.append(1, "abcd".into()).unwrap();
        let before = flow.all_rows();
        assert!(flow.replace(1, "01234567890".into()).is_err());
        assert_eq!(flow.blocks[0].source, "abcd");
        assert_eq!(flow.all_rows(), before);
    }

    #[test]
    fn row_budget_rejects_growth_atomically() {
        let mut flow = Flow::with_budgets(2, 4, 100, 3, false);
        flow.append(1, "ab".into()).unwrap();
        flow.anchor = Some((1, 0));
        let before = flow.all_rows();
        assert!(flow.replace(1, "a\nb\nc\nd".into()).is_err());
        assert_eq!(flow.all_rows(), before);
        assert_eq!(flow.anchor, Some((1, 0)));
    }

    #[test]
    fn append_evicts_oldest_with_receipt_and_held_anchor() {
        let mut flow = Flow::with_budgets(8, 4, 8, 8, false);
        flow.append(1, "abcd".into()).unwrap();
        flow.append(2, "efgh".into()).unwrap();
        flow.anchor = Some((1, 0));
        assert_eq!(flow.append(3, "ijk".into()).unwrap(), vec![1]);
        assert_eq!(flow.anchor, Some((2, 0)));
        assert_eq!(
            flow.all_rows(),
            vec![(2, 0, "efgh".into()), (3, 0, "ijk".into())]
        );
    }

    #[test]
    fn resize_refuses_excess_rows_without_changing_width() {
        let mut flow = Flow::with_budgets(4, 4, 32, 2, false);
        flow.append(1, "abcdefgh".into()).unwrap();
        let before = flow.all_rows();
        assert!(flow.resize(2).is_err());
        assert_eq!(flow.width, 4);
        assert_eq!(flow.all_rows(), before);
    }
    #[test]
    fn explicit_unicode_wrapping_fixtures() {
        let mut flow = Flow::with_budgets(3, 4, 100, 10, false);
        flow.append(1, "a界b".into()).unwrap();
        assert_eq!(
            flow.all_rows(),
            vec![(1, 0, "a界".into()), (1, 1, "b".into())]
        );
        flow.resize(1).unwrap();
        flow.append(2, "e\u{301}x".into()).unwrap();
        let rows = flow.all_rows();
        assert_eq!(&rows[3..], &[(2, 0, "e\u{301}".into()), (2, 1, "x".into())]);
    }

    #[test]
    fn oversized_append_cannot_evict_anything() {
        let mut flow = Flow::with_budgets(4, 1, 4, 4, false);
        flow.append(1, "good".into()).unwrap();
        flow.anchor = Some((1, 0));
        assert!(flow.append(2, "12345".into()).is_err());
        assert_eq!(flow.blocks[0].source, "good");
        assert_eq!(flow.anchor, Some((1, 0)));
    }

    #[test]
    fn anchor_moves_to_new_block_if_all_old_blocks_evict() {
        let mut flow = Flow::with_budgets(4, 1, 4, 4, false);
        flow.append(1, "good".into()).unwrap();
        flow.anchor = Some((1, 0));
        assert_eq!(flow.append(2, "next".into()).unwrap(), vec![1]);
        assert_eq!(flow.anchor, Some((2, 0)));
    }
    #[test]
    fn retained_source_survives_unicode_reflow_and_rejected_update() {
        let original = "e\u{301} 界👩‍🔬\n code  \n";
        let mut flow = Flow::with_budgets(20, 4, 100, 100, false);
        flow.append(7, original.into()).unwrap();
        flow.resize(3).unwrap();
        assert_eq!(flow.blocks[0].source, original);
        assert!(flow.replace(7, "bad\u{1b}[2J".into()).is_err());
        assert_eq!(flow.blocks[0].source, original);
    }
    #[test]
    fn individual_block_limit_applies_even_if_total_budget_is_larger() {
        let mut flow = Flow::with_budgets(80, 3, 20_000, 10_000, false);
        flow.append(1, "safe".into()).unwrap();
        assert!(flow.replace(1, "x".repeat(8193)).is_err());
        assert_eq!(flow.blocks[0].source, "safe");
    }
    #[test]
    fn aggregate_row_budget_evicts_on_append_and_rejects_replace() {
        let mut flow = Flow::with_budgets(4, 4, 100, 2, false);
        flow.append(1, "a".into()).unwrap();
        flow.append(2, "b".into()).unwrap();
        flow.anchor = Some((1, 0));
        assert_eq!(flow.append(3, "c".into()).unwrap(), vec![1]);
        assert_eq!(flow.anchor, Some((2, 0)));
        assert!(flow.replace(2, "b\nb".into()).is_err());
        assert_eq!(
            flow.all_rows(),
            vec![(2, 0, "b".into()), (3, 0, "c".into())]
        );
    }
}
