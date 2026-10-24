# Reaction-media provenance

The `libgibson_intro_reaction` finale composites a keyed reaction performer over the
LibGibson intro film. **No third-party media is shipped in this repository.**

- The default self-contained build (`--features embedded-media`) synthesizes an
  **original, procedural** chroma-key performer at runtime (see `embedded_films` /
  `synth_reaction_planar` in `../libgibson_intro_reaction.rs`). It is our own work, fully
  deterministic, and carries no third-party rights.
- The **real** greenscreen reaction is an *optional, local-only* input. You download the
  free-use source yourself and point the example at it with `--clip=PATH`; the bytes never
  live in this repository or in any published artifact.

## The free-use source (download it yourself)

The reaction footage this finale was authored against is the *Filthy Frank* green-screen
material, which the creator released publicly for free use in fan edits. It is mirrored on
the Internet Archive:

- **Internet Archive item:** *The Complete Works of Filthy Frank* —
  <https://archive.org/details/FilthyFrankCollection>
  - The relevant file is **`FilthyFrankChromaKeyColor.mp4`** (~452 MB per the Archive's
    listing): the full chroma-key source.
  - The item also carries `FilthyFrankGreenScreenAfterEffects.mp4` (~6.3 MB) and the
    original announcement clip `Filthy Frank GREENSCREEN PACKAGE (FREE DOWNLOAD).mp4`.
- **Original creator announcement:** "Filthy Frank GREENSCREEN PACKAGE (FREE DOWNLOAD)",
  TooDamnFilthy channel, published 2015-12-24 —
  <https://www.youtube.com/watch?v=3On7VEe6sRs>

## License status — NOT a redistribution grant

> **Redistribution rights are NOT established.** The creator offered the footage for free
> *editing and use* in fan content; that is **not** the same as a verified, durable license
> to **redistribute** the footage — or clips/frames/audio derived from it — inside another
> public Git repository or package. A byte-identical copy or a content hash proves byte
> identity, **not** distribution permission.
>
> This is exactly why LibGibson ships **no** footage: it only *references* the Archive item
> above and opens a **local** file you supply. **TODO (maintainer):** if you intend to
> redistribute any derived media, first establish and record the concrete terms here — do
> not infer a Creative Commons or public-domain grant where none is demonstrated.

## How to run the real reaction locally

1. Open the Internet Archive item: <https://archive.org/details/FilthyFrankCollection>.
2. Under its download options, find **`FilthyFrankChromaKeyColor.mp4`**.
3. Download it to your machine.
4. Point the example at it:

   ```
   cargo run --release --example libgibson_intro_reaction \
     --features embedded-media -- --clip=/path/to/FilthyFrankChromaKeyColor.mp4 --live
   ```

   - `--clip=PATH` bakes the reaction from your local source; `--live` forces a fresh bake
     instead of the synthetic embed.
   - **FFmpeg** must be installed for local video baking (and for extracting the clip's
     dialogue).
   - Optional live audio additionally requires the `audio-cpal` feature plus a supported
     audio backend/device; without it the film plays silent. The opt-in `--music` bed is
     original synthesis and rides the `--clip` dialogue path.

The example **never** auto-downloads or scrapes the source: you provide the path.

## Distribution boundary

- **The published crate is unaffected.** `Cargo.toml` excludes `/examples` from the packaged
  crate, so none of this ever ships in the library.
- **The GitHub source tree is clean too.** The repository contains no `.gmed`/`.m4a`
  reaction assets and no `include_bytes!` of third-party media — the public `embedded-media`
  build compiles from a fresh clone with the synthetic performer alone. Real footage is a
  local `--clip=PATH` on your own machine only.
