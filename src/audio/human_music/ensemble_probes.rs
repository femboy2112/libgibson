#![cfg(test)]
//! **Round VIII ensemble probes** — the band's VERTICAL claims, held on the flagship (the bounce
//! story, [`deflected_lift_trace`]`(120.0)`, forced DeflectedLift, seed 2112, every world) against
//! the negative control: [`EnsembleCoupling::Independent`], the R7b realization (each player
//! projecting the shared material alone), whose composition is pinned byte-for-byte below. Each
//! probe prints what it measured, so a failing run says exactly how far off it was.

use super::contract::CompositionGrammar;
use super::functor::{compose_full, Composition};
use super::performance::{EnsembleCoupling, PerformanceOptions};
use super::semantic::deflected_lift_trace;
use super::sonority::{ColorPolicy, EnsembleSonorityDiagnostics, MASKING_FLOOR_DB};
use super::theory::pitch_class;
use super::witness::audit;
use super::world::MusicWorld;

const SEED: u64 = 2112;

/// The six pitched role pairs (as [`EnsembleSonorityDiagnostics::role_pairs`] keys them).
const PITCHED_PAIRS: [&str; 6] = [
    "bass/keys",
    "bass/lead",
    "bass/pad",
    "keys/lead",
    "keys/pad",
    "lead/pad",
];

fn flagship(world: &MusicWorld, coupling: EnsembleCoupling) -> Composition {
    compose_full(
        &deflected_lift_trace(120.0),
        world,
        SEED,
        Some(CompositionGrammar::DeflectedLift),
        PerformanceOptions {
            coupling,
            ..PerformanceOptions::default()
        },
    )
}

/// FNV-1a over the composition a listener hears: every note (with its function and provenance),
/// drum hit, SFX event and chord, plus the melody repair counters. Not the R8 reports (the
/// Independent control leaves them empty by contract; that is asserted separately).
fn fingerprint(c: &Composition) -> u64 {
    let s = &c.score;
    let text = format!(
        "{:?}|{:?}|{:?}|{:?}|{}|{}",
        s.notes, s.drums, s.sfx, s.chords, s.melody_repairs, s.melody_rejudged
    );
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The Independent control IS the R7b composition. The pins were computed on the R7b tip
/// (4403b8e) with this same fingerprint and reproduced here; the coupling layer must never leak
/// into the control through a shared helper. A deliberate change to shared realization code
/// re-pins these — and says so in its commit.
#[test]
fn the_independent_control_is_the_r7b_composition() {
    let got: Vec<(String, u64, bool)> = MusicWorld::all()
        .iter()
        .map(|world| {
            let c = flagship(world, EnsembleCoupling::Independent);
            let quiet = c.score.vertical_decisions.is_empty() && c.score.support_report.is_none();
            (world.name.to_string(), fingerprint(&c), quiet)
        })
        .collect();
    for (name, fp, _) in &got {
        eprintln!("{name}: independent fingerprint {fp:#018x}");
    }
    for ((name, fp, quiet), pin) in got.iter().zip(R7B_PINS) {
        assert_eq!(*fp, pin, "{name}: the Independent control drifted from R7b");
        assert!(quiet, "{name}: the control makes no coupled decisions");
    }
}

/// `MusicWorld::all()` order: BLACK_ICE, VAPOR95, SWISS_SIGNAL.
const R7B_PINS: [u64; 3] = [
    0xb458_00e8_b7ec_8bb7,
    0xf987_f0cf_fd6b_0f4b,
    0xa3e6_0cd3_b8b2_86e0,
];

/// The Round VIII coupled realization the listen REJECTED ("trash": the bed revoiced itself to
/// satisfy the collision ruler), pinned at 5644c96 exactly as it was heard. It stays reachable as
/// the negative control; nothing in Round VIIIb may drift it.
#[test]
fn the_r8_control_is_the_composition_the_listen_rejected() {
    let got: Vec<u64> = MusicWorld::all()
        .iter()
        .map(|world| fingerprint(&flagship(world, EnsembleCoupling::CoupledR8)))
        .collect();
    for (world, fp) in MusicWorld::all().iter().zip(&got) {
        eprintln!("{}: r8 fingerprint {fp:#018x}", world.name);
    }
    for ((world, fp), pin) in MusicWorld::all().iter().zip(&got).zip(R8_PINS) {
        assert_eq!(*fp, pin, "{}: the R8 control drifted", world.name);
    }
}

/// The listen rejected Round VIII: the DEFAULT realization is the R7b band again, byte for byte, until
/// the ear approves another path.
#[test]
fn the_default_is_the_r7b_band_again() {
    assert_eq!(
        PerformanceOptions::default().coupling,
        EnsembleCoupling::Independent
    );
    for (world, pin) in MusicWorld::all().iter().zip(R7B_PINS) {
        let c = compose_full(
            &deflected_lift_trace(120.0),
            world,
            SEED,
            Some(CompositionGrammar::DeflectedLift),
            PerformanceOptions::default(),
        );
        assert_eq!(
            fingerprint(&c),
            pin,
            "{}: the default is not R7b",
            world.name
        );
    }
}

/// `MusicWorld::all()` order: BLACK_ICE, VAPOR95, SWISS_SIGNAL.
const R8_PINS: [u64; 3] = [
    0x1f97_14ee_5fea_d745,
    0xe6bb_ade7_2f60_1d07,
    0xef3d_6bdc_dd49_bd94,
];

/// The Round VIII result, per world: the control reproduces the listen's defect (lawful notes,
/// unowned union); the coupled band sounds one harmony — nominally AND at the audible (−20 dB
/// masking) lifetimes — keeps its floor, and pays nothing for it in receipts or melody repairs.
#[test]
fn the_coupled_band_sounds_one_harmony_where_the_control_did_not() {
    for world in MusicWorld::all() {
        let ind = flagship(&world, EnsembleCoupling::Independent);
        let cou = flagship(&world, EnsembleCoupling::CoupledR8);
        let policy = ColorPolicy::for_world(world.id, &cou.perf.language);
        let nominal = |c: &Composition| {
            EnsembleSonorityDiagnostics::measure(&c.score, &c.perf.contexts, &policy, &[])
        };
        let audible = |c: &Composition| {
            EnsembleSonorityDiagnostics::measure_audible_at(
                &c.score,
                &c.perf.contexts,
                &world,
                &policy,
                &[],
                MASKING_FLOOR_DB,
            )
        };
        let (di, dc) = (nominal(&ind), nominal(&cou));
        let (ai, ac) = (audible(&ind), audible(&cou));
        let pairs = |d: &EnsembleSonorityDiagnostics| -> Vec<usize> {
            PITCHED_PAIRS
                .iter()
                .map(|k| d.role_pairs.get(*k).copied().unwrap_or(0))
                .collect()
        };
        let r = audit(&cou.perf, &cou.score);
        eprintln!(
            "{}: nominal unowned m2/m9 {}/{} ({:.2} b) -> {}/{} ({:.2} b) | audible -20 dB {}/{} ({:.2} b) -> {}/{} ({:.2} b) | bass_function {} -> {} | pitched pairs {:?} -> {:?} | witnessed {}/{} repairs {}",
            world.name,
            di.unowned_m2,
            di.unowned_m9,
            di.unowned_beats,
            dc.unowned_m2,
            dc.unowned_m9,
            dc.unowned_beats,
            ai.unowned_m2,
            ai.unowned_m9,
            ai.unowned_beats,
            ac.unowned_m2,
            ac.unowned_m9,
            ac.unowned_beats,
            di.bass_function_violations,
            dc.bass_function_violations,
            pairs(&di),
            pairs(&dc),
            r.witnessed(),
            r.total(),
            cou.score.melody_repairs,
        );
        // The negative control reproduces the defect the listen heard.
        assert!(
            di.unowned_m2 + di.unowned_m9 >= 10,
            "{}: the control must carry the R7b union garbage",
            world.name
        );
        assert!(di.bass_function_violations > 0, "{}", world.name);
        // The coupled band: no pitched pair collides unowned; at most one residual (a sting).
        assert_eq!(pairs(&dc), vec![0; 6], "{}", world.name);
        assert!(
            dc.unowned_m2 + dc.unowned_m9 <= 1 && dc.unowned_beats <= 0.5,
            "{}",
            world.name
        );
        assert_eq!(dc.bass_function_violations, 0, "{}", world.name);
        // What actually rings: far less unowned collision than the control, under a beat.
        assert!(
            ac.unowned_beats < ai.unowned_beats / 4.0 && ac.unowned_beats <= 1.0,
            "{}",
            world.name
        );
        // No receipt or melody paid for it, and every decision says why.
        assert_eq!(r.witnessed(), r.total(), "{}", world.name);
        assert_eq!(cou.score.melody_repairs, 0, "{}", world.name);
        assert!(
            cou.score
                .vertical_decisions
                .iter()
                .all(|d| !d.what.is_empty() && !d.reason.is_empty()),
            "{}: an unexplained vertical decision",
            world.name
        );
    }
}

/// The joint pad+keys bed is not the two players' solo paths summed: on the SAME union cost, the
/// paths each player picks alone (the control the joint solve carries) collide unowned; the joint
/// choice does not, sheds no identity tone the solo pair kept, and keeps every edit witness.
#[test]
fn the_joint_bed_beats_each_player_alone_on_the_union() {
    for world in MusicWorld::all() {
        let c = flagship(&world, EnsembleCoupling::CoupledR8);
        let r = c
            .score
            .support_report
            .as_ref()
            .expect("the coupled realization reports its joint solve");
        eprintln!("{}: {}", world.name, r.report());
        assert!(r.independent.unowned > 0, "{}", world.name);
        assert_eq!(r.joint.unowned, 0, "{}", world.name);
        assert!(
            r.joint.identity_missing <= r.independent.identity_missing,
            "{}",
            world.name
        );
        assert_eq!(r.joint.witness_missing, 0, "{}", world.name);
        assert!(
            r.changed > 0 && r.changed <= r.steps,
            "{}: the joint solve must actually decide something",
            world.name
        );
    }
}

/// The coupled band must keep every receipt the control keeps, on inputs far beyond the flagship
/// (two stories, six lengths, eight seeds, two grammars, both languages, every world — 1152
/// compositions per arm). Run explicitly (`--ignored`, release): it prints every lost receipt and
/// the union totals, and fails on any loss.
#[test]
#[ignore = "fuzz sweep: run with --release -- --ignored"]
fn fuzz_the_coupled_band_keeps_every_receipt_the_control_keeps() {
    use super::language::MusicalLanguage;
    use super::semantic::demo_trace;
    let mut losses: Vec<String> = Vec::new();
    let (mut gains, mut runs) = (0usize, 0usize);
    let (mut unowned_ind, mut unowned_cou) = (0usize, 0usize);
    let (mut core_ind, mut core_cou) = (0f64, 0f64);
    for story in ["bounce", "demo"] {
        for beats in [24.0, 37.0, 48.0, 64.0, 96.0, 120.0] {
            let trace = match story {
                "bounce" => deflected_lift_trace(beats),
                _ => demo_trace(beats),
            };
            for seed in 0..8u64 {
                for grammar in [Some(CompositionGrammar::DeflectedLift), None] {
                    for simple in [false, true] {
                        for world in MusicWorld::all() {
                            let arm = |coupling| {
                                let language = if simple {
                                    MusicalLanguage::simple()
                                } else {
                                    MusicalLanguage::default()
                                };
                                compose_full(
                                    &trace,
                                    &world,
                                    seed,
                                    grammar,
                                    PerformanceOptions {
                                        coupling,
                                        language,
                                        ..PerformanceOptions::default()
                                    },
                                )
                            };
                            let (ind, cou) = (
                                arm(EnsembleCoupling::Independent),
                                arm(EnsembleCoupling::CoupledR8),
                            );
                            runs += 1;
                            let (ai, ac) =
                                (audit(&ind.perf, &ind.score), audit(&cou.perf, &cou.score));
                            for (wi, wc) in ai.rows.iter().zip(&ac.rows) {
                                assert_eq!(wi.action, wc.action, "one plan, two realizations");
                                if wi.witnessed && !wc.witnessed {
                                    losses.push(format!(
                                        "{story} {beats} seed {seed} {grammar:?} simple={simple} {}: {:?}@{} ({})",
                                        world.name, wc.kind, wc.action, wc.evidence
                                    ));
                                }
                                gains += usize::from(!wi.witnessed && wc.witnessed);
                            }
                            let policy = ColorPolicy::for_world(world.id, &cou.perf.language);
                            let di = EnsembleSonorityDiagnostics::measure(
                                &ind.score,
                                &ind.perf.contexts,
                                &policy,
                                &[],
                            );
                            let dc = EnsembleSonorityDiagnostics::measure(
                                &cou.score,
                                &cou.perf.contexts,
                                &policy,
                                &[],
                            );
                            let pitched = |d: &EnsembleSonorityDiagnostics| -> usize {
                                PITCHED_PAIRS
                                    .iter()
                                    .map(|k| d.role_pairs.get(*k).copied().unwrap_or(0))
                                    .sum()
                            };
                            unowned_ind += pitched(&di);
                            unowned_cou += pitched(&dc);
                            core_ind += di.missing_core_beats;
                            core_cou += dc.missing_core_beats;
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "fuzz: {runs} compositions per arm | receipts lost {} gained {gains} | unowned pitched-pair collisions {unowned_ind} -> {unowned_cou} | missing-core beats {core_ind:.1} -> {core_cou:.1}",
        losses.len()
    );
    for l in &losses {
        eprintln!("  LOST {l}");
    }
    assert!(losses.is_empty(), "{} receipts lost", losses.len());
}

/// The adversarial review's counterexamples, pinned (the fuzz above found them by the dozen): a
/// keys Hold cut at the change, a Thicken voiced thinner/lower by a complement, and a harmony whose
/// 3rd was credited to a keys voicing nobody plays. Each receipt the control keeps, the coupled
/// band keeps; the Dm under a Sustain bar's second harmony has its F.
#[test]
fn the_reviews_counterexamples_keep_their_receipts() {
    use super::ids::ActionId;
    use super::language::MusicalLanguage;
    use super::semantic::demo_trace;
    // (story, beats, seed, grammar, simple language, world index, action id)
    type Case = (
        &'static str,
        f64,
        u64,
        Option<CompositionGrammar>,
        bool,
        usize,
        u32,
    );
    let cases: [Case; 4] = [
        (
            "bounce",
            64.0,
            1,
            Some(CompositionGrammar::DeflectedLift),
            false,
            0,
            21,
        ), // Hold
        ("bounce", 24.0, 1, None, false, 0, 10), // Hold
        (
            "demo",
            37.0,
            2112,
            Some(CompositionGrammar::DeflectedLift),
            false,
            0,
            15,
        ), // Thicken
        (
            "bounce",
            64.0,
            1,
            Some(CompositionGrammar::DeflectedLift),
            true,
            2,
            7,
        ), // Thicken
    ];
    for (story, beats, seed, grammar, simple, wi, aid) in cases {
        let world = &MusicWorld::all()[wi];
        let trace = if story == "demo" {
            demo_trace(beats)
        } else {
            deflected_lift_trace(beats)
        };
        let arm = |coupling| {
            let language = if simple {
                MusicalLanguage::simple()
            } else {
                MusicalLanguage::default()
            };
            compose_full(
                &trace,
                world,
                seed,
                grammar,
                PerformanceOptions {
                    coupling,
                    language,
                    ..PerformanceOptions::default()
                },
            )
        };
        let (ind, cou) = (
            arm(EnsembleCoupling::Independent),
            arm(EnsembleCoupling::CoupledR8),
        );
        let row = |c: &Composition| {
            *audit(&c.perf, &c.score)
                .rows
                .iter()
                .find(|r| r.action == ActionId(aid))
                .expect("the action exists in both arms")
        };
        let (ri, rc) = (row(&ind), row(&cou));
        eprintln!(
            "{story} {beats} seed {seed} {}: {:?}@a{aid} control {} coupled {}",
            world.name, rc.kind, ri.witnessed, rc.witnessed
        );
        assert!(ri.witnessed, "the case is a control receipt");
        assert!(rc.witnessed, "{story} {beats} seed {seed} {}", world.name);
    }
    // SWISS_SIGNAL, bounce 96, seed 4: keys Sustain bar 18 strikes C at 72; Dm follows at 74. The
    // keys never play their Dm voicing, so the coupled band must carry Dm's 3rd (F) some other way.
    let c = compose_full(
        &deflected_lift_trace(96.0),
        &MusicWorld::swiss_signal(),
        4,
        None,
        PerformanceOptions {
            coupling: EnsembleCoupling::CoupledR8,
            ..PerformanceOptions::default()
        },
    );
    let f = c.score.notes.iter().any(|n| {
        pitch_class(n.pitch) == 5
            && n.start_beat < 76.0 - 1e-6
            && n.start_beat + n.dur_beats as f64 > 74.0 + 1e-6
    });
    assert!(f, "Dm's 3rd sounds under the Sustain bar's second harmony");
}

/// A sting's audible life is its OWN envelope (attack + decay + hold, then its release), not the
/// lead patch it is filed under: SWISS_SIGNAL's lead is a pluck (sustain 0), which would cut every
/// sting to a blip; VAPOR95's lead releases for 0.6 s, which would stretch them.
#[test]
fn a_stings_audible_life_is_its_own_envelope() {
    use super::instrument::Patch;
    use super::sonority::{audible_end_at, audible_voices, AUDIBLE_FLOOR_DB};
    let mut differs = 0;
    for world in MusicWorld::all() {
        let c = flagship(&world, EnsembleCoupling::CoupledR8);
        let s = &c.score;
        let voices = audible_voices(s, &c.perf.contexts, &world, AUDIBLE_FLOOR_DB);
        let bps = s.tempo_bpm as f64 / 60.0;
        for e in s.sfx.iter().filter(|e| e.is_pitched()) {
            let (a, d, _, _) = e.kind.envelope();
            let gated = (a + d + e.kind.hold_secs()) as f64 * bps;
            let own = Patch {
                adsr: e.kind.envelope(),
                ..world.lead
            };
            let want = audible_end_at(e.start_beat, gated, &own, s.tempo_bpm, AUDIBLE_FLOOR_DB);
            let as_lead = audible_end_at(
                e.start_beat,
                gated,
                &world.lead,
                s.tempo_bpm,
                AUDIBLE_FLOOR_DB,
            );
            for &p in &e.pitches {
                let v = voices
                    .iter()
                    .find(|v| v.sfx && v.pitch == p && (v.start - e.start_beat).abs() < 1e-9)
                    .expect("every pitched sting is a voice");
                assert!((v.end - want).abs() < 1e-9, "{}: {:?}", world.name, e.kind);
            }
            differs += usize::from((want - as_lead).abs() > 0.05);
        }
    }
    assert!(
        differs > 0,
        "the lead patch would have given different lifetimes"
    );
}
