//! Registry mirror: globals → `StoreEvent`s (VOL-ARCH-010).
//!
//! Nodes and devices are bound and their proxies kept, together with their
//! listeners, in [`Objects`] (part of the shared `State`); ports and links
//! carry everything the model needs in their global properties, so they are
//! only tracked by id. The pure converters at the top are unit-tested.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Weak;

use pipewire::core::{CoreRc, CoreWeak};
use pipewire::device::{Device, DeviceChangeMask, DeviceInfoRef, DeviceListener};
use pipewire::node::{Node, NodeChangeMask, NodeInfoRef, NodeListener};
use pipewire::registry::{GlobalObject, RegistryRc};
use pipewire::spa::param::ParamType;
use pipewire::spa::pod::Pod;
use pipewire::spa::utils::dict::DictRef;
use pipewire::types::ObjectType;

use super::Outbox;
use super::conn::{State, with_state};
use super::metadata::{self, DefaultMetadata};
use super::params;
use crate::backend::StoreEvent;
use crate::log;
use crate::model::{
    ActiveRoute, NodeState, ObjectId, PortDir, Profile, PropMap, RouteInfo, Serial,
};

/// Node params subscribed to and enumerated on bind (VOL-ARCH-032).
const NODE_PARAMS: [ParamType; 2] = [ParamType::Props, ParamType::ProcessLatency];

/// Device params subscribed to and enumerated on bind (VOL-ARCH-032).
const DEVICE_PARAMS: [ParamType; 4] = [
    ParamType::EnumProfile,
    ParamType::Profile,
    ParamType::EnumRoute,
    ParamType::Route,
];

/// Collects `(key, value)` pairs into a [`PropMap`], verbatim.
pub(super) fn props_to_map<'a>(iter: impl Iterator<Item = (&'a str, &'a str)>) -> PropMap {
    iter.map(|(k, v)| (k.to_owned(), v.to_owned())).collect()
}

/// `object.serial`, if present and numeric.
fn serial(props: &PropMap) -> Option<Serial> {
    props.get("object.serial")?.parse().ok().map(Serial)
}

/// A property parsed as a registry id.
fn id_prop(props: &PropMap, key: &str) -> Option<ObjectId> {
    props.get(key)?.parse().ok().map(ObjectId)
}

/// `NodeAdded`, or `None` (object skipped) without a usable `object.serial`.
fn node_added(id: ObjectId, props: PropMap) -> Option<StoreEvent> {
    let serial = serial(&props)?;
    Some(StoreEvent::NodeAdded { id, serial, props })
}

/// `DeviceAdded`, or `None` (object skipped) without a usable `object.serial`.
fn device_added(id: ObjectId, props: PropMap) -> Option<StoreEvent> {
    let serial = serial(&props)?;
    Some(StoreEvent::DeviceAdded { id, serial, props })
}

/// `PortAdded` from a port global's `node.id`, `port.direction` (both
/// required), `audio.channel` and `port.monitor`.
pub(super) fn port_from_props(id: ObjectId, props: &PropMap) -> Option<StoreEvent> {
    Some(StoreEvent::PortAdded {
        id,
        node: id_prop(props, "node.id")?,
        dir: PortDir::from_prop(props.get("port.direction")?)?,
        channel: props.get("audio.channel").cloned(),
        monitor: props.get("port.monitor").is_some_and(|v| v == "true"),
    })
}

/// `LinkAdded` from a link global's `link.{output,input}.{node,port}` (all required).
pub(super) fn link_from_props(id: ObjectId, props: &PropMap) -> Option<StoreEvent> {
    Some(StoreEvent::LinkAdded {
        id,
        out_node: id_prop(props, "link.output.node")?,
        out_port: id_prop(props, "link.output.port")?,
        in_node: id_prop(props, "link.input.node")?,
        in_port: id_prop(props, "link.input.port")?,
    })
}

/// Collects the objects of multi-object param replies (`EnumProfile`,
/// `EnumRoute`, `Route`) per object into one list per enumeration.
///
/// `enum_params` replies carry a daemon-generated seq (not the one passed),
/// shared by every object of one enumeration; a new seq for the same object
/// therefore completes the previous enumeration. The last one completes on
/// the next core `done` ([`Accumulator::take_all`]).
pub(super) struct Accumulator<T> {
    open: BTreeMap<ObjectId, (i32, Vec<T>)>,
}

impl<T> Default for Accumulator<T> {
    fn default() -> Self {
        Self {
            open: BTreeMap::new(),
        }
    }
}

impl<T> Accumulator<T> {
    /// Adds `item` of the reply with `seq` for `id`; returns the completed
    /// previous enumeration of `id` if `seq` starts a new one.
    pub(super) fn push(&mut self, id: ObjectId, seq: i32, item: T) -> Option<Vec<T>> {
        match self.open.get_mut(&id) {
            Some((open_seq, items)) if *open_seq == seq => {
                items.push(item);
                None
            }
            Some((open_seq, items)) => {
                *open_seq = seq;
                Some(std::mem::replace(items, vec![item]))
            }
            None => {
                self.open.insert(id, (seq, vec![item]));
                None
            }
        }
    }

    /// Completes every open enumeration.
    pub(super) fn take_all(&mut self) -> Vec<(ObjectId, Vec<T>)> {
        std::mem::take(&mut self.open)
            .into_iter()
            .map(|(id, (_, items))| (id, items))
            .collect()
    }

    /// Forgets the open enumeration of a removed object.
    pub(super) fn remove(&mut self, id: ObjectId) {
        self.open.remove(&id);
    }

    /// True if no enumeration is open.
    pub(super) fn is_empty(&self) -> bool {
        self.open.is_empty()
    }
}

/// The initial snapshot (VOL-ARCH-032): the first `core.sync` marks the end of
/// the global burst (binds and param requests are issued while it is
/// processed); a second `sync` then waits for the replies to those requests,
/// and its `done` is when `Connected` is emitted.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum InitialSync {
    /// No sync requested yet.
    Starting,
    /// Waiting for the `done` of the sync that follows the global burst.
    Globals(i32),
    /// Waiting for the `done` of the sync that follows the binds.
    Replies(i32),
    /// `Connected` was emitted.
    Done,
}

/// What a core `done` asks the caller to do.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum SyncStep {
    /// Not an initial-snapshot sync.
    Ignore,
    /// Globals are in: call `core.sync` again and pass its seq to
    /// [`InitialSync::awaiting_replies`].
    Resync,
    /// Snapshot complete: flush and emit `Connected` (exactly once).
    Connected,
}

impl InitialSync {
    /// Advances on a core `done` with `seq`.
    pub(super) fn on_done(&mut self, seq: i32) -> SyncStep {
        match *self {
            Self::Globals(expected) if expected == seq => SyncStep::Resync,
            Self::Replies(expected) if expected == seq => {
                *self = Self::Done;
                SyncStep::Connected
            }
            _ => SyncStep::Ignore,
        }
    }

    /// Records the seq of the second sync.
    pub(super) fn awaiting_replies(&mut self, seq: i32) {
        *self = Self::Replies(seq);
    }
}

fn node_state(state: &pipewire::node::NodeState<'_>) -> NodeState {
    match state {
        pipewire::node::NodeState::Error(_) => NodeState::Error,
        pipewire::node::NodeState::Creating => NodeState::Creating,
        pipewire::node::NodeState::Suspended => NodeState::Suspended,
        pipewire::node::NodeState::Idle => NodeState::Idle,
        pipewire::node::NodeState::Running => NodeState::Running,
    }
}

/// A bound node. Field order is drop order: the listener goes before its proxy.
struct BoundNode {
    _listener: NodeListener,
    _proxy: Node,
    /// Last full props (info carries props only when they changed).
    props: PropMap,
    state: NodeState,
}

/// A bound device. Field order is drop order.
struct BoundDevice {
    _listener: DeviceListener,
    _proxy: Device,
}

/// Registry objects of one connection.
#[derive(Default)]
pub(super) struct Objects {
    nodes: BTreeMap<ObjectId, BoundNode>,
    devices: BTreeMap<ObjectId, BoundDevice>,
    ports: BTreeSet<ObjectId>,
    links: BTreeSet<ObjectId>,
    profiles: Accumulator<Profile>,
    routes: Accumulator<RouteInfo>,
    active_routes: Accumulator<ActiveRoute>,
}

impl Objects {
    /// True while a multi-object param reply may still be incomplete.
    pub(super) fn has_open_enumerations(&self) -> bool {
        !(self.profiles.is_empty() && self.routes.is_empty() && self.active_routes.is_empty())
    }

    /// Emits every open enumeration as complete.
    pub(super) fn flush_enumerations(&mut self, out: &Outbox) {
        for (id, profiles) in self.profiles.take_all() {
            emit_profiles(out, id, profiles);
        }
        for (id, routes) in self.routes.take_all() {
            emit_routes(out, id, routes);
        }
        for (id, active) in self.active_routes.take_all() {
            out.store(StoreEvent::DeviceActive { id, active });
        }
    }

    /// One-line summary for the log.
    pub(super) fn counts(&self) -> String {
        format!(
            "{} nodes, {} devices, {} ports, {} links",
            self.nodes.len(),
            self.devices.len(),
            self.ports.len(),
            self.links.len()
        )
    }
}

fn emit_profiles(out: &Outbox, id: ObjectId, profiles: Vec<Profile>) {
    log::debug(format_args!(
        "vol-pw: device {}: {} profiles",
        id.0,
        profiles.len()
    ));
    out.store(StoreEvent::DeviceProfiles { id, profiles });
}

fn emit_routes(out: &Outbox, id: ObjectId, routes: Vec<RouteInfo>) {
    log::debug(format_args!(
        "vol-pw: device {}: {} routes",
        id.0,
        routes.len()
    ));
    out.store(StoreEvent::DeviceRoutes { id, routes });
}

/// Registry `global` handler.
pub(super) fn on_global(
    state: &Weak<RefCell<State>>,
    registry: &RegistryRc,
    core: &CoreRc,
    global: &GlobalObject<&DictRef>,
) {
    let id = ObjectId(global.id);
    let props = global
        .props
        .map(|props| props_to_map(props.iter()))
        .unwrap_or_default();
    match global.type_ {
        ObjectType::Node => bind_node(state, registry, global, id, props),
        ObjectType::Device => bind_device(state, registry, core, global, id, props),
        ObjectType::Port => match port_from_props(id, &props) {
            Some(event) => {
                with_state(state, |s| {
                    s.objects.ports.insert(id);
                    s.out.store(event);
                });
            }
            None => log::debug(format_args!("vol-pw: port {} skipped: {props:?}", id.0)),
        },
        ObjectType::Link => match link_from_props(id, &props) {
            Some(event) => {
                with_state(state, |s| {
                    s.objects.links.insert(id);
                    s.out.store(event);
                });
            }
            None => log::debug(format_args!("vol-pw: link {} skipped: {props:?}", id.0)),
        },
        ObjectType::Metadata => metadata::bind(state, registry, global, &props),
        _ => {}
    }
}

/// What a `global_remove` took out of the state, held only so that its
/// proxy is dropped after the state borrow has ended.
enum Removed {
    Node { _bound: BoundNode },
    Device { _bound: BoundDevice },
    Metadata { _bound: DefaultMetadata },
    Nothing,
}

/// Registry `global_remove` handler.
pub(super) fn on_global_remove(state: &Weak<RefCell<State>>, id: u32) {
    let id = ObjectId(id);
    let removed = with_state(state, |s| {
        let objects = &mut s.objects;
        let (event, removed) = if let Some(node) = objects.nodes.remove(&id) {
            (
                StoreEvent::NodeRemoved { id },
                Removed::Node { _bound: node },
            )
        } else if let Some(device) = objects.devices.remove(&id) {
            objects.profiles.remove(id);
            objects.routes.remove(id);
            objects.active_routes.remove(id);
            let removed = Removed::Device { _bound: device };
            (StoreEvent::DeviceRemoved { id }, removed)
        } else if objects.ports.remove(&id) {
            (StoreEvent::PortRemoved { id }, Removed::Nothing)
        } else if objects.links.remove(&id) {
            (StoreEvent::LinkRemoved { id }, Removed::Nothing)
        } else if let Some(metadata) = s.metadata.take_if(|m| m.id == id) {
            log::debug(format_args!("vol-pw: default metadata {} removed", id.0));
            let removed = Removed::Metadata { _bound: metadata };
            (StoreEvent::MetadataCleared, removed)
        } else {
            return Removed::Nothing;
        };
        s.out.store(event);
        removed
    });
    drop(removed);
}

fn bind_node(
    state: &Weak<RefCell<State>>,
    registry: &RegistryRc,
    global: &GlobalObject<&DictRef>,
    id: ObjectId,
    props: PropMap,
) {
    let Some(added) = node_added(id, props.clone()) else {
        log::debug(format_args!("vol-pw: node {} without object.serial", id.0));
        return;
    };
    let proxy: Node = match registry.bind(global) {
        Ok(proxy) => proxy,
        Err(err) => {
            log::warn(format_args!("vol-pw: cannot bind node {}: {err}", id.0));
            return;
        }
    };
    let listener = proxy
        .add_listener_local()
        .info({
            let state = state.clone();
            move |info| {
                with_state(&state, |s| node_info(s, id, info));
            }
        })
        .param({
            let state = state.clone();
            move |_seq, kind, _index, _next, pod| {
                if let Some(pod) = pod {
                    with_state(&state, |s| node_param(s, id, kind, pod));
                }
            }
        })
        .register();
    proxy.subscribe_params(&NODE_PARAMS);
    for kind in NODE_PARAMS {
        proxy.enum_params(0, Some(kind), 0, u32::MAX);
    }
    with_state(state, |s| {
        s.out.store(added);
        s.objects.nodes.insert(
            id,
            BoundNode {
                _listener: listener,
                _proxy: proxy,
                props,
                state: NodeState::Creating,
            },
        );
    });
}

fn node_info(s: &mut State, id: ObjectId, info: &NodeInfoRef) {
    let Some(node) = s.objects.nodes.get_mut(&id) else {
        return;
    };
    let mask = info.change_mask();
    if mask.contains(NodeChangeMask::PROPS)
        && let Some(props) = info.props()
    {
        node.props = props_to_map(props.iter());
    }
    if mask.contains(NodeChangeMask::STATE) {
        node.state = node_state(&info.state());
    }
    if mask.intersects(NodeChangeMask::PROPS | NodeChangeMask::STATE) {
        s.out.store(StoreEvent::NodeInfo {
            id,
            props: node.props.clone(),
            state: node.state.clone(),
        });
    }
}

fn node_param(s: &mut State, id: ObjectId, kind: ParamType, pod: &Pod) {
    if !s.objects.nodes.contains_key(&id) {
        return;
    }
    let event = match kind {
        ParamType::Props => {
            params::decode_props(pod).map(|audio| StoreEvent::NodeAudio { id, audio })
        }
        ParamType::ProcessLatency => {
            params::decode_process_latency(pod).map(|process_latency_ns| StoreEvent::NodeLatency {
                id,
                process_latency_ns,
            })
        }
        _ => return,
    };
    match event {
        Ok(event) => s.out.store(event),
        Err(err) => log::debug(format_args!(
            "vol-pw: node {}: skipped {kind:?} param: {err:?}",
            id.0
        )),
    }
}

fn bind_device(
    state: &Weak<RefCell<State>>,
    registry: &RegistryRc,
    core: &CoreRc,
    global: &GlobalObject<&DictRef>,
    id: ObjectId,
    props: PropMap,
) {
    let Some(added) = device_added(id, props) else {
        log::debug(format_args!(
            "vol-pw: device {} without object.serial",
            id.0
        ));
        return;
    };
    let proxy: Device = match registry.bind(global) {
        Ok(proxy) => proxy,
        Err(err) => {
            log::warn(format_args!("vol-pw: cannot bind device {}: {err}", id.0));
            return;
        }
    };
    let listener = proxy
        .add_listener_local()
        .info({
            let state = state.clone();
            move |info| {
                with_state(&state, |s| device_info(s, id, info));
            }
        })
        .param({
            let state = state.clone();
            let core = core.downgrade();
            move |seq, kind, _index, _next, pod| {
                let Some(pod) = pod else {
                    return;
                };
                let wants_sync = with_state(&state, |s| {
                    device_param(s, id, seq, kind, pod);
                    s.wants_flush_sync()
                });
                if wants_sync == Some(true) {
                    request_flush_sync(&state, &core);
                }
            }
        })
        .register();
    proxy.subscribe_params(&DEVICE_PARAMS);
    for kind in DEVICE_PARAMS {
        proxy.enum_params(0, Some(kind), 0, u32::MAX);
    }
    with_state(state, |s| {
        s.out.store(added);
        s.objects.devices.insert(
            id,
            BoundDevice {
                _listener: listener,
                _proxy: proxy,
            },
        );
    });
}

fn device_param(s: &mut State, id: ObjectId, seq: i32, kind: ParamType, pod: &Pod) {
    let objects = &mut s.objects;
    if !objects.devices.contains_key(&id) {
        return;
    }
    let decoded = match kind {
        ParamType::EnumProfile => params::decode_enum_profile(pod).map(|profile| {
            if let Some(done) = objects.profiles.push(id, seq, profile) {
                emit_profiles(&s.out, id, done);
            }
        }),
        ParamType::Profile => params::decode_profile_index(pod)
            .map(|index| s.out.store(StoreEvent::DeviceProfile { id, index })),
        ParamType::EnumRoute => params::decode_enum_route(pod).map(|route| {
            if let Some(done) = objects.routes.push(id, seq, route) {
                emit_routes(&s.out, id, done);
            }
        }),
        ParamType::Route => params::decode_route(pod).map(|route| {
            if let Some(active) = objects.active_routes.push(id, seq, route) {
                s.out.store(StoreEvent::DeviceActive { id, active });
            }
        }),
        _ => return,
    };
    if let Err(err) = decoded {
        log::debug(format_args!(
            "vol-pw: device {}: skipped {kind:?} param: {err:?}",
            id.0
        ));
    }
}

/// Asks for a core `done` that completes the enumerations opened by param
/// changes after the snapshot (called outside any state borrow).
fn request_flush_sync(state: &Weak<RefCell<State>>, core: &CoreWeak) {
    let Some(core) = core.upgrade() else {
        return;
    };
    match core.sync(0) {
        Ok(seq) => {
            with_state(state, |s| s.flush_sync = Some(seq.seq()));
        }
        Err(err) => log::warn(format_args!("vol-pw: flush sync failed: {err}")),
    }
}

fn device_info(s: &mut State, id: ObjectId, info: &DeviceInfoRef) {
    if !s.objects.devices.contains_key(&id) || !info.change_mask().contains(DeviceChangeMask::PROPS)
    {
        return;
    }
    if let Some(props) = info.props() {
        s.out.store(StoreEvent::DeviceInfo {
            id,
            props: props_to_map(props.iter()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> PropMap {
        props_to_map(pairs.iter().copied())
    }

    #[test]
    fn props_to_map_keeps_all_pairs() {
        let props = map(&[
            ("node.name", "sink"),
            ("media.class", "Audio/Sink"),
            ("object.serial", "42"),
            ("empty", ""),
        ]);
        assert_eq!(props.len(), 4);
        assert_eq!(props["node.name"], "sink");
        assert_eq!(props["media.class"], "Audio/Sink");
        assert_eq!(props["object.serial"], "42");
        assert_eq!(props["empty"], "");
    }

    #[test]
    fn serial_parsed_from_object_serial() {
        let props = map(&[("object.serial", "4711")]);
        assert_eq!(serial(&props), Some(Serial(4711)));
        assert_eq!(
            node_added(ObjectId(7), props.clone()),
            Some(StoreEvent::NodeAdded {
                id: ObjectId(7),
                serial: Serial(4711),
                props: props.clone(),
            })
        );
        assert_eq!(
            device_added(ObjectId(8), props.clone()),
            Some(StoreEvent::DeviceAdded {
                id: ObjectId(8),
                serial: Serial(4711),
                props,
            })
        );
    }

    #[test]
    fn missing_serial_skips_object() {
        let none = map(&[("node.name", "x")]);
        let garbage = map(&[("object.serial", "x12")]);
        for props in [none, garbage] {
            assert_eq!(serial(&props), None);
            assert_eq!(node_added(ObjectId(1), props.clone()), None);
            assert_eq!(device_added(ObjectId(1), props), None);
        }
    }

    #[test]
    fn port_from_props_input_and_monitor() {
        let input = map(&[
            ("node.id", "40"),
            ("port.direction", "in"),
            ("audio.channel", "FL"),
        ]);
        assert_eq!(
            port_from_props(ObjectId(50), &input),
            Some(StoreEvent::PortAdded {
                id: ObjectId(50),
                node: ObjectId(40),
                dir: PortDir::In,
                channel: Some("FL".to_owned()),
                monitor: false,
            })
        );
        let monitor = map(&[
            ("node.id", "40"),
            ("port.direction", "out"),
            ("port.monitor", "true"),
        ]);
        assert_eq!(
            port_from_props(ObjectId(51), &monitor),
            Some(StoreEvent::PortAdded {
                id: ObjectId(51),
                node: ObjectId(40),
                dir: PortDir::Out,
                channel: None,
                monitor: true,
            })
        );
        let no_dir = map(&[("node.id", "40")]);
        assert_eq!(port_from_props(ObjectId(52), &no_dir), None);
    }

    #[test]
    fn link_from_props_all_fields() {
        let props = map(&[
            ("link.output.node", "1"),
            ("link.output.port", "2"),
            ("link.input.node", "3"),
            ("link.input.port", "4"),
        ]);
        assert_eq!(
            link_from_props(ObjectId(9), &props),
            Some(StoreEvent::LinkAdded {
                id: ObjectId(9),
                out_node: ObjectId(1),
                out_port: ObjectId(2),
                in_node: ObjectId(3),
                in_port: ObjectId(4),
            })
        );
    }

    #[test]
    fn link_with_missing_field_skipped() {
        let full = [
            ("link.output.node", "1"),
            ("link.output.port", "2"),
            ("link.input.node", "3"),
            ("link.input.port", "4"),
        ];
        for skip in 0..full.len() {
            let pairs: Vec<_> = full
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != skip)
                .map(|(_, pair)| *pair)
                .collect();
            assert_eq!(link_from_props(ObjectId(9), &map(&pairs)), None);
        }
        let garbage = map(&[
            ("link.output.node", "one"),
            ("link.output.port", "2"),
            ("link.input.node", "3"),
            ("link.input.port", "4"),
        ]);
        assert_eq!(link_from_props(ObjectId(9), &garbage), None);
    }

    #[test]
    fn enum_replies_accumulate_until_next_seq() {
        let (a, b) = (ObjectId(1), ObjectId(2));
        let mut acc = Accumulator::default();
        assert!(acc.is_empty());
        // One enumeration per object; replies of different objects interleave.
        assert_eq!(acc.push(a, 10, "a0"), None);
        assert_eq!(acc.push(b, 11, "b0"), None);
        assert_eq!(acc.push(a, 10, "a1"), None);
        assert!(!acc.is_empty());
        // A new seq for `a` completes its previous enumeration.
        assert_eq!(acc.push(a, 12, "a2"), Some(vec!["a0", "a1"]));
        // The rest completes on demand (core `done`), per object, in id order.
        assert_eq!(acc.take_all(), vec![(a, vec!["a2"]), (b, vec!["b0"])]);
        assert!(acc.is_empty());
        // A removed object's open enumeration is dropped.
        assert_eq!(acc.push(b, 13, "b1"), None);
        acc.remove(b);
        assert!(acc.is_empty());
        assert_eq!(acc.take_all(), vec![]);
    }

    #[test]
    fn initial_sync_state_machine_emits_connected_once() {
        let mut sync = InitialSync::Starting;
        assert_eq!(sync.on_done(1), SyncStep::Ignore);
        sync = InitialSync::Globals(3);
        assert_eq!(sync.on_done(2), SyncStep::Ignore);
        assert_eq!(sync.on_done(3), SyncStep::Resync);
        sync.awaiting_replies(5);
        assert_eq!(sync.on_done(3), SyncStep::Ignore);
        assert_eq!(sync.on_done(4), SyncStep::Ignore);
        assert_eq!(sync.on_done(5), SyncStep::Connected);
        assert_eq!(sync, InitialSync::Done);
        assert_eq!(sync.on_done(5), SyncStep::Ignore);
        assert_eq!(sync.on_done(6), SyncStep::Ignore);
    }
}
