# DIRECTOR roadmap: from one performing actor to cinematic scenes (PROPOSED)

Status: **PROPOSED design only.** No code, no API freeze, nothing here is implemented. Everything
marked PROPOSED is a future seam, not a shipped feature. Numeric thresholds are proposals; the
owner approves any threshold before it becomes a gate. This document is the Lane D deliverable of
OPERATION INCARNATE II and must not consume the FIRST PERFORMANCE round.

## 0. Reading rules

Every capability named below carries exactly one status label.

| label | meaning |
|---|---|
| SHIPPED | on `main` (the `figure` feature, `figure::motion` C-1 module, `figure` contract sections) |
| ON-BRANCH | written on a lane-B feature branch, not merged to `main` when this was written; some of it was never compiled. Treat as DRAFT |
| DRAFT | specified in a design doc (studio API, tape format v1, naming contract v1), no merged code |
| PROPOSED | introduced by this document |

Hard constraints inherited from the rulings (they bind every milestone below):
- libgibson stays generic. It knows joints, channels, transforms, durations and vectors. It has no
  persona, character, Being, memory or permission concept, and never will.
- Zero runtime dependencies (ruling #39), MSRV 1.85, `figure` feature off by default.
- v0 semantics and the golden `rev_id` stay bit-identical; changed semantics get a new explicit
  version (`...v1`, `...v2`), never an in-place edit.
- Runtime, offline-authoring and renderer-backend dependencies are three different tiers (section 7).
- Motion is a pure function of presentation time, C1 in velocity, blended at every transition,
  never stop-motion (owner ruling 17; section 4).

## 1. Purpose and non-goals

Purpose: record how today's single-actor performance layer grows, in checkable steps, toward the
owner's long-term target of production-grade cinematic character scenes, without redesigning what
already exists and without letting the long-term target distort the near-term deliverable.

A DIRECTOR layer is what sits **above** the single-actor performance and answers questions that
one actor's script cannot: where is the camera, what is in frame, who else is in the scene, where
may an actor stand or reach, when does something happen, and can the whole thing be scrubbed,
re-evaluated and exported repeatably.

Non-goals of this document:
- implementing any of the above in the FIRST PERFORMANCE round;
- reinventing a mature production tool (Blender, glTF tooling) to satisfy a dependency ideology;
- any statement about who an actor is. Identity of a *character* belongs to the host application.

## 2. Architecture: five seams

The owner's decomposition is World / Living Form / Performance / Director / Render. The ruling
(#86) is "one Living Form, one performance, two renderers". The diagram shows data flowing down;
nothing flows up except diagnostics.

```text
  host application (owns who/why/permission)      <- outside libgibson
        | admitted intents, scene description, recipes
        v
  +-----------+   +---------------+
  |  WORLD    |   |  LIVING FORM  |   World: scene-local ids, transforms, props, targets
  | (scene)   |   | (recipe->body)|   Living Form: body, rig, garments, hair, materials
  +-----+-----+   +-------+-------+
        |                 |
        +--------+--------+
                 v
          +-------------+      Performance: pose(t) per actor, pure function of time,
          | PERFORMANCE |      plus the incremental Live evaluator
          +------+------+
                 v
          +-------------+      Director: shots, cameras, timeline, events, cuts,
          |  DIRECTOR   |      scrub / re-evaluate / export. Owns NO body and NO pose maths.
          +------+------+
                 v
   +-------------------------------+
   | tape(s) + scene file          |  <- the shared contract (single source of truth)
   +---+---------------------+-----+
       v                     v
  RENDER: libgibson      RENDER: offline adapter
  software preview       (Blender today; others optional)
```

Seam rules (PROPOSED, each one is a testable invariant):
1. **Down-only data flow.** Render never feeds back into Performance. A renderer may report
   diagnostics (for example "garment not drawn: missing FitStatus"), nothing more.
2. **One tape format, many consumers.** Every renderer consumes the same tape bytes. A renderer
   that needs data the tape lacks is a signal to extend the tape (new version), not to compute a
   second pose.
3. **Director owns no pose maths.** It places, times and frames. Joint locals come only from
   Performance. If a director feature seems to need IK, it is a Performance feature.
4. **Scene-local identity is separate from content identity.** `body_rev_id`, `fit_id` and
   `present_id` are content hashes; two actors may legitimately share all three. An actor id is
   assigned by the scene and never derived from a content hash.

## 3. What exists today, and what a director would add

Inventory of the single-actor layer, with honest status.

| element | status | relevance to DIRECTOR |
|---|---|---|
| `figure::motion`: `Motion::sample(t)` pure function, `Preempt` (inertialization), `Crossfade`, `Look`, C1 keys | SHIPPED | the template for every director curve: camera paths, light ramps and prop constraint weights must obey the same law |
| `studio::compile` / `RecipeV1` / three identities | DRAFT | per-actor body definition; the scene refers to recipes, never copies body state |
| `PerformanceScript` of `Beat`s (walk, stop, look, gesture, sit, stand, idle, express) plus `cuts` | ON-BRANCH | single actor, strictly sequential; `Express` takes no script time; walks cannot be cut |
| `Live`: `submit(Intent)`, `tick(now_ms)`, reduced motion, quiesce, stop | ON-BRANCH | the interactive source; stateful, so not scrubbable (section 4.3) |
| `Intent` kinds: look, acknowledge, wave, sit, stand, walk_to, idle, express (closed, non-exhaustive) | ON-BRANCH | the only semantic vocabulary a controller may use today |
| `Performance::stance_windows`, `WalkSummary`, `TimelineEvent` | ON-BRANCH | already expose planted-foot intervals and timeline events: raw material for blocking checks |
| Tape `gibson.figure.tape.v1`: header + little-endian f32 body, deterministic bytes | DRAFT (writer ON-BRANCH) | the contract every renderer shares |
| Tape header `events` ([{frame, kind, label}]) | DRAFT | the seed of an event track |
| Tape header `camera_cuts` ([{frame, camera_id}]) | DRAFT | the seed of a shot list; values are supplied by the caller through the identity block, the runtime does not generate them |

What a DIRECTOR layer would add, all PROPOSED:
- a camera model and a camera-motion type (section 5);
- a scene description that names actors, props, targets and cameras with scene-local ids;
- a timeline of tracks (actor tracks, camera track, event track, later light and prop tracks);
- spatial constraints shared between actors and props (contact, reach, aim, separation);
- scene export to the same interchange targets as bodies (section 7).

## 4. Time, continuity and the continuous-motion rule

The rule (owner ruling 17, section "C-1" of the figure contract) restated for scenes:
**every quantity a viewer can see change is a pure function of presentation time, has continuous
velocity, and is blended at any transition. Stop-motion, held-then-snapped values and
velocity-discontinuous cuts of a moving object are defects.**

### 4.1 What the rule covers beyond bodies
| scene quantity | requirement (PROPOSED) |
|---|---|
| camera position, orientation, focal length | pure f(t), C1; a retarget (new subject, new framing) is inertialized from current value and velocity, exactly as `Look` retargets are |
| light intensity, colour, direction | pure f(t), C1 ramps; no snap at a cue |
| prop transforms | pure f(t); attach and detach are a *weight ramp* over an explicit ease, never a one-frame reparent |
| actor handoff between performance sources (scripted to interactive) | goes through `Preempt`: current value and velocity are carried across |
| actor-to-actor and actor-to-prop constraints | constraint weight is itself a C1 curve; a constraint that becomes unsatisfiable degrades through the soft limit idea of the existing IK (no pop at full extension) |

### 4.2 Cuts are not motion, but they are not free
A hard **cut** is an editorial boundary between two shots, not a motion of one object. PROPOSED
position: the continuity rule applies *within* a shot and across any *camera move or dissolve*;
a hard cut is allowed only as an explicit `camera_cuts` entry. It is never produced by a
camera that teleports inside a shot. Whether a cut may happen mid-gesture is a direction
question, not an engine question; the engine only guarantees that the **actors' motion is
continuous through the cut** (the cut changes the viewpoint, never the pose). See open question Q2.

### 4.3 Scrubbing, re-evaluation and the stateful Live
- A compiled script is a pure function of time, so it can be evaluated at any `t` in any order:
  random access equals sequential access, bit for bit. That is the property that makes a scrub bar,
  partial re-render and repeatable evaluation possible.
- `Live` is incremental and stateful (an intent queue and carried velocities). It is **not**
  scrubbable. PROPOSED route: a live session is *recorded* as an ordered, timestamped intent log
  (an editable script), which is then compiled like any scripted performance. The interactive
  source and the offline director meet at the script, not at the evaluator.
- Tape v1 stores sampled poses only, with no velocities. A consumer that plays at a higher rate
  than the tape (for example a 60 fps preview of a 30 fps tape) can only interpolate samples,
  which is C0. PROPOSED: either re-evaluate the script at the render rate (preferred, since
  evaluation is deterministic and cheap) or add rate channels in a new tape version. See Q4.

### 4.4 Determinism and edit locality
- Same recipe + same script + same scene file gives byte-identical tapes.
- **Edit causality** (PROPOSED invariant): changing a key at time `T` leaves every frame earlier
  than `T` minus that edit's ease byte-identical. Normalizing or globally re-fitting on edit
  breaks this and is forbidden. It is also the gate that makes a timeline editor usable.

## 5. Shots, cameras and blocking (PROPOSED vocabulary)

All of this is **data**, interpreted by Director and consumed by renderers. None of it changes
body semantics.

- **Camera.** A named, scene-local object: position, orientation (or a look-at target by id), a
  vertical field of view or focal length plus sensor size and fit mode, near/far planes. The preview
  and the offline adapter must agree on the camera model; disagreement here is the most likely
  source of "same tape, different picture" (see D5 gate).
- **Camera motion.** A `Motion`-style pure function with C1 keys and inertialized retarget.
  Framing aids (look-at an actor joint role, keep-in-frame) are *aim layers* like `Look`: they
  smooth a direction, they do not solve physics.
- **Shot.** A named interval `[start, end)` binding one camera (or one camera move) to a framing
  intent: subject, size (wide, medium, close), headroom and lead-room as numbers. A shot list is
  an ordered sequence of shots; boundaries are cuts or blended transitions.
- **Camera transition.** Either a hard cut (a `camera_cuts` entry) or a blended move/dissolve
  with an explicit duration. A dissolve needs both cameras rendered, so it is a renderer feature
  and must be declared, not implied.
- **Blocking.** Named marks (a world point, a facing, optional tolerance) in scene-local
  coordinates; an actor beat targets a mark by id. Blocking converts "walk to the chair" into
  data that survives a scene edit. The existing planted-stop and seat-height beats are the
  single-actor case of a mark.
- **Composition checks** are measurable: subject bounding box as a fraction of frame height,
  distance of a joint role to the frame edge, head-room, and "no subject cut by the frame edge
  except by declared intent". They are the numeric hooks for the milestone gates.

For FIRST PERFORMANCE none of the above is a library layer. The required "camera-composition
change" is delivered by the caller supplying `camera_cuts` through the tape identity block, with
camera ids resolved by a small table in the example/kit. That is enough, and it is the honest
label: camera definitions live outside libgibson until milestone D1.

## 6. Timeline and events

PROPOSED model: a **scene timeline** is a set of **tracks** over one shared clock.

| track kind | content | form |
|---|---|---|
| actor track | a compiled single-actor performance (or an intent log, section 4.3) | script plus style, one per actor |
| camera track | camera move keys, shot boundaries, cuts | C1 keys plus discrete cut list |
| event track | named instants with kind and free label (cue, contact, line of action) | discrete, no effect on pose |
| prop track | rigid transforms, attach/detach with ease | C1 keys, weight ramps |
| light track | intensity/colour/direction keys | C1 keys (D5 and later) |

Rules:
- Tracks are independent data; a track never reads another track's output *except* through
  declared scene-local targets (section 8), so evaluation order is explicit and acyclic.
- The event track is **observation, not control**. An event may be consumed by a host (to start a
  sound, to log), but an event is never itself a command that changes a pose. The existing tape
  `events` list already has this character.
- Scene export is the pair (scene file, per-actor tapes). Re-importing the scene file and
  re-evaluating must reproduce the same tape shas (D4 gate).

## 7. Dependency tiers and interchange evaluation

### 7.1 Three tiers, never mixed
| tier | what lives here | rule |
|---|---|---|
| T0 runtime | the `gibson` crate, `figure` feature: evaluation, tape writer, software preview | zero dependencies (ruling #39); a new T0 dependency needs its own ruling |
| T1 offline authoring | Blender + MPFB and our scripts under `tools/figure_assets/` (pure `python3 -I` or inside Blender) | external tools, never linked; outputs only; per-file licence rows; GPL tool outputs are not encumbered but no GPL code or MPFB/MakeHuman python enters the repo |
| T2 renderer backend | Blender EEVEE/Cycles as an adapter consuming tape + scene file; other backends optional | an adapter is replaceable; the tape and scene file are the contract |

A fourth category, **T3 interchange libraries** (a glTF or USD parser/writer), is a runtime
dependency question and falls under ruling #39: either hand-written in pure Rust in the crate
(the tape writer already emits hand-formatted JSON) or kept in T1 tooling outside the crate.
The reserved features `figure-gltf` and `figure-vrm` need their own dependency ruling first.

### 7.2 Interchange candidates (evaluation, PROPOSED stance)
Statements about third-party formats are from general knowledge and must be verified against the
spec and licence before any commitment; they are not measurements made for this document.

| format | carries well | does not carry / weak | stance |
|---|---|---|---|
| glTF 2.0 | meshes, skins, morph targets, node animation, cameras (perspective/orthographic), scenes and nodes, punctual-light extension, extras for custom data | no timeline/shot/event model, no constraint model, no per-shot cameras, one animation clock per file in practice | **exchange format** for bodies, props and baked animation (ruling #86). Scene data we need beyond it goes in our own sidecar file, not into glTF `extras` |
| Blender `.blend` | everything | closed to the runtime, GPL tool | **authoring + offline render backend only**; never an interchange target |
| VRM | humanoid bone naming, expressions, look-at, metadata for avatars | single-avatar oriented; no scene; extension of glTF | **reserved**; revisit when a consumer needs avatar portability; naming contract v1 is a mapping source |
| USD (OpenUSD) | scene composition, layers, skeletal animation, time samples, cameras, lights, shot-style layering | large C++ dependency; heavy to link; the ASCII form (`usda`) is hand-writable in principle | **evaluated here**: PROPOSED as an *export-only* target at the scene level (D5+), written as text without linking the library if feasibility is shown; never a runtime dependency. Licence of the toolchain to be verified first |
| OpenTimelineIO | editorial timelines: tracks, clips, markers, transitions | no character or pose data | candidate for the *timeline view* of a scene only (D4+); optional T1 tool, never T0 |
| BVH / mocap clips | simple text joint rotations | no scale, no morph, no face, rig-name retargeting needed | candidate **input** for Performance (retargeting), not a scene format |
| FBX | broad DCC support | proprietary SDK | not adopted (licence and dependency) |

Reuse principle, owner-stated: reuse qualified tools where sensible. The expected end state is
that libgibson evaluates and previews; Blender renders production frames; glTF carries assets
across; our own small, versioned scene file carries what no format carries.

## 8. Multiple actors, identity isolation, performance sources

### 8.1 Isolation invariants (PROPOSED, each gets a test and a must-FAIL control in D3)
| invariant | statement |
|---|---|
| identity | each actor has a scene-local actor id independent of recipe hashes; two actors with identical recipes are still two actors |
| body | an actor's body state is derived only from its own recipe; editing actor B's recipe leaves actor A's tape byte-identical |
| state | each actor owns its own evaluator state (queue, carried velocities, face channels); there is no ambient or global performance state |
| authority | the library has no admission or permission concept; every command arrives as an already-admitted intent or a script from the host, per actor |
| cross-reference | an actor sees another actor only through a declared scene-local target (a named point, or a joint role of another actor sampled at time t); the dependency is listed in the scene file, so the actor dependency graph can be checked acyclic |
| resource | per-actor bounds exist (queue length, frames, duration), and the scene adds a total budget; exceeding it is a typed error, not a silent drop |

Assets (meshes, weights, target deltas) are shared **read-only and content-addressed**; sharing a
mesh between actors is allowed, sharing mutable state is not.

### 8.2 Performance sources are not permission grants
A character may be driven by a scripted timeline, an interactive user, an admitted semantic
controller, or a live host-side agent. To libgibson these are **four producers of the same thing**:
timestamped intents or beats for one actor id. Consequences (PROPOSED):
- the library attaches no rights to a source; the host decides who may drive which actor, before
  anything reaches libgibson;
- a source label may be recorded in an event for provenance, but, like the existing provenance
  passthrough, it never enters any identity hash and never changes evaluation;
- a controller emits intents at *beat rate* (events), not frame rate; evaluation between beats is
  deterministic and needs **no model inference per frame**.

### 8.3 Shared physical mechanisms
Contact, reach, aim and separation are scene mechanisms used identically for actor-to-prop and
actor-to-actor cases (a hand target is a hand target whether the target is a cup or another
actor's hand). Building them once, in Performance, with the existing soft-limit and planted-window
ideas, keeps "expressive exchange between actors" a later composition of existing parts rather
than a second system. Actors exchanging expressive performances is reserved as a route (targets
that are another actor's gaze or hand), not specified further here.

## 9. Explicitly OUT of FIRST PERFORMANCE

FIRST PERFORMANCE is one actor, one scripted performance, two renderers, matched cameras, two
body presets. Everything below is out of that gate, still in the destination, and lands where shown.

| item | why out now | lands in |
|---|---|---|
| multiple actors | needs the isolation machinery of section 8 | D3 (ladder 3) |
| scene graph, props, environments | no consumer yet; the mannequin and body need no hierarchy | D2 (ladder 3) |
| full wardrobe (more than the two reference garments, layering order, garment-garment fit) | two garments prove the fit pipeline | Living Form seam, after D2; needs a layering/precedence rule |
| cloth and hair dynamics | secondary dynamics need a stable body first and a bounded, deterministic solver | Performance seam, bounded secondary dynamics, after D3 |
| lip sync and eventual speech articulation | needs an audio/phoneme timeline input and viseme channels beyond the 22 face channels of naming contract v1 | Performance seam; the `face` channel mechanism is the natural carrier; audio timeline arrives with D4 |
| mocap, clip interchange, retargeting | needs a retarget map and a rig-name contract per source | Performance seam, after D2 |
| shot/camera layer in the library | the kit supplies `camera_cuts` by hand | D1 |
| lighting tracks, atmospheric effects | renderer features; tape has no light data | D5 |
| timeline editing UI, scene export | an editor is a host feature; the export file is D4 | D4 |
| runtime customization claims beyond what works | owner rule: label offline-only work as offline-only | applies always |

## 10. Acceptance ladder and staged milestones

Owner ladder: (1) one configurable humanoid, (2) one natural interactive performance,
(3) multiple actors and props with coherent spatial constraints, (4) editable cinematic scenes and
timeline, (5) high-quality offline production rendering. Rungs 1 and 2 are the FIRST PERFORMANCE
gate and are owned by Lane B; rungs 3 to 5 are outside it. D0 to D5 below are the *DIRECTOR*
steps, in dependency order. Every row has a numeric gate and a control that must FAIL, in the
style of the existing gates (the control proves the gate is not vacuous). Thresholds are
PROPOSED; those inherited from ratified #466 instruments are noted.

| id | rung | content | numeric gate (PROPOSED) | must-FAIL control |
|---|---|---|---|---|
| D0 | 1 and 2 | single actor, scripted performance, tape v1, matched cameras by caller table, preview + offline adapter (Lane B, not new work) | inherited: stance foot speed at most 2 mm/s; C-1 jump metric J within its declared, derived bound; joint cross-section ratio at least 0.85 at 120 degrees; byte-identical tape on re-run; oracle max and p99 vertex error as ratified | inherited controls: walk with IK off, hard-cut control, LBS pose, stale-fit mutant |
| D1 | 2.5 | camera layer for one actor: camera model, C1 camera motion, shot list, `camera_cuts` generated from it, framing aims | camera path velocity jump on a 1 ms grid within a derived bound; every declared cut lands on its declared frame (exact); subject bounding box height within a declared band (for example 55 to 90 percent of frame height) on every frame of a "full" shot; two runs byte-identical | a camera that snaps between keys (jump metric must exceed bound); a framing target that drives the subject out of frame (band check must fail) |
| D2 | 3a | scene description v0: scene-local ids, one actor, rigid props, marks, targets resolved to world points; attach/detach with weight ramp; reach to a prop | hand-to-target error at the contact frame within a declared tolerance (for example 20 mm); attach/detach velocity jump within bound; scene file round-trips with identical sha; prop penetration depth/count against a declared collider budget | attach with a one-frame reparent (jump must exceed bound); a mark placed unreachable (reach must report a typed error, not a silent clamp) |
| D3 | 3b | two or more actors in one scene; isolation invariants; shared contact/aim/separation mechanisms; acyclic cross-actor targets | editing actor B changes zero bytes of A's tape; minimum inter-actor separation at least the sum of declared collider radii on every sampled frame; actor dependency graph acyclic; per-actor budgets enforced with typed errors | a mutant sharing evaluator state (A's tape must change when B is edited); a script that walks one actor through another (penetration count must be nonzero); a deliberate two-actor target cycle (cycle check must reject) |
| D4 | 4 | timeline of tracks, event track, scrubbing, edit locality, scene export/import | random-order evaluation equals sequential evaluation on every sampled frame (zero byte diff); edit at time T leaves frames before T minus ease byte-identical; export then import then re-evaluate reproduces all tape shas | a stateful accumulator mutant (scrub order changes output); a global-renormalize mutant (earlier frames change); an exporter that drops one event (round trip must fail) |
| D5 | 5 | offline production path from scene file + tapes: multi-camera, lights, backend adapter parity | camera parity: projected pixel positions of fixed 3D test points agree between preview and backend within 0.5 px; per-frame posed-vertex oracle max and p99 within the ratified bounds on sampled frames for every actor; frame headers at least 1920x1080 stills and 1280x720 clips | swapped sensor-fit mode (parity must fail); backend that ignores the skeleton update; backend that ignores the surface update; stale-fit tape |

Gate hygiene (applies to every row): instruments are committed, run in a detached worktree under
heavy, and fixtures are synthetic; a mutant must be killed by a non-golden assertion;
the session **views** the rendered stills and strips and names concrete defects before any visual
claim, and a numeric pass is never reported as "looks good". Separate verdict rows remain:
structural, aesthetic, motion naturalness, likeness, owner approval, machine performance.

Parallel Performance-seam tracks that are *not* DIRECTOR steps and carry their own gates when
scheduled: P1 mocap/clip import and retargeting; P2 bounded cloth/hair secondary dynamics (must
be deterministic, bounded in time per frame, quiescent when hidden); P3 lip sync (viseme channels
driven from an audio timeline; reduced-motion and `lower_face_covered` rules apply unchanged).

## 11. Dependencies on shipped and draft APIs

| director step | needs from | gap to close first |
|---|---|---|
| D1 | `Motion`/`Preempt`/`Look` (SHIPPED); tape `camera_cuts` (DRAFT); `studio::Compiled::preview(&Camera...)` (DRAFT) | the Camera type is mentioned but its model (sensor fit, near/far, lens) is not frozen; tape has no camera definitions, only ids |
| D2 | `RigRoles`, IK, planted windows (ON-BRANCH) | need a world-target type with scene-local ids; the performance layer takes raw `Vec3` targets today |
| D3 | `Live`, per-actor `Performance` (ON-BRANCH); three identities (DRAFT) | evaluators must be provably free of shared state; per-actor resource bounds exist, a scene total does not |
| D4 | pure-function evaluation (SHIPPED law, ON-BRANCH implementation); tape events (DRAFT) | tape v1 has no rates and no scene reference; a new tape version (or sidecar) is needed, with v1 kept bit-identical |
| D5 | Blender adapter and posed-vertex oracle (Lane B work in progress) | adapter currently covers one actor and one camera table |

Compatibility promise (PROPOSED): tape v1 and `rev.v0`/`rev.v1` bytes never change. DIRECTOR adds
**new** files and **new** version strings; a v1 consumer keeps working on v1 files.

## 12. Risks and anomalies noticed in the current drafts

These are observations about drafts, not defects of shipped code.
1. **Camera ids with no camera definitions.** Tape `camera_cuts` carries ids, but no document
   defines a camera, so two consumers can resolve the same id to different views. FIRST
   PERFORMANCE survives this because one kit supplies the table; a library layer cannot.
2. **Rate loss in the tape.** Sampled poses without velocities cannot be resampled C1 (section 4.3).
3. **Single-actor script shape.** A `Beat` list is strictly sequential; `Express` is the only
   beat that overlaps another. Two things at once for one actor (walk while waving) is not
   expressible, and neither is a beat targeting another actor. Layering belongs to Performance,
   not to Director, and should be solved there before D2 needs it.
4. **Walks cannot be cut or preempted.** The script's cut mechanism excludes walks, and the live
   path preempts only non-walk beats. A director that wants to interrupt a walk (a cut-away, a
   new blocking mark) has no mechanism today; it needs a Performance-seam answer first.
5. **Closed intent vocabulary.** The `Intent` kind set is closed and non-exhaustive on purpose;
   a director must not smuggle new kinds through labels. New kinds need a version note.
6. **Event kinds are free strings** in the header with a closed set in the producer; consumers
   must not parse labels for behaviour.
7. **Frame budget per scene.** Per-script caps (beats, seconds, frames, queue) are per actor; ten
   actors multiply them. A scene-level budget is required before D3.
8. **Visual parity is not numeric parity.** Posed-vertex and camera parity can be proven
   numerically; shading, shadows and lighting will always differ between the software preview and
   a production renderer. The label "offline reference render of a runtime-evaluated performance"
   must stay on every offline render, and lighting parity is never claimed.

## 13. Open questions for the coordinator

| # | question | why it matters | default if unanswered |
|---|---|---|---|
| Q1 | Does a camera belong to the library (a typed, versioned `Camera` with a documented model) or stay a kit-level table until D1? | decides whether preview and Blender parity is a library gate or an example gate | kit-level table through FIRST PERFORMANCE; library type at D1 |
| Q2 | Is a hard cut exempt from the continuous-motion rule when the actor's motion is continuous through it (section 4.2)? | defines the camera gate (jump metric excludes declared cuts) | exempt, declared cuts only |
| Q3 | May the scene file be our own small versioned text format, with glTF used only for assets and baked animation? | avoids stuffing scene semantics into `extras` or taking a USD dependency | yes: own format, glTF for exchange, USD export-only later |
| Q4 | Tape v2 with rate channels, or always re-evaluate the script at the render rate? | decides whether preview at 60 fps can be C1 from stored tapes | re-evaluate at render rate; tapes stay samples |
| Q5 | Does ruling #39 allow a hand-written pure-Rust glTF writer inside the crate (as the tape writer already does for JSON), or must glTF live in T1 tooling? | gates the reserved `figure-gltf` feature | T1 tooling until a ruling |
| Q6 | Who owns the scene file schema once more than one consumer exists: libgibson (generic, no character meaning) or the host? | prevents host semantics leaking into libgibson | libgibson owns geometry/time/ids only; hosts extend through opaque, hash-excluded fields |
| Q7 | Is OpenTimelineIO worth evaluating for the timeline view, or does the owner want strictly our own track model? | scope of D4 | own model first; evaluate at D4 start |
| Q8 | Does the owner approve the PROPOSED thresholds in section 10 (for example the 55 to 90 percent framing band, 20 mm contact tolerance, 0.5 px camera parity), or want different bars? | the owner approves any threshold change | thresholds stay PROPOSED until approved |
| Q9 | Where should recorded interactive sessions live: as intent logs the host stores, or as a library file format? | section 4.3 depends on it | library defines the log format, host stores it |
| Q10 | Priority between P1 (mocap/retarget), P2 (secondary dynamics) and P3 (lip sync) relative to D2 to D4 | sequencing after FIRST PERFORMANCE | D1, D2, then P1; P2/P3 after D3 |

**Coordinator ruling #110 (recorded):**
- **Adopted as stated:** the defaults for Q1, Q2 (declared cuts only), Q3, Q4, Q5, Q6, Q7, Q9
  and Q10.
- **Q5 detail:** glTF stays in T1 tooling. There is no T0 glTF in the crate without an amendment
  to ruling #39.
- **Q8:** the thresholds stay PROPOSED and go to the owner's batch after FIRST PERFORMANCE.
- Lane D is closed for this round.

## 14. Provenance, licence and privacy notes

- This document is original design prose; it copies no third-party text. Format and tool
  statements in section 7 are unverified general knowledge, to be checked against the specs and
  licences before any dependency or export is committed.
- Any asset a later milestone uses needs a manifest row: source URL, sha256, quoted CC0 text;
  CC-BY, GPL or unverifiable assets are dropped and that part is authored procedurally.
- Fixtures for every gate are synthetic. Rendered frames stay under the local state directory,
  never in the repository, except small committed subsets approved for the generic preset.
- No part of this design reads a host's identity or memory. If a future milestone seems to need
  that, it belongs in the host and arrives as an already-admitted intent.

### SECOND PERFORMANCE candidate

Living Matter now sits between the admitted kinematic Performance and the geometry drawn by Render. Director selects an actor, scene-local chair and attention target, walk/stop/turn/sit beats and shot cuts. A production may choose a named authoritative physical backend; its state travels beside the kinematic tape. Read the implementation and compatibility limits in [Living Matter](living-matter.md). Multi-actor interaction, editable choreography and production authoring remain later milestones.
