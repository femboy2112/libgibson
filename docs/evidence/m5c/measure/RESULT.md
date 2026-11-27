# M5-C MEASURE lease (ruling #82 Q5): result

- **Instrument:** `tools/figure_assets/lease_m5c_measure.sh` at `0af48e0`, run in a detached worktree.
- **Lease:** heavy class s, 2026-11-27 01:51:28Z to 01:54:47Z (199 s, from heavy.log), `LEASE_FAILS=0`.
- **Tools:** Blender 4.5.17 LTS with MPFB 2.0.18, run headless.

## Numbers
| step | wall | max RSS |
|---|---|---|
| licence audit | 1 s | 33 MB |
| MPFB export (146 files, 7.0 MB) | 22 s | 412 MB |
| render probe | 173 s | 1.87 GB |

- **Render probe:** engine EEVEE Next. Headless EEVEE worked, so there was no Cycles fallback.
  - 64 samples per pixel.
  - Still at 1920×1080: 13.5 s.
  - 24 frames at 1280×720: 5.83 s/frame mean, 6.00 s max.
- **Thermal:** Tctl peaked at 69.8 °C over 40 samples. The kill threshold is 85 °C.
- **Exporter self-checks (all PASS):**
  - own `.target` parser vs MPFB shape key: 6.1e-8 m;
  - own mhclo fit vs MPFB ClothesService (`female_casualsuit01`): 8.5e-8 m;
  - landmark-centroid joints vs Blender armature: 3.0e-8 m over 53 bones;
  - 0 body vertices without skin weights.
- **Licence check:** 46 exported files ok, 0 bad.

The rendered PNGs and the export stay local. Their sha256 values are in `local_sha256.txt`. The
probe body is unclothed above the waist, so no probe image is committed.

## Viewed (session, §V): defects seen in the still and frames 1, 12 and 24
1. The figure floats about 0.4 m above the ground plane. There is no contact and no figure shadow
   is visible; the floor shows only a key-light pool.
   - Cause to check: the probe reports a stage height of 2.80 m, so the bounds probably include
     helper geometry, and the body is placed and framed from those wrong bounds.
2. The figure is small in frame, because the camera distance is computed from the same 2.80 m bound.
3. The fitted outfit (`female_casualsuit01`) is exported but not rendered. Only the procedural
   skirt appears.
4. The procedural skirt has a notch at the left waist and a stiff flared hem. It does not follow
   the arm-raise pose. This is expected, because it is bound but not skinned in the probe.
5. Presentation:
   - the bob hair covers one eye;
   - the eyebrows and eyelashes are barely legible at this distance;
   - the lighting is flat (no visible rim) and the skin reads as plastic.

Fine as seen: the body is continuous and sculpted, with a face, fingers and smooth shoulder
deformation through the arm raise. This probe is a measurement, not a checkpoint.

## Lease sizing derived from these numbers (EEVEE, 64 spp)
- Each 720p frame costs about 6 s, and each 1080p still about 14 s. Setup costs about 25 s.
- **Example 1:**
  - 5 stills: about 70 s.
  - 1 turntable, 8 s at 30 fps = 240 frames: about 24 min.
  - Plan: drop to 32 spp, or render the turntable at 15 fps, so it fits one ≤20 min lease.
- **Example 2:** 20 s at 30 fps = 600 frames, about 60 min at 64 spp. Plan: 32 spp with the
  denoiser, split into 2 leases of 300 frames each, ≤20 min per lease.
- **Example 3:** 2 presets × 2 outfits × 2 styles. Matched short clips of 4 s each = 960 frames,
  about 3 leases.
- Re-measure the cost at 32 spp in the first real render lease before committing to the split.
