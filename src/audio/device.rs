//! The **optional** realtime audio-device backend (`audio-cpal`): stream an
//! [`AudioSource`] to the default output device through cpal, fed from a separate render
//! thread over a lock-free SPSC ring.
//!
//! This whole file compiles only under the `audio-cpal` feature — the `pub mod device;`
//! in [`crate::audio`] carries the `#[cfg]`, so the body below is plain code that leans on
//! cpal and rtrb freely. A default build never sees it, which is the point: `cargo test`,
//! headless CI and server images never link ALSA/CoreAudio/WASAPI.
//!
//! # The one rule that shapes everything: realtime safety
//!
//! The cpal data callback runs on a high-priority audio thread with a hard deadline. Miss
//! it and the user hears a click. So the callback here is *surgically* small: it pops
//! already-interleaved `f32` samples out of an [`rtrb`] ring into the device buffer and
//! zero-fills any shortfall. No allocation, no mutex, no syscall, no arbitrary user code —
//! nothing that can block or page-fault. Everything expensive (running the
//! [`AudioSource`], interleaving, backpressure sleeps) happens on the render thread, where
//! blocking is legal.
//!
//! # No second synthesizer
//!
//! The device drives the *same* [`AudioSource`] the offline renderer does (see
//! [`crate::audio::render`]). Correctness proven offline is correctness on the wire; this
//! module only moves those blocks across a thread boundary and out a sound card.
//!
//! # Sample-rate honesty
//!
//! We do **not** resample. The requested [`SampleRate`] is honored only if the device can
//! actually deliver f32 stereo at it; otherwise we negotiate a rate the device supports and
//! render the source at *that* rate, reporting it via [`AudioDevice::sample_rate`]. A caller
//! that needs an exact rate should read it back and react, not assume.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

use crate::audio::buffer::StereoBlock;
use crate::audio::render::{AudioSource, RenderCtx};
use crate::audio::time::{SampleRate, SampleTime};

/// Frames rendered per block on the producer thread. A block is the unit of work handed to
/// the [`AudioSource`]; 1024 frames is ~21 ms at 48 kHz — long enough to amortize
/// per-block overhead, short enough that the ring stays responsive to `stop`.
const BLOCK_FRAMES: usize = 1024;

/// How many blocks the ring can hold. Bounded on purpose: this is the whole latency/safety
/// budget. Twelve blocks (~256 ms at 48 kHz) gives the render thread slack to survive a
/// scheduling hiccup without starving the callback, and never grows.
const RING_BLOCKS: usize = 12;

/// Stereo, so two samples per frame. The ring stores interleaved `L,R,L,R,…` `f32`.
const CHANNELS: usize = 2;

/// Backpressure nap when the ring is full. The producer is ahead of the device; it sleeps
/// here rather than spin, then rechecks `stop`. Never runs on the audio thread.
const BACKPRESSURE_NAP: Duration = Duration::from_millis(2);

/// A live audio stream: an [`AudioSource`] being rendered on a background thread and played
/// out the default output device.
///
/// Construct with [`AudioDevice::play`]; stop with [`AudioDevice::stop`] (or just drop it —
/// [`Drop`] signals the render thread and joins it either way). Deliberately not `Send`:
/// cpal's [`cpal::Stream`] is not `Send` on every platform, so the handle stays on the
/// thread that opened it.
pub struct AudioDevice {
    /// The cpal output stream. Held purely to keep playback alive — dropping it stops the
    /// callback. Never *read* (its whole job is the Drop side effect), so the dead-code
    /// lint is silenced deliberately rather than by a misleading `_` rename.
    #[allow(dead_code)]
    stream: cpal::Stream,
    /// The render thread. `Option` so [`AudioDevice::stop`] and [`Drop`] can each `take`
    /// and join exactly once without double-joining.
    handle: Option<JoinHandle<()>>,
    /// Set to request the render thread wind down at the next block boundary.
    stop_flag: Arc<AtomicBool>,
    /// Total zero-filled samples the callback emitted for want of data — a starvation
    /// meter. Should stay at 0 on a healthy stream.
    underruns: Arc<AtomicU64>,
    /// The rate we actually negotiated and are rendering/streaming at — may differ from the
    /// requested rate (see the module docs on sample-rate honesty).
    render_rate: SampleRate,
}

impl AudioDevice {
    /// Start streaming `source` to the default output device.
    ///
    /// Negotiates a supported f32 stereo config, preferring `sr`, then a standard rate
    /// (48 kHz, then 44.1 kHz), then whatever f32 stereo the device offers. The source runs
    /// on a dedicated render thread at the negotiated rate; read it back with
    /// [`AudioDevice::sample_rate`]. The returned handle keeps the stream alive.
    ///
    /// # Errors
    ///
    /// - [`DeviceError::NoDevice`] — no default output device (a headless box, no sound
    ///   server).
    /// - [`DeviceError::Config`] — the device exposes no f32 stereo output configuration.
    /// - [`DeviceError::Build`] — cpal failed to build or start the stream.
    pub fn play(
        source: Box<dyn AudioSource + Send>,
        sr: SampleRate,
    ) -> Result<AudioDevice, DeviceError> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or(DeviceError::NoDevice)?;

        // Decide the rate before we touch a thread or a ring: everything downstream is sized
        // and clocked off it.
        let rate_hz = negotiate_rate(&device, sr.get())?;
        let render_rate = SampleRate::new(rate_hz)
            .ok_or_else(|| DeviceError::Config("device reported a zero sample rate".into()))?;

        // Bounded SPSC ring, sized in interleaved samples. This is the sole channel between
        // the render thread and the audio callback — and the sole shared allocation.
        let capacity = BLOCK_FRAMES * CHANNELS * RING_BLOCKS;
        // `producer` is moved whole into the render thread; `consumer` is mutated in place
        // by the callback, hence only it needs `mut` here.
        let (producer, mut consumer) = rtrb::RingBuffer::<f32>::new(capacity);

        let stop_flag = Arc::new(AtomicBool::new(false));
        let underruns = Arc::new(AtomicU64::new(0));

        // --- the realtime callback: the smallest thing that could possibly work ---
        // No alloc, no lock, no syscall. Pop what's there, zero the rest, tally the wound.
        let underruns_cb = Arc::clone(&underruns);
        let data_cb = move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let starved = drain_into(&mut consumer, out);
            if starved != 0 {
                underruns_cb.fetch_add(starved, Ordering::Relaxed);
            }
        };
        // A stream error is terminal and arrives off the hot path; there is nothing safe to
        // *do* here, so we swallow it rather than pretend. The underrun counter and a dead
        // ring are how callers observe trouble.
        let err_cb = |_err: cpal::Error| {};

        let config = cpal::StreamConfig {
            channels: CHANNELS as cpal::ChannelCount,
            sample_rate: rate_hz,
            buffer_size: cpal::BufferSize::Default,
        };
        let stream = device
            .build_output_stream::<f32, _, _>(config, data_cb, err_cb, None)
            .map_err(|e| DeviceError::Build(e.to_string()))?;

        // --- the render thread: allowed to compute, allocate and sleep to its heart's
        // content, because it is not the one with the deadline ---
        let handle = {
            let stop = Arc::clone(&stop_flag);
            thread::Builder::new()
                .name("gibson-audio-render".into())
                .spawn(move || render_loop(source, producer, render_rate, stop))
                .map_err(|e| DeviceError::Build(format!("render thread: {e}")))?
        };

        // Pre-buffer is already filling; open the tap.
        stream
            .play()
            .map_err(|e| DeviceError::Build(e.to_string()))?;

        Ok(AudioDevice {
            stream,
            handle: Some(handle),
            stop_flag,
            underruns,
            render_rate,
        })
    }

    /// The sample rate the source is actually being rendered and streamed at.
    ///
    /// Equals the rate requested in [`AudioDevice::play`] when the device could honor it;
    /// otherwise the negotiated fallback. Read this instead of assuming.
    #[inline]
    pub fn sample_rate(&self) -> SampleRate {
        self.render_rate
    }

    /// Total samples the callback had to zero-fill for want of data since start.
    ///
    /// A nonzero value is audible starvation — the render thread failed to keep the ring
    /// ahead of the device. Healthy playback holds this at 0.
    #[inline]
    pub fn underruns(&self) -> u64 {
        self.underruns.load(Ordering::Relaxed)
    }

    /// Signal the render thread to stop, join it, and drop the stream — a clean close.
    ///
    /// Consumes the handle. Playback ends when the stream drops at the end of this call.
    /// Dropping an [`AudioDevice`] without calling this does the same thing via [`Drop`];
    /// this exists to make the intent (and the join) explicit at the call site.
    pub fn stop(mut self) {
        self.shutdown();
        // `self` drops here: `stream` is torn down, silencing the device.
    }

    /// Signal-and-join, factored so [`AudioDevice::stop`] and [`Drop`] share one idempotent
    /// path. Calling it twice is harmless: the second `take` yields `None`.
    fn shutdown(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for AudioDevice {
    fn drop(&mut self) {
        // Never leave the render thread orphaned, even on an unwinding drop. The stream
        // field is dropped by the compiler right after this, stopping the callback.
        self.shutdown();
    }
}

/// Drain up to `out.len()` interleaved samples from `consumer` into `out`, zero-filling any
/// shortfall. Returns the number of samples that had to be zeroed (the underrun count).
///
/// This is the *entire* body of the realtime callback, lifted out so it can be unit-tested
/// against a hand-fed ring with no audio hardware. It allocates nothing and locks nothing —
/// that property is load-bearing and must not regress.
#[inline]
fn drain_into(consumer: &mut rtrb::Consumer<f32>, out: &mut [f32]) -> u64 {
    let mut starved = 0u64;
    for slot in out.iter_mut() {
        match consumer.pop() {
            Ok(sample) => *slot = sample,
            Err(_) => {
                *slot = 0.0;
                starved += 1;
            }
        }
    }
    starved
}

/// The producer thread body: render `source` block by block at `rate`, interleave, and push
/// into the ring with bounded backpressure. Exits on `stop`, when the source reports
/// finished, or when the consumer end is abandoned (the stream was dropped).
fn render_loop(
    mut source: Box<dyn AudioSource + Send>,
    mut producer: rtrb::Producer<f32>,
    rate: SampleRate,
    stop: Arc<AtomicBool>,
) {
    // Reused across blocks: cleared and refilled, never reallocated in steady state.
    let mut block = StereoBlock::new(BLOCK_FRAMES);
    let mut interleaved: Vec<f32> = Vec::with_capacity(BLOCK_FRAMES * CHANNELS);
    let mut pos: u64 = 0;

    loop {
        if stop.load(Ordering::Relaxed) || producer.is_abandoned() {
            break;
        }
        if source.is_finished(SampleTime(pos)) {
            break;
        }

        // Render one block. The renderer contract pre-zeroes; a source may write or
        // accumulate, so we clear first, exactly like the offline path.
        block.clear();
        let ctx = RenderCtx {
            sr: rate,
            start: SampleTime(pos),
        };
        source.render(&mut block, &ctx);
        pos = pos.saturating_add(BLOCK_FRAMES as u64);

        interleaved.clear();
        block.interleave_into(&mut interleaved);

        // Feed the ring. When it's full the device just hasn't caught up yet — nap and
        // retry rather than drop audio, rechecking the exits so `stop` stays responsive.
        let mut i = 0;
        while i < interleaved.len() {
            if stop.load(Ordering::Relaxed) || producer.is_abandoned() {
                return;
            }
            match producer.push(interleaved[i]) {
                Ok(()) => i += 1,
                Err(_) => thread::sleep(BACKPRESSURE_NAP),
            }
        }
    }
}

/// Pick an f32 stereo output rate the device can actually deliver, preferring `requested`.
///
/// Order: an f32 stereo range containing `requested` → a standard rate (48 kHz then
/// 44.1 kHz) within some f32 stereo range → the max rate of any f32 stereo range → the
/// device default config, but only if it is itself f32 stereo. Anything else is an honest
/// [`DeviceError::Config`]: we refuse to fake stereo or resample.
fn negotiate_rate(device: &cpal::Device, requested: u32) -> Result<u32, DeviceError> {
    if let Ok(configs) = device.supported_output_configs() {
        let stereo_f32: Vec<_> = configs
            .filter(|r| r.channels() == CHANNELS as cpal::ChannelCount)
            .filter(|r| r.sample_format() == SampleFormat::F32)
            .collect();

        if stereo_f32.iter().any(|r| r.contains_rate(requested)) {
            return Ok(requested);
        }
        for range in &stereo_f32 {
            if let Some(cfg) = range.try_with_standard_sample_rate() {
                return Ok(cfg.sample_rate());
            }
        }
        if let Some(range) = stereo_f32.first() {
            return Ok(range.max_sample_rate());
        }
    }

    // Enumeration unavailable or yielded no f32 stereo range: fall back to the device's own
    // default, and only accept it if it is f32 stereo — no silent format surprises.
    let default = device
        .default_output_config()
        .map_err(|e| DeviceError::Config(e.to_string()))?;
    if default.channels() == CHANNELS as cpal::ChannelCount
        && default.sample_format() == SampleFormat::F32
    {
        Ok(default.sample_rate())
    } else {
        Err(DeviceError::Config(format!(
            "no f32 stereo output configuration (device default is {} ch, {})",
            default.channels(),
            default.sample_format()
        )))
    }
}

/// Why opening the device failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceError {
    /// No default output device is available (headless host, no audio server).
    NoDevice,
    /// The device offers no usable (f32 stereo) configuration. Carries a human-readable
    /// detail from cpal or from the negotiation.
    Config(String),
    /// cpal failed to build or start the stream, or the render thread failed to spawn.
    Build(String),
}

impl std::fmt::Display for DeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceError::NoDevice => write!(f, "no default audio output device"),
            DeviceError::Config(d) => write!(f, "unsupported audio device configuration: {d}"),
            DeviceError::Build(d) => write!(f, "failed to build audio output stream: {d}"),
        }
    }
}

impl std::error::Error for DeviceError {}

#[cfg(test)]
mod tests {
    use super::*;

    // Everything here runs with no audio hardware. The device-touching test is `#[ignore]`d
    // so CI (which has no output device) stays green; run it by hand on a real box.

    /// A constant-DC source, mirroring the offline renderer's test source: lets us assert
    /// exact interleaved bytes coming out the ring.
    struct Const(f32);
    impl AudioSource for Const {
        fn render(&mut self, out: &mut StereoBlock, _ctx: &RenderCtx) {
            for i in 0..out.frames() {
                out.left[i] = self.0;
                out.right[i] = self.0;
            }
        }
    }

    #[test]
    fn drain_pops_available_then_zero_fills_and_counts_underrun() {
        let (mut p, mut c) = rtrb::RingBuffer::<f32>::new(8);
        for v in [1.0f32, 2.0, 3.0, 4.0] {
            p.push(v).unwrap();
        }
        let mut out = [-9.0f32; 6];
        let starved = drain_into(&mut c, &mut out);
        // First four are the fed samples; the ring then runs dry and the rest go to silence.
        assert_eq!(&out[..4], &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(&out[4..], &[0.0, 0.0]);
        assert_eq!(starved, 2);
    }

    #[test]
    fn drain_full_supply_reports_no_underrun() {
        let (mut p, mut c) = rtrb::RingBuffer::<f32>::new(8);
        for v in [0.25f32, -0.25, 0.5, -0.5] {
            p.push(v).unwrap();
        }
        let mut out = [0.0f32; 4];
        let starved = drain_into(&mut c, &mut out);
        assert_eq!(out, [0.25, -0.25, 0.5, -0.5]);
        assert_eq!(starved, 0);
    }

    #[test]
    fn interleave_then_drain_round_trips_lrlr() {
        // The feed path in miniature: render a block, interleave, push, drain back. Proves
        // the callback sees exactly L,R,L,R with no reordering or loss.
        let mut src = Const(0.3);
        let mut block = StereoBlock::new(3);
        let ctx = RenderCtx {
            sr: SampleRate::STUDIO,
            start: SampleTime(0),
        };
        src.render(&mut block, &ctx);
        let mut interleaved = Vec::new();
        block.interleave_into(&mut interleaved);

        let (mut p, mut c) = rtrb::RingBuffer::<f32>::new(interleaved.len());
        for &s in &interleaved {
            p.push(s).unwrap();
        }
        let mut out = vec![0.0f32; interleaved.len()];
        let starved = drain_into(&mut c, &mut out);
        assert_eq!(starved, 0);
        assert_eq!(out, vec![0.3; 6]); // 3 frames * 2 ch, all 0.3
    }

    #[test]
    fn ring_capacity_math_is_bounded_and_stereo_even() {
        let capacity = BLOCK_FRAMES * CHANNELS * RING_BLOCKS;
        assert_eq!(capacity % CHANNELS, 0, "ring must hold whole stereo frames");
        assert!(
            capacity >= BLOCK_FRAMES * CHANNELS,
            "must hold at least one block"
        );
    }

    #[test]
    fn device_error_display_is_human_readable() {
        assert_eq!(
            DeviceError::NoDevice.to_string(),
            "no default audio output device"
        );
        assert!(DeviceError::Config("x".into()).to_string().contains('x'));
        assert!(DeviceError::Build("y".into()).to_string().contains('y'));
    }

    /// Real end-to-end open of the default device. Requires audio hardware, so it is
    /// ignored by default; CI has none. Run with `--ignored` on a machine with output.
    #[test]
    #[ignore = "requires a real audio output device"]
    fn play_and_stop_on_real_device() {
        let dev = AudioDevice::play(Box::new(Const(0.0)), SampleRate::STUDIO)
            .expect("default device should open");
        // Negotiated rate must be a real, nonzero rate.
        assert!(dev.sample_rate().get() > 0);
        std::thread::sleep(Duration::from_millis(50));
        dev.stop();
    }
}
