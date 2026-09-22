import re

with open("src/renderer.rs", "r") as f:
    text = f.read()

# Replace the body of insert_before_live
old_body_regex = r"pub fn insert_before_live\([\s\S]*?\{([\s\S]*?Ok\(\(\)\)\n    \})"

def replacement(m):
    return """pub fn insert_before_live(
        &mut self,
        lines: &[&str],
        session: &mut TerminalSession,
        writer: &mut dyn Write,
    ) -> io::Result<()> {
        if !session.is_tty {
            for line in lines {
                writeln!(writer, "{}", strip_ansi_escapes(line))?;
            }
            writer.flush()?;
            return Ok(());
        }

        if lines.is_empty() {
            return Ok(());
        }

        let mut tx = TerminalTransaction::new(writer, session.sync_updates());
        tx.begin();

        let m = lines.len() as u16;

        if self.live_region_height > 0 {
            // 1. Move to bottom of live region
            if self.last_cursor_y < self.live_region_height - 1 {
                write!(tx.buffer, "\\x1b[{}B", self.live_region_height - 1 - self.last_cursor_y).ok();
            }

            // 2. Emit M newlines to create space at bottom / scroll up
            for _ in 0..m {
                tx.push(b"\\r\\n");
            }

            // 3. Move UP by M + H - 1
            write!(tx.buffer, "\\x1b[{}A\\r", m + self.live_region_height - 1).ok();

            // 4. Insert M lines, pushing the live region down
            write!(tx.buffer, "\\x1b[{}L", m).ok();
        }

        // 5. Print the committed lines into the newly created gap
        for line in lines {
            tx.push(line.as_bytes());
            tx.push(b"\\r\\n");
        }

        // 6. If there was a live region, we are now exactly at its top.
        // We must restore the cursor to its relative position.
        if self.live_region_height > 0 {
            if self.last_cursor_y > 0 {
                write!(tx.buffer, "\\x1b[{}B", self.last_cursor_y).ok();
            }
            if self.last_cursor_x > 0 {
                write!(tx.buffer, "\\x1b[{}C", self.last_cursor_x).ok();
            }
        }

        tx.commit()?;
        Ok(())
    }"""

text = re.sub(old_body_regex, replacement, text, count=1)

with open("src/renderer.rs", "w") as f:
    f.write(text)
