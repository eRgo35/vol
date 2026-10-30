//! End-to-end scenarios against the mock backend: backend events travel
//! through the event channel into the app's store (VOL-ARCH-012, VOL-ARCH-020).
#![cfg(feature = "mock")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use ratatui::{buffer::Buffer, layout::Rect};
use tokio::sync::mpsc::unbounded_channel;
use vol::action::Action;
use vol::app::{App, Options};
use vol::backend::fixtures::default_system;
use vol::backend::mock::{MockControl, MockScript, disconnected, spawn_mock};
use vol::backend::{BackendEvent, DisconnectReason, StoreEvent};
use vol::event::EventHandler;
use vol::model::view::{RowKey, StreamFilter};
use vol::model::{ConnState, ObjectId, PropMap, Serial};
use vol::ui;

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

/// An app on the mock backend with `script` applied up to `Connected`, and
/// its event source.
async fn started(script: MockScript) -> (App, EventHandler, MockControl) {
    let (mut handle, control) = spawn_mock(script);
    let backend_events = std::mem::replace(&mut handle.events, unbounded_channel().1);
    let mut events = EventHandler::new(backend_events).unwrap();
    let options = Options {
        color: true,
        mouse: false,
    };
    let mut app = App::new(options, handle, Instant::now());
    app.resize(80, 24);
    run_until(&mut app, &mut events, |app| {
        matches!(app.store.conn, ConnState::Connected { .. })
    })
    .await;
    (app, events, control)
}

fn selected(app: &App) -> Option<RowKey> {
    app.ui.list(app.ui.tab).selected
}

fn stream(serial: u64) -> RowKey {
    RowKey::Stream(Serial(serial))
}

/// The 80x24 screen as text, one line per row.
fn screen(app: &App) -> String {
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    ui::render(app, area, &mut buf);
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn stream_added_and_removed_keeps_selection_by_serial() {
    let (mut app, mut events, control) = started(default_system()).await;
    // Firefox 412, Spotify 433, mpv 457; select Spotify.
    app.update(Action::RowNext);
    assert_eq!(selected(&app), Some(stream(433)));

    // A stream with a lower serial appears above it: Spotify stays selected.
    control
        .push(BackendEvent::Batch(vec![StoreEvent::NodeAdded {
            id: ObjectId(120),
            serial: Serial(400),
            props: props(&[
                ("media.class", "Stream/Output/Audio"),
                ("application.name", "Chromium"),
            ]),
        }]))
        .unwrap();
    run_until(&mut app, &mut events, |app| app.row_keys().len() == 4).await;
    assert_eq!(selected(&app), Some(stream(433)));
    let text = screen(&app);
    assert!(text.contains("▌● Spotify: Song Title"), "{text}");
    assert!(text.contains(" ○ Chromium"), "{text}");

    // Firefox goes away: still Spotify.
    control
        .push(BackendEvent::Batch(vec![StoreEvent::NodeRemoved {
            id: ObjectId(84),
        }]))
        .unwrap();
    run_until(&mut app, &mut events, |app| app.row_keys().len() == 3).await;
    assert_eq!(selected(&app), Some(stream(433)));
    assert!(!screen(&app).contains("Firefox"));

    // Spotify itself goes away: the row now at its index (mpv) is selected.
    control
        .push(BackendEvent::Batch(vec![StoreEvent::NodeRemoved {
            id: ObjectId(90),
        }]))
        .unwrap();
    run_until(&mut app, &mut events, |app| app.row_keys().len() == 2).await;
    assert_eq!(selected(&app), Some(stream(457)));
    let text = screen(&app);
    assert!(text.contains("▌○ mpv: video.mkv"), "{text}");
}

#[tokio::test]
async fn virtual_stream_hidden_in_applications_filter() {
    let (mut app, _events, _control) = started(default_system()).await;
    let loopback = stream(468);
    assert_eq!(app.ui.playback_filter, StreamFilter::Applications);
    assert_eq!(app.row_keys(), [stream(412), stream(433), stream(457)]);
    assert!(!screen(&app).contains("pw-loopback"));

    app.update(Action::FilterNext);
    assert_eq!(app.ui.playback_filter, StreamFilter::All);
    assert_eq!(app.row_keys().last(), Some(&loopback));
    let text = screen(&app);
    assert!(
        text.contains("pw-loopback: loopback-2841-13 output"),
        "{text}"
    );

    app.update(Action::FilterNext);
    assert_eq!(app.ui.playback_filter, StreamFilter::Virtual);
    assert_eq!(app.row_keys(), [loopback]);
    let text = screen(&app);
    assert!(text.contains("▌● pw-loopback"), "{text}");
}
