//! Audio nodes: sinks, sources and streams.

use super::{ObjectId, PropMap, Serial};

/// An audio node the app tracks.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Registry id.
    pub id: ObjectId,
    /// `object.serial`.
    pub serial: Serial,
    /// Role derived from `media.class`.
    pub kind: NodeKind,
    /// Properties, verbatim.
    pub props: PropMap,
    /// `info.state`.
    pub state: NodeState,
    /// From the `Props` param; `None` until it arrives.
    pub audio: Option<NodeAudio>,
    /// From the `ProcessLatency` param.
    pub process_latency_ns: Option<i64>,
}

/// Node role. Other media classes are not stored.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    /// `Audio/Sink*`.
    Sink,
    /// `Audio/Source*`.
    Source,
    /// `Stream/Output/Audio`.
    PlaybackStream,
    /// `Stream/Input/Audio`.
    CaptureStream,
}

/// `info.state` of a node.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeState {
    /// `creating`.
    Creating,
    /// `suspended`.
    Suspended,
    /// `idle`.
    Idle,
    /// `running`.
    Running,
    /// `error`.
    Error,
}

/// Volume state from the node's `Props` param.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeAudio {
    /// `Props.volume` (master gain multiplier), default 1.0.
    pub volume: f32,
    /// `Props.channelVolumes`, linear gain per channel.
    pub channel_volumes: Vec<f32>,
    /// `Props.mute`.
    pub mute: bool,
    /// `Props.channelMap`.
    pub channel_map: Vec<Channel>,
}

/// One `Props.channelMap` element: a raw `spa_audio_channel` id
/// (display names: `docs/specs/02-playback.md` §2.2).
#[derive(Debug, Clone, PartialEq)]
pub struct Channel(pub u32);
