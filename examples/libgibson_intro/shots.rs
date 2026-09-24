//! The city's finite camera grammar. Shots select a realization, never advance
//! the four-agent job model. The short facade holds deliberately interrupt motion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotKind {
    EstablishCity,
    OrbitCity,
    ArchitectFacade,
    ScoutFacade,
    BuilderFacade,
    VerifyFacade,
    FollowContract,
    FollowGraph,
    FollowCandidate,
    FollowVerification,
    Ascent,
    PlanetReveal,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub kind: ShotKind,
    pub start: f32,
    pub end: f32,
    pub focal_agent: Option<usize>,
}
pub const SHOTS: [Shot; 12] = [
    Shot {
        kind: ShotKind::EstablishCity,
        start: 28.,
        end: 32.,
        focal_agent: None,
    },
    Shot {
        kind: ShotKind::OrbitCity,
        start: 32.,
        end: 36.,
        focal_agent: None,
    },
    Shot {
        kind: ShotKind::ArchitectFacade,
        start: 36.,
        end: 38.,
        focal_agent: Some(0),
    },
    Shot {
        kind: ShotKind::ScoutFacade,
        start: 38.,
        end: 40.,
        focal_agent: Some(1),
    },
    Shot {
        kind: ShotKind::BuilderFacade,
        start: 40.,
        end: 42.,
        focal_agent: Some(2),
    },
    Shot {
        kind: ShotKind::VerifyFacade,
        start: 42.,
        end: 44.,
        focal_agent: Some(3),
    },
    Shot {
        kind: ShotKind::FollowContract,
        start: 44.,
        end: 46.1,
        focal_agent: Some(0),
    },
    Shot {
        kind: ShotKind::FollowGraph,
        start: 46.1,
        end: 48.2,
        focal_agent: Some(1),
    },
    Shot {
        kind: ShotKind::FollowCandidate,
        start: 48.2,
        end: 50.5,
        focal_agent: Some(2),
    },
    Shot {
        kind: ShotKind::FollowVerification,
        start: 50.5,
        end: 53.,
        focal_agent: Some(3),
    },
    Shot {
        kind: ShotKind::Ascent,
        start: 53.,
        end: 56.,
        focal_agent: None,
    },
    Shot {
        kind: ShotKind::PlanetReveal,
        start: 56.,
        end: 60.,
        focal_agent: None,
    },
];
pub fn at(seconds: f32) -> Shot {
    SHOTS
        .iter()
        .copied()
        .find(|s| seconds < s.end)
        .unwrap_or(SHOTS[11])
}
pub fn facade(seconds: f32) -> Option<usize> {
    let shot = at(seconds);
    matches!(
        shot.kind,
        ShotKind::ArchitectFacade
            | ShotKind::ScoutFacade
            | ShotKind::BuilderFacade
            | ShotKind::VerifyFacade
    )
    .then_some(shot.focal_agent)
    .flatten()
}
