//! End-to-end scenarios against the mock backend: backend events travel
//! through the event channel into the app's store (VOL-ARCH-012, VOL-ARCH-020).
#![cfg(feature = "mock")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use tokio::sync::mpsc::unbounded_channel;
use vol::app::{App, Options};
use vol::backend::mock::{disconnected, spawn_mock};
use vol::backend::{BackendEvent, DisconnectReason, StoreEvent};
use vol::event::EventHandler;
use vol::model::{ConnState, ObjectId, PropMap, Serial};

const TIMEOUT: Duration = Duration::from_secs(2);

fn props(pairs: &[(&str, &str)]) -> PropMap {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

/// Feeds events into `app` until `done` holds.
async fn run_until(app: &mut App, events: &mut EventHandler, done: impl Fn(&App) -> bool) {
    tokio::time::timeout(TIMEOUT, async {
        while !done(app) {
            app.on_event(events.next().await.unwrap());
        }
    })
    .await
    .expect("timed out waiting for the store");
}

#[tokio::test]
async fn startup_with_disconnected_then_connected_populates_store() {
    let (mut handle, control) = spawn_mock(disconnected(DisconnectReason::NoDaemon));
    let backend_events = std::mem::replace(&mut handle.events, unbounded_channel().1);
    let mut events = EventHandler::new(backend_events).unwrap();
    let options = Options {
        color: true,
        mouse: false,
    };
    let mut app = App::new(options, handle, Instant::now());

    run_until(&mut app, &mut events, |app| {
        matches!(app.store.conn, ConnState::Disconnected { .. })
    })
    .await;
    assert!(app.store.nodes.is_empty());

    // The backend sends the initial snapshot, then `Connected`.
    control
        .push(BackendEvent::Batch(vec![
            StoreEvent::NodeAdded {
                id: ObjectId(40),
                serial: Serial(140),
                props: props(&[("media.class", "Audio/Sink"), ("node.name", "speakers")]),
            },
            StoreEvent::DeviceAdded {
                id: ObjectId(30),
                serial: Serial(130),
                props: props(&[("media.class", "Audio/Device")]),
            },
            StoreEvent::Metadata {
                subject: 0,
                key: "default.audio.sink".to_owned(),
                type_: Some("Spa:String:JSON".to_owned()),
                value: Some(r#"{"name":"speakers"}"#.to_owned()),
            },
        ]))
        .unwrap();
    control
        .push(BackendEvent::Connected {
            server_version: "1.6.9".to_owned(),
            socket: "pipewire-0".to_owned(),
        })
        .unwrap();
    run_until(&mut app, &mut events, |app| {
        matches!(app.store.conn, ConnState::Connected { .. })
    })
    .await;

    assert_eq!(app.store.nodes.len(), 1);
    assert_eq!(app.store.devices.len(), 1);
    assert_eq!(app.store.defaults.sink.as_deref(), Some("speakers"));
    assert!(app.needs_draw());
}
