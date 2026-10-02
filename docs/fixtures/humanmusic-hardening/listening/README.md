# Hardening-round listening corpus

**Observed** at the recorded source. WAVs stay local under `target/humanmusic-hardening/listening/`;
their SHA256s are in `WAV_SHA256SUMS`. No listening claim is made by any file here.

- `drums/` — `drum_restraint_lab`: two songs (the generated BLACK_ICE lab song; a VAPOR95 RiffDrive song)
  under the historical drummer and the arbitrated surface at Foundation, Balanced, Expressive and Busy.
  Same plan and band per row: the lab refuses to render unless every non-drum note equals the historical
  take. Full mixes and drums-only stems; `*.percussion.txt` are the per-bar decisions.
- `ode-fidelity/` — `cover_fidelity_lab`: The homosapians — Ode to Joy at the v1 motif-only map and every
  fidelity preset, the same target world/seed/profile (`BAND`) per column (BLACK_ICE seed 901, VAPOR95 seed
  904, 95 BPM); `report.txt` lists each observational ceiling, the conformance at each relation, and the
  SATB harmony as DERIVED ANALYSIS (`satb-window-triad/v2`), never score metadata.
- `r17-verify-final.json` — the accepted R17 pocket at this source: 48/48 WAVs and 757/757 receipts
  byte-identical; the 16 earlier cover listening WAVs were also byte-identical (`bytegates`).

```sh
cargo +1.98.1 build --release --example drum_restraint_lab --example cover_fidelity_lab
./target/release/examples/drum_restraint_lab --out=target/humanmusic-hardening/listening/drums --render
./target/release/examples/cover_fidelity_lab --out=target/humanmusic-hardening/listening/ode-fidelity --render
```
