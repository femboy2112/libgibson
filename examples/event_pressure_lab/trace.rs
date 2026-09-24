//! Lab-local, bounded, out-of-band receipts. Linux/Unix CLOCK_MONOTONIC is shared
//! across these processes; timestamps are offsets from the parent's epoch.
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

pub const MAX_RECORDS: usize = 65_536;
pub const MAX_TRACE_BYTES: u64 = 8 * 1024 * 1024;

pub fn clock_us() -> u64 {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: initialized timespec, valid monotonic clock; no borrowed fd.
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut t) },
        0
    );
    (t.tv_sec as u64).saturating_mul(1_000_000) + t.tv_nsec as u64 / 1000
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub seq: u64,
    pub us: u64,
    pub source: String,
    pub kind: String,
    pub value: String,
}
impl Record {
    pub fn line(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}\n",
            self.seq, self.us, self.source, self.kind, self.value
        )
    }
    pub fn parse(line: &str) -> Option<Self> {
        let p: Vec<_> = line.split('\t').collect();
        if p.len() != 5 || line.len() > 512 || p.iter().any(|s| s.chars().any(char::is_control)) {
            return None;
        }
        Some(Self {
            seq: p[0].parse().ok()?,
            us: p[1].parse().ok()?,
            source: p[2].into(),
            kind: p[3].into(),
            value: p[4].into(),
        })
    }
}

pub struct Recorder {
    file: File,
    epoch: u64,
    seq: u64,
}
impl Recorder {
    pub fn new(path: &Path, epoch: u64) -> io::Result<Self> {
        Ok(Self {
            file: OpenOptions::new().create_new(true).write(true).open(path)?,
            epoch,
            seq: 0,
        })
    }
    pub fn emit(&mut self, source: &str, kind: &str, value: impl ToString) -> io::Result<()> {
        if self.seq >= MAX_RECORDS as u64 {
            return Err(io::Error::other("trace record budget exhausted"));
        }
        let value = value.to_string();
        if value.len() > 160
            || [source, kind, &value]
                .iter()
                .any(|s| s.chars().any(char::is_control))
        {
            return Err(io::Error::other("invalid diagnostic payload"));
        }
        let r = Record {
            seq: self.seq,
            us: clock_us().saturating_sub(self.epoch),
            source: source.into(),
            kind: kind.into(),
            value,
        };
        self.seq += 1;
        self.file.write_all(r.line().as_bytes())
    }
}
