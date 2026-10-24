//! The `default` metadata object (VOL-ARCH-046): defaults and stream targets.
//!
//! Only the metadata global named `default` is bound. Its `property` events
//! become `StoreEvent::Metadata`; its removal (e.g. a WirePlumber restart)
//! becomes `MetadataCleared`, and a reappearing `default` global is bound
//! again. The proxy stays bound for the setters added in M4/M5.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Weak;

use pipewire::metadata::{Metadata, MetadataListener};
use pipewire::registry::{GlobalObject, RegistryRc};
use pipewire::spa::utils::dict::DictRef;

use super::conn::{State, with_state};
use crate::backend::StoreEvent;
use crate::log;
use crate::model::{ObjectId, PropMap};

/// True for the metadata global the backend binds: `metadata.name == "default"`.
fn is_default_metadata(props: &PropMap) -> bool {
    props
        .get("metadata.name")
        .is_some_and(|name| name == "default")
}

/// Store events for one `property` event. `keys` tracks the `(subject, key)`
/// pairs currently set, so that a `None` key (every key of `subject`
/// removed) can be reported key by key. A `None` value removes the key.
fn property_events(
    keys: &mut BTreeSet<(u32, String)>,
    subject: u32,
    key: Option<&str>,
    type_: Option<&str>,
    value: Option<&str>,
) -> Vec<StoreEvent> {
    let removal = |(subject, key): (u32, String)| StoreEvent::Metadata {
        subject,
        key,
        type_: None,
        value: None,
    };
    let Some(key) = key else {
        let removed: Vec<_> = keys
            .iter()
            .filter(|(s, _)| *s == subject)
            .cloned()
            .collect();
        return removed
            .into_iter()
            .map(|pair| {
                keys.remove(&pair);
                removal(pair)
            })
            .collect();
    };
    if value.is_some() {
        keys.insert((subject, key.to_owned()));
    } else {
        keys.remove(&(subject, key.to_owned()));
    }
    vec![StoreEvent::Metadata {
        subject,
        key: key.to_owned(),
        type_: type_.map(str::to_owned),
        value: value.map(str::to_owned),
    }]
}

/// The bound `default` metadata. Field order is drop order: the listener
/// goes before its proxy.
pub(super) struct DefaultMetadata {
    _listener: MetadataListener,
    _proxy: Metadata,
    /// Registry id of the global.
    pub(super) id: ObjectId,
    keys: BTreeSet<(u32, String)>,
}

/// Binds `global` if it is the `default` metadata and none is bound yet.
pub(super) fn bind(
    state: &Weak<RefCell<State>>,
    registry: &RegistryRc,
    global: &GlobalObject<&DictRef>,
    props: &PropMap,
) {
    let id = ObjectId(global.id);
    if !is_default_metadata(props) {
        return;
    }
    if with_state(state, |s| s.metadata.is_some()) != Some(false) {
        log::debug(format_args!(
            "vol-pw: second default metadata {} ignored",
            id.0
        ));
        return;
    }
    let proxy: Metadata = match registry.bind(global) {
        Ok(proxy) => proxy,
        Err(err) => {
            log::warn(format_args!("vol-pw: cannot bind metadata {}: {err}", id.0));
            return;
        }
    };
    let listener = proxy
        .add_listener_local()
        .property({
            let state = state.clone();
            move |subject, key, type_, value| {
                with_state(&state, |s| {
                    if let Some(metadata) = &mut s.metadata {
                        for event in property_events(&mut metadata.keys, subject, key, type_, value)
                        {
                            s.out.store(event);
                        }
                    }
                });
                0
            }
        })
        .register();
    log::debug(format_args!("vol-pw: bound default metadata {}", id.0));
    with_state(state, |s| {
        s.metadata = Some(DefaultMetadata {
            _listener: listener,
            _proxy: proxy,
            id,
            keys: BTreeSet::new(),
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: &[(&str, &str)]) -> PropMap {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn metadata(subject: u32, key: &str, type_: Option<&str>, value: Option<&str>) -> StoreEvent {
        StoreEvent::Metadata {
            subject,
            key: key.to_owned(),
            type_: type_.map(str::to_owned),
            value: value.map(str::to_owned),
        }
    }

    #[test]
    fn only_default_name_is_bound() {
        assert!(is_default_metadata(&props(&[("metadata.name", "default")])));
        for name in ["settings", "route-settings", "Default", "default "] {
            assert!(!is_default_metadata(&props(&[("metadata.name", name)])));
        }
        assert!(!is_default_metadata(&props(&[])));
    }

    #[test]
    fn property_none_value_maps_to_removal() {
        let mut keys = BTreeSet::new();
        let json = Some("Spa:String:JSON");
        let set = property_events(&mut keys, 0, Some("default.audio.sink"), json, Some("{}"));
        assert_eq!(set, [metadata(0, "default.audio.sink", json, Some("{}"))]);
        let removed = property_events(&mut keys, 0, Some("default.audio.sink"), None, None);
        assert_eq!(removed, [metadata(0, "default.audio.sink", None, None)]);
        assert!(keys.is_empty());
    }

    #[test]
    fn none_key_removes_every_key_of_the_subject() {
        let mut keys = BTreeSet::new();
        property_events(&mut keys, 0, Some("default.audio.sink"), None, Some("a"));
        property_events(&mut keys, 42, Some("target.object"), None, Some("7"));
        property_events(&mut keys, 42, Some("target.node"), None, Some("x"));
        let removed = property_events(&mut keys, 42, None, None, None);
        assert_eq!(
            removed,
            [
                metadata(42, "target.node", None, None),
                metadata(42, "target.object", None, None),
            ]
        );
        assert_eq!(keys.len(), 1);
        assert_eq!(property_events(&mut keys, 42, None, None, None), []);
    }
}
