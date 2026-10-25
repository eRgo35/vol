//! Canned graphs for the mock backend (`docs/specs/09-testing-and-quality.md` §2).
//!
//! Property names and values follow `pw-dump` of PipeWire 1.6.9 + WirePlumber;
//! the streams are the rows of the Playback mockup (`docs/specs/02-playback.md` §2.3).

use super::mock::MockScript;
use super::{BackendEvent, StoreEvent};
use crate::model::{
    ActiveRoute, Availability, Channel, NodeAudio, NodeState, ObjectId, PortDir, Profile, PropMap,
    RouteInfo, Serial,
};

/// `node.name` of the default sink.
pub const DEFAULT_SINK: &str = "alsa_output.pci-0000_28_00.4.analog-stereo";
/// `node.name` of the default source.
pub const DEFAULT_SOURCE: &str = "alsa_input.usb-0d8c_USB_PnP_Audio_Device-00.mono-fallback";

const STARSHIP: &str = "Starship/Matisse HD Audio Controller Analog Stereo";
const CA0132_SINK: &str = "alsa_output.pci-0000_05_00.0.analog-stereo";
const STARSHIP_SOURCE: &str = "alsa_input.pci-0000_28_00.4.analog-stereo";

/// `spa_audio_channel` ids.
const MONO: &[u32] = &[2];
const STEREO: &[u32] = &[3, 4];

/// The Playback mockup's system, connected:
///
/// | id | serial | node | class | volume | mute | state | linked to |
/// |---|---|---|---|---|---|---|---|
/// | 45 | 45 | `alsa_card.pci-0000_28_00.4` | `Audio/Device` | | | | |
/// | 52 | 52 | Starship/Matisse sink (default) | `Audio/Sink` | 87 % | | running | |
/// | 58 | 58 | CA0132 sink | `Audio/Sink` | 31 % | | suspended | |
/// | 53 | 53 | Starship/Matisse source | `Audio/Source` | 70 % | | suspended | |
/// | 61 | 61 | USB PnP mono source (default) | `Audio/Source` | 94 % | | running | |
/// | 84 | 412 | Firefox | playback | 72 % | | running | sink 52 |
/// | 90 | 433 | Spotify | playback | 120 % | | running | sink 52 |
/// | 97 | 457 | mpv | playback | 55 % | yes | idle | sink 58 |
/// | 103 | 468 | pw-loopback (virtual) | playback | 100 % | | running | sink 52 |
/// | 110 | 481 | Telegram Desktop (mono) | capture | 100 % | | running | source 61 |
///
/// Everything except the mono nodes is stereo. Delivered as one `Batch`
/// followed by `Connected`, the order the real backend uses.
pub fn default_system() -> MockScript {
    let mut g = Graph::default();
    g.card();
    g.node(Spec {
        id: 52,
        serial: 52,
        state: NodeState::Running,
        gain: 0.658_492,
        mute: false,
        channels: STEREO,
        props: &[
            ("media.class", "Audio/Sink"),
            ("node.name", DEFAULT_SINK),
            ("node.description", STARSHIP),
            ("node.nick", "HD-Audio Generic"),
            ("device.id", "45"),
            ("card.profile.device", "1"),
            ("device.api", "alsa"),
            ("api.alsa.path", "front:1"),
            ("audio.channels", "2"),
            ("audio.position", "FL,FR"),
            ("priority.session", "1009"),
        ],
    });
    g.node(Spec {
        id: 58,
        serial: 58,
        state: NodeState::Suspended,
        gain: gain(31.0),
        mute: false,
        channels: STEREO,
        props: &[
            ("media.class", "Audio/Sink"),
            ("node.name", CA0132_SINK),
            ("node.description", "CA0132 Sound Core3D Analog Stereo"),
            ("node.nick", "CA0132 Analog"),
            ("device.api", "alsa"),
            ("audio.channels", "2"),
            ("audio.position", "FL,FR"),
            ("priority.session", "1009"),
        ],
    });
    g.node(Spec {
        id: 53,
        serial: 53,
        state: NodeState::Suspended,
        gain: gain(70.0),
        mute: false,
        channels: STEREO,
        props: &[
            ("media.class", "Audio/Source"),
            ("node.name", STARSHIP_SOURCE),
            ("node.description", STARSHIP),
            ("node.nick", "HD-Audio Generic"),
            ("device.id", "45"),
            ("card.profile.device", "0"),
            ("device.api", "alsa"),
            ("api.alsa.path", "front:1"),
            ("audio.channels", "2"),
            ("audio.position", "FL,FR"),
            ("priority.session", "2009"),
        ],
    });
    g.node(Spec {
        id: 61,
        serial: 61,
        state: NodeState::Running,
        gain: gain(94.0),
        mute: false,
        channels: MONO,
        props: &[
            ("media.class", "Audio/Source"),
            ("node.name", DEFAULT_SOURCE),
            ("node.description", "USB PnP Audio Device Mono"),
            ("node.nick", "USB PnP Audio Device"),
            ("device.api", "alsa"),
            ("audio.channels", "1"),
            ("audio.position", "MONO"),
            ("priority.session", "2010"),
        ],
    });
    g.node(Spec {
        id: 84,
        serial: 412,
        state: NodeState::Running,
        gain: gain(72.0),
        mute: false,
        channels: STEREO,
        props: &[
            ("media.class", "Stream/Output/Audio"),
            ("node.name", "Firefox"),
            ("application.name", "Firefox"),
            ("application.process.binary", "firefox"),
            ("application.process.id", "4127"),
            ("application.icon-name", "firefox"),
            ("media.name", "Playing music – Lo-fi beats"),
            ("audio.channels", "2"),
        ],
    });
    g.node(Spec {
        id: 90,
        serial: 433,
        state: NodeState::Running,
        gain: gain(120.0),
        mute: false,
        channels: STEREO,
        props: &[
            ("media.class", "Stream/Output/Audio"),
            ("node.name", "spotify"),
            ("application.name", "Spotify"),
            ("application.process.binary", "spotify"),
            ("application.process.id", "5310"),
            ("application.icon-name", "spotify-client"),
            ("media.name", "Song Title"),
            ("media.role", "Music"),
            ("audio.channels", "2"),
        ],
    });
    g.node(Spec {
        id: 97,
        serial: 457,
        state: NodeState::Idle,
        gain: gain(55.0),
        mute: true,
        channels: STEREO,
        props: &[
            ("media.class", "Stream/Output/Audio"),
            ("node.name", "mpv"),
            ("application.name", "mpv"),
            ("application.process.binary", "mpv"),
            ("application.process.id", "6044"),
            ("application.icon-name", "mpv"),
            ("media.name", "video.mkv"),
            ("media.role", "Movie"),
            ("audio.channels", "2"),
        ],
    });
    g.node(Spec {
        id: 103,
        serial: 468,
        state: NodeState::Running,
        gain: gain(100.0),
        mute: false,
        channels: STEREO,
        props: &[
            ("media.class", "Stream/Output/Audio"),
            ("node.name", "output.loopback-2841-13"),
            ("node.description", "loopback-2841-13 output"),
            ("node.link-group", "loopback-2841-13"),
            ("application.name", "pw-loopback"),
            ("application.process.binary", "pw-loopback"),
            ("media.name", "loopback-2841-13 output"),
            ("audio.channels", "2"),
        ],
    });
    g.node(Spec {
        id: 110,
        serial: 481,
        state: NodeState::Running,
        gain: gain(100.0),
        mute: false,
        channels: MONO,
        props: &[
            ("media.class", "Stream/Input/Audio"),
            ("node.name", "telegram-desktop"),
            ("application.name", "Telegram Desktop"),
            ("application.process.binary", "telegram-desktop"),
            ("application.process.id", "7182"),
            ("media.name", "Voice message"),
            ("audio.channels", "1"),
        ],
    });
    g.link(84, 52);
    g.link(90, 52);
    g.link(97, 58);
    g.link(103, 52);
    g.link(61, 110);
    for (key, name) in [
        ("default.audio.sink", DEFAULT_SINK),
        ("default.configured.audio.sink", DEFAULT_SINK),
        ("default.audio.source", DEFAULT_SOURCE),
        ("default.configured.audio.source", DEFAULT_SOURCE),
    ] {
        g.events.push(StoreEvent::Metadata {
            subject: 0,
            key: key.to_owned(),
            type_: Some("Spa:String:JSON".to_owned()),
            value: Some(format!(r#"{{"name":"{name}"}}"#)),
        });
    }
    MockScript {
        initial: vec![
            BackendEvent::Batch(g.events),
            BackendEvent::Connected {
                server_version: "1.6.9".to_owned(),
                socket: "pipewire-0".to_owned(),
            },
        ],
    }
}

/// Linear gain of `percent` on the cubic scale (`docs/specs/02-playback.md` §2.1).
fn gain(percent: f32) -> f32 {
    (percent / 100.0).powi(3)
}

fn props(pairs: &[(&str, &str)]) -> PropMap {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

/// One row of the node table of [`default_system`].
struct Spec<'a> {
    id: u32,
    serial: u64,
    state: NodeState,
    /// Gain of every channel; `Props.volume` is 1.0.
    gain: f32,
    mute: bool,
    /// `channelMap`.
    channels: &'a [u32],
    /// Properties besides `object.serial`.
    props: &'a [(&'a str, &'a str)],
}

/// A port: (id, node, direction, monitor).
type PortEntry = (ObjectId, ObjectId, PortDir, bool);

/// Store events under construction; ports and links get ids from 200 upwards.
#[derive(Default)]
struct Graph {
    events: Vec<StoreEvent>,
    ports: Vec<PortEntry>,
    next_id: u32,
}

impl Graph {
    fn take_id(&mut self) -> ObjectId {
        let id = ObjectId(200 + self.next_id);
        self.next_id += 1;
        id
    }

    /// The Starship/Matisse card: duplex profile active, Line Out and Rear
    /// Microphone active, headphones and microphones unplugged.
    fn card(&mut self) {
        let id = ObjectId(45);
        let profile = |index: i32, name: &str, description: &str, priority: i32| Profile {
            index,
            name: name.to_owned(),
            description: description.to_owned(),
            priority,
            available: Availability::Yes,
        };
        let route = |index: i32, dir: PortDir, name: &str, description: &str, available: u32| {
            let (priority, device) = match dir {
                PortDir::Out => (9000, 1),
                PortDir::In => (8000, 0),
            };
            RouteInfo {
                index,
                dir,
                name: name.to_owned(),
                description: description.to_owned(),
                priority: priority - index,
                available: Availability::from_spa(available),
                profiles: vec![1, 2],
                devices: vec![device],
            }
        };
        self.events.extend([
            StoreEvent::DeviceAdded {
                id,
                serial: Serial(45),
                props: props(&[
                    ("media.class", "Audio/Device"),
                    ("device.name", "alsa_card.pci-0000_28_00.4"),
                    ("device.description", "Starship/Matisse HD Audio Controller"),
                    ("device.nick", "HD-Audio Generic"),
                    ("device.api", "alsa"),
                    ("device.bus-path", "pci-0000:28:00.4"),
                    ("api.alsa.card", "1"),
                    ("object.serial", "45"),
                ]),
            },
            StoreEvent::DeviceProfiles {
                id,
                profiles: vec![
                    profile(0, "off", "Off", 0),
                    profile(
                        1,
                        "output:analog-stereo+input:analog-stereo",
                        "Analog Stereo Duplex",
                        6565,
                    ),
                    profile(2, "output:analog-stereo", "Analog Stereo Output", 6500),
                ],
            },
            StoreEvent::DeviceProfile { id, index: 1 },
            StoreEvent::DeviceRoutes {
                id,
                routes: vec![
                    route(
                        0,
                        PortDir::In,
                        "analog-input-front-mic",
                        "Front Microphone",
                        1,
                    ),
                    route(
                        1,
                        PortDir::In,
                        "analog-input-rear-mic",
                        "Rear Microphone",
                        1,
                    ),
                    route(2, PortDir::Out, "analog-output-lineout", "Line Out", 2),
                    route(3, PortDir::Out, "analog-output-headphones", "Headphones", 1),
                ],
            },
            StoreEvent::DeviceActive {
                id,
                active: vec![
                    ActiveRoute {
                        index: 1,
                        card_profile_device: 0,
                        dir: PortDir::In,
                    },
                    ActiveRoute {
                        index: 2,
                        card_profile_device: 1,
                        dir: PortDir::Out,
                    },
                ],
            },
        ]);
    }

    /// A node with its info, `Props` and ports. Sinks get input ports plus
    /// monitor outputs, sources and playback streams outputs, capture streams inputs.
    fn node(&mut self, spec: Spec<'_>) {
        let Spec {
            id,
            serial,
            state,
            gain,
            mute,
            channels,
            props: pairs,
        } = spec;
        let id = ObjectId(id);
        let mut props = props(pairs);
        props.insert("object.serial".to_owned(), serial.to_string());
        let class = props.get("media.class").cloned().unwrap_or_default();
        self.events.extend([
            StoreEvent::NodeAdded {
                id,
                serial: Serial(serial),
                props: props.clone(),
            },
            StoreEvent::NodeInfo { id, props, state },
            StoreEvent::NodeAudio {
                id,
                audio: NodeAudio {
                    volume: 1.0,
                    channel_volumes: vec![gain; channels.len()],
                    mute,
                    channel_map: channels.iter().copied().map(Channel::from_spa_id).collect(),
                },
            },
        ]);
        let port_sets: &[(PortDir, bool)] = match class.as_str() {
            "Audio/Sink" => &[(PortDir::In, false), (PortDir::Out, true)],
            "Stream/Input/Audio" => &[(PortDir::In, false)],
            _ => &[(PortDir::Out, false)],
        };
        for (dir, monitor) in port_sets {
            for (position, channel) in channels.iter().enumerate() {
                let port = self.take_id();
                self.ports.push((port, id, dir.clone(), *monitor));
                self.events.push(StoreEvent::PortAdded {
                    id: port,
                    node: id,
                    dir: dir.clone(),
                    channel: Some(Channel::from_spa_id(*channel).short_name(position)),
                    monitor: *monitor,
                });
            }
        }
    }

    /// Links the non-monitor output ports of `out_node` to the input ports of
    /// `in_node`, channel by channel.
    fn link(&mut self, out_node: u32, in_node: u32) {
        let (out_node, in_node) = (ObjectId(out_node), ObjectId(in_node));
        let ports_of = |node: ObjectId, dir: PortDir| -> Vec<ObjectId> {
            self.ports
                .iter()
                .filter(|(_, n, d, monitor)| *n == node && *d == dir && !monitor)
                .map(|(port, ..)| *port)
                .collect()
        };
        let pairs: Vec<_> = ports_of(out_node, PortDir::Out)
            .into_iter()
            .zip(ports_of(in_node, PortDir::In))
            .collect();
        for (out_port, in_port) in pairs {
            let id = self.take_id();
            self.events.push(StoreEvent::LinkAdded {
                id,
                out_node,
                out_port,
                in_node,
                in_port,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{NodeKind, Store};

    /// The store after the script, asserting that every store event changed it.
    fn applied() -> Store {
        let mut store = Store::default();
        for event in default_system().initial {
            if let BackendEvent::Batch(events) = event {
                for event in events {
                    let shown = format!("{event:?}");
                    assert!(store.apply(event), "no change: {shown}");
                }
            }
        }
        store
    }

    fn count(store: &Store, kind: &NodeKind) -> usize {
        store.nodes.values().filter(|n| n.kind == *kind).count()
    }

    #[test]
    fn default_system_applies_cleanly() {
        let script = default_system();
        assert!(matches!(
            script.initial.as_slice(),
            [BackendEvent::Batch(_), BackendEvent::Connected { .. }]
        ));
        let store = applied();
        assert_eq!(count(&store, &NodeKind::Sink), 2);
        assert_eq!(count(&store, &NodeKind::Source), 2);
        assert_eq!(count(&store, &NodeKind::PlaybackStream), 4);
        assert_eq!(count(&store, &NodeKind::CaptureStream), 1);
        assert_eq!(store.devices.len(), 1);
        // Sinks: 2 inputs + 2 monitors each; sources: 2 + 1 (USB is mono);
        // playback: 2 each; capture: 1 (mono).
        assert_eq!(store.ports.len(), 2 * 4 + 3 + 4 * 2 + 1);
        assert_eq!(store.links.len(), 4 * 2 + 1);
        assert!(store.nodes.values().all(|n| n.audio.is_some()));
        // The Playback mockup's percentages (02 §2.3).
        for (id, percent, mute) in [(84, 72.0, false), (90, 120.0, false), (97, 55.0, true)] {
            let audio = store.nodes[&ObjectId(id)].audio.as_ref().unwrap();
            let level = audio.channel_volumes[0].cbrt() * 100.0;
            assert_eq!(level.round(), percent, "node {id}");
            assert_eq!(audio.mute, mute, "node {id}");
        }
        let device = &store.devices[&ObjectId(45)];
        assert_eq!(device.profiles.len(), 3);
        assert_eq!(device.active_profile, Some(1));
        assert_eq!(device.routes.len(), 4);
        assert_eq!(device.active_routes.len(), 2);
        assert_eq!(store.defaults.source.as_deref(), Some(DEFAULT_SOURCE));
        assert_eq!(
            store.defaults.configured_source.as_deref(),
            Some(DEFAULT_SOURCE)
        );
    }

    #[test]
    fn default_system_has_exactly_one_default_sink() {
        let store = applied();
        assert_eq!(store.defaults.sink.as_deref(), Some(DEFAULT_SINK));
        assert_eq!(
            store.defaults.configured_sink.as_deref(),
            Some(DEFAULT_SINK)
        );
        let defaults: Vec<_> = store
            .nodes
            .values()
            .filter(|n| n.kind == NodeKind::Sink)
            .filter(|n| n.props.get("node.name").map(String::as_str) == Some(DEFAULT_SINK))
            .collect();
        assert_eq!(defaults.len(), 1);
        assert_eq!(defaults[0].props["device.id"], "45");
        assert_eq!(defaults[0].props["card.profile.device"], "1");
        let audio = defaults[0].audio.as_ref().unwrap();
        assert_eq!(audio.channel_volumes, [0.658_492, 0.658_492]);
    }
}
