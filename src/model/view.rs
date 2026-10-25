//! Derived row models for the tabs (`docs/specs/02-playback.md` §2–§3). Pure.

use super::{Node, NodeAudio, NodeKind, NodeState, ObjectId, PropMap, PropMapExt, Serial, Store};

/// Stable identity of a row across updates (VOL-ROW-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowKey {
    /// A playback or capture stream, by `object.serial`.
    Stream(Serial),
}

/// Stream list filter; variant order is the `f` cycle order (VOL-PB-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamFilter {
    /// Non-virtual streams.
    #[default]
    Applications,
    /// Every stream.
    All,
    /// Virtual streams only (see [`is_virtual_stream`]).
    Virtual,
}

impl StreamFilter {
    /// Filter-bar name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Applications => "Applications",
            Self::All => "All Streams",
            Self::Virtual => "Virtual Streams",
        }
    }

    /// Next filter in cycle order, wrapping.
    pub fn next(self) -> Self {
        match self {
            Self::Applications => Self::All,
            Self::All => Self::Virtual,
            Self::Virtual => Self::Applications,
        }
    }

    /// Previous filter in cycle order, wrapping.
    pub fn prev(self) -> Self {
        match self {
            Self::Applications => Self::Virtual,
            Self::All => Self::Applications,
            Self::Virtual => Self::All,
        }
    }

    fn matches(self, node: &Node) -> bool {
        match self {
            Self::Applications => !is_virtual_stream(node),
            Self::All => true,
            Self::Virtual => is_virtual_stream(node),
        }
    }
}

/// Optimistic UI values overlaid on the store; empty until volume writes exist (M3).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UiPending;

/// The device a stream is attached to (02 §3.3). Rendered as `→ <text>` or `(no device)`.
#[derive(Debug, Clone, PartialEq)]
pub enum TargetLabel {
    /// Linked to a device with this description.
    Device(String),
    /// Not linked to any device.
    NoDevice,
}

/// One stream row (02 §3.1–§3.2).
#[derive(Debug, Clone, PartialEq)]
pub struct StreamRow {
    /// Row identity.
    pub key: RowKey,
    /// Registry id of the stream node.
    pub node: ObjectId,
    /// Application title.
    pub title: String,
    /// Media name; `None` if empty or equal to `title`.
    pub media: Option<String>,
    /// Node state is `Running` (`●`); otherwise `○` and dim.
    pub running: bool,
    /// Current device.
    pub target: TargetLabel,
    /// Volume state; `None` until the `Props` param arrives (bar shows `--`).
    pub audio: Option<NodeAudio>,
    /// Channel display names (02 §2.2), one per `channelVolumes` entry; empty without audio.
    pub channel_names: Vec<String>,
}

/// Heuristic for "virtual" streams (loopbacks, filter chains): `media.role` is `abstract` or
/// `filter`, `node.virtual` is set, or the node has a `node.link-group`.
pub fn is_virtual_stream(node: &Node) -> bool {
    let p = &node.props;
    matches!(
        p.get("media.role").map(String::as_str),
        Some("abstract" | "filter")
    ) || p.flag("node.virtual")
        || p.contains_key("node.link-group")
}

/// One of vol's own peak-meter streams (VOL-MTR-006).
fn is_own_meter(node: &Node) -> bool {
    node.props.flag("vol.meter")
        || node
            .props
            .get("node.name")
            .is_some_and(|n| n.starts_with("vol.meter."))
}

/// Playback rows of `store` matching `filter`, sorted by serial (VOL-ROW-006).
pub fn playback_rows(store: &Store, filter: StreamFilter, _pending: &UiPending) -> Vec<StreamRow> {
    let mut nodes: Vec<&Node> = store
        .nodes
        .values()
        .filter(|n| n.kind == NodeKind::PlaybackStream && !is_own_meter(n) && filter.matches(n))
        .collect();
    nodes.sort_by_key(|n| n.serial);
    nodes.into_iter().map(|n| stream_row(store, n)).collect()
}

fn stream_row(store: &Store, node: &Node) -> StreamRow {
    let title = first_prop(
        &node.props,
        &[
            "application.name",
            "node.description",
            "application.process.binary",
            "node.name",
        ],
    )
    .unwrap_or_default()
    .to_owned();
    let media = first_prop(&node.props, &["media.name"])
        .filter(|m| *m != title)
        .map(str::to_owned);
    StreamRow {
        key: RowKey::Stream(node.serial),
        node: node.id,
        title,
        media,
        running: node.state == NodeState::Running,
        target: sink_label(store, node.id),
        channel_names: node.audio.as_ref().map(channel_names).unwrap_or_default(),
        audio: node.audio.clone(),
    }
}

/// Description of the sink on the lowest-id link out of `stream` (02 §3.3 steps 2–3).
fn sink_label(store: &Store, stream: ObjectId) -> TargetLabel {
    store
        .links
        .values()
        .filter(|l| l.out_node == stream)
        .find_map(|l| {
            store
                .nodes
                .get(&l.in_node)
                .filter(|n| n.kind == NodeKind::Sink)
        })
        .map_or(TargetLabel::NoDevice, |sink| {
            let desc = first_prop(&sink.props, &["node.description", "node.nick", "node.name"]);
            TargetLabel::Device(desc.unwrap_or_default().to_owned())
        })
}

/// First non-empty value among `keys`.
fn first_prop<'a>(props: &'a PropMap, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .filter_map(|k| props.get(*k))
        .map(String::as_str)
        .find(|v| !v.is_empty())
}

/// `channelMap` names, or `C1…Cn` when the map does not match `channelVolumes` (02 §2.2).
fn channel_names(audio: &NodeAudio) -> Vec<String> {
    let n = audio.channel_volumes.len();
    if audio.channel_map.len() == n {
        audio
            .channel_map
            .iter()
            .enumerate()
            .map(|(i, c)| c.short_name(i))
            .collect()
    } else {
        (1..=n).map(|i| format!("C{i}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Channel, Link};

    fn props(pairs: &[(&str, &str)]) -> PropMap {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn node(id: u32, serial: u64, kind: NodeKind, pairs: &[(&str, &str)]) -> Node {
        Node {
            id: ObjectId(id),
            serial: Serial(serial),
            kind,
            props: props(pairs),
            state: NodeState::Idle,
            audio: None,
            process_latency_ns: None,
        }
    }

    fn stream(id: u32, pairs: &[(&str, &str)]) -> Node {
        node(id, u64::from(id) + 1000, NodeKind::PlaybackStream, pairs)
    }

    fn store_of(nodes: impl IntoIterator<Item = Node>) -> Store {
        Store {
            nodes: nodes.into_iter().map(|n| (n.id, n)).collect(),
            ..Store::default()
        }
    }

    fn link(store: &mut Store, id: u32, out_node: u32, in_node: u32) {
        store.links.insert(
            ObjectId(id),
            Link {
                id: ObjectId(id),
                out_node: ObjectId(out_node),
                out_port: ObjectId(900),
                in_node: ObjectId(in_node),
                in_port: ObjectId(901),
            },
        );
    }

    fn rows(store: &Store, filter: StreamFilter) -> Vec<StreamRow> {
        playback_rows(store, filter, &UiPending)
    }

    fn only_row(n: Node) -> StreamRow {
        let mut all = rows(&store_of([n]), StreamFilter::All);
        assert_eq!(all.len(), 1);
        all.remove(0)
    }

    fn ids(rows: &[StreamRow]) -> Vec<u32> {
        rows.iter().map(|r| r.node.0).collect()
    }

    /// Streams 1 (application), 2 (`node.virtual`), 3 (`media.role=filter`).
    fn mixed() -> Store {
        store_of([
            stream(1, &[]),
            stream(2, &[("node.virtual", "true")]),
            stream(3, &[("media.role", "filter")]),
        ])
    }

    #[test]
    fn title_prefers_application_name() {
        let row = only_row(stream(
            1,
            &[
                ("application.name", "Firefox"),
                ("node.description", "desc"),
                ("application.process.binary", "firefox"),
                ("node.name", "Firefox.out"),
            ],
        ));
        assert_eq!(row.title, "Firefox");
    }

    #[test]
    fn title_falls_back_to_description_binary_name() {
        let full = [
            ("node.description", "desc"),
            ("application.process.binary", "bin"),
            ("node.name", "name"),
        ];
        assert_eq!(only_row(stream(1, &full)).title, "desc");
        assert_eq!(only_row(stream(1, &full[1..])).title, "bin");
        assert_eq!(only_row(stream(1, &full[2..])).title, "name");
        let empty_app = [("application.name", ""), ("node.name", "name")];
        assert_eq!(only_row(stream(1, &empty_app)).title, "name");
    }

    #[test]
    fn media_omitted_when_equal_to_title() {
        let row = only_row(stream(
            1,
            &[("application.name", "mpv"), ("media.name", "video.mkv")],
        ));
        assert_eq!(row.media.as_deref(), Some("video.mkv"));
        let same = only_row(stream(
            1,
            &[("application.name", "mpv"), ("media.name", "mpv")],
        ));
        assert_eq!(same.media, None);
        let empty = only_row(stream(
            1,
            &[("application.name", "mpv"), ("media.name", "")],
        ));
        assert_eq!(empty.media, None);
    }

    #[test]
    fn virtual_by_media_role_filter() {
        assert!(is_virtual_stream(&stream(1, &[("media.role", "filter")])));
        assert!(is_virtual_stream(&stream(1, &[("media.role", "abstract")])));
        assert!(!is_virtual_stream(&stream(1, &[("media.role", "Music")])));
        assert!(!is_virtual_stream(&stream(1, &[])));
    }

    #[test]
    fn virtual_by_node_virtual_true() {
        assert!(is_virtual_stream(&stream(1, &[("node.virtual", "true")])));
        assert!(!is_virtual_stream(&stream(1, &[("node.virtual", "false")])));
    }

    #[test]
    fn virtual_by_link_group() {
        assert!(is_virtual_stream(&stream(
            1,
            &[("node.link-group", "loopback-123-45")]
        )));
    }

    #[test]
    fn application_filter_excludes_virtual() {
        assert_eq!(ids(&rows(&mixed(), StreamFilter::Applications)), [1]);
    }

    #[test]
    fn virtual_filter_only_virtual() {
        assert_eq!(ids(&rows(&mixed(), StreamFilter::Virtual)), [2, 3]);
    }

    #[test]
    fn all_filter_includes_both() {
        assert_eq!(ids(&rows(&mixed(), StreamFilter::All)), [1, 2, 3]);
    }

    #[test]
    fn filter_cycle_order_and_names() {
        let order = [
            StreamFilter::Applications,
            StreamFilter::All,
            StreamFilter::Virtual,
        ];
        assert_eq!(StreamFilter::default(), StreamFilter::Applications);
        for (i, f) in order.iter().enumerate() {
            let next = order[(i + 1) % order.len()];
            assert_eq!(f.next(), next);
            assert_eq!(next.prev(), *f);
        }
        let names: Vec<_> = order.iter().map(|f| f.name()).collect();
        assert_eq!(names, ["Applications", "All Streams", "Virtual Streams"]);
    }

    #[test]
    fn own_meter_streams_hidden_by_prop() {
        let store = store_of([stream(1, &[]), stream(2, &[("vol.meter", "true")])]);
        for filter in [
            StreamFilter::Applications,
            StreamFilter::All,
            StreamFilter::Virtual,
        ] {
            assert!(!ids(&rows(&store, filter)).contains(&2), "{filter:?}");
        }
    }

    #[test]
    fn own_meter_streams_hidden_by_name_prefix() {
        let store = store_of([
            stream(1, &[("node.name", "vol.meterish")]),
            stream(2, &[("node.name", "vol.meter.57")]),
        ]);
        assert_eq!(ids(&rows(&store, StreamFilter::All)), [1]);
    }

    #[test]
    fn rows_sorted_by_serial() {
        let store = store_of([
            node(1, 30, NodeKind::PlaybackStream, &[]),
            node(2, 10, NodeKind::PlaybackStream, &[]),
            node(3, 20, NodeKind::PlaybackStream, &[]),
            node(4, 5, NodeKind::CaptureStream, &[]),
            node(5, 6, NodeKind::Sink, &[]),
        ]);
        let keys: Vec<RowKey> = rows(&store, StreamFilter::All)
            .iter()
            .map(|r| r.key)
            .collect();
        assert_eq!(
            keys,
            [10, 20, 30].map(|s| RowKey::Stream(Serial(s))),
            "only playback streams, by serial"
        );
    }

    #[test]
    fn target_label_from_first_link_to_sink() {
        let mut store = store_of([
            stream(1, &[]),
            node(
                10,
                10,
                NodeKind::Sink,
                &[("node.nick", "nick"), ("node.name", "sink.a")],
            ),
            node(11, 11, NodeKind::Sink, &[("node.description", "Sink B")]),
            node(12, 12, NodeKind::CaptureStream, &[]),
        ]);
        link(&mut store, 50, 1, 12);
        link(&mut store, 52, 1, 10);
        link(&mut store, 51, 1, 11);
        link(&mut store, 49, 10, 1);
        let target = rows(&store, StreamFilter::All).remove(0).target;
        assert_eq!(target, TargetLabel::Device("Sink B".to_owned()));

        store.links.remove(&ObjectId(51));
        let target = rows(&store, StreamFilter::All).remove(0).target;
        assert_eq!(target, TargetLabel::Device("nick".to_owned()));
    }

    #[test]
    fn target_label_none_when_unlinked() {
        let mut store = store_of([stream(1, &[]), node(10, 10, NodeKind::Sink, &[])]);
        link(&mut store, 49, 10, 1);
        let target = rows(&store, StreamFilter::All).remove(0).target;
        assert_eq!(target, TargetLabel::NoDevice);
    }

    #[test]
    fn running_marker_from_node_state() {
        let mut n = stream(1, &[]);
        for (state, running) in [
            (NodeState::Running, true),
            (NodeState::Idle, false),
            (NodeState::Suspended, false),
            (NodeState::Creating, false),
            (NodeState::Error, false),
        ] {
            n.state = state.clone();
            assert_eq!(only_row(n.clone()).running, running, "{state:?}");
        }
    }

    #[test]
    fn audio_none_gives_row_without_volume() {
        let row = only_row(stream(1, &[("application.name", "app")]));
        assert_eq!(row.title, "app");
        assert_eq!(row.audio, None);
        assert!(row.channel_names.is_empty());
    }

    #[test]
    fn channel_names_from_map_or_positions() {
        let mut n = stream(1, &[]);
        let audio = NodeAudio {
            volume: 1.0,
            channel_volumes: vec![1.0, 1.0],
            mute: false,
            channel_map: vec![Channel(3), Channel(4)],
        };
        n.audio = Some(audio.clone());
        let row = only_row(n.clone());
        assert_eq!(row.channel_names, ["FL", "FR"]);
        assert_eq!(row.audio, Some(audio.clone()));

        n.audio = Some(NodeAudio {
            channel_map: vec![Channel(2)],
            ..audio
        });
        assert_eq!(only_row(n).channel_names, ["C1", "C2"]);
    }
}
