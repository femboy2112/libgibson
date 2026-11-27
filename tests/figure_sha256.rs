#![cfg(feature = "figure")]
//! SHA-256: NIST vectors, padding boundaries, differential vs `sha2`, incremental splits.

use gibson::figure::sha256::{digest, Sha256};
use sha2::Digest as _;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn oracle(data: &[u8]) -> [u8; 32] {
    let r = sha2::Sha256::digest(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(r.as_slice());
    out
}

#[test]
fn nist_empty() {
    assert_eq!(
        hex(&digest(b"")),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn nist_abc() {
    assert_eq!(
        hex(&digest(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn nist_448_bit() {
    let m = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    assert_eq!(m.len() * 8, 448);
    assert_eq!(
        hex(&digest(m)),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn nist_896_bit_two_block() {
    let m = b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu";
    assert_eq!(m.len() * 8, 896);
    assert_eq!(
        hex(&digest(m)),
        "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1"
    );
}

#[test]
fn nist_one_million_a() {
    let m = vec![b'a'; 1_000_000];
    assert_eq!(
        hex(&digest(&m)),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
}

#[test]
fn padding_boundaries_match_oracle() {
    for len in [
        0usize, 1, 54, 55, 56, 57, 63, 64, 65, 119, 120, 127, 128, 129,
    ] {
        let m: Vec<u8> = (0..len).map(|i| (i * 7 + 3) as u8).collect();
        assert_eq!(digest(&m), oracle(&m), "length {len}");
    }
}

#[test]
fn padding_boundaries_known_values() {
    // 55 'a': last length that fits one block; 56 'a': forces a second block.
    let h = |n: usize| hex(&digest(&vec![b'a'; n]));
    assert_eq!(
        h(55),
        "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318"
    );
    assert_eq!(
        h(56),
        "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a"
    );
    assert_eq!(
        h(64),
        "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb"
    );
}

/// SplitMix64: tiny deterministic generator, no dependency.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.next() as u8).collect()
    }
}

const SEED: u64 = 0x5EED_F5A2_5600_0001;

#[test]
fn differential_against_sha2() {
    println!("differential seed = {SEED:#x}");
    let mut rng = Rng(SEED);
    for len in 0..=300usize {
        let m = rng.bytes(len);
        assert_eq!(digest(&m), oracle(&m), "seed {SEED:#x} len {len}");
    }
    for len in [1_000usize, 4_095, 4_096, 4_097, 65_536, 100_003, 1_000_000] {
        let m = rng.bytes(len);
        assert_eq!(digest(&m), oracle(&m), "seed {SEED:#x} len {len}");
    }
}

#[test]
fn incremental_split_equivalence() {
    println!("split seed = {SEED:#x}");
    let mut rng = Rng(SEED ^ 0xABCD);
    for len in [0usize, 1, 55, 56, 63, 64, 65, 119, 200, 300] {
        let m = rng.bytes(len);
        let want = digest(&m);
        // Every single split point.
        for cut in 0..=len {
            let mut h = Sha256::new();
            h.update(&m[..cut]);
            h.update(&m[cut..]);
            assert_eq!(h.finalize(), want, "len {len} cut {cut}");
        }
        // Byte-at-a-time.
        let mut h = Sha256::new();
        for b in &m {
            h.update(std::slice::from_ref(b));
        }
        assert_eq!(h.finalize(), want, "len {len} bytewise");
        // Empty updates are no-ops.
        let mut h = Sha256::default();
        h.update(&[]);
        h.update(&m);
        h.update(&[]);
        assert_eq!(h.finalize(), want, "len {len} empty updates");
    }
}

#[test]
fn incremental_random_chunks() {
    let mut rng = Rng(SEED ^ 0x1234);
    let m = rng.bytes(10_000);
    let want = oracle(&m);
    let mut h = Sha256::new();
    let mut i = 0;
    while i < m.len() {
        let n = ((rng.next() % 200) as usize + 1).min(m.len() - i);
        h.update(&m[i..i + n]);
        i += n;
    }
    assert_eq!(h.finalize(), want);
}
