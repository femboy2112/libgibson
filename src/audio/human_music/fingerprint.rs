//! Canonical v2 identities, independent of Rust `Debug` and memory layout.
//!
//! The stream starts with a schema identifier. Tags are length-prefixed UTF-8;
//! integers use little endian, lengths/usize use u64, enum discriminants are explicit
//! schema strings, and sequences preserve declared order. Floats use IEEE bits with
//! both signed zeros normalized to +0 and all NaNs to one quiet NaN. Maps require an
//! explicit sorted projection; there is deliberately no blanket map implementation.
//!
//! FNV-1a is a deterministic receipt digest, **not** a cryptographic identity proof.
//! Equality laws compare semantic values as well as digests. Historical `fingerprint()`
//! APIs retain their exact Debug-based v1 formulas; v2 is an explicit migration.

/// A streaming canonical encoder. Field tags and order belong to the schema.
pub struct FingerprintWriter {
    hash: u64,
}

impl Default for FingerprintWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl FingerprintWriter {
    pub fn new() -> Self {
        let mut writer = Self {
            hash: 0xcbf2_9ce4_8422_2325,
        };
        writer.tag("humanmusic-canonical/v2");
        writer
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.hash = (self.hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }

    /// A schema, field, or enum-discriminant tag; never a Debug representation.
    pub fn tag(&mut self, tag: &str) {
        self.bytes(&(tag.len() as u64).to_le_bytes());
        self.bytes(tag.as_bytes());
    }

    pub fn field<T: CanonicalFingerprint + ?Sized>(&mut self, tag: &str, value: &T) {
        self.tag(tag);
        value.encode(self);
    }

    pub fn finish(self) -> u64 {
        self.hash
    }
}

/// Explicit semantic encoding. Adding/changing a load-bearing field requires schema review.
pub trait CanonicalFingerprint {
    fn encode(&self, writer: &mut FingerprintWriter);

    fn canonical_fingerprint(&self) -> u64 {
        let mut writer = FingerprintWriter::new();
        self.encode(&mut writer);
        writer.finish()
    }
}

macro_rules! integer {
    ($($ty:ty => $tag:literal),+ $(,)?) => {$ (
        impl CanonicalFingerprint for $ty {
            fn encode(&self, w: &mut FingerprintWriter) {
                w.tag($tag);
                w.bytes(&self.to_le_bytes());
            }
        }
    )+};
}
integer!(u8 => "u8", u16 => "u16", u32 => "u32", u64 => "u64",
    i8 => "i8", i16 => "i16", i32 => "i32", i64 => "i64");

impl CanonicalFingerprint for usize {
    fn encode(&self, w: &mut FingerprintWriter) {
        (*self as u64).encode(w);
    }
}
impl CanonicalFingerprint for bool {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("bool");
        w.bytes(&[u8::from(*self)]);
    }
}
impl CanonicalFingerprint for f32 {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("f32");
        let bits = if *self == 0.0 {
            0
        } else if self.is_nan() {
            0x7fc0_0000
        } else {
            self.to_bits()
        };
        w.bytes(&bits.to_le_bytes());
    }
}
impl CanonicalFingerprint for f64 {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("f64");
        let bits = if *self == 0.0 {
            0
        } else if self.is_nan() {
            0x7ff8_0000_0000_0000
        } else {
            self.to_bits()
        };
        w.bytes(&bits.to_le_bytes());
    }
}
impl CanonicalFingerprint for str {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("str");
        w.tag(self);
    }
}
impl CanonicalFingerprint for String {
    fn encode(&self, w: &mut FingerprintWriter) {
        self.as_str().encode(w);
    }
}
impl<T: CanonicalFingerprint + ?Sized> CanonicalFingerprint for &T {
    fn encode(&self, w: &mut FingerprintWriter) {
        (*self).encode(w);
    }
}
impl<T: CanonicalFingerprint> CanonicalFingerprint for [T] {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("sequence");
        (self.len() as u64).encode(w);
        for value in self {
            value.encode(w);
        }
    }
}
impl<T: CanonicalFingerprint> CanonicalFingerprint for Vec<T> {
    fn encode(&self, w: &mut FingerprintWriter) {
        self.as_slice().encode(w);
    }
}
impl<T: CanonicalFingerprint, const N: usize> CanonicalFingerprint for [T; N] {
    fn encode(&self, w: &mut FingerprintWriter) {
        self.as_slice().encode(w);
    }
}
impl<T: CanonicalFingerprint> CanonicalFingerprint for Option<T> {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("option");
        match self {
            None => w.tag("none"),
            Some(value) => {
                w.tag("some");
                value.encode(w);
            }
        }
    }
}
macro_rules! tuple {
    ($($ty:ident : $index:tt),+) => {
        impl<$($ty: CanonicalFingerprint),+> CanonicalFingerprint for ($($ty,)+) {
            fn encode(&self, w: &mut FingerprintWriter) {
                w.tag(concat!("tuple:", $(stringify!($index),)+));
                $(self.$index.encode(w);)+
            }
        }
    };
}
tuple!(A: 0, B: 1);
tuple!(A: 0, B: 1, C: 2);
tuple!(A: 0, B: 1, C: 2, D: 3);

mod schema;
