# Scene Algebra — Law Ledger (RXIII Checkpoint A; sharpened in RXIV Gate E)

A source-linked, executable ledger of the laws the `gibson::scene` composition algebra actually
obeys — checked against the **operational semantics** of `src/scene.rs` on this head, not against
category-theoretic vocabulary. The governing rule: do not claim a structure because an API supplies
`then()`/`with()`; find the law the machine implements, prove or falsify it, and label how it is
known.

Every row is backed by an executable test; run the suite with
`cargo test --test scene_algebra_laws --test scene_algebra --test field_view_coordinate_laws --test surface_fx --test wire_instrument_viewport`.

## The headline: a timed scheduling algebra with a *conditional* interchange law

`Effect::duration` makes `Sequence` the **sum** of its children and `Parallel` their **max**.
`Effect::eval` evaluates a `Parallel`'s children in declaration order (each override write —
Position, visibility, custom scalars, camera — is last-write-wins; displacements add; surface
effects append in order), and re-evaluates a *finished* `Sequence` child at its end so its final
state persists. Those two facts govern the algebra:

- **Durations** (sum vs max) break the strict interchange law unless the first tile is synchronized.
- **Write order** breaks it again even when synchronized: the two sides of
  `(f⊗g);(h⊗k) = (f;h)⊗(g;k)` evaluate the crossed operands `g` and `h` in opposite orders.

The law that actually holds (derived below, tested on finite cases) is:

> **Interchange under S ∧ C.** If **S**: `dur(f) = dur(g)`, and **C**: `g` and `h` write commuting
> channels (disjoint override footprints; overlaps only on additive displacement), then
> `(f⊗g);(h⊗k)` and `(f;h)⊗(g;k)` produce the same presentation at every time `t`.

RXIII stated the condition as "duration-aligned tiles". RXIV Gate E refuted that as insufficient
(the overlap counterexample, E1) and found that alignment of the *second* tile is unnecessary (E3).

## The ledger, by how each law is known

### 1. Derivable from the implementation semantics (with a code citation)

| Law | Derivation (`src/scene.rs`) | Executable witness (`tests/`) |
|---|---|---|
| Sequential composition is associative, over unequal durations too | `eval` Sequence subtracts each child's exact duration; grouping does not change any child's local time | `scene_algebra_laws::sequence_is_associative_even_over_unequal_durations` |
| Parallel `Displace` commutes | `Presentation::add_displacement` is an integer vector sum | `scene_algebra_laws::parallel_displacements_commute_but_position_writes_do_not` |
| Zero-duration ramp = instantaneous set to its endpoint | `local_t` returns 1.0 for a zero duration (no 0/0) | `scene_algebra_laws::zero_duration_ramp_jumps_to_its_endpoint` |
| An effect on a retired target writes nothing | `Scene::resolve` of an absent id is empty | `scene_algebra_laws::an_effect_targeting_a_retired_entity_writes_nothing` |
| Interchange under **S ∧ C** | With `dur f = dur g = δ`: for `t < δ` both sides evaluate `f, g` at `t` (and nothing else); for `t ≥ δ` both evaluate `f, g` at their ends and `h, k` at `t − δ`; both totals are `δ + max(dur h, dur k)`. The only remaining difference is evaluation order (`f,g,h,k` vs `f,h,g,k`), i.e. the swap of `g` and `h`, which is invisible exactly when their writes commute (C) | `scene_algebra_laws::interchange_holds_under_alignment_when_the_crossed_writes_commute`, `::first_tile_alignment_with_commuting_writes_suffices_for_interchange`, `::interchange_holds_when_the_parallel_branches_are_duration_aligned` |

The S ∧ C derivation is a hand proof over the `eval` source, not a mechanized one; it is
**sufficient, not necessary** (e.g. if `k` writes nothing, misalignment of the first tile can be
unobservable).

### 2. Tested only on finite examples (no general derivation claimed)

| Law | Scope of the evidence | Witness |
|---|---|---|
| Non-finite (NaN) visibility omits the entity without panicking; methods agree | one NaN case | `scene_algebra_laws::a_non_finite_visibility_omits_the_entity_without_panicking` |
| Camera channel composes through sequence (later write wins, not summed) | one two-pan sequence | `scene_algebra_laws::camera_pan_composes_through_sequence` |
| `SurfaceFx` chains: empty = identity, concatenation associative, non-commutative | sampled chains | `surface_fx::ordered_chains_are_associative_but_not_commutative` |
| Complete graphemes indivisible; scoped effects bounded; holes stay holes | sampled wide/narrow glyph boundaries | `surface_fx::scoped_wide_boundary_is_indivisible_and_protects_neighbors`, `::scoped_movement_never_imports_outside_content_or_exports_partial_glyphs`, `::overlay_preserves_ordinary_glyphs_layout_and_holes` |
| `index → world → index` is the identity | exhaustive over the tested lattices (finite, but every cell) | `field_view_coordinate_laws::the_index_world_index_round_trip_is_the_identity_on_the_whole_lattice` |
| Half-open cell geometry; no fake inverse past the edges | edge cases of the tested extents | `field_view_coordinate_laws::cell_geometry_is_half_open_with_no_fake_inverse_at_the_edges` |
| Four distinct sample classes (Value / Masked / NonFinite / OffField); absence never a fake zero | constructed cases of each class | `field_view_coordinate_laws::the_four_sample_classes_are_distinct_and_absence_is_never_a_fake_zero` |
| Probe discloses quantization granularity + provenance | sampled probes | `field_view_coordinate_laws::a_probe_discloses_its_quantization_and_provenance` |
| Terminal cell ↔ source index under origin / anisotropy / zoom | sampled geometries | `wire_instrument_viewport::terminal_cells_map_back_to_their_source_index_under_origin_anisotropy_and_zoom` |
| Non-invertible display zoom is a typed refusal | the overflow case | `wire_instrument_viewport::a_cell_size_whose_zoom_overflows_is_refused_not_silently_blanked` |

### 3. Valid only under explicit independence / synchronization assumptions

| Law | Assumption | Witness |
|---|---|---|
| Interchange `(f⊗g);(h⊗k) = (f;h)⊗(g;k)` | S (`dur f = dur g`) ∧ C (`g`,`h` writes commute) | rows in §1 |
| Parallel composition commutes | the operands write only commuting channels (displacement, or disjoint targets/channels) | `scene_algebra_laws::parallel_displacements_commute_but_position_writes_do_not` |

### 4. Refuted unrestricted laws (counterexamples executed)

| Claimed law | Counterexample | Witness |
|---|---|---|
| Strict monoidal interchange, unconditionally | `f=10s, g=1s, h=1s, k=10s` on independent entities: left lasts `max(10,1)+max(1,10)=20s`, right `max(11,11)=11s`; at `t=5s` entity `d` is at `0` on the left and `4` on the right | `scene_algebra_laws::strict_monoidal_interchange_fails_under_the_timed_interpretation` |
| **(E1, RXIV)** Interchange under duration alignment alone | four 1s operands on ONE entity: `f` idle, `g` Position→10, `h` Position→6, `k` idle. Both sides last 2s; at `t=1.5s` left = **3** (`h` wrote last), right = **10** (`g` wrote last); at `t=2s` 6 vs 10. Recorded RED against the RXIII claim (`left: (3, 0)`, `right: (10, 0)`) | `scene_algebra_laws::interchange_fails_under_duration_alignment_when_crossed_operands_overwrite_one_channel` |
| Parallel `Translate` (Position override) commutes | swapping two parallel translates on one entity changes the result `(0,2)` ↔ `(3,0)` | `scene_algebra_laws::parallel_displacements_commute_but_position_writes_do_not` |

### 5. Stronger categorical interpretations — unverified (and not claimed)

| Interpretation | Status | Why |
|---|---|---|
| `(Effect, sequence, parallel)` is a strict monoidal category | **not claimed** | refuted unrestricted (§4); only the conditional interchange holds |
| a duoidal category / operad | **not claimed** | the code defines none of the extra operations or coherence laws |
| `Scene::to_node` is a strict functor `SCENE → UI` | **not claimed — it is a lowering** | it acts on a `Presentation` object and has no action on effect morphisms; two different effects evaluating to the same presentation lower to the identical node (`scene_algebra_laws::to_node_factors_through_presentation_so_it_is_a_lowering_not_a_strict_functor`) |
| associativity/identity "through the functor" (`scene_algebra::effect_identity_and_associativity_through_the_functor`) | **tested on equal-duration examples** | the test name predates this ledger; it is evidence for §1 associativity, not for functoriality |

The module docs of `src/scene.rs` use this vocabulary only as labelled analogy ("parallel
composition … named by analogy to a monoidal product; it is not a strict one"; "the lowering
`Render : SCENE → UI` … not a strict functor"). No categorical machinery is added to the public API:
no consumer needs it.

## Notes on the mechanism

- **Why durations break interchange (§4 row 1).** `Sequence` sums and `Parallel` maxes, so
  `max(10,1)+max(1,10)=20 ≠ max(10+1,1+10)=11`. The divergence is observable at `t=5s`, not a
  duration-field artifact.
- **Why write order breaks it again (E1).** With all four durations equal, every primitive sees the
  same local time on both sides — but the left evaluates `f, g, h, k` and the right `f, h, g, k`.
  Position is an override, so whichever of `g`/`h` is evaluated last wins: `h` on the left (mid-ramp
  `round(6·0.5)=3`), `g` on the right (its persisted endpoint `10`, because a finished sequence child
  is re-evaluated at its end).
- **Why only the first tile must be synchronized (§1, E3).** After `δ = dur f = dur g` both sides
  hold `f, g` at their ends and run `h, k` from the same instant, so the second tile's widths can
  differ freely (`1s` vs `3s` is pinned).
- **Absence is a class, not a value (§2).** `Value(0.0)`, `Masked`, `NonFinite`, and off-field are
  four distinct outcomes; nothing is coerced to a plausible zero.

_Last updated: 2026-11-05 (RXIV Gate E). Experimental, Rust-only API; tested semantics do not imply
an API-stability promise._
