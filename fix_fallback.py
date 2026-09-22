with open("src/renderer.rs", "r") as f:
    text = f.read()

import re
old_body_regex = r"pub fn insert_before_live\([\s\S]*?Ok\(bytes\)\n    \}"

replacement = """pub fn insert_before_live(
        &mut self,
        lines: &[&str],
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<usize> {
        if !session.is_tty {
            for line in lines {
                writeln!(writer, "{}", strip_ansi_escapes(line))?;
            }
            writer.flush()?;
            return Ok(0);
        }

        if lines.is_empty() {
            return Ok(0);
        }

        let m = lines.len() as u16;
        let (_, term_rows) = session.terminal_size();
        let combined_height = m.saturating_add(self.live_region_height);

        let mut tx = TerminalTransaction::new(writer, session.sync_updates());
        tx.begin();

        if self.live_region_height == 0 || combined_height <= term_rows {
            // == ScrollingRegionInsertion Strategy ==
            if self.live_region_height > 0 {
                if self.last_cursor_y < self.live_region_height - 1 {
                    write!(tx.buffer, "\\x1b[{}B", self.live_region_height - 1 - self.last_cursor_y).ok();
                }

                for _ in 0..m {
                    tx.push(b"\\r\\n");
                }

                write!(tx.buffer, "\\x1b[{}A\\r", m + self.live_region_height - 1).ok();
                write!(tx.buffer, "\\x1b[{}L", m).ok();
            }

            for line in lines {
                tx.push(line.as_bytes());
                tx.push(b"\\r\\n");
            }

            if self.live_region_height > 0 {
                if self.last_cursor_y > 0 {
                    write!(tx.buffer, "\\x1b[{}B", self.last_cursor_y).ok();
                }
                if self.last_cursor_x > 0 {
                    write!(tx.buffer, "\\x1b[{}C", self.last_cursor_x).ok();
                }
            }
        } else {
            // == RepaintFallback Strategy ==
            if self.live_region_height > 1 {
                if self.last_cursor_y > 0 {
                    write!(tx.buffer, "\\x1b[{}A", self.last_cursor_y).ok();
                }
                tx.push(b"\\r\\x1b[J");
            } else if self.live_region_height == 1 {
                tx.push(b"\\r\\x1b[K");
            }

            for line in lines {
                tx.push(line.as_bytes());
                tx.push(b"\\r\\n");
            }

            if self.live_region_height > 1 {
                for _ in 0..(self.live_region_height - 1) {
                    tx.push(b"\\r\\n");
                }
                write!(tx.buffer, "\\x1b[{}A\\r", self.live_region_height - 1).ok();
            } else if self.live_region_height == 1 {
                tx.push(b"\\r");
            }

            if let Some(ref prev) = self.previous_surface {
                self.compiler.reset_cursor(0, 0);
                let bytes = self.compiler.compile_full(prev);
                tx.push(&bytes);
                self.last_cursor_y = prev.height.saturating_sub(1);
                // cursor_x is handled by paint next frame anyway, but we should roughly place it
            }
        }

        let bytes = tx.buffer.len();
        tx.commit()?;
        Ok(bytes)
    }"""

text = re.sub(old_body_regex, lambda _: replacement, text, count=1)

with open("src/renderer.rs", "w") as f:
    f.write(text)
