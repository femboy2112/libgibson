# Show Plan Template

A **thinking fixture**, not runtime configuration. There is no parser, no schema,
no YAML dependency — nothing reads this file. It is a page you (or a coding agent)
fill in *before* writing rendering code, to force the decisions that make a show
coherent out into the open where they can be argued with.

Copy the template below into a scratch file, fill every field, and only then start
building. If a field is hard to fill, that is the plan telling you the show isn't
decided yet — which is exactly what you want to learn before you've written a
thousand lines of camera math.

See `docs/CINEMATIC_AUTHORING.md` for the method this template serves (the five
steps: semantic anchors → representations → directed sequence → transitions →
acceptance) and for the library/author boundary it assumes.

---

## The template

```
TITLE:
  <the show's name, one line>

LOGLINE:
  <one sentence: what the audience understands by the end that they didn't at the start>

SEMANTIC ANCHORS
  The identities that must SURVIVE every change of representation. If an anchor is
  lost when the visuals change, the audience loses the thread. Name 1–4.
  - <anchor>: <why it must persist>
  - ...

ACTS
  Each act teaches ONE thing. The audience should be able to say what they learned
  in it. Order them so each act earns the next.
  - id:
    purpose:              <what the audience learns here>
    representation:       <the visual basis: list / shelf / ring / plot / terrain / ...>
    dominant visual object: <the ONE thing on screen that owns attention>
    hold:                 <roughly how long it sits, and why that long>

TRANSITIONS
  The seams between acts are where shows fail. For each boundary, name what stays
  the same (the invariant the anchor rides across) and what changes.
  - from -> to:
    invariant:            <the anchor that is preserved across the basis change>
    transformation:       <what visually changes, and how continuity is kept>

TIMELINE
  The shape of the edit clock. Keep it to these five beats; most shows are one of
  these per act, bracketed by establish/payoff.
  - establish:            <the opening wide read — what the whole thing IS>
  - transform:            <the first real change of basis>
  - reveal:               <the moment the structure becomes legible>
  - hold:                 <the deliberate rest where the audience catches up>
  - payoff:               <the closing read that pays off the logline>

OBSERVABLES
  Any real data/instrument the show displays (a signal, a spectrum, a metric). The
  APPLICATION owns the analysis; the library only realizes it faithfully. Name the
  source and what it proves.
  - <observable>: <source> -> <what it lets the audience SEE>

FUTURE MUSIC INTENT  (conceptual only — no score is written here)
  How a later soundtrack pass would read this edit. This is a projection
  M : timeline -> score the music module will own; the timeline itself stays
  content-agnostic (it stores cues, not tones). Describe intent per beat so the
  future pass has direction, not data to parse.
  - beat:                 <which timeline beat>
    tone:                 <mood word>
    density:              <sparse | building | full>
    emphasis:             <what the music should underline — usually an anchor or a reveal>

REACTION OPPORTUNITIES  (conceptual only)
  Moments a later reaction/commentary layer could cut to. Again a projection the
  reaction module owns; noted here so the edit leaves room for it.
  - at:                   <beat / boundary>
    opportunity:          <what would land there>

ACCEPTANCE
  How you will actually LOOK at the show before believing it works. Render and
  inspect at each of these; the defects live between the hero frames, so inspect
  the transition corridors, not just the holds.
  - wide:                 <inspected at a large terminal?            yes/no>
  - compact:              <inspected at a small terminal?            yes/no>
  - mono:                 <inspected with color depth forced to mono/ASCII? yes/no>
  - b - epsilon:          <frame just before each major boundary b>
  - b:                    <frame exactly at b>
  - b + epsilon:          <frame just after b>
```

---

## Why these fields and not others

- **Semantic anchors before anything visual.** A show is a sequence of *different
  pictures of the same thing*. If you can't name the thing, you have a slideshow.
- **One lesson per act.** If an act teaches two things it usually teaches neither.
- **Transitions get their own section** because the seam is where identity is lost,
  and losing identity is the one failure the whole method exists to prevent.
- **Future music / reaction are intent, not content.** The timeline stores opaque
  cues; the score and the reaction cut are separate projections off it, written
  later, owned elsewhere. Writing tones or reaction payloads *into* the show plan
  couples the picture to layers that don't exist yet — don't.
- **Acceptance names the terminal sizes and the transition corridor** because a
  show that only looks right at one size, in truecolor, on the hero frames, is a
  show that hasn't been inspected.

## A filled fragment (for shape, not to copy)

```
TITLE: The Observatory
LOGLINE: the same shelf of media is six different interfaces, and then the data
         underneath it is three different instruments.

SEMANTIC ANCHORS
  - the shelf: the SAME set of items persists through every grammar
  - the selection: whatever is focused stays focused across the basis change

ACTS
  - id: grammars
    purpose: one experience model renders as six radically different interfaces
    representation: six experience grammars, one per hold
    dominant visual object: the focused item
    hold: ~6 s each — long enough to read the grammar, short enough to keep moving

TRANSITIONS
  - from -> to: list -> ring
    invariant: the focused item (same item, same label)
    transformation: the flat list lifts into a perspective ring; the focus stays centred
```

Keep it this terse. The plan is a lever, not a document.
