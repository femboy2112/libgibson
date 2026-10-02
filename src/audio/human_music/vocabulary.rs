//! The harmonic vocabulary a [`MusicWorld`] and a [`MusicalLanguage`] declare — ONE law.
//!
//! A world constrains the chords its room may sound: four-note (and larger) qualities only where
//! it uses sevenths, a tone outside its home scale only where it allows modal mixture. A language
//! constrains colour: a sixth or a ninth only at a non-zero colour depth ("0 = triads/7ths only").
//!
//! The same law judges a cover target's pinned harmony (a lawful refusal when a pinned chord is
//! outside it) and, under [`super::policy::HarmonyPolicy::Vocabulary`], every chord a
//! performance's own planner produces: the backbone's colours, the phrase engine's choices and
//! every harmonic edit. So a source can never sound a chord its own world would refuse to cover.
//!
//! [`HarmonicVocabulary::conform`] is a *retraction* onto the admitted chords: it fixes every
//! admitted chord, keeps the root and the triad family ([`QualityFamily`]) and chooses the family's
//! simplest admitted member. It is what a room does with its OWN colour choice. It never moves a
//! root: a chart chord whose family has no admitted member on that root (a borrowed bVI in a room
//! without mixture) is not recoloured into another chord — the room refuses the song
//! ([`VOCABULARY_REFUSAL`]).

use super::language::MusicalLanguage;
use super::theory::{Chord, Quality, QualityFamily, Scale};
use super::world::MusicWorld;

/// The refusal a room gives a song whose chart names a chord outside the room's vocabulary.
pub const VOCABULARY_REFUSAL: &str =
    "the song's chart names a chord outside the world's harmonic vocabulary";

/// The chords one world, spoken in one language, may sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarmonicVocabulary {
    /// The world's home region: a chromatic tone is judged against it.
    pub home: Scale,
    /// Four-note (and larger) qualities.
    pub sevenths: bool,
    /// Sixth and ninth colours (the language's colour depth is non-zero).
    pub colour: bool,
    /// Tones outside the home scale.
    pub mixture: bool,
}

impl HarmonicVocabulary {
    /// The vocabulary `world` declares, spoken in `language`.
    pub fn of(world: &MusicWorld, language: &MusicalLanguage) -> Self {
        Self {
            home: Scale::new(world.tonic_pc, world.mode),
            sevenths: world.use_sevenths,
            colour: language.color_depth > 0,
            mixture: world.allow_modal_mixture,
        }
    }

    /// Whether a quality is a sixth or ninth colour (the language's colour law).
    pub fn is_colour(quality: Quality) -> bool {
        matches!(
            quality,
            Quality::Maj9
                | Quality::Min9
                | Quality::Dom9
                | Quality::Add9
                | Quality::Maj6
                | Quality::Min6
        )
    }

    /// Whether this vocabulary admits `chord`.
    pub fn admits(&self, chord: Chord) -> bool {
        (self.colour || !Self::is_colour(chord.quality))
            && (self.sevenths || chord.quality.intervals().len() <= 3)
            && (self.mixture
                || chord
                    .quality
                    .intervals()
                    .iter()
                    .all(|offset| self.home.contains_pc(chord.root_pc + offset)))
    }

    /// The retraction onto the admitted chords: `chord` itself when admitted, else the simplest
    /// admitted member of its triad family on the SAME root; `None` when that family has no
    /// admitted member there. Idempotent, root- and family-preserving.
    pub fn conform(&self, chord: Chord) -> Option<Chord> {
        if self.admits(chord) {
            return Some(chord);
        }
        QualityFamily::of(chord.quality)
            .members()
            .iter()
            .map(|&q| Chord::new(chord.root_pc, q))
            .find(|&c| self.admits(c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Quality; 18] = [
        Quality::Maj,
        Quality::Min,
        Quality::Dim,
        Quality::Aug,
        Quality::Maj7,
        Quality::Min7,
        Quality::Dom7,
        Quality::Min7b5,
        Quality::Dim7,
        Quality::MinMaj7,
        Quality::Sus4,
        Quality::Sus2,
        Quality::Maj9,
        Quality::Min9,
        Quality::Dom9,
        Quality::Add9,
        Quality::Maj6,
        Quality::Min6,
    ];

    fn vocabularies() -> Vec<HarmonicVocabulary> {
        let mut out = Vec::new();
        for world in MusicWorld::all() {
            for language in [
                MusicalLanguage::simple(),
                MusicalLanguage::fusion_conversation(),
            ] {
                out.push(HarmonicVocabulary::of(&world, &language));
            }
        }
        out
    }

    /// The retraction laws, exhaustively over every root, quality and declared vocabulary.
    #[test]
    fn conform_is_a_root_and_family_preserving_retraction() {
        for v in vocabularies() {
            for root in 0..12 {
                for q in ALL {
                    let c = Chord::new(root, q);
                    match v.conform(c) {
                        Some(r) => {
                            assert!(v.admits(r), "{v:?} {c:?} -> {r:?} not admitted");
                            assert_eq!(r.root_pc, c.root_pc, "a room never moves a root");
                            assert_eq!(QualityFamily::of(r.quality), QualityFamily::of(q));
                            assert_eq!(v.conform(r), Some(r), "idempotent");
                            assert_eq!(v.admits(c), r == c, "fixes exactly the admitted chords");
                        }
                        None => assert!(QualityFamily::of(q)
                            .members()
                            .iter()
                            .all(|&m| !v.admits(Chord::new(root, m)))),
                    }
                }
            }
        }
    }

    /// Every diatonic triad of a world's home is admitted in either language, so a chart made of
    /// scale degrees can always be sounded; only a borrowed root can be refused.
    #[test]
    fn diatonic_triads_are_always_admitted() {
        for world in MusicWorld::all() {
            for language in [
                MusicalLanguage::simple(),
                MusicalLanguage::fusion_conversation(),
            ] {
                let v = HarmonicVocabulary::of(&world, &language);
                for d in 0..7 {
                    let c = super::super::harmony::diatonic_chord(&v.home, d, false);
                    assert!(v.admits(c), "{} {d}: {c:?}", world.name);
                }
            }
        }
    }

    /// SWISS_SIGNAL declares no sevenths and no mixture: its family retraction of a borrowed bVI
    /// has no admitted member, while VAPOR95 admits it.
    #[test]
    fn a_borrowed_root_has_no_retraction_without_mixture() {
        let bvi = Chord::new(8, Quality::Maj);
        let fusion = MusicalLanguage::fusion_conversation();
        let swiss = HarmonicVocabulary::of(&MusicWorld::swiss_signal(), &fusion);
        assert_eq!(swiss.conform(bvi), None);
        assert_eq!(
            swiss.conform(Chord::new(0, Quality::Maj7)),
            Some(Chord::new(0, Quality::Maj))
        );
        let vapor = HarmonicVocabulary::of(&MusicWorld::vapor95(), &fusion);
        assert_eq!(vapor.conform(bvi), Some(bvi));
    }
}
