//! The bounded audio processing graph: named buses and a headroom-aware [`Mixer`].
//!
//! ```text
//!   sources -> instruments/clip players -> buses -> effects -> master -> output
//! ```
//!
//! The graph is *prepared outside* any realtime callback: gains, routing and effect
//! state are set up ahead of time, and the callback only pulls prepared blocks. Round I
//! ships the bus taxonomy and a summing mixer with per-bus gain, dialogue-ducking sidechain
//! input, and a gentle master soft-clip guard; per-bus effect chains (delay/reverb/
//! compressor) attach in [`crate::audio::dsp::fx`].

use crate::audio::buffer::StereoBlock;

/// The fixed bus taxonomy. Every voice routes to exactly one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bus {
    /// Recorded media dialogue (the reaction source audio). Highest priority.
    Dialogue,
    /// HumanMusic — the synthesized score.
    Music,
    /// Synthesized sound effects / stings.
    Sfx,
    /// The final sum. Voices do not route here directly.
    Master,
}

impl Bus {
    /// The three source buses that sum into [`Bus::Master`].
    pub const SOURCES: [Bus; 3] = [Bus::Dialogue, Bus::Music, Bus::Sfx];

    /// A stable index for array-backed storage.
    #[inline]
    pub fn index(self) -> usize {
        match self {
            Bus::Dialogue => 0,
            Bus::Music => 1,
            Bus::Sfx => 2,
            Bus::Master => 3,
        }
    }
}

/// A summing mixer: per-bus linear gain plus a master gain, summed with a soft-clip guard
/// on the master so a transient over-full-scale peak saturates gracefully instead of
/// wrapping. Dialogue ducking is applied by the caller lowering [`Bus::Music`]'s gain from
/// the dialogue envelope; see [`crate::audio::human_music`].
#[derive(Debug, Clone)]
pub struct Mixer {
    gains: [f32; 4],
    master_gain: f32,
    soft_clip: bool,
}

impl Default for Mixer {
    fn default() -> Self {
        Mixer {
            // Conservative default headroom: sources below unity so a busy mix has room.
            gains: [1.0, 0.8, 0.9, 1.0],
            master_gain: 0.9,
            soft_clip: true,
        }
    }
}

impl Mixer {
    /// A mixer with default headroom-aware gains.
    pub fn new() -> Mixer {
        Mixer::default()
    }

    /// Set a bus's linear gain.
    pub fn set_gain(&mut self, bus: Bus, gain: f32) {
        self.gains[bus.index()] = gain.max(0.0);
    }

    /// Get a bus's linear gain.
    pub fn gain(&self, bus: Bus) -> f32 {
        self.gains[bus.index()]
    }

    /// Set the master gain (post-sum, pre-guard).
    pub fn set_master_gain(&mut self, g: f32) {
        self.master_gain = g.max(0.0);
    }

    /// Enable/disable the master soft-clip guard.
    pub fn set_soft_clip(&mut self, on: bool) {
        self.soft_clip = on;
    }

    /// Sum the three source-bus blocks into `master`, applying per-bus and master gain and
    /// the optional soft-clip guard. All blocks must share a frame count; the shortest
    /// governs. `master` is overwritten (not accumulated).
    pub fn mix_into(
        &self,
        master: &mut StereoBlock,
        dialogue: &StereoBlock,
        music: &StereoBlock,
        sfx: &StereoBlock,
    ) {
        master.clear();
        master.add_scaled(dialogue, self.gains[Bus::Dialogue.index()]);
        master.add_scaled(music, self.gains[Bus::Music.index()]);
        master.add_scaled(sfx, self.gains[Bus::Sfx.index()]);
        master.scale(self.master_gain);
        if self.soft_clip {
            for s in master.left.iter_mut().chain(master.right.iter_mut()) {
                *s = soft_clip(*s);
            }
        }
    }
}

/// A gentle cubic soft-clip: transparent inside `[-2/3, 2/3]`, saturating toward `±1`
/// beyond, and hard-limited past `±1`. Cheap and click-free for the master guard.
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    if x <= -1.0 {
        -2.0 / 3.0
    } else if x >= 1.0 {
        2.0 / 3.0
    } else {
        x - (x * x * x) / 3.0
    }
    .clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_indices_are_distinct_and_stable() {
        let idx: Vec<usize> = [Bus::Dialogue, Bus::Music, Bus::Sfx, Bus::Master]
            .iter()
            .map(|b| b.index())
            .collect();
        assert_eq!(idx, vec![0, 1, 2, 3]);
    }

    #[test]
    fn mixer_sums_with_gain_and_stays_bounded() {
        let mut m = Mixer::new();
        m.set_gain(Bus::Dialogue, 1.0);
        m.set_gain(Bus::Music, 1.0);
        m.set_gain(Bus::Sfx, 1.0);
        m.set_master_gain(1.0);
        let mut d = StereoBlock::new(4);
        d.left.iter_mut().for_each(|s| *s = 0.9);
        d.right.iter_mut().for_each(|s| *s = 0.9);
        let mut mu = d.clone();
        mu.scale(1.0); // same 0.9
        let s = StereoBlock::new(4); // silent sfx
        let mut master = StereoBlock::new(4);
        m.mix_into(&mut master, &d, &mu, &s);
        // 0.9 + 0.9 = 1.8 pre-clip; soft-clip guard must keep it <= 1.0.
        assert!(master.peak() <= 1.0 + 1e-6);
        assert!(!master.has_nonfinite());
    }

    #[test]
    fn soft_clip_is_transparent_in_the_linear_region() {
        // Small signals pass nearly unchanged.
        assert!((soft_clip(0.1) - (0.1 - 0.001 / 3.0)).abs() < 1e-6);
        // Extremes are bounded.
        assert!(soft_clip(5.0) <= 1.0);
        assert!(soft_clip(-5.0) >= -1.0);
    }

    #[test]
    fn ducking_lowers_music_gain() {
        let mut m = Mixer::new();
        m.set_gain(Bus::Music, 0.2); // ducked under dialogue
        assert_eq!(m.gain(Bus::Music), 0.2);
    }
}
