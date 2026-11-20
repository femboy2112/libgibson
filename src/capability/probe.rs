//! Terminal capability probe: pure parsers and the pure decision table (v0.9.0 design).
//!
//! The design is in `docs/design/v0.9-capability-probe.md`; the tests are in
//! `tests/capability_probe.rs`.
//! Nothing in this module writes to a terminal or reads from one: the request is a byte string,
//! the replies are byte strings, and the verdict is a pure function of both plus the environment.

use super::Capability;
use super::TerminalCapabilities;

/// A parsed kitty graphics reply: `ESC _ G <keys> ; <message> ESC \`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct KittyReply {
    /// The `i=` key echoed by the terminal, if present.
    pub id: Option<u32>,
    /// The message was exactly `OK`.
    pub ok: bool,
    /// The text after the first `;`, for example `OK` or `ENOENT:no such image`.
    pub message: String,
}

/// The `Ps` status of an XTSMGRAPHICS reply (xterm ctlseqs: 0 success, 1 error in `Pi`,
/// 2 error in `Pa`, 3 failure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum XtsmStatus {
    Success,
    ErrorInItem,
    ErrorInAction,
    Failure,
    /// Any other status number; never treated as success.
    Other(u16),
}

/// A parsed XTSMGRAPHICS reply: `CSI ? Pi ; Ps ; Pv... S`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct XtsmReply {
    /// `Pi`: 1 = colour registers, 2 = sixel geometry, 3 = ReGIS geometry.
    pub item: u16,
    pub status: XtsmStatus,
    /// `Pv...`: one number for item 1, width then height for item 2.
    pub values: Vec<u32>,
}

impl XtsmReply {
    /// `Ps` was 0.
    pub fn is_success(&self) -> bool {
        matches!(self.status, XtsmStatus::Success)
    }
}

/// The replies that arrived before the DA1 sentinel (or before the deadline), as parsed values.
/// `da1: None` means the sentinel never arrived (timeout); the other fields are `None` when
/// that reply did not arrive.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProbeEvidence {
    pub da1: Option<Vec<u16>>,
    pub kitty: Option<KittyReply>,
    pub xtsm_geometry: Option<XtsmReply>,
    pub xtsm_colors: Option<XtsmReply>,
    pub forced: UserForced,
}

/// The application's explicit overrides (`USER_FORCED`). `Some(b)` replaces the verdict and
/// beats the multiplexer and ssh policy; `None` leaves the probe in charge.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct UserForced {
    pub kitty_graphics: Option<bool>,
    pub sixel: Option<bool>,
}

/// Why a verdict is what it is. Each verdict carries exactly one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProbeReason {
    /// No evidence was applied.
    NoEvidence,
    /// The terminal answered the query affirmatively.
    Replied,
    /// The terminal answered the query with an error.
    ErrorReply,
    /// DA1 arrived and the terminal had not answered the query before it.
    NoReplyBeforeDa1,
    /// The DA1 sentinel never arrived before the deadline.
    TimedOut,
    /// DA1 attribute 4 is the only evidence for sixel.
    Da1Only,
    /// XTSMGRAPHICS and DA1 disagree, or the geometry is not positive.
    Conflict,
    /// XTSMGRAPHICS answered with a non-success status.
    XtsmError,
    /// Under tmux or screen the replies come from the multiplexer.
    Multiplexer,
    /// Under ssh the pixel path is off unless forced.
    Remote,
    /// The application forced the value.
    UserForced,
    /// A passive environment signal named the terminal.
    PassiveEnv,
}

/// A tri-state capability and the reason for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct CapabilityVerdict {
    pub state: Capability,
    pub reason: ProbeReason,
}

/// The block-glyph tier the terminal draws itself, lowest to highest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum GlyphTier {
    /// Half blocks, Braille and what every font has. Also the answer for an unrecognised terminal.
    Basic,
    /// 2x3 sextants (U+1FB00..U+1FB3B) drawn by the terminal itself.
    Sextant,
    /// 2x4 octants (U+1CD00..U+1CDE5) drawn by the terminal itself.
    Octant,
}

/// The result of [`decide`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProbeVerdict {
    pub sixel: CapabilityVerdict,
    pub kitty_graphics: CapabilityVerdict,
    pub glyph_tier: GlyphTier,
    pub glyph_reason: ProbeReason,
}

/// Parses one decimal parameter: non-empty, ASCII digits only, and it must fit `T`.
fn parse_num<T: std::str::FromStr>(part: &[u8]) -> Option<T> {
    if part.is_empty() || !part.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(part).ok()?.parse().ok()
}

/// Parses a complete DA1 reply `ESC [ ? Ps ; Ps ... c` into its attribute numbers.
///
/// A trailing empty parameter (`ESC [ ? 62 ; c`) is ignored; an empty one in the middle, a
/// non-digit, a number above `u16::MAX`, no attributes at all, or anything that is not a `?`
/// primary reply (a `>` secondary reply, say) is `None`.
pub fn parse_da1_attributes(bytes: &[u8]) -> Option<Vec<u16>> {
    let body = bytes.strip_prefix(b"\x1b[?")?.strip_suffix(b"c")?;
    let mut parts: Vec<&[u8]> = body.split(|b| *b == b';').collect();
    if parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    if parts.is_empty() {
        return None;
    }
    parts.into_iter().map(parse_num::<u16>).collect()
}

/// Parses a complete XTSMGRAPHICS reply `ESC [ ? Pi ; Ps ; Pv... S`.
pub fn parse_xtsmgraphics(bytes: &[u8]) -> Option<XtsmReply> {
    let body = bytes.strip_prefix(b"\x1b[?")?.strip_suffix(b"S")?;
    let parts: Vec<&[u8]> = body.split(|b| *b == b';').collect();
    if parts.len() < 3 {
        return None;
    }
    let item: u16 = parse_num(parts[0])?;
    let ps: u16 = parse_num(parts[1])?;
    let status = match ps {
        0 => XtsmStatus::Success,
        1 => XtsmStatus::ErrorInItem,
        2 => XtsmStatus::ErrorInAction,
        3 => XtsmStatus::Failure,
        n => XtsmStatus::Other(n),
    };
    let values = parts[2..]
        .iter()
        .map(|p| parse_num::<u32>(p))
        .collect::<Option<Vec<u32>>>()?;
    Some(XtsmReply {
        item,
        status,
        values,
    })
}

/// Parses a complete kitty graphics reply `ESC _ G <keys> ; <message> ESC \`. The string
/// terminator is required.
pub fn parse_kitty_graphics_reply(bytes: &[u8]) -> Option<KittyReply> {
    let body = bytes.strip_prefix(b"\x1b_G")?.strip_suffix(b"\x1b\\")?;
    let semi = body.iter().position(|b| *b == b';')?;
    let (keys, rest) = body.split_at(semi);
    let message = String::from_utf8(rest[1..].to_vec()).ok()?;
    let mut id = None;
    for key in keys.split(|b| *b == b',') {
        if let Some(value) = key.strip_prefix(b"i=") {
            id = parse_num::<u32>(value);
        }
    }
    Some(KittyReply {
        id,
        ok: message == "OK",
        message,
    })
}

/// The bytes the probe writes, in order: the kitty graphics query, the XTSMGRAPHICS reads, and
/// DA1 last as the sentinel. Non-destructive: no mode is set and no stack is pushed.
pub fn probe_request_bytes() -> &'static [u8] {
    b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[?2;1;0S\x1b[?1;1;0S\x1b[c"
}

fn verdict(state: Capability, reason: ProbeReason) -> CapabilityVerdict {
    CapabilityVerdict { state, reason }
}

fn forced_verdict(forced: bool) -> CapabilityVerdict {
    let state = if forced {
        Capability::Supported
    } else {
        Capability::Unsupported
    };
    verdict(state, ProbeReason::UserForced)
}

/// Why the pixel path is off regardless of evidence (multiplexer first, then ssh), if it is.
fn pixel_policy(multiplexer: bool, remote: bool) -> Option<CapabilityVerdict> {
    if multiplexer {
        Some(verdict(Capability::Unsupported, ProbeReason::Multiplexer))
    } else if remote {
        Some(verdict(Capability::Unsupported, ProbeReason::Remote))
    } else {
        None
    }
}

fn decide_kitty(evidence: &ProbeEvidence) -> CapabilityVerdict {
    match (&evidence.kitty, &evidence.da1) {
        (Some(reply), _) if reply.ok => verdict(Capability::Supported, ProbeReason::Replied),
        (Some(_), _) => verdict(Capability::Unsupported, ProbeReason::ErrorReply),
        (None, Some(_)) => verdict(Capability::Unsupported, ProbeReason::NoReplyBeforeDa1),
        (None, None) => verdict(Capability::Unknown, ProbeReason::TimedOut),
    }
}

fn decide_sixel(evidence: &ProbeEvidence) -> CapabilityVerdict {
    let Some(da1) = &evidence.da1 else {
        return verdict(Capability::Unknown, ProbeReason::TimedOut);
    };
    let has_sixel = da1.contains(&4);
    match &evidence.xtsm_geometry {
        Some(g) if g.is_success() => {
            let positive = g.values.len() >= 2 && g.values[0] > 0 && g.values[1] > 0;
            if has_sixel && positive {
                verdict(Capability::Supported, ProbeReason::Replied)
            } else {
                verdict(Capability::Unknown, ProbeReason::Conflict)
            }
        }
        Some(_) => verdict(Capability::Unsupported, ProbeReason::XtsmError),
        None if has_sixel => verdict(Capability::Unknown, ProbeReason::Da1Only),
        None => verdict(Capability::Unsupported, ProbeReason::NoReplyBeforeDa1),
    }
}

/// The decision table: evidence and environment in, verdict out. Pure; no I/O.
pub fn decide(evidence: &ProbeEvidence, env: impl Fn(&str) -> Option<String>) -> ProbeVerdict {
    let is_set = |name: &str| env(name).is_some_and(|v| !v.is_empty());
    let multiplexer = is_set("TMUX") || is_set("STY");
    let remote = is_set("SSH_CONNECTION") || is_set("SSH_TTY");
    let policy = pixel_policy(multiplexer, remote);

    let kitty_graphics = match (evidence.forced.kitty_graphics, policy) {
        (Some(forced), _) => forced_verdict(forced),
        (None, Some(off)) => off,
        (None, None) => decide_kitty(evidence),
    };
    let sixel = match (evidence.forced.sixel, policy) {
        (Some(forced), _) => forced_verdict(forced),
        (None, Some(off)) => off,
        (None, None) => decide_sixel(evidence),
    };

    let (glyph_tier, glyph_reason) = if multiplexer {
        (GlyphTier::Basic, ProbeReason::Multiplexer)
    } else if is_set("KITTY_WINDOW_ID") || env("TERM").as_deref() == Some("xterm-kitty") {
        (GlyphTier::Octant, ProbeReason::PassiveEnv)
    } else {
        match env("VTE_VERSION").and_then(|v| v.parse::<u32>().ok()) {
            Some(v) if v >= 7700 => (GlyphTier::Octant, ProbeReason::PassiveEnv),
            Some(v) if v >= 7600 => (GlyphTier::Sextant, ProbeReason::PassiveEnv),
            _ => (GlyphTier::Basic, ProbeReason::NoEvidence),
        }
    };

    ProbeVerdict {
        sixel,
        kitty_graphics,
        glyph_tier,
        glyph_reason,
    }
}

impl TerminalCapabilities {
    /// Applies a verdict: `Supported` and `Unsupported` replace the field, `Unknown` keeps what
    /// was there.
    pub fn with_probe(mut self, verdict: &ProbeVerdict) -> Self {
        if verdict.sixel.state != Capability::Unknown {
            self.sixel = verdict.sixel.state;
        }
        if verdict.kitty_graphics.state != Capability::Unknown {
            self.kitty_graphics = verdict.kitty_graphics.state;
        }
        self
    }
}
