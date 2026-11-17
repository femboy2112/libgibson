//! Synthetic owned-terminal resize probe. No application data, art or model.
//! Arguments: receipt-file, reclaim|preserve, inline|fullscreen, retained|fresh-step.
use gibson::{Context, InlineResizePolicy, Node, RenderMode, Style};
use std::fs::OpenOptions;
use std::io::{self, Read, Write};

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("expected receipt-file policy mode retained|fresh-step".into());
    }
    let policy = match args[1].as_str() {
        "reclaim" => InlineResizePolicy::ReclaimLiveRows,
        "preserve" => InlineResizePolicy::PreserveAbove,
        _ => return Err("unknown policy".into()),
    };
    let mode = match args[2].as_str() {
        "inline" => RenderMode::Inline,
        "fullscreen" => RenderMode::Fullscreen,
        _ => return Err("unknown mode".into()),
    };
    let start = if args[3] == "retained" {
        0
    } else {
        args[3].parse::<usize>()?
    };
    if start >= SIZES.len() {
        return Err("invalid fresh step".into());
    }
    let mut receipt = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[0])?;
    let mut ctx = Context::new(mode)?;
    ctx.enter_interactive()?;
    ctx.set_inline_resize_policy(policy);
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        for step in start..SIZES.len() {
            let (cols, rows) = ctx.session.terminal_size();
            if (cols, rows) != SIZES[step] {
                return Err(format!("geometry mismatch at {step}: {cols}x{rows}").into());
            }
            let height = rows.saturating_sub(1).max(1);
            let text = (0..height)
                .map(|row| format!("LIVE-{step:02}-{row:02}"))
                .collect::<Vec<_>>()
                .join("\n");
            ctx.set_root(
                Node::text(text, Style::default())
                    .width(cols as f32)
                    .height(height as f32),
            );
            ctx.render()?;
            let report = ctx.last_frame_report();
            let rewound = report
                .reanchor
                .map_or_else(|| "null".to_owned(), |r| r.rewound_rows.to_string());
            writeln!(
                receipt,
                "{{\"step\":{step},\"cols\":{cols},\"rows\":{rows},\"height\":{height},\"full_repaint\":{},\"bytes\":{},\"rewound_rows\":{rewound}}}",
                report.full_repaint, report.bytes_emitted
            )?;
            receipt.flush()?;
            let mut command = [0u8; 1];
            io::stdin().read_exact(&mut command)?;
            if command[0] == b'q' {
                break;
            }
            if command[0] != b'n' || args[3] != "retained" {
                return Err("invalid probe step command".into());
            }
        }
        Ok(())
    })();
    let restored = ctx.restore();
    result?;
    restored?;
    Ok(())
}
