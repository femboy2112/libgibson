# Kitty graphics images: design (experimental, not built)

Status: **design only**. Nothing in this document is implemented. It records the route,
contract and gates for drawing real pixel images inside LibGibson's cell layout. Requested by an
external application (ProjectGHOST/MINERVA) for a painted avatar. This is also the "managed
high-resolution image element on a qualified Kitty terminal" step of
[MATH_TYPESETTING_ROADMAP.md](MATH_TYPESETTING_ROADMAP.md).

Today LibGibson has **no image transport**. `capability::Capabilities` has `kitty_graphics`,
`sixel` and `iterm_images` fields, all `Unknown`, and nothing emits graphics escapes. Images are
drawn as half-block cells (`ui::image`, `ui::image_area`).

## Route: Unicode placeholders

The kitty graphics protocol can show an image through *placeholder cells*:
1. The image is transmitted once and given an id.
2. A **virtual placement** of `c × r` cells is created.
3. Ordinary text cells containing the placeholder codepoint `U+10EEEE` then display that
   image. The cell's foreground colour encodes the image id, and combining diacritics encode the
   row and column within the image.

That is the property that makes it fit LibGibson. The image lives *in the cell grid*, so the
existing pieces keep working unchanged:
- **The diff renderer** redraws, clips and scrolls placeholder cells like any other text. A
  frame whose pixels have not changed emits nothing.
- **Layout** reserves a fixed `columns × rows` box, exactly as `ui::image` does today.
- **Resize, partial redraw and the alternate screen** need no image-specific code path: the
  terminal draws whatever the placeholder cells say.

Sixel is not this route. It paints pixels at the cursor, outside the cell grid, so it must be
re-emitted after any damage. That collides with the open output-target work (#338 → #339 →
#343), and Sixel is out of scope until that lands.

## Contract (proposed)

- **Opt-in only.** Images are used only when the application sets a capability flag itself (for
  example `Capabilities.kitty_graphics = Supported` from its own configuration). LibGibson never
  enables it from `TERM`, `KITTY_WINDOW_ID` or similar environment hints.
  - No probing query is sent by default. A timed `a=q` query races the application's input loop
    for stdin bytes; if offered at all, it is a separate explicit call.
- **Same element, same box.** An image element reserves `columns × rows` cells whatever is
  painted. Without the flag it is drawn in half-blocks. In `Mono` it is the alt text. With the
  flag, placeholder cells are used. Layout is identical in all three.
- **Ids and reuse.** Each distinct pixel buffer gets an image id (a content hash mapped to a u32
  id ≥ 1). Pixels that have not changed reuse the id and transmit nothing. A changed image gets a
  new id; the old one is deleted once no placeholder cell refers to it.
- **Byte budgets.**
  - *Per-upload cap:* an image whose encoded payload exceeds the cap is not sent; it falls back
    to half-blocks and is reported.
  - *Per-frame budget:* uploads beyond the budget are deferred to later frames. Their boxes show
    half-blocks until each upload completes, so a frame never stalls on a large image.
  - Payloads are sent directly (`t=d`), base64, in chunks of at most 4096 bytes (`m=1` … `m=0`),
    with all control keys only on the first chunk. RGBA uses `f=32`; zlib (`o=z`) is optional.
- **Teardown and panic.** LibGibson owns the cleanup:
  - on normal teardown it deletes every image it transmitted, and frees their data;
  - a public hook (`delete_all_images()` or similar) lets an application run the same deletion
    from its own panic handler or signal path, so a crash never leaves an image pinned on screen.
- **Determinism.** Image ids and emitted bytes are a pure function of the pixels and the layout.
  There are no clocks and no randomness.

## To verify before any code ships

These were gathered from the protocol page
(<https://sw.kovidgoyal.net/kitty/graphics-protocol/>). Items marked † were not checked against
the page's verbatim text and must be verified against it, and against a real kitty, before
implementation:
- the transmission and chunking rules above (checked against the page text);
- † the virtual-placement keys (`U=1` with `a=p`/`a=T`, `i=`, `c=`, `r=`);
- † how the image id is encoded in the foreground colour (24-bit truecolour or 8-bit 256-colour,
  with an optional third diacritic for the most significant byte), and the placement id in the
  underline colour;
- † the row/column diacritics table (`rowcolumn-diacritics.txt`) and the rules for inferring
  omitted diacritics (unreliable with horizontal scrolling and overlapping images, so LibGibson
  should always write them);
- † the exact delete-all and delete-by-id syntax, and lowercase vs uppercase (freeing data);
- † behaviour on reset, `ESC [2J` and the alternate-screen switch;
- † which terminals besides kitty support placeholders. Many implement the graphics protocol; the
  page does not say they support placeholders.

## Qualification gates

1. **Byte-exact golden tests** for the transmit and placeholder sequences of a synthetic RGBA test
   card. No real or private art is used.
2. **A real kitty PTY test:** transmit, place, redraw unchanged (zero image bytes), change pixels
   (new id, old one deleted), resize, and tear down (nothing left on screen). The panic hook runs
   from a panicking owner.
3. **The fallback matrix:** flag off gives half-block cells identical to `ui::image`, Mono gives
   alt text, and layout is unchanged across all three.
4. **Budgets:** an oversize image falls back; a frame over budget defers uploads; idle frames emit
   zero image bytes.
5. **No regression** in the existing renderer goldens, and nothing emitted when the flag is unset.
