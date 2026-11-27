#![cfg(feature = "figure")]
//! Finite declarative clips: validation, sampling, binding.

use std::time::Duration;

use gibson::figure::clip::{
    Channel, ChannelTarget, Clip, ClipError, Keys, MAX_CHANNELS, MAX_DURATION, MAX_KEYS_PER_CHANNEL,
};
use gibson::figure::math::{Quat, Vec3};

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3 { x, y, z }
}
fn q(x: f32, y: f32, z: f32, w: f32) -> Quat {
    Quat { x, y, z, w }
}
fn ident() -> Quat {
    q(0.0, 0.0, 0.0, 1.0)
}
fn trans(joint: &str, keys: Vec<(Duration, Vec3)>) -> Channel {
    Channel {
        target: ChannelTarget::JointTranslation(joint.into()),
        keys: Keys::Translation(keys),
    }
}
fn rot(joint: &str, keys: Vec<(Duration, Quat)>) -> Channel {
    Channel {
        target: ChannelTarget::JointRotation(joint.into()),
        keys: Keys::Rotation(keys),
    }
}
fn scale(joint: &str, keys: Vec<(Duration, Vec3)>) -> Channel {
    Channel {
        target: ChannelTarget::JointScale(joint.into()),
        keys: Keys::Scale(keys),
    }
}
fn weight(morph: &str, keys: Vec<(Duration, f32)>) -> Channel {
    Channel {
        target: ChannelTarget::MorphWeight(morph.into()),
        keys: Keys::Weight(keys),
    }
}

/// One clip using every channel kind. Duration 1000 ms; keys at 100 ms and 900 ms.
fn demo() -> Clip {
    Clip::new(
        "demo",
        ms(1000),
        vec![
            trans(
                "arm",
                vec![(ms(100), v(0.0, 0.0, 0.0)), (ms(900), v(8.0, 4.0, -2.0))],
            ),
            rot(
                "arm",
                vec![(ms(100), ident()), (ms(900), q(0.0, 0.0, 1.0, 0.0))],
            ),
            scale(
                "arm",
                vec![(ms(100), v(1.0, 1.0, 1.0)), (ms(900), v(3.0, 5.0, 1.0))],
            ),
            weight("smile", vec![(ms(100), 0.0), (ms(900), 1.0)]),
        ],
    )
    .expect("demo clip is valid")
}

#[test]
fn exact_values_at_keys() {
    let c = demo();
    let s = c.sample(ms(100));
    assert_eq!(s.translations, vec![("arm".to_string(), v(0.0, 0.0, 0.0))]);
    assert_eq!(s.rotations, vec![("arm".to_string(), ident())]);
    assert_eq!(s.scales, vec![("arm".to_string(), v(1.0, 1.0, 1.0))]);
    assert_eq!(s.weights, vec![("smile".to_string(), 0.0)]);
    let s = c.sample(ms(900));
    assert_eq!(s.translations[0].1, v(8.0, 4.0, -2.0));
    assert_eq!(s.scales[0].1, v(3.0, 5.0, 1.0));
    assert_eq!(s.weights[0].1, 1.0);
    assert!(!s.settled, "900 ms < 1000 ms duration");
}

#[test]
fn exact_midpoints() {
    let s = demo().sample(ms(500));
    assert_eq!(s.translations[0].1, v(4.0, 2.0, -1.0));
    assert_eq!(s.scales[0].1, v(2.0, 3.0, 1.0));
    assert_eq!(s.weights[0].1, 0.5);
    // nlerp between identity and a half-turn about z: (0,0,.5,.5) normalized.
    let r = s.rotations[0].1;
    let h = std::f32::consts::FRAC_1_SQRT_2;
    assert!((r.z - h).abs() < 1e-6 && (r.w - h).abs() < 1e-6, "{r:?}");
    assert!(r.x == 0.0 && r.y == 0.0);
}

#[test]
fn quarter_point_is_linear_in_time() {
    // 300 ms is a quarter of the way from 100 ms to 900 ms.
    let s = demo().sample(ms(300));
    assert_eq!(s.translations[0].1, v(2.0, 1.0, -0.5));
    assert_eq!(s.weights[0].1, 0.25);
}

#[test]
fn holds_before_first_and_after_last_key() {
    let c = demo();
    for t in [Duration::ZERO, ms(1), ms(99)] {
        let s = c.sample(t);
        assert_eq!(s.translations[0].1, v(0.0, 0.0, 0.0));
        assert_eq!(s.rotations[0].1, ident());
        assert_eq!(s.weights[0].1, 0.0);
    }
    for t in [ms(901), ms(999), ms(1000), ms(5000)] {
        let s = c.sample(t);
        assert_eq!(s.translations[0].1, v(8.0, 4.0, -2.0));
        assert_eq!(s.scales[0].1, v(3.0, 5.0, 1.0));
        assert_eq!(s.weights[0].1, 1.0);
        assert_eq!(s.rotations[0].1, q(0.0, 0.0, 1.0, 0.0));
    }
}

#[test]
fn settled_exactly_at_duration() {
    let c = demo();
    assert!(!c.sample(Duration::ZERO).settled);
    assert!(!c.sample(ms(999)).settled);
    assert!(!c.sample(Duration::from_nanos(999_999_999)).settled);
    assert!(c.sample(ms(1000)).settled);
    assert!(c.sample(Duration::from_nanos(1_000_000_001)).settled);
    assert!(c.sample(ms(1001)).settled);
}

#[test]
fn single_key_channel_is_constant() {
    let c = Clip::new("k", ms(10), vec![weight("w", vec![(ms(5), 0.75)])]).unwrap();
    for t in [0, 5, 6, 10, 99] {
        assert_eq!(c.sample(ms(t)).weights[0].1, 0.75);
    }
}

#[test]
fn key_at_zero_and_at_duration_are_allowed() {
    let c = Clip::new(
        "k",
        ms(10),
        vec![weight("w", vec![(ms(0), 0.0), (ms(10), 1.0)])],
    )
    .unwrap();
    assert_eq!(c.sample(ms(0)).weights[0].1, 0.0);
    assert_eq!(c.sample(ms(5)).weights[0].1, 0.5);
    assert_eq!(c.sample(ms(10)).weights[0].1, 1.0);
}

#[test]
fn quaternion_sign_fix_takes_the_short_way() {
    // b is a (nearly) identical rotation to a but in the opposite hemisphere: b = -a'.
    // Without the sign fix the midpoint would be near zero length / flip through the long arc.
    let a = q(0.0, 0.0, 0.0, 1.0);
    let b = q(0.0, 0.0, -0.6, -0.8); // dot(a, b) = -0.8 < 0; same rotation as (0,0,0.6,0.8)
    let c = Clip::new("s", ms(100), vec![rot("j", vec![(ms(0), a), (ms(100), b)])]).unwrap();
    let mid = c.sample(ms(50)).rotations[0].1;
    // Short way: blend of a and -b = (0,0,0.6,0.8) -> (0,0,0.3,0.9)/|.|.
    let n = (0.3f32 * 0.3 + 0.9 * 0.9).sqrt();
    let want = q(0.0, 0.0, 0.3 / n, 0.9 / n);
    assert!((mid.z - want.z).abs() < 1e-6, "{mid:?} vs {want:?}");
    assert!((mid.w - want.w).abs() < 1e-6, "{mid:?} vs {want:?}");
    assert!(mid.w > 0.0, "stays in a's hemisphere");
    let len = (mid.x * mid.x + mid.y * mid.y + mid.z * mid.z + mid.w * mid.w).sqrt();
    assert!((len - 1.0).abs() < 1e-6);
    // The held end is the authored key, unchanged.
    assert_eq!(c.sample(ms(100)).rotations[0].1, b);
}

#[test]
fn quaternion_same_hemisphere_unchanged_by_fix() {
    let a = ident();
    let b = q(0.0, 0.0, 0.6, 0.8);
    let c = Clip::new("s", ms(100), vec![rot("j", vec![(ms(0), a), (ms(100), b)])]).unwrap();
    let mid = c.sample(ms(50)).rotations[0].1;
    let n = (0.3f32 * 0.3 + 0.9 * 0.9).sqrt();
    assert!((mid.z - 0.3 / n).abs() < 1e-6 && (mid.w - 0.9 / n).abs() < 1e-6);
}

#[test]
fn blended_unit_ish_quaternions_are_renormalized() {
    // Keys of length 1.0005 are within the 1e-3 tolerance and are returned as authored,
    // but any blend between them is exactly unit length.
    let k = q(0.0, 0.0, 0.0, 1.0005);
    let c = Clip::new(
        "n",
        ms(10),
        vec![rot("j", vec![(ms(0), k), (ms(10), q(0.0, 0.6, 0.0, 0.8))])],
    )
    .unwrap();
    assert_eq!(c.sample(ms(0)).rotations[0].1, k);
    let r = c.sample(ms(3)).rotations[0].1;
    let len = (r.x * r.x + r.y * r.y + r.z * r.z + r.w * r.w).sqrt();
    assert!((len - 1.0).abs() < 1e-6, "{r:?}");
}

#[test]
fn every_clip_error_variant() {
    let ok = || weight("w", vec![(ms(0), 0.0)]);

    assert_eq!(
        Clip::new("e", Duration::ZERO, vec![ok()]),
        Err(ClipError::ZeroDuration)
    );

    let long = MAX_DURATION + Duration::from_nanos(1);
    assert_eq!(
        Clip::new("e", long, vec![ok()]),
        Err(ClipError::DurationTooLong { duration: long })
    );
    assert!(
        Clip::new("e", MAX_DURATION, vec![ok()]).is_ok(),
        "the cap itself is allowed"
    );

    let many: Vec<Channel> = (0..=MAX_CHANNELS)
        .map(|i| weight(&format!("w{i}"), vec![(ms(0), 0.0)]))
        .collect();
    assert_eq!(
        Clip::new("e", ms(10), many),
        Err(ClipError::TooManyChannels {
            count: MAX_CHANNELS + 1
        })
    );

    assert_eq!(
        Clip::new("e", ms(10), vec![ok(), weight("x", vec![])]),
        Err(ClipError::EmptyChannel { channel: 1 })
    );

    let keys: Vec<(Duration, f32)> = (0..=MAX_KEYS_PER_CHANNEL)
        .map(|i| (Duration::from_nanos(i as u64), 0.0))
        .collect();
    assert_eq!(
        Clip::new("e", ms(10), vec![weight("x", keys)]),
        Err(ClipError::TooManyKeys {
            channel: 0,
            count: MAX_KEYS_PER_CHANNEL + 1
        })
    );

    let mismatch = Channel {
        target: ChannelTarget::JointRotation("j".into()),
        keys: Keys::Weight(vec![(ms(0), 0.0)]),
    };
    assert_eq!(
        Clip::new("e", ms(10), vec![mismatch]),
        Err(ClipError::KeyTypeMismatch { channel: 0 })
    );

    assert_eq!(
        Clip::new("e", ms(10), vec![ok(), ok()]),
        Err(ClipError::DuplicateTarget { channel: 1 })
    );

    assert_eq!(
        Clip::new(
            "e",
            ms(10),
            vec![weight("x", vec![(ms(1), 0.0), (ms(1), 1.0)])]
        ),
        Err(ClipError::KeysNotIncreasing { channel: 0, key: 1 })
    );
    assert_eq!(
        Clip::new(
            "e",
            ms(10),
            vec![weight("x", vec![(ms(2), 0.0), (ms(1), 1.0)])]
        ),
        Err(ClipError::KeysNotIncreasing { channel: 0, key: 1 })
    );

    assert_eq!(
        Clip::new(
            "e",
            ms(10),
            vec![ok(), weight("x", vec![(ms(0), 0.0), (ms(11), 1.0)])]
        ),
        Err(ClipError::KeyBeyondDuration { channel: 1, key: 1 })
    );

    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            Clip::new(
                "e",
                ms(10),
                vec![weight("x", vec![(ms(0), 0.0), (ms(1), bad)])]
            ),
            Err(ClipError::NonFiniteValue { channel: 0, key: 1 })
        );
        assert_eq!(
            Clip::new(
                "e",
                ms(10),
                vec![trans("x", vec![(ms(0), v(0.0, bad, 0.0))])]
            ),
            Err(ClipError::NonFiniteValue { channel: 0, key: 0 })
        );
        assert_eq!(
            Clip::new(
                "e",
                ms(10),
                vec![scale("x", vec![(ms(0), v(bad, 0.0, 0.0))])]
            ),
            Err(ClipError::NonFiniteValue { channel: 0, key: 0 })
        );
        assert_eq!(
            Clip::new(
                "e",
                ms(10),
                vec![rot("x", vec![(ms(0), q(0.0, 0.0, 0.0, bad))])]
            ),
            Err(ClipError::NonFiniteValue { channel: 0, key: 0 })
        );
    }

    for bad in [
        q(0.0, 0.0, 0.0, 0.0),
        q(0.0, 0.0, 0.0, 1.01),
        q(0.0, 0.0, 0.0, 0.99),
        q(1.0, 1.0, 0.0, 0.0),
    ] {
        assert_eq!(
            Clip::new("e", ms(10), vec![rot("x", vec![(ms(0), bad)])]),
            Err(ClipError::NonUnitQuaternion { channel: 0, key: 0 })
        );
    }

    assert_eq!(
        demo().bind(&["leg"], &["smile"]),
        Err(ClipError::UnknownJoint { name: "arm".into() })
    );
    assert_eq!(
        demo().bind(&["arm"], &["frown"]),
        Err(ClipError::UnknownMorph {
            name: "smile".into()
        })
    );
}

#[test]
fn errors_display_and_are_std_errors() {
    let e = Clip::new("e", Duration::ZERO, vec![]).unwrap_err();
    assert!(!e.to_string().is_empty());
    let b: Box<dyn std::error::Error> = Box::new(e);
    assert!(b.to_string().contains("zero"));
    let u = ClipError::UnknownJoint { name: "zz".into() };
    assert!(u.to_string().contains("zz"));
}

#[test]
fn clip_with_no_channels_is_valid_and_settles() {
    let c = Clip::new("empty", ms(10), vec![]).unwrap();
    assert!(!c.sample(ms(9)).settled);
    assert!(c.sample(ms(10)).settled);
    assert!(c.sample(ms(9)).translations.is_empty());
}

#[test]
fn bind_resolves_indices_and_samples_match_named_sample() {
    let c = demo();
    let b = c
        .bind(&["root", "leg", "arm"], &["blink", "smile"])
        .unwrap();
    assert_eq!(b.duration(), ms(1000));
    for t in [0u64, 50, 100, 300, 500, 900, 1000, 4000] {
        let named = c.sample(ms(t));
        let idx = b.sample(ms(t));
        assert_eq!(idx.settled, named.settled);
        assert_eq!(idx.translations, vec![(2, named.translations[0].1)]);
        assert_eq!(idx.rotations, vec![(2, named.rotations[0].1)]);
        assert_eq!(idx.scales, vec![(2, named.scales[0].1)]);
        assert_eq!(idx.weights, vec![(1, named.weights[0].1)]);
    }
}

#[test]
fn bind_first_duplicate_name_wins() {
    let c = Clip::new("d", ms(10), vec![weight("w", vec![(ms(0), 1.0)])]).unwrap();
    let b = c.bind(&[], &["a", "w", "w"]).unwrap();
    assert_eq!(b.sample(ms(0)).weights, vec![(1, 1.0)]);
}

#[test]
fn unknown_target_fails_at_bind_not_at_sample() {
    let c = Clip::new(
        "u",
        ms(10),
        vec![scale("ghost", vec![(ms(0), v(1.0, 1.0, 1.0))])],
    )
    .unwrap();
    // Sampling a never-bound clip is fine: it is named data.
    assert_eq!(c.sample(ms(0)).scales.len(), 1);
    assert_eq!(
        c.bind(&["root"], &[]),
        Err(ClipError::UnknownJoint {
            name: "ghost".into()
        })
    );
    // A morph name does not satisfy a joint target and vice versa.
    assert!(c.bind(&[], &["ghost"]).is_err());
}

#[test]
fn deterministic_across_calls_and_clones() {
    let c = demo();
    let c2 = c.clone();
    for t in 0..=1100u64 {
        let a = c.sample(ms(t));
        assert_eq!(a, c.sample(ms(t)));
        assert_eq!(a, c2.sample(ms(t)));
    }
    let b = c.bind(&["arm"], &["smile"]).unwrap();
    assert_eq!(b.sample(ms(321)), b.sample(ms(321)));
    // Sample order does not matter (no hidden state).
    let late = c.sample(ms(800));
    let _ = c.sample(ms(0));
    assert_eq!(late, c.sample(ms(800)));
}

#[test]
fn huge_time_holds_and_settles_without_panic() {
    let c = demo();
    let s = c.sample(Duration::MAX);
    assert!(s.settled);
    assert_eq!(s.translations[0].1, v(8.0, 4.0, -2.0));
    assert_eq!(s.weights[0].1, 1.0);
    let b = c.bind(&["arm"], &["smile"]).unwrap();
    let bs = b.sample(Duration::MAX);
    assert!(bs.settled);
    assert_eq!(bs.weights, vec![(0, 1.0)]);
    // Near-overflow in nanoseconds is also fine.
    assert!(c.sample(Duration::new(u64::MAX, 999_999_999)).settled);
}

#[test]
fn accessors_expose_validated_data() {
    let c = demo();
    assert_eq!(c.name(), "demo");
    assert_eq!(c.duration(), ms(1000));
    assert_eq!(c.channels().len(), 4);
}
