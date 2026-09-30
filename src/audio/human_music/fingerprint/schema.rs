//! Explicit v2 schemas. Tags/order are durable; never regenerate them on a Debug refactor.
use super::super::*;
use super::{CanonicalFingerprint, FingerprintWriter};

macro_rules! structure {
    ($ty:path, $schema:literal; $($field:ident => $tag:literal),* $(,)? $(; ignore $($ignored:ident),*)?) => {
        impl CanonicalFingerprint for $ty {
            fn encode(&self, w: &mut FingerprintWriter) {
                w.tag($schema);
                // Exhaustive destructuring makes a newly added field a schema-review error.
                let Self { $($field,)* $($($ignored: _,)*)? } = self;
                $(w.field($tag, $field);)*
            }
        }
    };
}

structure!(performance::AccentGrid, "performance/AccentGrid/v2";
    bars => "bars",
    cells => "cells",
);

impl CanonicalFingerprint for action::ActionCause {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("action/ActionCause/v2");
        match self {
            Self::Morphism {
                transition,
                morphism,
            } => {
                w.tag("Morphism");
                w.field("transition", transition);
                w.field("morphism", morphism);
            }
            Self::Gesture { slot, gesture } => {
                w.tag("Gesture");
                w.field("slot", slot);
                w.field("gesture", gesture);
            }
            Self::Interaction { call } => {
                w.tag("Interaction");
                w.field("call", call);
            }
            Self::Statement { phrase } => {
                w.tag("Statement");
                w.field("phrase", phrase);
            }
            Self::Discourse { obligation, phrase } => {
                w.tag("Discourse");
                w.field("obligation", obligation);
                w.field("phrase", phrase);
            }
        }
    }
}

structure!(action::ActionFamilies, "action/ActionFamilies/v2";
    lift_pickup => "lift_pickup",
    lift_reach => "lift_reach",
    reset_fill => "reset_fill",
    open_reentry => "open_reentry",
);

impl CanonicalFingerprint for action::ActionKind {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("action/ActionKind/v2");
        match self {
            Self::Pickup => w.tag("Pickup"),
            Self::Push => w.tag("Push"),
            Self::Pullback => w.tag("Pullback"),
            Self::Accelerate => w.tag("Accelerate"),
            Self::Hit => w.tag("Hit"),
            Self::Break => w.tag("Break"),
            Self::ReEntry => w.tag("ReEntry"),
            Self::Hold => w.tag("Hold"),
            Self::Reharmonize => w.tag("Reharmonize"),
            Self::Recolor => w.tag("Recolor"),
            Self::Tonicize => w.tag("Tonicize"),
            Self::Modulate => w.tag("Modulate"),
            Self::Deflect => w.tag("Deflect"),
            Self::Resolve => w.tag("Resolve"),
            Self::Displace => w.tag("Displace"),
            Self::Fragment => w.tag("Fragment"),
            Self::Sequence => w.tag("Sequence"),
            Self::Thicken => w.tag("Thicken"),
            Self::Thin => w.tag("Thin"),
            Self::Fill => w.tag("Fill"),
            Self::Call => w.tag("Call"),
            Self::Answer => w.tag("Answer"),
            Self::Unison => w.tag("Unison"),
        }
    }
}

structure!(action::ActionPlan, "action/ActionPlan/v2";
    actions => "actions",
    deferred => "deferred",
    stasis => "stasis",
    families => "families",
    manifestations => "manifestations",
);

impl CanonicalFingerprint for performance::Admission {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("performance/Admission/v2");
        match self {
            Self::Admitted { agent } => {
                w.tag("Admitted");
                w.field("agent", agent);
            }
            Self::Recast {
                from_kind,
                from,
                to_kind,
                to,
            } => {
                w.tag("Recast");
                w.field("from_kind", from_kind);
                w.field("from", from);
                w.field("to_kind", to_kind);
                w.field("to", to);
            }
            Self::Rejected { reason } => {
                w.tag("Rejected");
                w.field("reason", reason);
            }
        }
    }
}

structure!(performance::AdmissionRecord, "performance/AdmissionRecord/v2";
    action => "action",
    kind => "kind",
    start_beat => "start_beat",
    outcome => "outcome",
);

impl CanonicalFingerprint for action::Agent {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("action/Agent/v2");
        match self {
            Self::Lead => w.tag("Lead"),
            Self::Keys => w.tag("Keys"),
            Self::Bass => w.tag("Bass"),
            Self::Drums => w.tag("Drums"),
            Self::Pad => w.tag("Pad"),
            Self::Ensemble => w.tag("Ensemble"),
        }
    }
}

structure!(plan::ArrangementPlan, "plan/ArrangementPlan/v2";
    phrases => "phrases",
    foreground_budget => "foreground_budget",
);

impl CanonicalFingerprint for plan::ArrangementRole {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("plan/ArrangementRole/v2");
        match self {
            Self::Foreground => w.tag("Foreground"),
            Self::Support => w.tag("Support"),
            Self::Foundation => w.tag("Foundation"),
            Self::Pulse => w.tag("Pulse"),
            Self::Texture => w.tag("Texture"),
            Self::Punctuation => w.tag("Punctuation"),
            Self::Silent => w.tag("Silent"),
        }
    }
}

structure!(backbone::BackboneTimeline, "backbone/BackboneTimeline/v2";
    scales => "scales",
    slots => "slots",
    bindings => "bindings",
    total_bars => "total_bars",
);

impl CanonicalFingerprint for ensemble::BassMode {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("ensemble/BassMode/v2");
        match self {
            Self::Foundation => w.tag("Foundation"),
            Self::Pedal => w.tag("Pedal"),
            Self::Walk => w.tag("Walk"),
            Self::Counter => w.tag("Counter"),
            Self::Quote => w.tag("Quote"),
        }
    }
}

structure!(budget::Burst, "budget/Burst/v2";
    action => "action",
    kind => "kind",
    extra => "extra",
);

structure!(interaction::Call, "interaction/Call/v2";
    action => "action",
    initiator => "initiator",
    start_beat => "start_beat",
    end_beat => "end_beat",
    statement => "statement",
    material => "material",
);

impl CanonicalFingerprint for interaction::CallPolicy {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("interaction/CallPolicy/v2");
        match self {
            Self::Selective => w.tag("Selective"),
            Self::EveryStatement => w.tag("EveryStatement"),
        }
    }
}

structure!(backbone::ChartCell, "backbone/ChartCell/v2";
    lift => "lift",
    lift_alt => "lift_alt",
    pointer => "pointer",
    expected => "expected",
    deflect => "deflect",
    open => "open",
    reset => "reset",
    satellites => "satellites",
);

impl CanonicalFingerprint for backbone::ChartRoot {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("backbone/ChartRoot/v2");
        match self {
            Self::Degree(value0) => {
                w.tag("Degree");
                w.field("0", value0);
            }
            Self::Chromatic { semitones, quality } => {
                w.tag("Chromatic");
                w.field("semitones", semitones);
                w.field("quality", quality);
            }
        }
    }
}

structure!(theory::Chord, "theory/Chord/v2";
    root_pc => "root_pc",
    quality => "quality",
);

structure!(harmony::ChordSpan, "harmony/ChordSpan/v2";
    start_beat => "start_beat",
    dur_beats => "dur_beats",
    chord => "chord",
    function => "function",
    degree => "degree",
    note => "note",
);

impl CanonicalFingerprint for backbone::ClockBinding {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("backbone/ClockBinding/v2");
        match self {
            Self::SemanticPhase => w.tag("SemanticPhase"),
            Self::FixedTiling { bars_per_gesture } => {
                w.tag("FixedTiling");
                w.field("bars_per_gesture", bars_per_gesture);
            }
        }
    }
}

impl CanonicalFingerprint for meaning::Close {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("meaning/Close/v2");
        match self {
            Self::Home => w.tag("Home"),
            Self::Open => w.tag("Open"),
        }
    }
}

impl CanonicalFingerprint for discourse::Closure {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("discourse/Closure/v2");
        match self {
            Self::Open => w.tag("Open"),
            Self::Half => w.tag("Half"),
            Self::Deferred => w.tag("Deferred"),
            Self::Deceptive => w.tag("Deceptive"),
            Self::Weak => w.tag("Weak"),
            Self::Strong => w.tag("Strong"),
        }
    }
}

impl CanonicalFingerprint for contract::CoherenceAnchor {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("contract/CoherenceAnchor/v2");
        match self {
            Self::Motif => w.tag("Motif"),
            Self::Riff => w.tag("Riff"),
            Self::Groove => w.tag("Groove"),
            Self::HarmonicContour => w.tag("HarmonicContour"),
            Self::HarmonicLoop => w.tag("HarmonicLoop"),
            Self::Form => w.tag("Form"),
            Self::Orchestration => w.tag("Orchestration"),
            Self::BassFigure => w.tag("BassFigure"),
        }
    }
}

structure!(contract::CoherenceContract, "contract/CoherenceContract/v2";
    grammar => "grammar",
    anchors => "anchors",
    recurrence_bars => "recurrence_bars",
    max_transform => "max_transform",
    phrase_bars => "phrase_bars",
    resolution => "resolution",
    foreground_budget => "foreground_budget",
    novelty_budget => "novelty_budget",
);

structure!(budget::ComplexityAllocation, "budget/ComplexityAllocation/v2";
    bar => "bar",
    total => "total",
    reserved => "reserved",
    allowance => "allowance",
    burst => "burst",
);

impl CanonicalFingerprint for contract::CompositionGrammar {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("contract/CompositionGrammar/v2");
        match self {
            Self::HookArc => w.tag("HookArc"),
            Self::LoopEvolution => w.tag("LoopEvolution"),
            Self::RiffDrive => w.tag("RiffDrive"),
            Self::WorldSwitch => w.tag("WorldSwitch"),
            Self::DeflectedLift => w.tag("DeflectedLift"),
            Self::PropulsiveReturn => w.tag("PropulsiveReturn"),
        }
    }
}

impl CanonicalFingerprint for backbone::CycleVariation {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("backbone/CycleVariation/v2");
        match self {
            Self::Thesis => w.tag("Thesis"),
            Self::Statement => w.tag("Statement"),
            Self::Expanded => w.tag("Expanded"),
            Self::Compressed => w.tag("Compressed"),
            Self::Transformed => w.tag("Transformed"),
        }
    }
}

structure!(action::Deferral, "action/Deferral/v2";
    transition => "transition",
    morphism => "morphism",
    reason => "reason",
);

structure!(backbone::DeflectWitness, "backbone/DeflectWitness/v2";
    at_beat => "at_beat",
    pointer => "pointer",
    prepared => "prepared",
    expected => "expected",
    actual => "actual",
    common_tones => "common_tones",
    root_distance => "root_distance",
    voice_motion => "voice_motion",
    rejoin_beats => "rejoin_beats",
);

impl CanonicalFingerprint for semantic::Density {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("semantic/Density/v2");
        match self {
            Self::Compact => w.tag("Compact"),
            Self::Normal => w.tag("Normal"),
            Self::Spacious => w.tag("Spacious"),
        }
    }
}

structure!(discourse::DiscoursePlan, "discourse/DiscoursePlan/v2";
    thesis => "thesis",
    goals => "goals",
    ledger => "ledger",
    culmination => "culmination",
    answer => "answer",
);

impl CanonicalFingerprint for discourse::DiscourseRole {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("discourse/DiscourseRole/v2");
        match self {
            Self::Establish => w.tag("Establish"),
            Self::Restate => w.tag("Restate"),
            Self::Depart => w.tag("Depart"),
            Self::Intensify => w.tag("Intensify"),
            Self::Question => w.tag("Question"),
            Self::Withhold => w.tag("Withhold"),
            Self::Culminate => w.tag("Culminate"),
            Self::Answer => w.tag("Answer"),
            Self::Return => w.tag("Return"),
            Self::Dissolve => w.tag("Dissolve"),
        }
    }
}

structure!(score::DrumHit, "score/DrumHit/v2";
    start_beat => "start_beat",
    voice => "voice",
    velocity => "velocity",
    prov => "prov",
);

impl CanonicalFingerprint for score::DrumVoice {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("score/DrumVoice/v2");
        match self {
            Self::Kick => w.tag("Kick"),
            Self::Snare => w.tag("Snare"),
            Self::ClosedHat => w.tag("ClosedHat"),
            Self::OpenHat => w.tag("OpenHat"),
            Self::Clap => w.tag("Clap"),
        }
    }
}

impl CanonicalFingerprint for ensemble::DrumsMode {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("ensemble/DrumsMode/v2");
        match self {
            Self::Pocket => w.tag("Pocket"),
            Self::HalfTime => w.tag("HalfTime"),
            Self::DoubleTime => w.tag("DoubleTime"),
            Self::Break => w.tag("Break"),
            Self::Fill => w.tag("Fill"),
        }
    }
}

impl CanonicalFingerprint for region::EditKind {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("region/EditKind/v2");
        match self {
            Self::Recolor => w.tag("Recolor"),
            Self::TritoneSub => w.tag("TritoneSub"),
            Self::ThirdSub => w.tag("ThirdSub"),
            Self::AppliedDominant => w.tag("AppliedDominant"),
            Self::PivotIn => w.tag("PivotIn"),
            Self::PivotOut => w.tag("PivotOut"),
        }
    }
}

structure!(action::EffectVector, "action/EffectVector/v2";
    strength => "strength",
    d_pressure => "d_pressure",
    d_dynamic => "d_dynamic",
    d_density => "d_density",
    d_elevation => "d_elevation",
    compactness => "compactness",
    novelty => "novelty",
);

impl CanonicalFingerprint for semantic::Elevation {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("semantic/Elevation/v2");
        match self {
            Self::Flat => w.tag("Flat"),
            Self::Raised => w.tag("Raised"),
            Self::Overlay => w.tag("Overlay"),
        }
    }
}

impl CanonicalFingerprint for semantic::Emphasis {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("semantic/Emphasis/v2");
        match self {
            Self::Faint => w.tag("Faint"),
            Self::Muted => w.tag("Muted"),
            Self::Normal => w.tag("Normal"),
            Self::Strong => w.tag("Strong"),
        }
    }
}

structure!(ensemble::EnsembleBar, "ensemble/EnsembleBar/v2";
    bar => "bar",
    gesture => "gesture",
    cycle => "cycle",
    foreground => "foreground",
    pad => "pad",
    keys => "keys",
    bass => "bass",
    drums => "drums",
    budget => "budget",
    kinetic => "kinetic",
);

impl CanonicalFingerprint for performance::EnsembleCoupling {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("performance/EnsembleCoupling/v2");
        match self {
            Self::Independent => w.tag("Independent"),
            Self::CoupledR8 => w.tag("CoupledR8"),
            Self::Surgical => w.tag("Surgical"),
        }
    }
}

impl CanonicalFingerprint for semantic::EventKind {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("semantic/EventKind/v2");
        match self {
            Self::Prolong => w.tag("Prolong"),
            Self::ToneShift => w.tag("ToneShift"),
            Self::FocusAcquired => w.tag("FocusAcquired"),
            Self::SectionResolved => w.tag("SectionResolved"),
            Self::ActChanged => w.tag("ActChanged"),
            Self::ModalEntered => w.tag("ModalEntered"),
            Self::Impact => w.tag("Impact"),
            Self::Confirmation => w.tag("Confirmation"),
        }
    }
}

structure!(plan::FormGraph, "plan/FormGraph/v2";
    phrases => "phrases",
    total_bars => "total_bars",
    total_beats => "total_beats",
);

impl CanonicalFingerprint for theory::Function {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("theory/Function/v2");
        match self {
            Self::Tonic => w.tag("Tonic"),
            Self::Predominant => w.tag("Predominant"),
            Self::Dominant => w.tag("Dominant"),
        }
    }
}

structure!(backbone::GestureSlot, "backbone/GestureSlot/v2";
    gesture => "gesture",
    cycle => "cycle",
    start_bar => "start_bar",
    bars => "bars",
    binding => "binding",
    variation => "variation",
);

impl CanonicalFingerprint for motif::Handoff {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("motif/Handoff/v2");
        match self {
            Self::Restatement => w.tag("Restatement"),
            Self::Develop => w.tag("Develop"),
            Self::Call => w.tag("Call"),
            Self::Response => w.tag("Response"),
            Self::Hook => w.tag("Hook"),
            Self::Dissolve => w.tag("Dissolve"),
            Self::Consequent => w.tag("Consequent"),
        }
    }
}

structure!(context::HarmonicContext, "context/HarmonicContext/v2";
    start_beat => "start_beat",
    dur_beats => "dur_beats",
    chord => "chord",
    bass_pc => "bass_pc",
    region => "region",
    relation => "relation",
    tension => "tension",
    palette => "palette",
    expects => "expects",
);

structure!(region::HarmonicEdit, "region/HarmonicEdit/v2";
    action => "action",
    at_beat => "at_beat",
    before => "before",
    after => "after",
    kind => "kind",
);

impl CanonicalFingerprint for backbone::HarmonicGesture {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("backbone/HarmonicGesture/v2");
        match self {
            Self::Lift => w.tag("Lift"),
            Self::Deflect => w.tag("Deflect"),
            Self::Open => w.tag("Open"),
            Self::Reset => w.tag("Reset"),
        }
    }
}

structure!(song::HarmonicMap, "song/HarmonicMap/v2";
    cell => "cell",
    bars_per_chord => "bars_per_chord",
);

impl CanonicalFingerprint for context::HarmonicRelation {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("context/HarmonicRelation/v2");
        match self {
            Self::Arrival => w.tag("Arrival"),
            Self::Prolong => w.tag("Prolong"),
            Self::Depart => w.tag("Depart"),
            Self::Prepare => w.tag("Prepare"),
            Self::DominantTo { target } => {
                w.tag("DominantTo");
                w.field("target", target);
            }
            Self::Tonicize { target } => {
                w.tag("Tonicize");
                w.field("target", target);
            }
            Self::Deflected { expected, common } => {
                w.tag("Deflected");
                w.field("expected", expected);
                w.field("common", common);
            }
            Self::ModalShift => w.tag("ModalShift"),
            Self::Pedal => w.tag("Pedal"),
            Self::ChromaticConnector => w.tag("ChromaticConnector"),
        }
    }
}

impl CanonicalFingerprint for language::HarmonicRhythm {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("language/HarmonicRhythm/v2");
        match self {
            Self::AsCharted => w.tag("AsCharted"),
            Self::Diminished => w.tag("Diminished"),
        }
    }
}

structure!(phenomenal::HarmonicTarget, "phenomenal/HarmonicTarget/v2";
    beat => "beat",
    stability => "stability",
    expectation => "expectation",
    surprise => "surprise",
    confirm => "confirm",
);

impl CanonicalFingerprint for intent::IntentMorphism {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("intent/IntentMorphism/v2");
        match self {
            Self::Prolong => w.tag("Prolong"),
            Self::Prepare => w.tag("Prepare"),
            Self::Intensify => w.tag("Intensify"),
            Self::Relax => w.tag("Relax"),
            Self::Suspend => w.tag("Suspend"),
            Self::Pivot => w.tag("Pivot"),
            Self::Resolve => w.tag("Resolve"),
            Self::Modulate => w.tag("Modulate"),
            Self::Reharmonize => w.tag("Reharmonize"),
            Self::FragmentMotif => w.tag("FragmentMotif"),
            Self::SequenceMotif => w.tag("SequenceMotif"),
            Self::Augment => w.tag("Augment"),
            Self::Diminish => w.tag("Diminish"),
            Self::Syncopate => w.tag("Syncopate"),
            Self::ThinTexture => w.tag("ThinTexture"),
            Self::ThickenTexture => w.tag("ThickenTexture"),
            Self::Cadence => w.tag("Cadence"),
        }
    }
}

structure!(timeline::IntentSpan, "timeline/IntentSpan/v2";
    start => "start",
    end => "end",
    peak_energy => "peak_energy",
    peak_tension => "peak_tension",
    events_inside => "events_inside",
    salient_inside => "salient_inside",
    next_salient_beat => "next_salient_beat",
);

structure!(timeline::IntentTimeline, "timeline/IntentTimeline/v2";
    initial => "initial",
    transitions => "transitions",
    final_intent => "final_intent",
);

structure!(timeline::IntentTransition, "timeline/IntentTransition/v2";
    at_beat => "at_beat",
    raw_beat => "raw_beat",
    event_kind => "event_kind",
    prev => "prev",
    applied => "applied",
    next => "next",
    step_cost => "step_cost",
    acc_cost => "acc_cost",
    state => "state",
    prev_state => "prev_state",
    effect => "effect",
);

structure!(interaction::Interaction, "interaction/Interaction/v2";
    id => "id",
    call => "call",
    response => "response",
);

impl CanonicalFingerprint for material::InteractionMaterial {
    fn encode(&self, w: &mut FingerprintWriter) {
        let Self {
            id,
            owner,
            source,
            start_beat,
            events,
            pitch_basis,
        } = self;
        w.tag("material/InteractionMaterial/v2");
        w.field("id", id);
        w.field("owner", owner);
        w.field("source", source);
        w.field("start_beat", start_beat);
        w.field("events", events);
        if *pitch_basis != material::PitchBasis::ScaleSteps {
            w.field("pitch_basis", pitch_basis);
        }
    }
}
impl CanonicalFingerprint for theory::PitchBasis {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("theory/PitchBasis/v2");
        w.tag(match self {
            Self::ScaleSteps => "ScaleSteps",
            Self::Semitones => "Semitones",
        });
    }
}

structure!(interaction::InteractionOpportunity, "interaction/InteractionOpportunity/v2";
    source => "source",
    initiator => "initiator",
    start_beat => "start_beat",
    end_beat => "end_beat",
    openness => "openness",
    space => "space",
    headroom => "headroom",
    redundancy => "redundancy",
    score => "score",
    verdict => "verdict",
);

impl CanonicalFingerprint for ensemble::KeysMode {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("ensemble/KeysMode/v2");
        match self {
            Self::Comp => w.tag("Comp"),
            Self::Answer => w.tag("Answer"),
            Self::Stab => w.tag("Stab"),
            Self::Sustain => w.tag("Sustain"),
            Self::Space => w.tag("Space"),
        }
    }
}

impl CanonicalFingerprint for meaning::Lane {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("meaning/Lane/v2");
        match self {
            Self::Theme => w.tag("Theme"),
            Self::Harmony => w.tag("Harmony"),
        }
    }
}

impl CanonicalFingerprint for language::LanguageId {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("language/LanguageId/v2");
        match self {
            Self::Simple => w.tag("Simple"),
            Self::FusionConversation => w.tag("FusionConversation"),
        }
    }
}

structure!(interaction::LeadStatement, "interaction/LeadStatement/v2";
    phrase => "phrase",
    start_beat => "start_beat",
    motif => "motif",
    handoff => "handoff",
    role => "role",
    energy => "energy",
    register => "register",
    is_rupture => "is_rupture",
    call => "call",
    material => "material",
    answers => "answers",
    fragment => "fragment",
);

impl CanonicalFingerprint for meaning::Level {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("meaning/Level/v2");
        match self {
            Self::Low => w.tag("Low"),
            Self::Mid => w.tag("Mid"),
            Self::High => w.tag("High"),
        }
    }
}

impl CanonicalFingerprint for action::Manifestation {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("action/Manifestation/v2");
        match self {
            Self::LiftPickup => w.tag("LiftPickup"),
            Self::LiftDisplacedPush => w.tag("LiftDisplacedPush"),
            Self::LiftReach => w.tag("LiftReach"),
            Self::DeflectBreak => w.tag("DeflectBreak"),
            Self::DeflectHold => w.tag("DeflectHold"),
            Self::DeflectSubtract => w.tag("DeflectSubtract"),
            Self::OpenReEntry => w.tag("OpenReEntry"),
            Self::OpenStaged => w.tag("OpenStaged"),
            Self::OpenWiden => w.tag("OpenWiden"),
            Self::ResetFill => w.tag("ResetFill"),
            Self::ResetFillOther => w.tag("ResetFillOther"),
            Self::ResetDropout => w.tag("ResetDropout"),
            Self::ResetTag => w.tag("ResetTag"),
        }
    }
}

structure!(material::MaterialEvent, "material/MaterialEvent/v2";
    onset => "onset",
    dur => "dur",
    accent => "accent",
    step => "step",
);

impl CanonicalFingerprint for material::MaterialSource {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("material/MaterialSource/v2");
        match self {
            Self::Statement { statement, motif } => {
                w.tag("Statement");
                w.field("statement", statement);
                w.field("motif", motif);
            }
            Self::Figure { action, kind } => {
                w.tag("Figure");
                w.field("action", action);
                w.field("kind", kind);
            }
            Self::Derived { from, transform } => {
                w.tag("Derived");
                w.field("from", from);
                w.field("transform", transform);
            }
        }
    }
}

structure!(meaning::MeaningEvent, "meaning/MeaningEvent/v2";
    lane => "lane",
    at => "at",
    beat => "beat",
    kind => "kind",
);

impl CanonicalFingerprint for meaning::MeaningKind {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("meaning/MeaningKind/v2");
        match self {
            Self::Thesis(value0) => {
                w.tag("Thesis");
                w.field("0", value0);
            }
            Self::Learn => w.tag("Learn"),
            Self::Reinforce => w.tag("Reinforce"),
            Self::Payoff => w.tag("Payoff"),
            Self::Answer(value0) => {
                w.tag("Answer");
                w.field("0", value0);
            }
            Self::Develop => w.tag("Develop"),
            Self::Recognize => w.tag("Recognize"),
            Self::Establish => w.tag("Establish"),
            Self::Prepare(value0) => {
                w.tag("Prepare");
                w.field("0", value0);
            }
            Self::Miss(value0) => {
                w.tag("Miss");
                w.field("0", value0);
            }
            Self::Open => w.tag("Open"),
            Self::Reset => w.tag("Reset"),
            Self::Premature => w.tag("Premature"),
            Self::Unestablished => w.tag("Unestablished"),
            Self::Unprepared => w.tag("Unprepared"),
            Self::Unrelated => w.tag("Unrelated"),
            Self::Arrive => w.tag("Arrive"),
            Self::NoRelief => w.tag("NoRelief"),
            Self::NoHome => w.tag("NoHome"),
            Self::Stray => w.tag("Stray"),
            Self::Foreign => w.tag("Foreign"),
        }
    }
}

structure!(meaning::MeaningPlan, "meaning/MeaningPlan/v2";
    arc => "arc",
    resolution => "resolution",
    events => "events",
);

impl CanonicalFingerprint for theory::Mode {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("theory/Mode/v2");
        match self {
            Self::Ionian => w.tag("Ionian"),
            Self::Dorian => w.tag("Dorian"),
            Self::Phrygian => w.tag("Phrygian"),
            Self::Lydian => w.tag("Lydian"),
            Self::Mixolydian => w.tag("Mixolydian"),
            Self::Aeolian => w.tag("Aeolian"),
            Self::Locrian => w.tag("Locrian"),
            Self::HarmonicMinor => w.tag("HarmonicMinor"),
        }
    }
}

structure!(intent::MorphismCost, "intent/MorphismCost/v2";
    voice_leading => "voice_leading",
    tension_error => "tension_error",
    register_violation => "register_violation",
    parallel_motion => "parallel_motion",
    groove_disruption => "groove_disruption",
    motif_loss => "motif_loss",
    novelty => "novelty",
    repetition => "repetition",
);

impl CanonicalFingerprint for motif::Motif {
    fn encode(&self, w: &mut FingerprintWriter) {
        let Self {
            id,
            degrees,
            rhythm,
            pitch_basis,
        } = self;
        w.tag("motif/Motif/v2");
        w.field("id", id);
        w.field("degrees", degrees);
        w.field("rhythm", rhythm);
        if *pitch_basis != theory::PitchBasis::ScaleSteps {
            w.field("pitch_basis", pitch_basis);
        }
    }
}

structure!(motif::MotifBank, "motif/MotifBank/v2";
    identity => "identity",
    hook => "hook",
    rhythmic_cell => "rhythmic_cell",
    bass_cell => "bass_cell",
    countermotif => "countermotif",
);

structure!(intent::MotifState, "intent/MotifState/v2";
    id => "id",
    development => "development",
    transpose => "transpose",
);

structure!(intent::MusicIntent, "intent/MusicIntent/v2";
    energy => "energy",
    tension => "tension",
    density => "density",
    register => "register",
    function => "function",
    motif => "motif",
    expectation => "expectation",
);

structure!(action::MusicalAction, "action/MusicalAction/v2";
    id => "id",
    cause => "cause",
    initiator => "initiator",
    start_beat => "start_beat",
    dur_beats => "dur_beats",
    kind => "kind",
    target_beat => "target_beat",
    responders => "responders",
    binding => "binding",
    pays => "pays",
    effect => "effect",
);

structure!(language::MusicalLanguage, "language/MusicalLanguage/v2";
    id => "id",
    harmonic_rhythm => "harmonic_rhythm",
    color_depth => "color_depth",
    shell_voicings => "shell_voicings",
    surface_subdivision => "surface_subdivision",
    syncopation => "syncopation",
    interaction => "interaction",
    distributed_agency => "distributed_agency",
    unison_figures => "unison_figures",
    complexity_budget => "complexity_budget",
    chromatic_connectives => "chromatic_connectives",
    internal_rest => "internal_rest",
);

structure!(discourse::MusicalThesis, "discourse/MusicalThesis/v2";
    home_energy => "home_energy",
    home_tension => "home_tension",
    home_register => "home_register",
    home_density => "home_density",
    established_by => "established_by",
    anchors => "anchors",
);

structure!(score::Note, "score/Note/v2";
    start_beat => "start_beat",
    dur_beats => "dur_beats",
    pitch => "pitch",
    velocity => "velocity",
    role => "role",
    prov => "prov",
    function => "function",
);

structure!(discourse::Obligation, "discourse/Obligation/v2";
    id => "id",
    kind => "kind",
    source_phrase => "source_phrase",
    deadline => "deadline",
    strength => "strength",
    deferrable => "deferrable",
    settlement => "settlement",
);

impl CanonicalFingerprint for discourse::ObligationKind {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("discourse/ObligationKind/v2");
        match self {
            Self::HarmonicDeparture => w.tag("HarmonicDeparture"),
            Self::SuspendedCadence => w.tag("SuspendedCadence"),
            Self::MotifQuestion => w.tag("MotifQuestion"),
            Self::RegisterAscent => w.tag("RegisterAscent"),
            Self::GrooveDestabilization => w.tag("GrooveDestabilization"),
        }
    }
}

structure!(discourse::ObligationLedger, "discourse/ObligationLedger/v2";
    obligations => "obligations",
    phrases => "phrases",
);

impl CanonicalFingerprint for interaction::OpportunitySource {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("interaction/OpportunitySource/v2");
        match self {
            Self::Statement(value0) => {
                w.tag("Statement");
                w.field("0", value0);
            }
            Self::Figure(value0) => {
                w.tag("Figure");
                w.field("0", value0);
            }
        }
    }
}

impl CanonicalFingerprint for ensemble::PadMode {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("ensemble/PadMode/v2");
        match self {
            Self::Sustain => w.tag("Sustain"),
            Self::Shell => w.tag("Shell"),
            Self::CommonToneCarry => w.tag("CommonToneCarry"),
            Self::Swell => w.tag("Swell"),
            Self::UpperStructure => w.tag("UpperStructure"),
            Self::Silent => w.tag("Silent"),
        }
    }
}

// Absent cover constraints retain the v2 unconstrained encoding. Present constraints
// are an explicitly tagged extension, so the pre-cover receipt does not drift.
impl CanonicalFingerprint for performance::PerformancePlan {
    fn encode(&self, w: &mut FingerprintWriter) {
        let Self {
            song_fingerprint: _,
            cover_constraints,
            language,
            coupling,
            region,
            regions,
            chords,
            contexts,
            deflects,
            edits,
            actions,
            accent,
            statements,
            interactions,
            ensemble,
            bank,
            response_mode,
            call_policy,
            total_beats,
            materials,
            opportunities,
            stage,
            admissions,
            obligations,
            budget,
        } = self;
        w.tag("performance/PerformancePlan/v2");
        w.field("language", language);
        w.field("coupling", coupling);
        w.field("region", region);
        w.field("regions", regions);
        w.field("chords", chords);
        w.field("contexts", contexts);
        w.field("deflects", deflects);
        w.field("edits", edits);
        w.field("actions", actions);
        w.field("accent", accent);
        w.field("statements", statements);
        w.field("interactions", interactions);
        w.field("ensemble", ensemble);
        w.field("bank", bank);
        w.field("response_mode", response_mode);
        w.field("call_policy", call_policy);
        w.field("total_beats", total_beats);
        w.field("materials", materials);
        w.field("opportunities", opportunities);
        w.field("stage", stage);
        w.field("admissions", admissions);
        w.field("obligations", obligations);
        w.field("budget", budget);
        if let Some(constraints) = cover_constraints {
            w.field("cover_constraints", constraints);
        }
    }
}

impl CanonicalFingerprint for phenomenal::PhenomenalRegime {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("phenomenal/PhenomenalRegime/v2");
        match self {
            Self::SuspendedDeflection => w.tag("SuspendedDeflection"),
            Self::StablePropulsion => w.tag("StablePropulsion"),
        }
    }
}

structure!(phenomenal::PhenomenalState, "phenomenal/PhenomenalState/v2";
    stability => "stability",
    propulsion => "propulsion",
    expectation => "expectation",
    surprise => "surprise",
    openness => "openness",
    familiarity => "familiarity",
);

structure!(phenomenal::PhenomenalTarget, "phenomenal/PhenomenalTarget/v2";
    regime => "regime",
    total_beats => "total_beats",
    state => "state",
    confirm => "confirm",
    contour => "contour",
);

structure!(plan::Phrase, "plan/Phrase/v2";
    ix => "ix",
    start_bar => "start_bar",
    bars => "bars",
    family => "family",
    intent => "intent",
    span => "span",
    is_rupture => "is_rupture",
    piece_end => "piece_end",
);

structure!(plan::PhraseArrangement, "plan/PhraseArrangement/v2";
    pad => "pad",
    keys => "keys",
    bass => "bass",
    lead => "lead",
    drums => "drums",
);

structure!(discourse::PhraseGoal, "discourse/PhraseGoal/v2";
    phrase_ix => "phrase_ix",
    role => "role",
    closure => "closure",
    refers_to => "refers_to",
    next_goal => "next_goal",
    energy_target => "energy_target",
    tension_target => "tension_target",
    density_target => "density_target",
    register_target => "register_target",
    thematic_distance => "thematic_distance",
    harmonic_distance => "harmonic_distance",
    novelty_budget => "novelty_budget",
);

impl CanonicalFingerprint for score::PitchFunction {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("score/PitchFunction/v2");
        match self {
            Self::ChordTone => w.tag("ChordTone"),
            Self::LicensedExtension => w.tag("LicensedExtension"),
            Self::PedalTone => w.tag("PedalTone"),
            Self::DiatonicPassing => w.tag("DiatonicPassing"),
            Self::ChromaticPassing => w.tag("ChromaticPassing"),
            Self::Neighbor => w.tag("Neighbor"),
            Self::ChromaticApproach => w.tag("ChromaticApproach"),
            Self::Enclosure => w.tag("Enclosure"),
            Self::Suspension => w.tag("Suspension"),
            Self::Retardation => w.tag("Retardation"),
            Self::Anticipation => w.tag("Anticipation"),
            Self::Appoggiatura => w.tag("Appoggiatura"),
            Self::SlidePath => w.tag("SlidePath"),
            Self::ModalColor => w.tag("ModalColor"),
        }
    }
}

structure!(context::PitchPalette, "context/PitchPalette/v2";
    chord_tones => "chord_tones",
    guide_tones => "guide_tones",
    tensions => "tensions",
    color => "color",
    expensive => "expensive",
    next_targets => "next_targets",
    scale => "scale",
);

structure!(score::Provenance, "score/Provenance/v2";
    section => "section",
    phrase => "phrase",
    family => "family",
    anchor => "anchor",
    role_kind => "role_kind",
    motif_id => "motif_id",
    motif_xform => "motif_xform",
    groove_variation => "groove_variation",
    role => "role",
    closure => "closure",
    morphism => "morphism",
    role_note => "role_note",
    actions => "actions",
    interaction => "interaction",
    material => "material",
    obligation => "obligation",
);

impl CanonicalFingerprint for theory::Quality {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("theory/Quality/v2");
        match self {
            Self::Maj => w.tag("Maj"),
            Self::Min => w.tag("Min"),
            Self::Dim => w.tag("Dim"),
            Self::Aug => w.tag("Aug"),
            Self::Maj7 => w.tag("Maj7"),
            Self::Min7 => w.tag("Min7"),
            Self::Dom7 => w.tag("Dom7"),
            Self::Min7b5 => w.tag("Min7b5"),
            Self::Dim7 => w.tag("Dim7"),
            Self::MinMaj7 => w.tag("MinMaj7"),
            Self::Sus4 => w.tag("Sus4"),
            Self::Sus2 => w.tag("Sus2"),
            Self::Maj9 => w.tag("Maj9"),
            Self::Min9 => w.tag("Min9"),
            Self::Dom9 => w.tag("Dom9"),
            Self::Add9 => w.tag("Add9"),
            Self::Maj6 => w.tag("Maj6"),
            Self::Min6 => w.tag("Min6"),
        }
    }
}

impl CanonicalFingerprint for region::RegionKind {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("region/RegionKind/v2");
        match self {
            Self::Home => w.tag("Home"),
            Self::Modulated { from } => {
                w.tag("Modulated");
                w.field("from", from);
            }
            Self::Pivot { from, to } => {
                w.tag("Pivot");
                w.field("from", from);
                w.field("to", to);
            }
            Self::Return { to } => {
                w.tag("Return");
                w.field("to", to);
            }
        }
    }
}

structure!(region::RegionSpan, "region/RegionSpan/v2";
    start_beat => "start_beat",
    end_beat => "end_beat",
    scale => "scale",
    kind => "kind",
    cause => "cause",
    established => "established",
);

structure!(region::RegionTimeline, "region/RegionTimeline/v2";
    home => "home",
    spans => "spans",
    relabels => "relabels",
);

structure!(region::Relabel, "region/Relabel/v2";
    action => "action",
    from => "from",
    to => "to",
    reason => "reason",
);

impl CanonicalFingerprint for contract::ResolutionPolicy {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("contract/ResolutionPolicy/v2");
        match self {
            Self::Functional => w.tag("Functional"),
            Self::Loop => w.tag("Loop"),
            Self::ModalPedal => w.tag("ModalPedal"),
        }
    }
}

structure!(interaction::Response, "interaction/Response/v2";
    action => "action",
    responder => "responder",
    start_beat => "start_beat",
    dur_beats => "dur_beats",
    latency => "latency",
    overlap => "overlap",
    transform => "transform",
    crosses_chord => "crosses_chord",
    material => "material",
    realizes => "realizes",
);

impl CanonicalFingerprint for interaction::ResponseMode {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("interaction/ResponseMode/v2");
        match self {
            Self::Free => w.tag("Free"),
            Self::Clockwork => w.tag("Clockwork"),
        }
    }
}

impl CanonicalFingerprint for score::Role {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("score/Role/v2");
        match self {
            Self::Pad => w.tag("Pad"),
            Self::Bass => w.tag("Bass"),
            Self::Lead => w.tag("Lead"),
            Self::Keys => w.tag("Keys"),
        }
    }
}

structure!(theory::Scale, "theory/Scale/v2";
    tonic_pc => "tonic_pc",
    mode => "mode",
);

structure!(ensemble::Seat, "ensemble/Seat/v2";
    on => "on",
    gain => "gain",
    role => "role",
);

structure!(form::Section, "form/Section/v2";
    kind => "kind",
    start_bar => "start_bar",
    bars => "bars",
    energy => "energy",
    tension => "tension",
    density => "density",
);

impl CanonicalFingerprint for plan::SectionFamily {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("plan/SectionFamily/v2");
        match self {
            Self::Intro => w.tag("Intro"),
            Self::A => w.tag("A"),
            Self::APrime { base } => {
                w.tag("APrime");
                w.field("base", base);
            }
            Self::B => w.tag("B"),
            Self::Break => w.tag("Break"),
            Self::Climax => w.tag("Climax"),
            Self::Coda => w.tag("Coda"),
            Self::Named { identity } => {
                w.tag("Named");
                w.field("identity", identity);
            }
        }
    }
}

impl CanonicalFingerprint for form::SectionKind {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("form/SectionKind/v2");
        match self {
            Self::Intro => w.tag("Intro"),
            Self::A => w.tag("A"),
            Self::Development => w.tag("Development"),
            Self::Climax => w.tag("Climax"),
            Self::Contrast => w.tag("Contrast"),
            Self::Coda => w.tag("Coda"),
        }
    }
}

structure!(backbone::SemanticBinding, "backbone/SemanticBinding/v2";
    transition => "transition",
    at_beat => "at_beat",
    bar => "bar",
    event => "event",
    gesture => "gesture",
    morphisms => "morphisms",
);

structure!(semantic::SemanticState, "semantic/SemanticState/v2";
    tone => "tone",
    emphasis => "emphasis",
    density => "density",
    elevation => "elevation",
);

impl CanonicalFingerprint for discourse::SettleHow {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("discourse/SettleHow/v2");
        match self {
            Self::Paid => w.tag("Paid"),
            Self::Deflected => w.tag("Deflected"),
        }
    }
}

structure!(discourse::Settlement, "discourse/Settlement/v2";
    by_phrase => "by_phrase",
    how => "how",
    witness => "witness",
);

structure!(score::SfxEvent, "score/SfxEvent/v2";
    start_beat => "start_beat",
    kind => "kind",
    velocity => "velocity",
    prov => "prov",
    pitches => "pitches",
    function => "function",
    owned_by => "owned_by",
    dissonance_beats => "dissonance_beats",
);

impl CanonicalFingerprint for score::SfxKind {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("score/SfxKind/v2");
        match self {
            Self::Acquire => w.tag("Acquire"),
            Self::Confirm => w.tag("Confirm"),
            Self::Warning => w.tag("Warning"),
            Self::Danger => w.tag("Danger"),
            Self::Transition => w.tag("Transition"),
            Self::Impact => w.tag("Impact"),
        }
    }
}

structure!(ensemble::Stage, "ensemble/Stage/v2";
    seats => "seats",
    windows => "windows",
);

structure!(ensemble::StageWindow, "ensemble/StageWindow/v2";
    agent => "agent",
    start_beat => "start_beat",
    end_beat => "end_beat",
    action => "action",
);

structure!(action::StasisSpan, "action/StasisSpan/v2";
    start_beat => "start_beat",
    end_beat => "end_beat",
    reason => "reason",
);

structure!(performance::StepWeight, "performance/StepWeight/v2";
    structural => "structural",
    backbeat => "backbeat",
    syncopation => "syncopation",
    pickup => "pickup",
    push => "push",
    hole => "hole",
    hit => "hit",
);

structure!(context::TensionVector, "context/TensionVector/v2";
    pull => "pull",
    distance => "distance",
    color => "color",
    strain => "strain",
    surprise => "surprise",
    openness => "openness",
);

structure!(song::ThematicMap, "song/ThematicMap/v2";
    bank => "bank",
    sites => "sites",
);

structure!(song::ThemeSite, "song/ThemeSite/v2";
    phrase => "phrase",
    role => "role",
    motif => "motif",
    handoff => "handoff",
);

structure!(backbone::TimeScales, "backbone/TimeScales/v2";
    beats_per_bar => "beats_per_bar",
    phrase_bars => "phrase_bars",
    binding => "binding",
);

impl CanonicalFingerprint for semantic::Tone {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("semantic/Tone/v2");
        match self {
            Self::Neutral => w.tag("Neutral"),
            Self::Accent => w.tag("Accent"),
            Self::Info => w.tag("Info"),
            Self::Success => w.tag("Success"),
            Self::Warning => w.tag("Warning"),
            Self::Danger => w.tag("Danger"),
        }
    }
}

impl CanonicalFingerprint for interaction::Transform {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("interaction/Transform/v2");
        match self {
            Self::Quote => w.tag("Quote"),
            Self::Complete => w.tag("Complete"),
            Self::Invert => w.tag("Invert"),
            Self::Compress => w.tag("Compress"),
            Self::Echo => w.tag("Echo"),
            Self::Silence => w.tag("Silence"),
        }
    }
}

impl CanonicalFingerprint for interaction::Verdict {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("interaction/Verdict/v2");
        match self {
            Self::Call => w.tag("Call"),
            Self::StandsAlone(value0) => {
                w.tag("StandsAlone");
                w.field("0", value0);
            }
        }
    }
}

impl CanonicalFingerprint for ids::ActionStamp {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("ids/ActionStamp/v2");
        self.iter().collect::<Vec<_>>().encode(w);
    }
}

impl CanonicalFingerprint for ids::ActionId {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("ids/ActionId/v2");
        self.0.encode(w);
    }
}

impl CanonicalFingerprint for ids::InteractionId {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("ids/InteractionId/v2");
        self.0.encode(w);
    }
}

impl CanonicalFingerprint for ids::MaterialId {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("ids/MaterialId/v2");
        self.0.encode(w);
    }
}

impl CanonicalFingerprint for ids::ObligationId {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("ids/ObligationId/v2");
        self.0.encode(w);
    }
}

impl CanonicalFingerprint for song::SongMap {
    fn encode(&self, w: &mut FingerprintWriter) {
        let Self {
            trace: _,
            seed: _,
            frame,
            timeline,
            plan,
            thematic,
            harmonic,
            meaning,
            phenomenal,
        } = self;
        w.tag("song/SongMap/v2");
        w.field("frame", frame);
        w.field("timeline", timeline);
        w.field("contract", &plan.contract);
        w.field("form", &plan.form);
        w.field("discourse", &plan.discourse);
        w.field("arrangement", &plan.arrangement);
        w.field("backbone", &plan.backbone);
        w.field("thematic", thematic);
        w.field("harmonic", harmonic);
        w.field("meaning", meaning);
        w.field("phenomenal", phenomenal);
    }
}
// Event identity: render graph and performance evidence have separate schemas.
structure!(score::Score, "score/events/v2";
    notes => "notes", drums => "drums", sfx => "sfx", chords => "chords",
    sections => "sections", tempo_bpm => "tempo_bpm", beats_per_bar => "beats_per_bar",
    total_beats => "total_beats", melody_repairs => "melody_repairs",
    melody_rejudged => "melody_rejudged",
    ; ignore observed_lifetime, mono_voice, voice_continuity, vertical_decisions, support_report,
      vertical_repairs, tension_edits, pad_voicing_edits, hearings,
      expression_decisions, occupancy, phrase_plans, support_voicing_decisions
);

macro_rules! legacy_alias {
    ($($ty:path),+ $(,)?) => {$ (
        impl $ty {
            /// Historical Debug/FNV receipt, preserved byte for byte. For canonical
            /// semantic encoding use `CanonicalFingerprint::canonical_fingerprint`.
            pub fn legacy_fingerprint(&self) -> u64 { self.fingerprint() }
        }
    )+};
}
legacy_alias!(
    song::SongMap,
    song::ThematicMap,
    song::HarmonicMap,
    score::Score
);

macro_rules! policy_enum {
    ($ty:path, $tag:literal; $($variant:ident => $discriminant:literal),+ $(,)?) => {
        impl CanonicalFingerprint for $ty {
            fn encode(&self, w: &mut FingerprintWriter) {
                w.tag($tag);
                w.tag(match self { $(Self::$variant => $discriminant),+ });
            }
        }
    };
}
policy_enum!(policy::PitchPolicy, "policy/Pitch/v2"; Written => "written", Temporal => "temporal");
policy_enum!(policy::ContinuationAdmission, "policy/ContinuationAdmission/v2";
    ReleaseEnvelope => "envelope", ExplicitContinuation => "explicit");
policy_enum!(policy::OccupancyPolicy, "policy/Occupancy/v2";
    Acoustic => "acoustic", AuthoredIntent => "authored");
policy_enum!(policy::SupportPolicy, "policy/Support/v2";
    Independent => "independent", HeardHarmony => "heard", SourceVoicePath => "voice-path");
policy_enum!(policy::VoiceLifetimePolicy, "policy/VoiceLifetime/v2";
    ReleaseEnvelope => "envelope", ExplicitContinuations => "explicit");
policy_enum!(policy::SourceEvidencePolicy, "policy/SourceEvidence/v2";
    Events => "events", AuthoredSources => "authored");
policy_enum!(policy::HistoricalRepair, "policy/HistoricalRepair/v2";
    None => "none", SupportMass => "support-mass", SoundingTension => "sounding-tension");
structure!(policy::PulsePolicy, "policy/Pulse/v2";
    lattice_positions => "lattice_positions", legato_connectives => "legato_connectives",
    continuation_admission => "continuation_admission", stable_precursors => "stable_precursors",
);
impl CanonicalFingerprint for policy::ExpressionPolicy {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("policy/Expression/v2");
        match self {
            Self::Unchanged => w.tag("unchanged"),
            Self::LocalConnectives => w.tag("local-connectives"),
            Self::Phrase => w.tag("phrase"),
            Self::Pulse(pulse) => {
                w.tag("pulse");
                w.field("pulse", pulse);
            }
        }
    }
}
// Historical laws keep the exact v2 encoding; a non-historical admission (or later
// law) is an explicitly tagged extension, so no historical profile's receipt drifts.
impl CanonicalFingerprint for policy::PerformanceProfile {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("policy/PerformanceProfile/v2");
        // Exhaustive destructuring makes a newly added field a schema-review error.
        let Self {
            pitch,
            expression,
            occupancy,
            support,
            lifetime,
            observation,
            evidence,
            admission,
            repair,
        } = self;
        w.field("pitch", pitch);
        w.field("expression", expression);
        w.field("occupancy", occupancy);
        w.field("support", support);
        w.field("lifetime", lifetime);
        w.field("observation", observation);
        w.field("evidence", evidence);
        w.field("repair", repair);
        if *admission != policy::ActionAdmission::Planned {
            w.field("admission", admission);
        }
    }
}
impl CanonicalFingerprint for policy::ActionAdmission {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("policy/ActionAdmission/v1");
        match self {
            Self::Planned => w.tag("planned"),
            Self::Rehearsed => w.tag("rehearsed"),
        }
    }
}

// The legacy float-normalized shape is a diagnostic descriptor. The wrapper retains
// pitch units; exact cover rhythm uses rational metric positions instead.
structure!(motif::MotifIdentity, "motif/LegacyShape/v2";
    interval_contour => "interval_contour", rhythmic_profile => "rhythmic_profile",
    direction_signature => "direction_signature",
);
structure!(motif::TypedMotifIdentity, "motif/TypedShape/v2";
    pitch_basis => "pitch_basis", shape => "shape",
);
