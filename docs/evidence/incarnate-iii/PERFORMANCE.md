# Native performance and offline film

The producer in `tools/figure_assets/still_recipe` compiles the selected body
controls, garment order, slot bindings, hair and movement style through the
studio API. Every requested garment needs a Fitted report before the tool emits
a performance. The legacy three-argument command reproduces the original
generic tape binary with SHA-256
`60b86858f659c6287632cc9f555465df81a31247c2aee3a9b5a7643488b92da8`.

For a new named preset, append `--performance scene`, `idle` or `reference`.
The reference performance lowers both wrists through the rig's arm IK while
preserving bind positions. The scene uses native walk, stop, look, acknowledge,
sit and idle beats. It includes eight facial channels, a wide camera, a tracked
face close-up and a return to the wide camera for the sit. The chair center and
surface heights are resolved from the actual actor and its fitted garments.
The producer exports a chair collision slab and a rendered seat, back and four
legs in the same coordinates. The eye and body tracks remain on a 30 Hz clock.

The output directory contains `generic.tape.json`, `generic.tape.bin`,
`generic.fit.json`, `shots.json` and `scene.json`. A physics run also publishes
`matter.json` and its content-addressed binary. Choose a separate directory for
each identity and treatment. The optional `--kinematic-only` mode writes the
directed tape and stage. A physical run can set `--fabric light|medium|structured`,
`--gravity normal|zero|side`, `--support`, `--firmness`, `--damping`,
`--intensity` and `--preroll-frames`.

The generic trouser-suit preset changes stature, leg length, shoulders, hips and
waist. It uses the suit's actual trousers, a rust-colored material slot and the
expressive native motion style. Its physical trial on source `e4b40ef` wrote
283 frames and 63,899,136 cage bytes in 10.745 seconds after 15 settling frames.
Both the original skirted character and the transformed trouser suit passed
their runtime garment checks. Two earlier chair variants requested out-of-reach
hip positions when a full-width high box included arm geometry. The final
measurement checks triangle extents separately in the seat/leg and backrest
volumes. For the 283-frame trial the walk clearance lower bound was 25.0 mm,
and its posterior body witness was 0.000000013 m above the seat on the final
frame. Over the final 15 frames it ranged from −0.0000301 to 0.000000013 m.
These vertex measurements leave contact elsewhere on the actor unqualified.

For the generic offline film the class-s wrapper
`tools/figure_assets/lease_living_scene.sh render EXPORT PRESET TAPE_DIR OUT`
accepts `FILM_FROM` and `FILM_COUNT` for bounded slices. `encode` requires every
frame and verifies the decoded frame count. Source texture checks, the actual
renderer, camera, frame mapping and per-frame durations are recorded in
`render.json`. The authenticated seat witness can be remeasured with
`python3 -I docs/evidence/incarnate-iii/check_seat.py TAPE_DIR`.

The independent actor_history binary starts two real fitted actors from identical serialized state. One support translates laterally along an 8 cm excursion. Both are subsequently fed the identical tape pose for 135 frames. Before sampling, the proposed discriminators were a transient skirt/hair RMS over 1 mm and decay to below half that transient. Raw outcomes and any persistent resting folds are reported separately. The small analytical spring has a stronger millimetre convergence criterion.
