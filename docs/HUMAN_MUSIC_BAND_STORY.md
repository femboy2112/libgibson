# HumanMusic — The Band Tells the Story

> **Status: proof frozen 2026-10-09, abstraction design-approved.** This is the
> ear-gate record for the Rick-C137 *semantic-story* round and the build mandate
> that follows from it. It is design-facing: it describes the engine's own material
> (the `rick_probe` germ) and architecture, never any third-party or personal
> note-data.

## The thesis under test

The round's one claim: **a song reads as a story when the band *enacts* it — when a
thematic identity is handed from musician to musician over time — not when a lead
sings a bigger tune over fixed backing.** The requirement was explicitly *not* more
notes, more extensions, a busier lead, or higher "complexity." It was *who carries
which meaning, when*.

The governing law (plan §4/§20): **performance narrates μ(song)** — what the song
actually means — **not F(trace)**, the listener target. Performance may not secretly
repair bad composition. The band-story layer therefore assigns carriers from the
song's *meaning*, built before realization; it never rewrites meaning at perform
time (this subsumes the GEN-3 landmine where development rewrote `statement.motif`
during `perform`).

## How it was tested — the ear-first probe (`examples/band_story_probe.rs`)

The ear-accepted `rick_probe` song (same form, harmony, germ, pocket, tempo) with
**exactly one variable changed: who carries which meaning when.** Four casts, density-
and loudness-matched (activity 24.9–26.3 ev/s — narrative is *sparser* than static,
not busier; loudness −15.2 dB ±0.1; no clip), so any audible difference is the
*arrangement telling a story*, never a mix artifact:

| Cast | What it does |
|---|---|
| **STATIC** | lead carries the germ every phrase; the band's jobs never move (control). |
| **NARRATIVE** | the germ migrates through the band (table below). |
| **SCRAMBLED** | the same carriages in reversed order (`NARRATIVE[11-ix]`) — locally valid, story wrong. |
| **FLAT** | narrative's density and conviction, but the carrier *never leaves the lead* (density control). |

The NARRATIVE carriage, phrase by phrase (germ = `rick_probe`'s verse-question /
hook-answer):

| Phrase | Meaning | Carrier |
|---|---|---|
| Verse 1 (×2) | **Learn** | lead teaches the germ; band legible, restrained |
| Chorus 1 (×2) | **Answer** | lead, conviction up |
| Verse 2 (×2) | **Reinforce** | germ enters the **keys** an octave down — no longer the lead's alone |
| Chorus 2 (×2) | **Develop** | the **bass** quotes the hook's contour — same DNA, new function |
| Bridge | **Miss** | the lead **withholds** at the expected arrival; the **keys** carry the dark |
| (bridge) | **Prepare** | lead + keys build back |
| Final chorus (×2) | **Payoff** | the hook returns in an earned **ensemble octave-tutti** (lead + keys + bass) |

Stem isolations (`--no-lead` / `--lead-only` / `--rhythm` / `--support`) test §32: with
the lead muted, NARRATIVE must *still* carry the germ (keys/bass at the handoffs);
STATIC must go to pure backing with no tune.

## PROVED BY EAR — Leah's verdict, 2026-10-09 (ear = oracle)

Every axis of the gate passed. Her words, mapped to the law:

- **§15 — the handoff is load-bearing (the go/no-go).** NARRATIVE > FLAT, and the
  *reason* is the carrier migration: *"2 makes the song breathe, 3 has the song at
  one energy level, the handoff is loadbearing."* FLAT had narrative's density and
  conviction and still sat at one level. The abstraction is worth building. **PASS.**
- **§32 — the story survives lead-mute (flagship).** NARRATIVE `--no-lead`: *"6 is
  fire, i can feel the energy/dynamics of the song."* STATIC `--no-lead`: pure
  backing. The band, not the lead, carries the narrative. **PASS.**
- **Order matters.** SCRAMBLED (same carriages, reversed) read as *"semantically
  unjustified dynamics in places"* — setup→payoff *ordering* is a real perceptual
  object, not noise. **PASS.**
- **Band-as-agent is audible.** NARRATIVE vs STATIC: *"holy shit it sounds like the
  band is working together"* — the difference between an ensemble enacting a song
  and a lead over backing. **PASS.**
- **The withhold reads as intentional.** NARRATIVE `--lead-only` (the bridge gap):
  *"i can hear it :D"* — the lead *choosing* silence at the Miss lands as a Miss,
  not an absence. **PASS.**

Context: STATIC (the control) was already *"entering Casiopea territory"*; FLAT
*"could pass as c137 rick level"*; NARRATIVE was *"subtly better"* than both and the
only cast that made the song *breathe*.

## PLAUSIBLE — modeled, not independently ear-isolated

- **Germ identity surviving a carrier change as a first-class link.** The keys/bass
  "quotes" read as the *same* tune (she heard a band working together, not a
  different tune entering) — but a broken-identity carrier was not A/B'd, so the
  *identity-preservation* claim is inferred from the positive read, not isolated.
- **The specific MeaningKind tags** (Learn/Reinforce/Develop/Miss/Payoff) map to what
  she heard. She heard *breath, energy, handoff, intentional withhold* — consistent
  with the labels, but the labels themselves were not the unit of judgment.

## UNTESTED / KNOWN RESIDUAL

- **Bridge semantic awkwardness.** Present *even in STATIC* (*"the bridge feels a
  little awkward semantically"*), improved but not eliminated by NARRATIVE (*"still
  slightly awkward but much better than in static"*). This predates the handoff layer
  → it is **composition-level**, and per §4/§20 the performance layer narrates
  μ(song) and may not repair it. **Flagged for a later composition pass; it does not
  block the narrative abstraction** (the narrative treatment was a net *improvement*
  on the bridge, which is the layer doing its job honestly).
- **Why SCRAMBLED is worse**, beyond "unjustified dynamics" — not dissected.

---

## Build mandate (design-approved by the gate above)

The abstraction survives adversarial review (not a verdict-D duplicate of the
existing meaning layer). It must hold **four facts absent from every existing type**,
**subsume three partial overlaps** rather than double-assign, and **extend the
receipt** so the commuting law can witness a band carry.

### Four new facts the layer must own
1. **Per-phrase carrier assignment → Agent, with a lead co-role** (`Stating` /
   `Withheld` / `Counter`). No existing type binds a meaning site to *who in the band
   realizes it*.
2. **A thematic identity that survives an agent change**, as a first-class link
   (the germ in the keys *is* the germ the lead taught).
3. **A lead-demotion action** — no current mode makes the lead a countervoice while
   another agent carries the site (the Miss).
4. **The μ→performance wire.** `song.meaning` is set by `compose_meaning` and **no
   performance module reads it** today; the planner never sees
   Learn/Recognize/Payoff. The layer closes that loop.

### Three partial overlaps to subsume / reuse (never duplicate)
- `ActionFamilies.lift_reach` (`action.rs`) already picks a fragment-carrier from
  `[Lead, Bass, Keys]` per piece (gated on `distributed_agency`, else hard Lead) — a
  second assigner would double-assign. The narrative plan must *drive* this, not race it.
- `band_fragments` / `Response.realizes` already does Lead→band fragment handoff —
  reuse as the realization path.
- Responder selection (`interaction.rs`, cost+rng) — feed it the carrier assignment
  instead of leaving it to cost+rng.

### Receipt (§27)
`projection.rs::carries` refuses to witness a band carry as identity (Keys/Pad
`_ => false`; Bass only via `BASS_FIGURE_TAGS`). A **`NarrativeReceipt`** must extend
this so the commuting law ν(P) = μ(S) can *see* the handoff — otherwise the proof
that the band told the story is invisible to the engine's own checker.

### Gold already on the bench
`MotifBank.bass_cell` (`thesis.fragment(2).transpose(-7)`) and `countermotif`
(`thesis.invert()`) are **built** by composer/song and have **zero realizer
readers** — carrier material already scaffolded, waiting to be enacted.

### Shape (GEN-3 pattern)
Opt-in `NarrativePolicy { Archived, Ensemble }` on `PerformanceProfile`, **byte-exact
off** (every axis off == v0.4 output, fingerprint-gated), with a `with_narrative`
builder analogous to `with_lead_life`. The ear arms it; it is never an unheard
default. No onset jitter — structured/relational carrier assignment only.

### Standing rules (unchanged)
Ear = oracle (machine metrics may only *reject*, never certify "sounds right"). No
random onset jitter. Reference MIDIs never committed. Leah holds the launch button;
all work held uncommitted until her explicit go.
