//! Wave 5 canary — the presenter half of the SmartChem-facing transport.
//!
//! Reads a `gibson::wire` frame stream on stdin (pipe the reference producer into it),
//! decodes it through a bounded [`FrameDecoder`], stores the latest observation per source
//! in a bounded [`FieldInbox`], and renders the primary source's latest field through
//! [`FieldView`] — printing an ASCII heat dump plus the conservation receipts that prove
//! nothing was invented, dropped silently, or allowed to grow without bound.
//!
//! ```text
//! python3 bindings/python/producer.py --frames 60 --cols 60 --rows 24 \
//!     | cargo run --example smartchem_canary
//! ```
//!
//! The ASCII dump samples the decoded [`FieldView`] directly, so it is correct for any
//! producer geometry. The *render receipt* builds a 1:1 [`WorldView`] and therefore assumes
//! the reference producer's default unit-cell, origin-(0,0) grid; a different geometry would
//! be disclosed honestly as off-field cells rather than silently mis-rendered.

use gibson::field_view::{Colormap, FieldView, RangePolicy, Sample};
use gibson::snapshot::{Capacity, SnapshotBridge};
use gibson::wire::{FieldAdmission, FieldInbox, FrameDecoder, WireBody, WireCommand};
use gibson::world_view::WorldView;
use gibson::Rect;
use std::io::{self, Read, Write};

fn ascii_dump<W: Write>(out: &mut W, view: &FieldView, cols: usize, rows: usize) -> io::Result<()> {
    const RAMP: &[u8] = b" .:-=+*#%@";
    let (lo, hi) = view.finite_extremes().unwrap_or((0.0, 1.0));
    let span = if hi > lo { hi - lo } else { 1.0 };
    for iy in 0..rows {
        let mut line = String::with_capacity(cols);
        for ix in 0..cols {
            let ch = match view.sample(ix, iy) {
                Some(Sample::Value(v)) => {
                    let t = ((v - lo) / span).clamp(0.0, 1.0);
                    let idx = (t * (RAMP.len() - 1) as f64).round() as usize;
                    RAMP[idx] as char
                }
                Some(Sample::Masked) => ' ',
                Some(Sample::NonFinite) => '?',
                None => ' ',
            };
            line.push(ch);
        }
        writeln!(out, "{line}")?;
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let mut dec = FrameDecoder::new();
    let mut inbox = FieldInbox::new();
    // The command return channel is the Finding-A bounded SnapshotBridge, carrying our
    // WireCommand. A real presenter issues edits here as the user interacts.
    let mut outbound: SnapshotBridge<WireCommand> =
        SnapshotBridge::with_capacity(Capacity::new(16, 256));
    let mut primary: Option<u64> = None;
    let mut retired: u64 = 0;

    let mut stdin = io::stdin().lock();
    let mut chunk = [0u8; 65536];
    'outer: loop {
        let n = stdin.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        if let Err(e) = dec.push(&chunk[..n]) {
            eprintln!("transport fatal on push: {e}");
            break;
        }
        loop {
            match dec.next_message() {
                None => break,
                Some(Ok(msg)) => {
                    primary.get_or_insert(msg.source);
                    let (source, cursor) = (msg.source, msg.cursor);
                    match msg.body {
                        WireBody::Field(field) => {
                            if let FieldAdmission::SourceCapReached { capacity } =
                                inbox.admit(source, cursor, field)
                            {
                                eprintln!("source cap {capacity} reached; refused source {source}");
                            }
                        }
                        WireBody::Retire => {
                            if inbox.retire(source) {
                                retired += 1;
                            }
                        }
                        WireBody::Command(_) | WireBody::Ack(_) => {}
                    }
                }
                Some(Err(e)) => {
                    eprintln!("skipped malformed frame: {e}");
                    if e.is_fatal() {
                        break 'outer;
                    }
                }
            }
        }
    }
    if let Err(e) = dec.finish() {
        eprintln!("stream ended uncleanly: {e}");
    }

    // Demonstrate the bounded, ordered return channel (a real UI would issue real edits).
    let _ = outbound.issue(WireCommand {
        command_id: 0,
        kind: 1,
        arg: b"noop".to_vec(),
    });

    let stdout = io::stdout();
    let mut out = stdout.lock();
    if let Some(src) = primary {
        if let Some(field) = inbox.latest(src) {
            let ext = field.extent;
            let view = FieldView::new(ext, &field.data, field.mask.as_deref())
                .expect("a decoded field always has matching data/mask lengths");
            writeln!(
                out,
                "SmartChem canary — source {src}, model_tick {}, {}x{} cells",
                field.model_tick,
                ext.cols(),
                ext.rows()
            )?;
            ascii_dump(&mut out, &view, ext.cols(), ext.rows())?;

            let w = ext.cols().min(200) as u16;
            let h = ext.rows().min(80) as u16;
            let wv = WorldView::new(Rect::new(0, 0, w, h))
                .with_zoom(1.0, 1.0)
                .with_pixels_per_cell(1, 1);
            let frame = view.render(
                &wv,
                Rect::new(0, 0, w, h),
                Colormap::Heat,
                RangePolicy::Auto,
            );
            let r = &frame.report;
            writeln!(
                out,
                "render receipt: cells_in_area={} value={} masked={} nonfinite={} off_field={} clipped={} conserves={}",
                r.cells_in_area, r.value_cells, r.masked_cells, r.nonfinite_cells, r.off_field_cells, r.clipped_cells, r.conserves()
            )?;
        }
    } else {
        writeln!(out, "SmartChem canary — no frames received")?;
    }

    writeln!(
        out,
        "inbox receipt: live_sources={} stored={} superseded={} stale={} refused={} retired={} retained_cells={} conserves={}",
        inbox.live_sources(), inbox.stored(), inbox.superseded(), inbox.stale(), inbox.refused(), retired, inbox.retained_cells(), inbox.conserves()
    )?;
    writeln!(
        out,
        "command channel: offered={} accepted={} refused={} conserved={}",
        outbound.commands_offered(),
        outbound.commands_accepted(),
        outbound.commands_refused(),
        outbound.commands_conserved()
    )?;
    Ok(())
}
