//! Bounded Unix PTY capture for interactive, lifecycle and golden probes.
//!
//! Reads are nonblocking and polled on the test thread: there is no detached
//! reader to outlive a test. The capture budget reserves two seconds for killing
//! and reaping the direct child. This does not supervise arbitrary descendants.

// Each integration-test binary uses a different subset of this shared helper.
#![allow(dead_code)]

use portable_pty::{native_pty_system, Child, CommandBuilder, ExitStatus, MasterPty, PtySize};
use std::collections::BTreeSet;
use std::io;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const CLEANUP: Duration = Duration::from_secs(2);
const MAX_CAPTURE: usize = 16 * 1024 * 1024;

/// Ask Cargo to check freshness once per example in this test process. Merely
/// finding an executable is insufficient after editing an example or the core.
pub fn example_path(name: &str) -> PathBuf {
    static BUILT: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();
    let mut built = BUILT.get_or_init(Default::default).lock().unwrap();
    let exe = std::env::current_exe().expect("current executable");
    let profile = exe.parent().unwrap().parent().unwrap();
    if !built.contains(name) {
        let mut cmd = std::process::Command::new(env!("CARGO"));
        cmd.current_dir(env!("CARGO_MANIFEST_DIR"))
            .args(["build", "--example", name, "--target-dir"])
            .arg(profile.parent().unwrap());
        match profile.file_name().unwrap().to_str().unwrap() {
            "debug" => {}
            "release" => {
                cmd.arg("--release");
            }
            other => panic!("PTY example preparation does not support profile {other}"),
        }
        let mut child = cmd.spawn().expect("build example");
        let deadline = Instant::now() + Duration::from_secs(180);
        let status = loop {
            if let Some(status) = child.try_wait().expect("poll example build") {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let cleanup_end = Instant::now() + CLEANUP;
                let reaped = loop {
                    if matches!(child.try_wait(), Ok(Some(_))) {
                        break true;
                    }
                    if Instant::now() >= cleanup_end {
                        break false;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                };
                panic!("example {name} build exceeded 180 seconds (direct child reaped: {reaped})");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(status.success(), "example {name} build failed");
        built.insert(name.into());
    }
    profile.join("examples").join(name)
}

pub struct Capture {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    deadline: Instant,
    reaped: bool,
    pub initial_termios: Option<String>,
    output: Vec<u8>,
}

impl Capture {
    pub fn spawn(cmd: CommandBuilder, cols: u16, rows: u16, budget: Duration) -> Self {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("open PTY");
        let fd = pair.master.as_raw_fd().expect("Unix PTY descriptor");
        // SAFETY: master owns this live descriptor throughout both calls. Flags
        // affect only this test's PTY; no other code reads from the master.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        assert!(flags >= 0, "get PTY flags: {}", io::Error::last_os_error());
        assert!(
            unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } >= 0,
            "set nonblocking PTY: {}",
            io::Error::last_os_error()
        );
        let initial_termios = pair.master.get_termios().map(|t| format!("{t:?}"));
        let deadline = Instant::now() + budget + CLEANUP;
        let child = pair.slave.spawn_command(cmd).expect("spawn PTY child");
        // A retained slave would prevent EOF when the child exits.
        drop(pair.slave);
        Self {
            master: pair.master,
            child,
            deadline,
            reaped: false,
            initial_termios,
            output: Vec::new(),
        }
    }

    pub fn collect_until(&mut self, done: impl Fn(&[u8]) -> bool) -> io::Result<()> {
        self.collect_before(self.deadline - CLEANUP, done)
    }

    fn collect_before(
        &mut self,
        capture_end: Instant,
        done: impl Fn(&[u8]) -> bool,
    ) -> io::Result<()> {
        while Instant::now() < capture_end && !done(&self.output) {
            let fd = self.master.as_raw_fd().unwrap();
            let mut pollfd = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            let remaining = capture_end.saturating_duration_since(Instant::now());
            let timeout = remaining.as_millis().min(20) as libc::c_int;
            // SAFETY: pollfd points to one initialized entry, fd remains owned.
            let ready = unsafe { libc::poll(&mut pollfd, 1, timeout) };
            if ready < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(err);
            }
            if ready == 0 {
                continue;
            }
            let mut chunk = [0u8; 8192];
            // SAFETY: chunk is writable for its full length; master still owns fd.
            let len = unsafe { libc::read(fd, chunk.as_mut_ptr().cast(), chunk.len()) };
            if len == 0 {
                break;
            }
            if len < 0 {
                let err = io::Error::last_os_error();
                // Linux signals slave closure as EIO rather than EOF.
                if err.raw_os_error() == Some(libc::EIO) {
                    break;
                }
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) {
                    continue;
                }
                return Err(err);
            }
            let len = len as usize;
            if self.output.len() + len > MAX_CAPTURE {
                return Err(io::Error::other(
                    "PTY output exceeded 16 MiB capture budget",
                ));
            }
            self.output.extend_from_slice(&chunk[..len]);
        }
        Ok(())
    }

    pub fn collect_for(&mut self, duration: Duration) -> io::Result<()> {
        let end = self.deadline - CLEANUP;
        if Instant::now() >= end {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "PTY session deadline expired",
            ));
        }
        self.collect_before((Instant::now() + duration).min(end), |_| false)
    }

    pub fn raw(&self) -> &[u8] {
        &self.output
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let status = self.child.try_wait()?;
        if status.is_some() {
            self.reaped = true;
        }
        Ok(status)
    }

    pub fn termios(&self) -> Option<String> {
        self.master.get_termios().map(|t| format!("{t:?}"))
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("resize PTY");
    }

    pub fn write(&mut self, mut bytes: &[u8]) -> io::Result<()> {
        let until = (Instant::now() + Duration::from_secs(1)).min(self.deadline - CLEANUP);
        while !bytes.is_empty() {
            if Instant::now() >= until {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "PTY input deadline expired",
                ));
            }
            let fd = self.master.as_raw_fd().unwrap();
            // SAFETY: bytes is readable and the master owns this nonblocking fd.
            let n = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
            if n > 0 {
                bytes = &bytes[n as usize..];
                continue;
            }
            if n < 0 {
                let err = io::Error::last_os_error();
                if !matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) {
                    return Err(err);
                }
            }
            // Drain output too: a child may be waiting for us before reading.
            self.collect_for(Duration::from_millis(2))?;
        }
        Ok(())
    }

    pub fn terminate(&mut self) {
        assert!(
            self.cleanup(),
            "PTY child did not reap before cleanup deadline"
        );
    }

    pub fn process_id(&self) -> u32 {
        self.child.process_id().expect("Unix child process id")
    }

    pub fn finish(mut self) -> Vec<u8> {
        assert!(
            self.cleanup(),
            "PTY child {} did not reap before cleanup deadline",
            self.process_id()
        );
        std::mem::take(&mut self.output)
    }

    fn cleanup(&mut self) -> bool {
        if self.reaped {
            return true;
        }
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            self.reaped = true;
            return true;
        }
        // portable-pty's Unix kill first waits for SIGHUP handlers. A direct
        // SIGKILL keeps forced test cleanup inside our one explicit deadline.
        // SAFETY: an unreaped direct child's PID cannot be reused by the OS.
        unsafe {
            libc::kill(self.process_id() as libc::pid_t, libc::SIGKILL);
        }
        let cleanup_end = self.deadline.min(Instant::now() + CLEANUP);
        loop {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                self.reaped = true;
                return true;
            }
            if Instant::now() >= cleanup_end {
                return false;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        // Runs on assertion failure and I/O error too. No blocking wait or join.
        if !self.cleanup() {
            eprintln!("PTY cleanup deadline expired; child could not be reaped");
        }
    }
}
