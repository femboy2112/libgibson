# M5-B Slice 1 evidence

`golden_rev_id.py` is an independent numpy float32 replication of the documented canonical
serialization and evaluation order, for the synthetic fixture. It produces the golden `rev_id`
pinned in `tests/figure_mechanics.rs::golden_rev_id`. The value was derived before the Rust
implementation was ever run.

    python3 -I docs/evidence/m5b/golden_rev_id.py 1.25 0.5
    # 2127235299442fabca19419d1ae30a93ba9d075b592f0bc9a933451264a92ad5

If the Rust test disagrees, diff `rev.canonical_bytes()` against this script's bytes. Never copy
the Rust output into the test. Bit-stability is claimed on x86_64 only.
