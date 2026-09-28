//! A minimal, dependency-free RIFF/WAVE writer for offline listening artifacts.
//!
//! Two encodings: 16-bit signed PCM (universally playable — the default for review WAVs)
//! and 32-bit IEEE float (lossless, for inspecting the exact rendered signal). Both are
//! interleaved stereo, little-endian. The i16 path hard-clamps to `[-1, 1]` before
//! quantizing so a stray over-full-scale peak can never wrap to the opposite rail (an
//! audible click); the master limiter should prevent that, but the writer is defensive.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::audio::buffer::StereoBlock;
use crate::audio::time::SampleRate;

const CH: u16 = 2;

/// Write `block` as 16-bit PCM stereo WAV at `sr`.
pub fn write_wav_i16(
    path: impl AsRef<Path>,
    block: &StereoBlock,
    sr: SampleRate,
) -> io::Result<()> {
    let frames = block.frames();
    let bits = 16u16;
    let data_bytes = (frames * CH as usize * (bits as usize / 8)) as u32;
    let mut w = BufWriter::new(File::create(path)?);
    write_header(&mut w, 1 /* PCM */, sr.get(), bits, data_bytes)?;
    for i in 0..frames {
        w.write_all(&to_i16(block.left[i]).to_le_bytes())?;
        w.write_all(&to_i16(block.right[i]).to_le_bytes())?;
    }
    w.flush()
}

/// Write `block` as 32-bit float stereo WAV at `sr` (lossless).
pub fn write_wav_f32(
    path: impl AsRef<Path>,
    block: &StereoBlock,
    sr: SampleRate,
) -> io::Result<()> {
    let frames = block.frames();
    let bits = 32u16;
    let data_bytes = (frames * CH as usize * (bits as usize / 8)) as u32;
    let mut w = BufWriter::new(File::create(path)?);
    write_header(&mut w, 3 /* IEEE float */, sr.get(), bits, data_bytes)?;
    for i in 0..frames {
        w.write_all(&block.left[i].to_le_bytes())?;
        w.write_all(&block.right[i].to_le_bytes())?;
    }
    w.flush()
}

#[inline]
fn to_i16(s: f32) -> i16 {
    // Clamp to [-1, 1], then map to the full i16 range. Rounding to nearest.
    let c = s.clamp(-1.0, 1.0);
    (c * i16::MAX as f32).round() as i16
}

/// The 44-byte canonical WAVE header (RIFF + fmt + data), followed by nothing — the
/// caller writes `data_bytes` of samples next.
fn write_header<W: Write>(
    w: &mut W,
    audio_format: u16,
    sample_rate: u32,
    bits: u16,
    data_bytes: u32,
) -> io::Result<()> {
    let byte_rate = sample_rate * CH as u32 * (bits as u32 / 8);
    let block_align = CH * (bits / 8);
    let riff_size = 36 + data_bytes;

    w.write_all(b"RIFF")?;
    w.write_all(&riff_size.to_le_bytes())?;
    w.write_all(b"WAVE")?;

    w.write_all(b"fmt ")?;
    w.write_all(&16u32.to_le_bytes())?; // fmt chunk size
    w.write_all(&audio_format.to_le_bytes())?;
    w.write_all(&CH.to_le_bytes())?;
    w.write_all(&sample_rate.to_le_bytes())?;
    w.write_all(&byte_rate.to_le_bytes())?;
    w.write_all(&block_align.to_le_bytes())?;
    w.write_all(&bits.to_le_bytes())?;

    w.write_all(b"data")?;
    w.write_all(&data_bytes.to_le_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn tmp(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "libgibson_wavtest_{}_{}.wav",
            std::process::id(),
            name
        ));
        p
    }

    #[test]
    fn i16_wav_has_canonical_header_and_size() {
        let mut b = StereoBlock::new(3);
        b.left = vec![0.0, 1.0, -1.0];
        b.right = vec![0.0, 0.5, -0.5];
        let path = tmp("i16");
        write_wav_i16(&path, &b, SampleRate::STUDIO).unwrap();

        let mut bytes = Vec::new();
        File::open(&path).unwrap().read_to_end(&mut bytes).unwrap();
        // 44-byte header + 3 frames * 2 ch * 2 bytes = 44 + 12 = 56.
        assert_eq!(bytes.len(), 56);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        assert_eq!(&bytes[36..40], b"data");
        // fmt: PCM (1), 2ch, 48000 Hz.
        assert_eq!(u16::from_le_bytes([bytes[20], bytes[21]]), 1);
        assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 2);
        assert_eq!(
            u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
            48_000
        );
        // First data sample (frame 0, left) is 0.
        assert_eq!(i16::from_le_bytes([bytes[44], bytes[45]]), 0);
        // Frame 1 left == 1.0 -> i16::MAX.
        assert_eq!(i16::from_le_bytes([bytes[48], bytes[49]]), i16::MAX);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn i16_clamps_beyond_full_scale_without_wrapping() {
        // A +1.5 sample must clamp to +full-scale, never wrap to a negative rail.
        assert_eq!(to_i16(1.5), i16::MAX);
        assert_eq!(to_i16(-1.5), -i16::MAX); // symmetric clamp
        assert_eq!(to_i16(0.0), 0);
    }

    #[test]
    fn f32_wav_is_format_3_and_lossless() {
        let mut b = StereoBlock::new(2);
        b.left = vec![0.123_456, -0.9];
        b.right = vec![0.5, 0.25];
        let path = tmp("f32");
        write_wav_f32(&path, &b, SampleRate::STUDIO).unwrap();
        let mut bytes = Vec::new();
        File::open(&path).unwrap().read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes.len(), 44 + 2 * 2 * 4);
        assert_eq!(u16::from_le_bytes([bytes[20], bytes[21]]), 3); // IEEE float
        let s0 = f32::from_le_bytes([bytes[44], bytes[45], bytes[46], bytes[47]]);
        assert!((s0 - 0.123_456).abs() < 1e-9);
        std::fs::remove_file(&path).ok();
    }
}
