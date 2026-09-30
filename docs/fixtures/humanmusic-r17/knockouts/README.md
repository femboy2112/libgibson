# SWISS Round XVI note knockouts for Round XVII localization

These are counterfactual listening probes of the frozen `perform_phrased` arm,
score `03817af045ff24d6`, before the support solver changes. Each WAV is a full mix
with exactly one Pad, Keys, or Lead note muted. The baseline retains all notes.
The probes do not change production realization or certify the source of the
maintainer's reported horn blast.

Windows are `[26,34)`, `[34,42)`, `[42,50)`, and the accepted control `[58,66)`
beats. Rendering advances the original synth from sample zero through the end
of each window, retaining its voice, effect, and shared bus history. WAVs are
48 kHz stereo PCM16; the only synthesis is the repository's Rust synth. Each
mute sets one note's velocity to zero, preserving the note's scheduling,
envelope activity, voice allocation, oscillator, and filter processing. The
nonlinear compressor, limiter, and effect response can change after the mute;
the difference is consequently not a linear isolated-note stem.

`manifest.txt` identifies each zero-based frozen score note index, pitch,
onset, gate, model audible end, provenance, whether it entered before the
window, and whether it was sounding at the named site. The enumeration includes
every Pad/Keys/Lead note whose onset/release intersects the window, using
`sonority::audible_end`. **Boundary:** this envelope model omits shared reverb
tails, so notes audible only through older reverb are not enumerated. The render
itself retains those tails. RMS differences are descriptive signal measurements,
not listening verdicts or rankings of musical quality.

Conjectured audition order (rivals remain unresolved until human listening):

1. The isolated static G5 pad top voice: compare baseline with
   `swiss_r16_b028_mute_pad_0031_m79.wav`,
   `swiss_r16_b036_mute_pad_0039_m79.wav`, and
   `swiss_r16_b044_mute_pad_0047_m79.wav`. Removing G5 should remove the blast
   if this is its source. Control b060 has E5 rather than G5 on top.
2. The new C4 root / B2 bass contact: compare the corresponding C4 knockouts
   `...b028_mute_pad_0028_m60.wav`, `...b036_mute_pad_0036_m60.wav`, and
   `...b044_mute_pad_0044_m60.wav`. If contact is the issue, relief should follow
   C4 removal around the bass contact instead of G5 removal. This test removes
   the whole C4 note, so it cannot by itself identify the interaction mechanism.
3. An untouched lead or keys attack previously masked by the cluster: compare
   individual `mute_lead` and `mute_keys` WAVs using the manifest's onset times.
   If one of these removes the blast while the pad knockouts do not, the pad
   attribution is falsified. Multiple beneficial knockouts leave interaction,
   masking, or release-tail explanations unresolved.

The rank follows the score changes and the maintainer's description; it is
**Conjectured**, not an acoustic identification. No assistant listening claim is
made. Human acceptance is **Not Established**.

Reproduce from repository root:

```sh
cargo +1.98.1 run --release --example pocket_knockout_lab -- --out=target/humanmusic-r17/knockouts
python3 scripts/dev/humanmusic_r17_knockouts.py target/humanmusic-r17/knockouts
```

`SHA256SUMS` preserves the WAV hashes. The WAVs themselves are local listening
artifacts under `target/humanmusic-r17/knockouts/`. Open `audition.html` there for
individual audio controls, or `audition.m3u` in a playlist player. Both put each
window's baseline, top-voice mute, and C4 mute first, followed by every other
note knockout. `control_checks.txt` compares all four baseline WAV hashes with
the frozen R16 receipt.
