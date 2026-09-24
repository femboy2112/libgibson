//! Independent nonblocking PTY supervisor. No detached reader, no screen-based
//! acceptance, no later diagnostic key. Every trial is bounded and reaped.
use super::trace::{clock_us, Record, MAX_RECORDS, MAX_TRACE_BYTES};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::Duration,
};

pub const MODES: [&str; 3] = ["raw", "crossterm", "context"];
pub const SCENARIOS: [&str; 7] = [
    "normal",
    "resize-key",
    "key-resize",
    "coincident",
    "slow-drain",
    "burst",
    "storm",
];
#[derive(Clone, Debug)]
pub struct Config {
    pub mode: String,
    pub scenario: String,
    pub graphics: bool,
    pub pause_ms: u64,
    pub deadline_ms: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            mode: "context".into(),
            scenario: "normal".into(),
            graphics: true,
            pause_ms: 60,
            deadline_ms: 700,
        }
    }
}
impl Config {
    pub fn validate(&self) -> io::Result<()> {
        if !MODES.contains(&self.mode.as_str())
            || !SCENARIOS.contains(&self.scenario.as_str())
            || self.pause_ms > 500
            || !(100..=3000).contains(&self.deadline_ms)
        {
            return Err(io::Error::other("invalid probe configuration"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
enum Action {
    Go,
    Resize(u16, u16),
    Key(u8),
    Pause,
    Resume,
}
#[derive(Debug)]
pub struct Report {
    pub sent: String,
    pub received: String,
    pub verdict: String,
    pub latencies_us: Vec<u64>,
    pub drained: u64,
    pub generated: u64,
    pub frames: u64,
    pub restored: bool,
    pub records: Vec<Record>,
}
impl Report {
    pub fn passed(&self) -> bool {
        self.verdict == "DELIVERED" && self.restored
    }
}

struct TempDir(PathBuf);
impl std::ops::Deref for TempDir {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub struct Trial {
    pub config: Config,
    pub epoch: u64,
    pub records: Vec<Record>,
    pub drained: u64,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    dir: TempDir,
    initial_termios: Option<String>,
    read_offset: u64,
    partial: String,
    gate: Option<u64>,
    actions: Vec<(u64, Action)>,
    next_action: usize,
    pause: bool,
    stop_at: Option<u64>,
    reaped: bool,
    finished: bool,
    pub report: Option<Report>,
    last_drain: u64,
}
impl Trial {
    pub fn start(exe: &Path, config: Config) -> io::Result<Self> {
        config.validate()?;
        let epoch = clock_us();
        let dir =
            std::env::temp_dir().join(format!("gibson-pressure-{}-{epoch}", std::process::id()));
        fs::create_dir(&dir)?;
        let dir = TempDir(dir);
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 32,
                cols: 120,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(io::Error::other)?;
        let fd = pair
            .master
            .as_raw_fd()
            .ok_or_else(|| io::Error::other("Unix master fd required"))?;
        // SAFETY: master owns fd; supervisor is its only reader/writer.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
        let initial_termios = pair.master.get_termios().map(|t| format!("{t:?}"));
        let mut cmd = CommandBuilder::new(exe);
        cmd.args([
            "--child",
            &config.mode,
            "--trace-dir",
            dir.to_str()
                .ok_or_else(|| io::Error::other("nonutf8 temp path"))?,
            "--epoch",
            &epoch.to_string(),
        ]);
        if config.graphics {
            cmd.arg("--graphics");
        }
        if config.scenario == "burst" {
            cmd.arg("--burst");
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        let child = pair.slave.spawn_command(cmd).map_err(io::Error::other)?;
        drop(pair.slave);
        let metadata = format!(
            "{},{},{},{},{}",
            config.mode, config.scenario, config.graphics, config.pause_ms, config.deadline_ms
        );
        let mut trial = Self {
            actions: script(&config),
            config,
            epoch,
            records: vec![],
            drained: 0,
            master: pair.master,
            child,
            dir,
            initial_termios,
            read_offset: 0,
            partial: String::new(),
            gate: None,
            next_action: 0,
            pause: false,
            stop_at: None,
            reaped: false,
            finished: false,
            report: None,
            last_drain: 0,
        };
        trial.emit("Config", metadata);
        Ok(trial)
    }
    pub fn elapsed_us(&self) -> u64 {
        clock_us().saturating_sub(self.epoch)
    }
    fn emit(&mut self, kind: &str, value: impl ToString) {
        self.records.push(Record {
            seq: self.records.len() as u64,
            us: self.elapsed_us(),
            source: "PTY".into(),
            kind: kind.into(),
            value: value.to_string(),
        });
    }
    pub fn send_key(&mut self, b: u8) -> io::Result<()> {
        let begin = self.elapsed_us();
        let fd = self.master.as_raw_fd().unwrap();
        let n = unsafe { libc::write(fd, (&b as *const u8).cast(), 1) };
        if n != 1 {
            return Err(io::Error::last_os_error());
        }
        self.emit("Write", u32::from(b));
        // Timestamp the beginning of the successful write, not its return:
        // the child can run before the parent returns from write().
        self.records.last_mut().unwrap().us = begin;
        Ok(())
    }
    fn read_receipts(&mut self) -> io::Result<()> {
        let path = self.dir.join("child.tsv");
        if !path.exists() {
            return Ok(());
        }
        let mut f = File::open(path)?;
        if f.metadata()?.len() > MAX_TRACE_BYTES {
            return Err(io::Error::other("child trace byte budget exhausted"));
        }
        f.seek(SeekFrom::Start(self.read_offset))?;
        let mut chunk = String::new();
        f.read_to_string(&mut chunk)?;
        self.read_offset += chunk.len() as u64;
        self.partial.push_str(&chunk);
        while let Some(end) = self.partial.find('\n') {
            let line: String = self.partial.drain(..=end).collect();
            let r = Record::parse(line.trim_end_matches('\n'))
                .ok_or_else(|| io::Error::other("invalid child trace"))?;
            if r.kind == "Gate" && self.gate.is_none() {
                self.gate = Some(self.elapsed_us());
            }
            self.records.push(r);
        }
        Ok(())
    }
    fn drain(&mut self) -> io::Result<()> {
        if self.pause {
            return Ok(());
        }
        let mut b = [0u8; 8192];
        let fd = self.master.as_raw_fd().unwrap();
        // Bound one supervisor turn so child output cannot starve actions/input.
        for _ in 0..8 {
            let n = unsafe { libc::read(fd, b.as_mut_ptr().cast(), b.len()) };
            if n <= 0 {
                let e = io::Error::last_os_error();
                if n == 0
                    || matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    )
                    || e.raw_os_error() == Some(libc::EIO)
                {
                    break;
                }
                return Err(e);
            }
            self.drained += n as u64;
        }
        if self.drained != self.last_drain {
            self.emit("Drained", self.drained);
            self.last_drain = self.drained;
        }
        Ok(())
    }
    pub fn tick(&mut self) -> io::Result<bool> {
        if self.finished {
            return Ok(true);
        }
        self.read_receipts()?;
        if self.records.len() >= MAX_RECORDS {
            return Err(io::Error::other("supervisor trace budget exhausted"));
        }
        if self.elapsed_us() > 7_000_000 {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "supervisor lifetime exceeded",
            ));
        }
        if let Some(gate) = self.gate {
            while self.next_action < self.actions.len()
                && self.elapsed_us() - gate >= self.actions[self.next_action].0 * 1000
            {
                let action = self.actions[self.next_action].1.clone();
                self.next_action += 1;
                match action {
                    Action::Go => {
                        fs::write(self.dir.join("go"), b"go")?;
                        self.emit("Release", "");
                    }
                    Action::Key(b) => self.send_key(b)?,
                    Action::Resize(w, h) => {
                        self.master
                            .resize(PtySize {
                                cols: w,
                                rows: h,
                                pixel_width: 0,
                                pixel_height: 0,
                            })
                            .map_err(io::Error::other)?;
                        self.emit("Resize", format!("{w}x{h}"));
                    }
                    Action::Pause => {
                        self.pause = true;
                        self.emit("DrainPaused", self.config.pause_ms);
                    }
                    Action::Resume => {
                        self.pause = false;
                        self.emit("DrainResumed", "");
                    }
                }
            }
            let all_sent = self.next_action == self.actions.len();
            let sent = self
                .records
                .iter()
                .filter(|r| r.source == "PTY" && r.kind == "Write")
                .count();
            let got = self
                .records
                .iter()
                .filter(|r| r.source == "APP" && r.kind == "Key")
                .count();
            let deadline = gate + (self.actions.last().unwrap().0 + self.config.deadline_ms) * 1000;
            if self.stop_at.is_none() && all_sent && (self.elapsed_us() >= deadline || got >= sent)
            {
                // No key is injected here, even on failure. Observe duplicates
                // for 60 ms, then stop over a separate filesystem control channel.
                self.stop_at = Some(self.elapsed_us() + 60_000);
                let late = self
                    .records
                    .iter()
                    .any(|r| r.source == "APP" && r.kind == "Key" && r.us > deadline);
                self.emit(
                    "VerdictWindow",
                    if got >= sent && !late {
                        "received"
                    } else {
                        "deadline"
                    },
                );
            }
        } else if self.elapsed_us() > 2_000_000 {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "child startup exceeded",
            ));
        }
        if self.stop_at.is_some_and(|t| self.elapsed_us() >= t) {
            self.pause = false;
            fs::write(self.dir.join("stop"), b"stop")?;
        }
        self.drain()?;
        if let Some(status) = self.child.try_wait()? {
            self.reaped = true;
            self.read_receipts()?;
            self.emit("Exit", status.exit_code());
            self.finished = true;
            self.records.sort_by_key(|r| r.us);
            for (seq, r) in self.records.iter_mut().enumerate() {
                r.seq = seq as u64;
            }
            let restored = self.initial_termios.is_some()
                && self.initial_termios == self.master.get_termios().map(|t| format!("{t:?}"))
                && self.records.iter().any(|r| r.kind == "Restored");
            self.emit("TerminalRestoration", restored);
            let mut report = analyze(&self.records, self.drained, restored);
            if !status.success() {
                report.verdict = "CHILD FAILED".into();
            }
            self.report = Some(report);
        }
        Ok(self.finished)
    }
}
impl Drop for Trial {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.child.kill();
            let deadline = clock_us() + 2_000_000;
            while clock_us() < deadline {
                if matches!(self.child.try_wait(), Ok(Some(_))) {
                    self.reaped = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            if !self.reaped {
                eprintln!("pressure lab: direct child could not be reaped within cleanup bound");
            }
        }
    }
}
fn script(c: &Config) -> Vec<(u64, Action)> {
    use Action::*;
    match c.scenario.as_str() {
        "resize-key" => vec![(0, Go), (20, Resize(80, 24)), (60, Key(b'r'))],
        "key-resize" => vec![(0, Key(b'r')), (10, Resize(80, 24)), (20, Go)],
        "coincident" => vec![(0, Resize(80, 24)), (10, Key(b'r')), (20, Go)],
        "slow-drain" => vec![
            (0, Go),
            (10, Pause),
            (10 + (c.pause_ms / 10).min(2), Resize(56, 24)),
            (10 + c.pause_ms / 2, Key(b'r')),
            (10 + c.pause_ms, Resume),
        ],
        "storm" => vec![
            (0, Go),
            (20, Resize(56, 24)),
            (25, Key(b'a')),
            (40, Resize(80, 24)),
            (45, Key(b'b')),
            (60, Resize(160, 40)),
            (65, Key(b'c')),
        ],
        _ => vec![(0, Go), (30, Key(b'a')), (50, Key(b'b')), (70, Key(b'c'))],
    }
}
pub fn analyze(records: &[Record], drained: u64, restored: bool) -> Report {
    let writes: Vec<_> = records
        .iter()
        .filter(|r| r.source == "PTY" && r.kind == "Write")
        .collect();
    let keys: Vec<_> = records
        .iter()
        .filter(|r| r.source == "APP" && r.kind == "Key")
        .collect();
    let text = |items: &[&Record]| {
        items
            .iter()
            .filter_map(|r| r.value.parse().ok().and_then(char::from_u32))
            .collect::<String>()
    };
    let sent = text(&writes);
    let received = text(&keys);
    let valid = writes.iter().chain(&keys).all(|r| {
        r.value
            .parse::<u32>()
            .ok()
            .and_then(char::from_u32)
            .is_some()
    });
    let missed_deadline = records
        .iter()
        .any(|r| r.source == "PTY" && r.kind == "VerdictWindow" && r.value == "deadline");
    let causal = writes.iter().zip(&keys).all(|(a, b)| a.us <= b.us);
    let latencies_us = if sent == received && !missed_deadline && causal && valid {
        writes
            .iter()
            .zip(&keys)
            .map(|(a, b)| b.us.saturating_sub(a.us))
            .collect()
    } else {
        vec![]
    };
    let mut verdict = if sent == received && !sent.is_empty() && !missed_deadline && causal {
        "DELIVERED"
    } else {
        "DEADLINE / UNRESOLVED"
    };
    let last_write = writes.last().map_or(0, |r| r.us);
    let readable = records.iter().rev().find(|r| r.kind == "Readable");
    let empties = records
        .iter()
        .filter(|r| r.kind == "Empty" && r.us > last_write)
        .count();
    if sent != received
        && readable.is_some_and(|r| r.us > last_write.saturating_add(50_000))
        && empties >= 3
    {
        verdict = "READABLE / NOT DELIVERED";
    }
    if sent == received && missed_deadline {
        verdict = "DEADLINE / LATE DELIVERY";
    }
    if !causal {
        verdict = "INVALID CAUSAL TIMESTAMPS";
    }
    if !valid {
        verdict = "INVALID KEY RECEIPT";
    }
    if records
        .iter()
        .any(|r| r.source == "PTY" && r.kind == "Exit" && r.value != "0")
    {
        verdict = "CHILD FAILED";
    } else if records
        .iter()
        .any(|r| r.source == "PTY" && r.kind == "TerminalRestoration" && r.value != "true")
    {
        verdict = "RESTORATION FAILED";
    }
    let commits: Vec<_> = records
        .iter()
        .filter(|r| r.kind == "FrameCommitted")
        .collect();
    let generated = commits
        .last()
        .and_then(|r| r.value.split(':').nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    Report {
        sent,
        received,
        verdict: verdict.into(),
        latencies_us,
        drained,
        generated,
        frames: commits.len() as u64,
        restored,
        records: records.to_vec(),
    }
}
pub fn run(exe: &Path, c: Config) -> io::Result<Report> {
    let mut trial = Trial::start(exe, c)?;
    while !trial.tick()? {
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(trial.report.take().unwrap())
}
