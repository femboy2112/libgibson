use std::io::{self, Write};

/// Represents a single atomic write to the terminal.
///
/// The byte count returned by [`TerminalTransaction::commit`] is the **exact**
/// number of bytes written to the writer, including the synchronized-update
/// terminator that `commit` itself appends. Callers must use that return value
/// (not `buffer.len()` sampled before commit) for wire-byte accounting.
pub struct TerminalTransaction<'a> {
    pub buffer: Vec<u8>,
    writer: &'a mut dyn Write,
    sync_updates: bool,
    committed_bytes: Option<usize>,
}

impl<'a> TerminalTransaction<'a> {
    pub fn new(writer: &'a mut dyn Write, sync_updates: bool) -> Self {
        Self {
            buffer: Vec::with_capacity(8192),
            writer,
            sync_updates,
            committed_bytes: None,
        }
    }

    /// Appends the synchronized-update begin marker, if enabled.
    pub fn begin(&mut self) {
        if self.sync_updates {
            self.buffer.extend_from_slice(b"\x1b[?2026h");
        }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// Number of bytes currently buffered (does **not** include the terminator
    /// that [`TerminalTransaction::commit`] appends).
    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }

    /// Flushes the transaction and returns the exact number of wire bytes
    /// written, including the synchronized-update terminator.
    pub fn commit(mut self) -> io::Result<usize> {
        if self.sync_updates {
            self.buffer.extend_from_slice(b"\x1b[?2026l");
        }
        let n = self.buffer.len();
        self.writer.write_all(&self.buffer)?;
        self.writer.flush()?;
        self.committed_bytes = Some(n);
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_reports_exact_wire_bytes_including_terminator() {
        let mut sink = Vec::new();
        let mut tx = TerminalTransaction::new(&mut sink, true);
        tx.begin();
        tx.push(b"abc");
        let n = tx.commit().unwrap();
        // begin (8) + payload (3) + terminator (8)
        assert_eq!(n, 19);
        assert_eq!(sink.len(), 19);
    }

    #[test]
    fn commit_without_sync_updates_counts_payload_only() {
        let mut sink = Vec::new();
        let mut tx = TerminalTransaction::new(&mut sink, false);
        tx.begin();
        tx.push(b"abc");
        let n = tx.commit().unwrap();
        assert_eq!(n, 3);
        assert_eq!(sink.len(), 3);
    }
}
