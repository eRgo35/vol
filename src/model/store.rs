//! The registry mirror and its reducer (`docs/specs/01-architecture.md` §5).

use std::collections::BTreeMap;
use std::time::Instant;

use super::{
    Defaults, Device, Link, Node, NodeKind, NodeState, ObjectId, Port, StreamTarget,
    is_audio_device, parse_default_name,
};
use crate::backend::{DisconnectReason, StoreEvent};

/// Connection state shown by the UI.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ConnState {
    /// Before the first `Connected`/`Disconnected`.
    #[default]
    Connecting,
    /// Initial snapshot received.
    Connected {
        /// Daemon version string.
        server_version: String,
    },
    /// No connection; the backend retries.
    Disconnected {
        /// Why.
        reason: DisconnectReason,
        /// When the next attempt happens.
        retry_at: Instant,
        /// Attempt counter.
        attempt: u32,
    },
}

/// The only holder of PipeWire-derived state (VOL-ARCH-020).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Store {
    /// Audio nodes the app tracks (see [`NodeKind`]).
    pub nodes: BTreeMap<ObjectId, Node>,
    /// `Audio/Device` devices.
    pub devices: BTreeMap<ObjectId, Device>,
    /// Ports of stored nodes.
    pub ports: BTreeMap<ObjectId, Port>,
    /// Links between stored nodes.
    pub links: BTreeMap<ObjectId, Link>,
    /// Default sink/source names.
    pub defaults: Defaults,
    /// Routing targets keyed by stream node id.
    pub stream_targets: BTreeMap<ObjectId, StreamTarget>,
    /// Connection state.
    pub conn: ConnState,
    /// +1 per applied change; the UI redraws iff it changed.
    pub revision: u64,
}

/// Stores `value` in `slot`; true iff it differs from what was there.
fn replace<T: PartialEq>(slot: &mut T, value: T) -> bool {
    if *slot == value {
        return false;
    }
    *slot = value;
    true
}

/// Inserts `value` at `id`; true iff the map did not already hold an equal value.
fn upsert<T: PartialEq>(map: &mut BTreeMap<ObjectId, T>, id: ObjectId, value: T) -> bool {
    if map.get(&id) == Some(&value) {
        return false;
    }
    map.insert(id, value);
    true
}

impl Store {
    /// Applies one event; returns `true` iff observable state changed, and then bumps
    /// `revision` (VOL-ARCH-021). Never panics; events for unknown ids are dropped.
    pub fn apply(&mut self, event: StoreEvent) -> bool {
        let changed = self.reduce(event);
        self.bump(changed)
    }

    /// Empties every map, the defaults and the stream targets; keeps `conn` (VOL-ARCH-027).
    /// Returns `true` iff anything was removed.
    pub fn clear(&mut self) -> bool {
        let cleared = Self {
            conn: self.conn.clone(),
            revision: self.revision,
            ..Self::default()
        };
        let changed = replace(self, cleared);
        self.bump(changed)
    }

    /// Sets the connection state; returns `true` (and bumps `revision`) iff it changed.
    pub fn set_conn(&mut self, conn: ConnState) -> bool {
        let changed = replace(&mut self.conn, conn);
        self.bump(changed)
    }

    fn bump(&mut self, changed: bool) -> bool {
        if changed {
            self.revision += 1;
        }
        changed
    }

    fn reduce(&mut self, event: StoreEvent) -> bool {
        match event {
            StoreEvent::NodeAdded { id, serial, props } => {
                let Some(kind) = props
                    .get("media.class")
                    .and_then(|class| NodeKind::from_media_class(class))
                else {
                    return false;
                };
                if let Some(node) = self.nodes.get(&id)
                    && node.serial == serial
                    && node.kind == kind
                    && node.props == props
                {
                    return false;
                }
                let node = Node {
                    id,
                    serial,
                    kind,
                    props,
                    state: NodeState::Creating,
                    audio: None,
                    process_latency_ns: None,
                };
                self.nodes.insert(id, node);
                true
            }
            StoreEvent::NodeInfo { id, props, state } => {
                self.nodes.get_mut(&id).is_some_and(|node| {
                    let props_changed = replace(&mut node.props, props);
                    replace(&mut node.state, state) | props_changed
                })
            }
            StoreEvent::NodeAudio { id, audio } => self
                .nodes
                .get_mut(&id)
                .is_some_and(|node| replace(&mut node.audio, Some(audio))),
            StoreEvent::NodeLatency {
                id,
                process_latency_ns,
            } => self.nodes.get_mut(&id).is_some_and(|node| {
                replace(&mut node.process_latency_ns, Some(process_latency_ns))
            }),
            StoreEvent::NodeRemoved { id } => {
                if self.nodes.remove(&id).is_none() {
                    return false;
                }
                self.ports.retain(|_, port| port.node != id);
                self.links
                    .retain(|_, link| link.out_node != id && link.in_node != id);
                self.stream_targets.remove(&id);
                true
            }

            StoreEvent::DeviceAdded { id, serial, props } => {
                if !is_audio_device(&props) {
                    return false;
                }
                if let Some(device) = self.devices.get(&id)
                    && device.serial == serial
                    && device.props == props
                {
                    return false;
                }
                let device = Device {
                    id,
                    serial,
                    props,
                    profiles: Vec::new(),
                    active_profile: None,
                    routes: Vec::new(),
                    active_routes: Vec::new(),
                };
                self.devices.insert(id, device);
                true
            }
            StoreEvent::DeviceInfo { id, props } => self
                .devices
                .get_mut(&id)
                .is_some_and(|device| replace(&mut device.props, props)),
            StoreEvent::DeviceProfiles { id, profiles } => self
                .devices
                .get_mut(&id)
                .is_some_and(|device| replace(&mut device.profiles, profiles)),
            StoreEvent::DeviceProfile { id, index } => self
                .devices
                .get_mut(&id)
                .is_some_and(|device| replace(&mut device.active_profile, Some(index))),
            StoreEvent::DeviceRoutes { id, routes } => self
                .devices
                .get_mut(&id)
                .is_some_and(|device| replace(&mut device.routes, routes)),
            StoreEvent::DeviceActive { id, active } => self
                .devices
                .get_mut(&id)
                .is_some_and(|device| replace(&mut device.active_routes, active)),
            StoreEvent::DeviceRemoved { id } => self.devices.remove(&id).is_some(),

            StoreEvent::PortAdded {
                id,
                node,
                dir,
                channel,
                monitor,
            } => {
                if !self.nodes.contains_key(&node) {
                    return false;
                }
                let port = Port {
                    id,
                    node,
                    dir,
                    channel,
                    monitor,
                };
                upsert(&mut self.ports, id, port)
            }
            StoreEvent::PortRemoved { id } => self.ports.remove(&id).is_some(),
            StoreEvent::LinkAdded {
                id,
                out_node,
                out_port,
                in_node,
                in_port,
            } => {
                if !self.nodes.contains_key(&out_node) || !self.nodes.contains_key(&in_node) {
                    return false;
                }
                let link = Link {
                    id,
                    out_node,
                    out_port,
                    in_node,
                    in_port,
                };
                upsert(&mut self.links, id, link)
            }
            StoreEvent::LinkRemoved { id } => self.links.remove(&id).is_some(),

            StoreEvent::Metadata {
                subject,
                key,
                value,
                ..
            } => self.apply_metadata(subject, &key, value),
            StoreEvent::MetadataCleared => {
                let defaults_changed = replace(&mut self.defaults, Defaults::default());
                let targets_changed = !self.stream_targets.is_empty();
                self.stream_targets.clear();
                defaults_changed | targets_changed
            }
        }
    }

    /// Subject 0: the four `default.*` keys (VOL-ARCH-025). Other subjects: `target.object`
    /// and `target.node` of a stored stream node; `None` removes the key (VOL-ARCH-026).
    /// Everything else is ignored.
    fn apply_metadata(&mut self, subject: u32, key: &str, value: Option<String>) -> bool {
        if subject == 0 {
            return self
                .defaults
                .slot_mut(key)
                .is_some_and(|slot| replace(slot, value.as_deref().and_then(parse_default_name)));
        }
        let id = ObjectId(subject);
        if !self.nodes.contains_key(&id) {
            return false;
        }
        let mut target = self.stream_targets.get(&id).cloned().unwrap_or_default();
        let Some(slot) = target.slot_mut(key) else {
            return false;
        };
        *slot = value;
        if target == StreamTarget::default() {
            self.stream_targets.remove(&id).is_some()
        } else {
            upsert(&mut self.stream_targets, id, target)
        }
    }
}

#[cfg(test)]
pub(crate) mod strategies {
    //! Random `StoreEvent` sequences over a small id space, shared by the reducer and
    //! coalescing property tests.

    use std::collections::HashSet;

    use proptest::prelude::*;

    use crate::backend::StoreEvent;
    use crate::model::{
        ActiveRoute, Availability, Channel, NodeAudio, NodeState, ObjectId, PortDir, Profile,
        PropMap, RouteInfo, Serial,
    };

    fn id() -> impl Strategy<Value = ObjectId> {
        (1u32..=6).prop_map(ObjectId)
    }

    fn serial() -> impl Strategy<Value = Serial> {
        (100u64..=103).prop_map(Serial)
    }

    fn props() -> impl Strategy<Value = PropMap> {
        let class = prop::sample::select(vec![
            "Audio/Sink",
            "Audio/Source/Virtual",
            "Stream/Output/Audio",
            "Stream/Input/Audio",
            "Stream/Output/Video",
            "Audio/Duplex",
            "Audio/Device",
            "Midi/Bridge",
        ]);
        let name = prop::option::of(prop::sample::select(vec!["a", "b"]));
        (class, name).prop_map(|(class, name)| {
            let mut props = PropMap::from([("media.class".to_owned(), class.to_owned())]);
            if let Some(name) = name {
                props.insert("node.name".to_owned(), name.to_owned());
            }
            props
        })
    }

    fn state() -> impl Strategy<Value = NodeState> {
        prop::sample::select(vec![
            NodeState::Creating,
            NodeState::Suspended,
            NodeState::Idle,
            NodeState::Running,
            NodeState::Error,
        ])
    }

    fn audio() -> impl Strategy<Value = NodeAudio> {
        let gain = prop::sample::select(vec![0.0f32, 0.5, 1.0]);
        (
            gain.clone(),
            prop::collection::vec(gain, 0..3),
            any::<bool>(),
            prop::collection::vec((0u32..16).prop_map(Channel), 0..3),
        )
            .prop_map(|(volume, channel_volumes, mute, channel_map)| NodeAudio {
                volume,
                channel_volumes,
                mute,
                channel_map,
            })
    }

    fn availability() -> impl Strategy<Value = Availability> {
        (0u32..3).prop_map(Availability::from_spa)
    }

    fn dir() -> impl Strategy<Value = PortDir> {
        prop::sample::select(vec![PortDir::In, PortDir::Out])
    }

    fn profiles() -> impl Strategy<Value = Vec<Profile>> {
        let profile = (0i32..3, availability()).prop_map(|(index, available)| Profile {
            index,
            name: format!("p{index}"),
            description: String::new(),
            priority: index,
            available,
        });
        prop::collection::vec(profile, 0..3)
    }

    fn routes() -> impl Strategy<Value = Vec<RouteInfo>> {
        let route =
            (0i32..3, dir(), availability()).prop_map(|(index, dir, available)| RouteInfo {
                index,
                dir,
                name: format!("r{index}"),
                description: String::new(),
                priority: index,
                available,
                profiles: vec![index],
                devices: vec![index],
            });
        prop::collection::vec(route, 0..3)
    }

    fn active() -> impl Strategy<Value = Vec<ActiveRoute>> {
        let route =
            (0i32..3, 0i32..2, dir()).prop_map(|(index, card_profile_device, dir)| ActiveRoute {
                index,
                card_profile_device,
                dir,
            });
        prop::collection::vec(route, 0..3)
    }

    fn metadata() -> impl Strategy<Value = StoreEvent> {
        let key = prop::sample::select(vec![
            "default.audio.sink",
            "default.audio.source",
            "default.configured.audio.sink",
            "default.configured.audio.source",
            "target.object",
            "target.node",
            "other.key",
        ]);
        let type_ = prop::option::of(prop::sample::select(vec!["Spa:String:JSON", "Spa:Id"]));
        let value = prop::option::of(prop::sample::select(vec![
            r#"{"name":"a"}"#,
            r#"{"name":"b"}"#,
            r#"{"name":5}"#,
            "garbage",
            "42",
        ]));
        (0u32..=6, key, type_, value).prop_map(|(subject, key, type_, value)| {
            StoreEvent::Metadata {
                subject,
                key: key.to_owned(),
                type_: type_.map(str::to_owned),
                value: value.map(str::to_owned),
            }
        })
    }

    /// Any single event, including ones the backend would never send in that order.
    pub(crate) fn event() -> impl Strategy<Value = StoreEvent> {
        prop_oneof![
            (id(), serial(), props()).prop_map(|(id, serial, props)| StoreEvent::NodeAdded {
                id,
                serial,
                props
            }),
            (id(), props(), state()).prop_map(|(id, props, state)| StoreEvent::NodeInfo {
                id,
                props,
                state
            }),
            (id(), audio()).prop_map(|(id, audio)| StoreEvent::NodeAudio { id, audio }),
            (id(), 0i64..3).prop_map(|(id, process_latency_ns)| StoreEvent::NodeLatency {
                id,
                process_latency_ns
            }),
            id().prop_map(|id| StoreEvent::NodeRemoved { id }),
            (id(), serial(), props()).prop_map(|(id, serial, props)| StoreEvent::DeviceAdded {
                id,
                serial,
                props
            }),
            (id(), props()).prop_map(|(id, props)| StoreEvent::DeviceInfo { id, props }),
            (id(), profiles())
                .prop_map(|(id, profiles)| StoreEvent::DeviceProfiles { id, profiles }),
            (id(), 0i32..3).prop_map(|(id, index)| StoreEvent::DeviceProfile { id, index }),
            (id(), routes()).prop_map(|(id, routes)| StoreEvent::DeviceRoutes { id, routes }),
            (id(), active()).prop_map(|(id, active)| StoreEvent::DeviceActive { id, active }),
            id().prop_map(|id| StoreEvent::DeviceRemoved { id }),
            (
                id(),
                id(),
                dir(),
                prop::option::of(Just("FL".to_owned())),
                any::<bool>()
            )
                .prop_map(|(id, node, dir, channel, monitor)| StoreEvent::PortAdded {
                    id,
                    node,
                    dir,
                    channel,
                    monitor
                }),
            id().prop_map(|id| StoreEvent::PortRemoved { id }),
            (id(), id(), id(), id(), id()).prop_map(
                |(id, out_node, out_port, in_node, in_port)| StoreEvent::LinkAdded {
                    id,
                    out_node,
                    out_port,
                    in_node,
                    in_port
                }
            ),
            id().prop_map(|id| StoreEvent::LinkRemoved { id }),
            metadata(),
            Just(StoreEvent::MetadataCleared),
        ]
    }

    /// Object family of an `*Added`/`*Removed` event.
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    enum Family {
        Node,
        Device,
        Port,
        Link,
    }

    /// `(family, id, is_added)` for lifecycle events.
    fn lifecycle(event: &StoreEvent) -> Option<(Family, ObjectId, bool)> {
        match event {
            StoreEvent::NodeAdded { id, .. } => Some((Family::Node, *id, true)),
            StoreEvent::NodeRemoved { id } => Some((Family::Node, *id, false)),
            StoreEvent::DeviceAdded { id, .. } => Some((Family::Device, *id, true)),
            StoreEvent::DeviceRemoved { id } => Some((Family::Device, *id, false)),
            StoreEvent::PortAdded { id, .. } => Some((Family::Port, *id, true)),
            StoreEvent::PortRemoved { id } => Some((Family::Port, *id, false)),
            StoreEvent::LinkAdded { id, .. } => Some((Family::Link, *id, true)),
            StoreEvent::LinkRemoved { id } => Some((Family::Link, *id, false)),
            _ => None,
        }
    }

    /// Event sequences as the backend can produce them (starting from an empty store): an
    /// `*Added` never repeats an id that is still alive. Everything else is arbitrary:
    /// unknown ids, removals of unknown ids, references to missing nodes.
    pub(crate) fn events() -> impl Strategy<Value = Vec<StoreEvent>> {
        prop::collection::vec(event(), 0..64).prop_map(|raw| {
            let mut alive = HashSet::new();
            raw.into_iter()
                .filter(|event| match lifecycle(event) {
                    Some((family, id, true)) => alive.insert((family, id)),
                    Some((family, id, false)) => {
                        alive.remove(&(family, id));
                        true
                    }
                    None => true,
                })
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use proptest::prelude::*;

    use super::*;
    use crate::backend::{DisconnectReason, StoreEvent};
    use crate::model::{
        ActiveRoute, Availability, Channel, NodeAudio, NodeKind, NodeState, ObjectId, PortDir,
        Profile, PropMap, RouteInfo, Serial, StreamTarget,
    };

    fn class(class: &str) -> PropMap {
        PropMap::from([("media.class".to_owned(), class.to_owned())])
    }

    fn node_added(id: u32, media_class: &str) -> StoreEvent {
        StoreEvent::NodeAdded {
            id: ObjectId(id),
            serial: Serial(u64::from(id) + 1000),
            props: class(media_class),
        }
    }

    fn sink(id: u32) -> StoreEvent {
        node_added(id, "Audio/Sink")
    }

    fn device_added(id: u32, media_class: &str) -> StoreEvent {
        StoreEvent::DeviceAdded {
            id: ObjectId(id),
            serial: Serial(u64::from(id) + 1000),
            props: class(media_class),
        }
    }

    fn audio(gain: f32) -> NodeAudio {
        NodeAudio {
            volume: 1.0,
            channel_volumes: vec![gain, gain],
            mute: false,
            channel_map: vec![Channel(3), Channel(4)],
        }
    }

    fn port_added(id: u32, node: u32) -> StoreEvent {
        StoreEvent::PortAdded {
            id: ObjectId(id),
            node: ObjectId(node),
            dir: PortDir::Out,
            channel: Some("FL".to_owned()),
            monitor: false,
        }
    }

    fn link_added(id: u32, out_node: u32, in_node: u32) -> StoreEvent {
        StoreEvent::LinkAdded {
            id: ObjectId(id),
            out_node: ObjectId(out_node),
            out_port: ObjectId(90),
            in_node: ObjectId(in_node),
            in_port: ObjectId(91),
        }
    }

    fn store_with(events: impl IntoIterator<Item = StoreEvent>) -> Store {
        let mut store = Store::default();
        for event in events {
            store.apply(event);
        }
        store
    }

    #[test]
    fn node_added_stores_sink() {
        let mut store = Store::default();
        assert!(store.apply(sink(40)));
        let node = &store.nodes[&ObjectId(40)];
        assert_eq!(node.kind, NodeKind::Sink);
        assert_eq!(node.serial, Serial(1040));
        assert_eq!(node.props, class("Audio/Sink"));
        assert_eq!(node.audio, None);
        assert_eq!(node.process_latency_ns, None);
    }

    #[test]
    fn node_added_ignores_video_stream() {
        let mut store = Store::default();
        assert!(!store.apply(node_added(40, "Stream/Output/Video")));
        assert!(!store.apply(node_added(41, "Stream/Input/Video")));
        assert!(store.nodes.is_empty());
        assert_eq!(store.revision, 0);
    }

    #[test]
    fn node_added_ignores_midi_and_duplex() {
        let mut store = Store::default();
        assert!(!store.apply(node_added(40, "Midi/Bridge")));
        assert!(!store.apply(node_added(41, "Audio/Duplex")));
        assert!(!store.apply(StoreEvent::NodeAdded {
            id: ObjectId(42),
            serial: Serial(1),
            props: PropMap::new(),
        }));
        assert!(store.nodes.is_empty());
    }

    #[test]
    fn duplicate_node_added_with_same_props_returns_false() {
        let mut store = store_with([sink(40)]);
        store.apply(StoreEvent::NodeAudio {
            id: ObjectId(40),
            audio: audio(0.5),
        });
        assert!(!store.apply(sink(40)));
        assert_eq!(store.nodes[&ObjectId(40)].audio, Some(audio(0.5)));
    }

    #[test]
    fn node_info_replaces_props_and_reports_change() {
        let mut store = store_with([sink(40)]);
        let mut props = class("Audio/Sink");
        props.insert("node.name".to_owned(), "out".to_owned());
        let info = StoreEvent::NodeInfo {
            id: ObjectId(40),
            props: props.clone(),
            state: NodeState::Running,
        };
        assert!(store.apply(info.clone()));
        assert_eq!(store.nodes[&ObjectId(40)].props, props);
        assert_eq!(store.nodes[&ObjectId(40)].state, NodeState::Running);
        assert!(!store.apply(info));
    }

    #[test]
    fn node_info_for_unknown_id_is_noop() {
        let mut store = store_with([sink(40)]);
        let before = store.clone();
        assert!(!store.apply(StoreEvent::NodeInfo {
            id: ObjectId(41),
            props: PropMap::new(),
            state: NodeState::Idle,
        }));
        assert_eq!(store, before);
    }

    #[test]
    fn node_audio_before_node_added_is_dropped() {
        let mut store = Store::default();
        assert!(!store.apply(StoreEvent::NodeAudio {
            id: ObjectId(40),
            audio: audio(0.5),
        }));
        assert!(!store.apply(StoreEvent::NodeLatency {
            id: ObjectId(40),
            process_latency_ns: 5,
        }));
        store.apply(sink(40));
        assert_eq!(store.nodes[&ObjectId(40)].audio, None);
        assert_eq!(store.nodes[&ObjectId(40)].process_latency_ns, None);
    }

    #[test]
    fn node_audio_sets_and_replaces() {
        let mut store = store_with([sink(40)]);
        let set = |gain| StoreEvent::NodeAudio {
            id: ObjectId(40),
            audio: audio(gain),
        };
        assert!(store.apply(set(0.5)));
        assert_eq!(store.nodes[&ObjectId(40)].audio, Some(audio(0.5)));
        assert!(!store.apply(set(0.5)));
        assert!(store.apply(set(0.25)));
        assert_eq!(store.nodes[&ObjectId(40)].audio, Some(audio(0.25)));
        let latency = StoreEvent::NodeLatency {
            id: ObjectId(40),
            process_latency_ns: 1000,
        };
        assert!(store.apply(latency.clone()));
        assert!(!store.apply(latency));
        assert_eq!(store.nodes[&ObjectId(40)].process_latency_ns, Some(1000));
    }

    #[test]
    fn node_removed_cascades_ports_links_and_stream_target() {
        let mut store = store_with([
            sink(40),
            node_added(50, "Stream/Output/Audio"),
            node_added(60, "Stream/Output/Audio"),
            port_added(41, 40),
            port_added(51, 50),
            link_added(70, 50, 40),
            link_added(71, 40, 60),
            link_added(72, 60, 60),
        ]);
        let target = StreamTarget {
            object: Some("40".to_owned()),
            node: None,
        };
        store.stream_targets.insert(ObjectId(50), target.clone());
        store.stream_targets.insert(ObjectId(40), target.clone());

        assert!(store.apply(StoreEvent::NodeRemoved { id: ObjectId(40) }));
        assert!(!store.nodes.contains_key(&ObjectId(40)));
        assert_eq!(store.ports.keys().collect::<Vec<_>>(), [&ObjectId(51)]);
        assert_eq!(store.links.keys().collect::<Vec<_>>(), [&ObjectId(72)]);
        assert_eq!(
            store.stream_targets.keys().collect::<Vec<_>>(),
            [&ObjectId(50)]
        );
    }

    #[test]
    fn node_removed_unknown_is_noop() {
        let mut store = store_with([sink(40)]);
        let before = store.clone();
        assert!(!store.apply(StoreEvent::NodeRemoved { id: ObjectId(41) }));
        assert_eq!(store, before);
    }

    #[test]
    fn device_added_only_for_audio_device() {
        let mut store = Store::default();
        assert!(!store.apply(device_added(30, "Video/Device")));
        assert!(!store.apply(StoreEvent::DeviceAdded {
            id: ObjectId(31),
            serial: Serial(1),
            props: PropMap::new(),
        }));
        assert!(store.apply(device_added(32, "Audio/Device")));
        assert!(!store.apply(device_added(32, "Audio/Device")));
        assert_eq!(store.devices.keys().collect::<Vec<_>>(), [&ObjectId(32)]);
        let device = &store.devices[&ObjectId(32)];
        assert_eq!(device.serial, Serial(1032));
        assert!(device.profiles.is_empty() && device.routes.is_empty());
        assert_eq!(device.active_profile, None);

        let mut props = class("Audio/Device");
        props.insert("device.name".to_owned(), "card".to_owned());
        let info = StoreEvent::DeviceInfo {
            id: ObjectId(32),
            props: props.clone(),
        };
        assert!(store.apply(info.clone()));
        assert!(!store.apply(info));
        assert_eq!(store.devices[&ObjectId(32)].props, props);
        assert!(!store.apply(StoreEvent::DeviceInfo {
            id: ObjectId(30),
            props,
        }));

        assert!(store.apply(StoreEvent::DeviceRemoved { id: ObjectId(32) }));
        assert!(!store.apply(StoreEvent::DeviceRemoved { id: ObjectId(32) }));
        assert!(store.devices.is_empty());
    }

    #[test]
    fn device_profiles_enum_and_active_index() {
        let mut store = store_with([device_added(30, "Audio/Device")]);
        let profiles = vec![Profile {
            index: 1,
            name: "output:analog-stereo".to_owned(),
            description: "Analog Stereo Output".to_owned(),
            priority: 6500,
            available: Availability::Yes,
        }];
        let enum_profile = StoreEvent::DeviceProfiles {
            id: ObjectId(30),
            profiles: profiles.clone(),
        };
        assert!(store.apply(enum_profile.clone()));
        assert!(!store.apply(enum_profile));
        let active = StoreEvent::DeviceProfile {
            id: ObjectId(30),
            index: 1,
        };
        assert!(store.apply(active.clone()));
        assert!(!store.apply(active));
        let device = &store.devices[&ObjectId(30)];
        assert_eq!(device.profiles, profiles);
        assert_eq!(device.active_profile, Some(1));
        assert!(!store.apply(StoreEvent::DeviceProfile {
            id: ObjectId(31),
            index: 1,
        }));
    }

    #[test]
    fn device_routes_enum_and_active() {
        let mut store = store_with([device_added(30, "Audio/Device")]);
        let routes = vec![RouteInfo {
            index: 3,
            dir: PortDir::Out,
            name: "analog-output-lineout".to_owned(),
            description: "Line Out".to_owned(),
            priority: 9000,
            available: Availability::Unknown,
            profiles: vec![1, 2],
            devices: vec![4],
        }];
        let enum_route = StoreEvent::DeviceRoutes {
            id: ObjectId(30),
            routes: routes.clone(),
        };
        assert!(store.apply(enum_route.clone()));
        assert!(!store.apply(enum_route));
        let active = vec![ActiveRoute {
            index: 3,
            card_profile_device: 4,
            dir: PortDir::Out,
        }];
        let route = StoreEvent::DeviceActive {
            id: ObjectId(30),
            active: active.clone(),
        };
        assert!(store.apply(route.clone()));
        assert!(!store.apply(route));
        let device = &store.devices[&ObjectId(30)];
        assert_eq!(device.routes, routes);
        assert_eq!(device.active_routes, active);
        assert!(!store.apply(StoreEvent::DeviceRoutes {
            id: ObjectId(31),
            routes,
        }));
    }

    #[test]
    fn port_added_requires_known_node() {
        let mut store = Store::default();
        assert!(!store.apply(port_added(41, 40)));
        store.apply(sink(40));
        assert!(store.apply(port_added(41, 40)));
        assert!(!store.apply(port_added(41, 40)));
        assert_eq!(
            store.ports[&ObjectId(41)],
            Port {
                id: ObjectId(41),
                node: ObjectId(40),
                dir: PortDir::Out,
                channel: Some("FL".to_owned()),
                monitor: false,
            }
        );
        assert!(store.apply(StoreEvent::PortRemoved { id: ObjectId(41) }));
        assert!(store.ports.is_empty());
    }

    #[test]
    fn port_removed_unknown_is_noop() {
        let mut store = store_with([sink(40), port_added(41, 40)]);
        let before = store.clone();
        assert!(!store.apply(StoreEvent::PortRemoved { id: ObjectId(42) }));
        assert_eq!(store, before);
    }

    #[test]
    fn link_added_requires_known_nodes() {
        let mut store = store_with([sink(40)]);
        assert!(!store.apply(link_added(70, 50, 40)));
        assert!(!store.apply(link_added(70, 40, 50)));
        store.apply(node_added(50, "Stream/Output/Audio"));
        assert!(store.apply(link_added(70, 50, 40)));
        assert!(!store.apply(link_added(70, 50, 40)));
        assert_eq!(
            store.links[&ObjectId(70)],
            Link {
                id: ObjectId(70),
                out_node: ObjectId(50),
                out_port: ObjectId(90),
                in_node: ObjectId(40),
                in_port: ObjectId(91),
            }
        );
        assert!(store.apply(StoreEvent::LinkRemoved { id: ObjectId(70) }));
        assert!(store.links.is_empty());
    }

    #[test]
    fn link_removed_unknown_is_noop() {
        let mut store = store_with([sink(40), sink(41), link_added(70, 40, 41)]);
        let before = store.clone();
        assert!(!store.apply(StoreEvent::LinkRemoved { id: ObjectId(71) }));
        assert_eq!(store, before);
    }

    #[test]
    fn disconnected_clears_maps_keeps_conn() {
        let mut store = store_with([
            sink(40),
            sink(41),
            port_added(42, 40),
            link_added(70, 40, 41),
            device_added(30, "Audio/Device"),
        ]);
        store.defaults.sink = Some("out".to_owned());
        store.stream_targets.insert(
            ObjectId(40),
            StreamTarget {
                object: Some("x".to_owned()),
                node: None,
            },
        );
        let conn = ConnState::Disconnected {
            reason: DisconnectReason::ConnectionLost,
            retry_at: Instant::now(),
            attempt: 1,
        };
        store.set_conn(conn.clone());
        let revision = store.revision;

        assert!(store.clear());
        assert_eq!(store.revision, revision + 1);
        assert_eq!(store.conn, conn);
        let mut expected = Store {
            conn,
            revision: store.revision,
            ..Store::default()
        };
        assert_eq!(store, expected);
        assert!(!store.clear());
        expected.revision = store.revision;
        assert_eq!(store, expected);
    }

    #[test]
    fn connected_sets_conn() {
        let mut store = Store::default();
        assert_eq!(store.conn, ConnState::Connecting);
        let connected = ConnState::Connected {
            server_version: "1.6.9".to_owned(),
        };
        assert!(store.set_conn(connected.clone()));
        assert_eq!(store.conn, connected);
        assert_eq!(store.revision, 1);
        assert!(!store.set_conn(connected));
        assert_eq!(store.revision, 1);
    }

    #[test]
    fn revision_increments_iff_apply_returns_true() {
        let events = [
            sink(40),
            sink(40),
            StoreEvent::NodeAudio {
                id: ObjectId(40),
                audio: audio(0.5),
            },
            StoreEvent::NodeAudio {
                id: ObjectId(41),
                audio: audio(0.5),
            },
            port_added(41, 40),
            StoreEvent::NodeRemoved { id: ObjectId(40) },
            StoreEvent::NodeRemoved { id: ObjectId(40) },
        ];
        let mut store = Store::default();
        for event in events {
            let before = store.revision;
            let changed = store.apply(event);
            assert_eq!(store.revision, before + u64::from(changed));
        }
        assert_eq!(store.revision, 4);
    }

    proptest! {
        #[test]
        fn random_event_sequences_never_panic(
            events in prop::collection::vec(strategies::event(), 0..64),
            plausible in strategies::events(),
        ) {
            for events in [events, plausible] {
                let mut store = Store::default();
                for event in events {
                    let before = store.clone();
                    let changed = store.apply(event.clone());
                    prop_assert_eq!(store.revision, before.revision + u64::from(changed));
                    let after = store.clone();
                    prop_assert!(!store.apply(event), "re-apply changed the store");
                    prop_assert_eq!(&store, &after);
                    let unchanged = Store { revision: before.revision, ..store.clone() } == before;
                    prop_assert_eq!(unchanged, !changed);
                }
            }
        }
    }
}
