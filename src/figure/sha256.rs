//! SHA-256 (FIPS 180-4), hand-written, scalar, safe Rust.
//!
//! This is a **content identifier**, not a security primitive. It names a compiled character
//! revision by its bytes (`rev_id`, asset manifests). Nothing here makes a constant-time,
//! side-channel-resistance, MAC, key-derivation or any other security claim, and it must not be
//! used where an attacker's view of timing or memory access matters.
//!
//! Design choice: the coordinator ruled (#39) for zero runtime dependencies, so the digest is
//! written here instead of pulling in a hashing crate. Correctness is established by the NIST
//! vectors and by a differential test against the `sha2` crate, which is a dev-dependency only.

/// Incremental SHA-256 hasher.
#[derive(Debug, Clone)]
pub struct Sha256 {}

impl Sha256 {
    /// A fresh hasher.
    pub fn new() -> Self {
        todo!("RED stub")
    }

    /// Absorb more bytes.
    pub fn update(&mut self, _data: &[u8]) {
        todo!("RED stub")
    }

    /// Finish and return the 32-byte digest.
    pub fn finalize(self) -> [u8; 32] {
        todo!("RED stub")
    }
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

/// One-shot SHA-256 of `data`.
pub fn digest(_data: &[u8]) -> [u8; 32] {
    todo!("RED stub")
}
