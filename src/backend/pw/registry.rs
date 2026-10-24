//! Registry mirror: globals → `StoreEvent`s (VOL-ARCH-010).
//!
//! Nodes and devices are bound and their proxies kept, together with their
//! listeners, in [`Objects`] (part of the shared `State`); ports and links
//! carry everything the model needs in their global properties, so they are
//! only tracked by id. The pure converters at the top are unit-tested.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Weak;

use pipewire::device::{Device, DeviceChangeMask, DeviceInfoRef, DeviceListener};
use pipewire::node::{Node, NodeChangeMask, NodeInfoRef, NodeListener};
use pipewire::registry::{GlobalObject, RegistryRc};
use pipewire::spa::utils::dict::DictRef;
use pipewire::types::ObjectType;

use super::conn::{State, with_state};
use crate::backend::StoreEvent;
use crate::log;
use crate::model::{NodeState, ObjectId, PortDir, PropMap, Serial};

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
}

impl Objects {
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

/// Registry `global` handler.
pub(super) fn on_global(
    state: &Weak<RefCell<State>>,
    registry: &RegistryRc,
    global: &GlobalObject<&DictRef>,
) {
    let id = ObjectId(global.id);
    let props = global
        .props
        .map(|props| props_to_map(props.iter()))
        .unwrap_or_default();
    match global.type_ {
        ObjectType::Node => bind_node(state, registry, global, id, props),
        ObjectType::Device => bind_device(state, registry, global, id, props),
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
        _ => {}
    }
}

/// Registry `global_remove` handler. A removed proxy is dropped after the
/// state borrow ends.
pub(super) fn on_global_remove(state: &Weak<RefCell<State>>, id: u32) {
    let id = ObjectId(id);
    let removed = with_state(state, |s| {
        let objects = &mut s.objects;
        let (event, node, device) = if let Some(node) = objects.nodes.remove(&id) {
            (Some(StoreEvent::NodeRemoved { id }), Some(node), None)
        } else if let Some(device) = objects.devices.remove(&id) {
            (Some(StoreEvent::DeviceRemoved { id }), None, Some(device))
        } else if objects.ports.remove(&id) {
            (Some(StoreEvent::PortRemoved { id }), None, None)
        } else if objects.links.remove(&id) {
            (Some(StoreEvent::LinkRemoved { id }), None, None)
        } else {
            (None, None, None)
        };
        if let Some(event) = event {
            s.out.store(event);
        }
        (node, device)
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
        .register();
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

fn bind_device(
    state: &Weak<RefCell<State>>,
    registry: &RegistryRc,
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
        .register();
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
}
