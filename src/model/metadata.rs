//! Values from the `default` metadata object.

/// Default devices, as `node.name`s.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Defaults {
    /// Effective default sink (`default.audio.sink`).
    pub sink: Option<String>,
    /// Effective default source (`default.audio.source`).
    pub source: Option<String>,
    /// User-configured default sink (`default.configured.audio.sink`).
    pub configured_sink: Option<String>,
    /// User-configured default source (`default.configured.audio.source`).
    pub configured_source: Option<String>,
}

/// Per-stream routing target (`target.object` / `target.node`, subject = stream node id).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StreamTarget {
    /// `target.object`.
    pub object: Option<String>,
    /// `target.node`.
    pub node: Option<String>,
}

/// Parses a `default.*` metadata value, JSON `{"name": String}`; anything else is `None`
/// (VOL-ARCH-025).
pub fn parse_default_name(value: &str) -> Option<String> {
    match serde_json::from_str::<serde_json::Value>(value)
        .ok()?
        .get("name")?
    {
        serde_json::Value::String(name) => Some(name.clone()),
        _ => None,
    }
}

impl Defaults {
    /// The field a subject-0 metadata key sets, if it is one of the four default keys.
    pub fn slot_mut(&mut self, key: &str) -> Option<&mut Option<String>> {
        match key {
            "default.audio.sink" => Some(&mut self.sink),
            "default.audio.source" => Some(&mut self.source),
            "default.configured.audio.sink" => Some(&mut self.configured_sink),
            "default.configured.audio.source" => Some(&mut self.configured_source),
            _ => None,
        }
    }
}

impl StreamTarget {
    /// The field a per-stream metadata key sets: `target.object` or `target.node`.
    pub fn slot_mut(&mut self, key: &str) -> Option<&mut Option<String>> {
        match key {
            "target.object" => Some(&mut self.object),
            "target.node" => Some(&mut self.node),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::StoreEvent;
    use crate::model::{ObjectId, PropMap, Serial, Store};

    const JSON: &str = "Spa:String:JSON";

    fn meta(subject: u32, key: &str, value: Option<&str>) -> StoreEvent {
        StoreEvent::Metadata {
            subject,
            key: key.to_owned(),
            type_: Some(JSON.to_owned()),
            value: value.map(str::to_owned),
        }
    }

    fn stream(id: u32) -> StoreEvent {
        StoreEvent::NodeAdded {
            id: ObjectId(id),
            serial: Serial(u64::from(id)),
            props: PropMap::from([("media.class".to_owned(), "Stream/Output/Audio".to_owned())]),
        }
    }

    const SINK: &str = r#"{"name":"alsa_output.pci-0000_28_00.4.analog-stereo"}"#;

    #[test]
    fn default_sink_parsed() {
        let mut store = Store::default();
        assert!(store.apply(meta(0, "default.audio.sink", Some(SINK))));
        assert_eq!(
            store.defaults.sink.as_deref(),
            Some("alsa_output.pci-0000_28_00.4.analog-stereo")
        );
        assert!(!store.apply(meta(0, "default.audio.sink", Some(SINK))));
        assert_eq!(store.defaults.source, None);
    }

    #[test]
    fn default_source_parsed() {
        let mut store = Store::default();
        let value = r#"{ "name": "alsa_input.usb-mic" }"#;
        assert!(store.apply(meta(0, "default.audio.source", Some(value))));
        assert_eq!(store.defaults.source.as_deref(), Some("alsa_input.usb-mic"));
        assert_eq!(store.defaults.sink, None);
    }

    #[test]
    fn configured_keys_parsed_separately() {
        let mut store = Store::default();
        store.apply(meta(0, "default.audio.sink", Some(r#"{"name":"a"}"#)));
        store.apply(meta(
            0,
            "default.configured.audio.sink",
            Some(r#"{"name":"b"}"#),
        ));
        store.apply(meta(
            0,
            "default.configured.audio.source",
            Some(r#"{"name":"c"}"#),
        ));
        assert_eq!(
            store.defaults,
            Defaults {
                sink: Some("a".to_owned()),
                source: None,
                configured_sink: Some("b".to_owned()),
                configured_source: Some("c".to_owned()),
            }
        );
    }

    #[test]
    fn malformed_json_gives_none() {
        for value in ["", "{", r#"{"name":"a""#, "name", r#"{"name":"a"} x"#] {
            assert_eq!(parse_default_name(value), None, "{value:?}");
        }
        let mut store = Store::default();
        store.apply(meta(0, "default.audio.sink", Some(r#"{"name":"a"}"#)));
        assert!(store.apply(meta(0, "default.audio.sink", Some("{"))));
        assert_eq!(store.defaults.sink, None);
        assert!(!store.apply(meta(0, "default.audio.sink", Some("{"))));
    }

    #[test]
    fn wrong_type_value_gives_none() {
        for value in [
            "42",
            r#""a""#,
            "null",
            r#"["a"]"#,
            r#"{"name":5}"#,
            r#"{"name":null}"#,
            r#"{"name":["a"]}"#,
            r#"{"other":"a"}"#,
        ] {
            assert_eq!(parse_default_name(value), None, "{value:?}");
        }
        let mut store = Store::default();
        assert!(!store.apply(meta(0, "default.audio.source", Some("42"))));
        assert_eq!(store.defaults.source, None);
    }

    #[test]
    fn unrelated_keys_ignored() {
        let mut store = Store::default();
        assert!(!store.apply(meta(0, "default.video.source", Some(SINK))));
        assert!(!store.apply(meta(0, "default.audio.sink.x", Some(SINK))));
        assert!(!store.apply(meta(7, "default.audio.sink", Some(SINK))));
        assert!(!store.apply(meta(0, "target.object", Some("47"))));
        assert_eq!(store, Store::default());
    }

    #[test]
    fn removal_clears_default() {
        let mut store = Store::default();
        store.apply(meta(0, "default.configured.audio.sink", Some(SINK)));
        assert!(store.apply(meta(0, "default.configured.audio.sink", None)));
        assert_eq!(store.defaults.configured_sink, None);
        assert!(!store.apply(meta(0, "default.configured.audio.sink", None)));
    }

    #[test]
    fn target_keys_stored_per_subject() {
        let mut store = Store::default();
        assert!(!store.apply(meta(50, "target.object", Some("47"))));
        store.apply(stream(50));
        store.apply(stream(51));
        assert!(store.apply(meta(50, "target.object", Some("47"))));
        assert!(!store.apply(meta(50, "target.object", Some("47"))));
        assert!(store.apply(meta(50, "target.node", Some("40"))));
        assert!(store.apply(meta(51, "target.node", Some("41"))));
        assert_eq!(
            store.stream_targets[&ObjectId(50)],
            StreamTarget {
                object: Some("47".to_owned()),
                node: Some("40".to_owned()),
            }
        );
        assert_eq!(
            store.stream_targets[&ObjectId(51)],
            StreamTarget {
                object: None,
                node: Some("41".to_owned()),
            }
        );
    }

    #[test]
    fn target_key_removed_when_value_none() {
        let mut store = Store::default();
        store.apply(stream(50));
        store.apply(meta(50, "target.object", Some("47")));
        store.apply(meta(50, "target.node", Some("40")));
        assert!(store.apply(meta(50, "target.node", None)));
        assert_eq!(
            store.stream_targets[&ObjectId(50)],
            StreamTarget {
                object: Some("47".to_owned()),
                node: None,
            }
        );
        assert!(store.apply(meta(50, "target.object", None)));
        assert!(!store.stream_targets.contains_key(&ObjectId(50)));
        assert!(!store.apply(meta(50, "target.object", None)));
    }

    #[test]
    fn metadata_cleared_empties_everything() {
        let mut store = Store::default();
        store.apply(stream(50));
        store.apply(meta(0, "default.audio.sink", Some(SINK)));
        store.apply(meta(0, "default.configured.audio.source", Some(SINK)));
        store.apply(meta(50, "target.object", Some("47")));
        assert!(store.apply(StoreEvent::MetadataCleared));
        assert_eq!(store.defaults, Defaults::default());
        assert!(store.stream_targets.is_empty());
        assert!(store.nodes.contains_key(&ObjectId(50)));
        assert!(!store.apply(StoreEvent::MetadataCleared));
    }

    #[test]
    fn name_with_escaped_quotes_roundtrips() {
        assert_eq!(
            parse_default_name(r#"{"name":"a\"b"}"#).as_deref(),
            Some(r#"a"b"#)
        );
    }
}
