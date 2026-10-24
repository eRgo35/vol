//! L5 live tests against a **private** PipeWire daemon (VOL-QA-007).
//!
//! # Running
//!
//! `just test-live` (= `env -u PIPEWIRE_REMOTE -u PIPEWIRE_RUNTIME_DIR -u
//! XDG_RUNTIME_DIR VOL_TEST_PIPEWIRE=1 cargo test --test live -- --ignored
//! --test-threads=1`). The in-process backend is only ever given the private
//! daemon's socket path, and tests refuse to run while any of those three
//! variables could redirect it. Every test is `#[ignore]`d, so `just check`
//! never runs them, and each returns early unless `VOL_TEST_PIPEWIRE=1`.
//! Needs `pipewire` and `pw-cli`/`pw-dump` on `PATH`; `wireplumber` is
//! optional (tests that need metadata skip without it).
//!
//! Each test starts its own daemon with `common::PrivateDaemon::start`, which
//! creates `$TMPDIR/vol-pw-<pid>-<n>/`, spawns `pipewire` (and `wireplumber`)
//! there, checks with `pw-cli info 0` that the socket is served by that child,
//! and on drop kills the daemons and removes the directory. The user's own
//! session is never contacted: every PipeWire process gets `env_clear()` plus
//! `PrivateDaemon::env()`.
//!
//! # Verified environment recipe (PipeWire 1.6.9, WirePlumber 0.5.18, 2026-10-24)
//!
//! * Socket location: the daemon creates `pipewire-0` (+ `pipewire-0-manager`)
//!   in `$PIPEWIRE_RUNTIME_DIR`, else `$XDG_RUNTIME_DIR`. Clients resolve
//!   `$PIPEWIRE_REMOTE` (an absolute socket path works), else
//!   `$PIPEWIRE_RUNTIME_DIR`, else `$XDG_RUNTIME_DIR`; each one alone was
//!   enough. With none of them set the client fails with "Host is down"
//!   (no fallback to `/run/user/<uid>`). The helper sets both runtime vars.
//! * `PIPEWIRE_REMOTE` beats an explicit remote: with it set to a bogus path,
//!   `pw-cli -r <private socket>` fails. In-process clients must therefore run
//!   with `PIPEWIRE_REMOTE` unset (or pointing at the private socket).
//! * The socket path must fit `sun_path` (≤ 107 bytes): a long `TMPDIR`
//!   makes the daemon exit with "File name too long".
//! * D-Bus: both bus addresses point at a missing socket. `pipewire` then
//!   logs errors for rtkit, the portal module and RAOP discovery and runs
//!   fine; this also keeps network sinks (RAOP/Avahi) out of the session.
//! * `XDG_CONFIG_HOME`, `XDG_STATE_HOME` and `HOME` point inside the temp
//!   dir so neither the user's config drop-ins are read nor WirePlumber's
//!   restore state (`state/wireplumber/…`) is written to the real home.
//! * WirePlumber runs privately without a seat or D-Bus (optional D-Bus,
//!   portal, logind and MPRIS components are skipped). Its hardware monitors
//!   are disabled by a config drop-in (`hardware.audio`/`bluetooth`/
//!   `video-capture = disabled`) so it never opens the real sound cards. It
//!   still provides the `default` metadata and sets `default.audio.sink` to
//!   the null sink.
//! * Null sink: `pw-cli create-node adapter '{ factory.name=support.null-audio-sink
//!   node.name=<n> media.class=Audio/Sink object.linger=true audio.position=[FL FR] }'`;
//!   `object.linger` keeps it after `pw-cli` exits.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::io::Write;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

use common::{PrivateDaemon, require_live};
use tokio::sync::mpsc::error::TryRecvError;
use vol::backend::pw::spawn_pipewire_at;
use vol::backend::{
    BackendEvent, BackendGone, BackendHandle, Command, DisconnectReason, StoreEvent,
};
use vol::model::{ObjectId, PortDir};

/// Spawns the in-process backend on the socket at `path`, and only there.
///
/// `PIPEWIRE_REMOTE` would take precedence over the explicit socket, and the
/// runtime-dir variables are where a default lookup would find the user's
/// session, so any of them being set fails the test before connecting.
fn spawn_at(path: PathBuf) -> BackendHandle {
    for var in ["PIPEWIRE_REMOTE", "PIPEWIRE_RUNTIME_DIR", "XDG_RUNTIME_DIR"] {
        assert!(
            std::env::var_os(var).is_none(),
            "{var} is set; run `just test-live`, which unsets it, so the backend \
             cannot reach a session other than the private daemon"
        );
    }
    vol::log::init().unwrap();
    spawn_pipewire_at(path).unwrap()
}

/// Name of the null sink every connecting test creates.
const SINK: &str = "vol-test-sink";

/// A private daemon (with WirePlumber if asked) holding the null sink
/// [`SINK`], and a backend connected to it. Returns the store events received
/// up to `Connected`, after checking they come from the private daemon.
fn start_private(with_wireplumber: bool) -> (PrivateDaemon, BackendHandle, Vec<StoreEvent>) {
    let daemon = PrivateDaemon::start(with_wireplumber);
    daemon.create_null_sink(SINK);
    let needle = format!("\"node.name\": \"{SINK}\"");
    let dump = wait_for_dump(&daemon, Duration::from_secs(3), |d| d.contains(&needle));
    assert!(dump.contains(&needle), "{dump}");
    let mut handle = spawn_at(daemon.socket());
    let snapshot = wait_connected(&mut handle, &daemon);
    assert_private(&snapshot);
    (daemon, handle, snapshot)
}

/// The snapshot must hold the test's null sink and no hardware device:
/// otherwise the backend is not talking to the private daemon.
fn assert_private(snapshot: &[StoreEvent]) {
    assert!(
        snapshot.iter().any(|e| matches!(
            e,
            StoreEvent::NodeAdded { props, .. }
                if props.get("node.name").map(String::as_str) == Some(SINK)
        )),
        "the private null sink is missing: {snapshot:?}"
    );
    let hardware: Vec<_> = snapshot
        .iter()
        .filter(|e| {
            matches!(
                e,
                StoreEvent::DeviceAdded { props, .. }
                    if props.get("media.class").map(String::as_str) == Some("Audio/Device")
            )
        })
        .collect();
    assert!(hardware.is_empty(), "not the private daemon: {hardware:?}");
}

/// Next backend event within `timeout`; `None` on timeout or when the
/// backend has closed its channel.
fn next_event(handle: &mut BackendHandle, timeout: Duration) -> Option<BackendEvent> {
    let deadline = Instant::now() + timeout;
    loop {
        match handle.events.try_recv() {
            Ok(event) => return Some(event),
            Err(TryRecvError::Disconnected) => return None,
            Err(TryRecvError::Empty) if Instant::now() >= deadline => return None,
            Err(TryRecvError::Empty) => sleep(Duration::from_millis(5)),
        }
    }
}

/// Waits (≤ 3 s) for `Connected`, checks it names the private socket and
/// returns the store events received before it, in order.
fn wait_connected(handle: &mut BackendHandle, daemon: &PrivateDaemon) -> Vec<StoreEvent> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut snapshot = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match next_event(handle, left) {
            Some(BackendEvent::Connected { socket, .. }) => {
                assert_eq!(socket, daemon.socket().display().to_string());
                return snapshot;
            }
            Some(BackendEvent::Batch(events)) => snapshot.extend(events),
            Some(other) => panic!("unexpected before Connected: {other:?}"),
            None => panic!("no Connected within 3 s"),
        }
    }
}

/// Appends store events to `events` until `done(events)` holds (≤ 3 s).
fn poll_until(
    handle: &mut BackendHandle,
    events: &mut Vec<StoreEvent>,
    done: impl Fn(&[StoreEvent]) -> bool,
) -> bool {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !done(events) {
        let left = deadline.saturating_duration_since(Instant::now());
        match next_event(handle, left) {
            Some(BackendEvent::Batch(batch)) => events.extend(batch),
            Some(other) => panic!("unexpected event: {other:?}"),
            None => return false,
        }
    }
    true
}

/// Drains events until the backend closes its channel (its thread is gone).
fn wait_closed(handle: &mut BackendHandle, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        match handle.events.try_recv() {
            Err(TryRecvError::Disconnected) => return,
            Ok(_) | Err(TryRecvError::Empty) => {
                assert!(Instant::now() < deadline, "backend thread still running");
                sleep(Duration::from_millis(5));
            }
        }
    }
}

/// Polls `pw-dump` until `pred` holds, for at most `timeout`.
fn wait_for_dump(daemon: &PrivateDaemon, timeout: Duration, pred: impl Fn(&str) -> bool) -> String {
    let deadline = Instant::now() + timeout;
    loop {
        let dump = daemon.pw_dump();
        if pred(&dump) || Instant::now() >= deadline {
            return dump;
        }
        sleep(Duration::from_millis(50));
    }
}

#[test]
#[ignore = "live: needs PipeWire; run with `just test-live`"]
fn private_daemon_starts_and_has_null_sink() {
    if !require_live() {
        return;
    }
    let mut daemon = PrivateDaemon::start(true);
    daemon.create_null_sink("vol-test-sink");

    let dump = wait_for_dump(&daemon, Duration::from_secs(3), |d| {
        d.contains("\"node.name\": \"vol-test-sink\"")
    });
    assert!(dump.contains("\"node.name\": \"vol-test-sink\""), "{dump}");
    assert!(dump.contains("\"media.class\": \"Audio/Sink\""), "{dump}");
    assert!(
        !dump.contains("\"media.class\": \"Audio/Device\""),
        "real hardware leaked into the private session: {dump}"
    );

    if daemon.has_wireplumber() {
        let dump = wait_for_dump(&daemon, Duration::from_secs(3), |d| {
            d.contains("\"default.audio.sink\"")
        });
        assert!(
            dump.contains(r#""key": "default.audio.sink", "type": "Spa:String:JSON", "value": { "name": "vol-test-sink" }"#),
            "{dump}"
        );
    }
}

#[test]
#[ignore = "live: needs PipeWire; run with `just test-live`"]
fn spawn_and_shutdown_within_one_second() {
    if !require_live() {
        return;
    }
    let (_daemon, handle, _) = start_private(false);
    let started = Instant::now();
    assert!(
        handle.shutdown(Duration::from_secs(1)),
        "not joined in time"
    );
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
#[ignore = "live: needs PipeWire; run with `just test-live`"]
fn sending_after_shutdown_returns_backend_gone() {
    if !require_live() {
        return;
    }
    let (_daemon, mut handle, _) = start_private(false);
    assert_eq!(handle.commands.send(Command::Shutdown), Ok(()));
    wait_closed(&mut handle, Duration::from_secs(1));
    let mute = Command::SetMute {
        node: ObjectId(1),
        mute: true,
    };
    assert_eq!(handle.commands.send(mute), Err(BackendGone));
}

#[test]
#[ignore = "live: needs PipeWire; run with `just test-live`"]
fn no_daemon_reports_disconnected_no_daemon() {
    if !require_live() {
        return;
    }
    // A socket path in a directory that does not exist: nothing listens there.
    let missing = std::env::temp_dir()
        .join(format!("vol-no-daemon-{}", std::process::id()))
        .join("pipewire-0");
    assert!(!missing.exists());
    let mut handle = spawn_at(missing);
    let event = next_event(&mut handle, Duration::from_secs(1));
    assert!(
        matches!(
            event,
            Some(BackendEvent::Disconnected {
                reason: DisconnectReason::NoDaemon,
                ..
            })
        ),
        "{event:?}"
    );
    assert!(handle.shutdown(Duration::from_secs(1)));
}

#[test]
#[ignore = "live: needs PipeWire; run with `just test-live`"]
fn registry_snapshot_contains_a_sink() {
    if !require_live() {
        return;
    }
    let (mut daemon, mut handle, snapshot) = start_private(true);
    let sink = snapshot.iter().find_map(|e| match e {
        StoreEvent::NodeAdded { id, props, .. }
            if props.get("node.name").map(String::as_str) == Some(SINK) =>
        {
            Some((*id, props.clone()))
        }
        _ => None,
    });
    let (id, props) = sink.expect("NodeAdded for the null sink");
    assert_eq!(props["media.class"], "Audio/Sink");
    let mut events = snapshot;
    let is_info = |e: &StoreEvent| {
        matches!(
            e,
            StoreEvent::NodeInfo { id: info_id, props, .. }
                if *info_id == id && props.contains_key("node.name")
        )
    };
    assert!(
        poll_until(&mut handle, &mut events, |all| all.iter().any(is_info)),
        "no NodeInfo for the sink: {events:?}"
    );
    // The adapter gets its ports only once a session manager configures it.
    if daemon.has_wireplumber() {
        let inputs = |all: &[StoreEvent]| {
            all.iter()
                .filter(|e| {
                    matches!(
                        e,
                        StoreEvent::PortAdded { node, dir: PortDir::In, .. } if *node == id
                    )
                })
                .count()
        };
        assert!(
            poll_until(&mut handle, &mut events, |all| inputs(all) == 2),
            "expected FL/FR input ports: {events:?}"
        );
    }
    assert!(handle.shutdown(Duration::from_secs(1)));
}

#[test]
#[ignore = "live: needs PipeWire; run with `just test-live`"]
fn connected_arrives_after_snapshot_with_params() {
    if !require_live() {
        return;
    }
    let (_daemon, handle, snapshot) = start_private(false);
    let sink = snapshot
        .iter()
        .position(|e| {
            matches!(
                e,
                StoreEvent::NodeAdded { props, .. }
                    if props.get("node.name").map(String::as_str) == Some(SINK)
            )
        })
        .expect("NodeAdded for the null sink");
    let StoreEvent::NodeAdded { id, .. } = snapshot[sink] else {
        unreachable!()
    };
    // Volume state is part of the snapshot, after its NodeAdded (VOL-ARCH-023).
    let audio = snapshot[sink..].iter().find_map(|e| match e {
        StoreEvent::NodeAudio {
            id: audio_id,
            audio,
        } if *audio_id == id => Some(audio),
        _ => None,
    });
    let audio = audio.unwrap_or_else(|| panic!("no NodeAudio before Connected: {snapshot:?}"));
    assert_eq!(audio.channel_volumes.len(), 2, "{audio:?}");
    assert_eq!(audio.channel_map.len(), 2, "{audio:?}");
    assert!(snapshot[sink..].iter().any(|e| matches!(
        e,
        StoreEvent::NodeInfo { id: info_id, .. } if *info_id == id
    )));
    assert!(handle.shutdown(Duration::from_secs(1)));
}

/// True if `events` set `default.audio.sink` to the test's null sink.
fn sets_default_sink(events: &[StoreEvent]) -> bool {
    events.iter().any(|e| {
        matches!(
            e,
            StoreEvent::Metadata { subject: 0, key, value: Some(value), .. }
                if key == "default.audio.sink" && value.contains(SINK)
        )
    })
}

/// Notes on stderr that a WirePlumber-dependent test was skipped.
fn skip_without_wireplumber() {
    let _ = writeln!(
        std::io::stderr(),
        "skipped: wireplumber is not available for the private session"
    );
}

#[test]
#[ignore = "live: needs PipeWire; run with `just test-live`"]
fn default_metadata_exposes_default_audio_sink() {
    if !require_live() {
        return;
    }
    let (mut daemon, mut handle, mut events) = start_private(true);
    if !daemon.has_wireplumber() {
        skip_without_wireplumber();
        return;
    }
    assert!(
        poll_until(&mut handle, &mut events, sets_default_sink),
        "no default.audio.sink: {events:?}"
    );
    assert!(handle.shutdown(Duration::from_secs(1)));
}

#[test]
#[ignore = "live: needs PipeWire; run with `just test-live`"]
fn wireplumber_restart_clears_and_rebinds_default_metadata() {
    if !require_live() {
        return;
    }
    let (mut daemon, mut handle, mut events) = start_private(true);
    if !daemon.has_wireplumber() {
        skip_without_wireplumber();
        return;
    }
    assert!(poll_until(&mut handle, &mut events, sets_default_sink));
    let before = events.len();
    assert!(daemon.restart_wireplumber());
    let cleared_then_set = |all: &[StoreEvent]| {
        let after = &all[before..];
        after
            .iter()
            .position(|e| *e == StoreEvent::MetadataCleared)
            .is_some_and(|i| sets_default_sink(&after[i..]))
    };
    assert!(
        poll_until(&mut handle, &mut events, cleared_then_set),
        "{:?}",
        &events[before..]
    );
    assert!(handle.shutdown(Duration::from_secs(1)));
}
