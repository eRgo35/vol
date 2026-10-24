//! Messages between the app and the audio backend (`docs/specs/01-architecture.md` §3.1, §4).

pub mod coalesce;

use std::time::Duration;

use crate::model::{
    ActiveRoute, NodeAudio, NodeState, ObjectId, PortDir, Profile, PropMap, RouteInfo, Serial,
};

/// Backend → app.
#[derive(Debug, Clone, PartialEq)]
pub enum BackendEvent {
    /// Initial enumeration finished (core sync "done" received after all binds). Sent once per connection.
    Connected {
        /// Daemon version string.
        server_version: String,
        /// Socket the backend connected to.
        socket: String,
    },
    /// Connection lost or never established. The app MUST clear the store on receipt.
    Disconnected {
        /// Why the connection is down.
        reason: DisconnectReason,
        /// Delay before the next attempt.
        retry_in: Option<Duration>,
        /// Attempt counter (resets on `Connected`).
        attempt: u32,
    },
    /// Coalesced registry/param changes. Order within the `Vec` is application order.
    Batch(Vec<StoreEvent>),
    /// Peak levels, at most one message per 40 ms (25 Hz). Linear 0.0..=1.0+ (not dB).
    Meters(Vec<MeterSample>),
    /// A previously sent command could not be applied.
    CommandFailed {
        /// Which command failed.
        cmd: CommandKind,
        /// Why.
        error: BackendError,
    },
}

/// Why the backend is disconnected.
#[derive(Debug, Clone, PartialEq)]
pub enum DisconnectReason {
    /// No daemon (`ENOENT`/`ECONNREFUSED`).
    NoDaemon,
    /// `EPIPE` or core error id 0.
    ConnectionLost,
    /// Not allowed to connect.
    PermissionDenied,
}

/// Peak level of one meter target.
#[derive(Debug, Clone, PartialEq)]
pub struct MeterSample {
    /// Metered node.
    pub target: Serial,
    /// Max `|x|` over the window, all channels.
    pub peak: f32,
}

/// Reducer input; produced only by the backend, consumed only by `Store::apply`.
#[derive(Debug, Clone, PartialEq)]
pub enum StoreEvent {
    /// A node appeared.
    NodeAdded {
        /// Registry id.
        id: ObjectId,
        /// `object.serial`.
        serial: Serial,
        /// Properties.
        props: PropMap,
    },
    /// Node info changed; full replacement of props.
    NodeInfo {
        /// Registry id.
        id: ObjectId,
        /// Properties.
        props: PropMap,
        /// `info.state`.
        state: NodeState,
    },
    /// From the node's `Props` param.
    NodeAudio {
        /// Registry id.
        id: ObjectId,
        /// Volume state.
        audio: NodeAudio,
    },
    /// From the node's `ProcessLatency` param.
    NodeLatency {
        /// Registry id.
        id: ObjectId,
        /// Latency in nanoseconds.
        process_latency_ns: i64,
    },
    /// A node disappeared.
    NodeRemoved {
        /// Registry id.
        id: ObjectId,
    },

    /// A device appeared.
    DeviceAdded {
        /// Registry id.
        id: ObjectId,
        /// `object.serial`.
        serial: Serial,
        /// Properties.
        props: PropMap,
    },
    /// Device info changed.
    DeviceInfo {
        /// Registry id.
        id: ObjectId,
        /// Properties.
        props: PropMap,
    },
    /// `EnumProfile`.
    DeviceProfiles {
        /// Registry id.
        id: ObjectId,
        /// All profiles.
        profiles: Vec<Profile>,
    },
    /// `Profile`.
    DeviceProfile {
        /// Registry id.
        id: ObjectId,
        /// Active profile index.
        index: i32,
    },
    /// `EnumRoute`.
    DeviceRoutes {
        /// Registry id.
        id: ObjectId,
        /// All routes.
        routes: Vec<RouteInfo>,
    },
    /// `Route`.
    DeviceActive {
        /// Registry id.
        id: ObjectId,
        /// Active routes.
        active: Vec<ActiveRoute>,
    },
    /// A device disappeared.
    DeviceRemoved {
        /// Registry id.
        id: ObjectId,
    },

    /// A port appeared.
    PortAdded {
        /// Registry id.
        id: ObjectId,
        /// Owning node.
        node: ObjectId,
        /// Direction.
        dir: PortDir,
        /// `audio.channel`.
        channel: Option<String>,
        /// `port.monitor`.
        monitor: bool,
    },
    /// A port disappeared.
    PortRemoved {
        /// Registry id.
        id: ObjectId,
    },
    /// A link appeared.
    LinkAdded {
        /// Registry id.
        id: ObjectId,
        /// Output node.
        out_node: ObjectId,
        /// Output port.
        out_port: ObjectId,
        /// Input node.
        in_node: ObjectId,
        /// Input port.
        in_port: ObjectId,
    },
    /// A link disappeared.
    LinkRemoved {
        /// Registry id.
        id: ObjectId,
    },

    /// "default" metadata only. `value` `None` = key removed. `type_` as delivered
    /// (e.g. `"Spa:String:JSON"`, `"Spa:Id"`).
    Metadata {
        /// Subject id (0 for global keys, the stream node id for targets).
        subject: u32,
        /// Key.
        key: String,
        /// Value type.
        type_: Option<String>,
        /// Value.
        value: Option<String>,
    },
    /// The "default" metadata object disappeared.
    MetadataCleared,
}

/// App → backend.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Write `Props.channelVolumes` (per-channel gains, already cubed and divided by
    /// `Props.volume`; see `docs/specs/02-playback.md` §2.1).
    /// `master = Some(v)` additionally writes `Props.volume = v` (only used to reset a zero master to 1.0).
    SetVolumes {
        /// Target node.
        node: ObjectId,
        /// Linear gain per channel.
        channel_volumes: Vec<f32>,
        /// Optional master gain.
        master: Option<f32>,
    },
    /// Write `Props.mute`.
    SetMute {
        /// Target node.
        node: ObjectId,
        /// New mute state.
        mute: bool,
    },
    /// Write metadata "default" key `default.configured.audio.{sink|source}` = `{"name": node_name}`.
    SetDefault {
        /// Sink or source.
        kind: DefaultKind,
        /// `node.name` of the new default.
        node_name: String,
    },
    /// `target = None` clears `target.object`/`target.node` for the stream (follow default policy).
    MoveStream {
        /// Stream node.
        stream: ObjectId,
        /// New target node.
        target: Option<ObjectId>,
    },
    /// Select a device profile.
    SetProfile {
        /// Target device.
        device: ObjectId,
        /// Profile index.
        index: i32,
    },
    /// Select a device route (port).
    SetRoute {
        /// Target device.
        device: ObjectId,
        /// Route index.
        route_index: i32,
        /// Card profile device the route applies to.
        card_profile_device: i32,
    },
    /// Idempotent: backend diffs against running meters. Replaces the full desired set.
    SetMeters {
        /// Desired meters.
        targets: Vec<MeterTarget>,
    },
    /// Tear down and stop the backend.
    Shutdown,
}

impl Command {
    /// The kind reported in `CommandFailed`; `None` for `SetMeters` and `Shutdown`,
    /// which never fail that way.
    pub fn kind(&self) -> Option<CommandKind> {
        match self {
            Self::SetVolumes { .. } => Some(CommandKind::SetVolumes),
            Self::SetMute { .. } => Some(CommandKind::SetMute),
            Self::SetDefault { .. } => Some(CommandKind::SetDefault),
            Self::MoveStream { .. } => Some(CommandKind::MoveStream),
            Self::SetProfile { .. } => Some(CommandKind::SetProfile),
            Self::SetRoute { .. } => Some(CommandKind::SetRoute),
            Self::SetMeters { .. } | Self::Shutdown => None,
        }
    }
}

/// Which default to set.
#[derive(Debug, Clone, PartialEq)]
pub enum DefaultKind {
    /// Default output.
    Sink,
    /// Default input.
    Source,
}

/// A node to meter.
#[derive(Debug, Clone, PartialEq)]
pub struct MeterTarget {
    /// Registry id.
    pub node: ObjectId,
    /// `object.serial`.
    pub serial: Serial,
    /// How to capture it.
    pub kind: MeterKind,
}

/// How a meter captures its target.
#[derive(Debug, Clone, PartialEq)]
pub enum MeterKind {
    /// Monitor of a sink.
    SinkMonitor,
    /// A source.
    Source,
    /// Output of a playback stream.
    StreamOutput,
}

/// The kind of a command that can fail (reported in `BackendEvent::CommandFailed`).
#[derive(Debug, Clone, PartialEq)]
pub enum CommandKind {
    /// `Command::SetVolumes`.
    SetVolumes,
    /// `Command::SetMute`.
    SetMute,
    /// `Command::SetDefault`.
    SetDefault,
    /// `Command::MoveStream`.
    MoveStream,
    /// `Command::SetProfile`.
    SetProfile,
    /// `Command::SetRoute`.
    SetRoute,
}

/// Why a command failed.
#[derive(Debug, Clone, PartialEq)]
pub enum BackendError {
    /// No connection to the daemon.
    NotConnected,
    /// The node no longer exists.
    NodeGone(ObjectId),
    /// The device no longer exists.
    DeviceGone(ObjectId),
    /// The daemon refused the operation.
    PermissionDenied,
    /// Other PipeWire error: errno and message.
    Pipewire(i32, String),
}

/// The backend thread is gone; commands can no longer be delivered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendGone;

/// Command sink of a backend.
pub trait AudioBackend: Send {
    /// Non-blocking. `Err` only if the backend thread is gone.
    fn send(&self, cmd: Command) -> Result<(), BackendGone>;
}

/// The app's connection to a backend.
pub struct BackendHandle {
    /// Where commands go.
    pub commands: Box<dyn AudioBackend>,
    /// Where events come from.
    pub events: tokio::sync::mpsc::UnboundedReceiver<BackendEvent>,
}

impl BackendHandle {
    /// A backend that never emits an event and accepts (and drops) every command.
    pub fn inert() -> Self {
        let (sender, events) = tokio::sync::mpsc::unbounded_channel();
        Self {
            commands: Box::new(InertBackend { _open: sender }),
            events,
        }
    }
}

/// Command sink of [`BackendHandle::inert`].
struct InertBackend {
    /// Never used to send; held so the event channel stays open and `recv` pends forever.
    _open: tokio::sync::mpsc::UnboundedSender<BackendEvent>,
}

impl AudioBackend for InertBackend {
    fn send(&self, _cmd: Command) -> Result<(), BackendGone> {
        Ok(())
    }
}

#[cfg(any(test, feature = "mock"))]
pub mod mock;
pub mod pw;

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::error::TryRecvError;

    #[tokio::test]
    async fn inert_backend_never_emits_and_accepts_commands() {
        let mut handle = BackendHandle::inert();
        assert_eq!(handle.commands.send(Command::Shutdown), Ok(()));
        assert_eq!(
            handle.commands.send(Command::SetMute {
                node: ObjectId(1),
                mute: true,
            }),
            Ok(())
        );
        assert_eq!(handle.events.try_recv(), Err(TryRecvError::Empty));
        let waited = tokio::time::timeout(Duration::from_millis(20), handle.events.recv()).await;
        assert!(waited.is_err(), "inert backend yielded {waited:?}");
    }
}
