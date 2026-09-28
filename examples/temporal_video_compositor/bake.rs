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

impl BakeSpec {
    fn cache_path(&self) -> PathBuf {
        let stem = self
            .clip
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("clip");
        // Keep the cache name short but collision-resistant across specs.
        let short: String = stem.chars().take(24).collect();
        let name = format!(
            "libgibson_{}_{:.2}_{:.2}_{}x{}_{:.0}.rgb",
            short, self.start_s, self.dur_s, self.film_w, self.film_h, self.fps
        );
        std::env::temp_dir()
            .join("libgibson_video_cache")
            .join(name)
    }
}

/// Load a baked film from cache, running ffmpeg to produce it if absent. Returns
/// the film and the cache path used (so the caller can report it).
pub fn bake(spec: &BakeSpec) -> Result<(KeyedFilm, PathBuf), String> {
    let cache = spec.cache_path();
    if !cache.exists() {
        if !spec.clip.exists() {
            return Err(format!("clip not found: {}", spec.clip.display()));
        }
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
        .arg(out)
        .status()
        .map_err(|e| format!("failed to launch ffmpeg (is it installed?): {e}"))?;
    if !status.success() {
        return Err(format!("ffmpeg exited with {status}"));
    }
    Ok(())
}
