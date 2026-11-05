# Scene Algebra — Law Ledger (RXIII Checkpoint A / Gate A)

A source-linked, executable ledger of the laws the `gibson::scene` composition algebra actually
obeys — checked against the **operational semantics** of `src/scene.rs` on this head, not against the
category-theoretic vocabulary in its module docs. The governing rule of this gate: do not claim a
structure because an API supplies `then()`/`with()`; find the law the machine really implements,
prove or falsify it, and label it honestly.

Verdicts: **VALID** (holds for all accepted inputs) · **VALID UNDER CONDITIONS** (holds under a
stated synchronization/domain condition) · **COUNTEREXAMPLE** (a claimed law is false, with the
witness and mechanism) · **UNVERIFIED** (claimed by the docs but not established as stated).

Every row is backed by an executable test; run the suite with
`cargo test --test scene_algebra_laws --test field_view_coordinate_laws --test surface_fx`.

## The headline: it is a timed scheduling algebra, not a strict monoidal category

`Effect::duration` defines `Sequence` as the **sum** of its children
(`src/scene.rs` — `fold(ZERO, saturating_add)`) and `Parallel` as their **max**
(`src/scene.rs` — `.map(duration).max()`). Sum and max disagree, and that single fact governs the
whole algebra: sequential and parallel composition do **not** satisfy the strict monoidal
interchange law, because the two operators measure a composite's duration differently. Interchange
is recovered only when the parallel tiles are duration-aligned. The honest name for this structure
is a **timed scheduling algebra** (equivalently, a monoidal category with an explicit
same-duration synchronization condition on `⊗`), in which interchange is a *conditional* law — not an
operad, duoidal category, or strict monoidal category, none of whose extra operations/laws the code
defines.

| # | Law | Verdict | Source (`src/scene.rs`) | Proof (`tests/`) |
|---|-----|---------|-------------------------|------------------|
| A2 | Strict monoidal interchange `(f⊗g);(h⊗k) = (f;h)⊗(g;k)` | **COUNTEREXAMPLE** | `duration` (Sequence=sum, Parallel=max); `eval` Sequence/Parallel | `scene_algebra_laws::strict_monoidal_interchange_fails_under_the_timed_interpretation` |
| A2 | Interchange under duration-aligned tiles | **VALID UNDER CONDITIONS** | same | `scene_algebra_laws::interchange_holds_when_the_parallel_branches_are_duration_aligned` |
| A3 | Sequential composition is associative (also over **unequal** durations) | **VALID** | `eval` Sequence (exact per-child elapsed subtraction) | `scene_algebra_laws::sequence_is_associative_even_over_unequal_durations`; `scene_algebra::effect_identity_and_associativity_through_the_functor` (equal durations) |
| A3 | Parallel `Displace` (additive channel) commutes | **VALID** | `Presentation::add_displacement` (i128 vector sum) | `scene_algebra_laws::parallel_displacements_commute_but_position_writes_do_not` |
| A3 | Parallel `Translate` (Position override) commutes | **COUNTEREXAMPLE** | `eval` Translate → `set_offset` (last write wins, by iteration order) | same test |
| A3 | Zero-duration ramp = instantaneous set to endpoint | **VALID** | `local_t` (`d.is_zero() ⇒ 1.0`) | `scene_algebra_laws::zero_duration_ramp_jumps_to_its_endpoint` |
| A3 | Effect on a retired target writes nothing (failure identity preserved) | **VALID** | `Scene::resolve` (absent id ⇒ empty) | `scene_algebra_laws::an_effect_targeting_a_retired_entity_writes_nothing` |
| A3 | Non-finite (NaN) visibility omits the entity, no panic, methods agree | **VALID** | `to_node` positive predicate `v > 0.0` (Finding A, #77) | `scene_algebra_laws::a_non_finite_visibility_omits_the_entity_without_panicking` |
| A3 | Camera channel composes through sequence (later write wins, not summed) | **VALID** | `eval` CameraPan → `out.camera`; `to_node` Viewport wrap | `scene_algebra_laws::camera_pan_composes_through_sequence` |
| A4 | `Render : SCENE → UI` is a **strict functor** | **UNVERIFIED** → lowering | `Scene::to_node` (consumes a `Presentation`, emits a `Node`) | `scene_algebra_laws::to_node_factors_through_presentation_so_it_is_a_lowering_not_a_strict_functor` |
| A5 | `SurfaceFx` chains: empty = identity, concat associative, non-commutative | **VALID** | `src/surface_fx.rs` `apply_chain` (sequential), `apply` | `surface_fx::ordered_chains_are_associative_but_not_commutative` |
| A5 | Complete graphemes indivisible; scoped effects bounded to their domain; holes stay holes | **VALID** | `src/surface_fx.rs` `scoped`, `overlay`, `masked`, `shift_row` | `surface_fx::scoped_wide_boundary_is_indivisible_and_protects_neighbors`, `::scoped_movement_never_imports_outside_content_or_exports_partial_glyphs`, `::overlay_preserves_ordinary_glyphs_layout_and_holes` |
| A6 | `index → world → index` round-trip is the identity on the whole lattice | **VALID** | `src/field_view.rs` `FieldExtent::{cell_center, world_to_index}` | `field_view_coordinate_laws::the_index_world_index_round_trip_is_the_identity_on_the_whole_lattice` |
| A6 | Half-open cell geometry; no fake inverse past the edges | **VALID** | `FieldExtent::world_to_index` (`fx<0`/`≥cols` ⇒ None) | `field_view_coordinate_laws::cell_geometry_is_half_open_with_no_fake_inverse_at_the_edges` |
| A6 | Four distinct result classes (Value / Masked / NonFinite / OffField); absence is never a fake zero | **VALID** | `src/field_view.rs` `Sample`, `FieldView::{sample, probe}` | `field_view_coordinate_laws::the_four_sample_classes_are_distinct_and_absence_is_never_a_fake_zero` |
| A6 | Probe discloses quantization granularity + provenance | **VALID** | `FieldView::probe` → `ProbeReport{quantization, cell_center, world}` | `field_view_coordinate_laws::a_probe_discloses_its_quantization_and_provenance` |
| A6 | Non-invertible display zoom is a typed refusal, not a silent blank | **VALID** | `examples/wire_instrument/viewport.rs` `world_view_for` → `UnsupportedView` | `wire_instrument_viewport::a_cell_size_whose_zoom_overflows_is_refused_not_silently_blanked` |
| A6 | Terminal cell ↔ source index under origin/anisotropy/zoom | **VALID** | `world_view_for` + `FieldView::probe_cell` | `wire_instrument_viewport::terminal_cells_map_back_to_their_source_index_under_origin_anisotropy_and_zoom` |

## Notes on the mechanism

- **Why interchange fails (A2).** Operands `f=10s, g=1s, h=1s, k=10s`:
  `sequence(parallel(f,g), parallel(h,k))` has duration `max(10,1)+max(1,10) = 20s`;
  `parallel(sequence(f,h), sequence(g,k))` has `max(10+1, 1+10) = 11s`. The divergence is
  **observable**, not a duration-field artifact: at `t=5s` the entity driven by `k` has slid to
  `round(10·0.4)=4` on the right (its `sequence(g,k)` is 4s into `k`) while on the left the tile
  `parallel(h,k)` has not begun, so it sits at baseline `0`. Equalize the four durations and the two
  compositions agree at every sampled instant — the synchronization condition.

- **Why `to_node` is a lowering, not a functor (A4).** A functor needs an action on morphisms
  (effects) preserving identity and composition. `to_node` acts on a `Presentation` *object* and has
  no morphism action; it factors entirely through the presentation. Two different effects that
  evaluate to the same presentation lower to the identical node, so the map carries no information
  about which morphism produced the state. It is an object/state lowering — accurate and useful, but
  not a strict functor, and the module docs are reworded accordingly where they overclaim. (No
  standalone categorical framework is added to `src/`: none is warranted by the operations that
  exist.)

- **Absence is a class, not a value (A6).** `Value(0.0)`, `Masked`, `NonFinite`, and off-field
  (`index: None`) are four distinct outcomes. A cell holding `0.0` is a `Value`; a masked or
  non-finite cell is reported as such; an off-field probe returns no index and no sample. Nothing is
  coerced to a plausible zero.

_Last updated: 2026-11-04 (RXIII Checkpoint A). Experimental, Rust-only API; tested semantics do not
imply an API-stability promise._
