//! VAPOR95 palette revision — archival reproducibility and world-law gates.
//!
//! The archival VAPOR95 v1 world lives in `tests/common/vapor95_v1.rs` (a fixture, never a
//! `WorldId`). Before the revision it must BE the product world, field for field.
#[path = "common/vapor95_v1.rs"]
mod vapor95_v1;

use gibson::audio::human_music::MusicWorld;

#[test]
fn the_archival_v1_fixture_is_the_current_world_before_the_revision() {
    assert_eq!(
        format!("{:?}", vapor95_v1::vapor95_v1()),
        format!("{:?}", MusicWorld::vapor95())
    );
}
