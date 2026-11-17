//! Public synthetic controls for full-height inline regrow reservation.
use core::num::NonZeroU16;
use gibson::renderer::Renderer;
use gibson::session::TerminalSession;
use gibson::{Context, InlineResizePolicy, Node, RenderMode, Style};
use std::io::{self, Write};

const SIZES: [(u16, u16); 18] = [
    (40, 15),
    (80, 24),
    (100, 32),
    (120, 40),
    (160, 48),
    (120, 40),
    (100, 32),
    (80, 24),
    (40, 15),
    (80, 24),
    (79, 23),
    (81, 25),
    (80, 24),
    (119, 39),
    (120, 40),
    (121, 41),
    (40, 15),
    (160, 48),
];
const KEEP: &str = "COMMITTED-KEEP";

fn nz(n: u16) -> NonZeroU16 {
    NonZeroU16::new(n).unwrap()
}
fn tree(step: usize, cols: u16, height: u16, cursor: Option<u16>) -> Node {
    Node::col()
        .width(cols as f32)
        .height(height as f32)
        .children(
            (0..height)
                .map(|row| {
                    let label = format!("LIVE-{step:02}-{row:02}");
                    let n = if cursor == Some(row) {
                        Node::text_input(label, 0, None, Style::default())
                    } else {
                        Node::text(label, Style::default())
                    };
                    n.width(cols as f32).height(1.0)
                })
                .collect(),
        )
}

struct Term {
    ctx: Context,
    vt: vt100::Parser,
    cols: u16,
    rows: u16,
}
impl Term {
    fn new(
        cols: u16,
        rows: u16,
        capacity: usize,
        mode: RenderMode,
        policy: InlineResizePolicy,
    ) -> Self {
        let mut ctx = Context::headless(mode, cols, rows);
        ctx.set_inline_resize_policy(policy);
        Self {
            ctx,
            vt: vt100::Parser::new(nz(rows), nz(cols), capacity),
            cols,
            rows,
        }
    }
    fn pump(&mut self) -> Vec<u8> {
        let bytes = self.ctx.take_rendered_bytes();
        self.vt.process(&bytes);
        bytes
    }
    fn resize(&mut self, cols: u16, rows: u16) {
        self.vt.set_size(nz(rows), nz(cols));
        self.ctx.resize(cols, rows).unwrap();
        self.cols = cols;
        self.rows = rows;
    }
    fn draw(&mut self, step: usize, height: u16, cursor: Option<u16>) -> Vec<u8> {
        self.ctx.set_root(tree(step, self.cols, height, cursor));
        self.ctx.render().unwrap();
        self.pump()
    }
    fn history(&mut self) -> Vec<String> {
        let screen = self.vt.screen_mut();
        screen.set_scrollback(usize::MAX);
        let count = screen.scrollback();
        let mut lines = Vec::new();
        for offset in (1..=count).rev() {
            screen.set_scrollback(offset);
            lines.push(screen.rows(0, self.cols).next().unwrap_or_default());
        }
        screen.set_scrollback(0);
        lines
    }
    fn assert_marker(&mut self, marker: &str) {
        let mut lines = self.history();
        lines.extend(self.vt.screen().rows(0, self.cols));
        assert_eq!(
            lines
                .iter()
                .filter(|line| line.trim_end() == marker)
                .count(),
            1,
            "committed marker {marker} lost or duplicated"
        );
    }
}

fn cells(vt: &vt100::Parser, cols: u16, rows: u16) -> Vec<String> {
    (0..rows)
        .flat_map(|r| (0..cols).map(move |c| format!("{:?}", vt.screen().cell(r, c).unwrap())))
        .collect()
}

#[test]
fn full_height_reclaim_matches_fresh_through_the_18_step_regrow_sequence() {
    for capacity in [0, 4096] {
        let mut actual = Term::new(
            40,
            15,
            capacity,
            RenderMode::Inline,
            InlineResizePolicy::ReclaimLiveRows,
        );
        for (step, (cols, rows)) in SIZES.into_iter().enumerate() {
            actual.resize(cols, rows);
            actual.draw(step, rows - 1, None);
            let mut fresh = Term::new(
                cols,
                rows,
                capacity,
                RenderMode::Inline,
                InlineResizePolicy::ReclaimLiveRows,
            );
            fresh.draw(step, rows - 1, None);
            assert_eq!(
                actual.vt.screen().cursor_position(),
                fresh.vt.screen().cursor_position(),
                "capacity={capacity}, step={step}, report={:?}",
                actual.ctx.last_frame_report()
            );
            assert_eq!(
                cells(&actual.vt, cols, rows),
                cells(&fresh.vt, cols, rows),
                "capacity={capacity}, step={step}"
            );
        }
    }
}

#[test]
fn fullscreen_control_matches_fresh_for_the_same_resize_sequence() {
    let mut actual = Term::new(
        40,
        15,
        0,
        RenderMode::Fullscreen,
        InlineResizePolicy::ReclaimLiveRows,
    );
    for (step, (cols, rows)) in SIZES.into_iter().enumerate() {
        actual.resize(cols, rows);
        actual.draw(step, rows - 1, None);
        let mut fresh = Term::new(
            cols,
            rows,
            0,
            RenderMode::Fullscreen,
            InlineResizePolicy::ReclaimLiveRows,
        );
        fresh.draw(step, rows - 1, None);
        assert_eq!(cells(&actual.vt, cols, rows), cells(&fresh.vt, cols, rows));
    }
}

#[test]
fn committed_marker_survives_visible_short_regions_and_full_height_scrollback() {
    let mut t = Term::new(
        40,
        15,
        4096,
        RenderMode::Inline,
        InlineResizePolicy::ReclaimLiveRows,
    );
    t.ctx.commit_text(KEEP).unwrap();
    t.pump();
    t.draw(0, 3, None);
    assert!(t.vt.screen().contents().contains(KEEP));
    t.assert_marker(KEEP);
    for (step, (cols, rows)) in SIZES.into_iter().enumerate() {
        t.resize(cols, rows);
        t.draw(step, rows - 1, None);
        t.assert_marker(KEEP);
    }
    assert!(
        t.history().iter().any(|line| line.trim_end() == KEEP),
        "the committed marker actually reached scrollback"
    );
}

#[test]
fn shorter_live_regions_keep_committed_history_and_only_current_live_rows_visible() {
    let mut t = Term::new(
        40,
        15,
        4096,
        RenderMode::Inline,
        InlineResizePolicy::ReclaimLiveRows,
    );
    t.ctx.commit_text(KEEP).unwrap();
    t.pump();
    for (step, (cols, rows)) in SIZES.into_iter().enumerate() {
        t.resize(cols, rows);
        t.draw(step, 3, None);
        t.assert_marker(KEEP);
        let live: Vec<_> =
            t.vt.screen()
                .rows(0, cols)
                .filter(|line| line.starts_with("LIVE-"))
                .collect();
        assert_eq!(live.len(), 3);
        assert!(live
            .iter()
            .all(|line| line.starts_with(&format!("LIVE-{step:02}-"))));
    }
}

#[test]
fn top_middle_and_bottom_widget_cursors_keep_full_height_regrow_clean() {
    for position in 0..3 {
        let mut t = Term::new(
            40,
            15,
            4096,
            RenderMode::Inline,
            InlineResizePolicy::ReclaimLiveRows,
        );
        t.ctx.commit_text(KEEP).unwrap();
        t.pump();
        for (step, (cols, rows)) in SIZES.into_iter().enumerate() {
            t.resize(cols, rows);
            let cursor = match position {
                0 => 0,
                1 => (rows - 2) / 2,
                _ => rows - 2,
            };
            t.draw(step, rows - 1, Some(cursor));
            t.assert_marker(KEEP);
            if step > 0 {
                let mut fresh = Term::new(
                    cols,
                    rows,
                    4096,
                    RenderMode::Inline,
                    InlineResizePolicy::ReclaimLiveRows,
                );
                fresh.draw(step, rows - 1, Some(cursor));
                assert_eq!(
                    cells(&t.vt, cols, rows),
                    cells(&fresh.vt, cols, rows),
                    "cursor case {position}, step{step}"
                );
            }
        }
    }
}

#[test]
fn pending_resize_clear_commit_and_insertion_keep_committed_markers() {
    for operation in 0..3 {
        let mut t = Term::new(
            80,
            24,
            4096,
            RenderMode::Inline,
            InlineResizePolicy::ReclaimLiveRows,
        );
        t.ctx.commit_text(KEEP).unwrap();
        t.pump();
        t.draw(0, 23, None);
        for (step, (cols, rows)) in [(40, 15), (80, 24), (79, 23), (81, 25)]
            .into_iter()
            .enumerate()
        {
            t.resize(cols, rows);
            let marker = format!("OP-{operation}-{step}");
            match operation {
                0 => t.ctx.clear_live_region().unwrap(),
                1 => t.ctx.commit_text(&marker).unwrap(),
                _ => t.ctx.insert_text_before_live(&marker).unwrap(),
            };
            t.pump();
            t.draw(step + 1, rows - 1, None);
            t.assert_marker(KEEP);
            if operation != 0 {
                t.assert_marker(&marker);
            }
            assert!(t
                .vt
                .screen()
                .contents()
                .contains(&format!("LIVE-{:02}-00", step + 1)));
        }
    }
}

#[test]
fn two_row_terminals_never_emit_zero_distance_cursor_moves() {
    let mut t = Term::new(
        40,
        2,
        0,
        RenderMode::Inline,
        InlineResizePolicy::ReclaimLiveRows,
    );
    for (step, (cols, rows)) in [(40, 2), (41, 2), (40, 3), (40, 2), (80, 24), (40, 2)]
        .into_iter()
        .enumerate()
    {
        t.resize(cols, rows);
        let bytes = t.draw(step, rows - 1, None);
        assert!(!bytes.windows(4).any(|w| w == b"\x1b[0A" || w == b"\x1b[0B"));
        let mut fresh = Term::new(
            cols,
            rows,
            0,
            RenderMode::Inline,
            InlineResizePolicy::ReclaimLiveRows,
        );
        fresh.draw(step, rows - 1, None);
        assert_eq!(
            cells(&t.vt, cols, rows),
            cells(&fresh.vt, cols, rows),
            "step{step}"
        );
    }
}

struct FailAfter {
    remaining: usize,
    bytes: Vec<u8>,
}
impl Write for FailAfter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "synthetic torn frame",
            ));
        }
        let n = self.remaining.min(bytes.len());
        self.bytes.extend_from_slice(&bytes[..n]);
        self.remaining -= n;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn reclaim_reanchor_recovers_from_zero_and_partial_writes_without_history_loss() {
    for cut in [0, 1, 16, 60] {
        let mut r = Renderer::new(RenderMode::Inline);
        r.set_inline_resize_policy(InlineResizePolicy::ReclaimLiveRows);
        let mut s = TerminalSession::headless(40, 15);
        let mut vt = vt100::Parser::new(nz(15), nz(40), 4096);
        let mut bytes = Vec::new();
        r.commit_text(KEEP, &mut s, &mut bytes).unwrap();
        r.render(&mut tree(0, 40, 14, None), &mut s, &mut bytes)
            .unwrap();
        vt.process(&bytes);
        vt.set_size(nz(24), nz(80));
        s.set_terminal_size(80, 24);
        let mut fail = FailAfter {
            remaining: cut,
            bytes: Vec::new(),
        };
        assert!(r
            .render(&mut tree(1, 80, 23, None), &mut s, &mut fail)
            .is_err());
        assert_eq!(fail.bytes.len(), cut);
        vt.process(&fail.bytes);
        assert_eq!(r.physical_state_uncertain(), cut > 0);
        let mut retry = Vec::new();
        r.render(&mut tree(2, 80, 23, None), &mut s, &mut retry)
            .unwrap();
        vt.process(&retry);
        assert!(!r.physical_state_uncertain());
        let bottom = vt.screen().cursor_position().0;
        let top = bottom - 22;
        for row in 0..23 {
            assert!(
                vt.screen()
                    .rows(0, 80)
                    .nth((top + row) as usize)
                    .unwrap()
                    .starts_with(&format!("LIVE-02-{row:02}")),
                "cut{cut}, row{row}"
            );
        }
        let mut term = Term {
            ctx: Context::headless(RenderMode::Inline, 80, 24),
            vt,
            cols: 80,
            rows: 24,
        };
        term.assert_marker(KEEP);
    }
}
