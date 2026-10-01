# Closeout listening corpus — what this round changed, and why (machine explanation)

**Observed** from note-level diffs of the BAND candidate (`perform_candidate`) rendered by one device
(`tests/audio_closeout_listening.rs`) at the pre-round head `9204ef4` (*before*) and at the final head
(*after*, library source `abc3f9f`). No musical quality is claimed; the maintainer listens.

Each block lists the receipt verdict before and after, the changed note events by role, and up to 8
removed (`-`) / added (`+`) events (`role  onset  duration  midi  role_note  function`).

## Why each change happened

- **Relational function** (`appoggiatura_*`): the line engine no longer claims an appoggiatura F#6
  that "resolves" onto Dm9's 9th; it chooses its next justified pitch for the slot (E6, then D6 —
  plain chord tones). Lead only.
- **Bass-figure room** (`bassfigure_*`): the bass no longer gives its only downbeat to a borrowed
  motif fragment; it states its own root there. Ripple (disclosed): with no bass call at that onset
  the interaction plan has nothing for the keys to answer, so the keys hold through instead of
  answering (bass + keys change).
- **Support identity** (`identity_*`):
  - `identity_keys_*_s96800001` — the keys answer a Thin bar's rootless F4-C5-E5 hold: E5 → D5 (the
    root) at Dm7, beat 8. Ripple at beat 4 (C7): the pad's rooting now takes its FIRST preference —
    the colour D5 gives way to the root C5 — instead of adding C5 beside D5 (the strict pointwise
    rule had vetoed the replacement; the hold-law acceptance admits it).
  - `identity_keys_*_s96860001` — the keys answer at Dm7, beat 48: G5 → D5.
  - `identity_pad_*_s96800019` — the pad adds G4 at Gm7 (beat 0), which its strict acceptance had
    vetoed over a sub-hold overlap.
  - `identity_pad_*_s96900010`, `identity_pad_*_66bpm_*_s96910000` — the pad roots the harmony
    (A at Am9; F3 at Fmaj7) and lets its tails go where they would complete another chord in the
    next harmony (durations 3.92 → 3.45 / 3.11).
  - `identity_release_*_s96910002` — the silent-bar case: the pad's Am7 tails from the harmony
    before are let go so they no longer complete A minor over the final F6.
  - `identity_swell_*_s96860007` — the pad answers its own staged swell at Gm9 (beat 8): the 5th D4
    gives way to the root G3 in a LOW voice that sounds from the bar's start (the remove-first
    preference), with G4 entering in the swell's upper layer at beat 9. Keys unchanged.

Unchanged by this round (byte-identical before/after): the drum restraint A/B (20/20 WAVs) and the
Ode-fidelity mixes (8/8). The Ode mixes that the PREVIOUS round changed (all 8 versus the hardening
corpus' `../../humanmusic-hardening/listening/WAV_SHA256SUMS`; Strict renders nothing in the lab) are
therefore preserved exactly as that round left them.

### `appoggiatura_black_ice_deflected33.25_s96800004` — relational function

- before (`9204ef4`): receipt: FAIL ["1 false temporal function claims"]  
- after (final): receipt: PASS  
- changed note events by role: {'lead': 4}

```text
- lead	25.0000	0.4500	90	melody	Some(Appoggiatura)
- lead	25.5000	0.5000	88	melody	Some(ChordTone)
+ lead	25.0000	0.4500	88	melody	Some(ChordTone)
+ lead	25.5000	0.5000	86	melody	Some(ChordTone)
```

### `appoggiatura_black_ice_deflected33.25_s96800009` — relational function

- before (`9204ef4`): receipt: FAIL ["1 false temporal function claims"]  
- after (final): receipt: PASS  
- changed note events by role: {'lead': 4}

```text
- lead	25.0000	0.4500	90	melody	Some(Appoggiatura)
- lead	25.5000	0.5000	88	melody	Some(ChordTone)
+ lead	25.0000	0.4500	88	melody	Some(ChordTone)
+ lead	25.5000	0.5000	86	melody	Some(ChordTone)
```

### `bassfigure_black_ice_deflected2.5_riffdrive_s96810002` — bass-figure room

- before (`9204ef4`): receipt: FAIL ["anchors: [\"Riff: neither the lead's identity nor the bass figure sounds\", \"BassFigure: the bass states no figure of its own\"]"]  
- after (final): receipt: PASS  
- changed note events by role: {'bass': 4, 'keys': 7}

```text
- bass	0.0000	0.2250	40	quote	Some(ChordTone)
- bass	0.2500	0.2250	48	quote	Some(ChordTone)
- bass	0.5000	0.4500	45	quote	Some(ChordTone)
- keys	0.0000	1.9400	72	hold	Some(ChordTone)
- keys	0.0000	1.9400	76	hold	Some(ChordTone)
- keys	1.5000	0.2250	72	answer	Some(ChordTone)
- keys	1.7500	0.2250	81	answer	Some(ChordTone)
- keys	2.0000	0.4500	76	answer	Some(ChordTone)
+ bass	0.0000	1.4250	45	root	Some(ChordTone)
+ keys	0.0000	2.5000	72	hold	Some(ChordTone)
+ keys	0.0000	2.5000	76	hold	Some(ChordTone)
```

### `bassfigure_swiss_deflected2.5_worldswitch_meaning_s96810007` — bass-figure room

- before (`9204ef4`): receipt: FAIL ["anchors: [\"BassFigure: the bass states no figure of its own\"]"]  
- after (final): receipt: PASS  
- changed note events by role: {'bass': 6, 'keys': 7}

```text
- bass	0.0000	0.3000	41	quote	Some(ChordTone)
- bass	0.3333	0.3000	45	quote	Some(ChordTone)
- bass	0.6667	0.3000	41	quote	Some(ChordTone)
- bass	2.0000	0.1583	48	quote	Some(ChordTone)
- keys	0.0000	1.9400	67	hold	Some(LicensedExtension)
- keys	0.0000	1.9400	72	hold	Some(ChordTone)
- keys	1.5000	0.3000	71	answer	Some(LicensedExtension)
- keys	2.0000	0.3000	72	answer	Some(ChordTone)
+ bass	0.0000	1.4250	41	root	Some(ChordTone)
+ bass	1.8333	0.1617	45	quote	Some(ChordTone)
+ keys	0.0000	2.5000	67	hold	Some(LicensedExtension)
+ keys	0.0000	2.5000	72	hold	Some(ChordTone)
  (9 removed / 4 added in total)
```

### `bassfigure_vapor95_deflected4_worldswitch_s96810002` — bass-figure room

- before (`9204ef4`): receipt: FAIL ["anchors: [\"BassFigure: the bass states no figure of its own\"]"]  
- after (final): receipt: PASS  
- changed note events by role: {'bass': 4, 'keys': 7}

```text
- bass	0.0000	0.2250	41	quote	Some(ChordTone)
- bass	0.2500	0.2250	46	quote	Some(ChordTone)
- bass	0.5000	0.4500	41	quote	Some(ChordTone)
- keys	3.0000	0.5000	72	answer	Some(ChordTone)
- keys	3.5000	0.5000	69	answer	Some(ChordTone)
+ bass	0.0000	1.9400	34	root	Some(ChordTone)
+ keys	1.0000	0.9700	74	hold	Some(ChordTone)
+ keys	1.0000	1.9400	65	hold	Some(ChordTone)
+ keys	1.0000	1.9400	72	hold	Some(LicensedExtension)
+ keys	3.0000	0.5000	67	answer	Some(LicensedExtension)
+ keys	3.5000	0.5000	65	answer	Some(ChordTone)
```

### `identity_keys_vapor95_deflected33.25_s96800001` — support identity

- before (`9204ef4`): receipt: FAIL ["1 held-identity flips"]  
- after (final): receipt: PASS  
- changed note events by role: {'keys': 2, 'pad': 1}

```text
- keys	8.0000	3.9000	76	hold	Some(LicensedExtension)
- pad	4.0000	3.9200	74	pad	Some(LicensedExtension)
+ keys	8.0000	3.9000	74	hold	Some(ChordTone)
```

### `identity_keys_vapor95_deflected64_s96860001` — support identity

- before (`9204ef4`): receipt: FAIL ["1 held-identity flips"]  
- after (final): receipt: PASS  
- changed note events by role: {'keys': 2}

```text
- keys	48.0000	3.9000	79	hold	Some(LicensedExtension)
+ keys	48.0000	3.9000	74	hold	Some(ChordTone)
```

### `identity_pad_black_ice_deflected64_meaning_s96900010` — support identity

- before (`9204ef4`): receipt: FAIL ["1 held-identity flips"]  
- after (final): receipt: PASS  
- changed note events by role: {'pad': 8}

```text
- pad	40.0000	3.9200	67	pad	Some(ChordTone)
- pad	40.0000	3.9200	72	pad	Some(ChordTone)
- pad	40.0000	3.9200	76	pad	Some(ChordTone)
- pad	40.0000	3.9200	83	pad	Some(ChordTone)
+ pad	40.0000	3.4541	67	pad	Some(ChordTone)
+ pad	40.0000	3.4541	72	pad	Some(ChordTone)
+ pad	40.0000	3.4541	81	pad	Some(ChordTone)
+ pad	40.0000	3.4541	83	pad	Some(ChordTone)
```

### `identity_pad_vapor95_66bpm_deflected64_meaning_s96910000` — support identity

- before (`9204ef4`): receipt: FAIL ["1 held-identity flips"]  
- after (final): receipt: PASS  
- changed note events by role: {'pad': 8}

```text
- pad	4.0000	3.9200	64	pad	Some(ChordTone)
- pad	4.0000	3.9200	67	pad	Some(LicensedExtension)
- pad	4.0000	3.9200	69	pad	Some(ChordTone)
- pad	4.0000	3.9200	72	pad	Some(ChordTone)
+ pad	4.0000	3.1121	53	pad	Some(ChordTone)
+ pad	4.0000	3.1121	64	pad	Some(ChordTone)
+ pad	4.0000	3.1121	69	pad	Some(ChordTone)
+ pad	4.0000	3.1121	72	pad	Some(ChordTone)
```

### `identity_pad_vapor95_demo7.25_s96800019` — support identity

- before (`9204ef4`): receipt: FAIL ["1 held-identity flips"]  
- after (final): receipt: PASS  
- changed note events by role: {'pad': 1}

```text
+ pad	0.0000	1.9600	67	pad	Some(ChordTone)
```

### `identity_release_vapor95_96bpm_deflected33.25_propulsive_s96910002` — support identity

- before (`9204ef4`): receipt: FAIL ["1 held-identity flips"]  
- after (final): receipt: PASS  
- changed note events by role: {'pad': 8}

```text
- pad	28.0000	3.9200	69	pad	Some(ChordTone)
- pad	28.0000	3.9200	72	pad	Some(ChordTone)
- pad	28.0000	3.9200	76	pad	Some(ChordTone)
- pad	28.0000	3.9200	79	pad	Some(LicensedExtension)
+ pad	28.0000	2.7086	69	pad	Some(ChordTone)
+ pad	28.0000	2.7086	72	pad	Some(ChordTone)
+ pad	28.0000	2.7086	76	pad	Some(ChordTone)
+ pad	28.0000	2.7086	79	pad	Some(LicensedExtension)
```

### `identity_swell_vapor95_demo64_meaning_s96860007` — support identity

- before (`9204ef4`): receipt: FAIL ["1 held-identity flips"]  
- after (final): receipt: PASS  
- changed note events by role: {'pad': 3}

```text
- pad	8.0000	3.9200	62	pad	Some(ChordTone)
+ pad	8.0000	3.9200	55	pad	Some(ChordTone)
+ pad	9.0000	2.9200	67	pad	Some(ChordTone)
```

