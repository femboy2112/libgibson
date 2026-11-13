//! A dedicated output PTY can fail while stdin/stderr retain the reporting terminal.
//! Install before session creation; disconnect the actual captured endpoint afterwards.
use std::io::{self, Write};
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};

pub struct OutputFault {
    master: Option<OwnedFd>,
    saved: Option<OwnedFd>,
}
impl OutputFault {
    pub fn new() -> io::Result<Self> {
        io::stdout().flush()?;
        let saved = io::stdout().as_fd().try_clone_to_owned()?;
        let mut master = -1;
        let mut slave = -1;
        let size = libc::winsize {
            ws_row: 24,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: valid descriptor out-pointers, default termios, initialized fixed geometry.
        if unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                &size,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: successful openpty returned two new owned descriptors.
        let master = unsafe { OwnedFd::from_raw_fd(master) };
        let slave = unsafe { OwnedFd::from_raw_fd(slave) };
        for fd in [&master, &slave] {
            // SAFETY: each descriptor is owned and live; preserve no executable inheritance.
            if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } != 0 {
                return Err(io::Error::last_os_error());
            }
        }
        // SAFETY: replace stdout only in a dedicated child; saved owns its original destination.
        if unsafe { libc::dup2(slave.as_raw_fd(), libc::STDOUT_FILENO) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            master: Some(master),
            saved: Some(saved),
        })
    }
    pub fn disconnect(&mut self) {
        // A calibrated negative control: leaving the master alive must make error assertions fail.
        if std::env::var_os("GIBSON_TEST_OUTPUT_FAULT_KEEP_MASTER").is_none() {
            drop(self.master.take());
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
            loop {
                let mut fd = libc::pollfd {
                    fd: libc::STDOUT_FILENO,
                    events: 0,
                    revents: 0,
                };
                // SAFETY: this dedicated child still owns the captured output's slave on fd 1.
                let ready = unsafe { libc::poll(&mut fd, 1, 10) };
                if ready > 0 && fd.revents & (libc::POLLHUP | libc::POLLERR) != 0 {
                    eprintln!("OUTPUT_HUP_OBSERVED=1");
                    break;
                }
                if ready < 0 {
                    assert_eq!(
                        io::Error::last_os_error().kind(),
                        io::ErrorKind::Interrupted
                    );
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "output hangup not observed"
                );
            }
        }
    }
    pub fn restore_stdout(&mut self) -> io::Result<()> {
        if let Some(saved) = &self.saved {
            // SAFETY: the saved descriptor is live and owned, and fd 1 is this child's stdout.
            if unsafe { libc::dup2(saved.as_raw_fd(), libc::STDOUT_FILENO) } < 0 {
                return Err(io::Error::last_os_error());
            }
            self.saved = None;
        }
        Ok(())
    }
}
impl Drop for OutputFault {
    fn drop(&mut self) {
        let _ = self.restore_stdout();
    }
}
