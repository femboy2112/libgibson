//! Supervised-child live modes (workstream D4/D5): Ownership-Duel and
//! Restore-Failure.
//!
//! Both run a REAL `terminal_ownership_probe` scenario as a child under its own
//! fresh PTY — never the Observatory's terminal — capture exactly what the child
//! emitted, and reveal those genuine receipts paced for the human eye. The
//! pacing is a presentation choice; every line shown is a real receipt from a
//! real child run this session. If the sibling probe binary isn't built, the
//! mode says so with a fault row rather than inventing a trace (the epistemic
//! rule: unobserved is `?`, never fabricated).

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::Read;
use std::time::{Duration, Instant};

use crate::diag::{Category, DiagLog};
use crate::live::LiveMode;
use crate::visual::{PanelRow, Spark, Tone, View};

/// One revealed receipt from the child.
struct Receipt {
    us: Option<u64>,
    category: Category,
    kind: String,
    source: String,
    value: String,
    tone: Tone,
}

/// Run a sibling example probe under a fresh PTY, to completion (bounded), and
/// return everything it wrote. The child owns its OWN terminal, so it can enter
/// raw/alt mode and induce failures without touching the live instrument's
/// screen. A reader thread drains the master so a slow child can't wedge us; the
/// deadline kills a child that never exits.
fn run_probe(scenario: &str, deadline: Duration) -> std::io::Result<String> {
    let exe = std::env::current_exe()?;
    let probe = exe.with_file_name("terminal_ownership_probe");
    if !probe.exists() {
        return Err(std::io::Error::other(format!(
            "sibling probe not built at {} (run `cargo build --examples`)",
            probe.display()
        )));
    }
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(std::io::Error::other)?;
    let mut cmd = CommandBuilder::new(&probe);
    cmd.arg(scenario);
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(std::io::Error::other)?;
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(std::io::Error::other)?;
    // The reader blocks until EOF (all slave fds closed = child exited). Run it
    // on a thread so we can bound the wait and kill a child that hangs.
    let handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf);
        buf
    });

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(e) => return Err(std::io::Error::other(e)),
        }
        if start.elapsed() > deadline {
            let _ = child.kill();
            break;
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    let _ = child.wait();
    drop(pair.master); // close our master fd so read_to_end sees EOF
    let buf = handle.join().unwrap_or_default();
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Parse the duel-trace TSV (`seq\tus\tsource\tkind\tvalue`) into receipts.
fn parse_duel(out: &str) -> Vec<Receipt> {
    let mut receipts = Vec::new();
    for line in out.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 5 {
            continue; // DUEL_DONE and blanks
        }
        let us = f[1].parse::<u64>().ok();
        let kind = f[3].to_string();
        let value = f[4].to_string();
        let tone = if value.contains("rejected") {
            Tone::Warn
        } else {
            Tone::Nominal
        };
        receipts.push(Receipt {
            us,
            category: Category::Session,
            kind,
            source: f[2].to_string(),
            value,
            tone,
        });
    }
    receipts
}

/// Parse restore-output-failure's `KEY=value` markers into ordered receipts.
fn parse_restore_failure(out: &str) -> Vec<Receipt> {
    let mut receipts = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if !matches!(
            key,
            "RAW_BEFORE" | "RESTORE_RESULT" | "RAW_AFTER" | "LEASE_AFTER"
        ) {
            continue;
        }
        // Tone encodes the contract, not just success/failure: reporting the
        // write error is the CORRECT behaviour here, so it is Warn (expected),
        // and the teardown completing anyway is Nominal (the whole point of B1).
        let tone = match (key, value) {
            ("RESTORE_RESULT", v) if v.starts_with("ERR") => Tone::Warn,
            ("RAW_AFTER", "false") => Tone::Nominal,
            ("LEASE_AFTER", "Available") => Tone::Nominal,
            ("RAW_BEFORE", "true") => Tone::Nominal,
            _ => Tone::Fault, // any other value would be a contract violation
        };
        receipts.push(Receipt {
            us: None,
            category: Category::Session,
            kind: key.to_string(),
            source: "terminal_ownership_probe restore-output-failure".to_string(),
            value: value.to_string(),
            tone,
        });
    }
    receipts
}

pub struct SupervisedReplay {
    key: char,
    mode_label: String,
    scenario: String,
    parser: fn(&str) -> Vec<Receipt>,
    subtitle: String,
    footer: String,
    ran: bool,
    error: Option<String>,
    receipts: Vec<Receipt>,
    revealed: usize,
    started_us: Option<u64>,
    step_us: u64,
    log: DiagLog,
}

impl SupervisedReplay {
    pub fn ownership_duel() -> Self {
        Self::new(
            '3',
            "OWNERSHIP-DUEL",
            "duel-trace",
            parse_duel,
            "two contexts fight over the process-global lease, for real · issue #11",
            "Real receipts from a supervised terminal_ownership_probe child, run this session under its own PTY.",
        )
    }

    pub fn restore_failure() -> Self {
        Self::new(
            '5',
            "RESTORE-FAILURE",
            "restore-output-failure",
            parse_restore_failure,
            "explicit restore under induced stdout failure · issue #11 B1",
            "Real child: fd 1 -> /dev/full (ENOSPC). restore() reports Err yet still tears down raw mode and releases the lease.",
        )
    }

    fn new(
        key: char,
        mode_label: &str,
        scenario: &str,
        parser: fn(&str) -> Vec<Receipt>,
        subtitle: &str,
        footer: &str,
    ) -> Self {
        Self {
            key,
            mode_label: mode_label.to_string(),
            scenario: scenario.to_string(),
            parser,
            subtitle: subtitle.to_string(),
            footer: footer.to_string(),
            ran: false,
            error: None,
            receipts: Vec::new(),
            revealed: 0,
            started_us: None,
            step_us: 380_000, // reveal one receipt every ~0.38s, human-paced
            log: DiagLog::new(64),
        }
    }

    fn ensure_run(&mut self) {
        if self.ran {
            return;
        }
        self.ran = true;
        match run_probe(&self.scenario, Duration::from_secs(6)) {
            Ok(out) => self.receipts = (self.parser)(&out),
            Err(e) => self.error = Some(e.to_string()),
        }
    }
}

impl LiveMode for SupervisedReplay {
    fn key(&self) -> char {
        self.key
    }

    fn tick(&mut self, now_us: u64, _intensity: i32) {
        self.ensure_run();
        let start = *self.started_us.get_or_insert(now_us);
        let elapsed = now_us.saturating_sub(start);
        let target = ((elapsed / self.step_us) as usize + 1).min(self.receipts.len());
        while self.revealed < target {
            let r = &self.receipts[self.revealed];
            self.log.push(
                r.us.unwrap_or(now_us),
                r.category,
                r.kind.clone(),
                r.source.clone(),
                r.value.clone(),
                None,
            );
            self.revealed += 1;
        }
    }

    fn build_view(&self, now_us: u64, paused: bool, _intensity: i32) -> View<'_> {
        // Hero state = the latest revealed receipt's meaning, or a fault if the
        // child could not be run at all.
        let (hero, hero_tone) = if let Some(err) = &self.error {
            (format!("PROBE UNAVAILABLE: {err}"), Tone::Fault)
        } else if self.receipts.is_empty() {
            ("NO RECEIPTS".to_string(), Tone::Warn)
        } else if self.revealed == 0 {
            ("SPAWNING…".to_string(), Tone::Warn)
        } else {
            let last = &self.receipts[self.revealed - 1];
            let tone = if paused { Tone::Warn } else { last.tone };
            (format!("{} = {}", last.kind, last.value), tone)
        };

        let stage_rows: Vec<PanelRow> = self
            .receipts
            .iter()
            .take(self.revealed)
            .map(|r| {
                let label = match r.us {
                    Some(us) => format!("{:>7.3}s {}", us as f64 / 1_000_000.0, r.kind),
                    None => r.kind.clone(),
                };
                PanelRow::new(label, r.value.clone(), r.tone)
            })
            .collect();

        let panel_rows = vec![
            PanelRow::new("scenario", self.scenario.clone(), Tone::Nominal),
            PanelRow::new(
                "receipts",
                format!("{}/{}", self.revealed, self.receipts.len()),
                Tone::Nominal,
            ),
            PanelRow::new(
                "source",
                "supervised child (PTY)".to_string(),
                Tone::Nominal,
            ),
        ];

        View {
            monotonic_us: now_us,
            mode_label: self.mode_label.clone(),
            hero_state: hero,
            hero_tone,
            subtitle: self.subtitle.clone(),
            panel_title: "SUPERVISED RECEIPTS (real child, revealed in order)".into(),
            panel_rows,
            stage_rows,
            sparks: Vec::<Spark>::new(),
            log: &self.log,
            footer: self.footer.clone(),
            controls: "1-6 mode · Space pause · R rerun child · Esc exit".into(),
        }
    }

    fn restart(&mut self) {
        self.ran = false;
        self.error = None;
        self.receipts.clear();
        self.revealed = 0;
        self.started_us = None;
        self.log = DiagLog::new(64);
    }
}
