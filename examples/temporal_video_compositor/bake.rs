//! Offline video ingest: shell out to `ffmpeg` to decode + downscale a short
//! segment of a clip into a cached raw-RGB24 film. This is the "asset bake" half of
//! the offline-prep / runtime-playback split: the runtime never touches the codec.
//!
//! Example-only (uses `std::process`); the compositing/keying logic it feeds is
//! tested against synthetic film with no ffmpeg dependency, so CI stays hermetic.

#![allow(dead_code)]

use super::film::KeyedFilm;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

/// What to extract and at what resolution.
#[derive(Debug, Clone)]
pub struct BakeSpec {
    pub clip: PathBuf,
    pub start_s: f32,
    pub dur_s: f32,
    pub film_w: u32,
    pub film_h: u32,
    pub fps: f32,
}

/// Deterministic hex fingerprint of everything that must make two bakes distinct:
/// the source's identity (canonical path + byte length + mtime) and the full,
/// untruncated spec (`f32::to_bits` of the float params — exact, so 2.001 and 2.004
/// never collide the way `{:.2}` formatting did). Pure: no filesystem access, so it
/// is testable without a clip. FNV-1a over a canonical byte string — a small, fixed
/// hash chosen over `DefaultHasher` precisely because its output is stable across
/// toolchain versions, which is what a persisted cache key actually needs.
#[allow(clippy::too_many_arguments)] // the key IS these eight fields; folding them
                                     // into a struct would just move the argument list, not shorten the contract.
pub(crate) fn cache_fingerprint(
    canonical_path: &str,
    file_len: u64,
    mtime_bits: u64,
    start_s: f32,
    dur_s: f32,
    film_w: u32,
    film_h: u32,
    fps: f32,
) -> String {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = FNV_OFFSET;
    let mut eat = |bytes: &[u8]| {
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(FNV_PRIME);
        }
    };
    eat(canonical_path.as_bytes());
    eat(&[0xff]); // domain separator: keep the path from bleeding into the numbers
    eat(&file_len.to_le_bytes());
    eat(&mtime_bits.to_le_bytes());
    eat(&start_s.to_bits().to_le_bytes());
    eat(&dur_s.to_bits().to_le_bytes());
    eat(&film_w.to_le_bytes());
    eat(&film_h.to_le_bytes());
    eat(&fps.to_bits().to_le_bytes());
    format!("{h:016x}")
}

impl BakeSpec {
    /// The cache path for this spec, keyed on the source's fingerprint. Errors if the
    /// clip is absent — the file's identity (length + mtime) is part of the key, so
    /// there is nothing to canonicalize and no honest cache name without it.
    fn cache_path(&self) -> Result<PathBuf, String> {
        let canonical = std::fs::canonicalize(&self.clip)
            .map_err(|e| format!("clip not found: {} ({e})", self.clip.display()))?;
        let meta = std::fs::metadata(&canonical).map_err(|e| format!("stat clip: {e}"))?;
        let file_len = meta.len();
        // Nanoseconds since the epoch as raw bits; a clip re-encoded in place bumps
        // this and invalidates a stale bake even at identical length.
        let mtime_bits = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let hash = cache_fingerprint(
            &canonical.to_string_lossy(),
            file_len,
            mtime_bits,
            self.start_s,
            self.dur_s,
            self.film_w,
            self.film_h,
            self.fps,
        );
        // Human-readable stem is decoration for the operator; correctness lives in
        // the hash suffix, not the name.
        let stem = self
            .clip
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("clip");
        let short: String = stem.chars().take(24).collect();
        let name = format!("libgibson_{short}_{hash}.rgb");
        Ok(std::env::temp_dir()
            .join("libgibson_video_cache")
            .join(name))
    }
}

/// Load a baked film from cache, running ffmpeg to produce it if absent. Returns
/// the film and the cache path used (so the caller can report it).
pub fn bake(spec: &BakeSpec) -> Result<(KeyedFilm, PathBuf), String> {
    let cache = spec.cache_path()?;
    if !cache.exists() {
        run_ffmpeg(spec, &cache)?;
    }
    let bytes = std::fs::read(&cache).map_err(|e| format!("read cache: {e}"))?;
    let film = KeyedFilm::from_raw(bytes, spec.film_w, spec.film_h, spec.fps).ok_or_else(|| {
        format!(
            "cache {} is not a whole number of {}x{} RGB24 frames (delete it and re-bake)",
            cache.display(),
            spec.film_w,
            spec.film_h
        )
    })?;
    Ok((film, cache))
}

fn run_ffmpeg(spec: &BakeSpec, out: &Path) -> Result<(), String> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir cache dir: {e}"))?;
    }
    // Bake into a per-process temp sibling, then rename only after the output is
    // proven whole. An interrupted or truncated ffmpeg can litter this .tmp file, but
    // it can never leave a partial at the final path that a later run would trust.
    let mut tmp = out.as_os_str().to_owned();
    tmp.push(format!(".tmp.{}", std::process::id()));
    let tmp = PathBuf::from(tmp);
    let vf = format!(
        "scale={}:{},fps={}",
        spec.film_w, spec.film_h, spec.fps as u32
    );
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-ss",
            &format!("{}", spec.start_s),
            "-t",
            &format!("{}", spec.dur_s),
            "-i",
        ])
        .arg(&spec.clip)
        .args(["-vf", &vf, "-pix_fmt", "rgb24", "-f", "rawvideo"])
        .arg(&tmp)
        .status()
        .map_err(|e| format!("failed to launch ffmpeg (is it installed?): {e}"));
    // From here on every failure path must sweep the temp; a helper keeps the
    // cleanup honest instead of scattering `remove_file` calls.
    let scrub = |msg: String| -> String {
        let _ = std::fs::remove_file(&tmp);
        msg
    };
    let status = status.map_err(&scrub)?;
    if !status.success() {
        return Err(scrub(format!("ffmpeg exited with {status}")));
    }
    // Validate before we bless it: ffmpeg can exit 0 yet hand back an empty or
    // truncated stream. Read it back through the same gate the runtime uses; only a
    // film with at least one whole frame earns the final path.
    let bytes = std::fs::read(&tmp).map_err(|e| scrub(format!("read baked temp: {e}")))?;
    let whole = KeyedFilm::from_raw(bytes, spec.film_w, spec.film_h, spec.fps)
        .is_some_and(|f| f.nframes >= 1);
    if !whole {
        return Err(scrub(format!(
            "ffmpeg produced empty or truncated output (not a whole {}x{} RGB24 frame)",
            spec.film_w, spec.film_h
        )));
    }
    std::fs::rename(&tmp, out).map_err(|e| scrub(format!("commit baked cache: {e}")))?;
    Ok(())
}
