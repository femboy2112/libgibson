//! Terminal capability probe: pure parsers and the pure decision table (v0.9.0 design).
//!
//! RED slice: every function here compiles and does nothing yet. The design is in
//! `docs/design/v0.9-capability-probe.md`; the failing tests are in `tests/capability_probe.rs`.
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
    /// No evidence was applied (also what the RED stub returns).
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

/// Parses a complete DA1 reply `ESC [ ? Ps ; Ps ... c` into its attribute numbers.
///
/// A trailing empty parameter (`ESC [ ? 62 ; c`) is ignored; an empty one in the middle, a
/// non-digit, a number above `u16::MAX`, no attributes at all, or anything that is not a `?`
/// primary reply (a `>` secondary reply, say) is `None`.
pub fn parse_da1_attributes(_bytes: &[u8]) -> Option<Vec<u16>> {
    // RED stub: v0.9.0 design, not implemented
    None
}

/// Parses a complete XTSMGRAPHICS reply `ESC [ ? Pi ; Ps ; Pv... S`.
pub fn parse_xtsmgraphics(_bytes: &[u8]) -> Option<XtsmReply> {
    // RED stub: v0.9.0 design, not implemented
    None
}

/// Parses a complete kitty graphics reply `ESC _ G <keys> ; <message> ESC \`. The string
/// terminator is required.
pub fn parse_kitty_graphics_reply(_bytes: &[u8]) -> Option<KittyReply> {
    // RED stub: v0.9.0 design, not implemented
    None
}

/// The bytes the probe writes, in order: the kitty graphics query, the XTSMGRAPHICS reads, and
/// DA1 last as the sentinel. Non-destructive: no mode is set and no stack is pushed.
pub fn probe_request_bytes() -> &'static [u8] {
    // RED stub: v0.9.0 design, not implemented
    b""
}

/// The decision table: evidence and environment in, verdict out. Pure; no I/O.
pub fn decide(_evidence: &ProbeEvidence, _env: impl Fn(&str) -> Option<String>) -> ProbeVerdict {
    // RED stub: v0.9.0 design, not implemented
    let nothing = CapabilityVerdict {
        state: Capability::Unknown,
        reason: ProbeReason::NoEvidence,
    };
    ProbeVerdict {
        sixel: nothing,
        kitty_graphics: nothing,
        glyph_tier: GlyphTier::Basic,
        glyph_reason: ProbeReason::NoEvidence,
    }
}

impl TerminalCapabilities {
    /// Applies a verdict: `Supported` and `Unsupported` replace the field, `Unknown` keeps what
    /// was there.
    pub fn with_probe(self, _verdict: &ProbeVerdict) -> Self {
        // RED stub: v0.9.0 design, not implemented
        self
    }
}
