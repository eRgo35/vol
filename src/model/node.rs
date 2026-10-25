//! Audio nodes: sinks, sources and streams.

use super::volume::{gain_to_level, level_to_percent};
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

impl NodeKind {
    /// Classifies `props["media.class"]` (`docs/specs/01-architecture.md` §5): prefix
    /// `Audio/Sink` or `Audio/Source`, exact `Stream/Output/Audio` or `Stream/Input/Audio`;
    /// anything else (including `Audio/Duplex`) is `None` and is not stored.
    pub fn from_media_class(class: &str) -> Option<Self> {
        match class {
            "Stream/Output/Audio" => Some(Self::PlaybackStream),
            "Stream/Input/Audio" => Some(Self::CaptureStream),
            c if c.starts_with("Audio/Sink") => Some(Self::Sink),
            c if c.starts_with("Audio/Source") => Some(Self::Source),
            _ => None,
        }
    }
}

impl NodeState {
    /// Parses `info.state`; unknown strings are `None`.
    pub fn from_info(state: &str) -> Option<Self> {
        match state {
            "creating" => Some(Self::Creating),
            "suspended" => Some(Self::Suspended),
            "idle" => Some(Self::Idle),
            "running" => Some(Self::Running),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

impl NodeAudio {
    /// Effective linear gain per channel: `volume × channelVolumes[i]` (VOL-VM-001).
    pub fn effective_gains(&self) -> Vec<f32> {
        self.channel_volumes
            .iter()
            .map(|g| self.volume * g)
            .collect()
    }

    /// Master gain: the largest effective channel gain; `None` without channels (VOL-VM-002).
    pub fn master_gain(&self) -> Option<f32> {
        self.effective_gains().into_iter().reduce(f32::max)
    }

    /// Master level `∛(max_i g_i)`; `None` without channels (VOL-VM-002).
    pub fn master_level(&self) -> Option<f32> {
        self.master_gain().map(gain_to_level)
    }

    /// Displayed master percent; above 150 % it is the true value (VOL-VM-003).
    pub fn master_percent(&self) -> Option<i32> {
        self.master_level().map(level_to_percent)
    }

    /// Displayed percent per channel, in `channelVolumes` order.
    pub fn channel_percents(&self) -> Vec<i32> {
        self.effective_gains()
            .into_iter()
            .map(|g| level_to_percent(gain_to_level(g)))
            .collect()
    }
}

impl Channel {
    /// Wraps a raw `spa_audio_channel` id.
    pub fn from_spa_id(id: u32) -> Self {
        Self(id)
    }

    /// Display name (`docs/specs/02-playback.md` §2.2). `position` is the 0-based index in
    /// the channel map; ids without a table entry are named `C<position + 1>`.
    pub fn short_name(&self, position: usize) -> String {
        let name = match self.0 {
            2 => "MON",
            3 => "FL",
            4 => "FR",
            5 => "FC",
            6 => "LFE",
            7 => "SL",
            8 => "SR",
            9 => "FLC",
            10 => "FRC",
            11 => "RC",
            12 => "RL",
            13 => "RR",
            14 => "TC",
            _ => return format!("C{}", position + 1),
        };
        name.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_from_media_class_table() {
        let table = [
            ("Audio/Sink", Some(NodeKind::Sink)),
            ("Audio/Sink/Virtual", Some(NodeKind::Sink)),
            ("Audio/Source", Some(NodeKind::Source)),
            ("Audio/Source/Virtual", Some(NodeKind::Source)),
            ("Stream/Output/Audio", Some(NodeKind::PlaybackStream)),
            ("Stream/Input/Audio", Some(NodeKind::CaptureStream)),
            ("Stream/Output/Video", None),
            ("Stream/Input/Video", None),
            ("Midi/Bridge", None),
            ("Audio/Duplex", None),
            ("Audio/Device", None),
            ("", None),
        ];
        for (class, kind) in table {
            assert_eq!(NodeKind::from_media_class(class), kind, "{class:?}");
        }
    }

    #[test]
    fn node_state_from_info_strings() {
        assert_eq!(NodeState::from_info("creating"), Some(NodeState::Creating));
        assert_eq!(
            NodeState::from_info("suspended"),
            Some(NodeState::Suspended)
        );
        assert_eq!(NodeState::from_info("idle"), Some(NodeState::Idle));
        assert_eq!(NodeState::from_info("running"), Some(NodeState::Running));
        assert_eq!(NodeState::from_info("error"), Some(NodeState::Error));
        assert_eq!(NodeState::from_info(""), None);
        assert_eq!(NodeState::from_info("Running"), None);
    }

    #[test]
    fn channel_names_for_known_ids() {
        let names = [
            "MON", "FL", "FR", "FC", "LFE", "SL", "SR", "FLC", "FRC", "RC", "RL", "RR", "TC",
        ];
        for (id, name) in (2..=14).zip(names) {
            assert_eq!(Channel::from_spa_id(id).short_name(0), name, "id {id}");
        }
    }

    #[test]
    fn unknown_channel_is_c_plus_position() {
        for id in [0, 1, 15, u32::MAX] {
            assert_eq!(Channel::from_spa_id(id).short_name(4), "C5", "id {id}");
        }
        assert_eq!(Channel::from_spa_id(0).short_name(0), "C1");
    }
}
