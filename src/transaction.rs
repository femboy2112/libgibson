use std::io::{self, Write};

/// Represents a single atomic write to the terminal.
pub struct TerminalTransaction<'a> {
    pub buffer: Vec<u8>,
    writer: &'a mut dyn Write,
    sync_updates: bool,
}

impl<'a> TerminalTransaction<'a> {
    pub fn new(writer: &'a mut dyn Write, sync_updates: bool) -> Self {
        Self {
            buffer: Vec::with_capacity(8192),
            writer,
            sync_updates,
        }
    }

    pub fn begin(&mut self) {
        if self.sync_updates {
            self.buffer.extend_from_slice(b"\x1b[?2026h");
        }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    pub fn commit(mut self) -> io::Result<()> {
        if self.sync_updates {
            self.buffer.extend_from_slice(b"\x1b[?2026l");
        }
        self.writer.write_all(&self.buffer)?;
        self.writer.flush()?;
        Ok(())
    }
}
