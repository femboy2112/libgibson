use std::io::{self, Write};

/// Buffers one terminal update and commits it with one counted write loop + `flush`.
///
/// This is **not atomic** in the all-or-nothing sense. [`TerminalTransaction::commit_counted`]
/// writes until every byte is accepted, and a write can fail AFTER a partial write: the OS — and the physical
/// terminal — may already have received a prefix of the byte stream, and nothing here
/// can roll that back. When synchronized-update framing is enabled the buffer is wrapped
/// in `?2026h`/`?2026l`, which asks the terminal to present the update atomically on the
/// **display** — but that is a rendering hint, not a transport guarantee: a write torn
/// mid-buffer can leave the terminal having received `?2026h` and a partial escape stream
/// with no closing `?2026l`. Reconciling a partial commit is the caller's concern: the renderer
/// voids its physical belief and re-establishes the screen on the next frame
/// (`tests/renderer_partial_write.rs`, RXIV Gate G3).
///
/// The byte count returned by `commit_counted` is, on a **successful** commit, the **exact**
/// number of bytes written to the writer, including the synchronized-update terminator
/// that `commit_counted` itself appends. Callers must use that return value (not `buffer.len()`
/// sampled before commit) for wire-byte accounting.
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
    /// that [`TerminalTransaction::commit_counted`] appends).
    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }

    /// Commits the buffered update (all bytes, then one `flush`); on success returns the exact
    /// number of wire bytes written, including the synchronized-update terminator. **Not atomic:**
    /// a write can fail after a partial write, leaving the terminal holding a prefix of the stream
    /// (possibly a `?2026h` with no closing `?2026l`) that cannot be rolled back. A failure
    /// therefore reports how many bytes were handed to the writer before it (RXIV Gate G3). That count is what separates a CERTAIN failure — zero bytes of this
    /// update reached the writer, so the terminal is exactly as it was — from an UNCERTAIN one: a
    /// prefix was handed off (the terminal may hold a torn update), or every byte was written but
    /// the flush failed (a buffered writer may or may not have delivered them).
    pub fn commit_counted(mut self) -> Result<usize, CommitFailure> {
        if self.sync_updates {
            self.buffer.extend_from_slice(b"\x1b[?2026l");
        }
        let n = self.buffer.len();
        let mut written = 0usize;
        while written < n {
            match self.writer.write(&self.buffer[written..]) {
                Ok(0) => {
                    return Err(CommitFailure {
                        error: io::Error::new(
                            io::ErrorKind::WriteZero,
                            "terminal accepted 0 bytes",
                        ),
                        written,
                        flush_failed: false,
                    })
                }
                Ok(k) => written += k,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    return Err(CommitFailure {
                        error,
                        written,
                        flush_failed: false,
                    })
                }
            }
        }
        if let Err(error) = self.writer.flush() {
            return Err(CommitFailure {
                error,
                written,
                flush_failed: true,
            });
        }
        self.committed_bytes = Some(n);
        Ok(n)
    }
}

/// A failed [`TerminalTransaction::commit_counted`]: the error, how many bytes of the update were
/// handed to the writer before it, and whether the failure was the final flush.
#[derive(Debug)]
pub struct CommitFailure {
    pub error: io::Error,
    pub written: usize,
    pub flush_failed: bool,
}

impl CommitFailure {
    /// No byte of this update reached the writer: the terminal is exactly as it was.
    pub fn is_certain_noop(&self) -> bool {
        self.written == 0 && !self.flush_failed
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
        let n = tx.commit_counted().unwrap();
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
        let n = tx.commit_counted().unwrap();
        assert_eq!(n, 3);
        assert_eq!(sink.len(), 3);
    }
}
