//! Isolated cover laws: one variable changed per test. kappa_I(P') = kappa_I(P0) is the law;
//! a red law is a finding and stays red (see the `ignore` reasons), it is never tuned away.
use gibson::audio::human_music::{
    contract::CompositionGrammar,
    cover::{
        cover_candidate, CoverAxis, CoverConformance, CoverError, CoverFreedom, CoverMap,
        CoverSpec, CoverTarget,
    },
    fingerprint::CanonicalFingerprint,
    functor::{perform_pocketed, Composition},
    language::MusicalLanguage,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    reference_song::ReferenceSong,
    score::{DrumVoice, Role},
    semantic::demo_trace,
    song::SongMap,
    MusicWorld,
};

const ODE: &str = include_str!("../docs/fixtures/humanmusic-cover/ode-import/reference.tsv");

fn ode_map() -> CoverMap {
    ReferenceSong::from_tsv(ODE, "sop")
        .unwrap()
        .extract(CoverSpec::new([CoverAxis::Motif]))
        .unwrap()
}

fn generated_source(world: &MusicWorld) -> Composition {
    let song = SongMap::build(&demo_trace(64.0), 2112, Some(CompositionGrammar::HookArc));
    perform_pocketed(&song, world, PerformanceOptions::default())
}

fn contract_map(world: &MusicWorld) -> (Composition, CoverSpec, CoverMap) {
    let source = generated_source(world);
    let spec = CoverSpec::from_contract(&source.song.plan.contract);
    let map = CoverMap::extract(&source, world, spec.clone()).unwrap();
    (source, spec, map)
}

fn target_with<'a>(
    world: &'a MusicWorld,
    seed: u64,
    options: PerformanceOptions,
) -> CoverTarget<'a> {
    CoverTarget {
        world,
        seed,
        grammar: CompositionGrammar::HookArc,
        options,
        profile: PerformanceProfile::POCKET,
    }
}

fn target(world: &MusicWorld, seed: u64) -> CoverTarget<'_> {
    target_with(world, seed, PerformanceOptions::default())
}

fn lift_checked(map: &CoverMap, world: &MusicWorld, seed: u64) -> Composition {
    let c = cover_candidate(map, target(world, seed)).expect("cover_candidate refused");
    let law = CoverConformance::check(map, &c, world);
    assert!(law.passes(), "{}", law.report());
    c
}

fn lead_pitches(c: &Composition) -> Vec<i32> {
    let mut n: Vec<_> = c.score.role_notes(Role::Lead).collect();
    n.sort_by(|a, b| {
        a.start_beat
            .total_cmp(&b.start_beat)
            .then(a.pitch.cmp(&b.pitch))
    });
    n.iter().map(|n| n.pitch).collect()
}

fn evidence(c: &Composition) -> String {
    format!("{:?}", (&c.song, &c.perf, &c.score))
}

fn copy(c: &Composition) -> Composition {
    Composition {
        score: c.score.clone(),
        song: c.song.clone(),
        perf: c.perf.clone(),
    }
}

fn tempo_law(map: &CoverMap, spec: &CoverSpec, base: &MusicWorld) {
    let mut a = base.clone();
    let mut b = base.clone();
    a.tempo_bpm = 95.0;
    b.tempo_bpm = 131.0;
    let ca = lift_checked(map, &a, 7);
    let cb = lift_checked(map, &b, 7);
    let ma = CoverMap::extract(&ca, &a, spec.clone()).unwrap();
    let mb = CoverMap::extract(&cb, &b, spec.clone()).unwrap();
    assert_eq!(ma.length, mb.length);
    assert_eq!(ma.length, map.length);
    assert_eq!(ca.score.total_beats, cb.score.total_beats);
    assert_eq!(ma.motif, mb.motif);
    assert_eq!(
        ma.motif.as_ref().unwrap().canonical_fingerprint(),
        mb.motif.as_ref().unwrap().canonical_fingerprint()
    );
    assert_eq!(ma.canonical_fingerprint(), mb.canonical_fingerprint());
    assert_eq!(ma.motif, map.motif);
}

#[test]
fn tempo_only_change_preserves_metric_cover_identity() {
    let world = MusicWorld::black_ice();
    let (_, spec, map) = contract_map(&world);
    tempo_law(&map, &spec, &world);
    tempo_law(&ode_map(), &CoverSpec::new([CoverAxis::Motif]), &world);
}

fn transposition_law(map: &CoverMap, spec: &CoverSpec, base: &MusicWorld) {
    let mut up = base.clone();
    up.tonic_pc = (base.tonic_pc + 5).rem_euclid(12);
    let c0 = lift_checked(map, base, 7);
    let c1 = lift_checked(map, &up, 7);
    let m0 = CoverMap::extract(&c0, base, spec.clone()).unwrap();
    let m1 = CoverMap::extract(&c1, &up, spec.clone()).unwrap();
    assert_eq!(m0.motif, m1.motif);
    assert_eq!(m0.harmony, m1.harmony);
    assert_eq!(m0.canonical_fingerprint(), m1.canonical_fingerprint());
    assert_ne!(
        lead_pitches(&c0),
        lead_pitches(&c1),
        "absolute pitches did not move"
    );
}

#[test]
fn transposition_only_preserves_relative_harmony_and_motif() {
    let world = MusicWorld::black_ice();
    let (_, _, map) = contract_map(&world);
    let spec = CoverSpec::new([CoverAxis::HarmonicContour, CoverAxis::Motif]);
    let mut m = map.clone();
    m.spec = spec.clone();
    m.groove = None;
    m.form = None;
    m.validate().unwrap();
    transposition_law(&m, &spec, &world);
}

#[test]
fn language_only_change_is_either_conformant_or_an_explicit_refusal() {
    let world = MusicWorld::black_ice();
    let (_, _, generated) = contract_map(&world);
    let ode = ode_map();
    assert_ne!(
        format!("{:?}", MusicalLanguage::simple()),
        format!("{:?}", MusicalLanguage::default())
    );
    for (name, map, must_be_ok) in [
        ("ode-motif", &ode, true),
        ("generated-default", &generated, false),
    ] {
        for (lname, lang) in [
            ("simple", MusicalLanguage::simple()),
            ("default", MusicalLanguage::default()),
        ] {
            let o = PerformanceOptions {
                language: lang,
                ..PerformanceOptions::default()
            };
            match cover_candidate(map, target_with(&world, 7, o)) {
                Ok(c) => {
                    let law = CoverConformance::check(map, &c, &world);
                    assert!(
                        law.passes(),
                        "{name}/{lname}: Ok but nonconformant\n{}",
                        law.report()
                    );
                    eprintln!("{name}/{lname}: OK conformant");
                }
                Err(CoverError::Invalid(msg)) => {
                    assert!(!must_be_ok, "{name}/{lname}: unexpected refusal {msg}");
                    assert!(!msg.is_empty());
                    eprintln!("{name}/{lname}: REFUSAL Invalid({msg:?})");
                }
                Err(e) => panic!("{name}/{lname}: non-explicit error {e:?}"),
            }
        }
    }
}

#[test]
fn contract_default_noninterference() {
    let w0 = MusicWorld::black_ice();
    let (p0, spec, map0) = contract_map(&w0);
    // Contract-default reads: Lead notes (motif), kick/snare (groove), chords, form/sections,
    // total length. Perturb only Pad/Keys/Bass pitch+velocity, Lead velocity, hat/clap velocity.
    let mut p1 = copy(&p0);
    for n in p1.score.notes.iter_mut() {
        match n.role {
            Role::Pad | Role::Keys => {
                n.pitch += 12;
                n.velocity *= 0.5;
            }
            Role::Bass => {
                n.pitch += 12;
                n.velocity *= 0.7;
            }
            Role::Lead => n.velocity *= 0.8,
        }
    }
    let mut hats = 0;
    for d in p1.score.drums.iter_mut() {
        if matches!(
            d.voice,
            DrumVoice::ClosedHat | DrumVoice::OpenHat | DrumVoice::Clap
        ) {
            d.velocity *= 0.5;
            hats += 1;
        }
    }
    assert!(hats > 0, "no free drum hits to perturb");
    assert!(p1
        .score
        .notes
        .iter()
        .zip(&p0.score.notes)
        .any(|(a, b)| a.pitch != b.pitch));
    let map1 = CoverMap::extract(&p1, &w0, spec.clone()).unwrap();
    assert_eq!(map0.canonical_fingerprint(), map1.canonical_fingerprint());
    assert_eq!(format!("{map0:?}"), format!("{map1:?}"));

    let mut w1 = MusicWorld::swiss_signal();
    w1.tonic_pc = w0.tonic_pc;
    let mut oks = 0;
    for seed in [3u64, 4] {
        for w in [&w0, &w1] {
            let a = cover_candidate(&map0, target(w, seed));
            let b = cover_candidate(&map1, target(w, seed));
            match (a, b) {
                (Ok(a), Ok(b)) => {
                    assert_eq!(evidence(&a), evidence(&b), "seed {seed} world {}", w.name);
                    assert_eq!(
                        a.score.canonical_fingerprint(),
                        b.score.canonical_fingerprint()
                    );
                    assert_eq!(
                        a.perf.canonical_fingerprint(),
                        b.perf.canonical_fingerprint()
                    );
                    assert_eq!(
                        a.song.canonical_fingerprint(),
                        b.song.canonical_fingerprint()
                    );
                    oks += 1;
                }
                (Err(a), Err(b)) => {
                    // An explicit refusal is lawful, but it must not depend on the free data.
                    assert_eq!(a, b);
                    eprintln!("seed {seed} world {}: identical refusal {a:?}", w.name);
                }
                (a, b) => panic!(
                    "seed {seed} world {}: outcomes diverge {:?} vs {:?}",
                    w.name,
                    a.map(|_| "Ok"),
                    b.map(|_| "Ok")
                ),
            }
        }
    }
    assert!(
        oks >= 1,
        "no cover was produced; the identity check is vacuous"
    );

    // Positive controls: perturbing a pinned coordinate must move the map.
    let mut p2 = copy(&p0);
    let last_lead = p2
        .score
        .notes
        .iter_mut()
        .filter(|n| n.role == Role::Lead)
        .max_by(|a, b| a.start_beat.total_cmp(&b.start_beat))
        .unwrap();
    last_lead.pitch += 1;
    let m2 = CoverMap::extract(&p2, &w0, spec.clone()).unwrap();
    assert_ne!(map0.canonical_fingerprint(), m2.canonical_fingerprint());
    assert_ne!(map0.motif, m2.motif);

    let mut p3 = copy(&p0);
    let kick = p3
        .score
        .drums
        .iter_mut()
        .find(|d| d.voice == DrumVoice::Kick)
        .unwrap();
    kick.voice = DrumVoice::Snare;
    let m3 = CoverMap::extract(&p3, &w0, spec.clone()).unwrap();
    assert_ne!(map0.canonical_fingerprint(), m3.canonical_fingerprint());
    assert_ne!(map0.groove, m3.groove);

    let mut p4 = copy(&p0);
    p4.score.chords[0].chord.root_pc = (p4.score.chords[0].chord.root_pc + 1) % 12;
    let m4 = CoverMap::extract(&p4, &w0, spec).unwrap();
    assert_ne!(map0.canonical_fingerprint(), m4.canonical_fingerprint());
    assert_ne!(map0.harmony, m4.harmony);
}

#[test]
fn every_free_group_varies_somewhere_in_a_small_sweep() {
    let map = ode_map();
    let spec = CoverSpec::new([CoverAxis::Motif]);
    let mut covers = Vec::new();
    for seed in [901u64, 902, 903] {
        for world in [
            MusicWorld::black_ice(),
            MusicWorld::swiss_signal(),
            MusicWorld::vapor95(),
        ] {
            let c = cover_candidate(&map, target(&world, seed)).expect("cover refused");
            let law = CoverConformance::check(&map, &c, &world);
            assert!(law.passes(), "seed {seed} {}: {}", world.name, law.report());
            assert_eq!(
                CoverMap::extract(&c, &world, spec.clone()).unwrap().motif,
                map.motif
            );
            covers.push(c);
        }
    }
    let (mut support, mut dynamics, mut percussion, mut interactions) =
        (false, false, false, false);
    for i in 0..covers.len() {
        for j in (i + 1)..covers.len() {
            let f = CoverFreedom::compare(&covers[i], &covers[j]);
            support |= f.support_voicing;
            dynamics |= f.dynamics;
            percussion |= f.percussion_detail;
            interactions |= f.interactions;
        }
    }
    assert!(support, "support_voicing never varied");
    assert!(dynamics, "dynamics never varied");
    assert!(percussion, "percussion_detail never varied");
    assert!(interactions, "interactions never varied");
}
