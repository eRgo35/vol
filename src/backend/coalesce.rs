//! Coalescing of `StoreEvent`s inside one flush window (VOL-ARCH-031).
//!
//! Pure: no PipeWire, no async runtime (VOL-ARCH-001, checked by `tests/layering.rs`).

use std::mem::{Discriminant, discriminant};

use super::StoreEvent;
use crate::model::ObjectId;

/// A batch holding this many events is flushed immediately (VOL-ARCH-030).
pub const FLUSH_THRESHOLD: usize = 256;

/// Events waiting for the next flush, coalesced as they are pushed.
#[derive(Debug, Default)]
pub struct Batch {
    events: Vec<StoreEvent>,
}

/// Object families with their own `*Added`/`*Removed` lifecycle.
#[derive(Clone, Copy, PartialEq)]
enum Family {
    Node,
    Device,
    Port,
    Link,
}

/// The object an event belongs to, and whether it is that object's `*Added` (`Some(true)`),
/// `*Removed` (`Some(false)`) or an update in between (`None`).
fn owner(event: &StoreEvent) -> Option<((Family, ObjectId), Option<bool>)> {
    use StoreEvent as E;
    match event {
        E::NodeAdded { id, .. } => Some(((Family::Node, *id), Some(true))),
        E::NodeRemoved { id } => Some(((Family::Node, *id), Some(false))),
        E::NodeInfo { id, .. } | E::NodeAudio { id, .. } | E::NodeLatency { id, .. } => {
            Some(((Family::Node, *id), None))
        }
        E::DeviceAdded { id, .. } => Some(((Family::Device, *id), Some(true))),
        E::DeviceRemoved { id } => Some(((Family::Device, *id), Some(false))),
        E::DeviceInfo { id, .. }
        | E::DeviceProfiles { id, .. }
        | E::DeviceProfile { id, .. }
        | E::DeviceRoutes { id, .. }
        | E::DeviceActive { id, .. } => Some(((Family::Device, *id), None)),
        E::PortAdded { id, .. } => Some(((Family::Port, *id), Some(true))),
        E::PortRemoved { id } => Some(((Family::Port, *id), Some(false))),
        E::LinkAdded { id, .. } => Some(((Family::Link, *id), Some(true))),
        E::LinkRemoved { id } => Some(((Family::Link, *id), Some(false))),
        E::Metadata { .. } | E::MetadataCleared => None,
    }
}

/// The state slot an event fully overwrites; a later event for the same slot supersedes
/// an earlier one (rules 1, 2, 3, 5).
fn slot(event: &StoreEvent) -> Option<(Discriminant<StoreEvent>, u32, Option<&str>)> {
    use StoreEvent as E;
    let kind = discriminant(event);
    match event {
        E::NodeInfo { id, .. }
        | E::NodeAudio { id, .. }
        | E::NodeLatency { id, .. }
        | E::DeviceInfo { id, .. }
        | E::DeviceProfiles { id, .. }
        | E::DeviceProfile { id, .. }
        | E::DeviceRoutes { id, .. }
        | E::DeviceActive { id, .. } => Some((kind, id.0, None)),
        E::Metadata { subject, key, .. } => Some((kind, *subject, Some(key.as_str()))),
        _ => None,
    }
}

impl Batch {
    /// An empty batch.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends `event`, coalescing it with what is already queued (VOL-ARCH-031): an event
    /// for the same slot replaces the earlier one, and a `*Removed` whose `*Added` is in
    /// this batch drops both and every update of that object in between. Survivors keep
    /// their relative order.
    pub fn push(&mut self, event: StoreEvent) {
        if let Some(new) = slot(&event) {
            self.events.retain(|queued| slot(queued) != Some(new));
        }
        if let Some((object, Some(false))) = owner(&event)
            && let Some(added) = self
                .events
                .iter()
                .rposition(|queued| owner(queued) == Some((object, Some(true))))
        {
            let mut index = 0;
            self.events.retain(|queued| {
                let keep = index < added || owner(queued).is_none_or(|(o, _)| o != object);
                index += 1;
                keep
            });
            return;
        }
        self.events.push(event);
    }

    /// Number of queued events after coalescing.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// True iff nothing is queued.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// True iff the batch reached [`FLUSH_THRESHOLD`] and must be flushed now.
    pub fn is_full(&self) -> bool {
        self.events.len() >= FLUSH_THRESHOLD
    }

    /// Removes and returns the queued events in application order.
    pub fn take(&mut self) -> Vec<StoreEvent> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::backend::StoreEvent;
    use crate::model::store::strategies;
    use crate::model::{Channel, NodeAudio, NodeState, ObjectId, PropMap, Serial, Store};

    fn audio(id: u32, gain: f32) -> StoreEvent {
        StoreEvent::NodeAudio {
            id: ObjectId(id),
            audio: NodeAudio {
                volume: 1.0,
                channel_volumes: vec![gain],
                mute: false,
                channel_map: vec![Channel(2)],
            },
        }
    }

    fn info(id: u32, state: NodeState) -> StoreEvent {
        StoreEvent::NodeInfo {
            id: ObjectId(id),
            props: PropMap::new(),
            state,
        }
    }

    fn meta(subject: u32, key: &str, value: &str) -> StoreEvent {
        StoreEvent::Metadata {
            subject,
            key: key.to_owned(),
            type_: None,
            value: Some(value.to_owned()),
        }
    }

    fn node_added(id: u32) -> StoreEvent {
        StoreEvent::NodeAdded {
            id: ObjectId(id),
            serial: Serial(u64::from(id)),
            props: PropMap::from([("media.class".to_owned(), "Audio/Sink".to_owned())]),
        }
    }

    fn node_removed(id: u32) -> StoreEvent {
        StoreEvent::NodeRemoved { id: ObjectId(id) }
    }

    fn coalesce(events: impl IntoIterator<Item = StoreEvent>) -> Vec<StoreEvent> {
        let mut batch = Batch::new();
        for event in events {
            batch.push(event);
        }
        let len = batch.len();
        let out = batch.take();
        assert_eq!(out.len(), len);
        assert!(batch.is_empty());
        out
    }

    #[test]
    fn consecutive_node_audio_keeps_last() {
        assert_eq!(
            coalesce([
                audio(40, 0.1),
                audio(40, 0.2),
                audio(41, 0.3),
                audio(40, 0.4)
            ]),
            [audio(41, 0.3), audio(40, 0.4)]
        );
    }

    #[test]
    fn node_info_keeps_last() {
        assert_eq!(
            coalesce([
                info(40, NodeState::Idle),
                info(41, NodeState::Idle),
                info(40, NodeState::Running),
            ]),
            [info(41, NodeState::Idle), info(40, NodeState::Running)]
        );
    }

    #[test]
    fn metadata_same_subject_key_keeps_last() {
        assert_eq!(
            coalesce([
                meta(0, "default.audio.sink", "a"),
                meta(0, "default.audio.sink", "b"),
            ]),
            [meta(0, "default.audio.sink", "b")]
        );
    }

    #[test]
    fn metadata_different_keys_both_kept() {
        let events = [
            meta(0, "default.audio.sink", "a"),
            meta(0, "default.configured.audio.sink", "a"),
            meta(50, "default.audio.sink", "a"),
        ];
        assert_eq!(coalesce(events.clone()), events);
    }

    #[test]
    fn added_then_removed_cancels_including_param_events() {
        let device = |id| StoreEvent::DeviceProfile {
            id: ObjectId(id),
            index: 1,
        };
        assert_eq!(
            coalesce([
                node_added(40),
                audio(40, 0.5),
                audio(41, 0.5),
                info(40, NodeState::Running),
                StoreEvent::NodeLatency {
                    id: ObjectId(40),
                    process_latency_ns: 1,
                },
                StoreEvent::DeviceAdded {
                    id: ObjectId(30),
                    serial: Serial(30),
                    props: PropMap::new(),
                },
                device(30),
                device(31),
                StoreEvent::DeviceRemoved { id: ObjectId(30) },
                node_removed(40),
            ]),
            [audio(41, 0.5), device(31)]
        );
    }

    #[test]
    fn removed_then_added_both_kept() {
        let events = [node_removed(40), node_added(40), audio(40, 0.5)];
        assert_eq!(coalesce(events.clone()), events);
    }

    #[test]
    fn order_of_survivors_preserved() {
        let port = StoreEvent::PortRemoved { id: ObjectId(60) };
        assert_eq!(
            coalesce([
                audio(40, 0.1),
                node_added(41),
                meta(0, "k", "a"),
                port.clone(),
                audio(40, 0.2),
                meta(0, "k", "b"),
                info(41, NodeState::Idle),
            ]),
            [
                node_added(41),
                port,
                audio(40, 0.2),
                meta(0, "k", "b"),
                info(41, NodeState::Idle),
            ]
        );
    }

    #[test]
    fn flush_threshold_256_reports_full() {
        let mut batch = Batch::new();
        assert!(batch.is_empty() && !batch.is_full());
        for id in 0..255 {
            batch.push(StoreEvent::PortRemoved { id: ObjectId(id) });
        }
        assert_eq!(batch.len(), 255);
        assert!(!batch.is_full());
        batch.push(StoreEvent::PortRemoved { id: ObjectId(255) });
        assert_eq!(batch.len(), FLUSH_THRESHOLD);
        assert!(batch.is_full());
        assert_eq!(batch.take().len(), 256);
        assert!(batch.is_empty() && !batch.is_full());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn coalesced_application_equals_sequential_application(
            events in strategies::events(),
            window in 1usize..=64,
        ) {
            let mut sequential = Store::default();
            for event in events.clone() {
                sequential.apply(event);
            }

            let mut coalesced = Store::default();
            for chunk in events.chunks(window) {
                let mut batch = Batch::new();
                for event in chunk.iter().cloned() {
                    batch.push(event);
                }
                for event in batch.take() {
                    coalesced.apply(event);
                }
            }

            coalesced.revision = sequential.revision;
            prop_assert_eq!(coalesced, sequential);
        }
    }
}
