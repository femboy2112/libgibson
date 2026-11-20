//! v0.9.0 terminal capability probe: RED slice (design in docs/design/v0.9-capability-probe.md).
//!
//! Everything here runs on byte strings. Nothing writes to a terminal, reads from one, or reads
//! the process environment: the environment is an injected closure.
//!
//! `red_*` tests fail against the stubs in `gibson::capability::probe` until the GREEN slice.
//! `guard_*` tests pass today and must keep passing.

use gibson::capability::probe::{
    decide, parse_da1_attributes, parse_kitty_graphics_reply, parse_xtsmgraphics,
    probe_request_bytes, GlyphTier, ProbeEvidence, ProbeReason, XtsmStatus,
};
use gibson::{Capability, TerminalCapabilities};

// ---- fixtures -----------------------------------------------------------------------------

/// VTE 0.76 (mate-terminal): measured. No attribute 4.
const VTE_DA1: &[u8] = b"\x1b[?61;1;21;22c";
/// A sixel-capable xterm-style reply: synthetic, attribute 4 present.
const XTERM_SIXEL_DA1: &[u8] = b"\x1b[?64;1;2;4;6;9;15;21;22c";
/// kitty answers with a trailing semicolon: recalled, not measured here.
const KITTY_DA1: &[u8] = b"\x1b[?62;c";

const KITTY_OK: &[u8] = b"\x1b_Gi=31;OK\x1b\\";
const KITTY_ERR: &[u8] = b"\x1b_Gi=31;ENOTSUPPORTED:no graphics here\x1b\\";

const XTSM_GEOMETRY_OK: &[u8] = b"\x1b[?2;0;1920;1080S";
const XTSM_COLORS_OK: &[u8] = b"\x1b[?1;0;256S";
/// VTE 0.76: status 1.
const XTSM_GEOMETRY_ERR: &[u8] = b"\x1b[?2;1;0S";
const XTSM_COLORS_ERR: &[u8] = b"\x1b[?1;1;0S";
const XTSM_GEOMETRY_ZERO: &[u8] = b"\x1b[?2;0;0;0S";

fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |k| {
        pairs
            .iter()
            .find(|(name, _)| *name == k)
            .map(|(_, v)| (*v).to_string())
    }
}

fn vte076_evidence() -> ProbeEvidence {
    let mut e = ProbeEvidence::default();
    e.da1 = parse_da1_attributes(VTE_DA1);
    e.xtsm_geometry = parse_xtsmgraphics(XTSM_GEOMETRY_ERR);
    e.xtsm_colors = parse_xtsmgraphics(XTSM_COLORS_ERR);
    e
}

fn kitty_evidence() -> ProbeEvidence {
    let mut e = ProbeEvidence::default();
    e.kitty = parse_kitty_graphics_reply(KITTY_OK);
    e.da1 = parse_da1_attributes(KITTY_DA1);
    e
}

fn sixel_evidence(da1: &[u8], geometry: &[u8], colors: &[u8]) -> ProbeEvidence {
    let mut e = ProbeEvidence::default();
    e.da1 = parse_da1_attributes(da1);
    e.xtsm_geometry = parse_xtsmgraphics(geometry);
    e.xtsm_colors = parse_xtsmgraphics(colors);
    e
}

const KITTY_ENV: &[(&str, &str)] = &[("TERM", "xterm-kitty"), ("KITTY_WINDOW_ID", "1")];
const VTE76_ENV: &[(&str, &str)] = &[("TERM", "xterm-256color"), ("VTE_VERSION", "7600")];
const PLAIN_ENV: &[(&str, &str)] = &[("TERM", "xterm-256color")];

// ---- DA1 attribute list -------------------------------------------------------------------

#[test]
fn red_da1_vte076_attributes() {
    assert_eq!(parse_da1_attributes(VTE_DA1), Some(vec![61, 1, 21, 22]));
}

#[test]
fn red_da1_xterm_sixel_has_attribute_4() {
    let attrs = parse_da1_attributes(XTERM_SIXEL_DA1).expect("a primary reply parses");
    assert_eq!(attrs[0], 64);
    assert!(attrs.contains(&4), "{attrs:?}");
}

#[test]
fn red_da1_kitty_trailing_semicolon_is_ignored() {
    assert_eq!(parse_da1_attributes(KITTY_DA1), Some(vec![62]));
}

#[test]
fn red_da1_rejects_what_is_not_a_primary_reply() {
    // The positive case keeps this test RED against a stub that returns None for everything.
    assert_eq!(parse_da1_attributes(VTE_DA1), Some(vec![61, 1, 21, 22]));
    assert_eq!(
        parse_da1_attributes(b"\x1b[>1;7600;0c"),
        None,
        "secondary DA"
    );
    assert_eq!(parse_da1_attributes(b"\x1b[?c"), None, "no attributes");
    assert_eq!(parse_da1_attributes(b"\x1b[?61;;22c"), None, "empty middle");
    assert_eq!(parse_da1_attributes(b"\x1b[?61;x;22c"), None, "non-digit");
    assert_eq!(parse_da1_attributes(b"\x1b[?99999c"), None, "above u16");
    assert_eq!(parse_da1_attributes(b"\x1b[?61;1"), None, "unterminated");
    assert_eq!(
        parse_da1_attributes(b"\x1b[61;1c"),
        None,
        "no question mark"
    );
    assert_eq!(parse_da1_attributes(b""), None);
}

// ---- kitty graphics reply -----------------------------------------------------------------

#[test]
fn red_kitty_ok_reply() {
    let r = parse_kitty_graphics_reply(KITTY_OK).expect("OK reply parses");
    assert_eq!(r.id, Some(31));
    assert!(r.ok);
    assert_eq!(r.message, "OK");
}

#[test]
fn red_kitty_error_reply() {
    let r = parse_kitty_graphics_reply(KITTY_ERR).expect("an error reply still parses");
    assert_eq!(r.id, Some(31));
    assert!(!r.ok);
    assert_eq!(r.message, "ENOTSUPPORTED:no graphics here");
}

#[test]
fn red_kitty_reply_rejects_what_is_not_one() {
    assert!(parse_kitty_graphics_reply(KITTY_OK).is_some());
    assert_eq!(parse_kitty_graphics_reply(b"\x1b_Gi=31;OK"), None, "no ST");
    assert_eq!(
        parse_kitty_graphics_reply(b"\x1b_Xi=31;OK\x1b\\"),
        None,
        "not G"
    );
    assert_eq!(
        parse_kitty_graphics_reply(b"\x1b_Gi=31\x1b\\"),
        None,
        "no message"
    );
    assert_eq!(parse_kitty_graphics_reply(b"i=31;OK"), None, "no APC");
    assert_eq!(parse_kitty_graphics_reply(b""), None);
}

// ---- XTSMGRAPHICS reply -------------------------------------------------------------------

#[test]
fn red_xtsm_geometry_success() {
    let r = parse_xtsmgraphics(XTSM_GEOMETRY_OK).expect("success parses");
    assert_eq!(r.item, 2);
    assert_eq!(r.status, XtsmStatus::Success);
    assert!(r.is_success());
    assert_eq!(r.values, vec![1920, 1080]);
}

#[test]
fn red_xtsm_colors_success() {
    let r = parse_xtsmgraphics(XTSM_COLORS_OK).expect("success parses");
    assert_eq!(r.item, 1);
    assert!(r.is_success());
    assert_eq!(r.values, vec![256]);
}

#[test]
fn red_xtsm_vte_error_status_is_not_success() {
    let r = parse_xtsmgraphics(XTSM_GEOMETRY_ERR).expect("an error reply still parses");
    assert_eq!(r.item, 2);
    assert_eq!(r.status, XtsmStatus::ErrorInItem);
    assert!(!r.is_success());
}

#[test]
fn red_xtsm_rejects_what_is_not_one() {
    assert!(parse_xtsmgraphics(XTSM_GEOMETRY_OK).is_some());
    assert_eq!(
        parse_xtsmgraphics(b"\x1b[2;0;1920;1080S"),
        None,
        "no question mark"
    );
    assert_eq!(
        parse_xtsmgraphics(b"\x1b[?2;0S"),
        None,
        "too few parameters"
    );
    assert_eq!(
        parse_xtsmgraphics(b"\x1b[?2;0;1920;1080"),
        None,
        "unterminated"
    );
    assert_eq!(parse_xtsmgraphics(b"\x1b[?2;x;1;1S"), None, "non-digit");
    assert_eq!(parse_xtsmgraphics(b""), None);
}

// ---- the request ---------------------------------------------------------------------------

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

#[test]
fn red_request_ends_with_da1_sentinel() {
    let req = probe_request_bytes();
    assert!(req.ends_with(b"\x1b[c"), "{req:?}");
    let da1_count = req.windows(3).filter(|w| *w == b"\x1b[c").count();
    assert_eq!(da1_count, 1, "DA1 is written exactly once, last");
}

#[test]
fn red_request_kitty_query_precedes_da1() {
    let req = probe_request_bytes();
    let kitty = find(req, b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\").expect("kitty query");
    let da1 = find(req, b"\x1b[c").expect("DA1");
    assert!(kitty < da1);
}

#[test]
fn red_request_reads_xtsmgraphics_before_da1() {
    let req = probe_request_bytes();
    let da1 = find(req, b"\x1b[c").expect("DA1");
    let geometry = find(req, b"\x1b[?2;1;0S").expect("geometry read");
    let colors = find(req, b"\x1b[?1;1;0S").expect("colour register read");
    assert!(geometry < da1 && colors < da1);
}

#[test]
fn red_request_is_non_destructive() {
    let req = probe_request_bytes();
    assert!(!req.is_empty());
    // Every CSI in the request is a query: final byte `S` (XTSMGRAPHICS) or `c` (DA1). No mode
    // set or reset (`h`/`l`), no keyboard push (`u`), no cursor movement or erase.
    let mut i = 0;
    while i < req.len() {
        if req[i] == 0x1b && req.get(i + 1) == Some(&b'[') {
            let end = req[i + 2..]
                .iter()
                .position(|b| (0x40..=0x7e).contains(b))
                .expect("a CSI has a final byte");
            let fin = req[i + 2 + end];
            assert!(
                fin == b'S' || fin == b'c',
                "unexpected CSI final {:?}",
                fin as char
            );
            i += 2 + end + 1;
        } else {
            i += 1;
        }
    }
}

// ---- the decision table: sixel, kitty graphics, glyph tier ----------------------------------

#[test]
fn red_decide_vte076_is_sextant_with_no_pixels() {
    let v = decide(&vte076_evidence(), env(VTE76_ENV));
    assert_eq!(v.sixel.state, Capability::Unsupported);
    assert_eq!(v.sixel.reason, ProbeReason::XtsmError);
    assert_eq!(v.kitty_graphics.state, Capability::Unsupported);
    assert_eq!(v.kitty_graphics.reason, ProbeReason::NoReplyBeforeDa1);
    assert_eq!(v.glyph_tier, GlyphTier::Sextant);
    assert_eq!(v.glyph_reason, ProbeReason::PassiveEnv);
}

#[test]
fn red_decide_vte_7700_reaches_octants() {
    let v = decide(
        &vte076_evidence(),
        env(&[("TERM", "xterm-256color"), ("VTE_VERSION", "7700")]),
    );
    assert_eq!(v.glyph_tier, GlyphTier::Octant);
}

#[test]
fn red_decide_old_vte_is_basic() {
    let v = decide(
        &vte076_evidence(),
        env(&[("TERM", "xterm-256color"), ("VTE_VERSION", "5400")]),
    );
    assert_eq!(v.glyph_tier, GlyphTier::Basic);
    assert_eq!(v.glyph_reason, ProbeReason::NoEvidence);
    assert_eq!(
        v.sixel.state,
        Capability::Unsupported,
        "keeps the stub from passing"
    );
}

#[test]
fn red_decide_kitty_is_graphics_and_octants() {
    let v = decide(&kitty_evidence(), env(KITTY_ENV));
    assert_eq!(v.kitty_graphics.state, Capability::Supported);
    assert_eq!(v.kitty_graphics.reason, ProbeReason::Replied);
    assert_eq!(v.glyph_tier, GlyphTier::Octant);
    // kitty's DA1 has no 4 and it gave no XTSMGRAPHICS reply: no sixel, and the table says so.
    assert_eq!(v.sixel.state, Capability::Unsupported);
}

#[test]
fn red_decide_a_graphics_reply_alone_does_not_imply_octants() {
    // Other terminals answer the kitty query; the glyph tier comes from naming the terminal.
    let v = decide(&kitty_evidence(), env(PLAIN_ENV));
    assert_eq!(v.kitty_graphics.state, Capability::Supported);
    assert_eq!(v.glyph_tier, GlyphTier::Basic);
}

#[test]
fn red_decide_kitty_error_reply_is_unsupported() {
    let mut e = ProbeEvidence::default();
    e.kitty = parse_kitty_graphics_reply(KITTY_ERR);
    e.da1 = parse_da1_attributes(KITTY_DA1);
    let v = decide(&e, env(PLAIN_ENV));
    assert_eq!(v.kitty_graphics.state, Capability::Unsupported);
    assert_eq!(v.kitty_graphics.reason, ProbeReason::ErrorReply);
}

#[test]
fn red_decide_da1_4_alone_is_sixel_unknown() {
    let mut e = ProbeEvidence::default();
    e.da1 = parse_da1_attributes(XTERM_SIXEL_DA1);
    let v = decide(&e, env(PLAIN_ENV));
    assert_eq!(v.sixel.state, Capability::Unknown);
    assert_eq!(v.sixel.reason, ProbeReason::Da1Only);
}

#[test]
fn red_decide_da1_4_and_positive_geometry_is_sixel_supported() {
    let e = sixel_evidence(XTERM_SIXEL_DA1, XTSM_GEOMETRY_OK, XTSM_COLORS_OK);
    let v = decide(&e, env(PLAIN_ENV));
    assert_eq!(v.sixel.state, Capability::Supported);
    assert_eq!(v.sixel.reason, ProbeReason::Replied);
}

#[test]
fn red_decide_xtsm_success_without_da1_4_is_unknown_not_supported() {
    let e = sixel_evidence(VTE_DA1, XTSM_GEOMETRY_OK, XTSM_COLORS_OK);
    let v = decide(&e, env(PLAIN_ENV));
    assert_eq!(v.sixel.state, Capability::Unknown);
    assert_eq!(v.sixel.reason, ProbeReason::Conflict);
}

#[test]
fn red_decide_zero_geometry_is_a_conflict_not_support() {
    let e = sixel_evidence(XTERM_SIXEL_DA1, XTSM_GEOMETRY_ZERO, XTSM_COLORS_OK);
    let v = decide(&e, env(PLAIN_ENV));
    assert_eq!(v.sixel.state, Capability::Unknown);
    assert_eq!(v.sixel.reason, ProbeReason::Conflict);
}

#[test]
fn red_decide_xtsm_error_with_da1_4_is_unsupported() {
    let e = sixel_evidence(XTERM_SIXEL_DA1, XTSM_GEOMETRY_ERR, XTSM_COLORS_ERR);
    let v = decide(&e, env(PLAIN_ENV));
    assert_eq!(v.sixel.state, Capability::Unsupported);
    assert_eq!(v.sixel.reason, ProbeReason::XtsmError);
}

#[test]
fn red_decide_timeout_is_unknown_never_unsupported() {
    let v = decide(&ProbeEvidence::default(), env(PLAIN_ENV));
    assert_eq!(v.sixel.state, Capability::Unknown);
    assert_eq!(v.sixel.reason, ProbeReason::TimedOut);
    assert_eq!(v.kitty_graphics.state, Capability::Unknown);
    assert_eq!(v.kitty_graphics.reason, ProbeReason::TimedOut);
}

#[test]
fn red_decide_timeout_keeps_the_passive_glyph_tier() {
    let v = decide(&ProbeEvidence::default(), env(VTE76_ENV));
    assert_eq!(v.sixel.state, Capability::Unknown);
    assert_eq!(v.kitty_graphics.state, Capability::Unknown);
    assert_eq!(v.glyph_tier, GlyphTier::Sextant);
}

#[test]
fn red_decide_a_positive_reply_survives_a_missing_sentinel() {
    let mut e = ProbeEvidence::default();
    e.kitty = parse_kitty_graphics_reply(KITTY_OK);
    let v = decide(&e, env(KITTY_ENV));
    assert_eq!(v.kitty_graphics.state, Capability::Supported);
}

// ---- multiplexer, ssh, USER_FORCED ---------------------------------------------------------

#[test]
fn red_decide_tmux_turns_the_pixel_path_off() {
    let v = decide(
        &kitty_evidence(),
        env(&[
            ("TERM", "tmux-256color"),
            ("TMUX", "/tmp/tmux-1000/default,1234,0"),
            ("KITTY_WINDOW_ID", "1"),
        ]),
    );
    assert_eq!(v.kitty_graphics.state, Capability::Unsupported);
    assert_eq!(v.kitty_graphics.reason, ProbeReason::Multiplexer);
    assert_eq!(v.sixel.state, Capability::Unsupported);
    assert_eq!(v.sixel.reason, ProbeReason::Multiplexer);
    assert_eq!(
        v.glyph_tier,
        GlyphTier::Basic,
        "the outer terminal is not known"
    );
    assert_eq!(v.glyph_reason, ProbeReason::Multiplexer);
}

#[test]
fn red_decide_screen_turns_the_pixel_path_off() {
    let v = decide(
        &kitty_evidence(),
        env(&[("TERM", "screen-256color"), ("STY", "1234.pts-0.host")]),
    );
    assert_eq!(v.kitty_graphics.state, Capability::Unsupported);
    assert_eq!(v.kitty_graphics.reason, ProbeReason::Multiplexer);
}

#[test]
fn red_decide_an_empty_multiplexer_variable_is_unset() {
    let v = decide(
        &kitty_evidence(),
        env(&[("TERM", "xterm-kitty"), ("TMUX", "")]),
    );
    assert_eq!(v.kitty_graphics.state, Capability::Supported);
}

#[test]
fn red_decide_ssh_turns_the_pixel_path_off() {
    for var in ["SSH_CONNECTION", "SSH_TTY"] {
        let get = move |k: &str| match k {
            "TERM" => Some("xterm-kitty".to_string()),
            k if k == var => Some("10.0.0.2 5555 10.0.0.1 22".to_string()),
            _ => None,
        };
        let v = decide(&kitty_evidence(), get);
        assert_eq!(v.kitty_graphics.state, Capability::Unsupported, "{var}");
        assert_eq!(v.kitty_graphics.reason, ProbeReason::Remote, "{var}");
        assert_eq!(v.sixel.reason, ProbeReason::Remote, "{var}");
    }
}

#[test]
fn red_decide_user_forced_beats_ssh_and_tmux() {
    let mut e = kitty_evidence();
    e.forced.kitty_graphics = Some(true);
    let ssh = env(&[("TERM", "xterm-256color"), ("SSH_CONNECTION", "a 1 b 22")]);
    let v = decide(&e, ssh);
    assert_eq!(v.kitty_graphics.state, Capability::Supported);
    assert_eq!(v.kitty_graphics.reason, ProbeReason::UserForced);

    let tmux = env(&[("TERM", "tmux-256color"), ("TMUX", "x")]);
    let v = decide(&e, tmux);
    assert_eq!(v.kitty_graphics.state, Capability::Supported);
    assert_eq!(v.kitty_graphics.reason, ProbeReason::UserForced);
}

#[test]
fn red_decide_user_forced_off_beats_positive_evidence() {
    let mut e = sixel_evidence(XTERM_SIXEL_DA1, XTSM_GEOMETRY_OK, XTSM_COLORS_OK);
    e.forced.sixel = Some(false);
    let v = decide(&e, env(PLAIN_ENV));
    assert_eq!(v.sixel.state, Capability::Unsupported);
    assert_eq!(v.sixel.reason, ProbeReason::UserForced);
}

#[test]
fn red_decide_user_forced_needs_no_evidence() {
    let mut e = ProbeEvidence::default();
    e.forced.kitty_graphics = Some(true);
    let v = decide(&e, env(PLAIN_ENV));
    assert_eq!(v.kitty_graphics.state, Capability::Supported);
    assert_eq!(v.kitty_graphics.reason, ProbeReason::UserForced);
    // Forcing one capability does not touch the other.
    assert_eq!(v.sixel.state, Capability::Unknown);
}

// ---- applying a verdict --------------------------------------------------------------------

#[test]
fn red_with_probe_applies_known_values_and_keeps_unknown_ones() {
    let v = decide(&kitty_evidence(), env(KITTY_ENV));
    let caps = TerminalCapabilities::truecolor().with_probe(&v);
    assert_eq!(caps.kitty_graphics, Capability::Supported);
    assert_eq!(caps.sixel, Capability::Unsupported);
    assert_eq!(caps.iterm_images, Capability::Unknown, "not probed");

    let v = decide(&ProbeEvidence::default(), env(PLAIN_ENV));
    let caps = TerminalCapabilities::truecolor().with_probe(&v);
    assert_eq!(caps.kitty_graphics, Capability::Unknown);
    assert_eq!(caps.sixel, Capability::Unknown);
}

// ---- guards: pass today ---------------------------------------------------------------------
//
// There is no guard for the existing DA1 parse (`PrimaryDeviceAttributes`): `tty_source` is a
// private module and the event is `pub(crate)`, so an integration test has no path to it. The
// in-crate parser tests cover it and the stub slice does not touch them.

#[test]
fn guard_passive_detection_never_promotes_pixel_capabilities() {
    let caps = TerminalCapabilities::from_env(env(&[
        ("TERM", "xterm-kitty"),
        ("COLORTERM", "truecolor"),
        ("KITTY_WINDOW_ID", "1"),
        ("VTE_VERSION", "7600"),
    ]));
    assert_eq!(caps.kitty_graphics, Capability::Unknown);
    assert_eq!(caps.sixel, Capability::Unknown);
    assert_eq!(caps.iterm_images, Capability::Unknown);
}

#[test]
fn guard_every_constructor_leaves_pixel_capabilities_unknown() {
    for caps in [
        TerminalCapabilities::mono(),
        TerminalCapabilities::ansi16(),
        TerminalCapabilities::ansi256(),
        TerminalCapabilities::truecolor(),
    ] {
        assert_eq!(caps.kitty_graphics, Capability::Unknown);
        assert_eq!(caps.sixel, Capability::Unknown);
        assert_eq!(caps.iterm_images, Capability::Unknown);
    }
}
