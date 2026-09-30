//! Read-only classification of three already-failed fresh cases; never tunes the arm or holdout.
use gibson::audio::human_music::{
    comp,
    composer::Composer,
    functor::{
        perform_coherent, perform_expressive, perform_phrased, perform_pocketed, Composition,
    },
    performance::PerformanceOptions,
    score::{Note, Role},
    semantic::{deflected_lift_trace, demo_trace},
    temporal::TemporalPitchDiagnostics,
    voicing::keys_path,
    MusicWorld, SongMap,
};

fn neighborhood(c: &Composition, a: f64, b: f64) {
    for n in c
        .score
        .notes
        .iter()
        .filter(|n| n.role != Role::Pad && n.start_beat >= a && n.start_beat <= b)
    {
        println!("NOTE {n:?}");
    }
    for d in c
        .score
        .expression_decisions
        .iter()
        .filter(|d| d.before.note.start_beat >= a && d.before.note.start_beat <= b)
    {
        println!("DECISION {d:?}");
    }
    for p in c.score.phrase_plans.iter().filter(|p| {
        p.source
            .iter()
            .any(|n| n.start_beat >= a && n.start_beat <= b)
    }) {
        println!("PHRASE {p:?}");
    }
}
fn main() {
    for (name, story, composer, mut world, window) in [
        (
            "black_demo_meaning76",
            false,
            Composer::MeaningDirected,
            MusicWorld::black_ice(),
            None,
        ),
        (
            "swiss_demo_stable106",
            false,
            Composer::StablePropulsion,
            MusicWorld::swiss_signal(),
            Some((65.0, 69.0)),
        ),
        (
            "swiss_deflected_stable106",
            true,
            Composer::StablePropulsion,
            MusicWorld::swiss_signal(),
            Some((34.0, 38.0)),
        ),
    ] {
        world.tempo_bpm -= 12.0;
        let trace = if story {
            deflected_lift_trace(120.0)
        } else {
            demo_trace(96.0)
        };
        let song = SongMap::compose(&trace, 3, None, composer);
        let arms = [
            (
                "r14",
                perform_coherent(&song, &world, PerformanceOptions::default()),
            ),
            (
                "r15",
                perform_expressive(&song, &world, PerformanceOptions::default()),
            ),
            (
                "r16",
                perform_phrased(&song, &world, PerformanceOptions::default()),
            ),
            (
                "r17",
                perform_pocketed(&song, &world, PerformanceOptions::default()),
            ),
        ];
        println!(
            "CASE {name} seed=3 tempo={} song={:016x}",
            world.tempo_bpm,
            song.fingerprint()
        );
        let mut problem_window = window;
        for (arm, c) in &arms {
            let temporal = TemporalPitchDiagnostics::measure(&c.perf, &c.score);
            println!(
                "ARM {arm} score={:016x} false_function_claims={} stale_hearings={:?}",
                c.score.fingerprint(),
                temporal.false_function_claims,
                c.score.stale_hearings()
            );
            if name == "black_demo_meaning76" {
                let proof = temporal
                    .rows
                    .iter()
                    .find(|row| {
                        let n = &c.score.notes[row.note_index];
                        n.role == Role::Bass && n.start_beat == 62.0
                    })
                    .unwrap();
                println!("BASS62_PROOF {}", proof.detail);
            }
            for row in temporal.rows.iter().filter(|row| {
                c.score.notes[row.note_index]
                    .function
                    .is_some_and(|f| !row.supported.contains(&f))
            }) {
                println!(
                    "FALSE_CLAIM {}\nEXACT_NOTE {:?}",
                    row.detail, c.score.notes[row.note_index]
                );
                let beat = c.score.notes[row.note_index].start_beat;
                if window.is_none() {
                    problem_window = Some((beat - 2.0, beat + 2.0));
                }
            }
        }
        if let Some((a, b)) = problem_window {
            for (arm, c) in &arms {
                println!("NEIGHBORHOOD {arm} [{a},{b}]");
                neighborhood(c, a, b);
            }
        }
        if name == "swiss_demo_stable106" {
            let r16 = &arms[2].1;
            let r17 = &arms[3].1;
            let ownership = r17
                .score
                .occupancy
                .iter()
                .find(|o| o.role == Role::Lead)
                .unwrap();
            println!(
                "KEYS_SOURCE_INTERVENTION fixed perf/world/seed/occupancy, swap only final lead"
            );
            for (arm, c) in [("r16", r16), ("r17", r17)] {
                let lead: Vec<Note> = c.score.role_notes(Role::Lead).copied().collect();
                for at in [64.75, 65.25, 67.25, 67.75] {
                    println!(
                        "COMP_ELIGIBILITY lead={arm} beat={at} authored={:?} allowed={}",
                        ownership.opportunity(at),
                        ownership.allows_comp_at(at, &lead)
                    );
                }
                let keys = comp::realize_keys_owned(
                    &r17.perf, &song.plan, &world, &lead, ownership, song.seed,
                );
                for n in keys
                    .iter()
                    .filter(|n| n.start_beat >= 64.0 && n.start_beat <= 68.0)
                {
                    println!("REPLAY lead={arm} {n:?}");
                }
                for n in [3, 4] {
                    let kp = keys_path(&r17.perf, world.voicing_spread, &lead, n);
                    println!(
                        "KEYS_PATH lead={arm} voices={n} at67.25={:?}",
                        kp.voicing_at(&r17.perf, 67.25)
                    );
                }
            }
        }
    }
}
