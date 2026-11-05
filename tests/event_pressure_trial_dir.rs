//! #255: an event-pressure trial directory must be unique per trial, whatever the clock reads.
//!
//! Before the fix, `Trial::start` named the directory `gibson-pressure-{pid}-{µs}` from
//! CLOCK_MONOTONIC and created it with `fs::create_dir`. Parallel test threads share a pid, so
//! two trials starting in the same microsecond built the same path and the second failed with
//! `EEXIST` — a false failure of a test whose code under test was fine.
//!
//! The legacy strategy is reproduced verbatim below as the negative control. With a frozen
//! clock and barrier-synchronized starts it collides deterministically: one creator wins and
//! every other gets `AlreadyExists`, with no dependence on scheduling luck.
#![cfg(unix)]
#![allow(dead_code)]

#[path = "../examples/event_pressure_lab/probe.rs"]
mod probe;
#[path = "../examples/event_pressure_lab/trace.rs"]
mod trace;

use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    sync::Barrier,
    thread,
};

const THREADS: usize = 16;
const ROUNDS: usize = 8;
/// A frozen CLOCK_MONOTONIC reading: every racer sees the same microsecond.
const FROZEN_US: u64 = 42;

/// A test-owned scratch root, removed on drop, so these tests never meet a real trial.
struct Root(PathBuf);
impl Root {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!("gibson-255-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).expect("test root");
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The pre-#255 naming and creation, verbatim from `probe.rs` before the fix.
fn legacy_create(root: &Path, epoch: u64) -> io::Result<PathBuf> {
    let dir = root.join(format!("gibson-pressure-{}-{epoch}", std::process::id()));
    fs::create_dir(&dir)?;
    Ok(dir)
}

/// Release `THREADS` creators together through a barrier; creator `i` uses `epoch_of(i)`.
/// Returns the successes and the number of `AlreadyExists` failures; any other error panics.
fn race<T: Send>(
    create: impl Fn(u64) -> io::Result<T> + Sync,
    epoch_of: impl Fn() -> u64 + Sync,
) -> (Vec<T>, usize) {
    let barrier = Barrier::new(THREADS);
    let results: Vec<io::Result<T>> = thread::scope(|s| {
        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                s.spawn(|| {
                    barrier.wait();
                    create(epoch_of())
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("racer"))
            .collect()
    });
    let mut ok = Vec::new();
    let mut eexist = 0;
    for r in results {
        match r {
            Ok(v) => ok.push(v),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => eexist += 1,
            Err(e) => panic!("unexpected creation error: {e}"),
        }
    }
    (ok, eexist)
}

fn seq_of(dir: &Path) -> u64 {
    let name = dir.file_name().unwrap().to_str().unwrap();
    name.rsplit('-')
        .next()
        .unwrap()
        .parse()
        .expect("sequence suffix")
}

#[test]
fn legacy_name_collides_deterministically_on_a_shared_microsecond() {
    let root = Root::new("legacy");
    let (ok, eexist) = race(|epoch| legacy_create(&root.0, epoch), || FROZEN_US);
    assert_eq!(
        (ok.len(), eexist),
        (1, THREADS - 1),
        "the legacy pid+µs name is shared by every racer: exactly one create_dir can win"
    );
}

#[test]
fn sequenced_trial_dirs_never_collide_on_a_shared_microsecond() {
    let root = Root::new("frozen");
    let (dirs, eexist) = race(
        |epoch| probe::create_trial_dir(&root.0, epoch),
        || FROZEN_US,
    );
    assert_eq!(eexist, 0, "a shared microsecond must not produce EEXIST");
    assert_eq!(dirs.len(), THREADS);
    let paths: HashSet<PathBuf> = dirs.iter().map(|d| d.to_path_buf()).collect();
    assert_eq!(
        paths.len(),
        THREADS,
        "every trial owns a distinct directory"
    );
    assert!(paths.iter().all(|p| p.is_dir()));
    drop(dirs);
    assert!(
        paths.iter().all(|p| !p.exists()),
        "the guard still removes each trial directory on drop"
    );
}

#[test]
fn sequenced_trial_dirs_never_collide_under_the_real_clock() {
    let root = Root::new("realclock");
    let mut legacy_collisions = 0;
    for _ in 0..ROUNDS {
        let (dirs, eexist) = race(
            |epoch| probe::create_trial_dir(&root.0, epoch),
            trace::clock_us,
        );
        assert_eq!((dirs.len(), eexist), (THREADS, 0));
        let legacy_root = Root::new("realclock-legacy");
        let (_, legacy) = race(
            |epoch| legacy_create(&legacy_root.0, epoch),
            trace::clock_us,
        );
        legacy_collisions += legacy;
    }
    // Informational only: the real clock makes the legacy rate host-dependent, so it is
    // reported, never asserted. The deterministic proof is the frozen-clock pair above.
    eprintln!(
        "legacy collisions under the real clock: {legacy_collisions} of {}",
        ROUNDS * THREADS
    );
}

#[test]
fn an_existing_directory_is_skipped_never_adopted() {
    let root = Root::new("stale");
    // Any fixed clock reading works: this test owns its root.
    let epoch = 255_000_001;
    // Stale directories (as a killed process with a recycled pid could leave) at every even
    // sequence, each holding a marker the new trial must never see.
    for seq in (0..1024).step_by(2) {
        let stale = root
            .0
            .join(probe::trial_dir_name(std::process::id(), epoch, seq));
        fs::create_dir(&stale).unwrap();
        fs::write(stale.join("marker"), b"stale").unwrap();
    }
    for _ in 0..8 {
        let dir = probe::create_trial_dir(&root.0, epoch).expect("a free name within bounds");
        assert_eq!(seq_of(&dir) % 2, 1, "an existing directory was adopted");
        assert!(!dir.join("marker").exists(), "trial dir must start empty");
    }
    for seq in (0..1024).step_by(2) {
        let stale = root
            .0
            .join(probe::trial_dir_name(std::process::id(), epoch, seq));
        assert!(
            stale.join("marker").exists(),
            "a stale directory was removed"
        );
    }
}

#[test]
fn exhausting_the_bounded_attempts_is_an_error_not_a_hang() {
    let root = Root::new("exhaust");
    // Any fixed clock reading works: this test owns its root.
    let epoch = 255_000_002;
    // Occupy far more names than this binary can consume, so every bounded attempt meets an
    // existing directory and the creator must give up with AlreadyExists.
    for seq in 0..1024 {
        fs::create_dir(
            root.0
                .join(probe::trial_dir_name(std::process::id(), epoch, seq)),
        )
        .unwrap();
    }
    let err = probe::create_trial_dir(&root.0, epoch)
        .err()
        .expect("every candidate name is occupied");
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
}

#[test]
fn other_creation_errors_propagate_without_retry() {
    let missing = std::env::temp_dir().join(format!("gibson-255-missing-{}", std::process::id()));
    let _ = fs::remove_dir_all(&missing);
    let err = probe::create_trial_dir(&missing, FROZEN_US)
        .err()
        .expect("a missing root cannot hold a trial directory");
    assert_eq!(err.kind(), io::ErrorKind::NotFound);
}
