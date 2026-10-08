# Reference MIDI corpus — manifest & predeclared split

**Custody (§34):** the reference MIDIs are third-party copyrighted material and are **never
committed**. This file records only non-reconstructive metadata — SHA256, byte size, and SMF
header facts — plus the **predeclared calibration / holdout / negative split**. No note content,
no piano-roll, nothing from which a source could be reconstructed.

Local corpus directory (not in repo): resolved in priority order `$HUMANMUSIC_MIDI_DIR` →
`~/Downloads/GOOD_MIDI/`. First contact / manifest frozen: **2026-10-08**.

## Split rule (deterministic, blind — recorded before musical parsing, §3)

- **negative** = `AUD_DW0160.mid` ("Attention", Charlie Puth) — maintainer-declared bad-timing
  reference. Its microtiming must **never** be copied (§22).
- **holdout** = the **3 positives whose SHA256 sorts lexicographically smallest** — chosen
  blind to musical content, so the choice cannot be reverse-engineered from what we wanted it
  to be. Reserved for a **single** first-contact run after recurrence + feel are frozen (§24).
- **calibration** = the remaining positives — used to derive/tune the corrected instruments
  and the synthesis laws.

### First-contact honesty note

All 10 positives were **coarsely** contacted in the prior gap-analysis / deep-study round
(aggregate gap metrics + prose findings, 2026-10-08) — so **no truly-virgin holdout exists
among these 11 files**. This split therefore holds the three holdout files out from:
(a) the **corrected, lattice-aware instrument** (none of the fixed metrics have been run on
them), and (b) **all synthesis tuning** (recurrence / feel laws will be designed on calibration
+ synthetic controls only). It does **not** claim they are unseen at the coarse level. A
genuinely virgin §24 holdout would require new reference files the maintainer has not yet shared.

## Manifest

| class | file | SMF | tpq | tracks | bytes | SHA256 |
|---|---|---|---|---|---|---|
| calibration | ABBA — SOS | fmt1 | 384 | 14 | 49109 | `e63a34fcdb267b5f1fca146ea9cc12d4fbc262a71bb41af8239ec4a7c2f7fcef` |
| calibration | Hall & Oates — I Can't Go For That | fmt1 | 384 | 16 | 71210 | `71bc3c704b3270224865ef5f83f637b1c3d73f75176f893c642bee88e6d315e2` |
| calibration | Men at Work — Overkill | fmt1 | 96 | 8 | 29373 | `870fe4586b019022acf7acd47c3917ffb81282452baa5c515df2671b32ec8753` |
| calibration | MJ / Rod Temperton — Off The Wall | fmt1 | 384 | 12 | 96092 | `63923025138e1ea9e8d08182c2d9b053736b0b60cde569e6abc2fa3f59a374c3` |
| calibration | Rush — Limelight | fmt1 | 240 | 8 | 23661 | `b1b41ac2e2e097eeb5e302b1898a0341385b35f2423386dc5303937bffdbb878` |
| calibration | Rush — Subdivisions | fmt1 | 480 | 15 | 80099 | `6c1707e8803d6af471a560f386e9ea93ae5991875e4f7429571e13436d0f51e7` |
| calibration | Rush — Tom Sawyer | fmt1 | 96 | 17 | 31660 | `f1cb454d5aa44ba600879d562ed807d6196bce989040cfdb58bf87cbc511d27c` |
| **holdout** | Herbie Hancock — Chameleon | fmt1 | 384 | 16 | 161208 | `414bd87103c836a8f94a4e384135672c849fa51c51b4b895f86dc164da58cf6d` |
| **holdout** | Rush — Fly by Night | fmt0 | 96 | 1 | 99504 | `023198aee128b0491ef336e63f3a09b9205983cdd97ff73167b728fedcb59040` |
| **holdout** | The Rembrandts — I'll Be There For You | fmt1 | 120 | 21 | 47073 | `34e0fbd38c1b8f92f94d2ac6cc6fb1e1be337559901282a5f9c6088f714008c2` |
| negative | Charlie Puth — Attention (AUD_DW0160) | fmt0 | 480 | 1 | 23925 | `d24f0c776be8f679f4cfd0e1d240296d255c08f3a7ff97e13b8206ed97914290` |

**SMF header observations that constrain the tooling (§7):** division is **not** uniform —
tpq ranges over 96 / 120 / 240 / 384 / 480. Any parser that assumes 480 is wrong. Two files
are **format 0** (single track; voices separable only by channel). None of the 11 use SMPTE
division, but the parser must still refuse or correctly support SMPTE rather than silently
treating it as tpq=480.
