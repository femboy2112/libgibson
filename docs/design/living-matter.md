# Living Matter candidate contract

World and Living Form supply scene geometry and the shaped, fitted neutral actor. Performance supplies admitted skeletal targets and face channels. Living Matter integrates the trajectory between targets. Director chooses the scene, authoritative realization and shots. Render consumes the realized positions. The host remains responsible for character ownership, admission, background behavior and STOP.

## Units and state

World space is right handed, Z up. Positions and collision distances are metres, time is seconds, mass is kilograms, and the default gravity is (0,0,−9.80665) m/s². The fitted neutral actor is immutable. Body, fit and presentation identifiers retain their public definitions. The solver separately hashes the source body, fit, hair geometry, material coefficients selector, regional settings, gravity, substep and iteration counts and scene boxes. A checkpoint binds both this configuration and the presentation. Gravity and wardrobe support change force balance without writing morphology.

Advance once for each admitted fixed-frequency input (10–240 Hz). The default is four substeps and six ordered projections per substep. Valid limits are 1–16 and 1–32. Pin targets interpolate from their previous admitted world position. Contact geometry is sampled at the target pose. Reset by constructing the solver from the fitted rest mesh. Offline playback holds the first pose for an explicit pre-roll. The host may pause while hidden and construct a new bounded state when visible. Advancing on stale wall time after a long suspension is outside this contract.

The skirt uses the existing sewn periodic cut. Only its waist row is constrained to the body. Area distributes mass, distance constraints model stretch, shear and span bending, and material coefficients select light, medium or structured response. The surface hash includes the full posed body under its coverage masks and fitted inner garments. Contact has a 4 mm margin and a 28 mm narrow band. It is a discrete projection with friction. Hair uses eight rooted groups of five guides and a blended cage. Regional tissue uses a bounded anisotropic local damped mode. The upright, unsupported neutral is a loaded equilibrium. Gravity changes are evaluated relative to that equilibrium. Support adds a parallel spring and reversible lift. The mesh displacement field has zero first-order divergence. Finite-deformation volume error needs measurement. This is a reduced-order artistic model.

## Backend decision

XPBD scales compliance by the inverse square of the substep and accumulates a constraint multiplier through the iterations. This makes the spring fixture independently checkable. The implementation uses the existing scalar Rust math and needs no runtime dependencies. [Macklin, Müller and Chentanez, 2016](https://mmacklin.com/xpbd.pdf)

PositionBasedDynamics has a broad C++ constraint implementation, useful for an offline comparison but requiring a new build and FFI for the live Rust target. Rapier supplies maintained Rust rigid-body collision and constraints, and introduces a dependency graph. Neither was added to the runtime for this candidate. Blender's independently integrated cloth fixture is an offline direction and attachment comparison. Its numerical trajectory and configuration are backend-specific. [PositionBasedDynamics](https://positionbaseddynamics.readthedocs.io/en/latest/about.html) [Rapier](https://rapier.rs/docs/user_guides/rust/getting_started/)

## Interchange

The kinematic tape is still gibson.figure.tape.v1. The optional gibson.living-matter.bake.v1 header records source tape, export, body/fit/presentation and simulation identities, rate, pre-roll, parameters, mesh topology, binary SHA-256 and stride. The binary stores world XYZ f32 LE in frame and named cage order. Seeking a production frame is one indexed read. Rendering a physically driven cage bypasses its armature to avoid applying skinning twice. Host-authored relative expressions layer on top.

GLMCKP01 stores the config and presentation digests, tick, locked rate, local regional displacement, speed, previous anatomical frame and velocity, and all particles' position, velocity and previous targets. Fields use little endian and the final SHA-256 protects the payload. A production checkpoint specifies the next input frame. Resume requires the identical configuration, kinematics and solver build. Validation rejects damaged or mismatched state atomically. Same-build scalar repeatability and cross-platform bitwise equivalence are distinct claims.

## Proposed laboratory gates

These thresholds were selected on the analytical setup before the comparative actor runs. They are proposed for GOD review. Existing visual and fit bars retain their original values.

| Probe | Pass | Falsifying control |
|---|---|---|
| Loaded hanging mass | 0.5 kg × 9.80665 m/s² × 0.004 m/N gives 19.61 mm extension, within 2 mm. Fresh zero gravity stays in place. Rotating load rotates the response. | Feed zero force to the loaded case. |
| History | Same target after a lateral excursion differs by >1 mm transiently, converges within 1 mm after settling. | Replace transient with the common pose-indexed state. |
| Material | Identical topology and force, profiles change the result by >0.5 mm on the calibration patch. | Alias all profiles to medium. |
| Support | A support change reversibly lifts the region >3 mm. After removal residual is <0.01 mm. | Hold the unsupported settings. |
| Collision | A test point just inside a known oriented triangle returns outside at the 4 mm boundary to 0.01 mm. | Disable projection in the same trial. |
| Time | Repeated identical runs reconstruct identical checkpoint bytes. 30/60 Hz and 2/4/8 substeps agree on static analytical extension to within 2 mm. | Reject nonfinite, damaged state and a differing rate. |

Actual body crossings, settling energy, render visibility and garment fine-surface self-contact are reported separately from these fixtures.
