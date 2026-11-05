//! Issue #254 — the documented terminal-write contract must match `src/transaction.rs`.
//!
//! `TerminalTransaction::commit_counted` is a counted write loop plus one `flush`. A write can fail
//! after a prefix reached the terminal, so a frame is **not** all-or-nothing I/O, and
//! synchronized-update framing (`CSI ?2026`) is a presentation request, not a transport guarantee.
//! The renderer's recovery (RXIV Gate G3, `tests/renderer_partial_write.rs`) depends on that
//! distinction; documentation that promises an atomic terminal write invites integrators to skip
//! partial-write recovery.
//!
//! This is a cheap textual guard over the *current* documentation and the public comments, not a
//! proof: it fails on the specific overclaims #254 found (and close paraphrases), and it checks
//! that the counted-write contract is stated where readers look for it. Filesystem atomicity
//! (`capture::atomic_write` for capture and trace files) is a different, tested mechanism and is
//! deliberately out of scope: lines that talk about files are skipped.

use std::path::Path;

/// Current, reader-facing documents and the public/source comments that describe the terminal
/// write path. Historical design notes (`docs/acid-vs-crash-validation.md`, the v0.5 roadmap) are
/// annotated in place instead of policed here.
const SCOPE: &[&str] = &[
    "README.md",
    "DESIGN.md",
    "ROADMAP.md",
    "docs/FEATURES.md",
    "docs/RELEASE_CONTRACT.md",
    "docs/STATE_OF_LIBGIBSON.md",
    "include/gibson.h",
    "bindings/go/gibson/gibson.go",
    "bindings/python/gibson.py",
    "src/lib.rs",
    "src/context.rs",
    "src/session.rs",
    "src/renderer.rs",
    "src/transaction.rs",
];

/// Lower-cased phrases that promise all-or-nothing terminal I/O (or an infallible anchor).
const OVERCLAIMS: &[&str] = &[
    "atomic write",
    "atomic terminal",
    "atomic `terminaltransaction`",
    "atomic terminaltransaction",
    "atomic transaction",
    "atomic wire",
    "wire-atomic",
    "atomic from the terminal",
    "always knows",
];

fn read(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A phrase preceded by an explicit negation ("not an atomic write") states the contract.
fn negated(line: &str, at: usize) -> bool {
    let tail: String = line[..at]
        .chars()
        .rev()
        .take(16)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    tail.contains("not ")
        || tail.contains("not** ")
        || tail.contains("never ")
        || tail.ends_with("no ")
}

/// Every overclaim in `text`, as `file:line: excerpt`.
fn overclaims(rel: &str, text: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let lower = line.to_lowercase();
        // Filesystem atomicity is a separate, tested contract.
        if lower.contains("file") || lower.contains("atomic_write") {
            continue;
        }
        for phrase in OVERCLAIMS {
            let mut from = 0;
            while let Some(i) = lower[from..].find(phrase) {
                let at = from + i;
                if !negated(&lower, at) {
                    hits.push(format!("{rel}:{}: {phrase:?} in {:?}", n + 1, line.trim()));
                }
                from = at + phrase.len();
            }
        }
        // The pre-RXIV method name: `TerminalTransaction::commit` without `_counted`.
        let old = "terminaltransaction::commit";
        let mut from = 0;
        while let Some(i) = lower[from..].find(old) {
            let end = from + i + old.len();
            if !lower[end..].starts_with("_counted") {
                hits.push(format!(
                    "{rel}:{}: obsolete `TerminalTransaction::commit` in {:?}",
                    n + 1,
                    line.trim()
                ));
            }
            from = end;
        }
    }
    hits
}

#[test]
fn current_documentation_never_promises_an_atomic_terminal_write() {
    let hits: Vec<String> = SCOPE
        .iter()
        .flat_map(|rel| overclaims(rel, &read(rel)))
        .collect();
    assert!(
        hits.is_empty(),
        "issue #254: documentation promises all-or-nothing terminal I/O (or names the obsolete \
         `TerminalTransaction::commit`). The contract is a counted write loop + flush; a torn prefix \
         is possible and is recovered, not prevented (src/transaction.rs, Release Contract §9). \
         Rephrase, or negate explicitly:\n  {}",
        hits.join("\n  ")
    );
}

#[test]
fn the_counted_write_contract_is_stated_where_readers_look() {
    // Positive control: the guard above could be satisfied by deleting every sentence about the
    // write path. The contract must actually be stated.
    for (rel, needles) in [
        ("README.md", &["not atomic", "physical_state_uncertain"][..]),
        (
            "DESIGN.md",
            &[
                "commit_counted",
                "not atomic",
                "is_certain_noop",
                "scrollback",
            ][..],
        ),
        ("docs/FEATURES.md", &["not atomic", "commit_counted"][..]),
        (
            "docs/RELEASE_CONTRACT.md",
            &["terminal writes are not atomic", "presentation request"][..],
        ),
    ] {
        let lower = read(rel).to_lowercase();
        for needle in needles {
            assert!(
                lower.contains(&needle.to_lowercase()),
                "issue #254: {rel} must state the counted-write contract (missing {needle:?})"
            );
        }
    }
}

#[test]
fn the_guard_flags_the_overclaims_it_exists_for_and_spares_the_true_statements() {
    // Teeth: the exact sentences #254 found at 4e33631 are flagged …
    for bad in [
        "The rendering path is a clean vertical pipeline — a single atomic write per frame,",
        "with an explicit physical **anchor** so the renderer always knows where its live",
        "and one atomic `TerminalTransaction` per frame with synchronized-update framing.",
        "`TerminalTransaction` represents a single atomic write to the terminal.",
        "This is what makes a frame atomic from the terminal's perspective",
        "- **Atomic `TerminalTransaction`** (TESTED): batches sync-update markers, temporary",
        "and `?2026` guarantees only a wire-atomic write, not atomic panel presentation.",
        "`transaction::TerminalTransaction` is internal renderer plumbing (the atomic wire",
        "// SetSyncUpdates enables or disables atomic terminal updates (DECSM 2026).",
        "`TerminalTransaction::commit` performs **one `write_all` followed by one `flush`**.",
    ] {
        assert!(!overclaims("probe", bad).is_empty(), "guard misses {bad:?}");
    }
    // … and true statements, including filesystem atomicity, are not.
    for good in [
        "- **Terminal writes are not atomic.** A failed write may leave a torn prefix of a frame on the",
        "It is not an atomic write: a prefix can reach the terminal.",
        "strict, bounded, atomically committed capture files and a causal trace (v2)",
        "files commit atomically (`atomic_write`).",
        "`TerminalTransaction::commit_counted` writes until every byte is accepted",
    ] {
        assert!(overclaims("probe", good).is_empty(), "guard flags a true statement {good:?}");
    }
}
